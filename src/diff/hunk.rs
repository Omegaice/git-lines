use nom::{
    IResult, Parser,
    bytes::complete::{tag, take_until},
    character::complete::{digit1, line_ending, not_line_ending},
    combinator::{map_res, opt, value},
    multi::fold_many0,
    sequence::{delimited, pair, preceded, separated_pair, terminated},
};
use std::fmt;

/// Lines modified in the old or new version of a file.
///
/// Represents either deletions (old lines) or additions (new lines) within a hunk.
#[derive(Debug, PartialEq, Eq)]
pub struct ModifiedLines {
    /// Starting line number (1-indexed)
    pub start: u32,
    /// The actual line content (without +/- prefix)
    pub lines: Vec<String>,
    /// Whether the last line lacks a trailing newline
    pub missing_final_newline: bool,
}

/// A single hunk from a git diff.
///
/// A hunk represents one contiguous block of changes in a file. With `-U0`
/// (zero context lines), each hunk contains only modified lines with no
/// surrounding context.
///
/// # Structure
///
/// - `old`: Lines that were deleted (prefixed with `-` in diff)
/// - `new`: Lines that were added (prefixed with `+` in diff)
///
/// # Hunk Types
///
/// - Pure addition: `old.lines` is empty
/// - Pure deletion: `new.lines` is empty
/// - Replacement: Both `old` and `new` have lines
#[derive(Debug, PartialEq, Eq)]
pub struct Hunk {
    /// Lines from the old version (deletions)
    pub old: ModifiedLines,
    /// Lines from the new version (additions)
    pub new: ModifiedLines,
}

impl Hunk {
    /// Parse a hunk from diff text (header + content lines).
    ///
    /// Expects text starting with `@@ -old +new @@` header followed by
    /// content lines prefixed with `-`, `+`, or `\` (for no-newline marker).
    ///
    /// Returns `None` if parsing fails.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        parse_hunk(text).ok().map(|(_, hunk)| hunk)
    }

    /// Filter lines in the hunk, returning a new hunk with only matching lines.
    ///
    /// # Parameters
    ///
    /// - `keep_old`: Predicate for old lines (deletions). Called with old line number.
    /// - `keep_new`: Predicate for new lines (additions). Called with new line number.
    ///
    /// # Returns
    ///
    /// - `Some(Hunk)` with only the lines where predicates returned `true`
    /// - `None` if no lines matched either predicate
    ///
    /// # Line Number Recalculation
    ///
    /// When filtering to pure additions (no deletions kept), the new start position
    /// is recalculated as `old_start + 1` since the insertion appears right after
    /// the old position.
    ///
    /// # No-Newline Bridge Synthesis
    ///
    /// If the old lines had no trailing newline and you're keeping additions after it,
    /// the method automatically includes the old deletion to provide the required
    /// newline separator. This prevents corrupted git index state.
    #[must_use]
    pub fn retain<F, G>(&self, keep_old: F, keep_new: G) -> Option<Self>
    where
        F: FnMut(u32) -> bool,
        G: FnMut(u32) -> bool,
    {
        // Phase 1: Filter lines
        let mut old_filtered = filter_lines(&self.old, keep_old);
        let mut new_filtered = filter_lines(&self.new, keep_new);

        if old_filtered.is_empty() && new_filtered.is_empty() {
            return None;
        }

        // Phase 2: Insert separator if needed
        // When the original last line had no newline and we're adding content after it,
        // we must include that line (deleted then re-added) to provide line separation
        if requires_line_separator(&self.old, &new_filtered) {
            insert_line_separator(&self.old, &mut old_filtered, &mut new_filtered);
        }

        // Phase 3: Calculate positions based on change type
        let old_start = old_filtered.first_line_num.unwrap_or(self.old.start);

        // Check if we kept all lines (idempotence case)
        // Only applies to mixed hunks - pure insertions/deletions always recalculate
        let kept_all = !self.old.lines.is_empty()
            && !self.new.lines.is_empty()
            && old_filtered.lines.len() == self.old.lines.len()
            && new_filtered.lines.len() == self.new.lines.len();

        let new_start = if kept_all {
            // Preserve original position when nothing was filtered from a mixed hunk
            self.new.start
        } else {
            // Recalculate for filtered subset or pure insertion/deletion
            match change_type(&old_filtered, &new_filtered) {
                ChangeType::PureInsertion => old_start + 1,
                ChangeType::PureDeletion => old_start,
                ChangeType::Mixed => new_filtered.first_line_num.unwrap_or(self.new.start),
            }
        };

        // Phase 4: Assemble result
        Some(Hunk {
            old: ModifiedLines {
                start: old_start,
                lines: old_filtered.lines,
                missing_final_newline: old_filtered.kept_last_boundary
                    && self.old.missing_final_newline,
            },
            new: ModifiedLines {
                start: new_start,
                lines: new_filtered.lines,
                missing_final_newline: new_filtered.kept_last_boundary
                    && self.new.missing_final_newline,
            },
        })
    }
}

/// Result of filtering lines, tracking boundary alignment with the original
struct FilterResult {
    lines: Vec<String>,
    first_line_num: Option<u32>,
    kept_first_boundary: bool,
    kept_last_boundary: bool,
}

impl FilterResult {
    fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }
}

/// Filter lines from a ModifiedLines based on a predicate
fn filter_lines<F>(source: &ModifiedLines, mut keep: F) -> FilterResult
where
    F: FnMut(u32) -> bool,
{
    let mut result = FilterResult {
        lines: Vec::new(),
        first_line_num: None,
        kept_first_boundary: false,
        kept_last_boundary: false,
    };

    let last_idx = source.lines.len().saturating_sub(1);

    for (i, line) in source.lines.iter().enumerate() {
        let line_num = source.start + i as u32;
        if keep(line_num) {
            if result.first_line_num.is_none() {
                result.first_line_num = Some(line_num);
            }
            result.lines.push(line.clone());
            if i == 0 {
                result.kept_first_boundary = true;
            }
            if i == last_idx {
                result.kept_last_boundary = true;
            }
        }
    }

    result
}

/// Check if we need to insert a line separator
///
/// This occurs when: the original deletions had no trailing newline,
/// we have additions to keep, but we didn't keep the first addition.
/// Without the separator, new content would concatenate onto the previous line.
fn requires_line_separator(old_source: &ModifiedLines, new_filtered: &FilterResult) -> bool {
    old_source.missing_final_newline
        && !new_filtered.is_empty()
        && !new_filtered.kept_first_boundary
}

/// Insert a line separator by including bridge content
///
/// Forces inclusion of the last deletion (if not already kept) and
/// synthesizes the first addition with the same content, providing
/// the newline that separates subsequent additions.
fn insert_line_separator(
    old_source: &ModifiedLines,
    old_filtered: &mut FilterResult,
    new_filtered: &mut FilterResult,
) {
    let Some(last_old_line) = old_source.lines.last() else {
        return;
    };

    // Include the last deletion if not already kept
    if !old_filtered.kept_last_boundary {
        let last_idx = old_source.lines.len() - 1;
        let last_line_num = old_source.start + last_idx as u32;

        old_filtered.lines.push(last_old_line.clone());
        if old_filtered.first_line_num.is_none() {
            old_filtered.first_line_num = Some(last_line_num);
        }
        old_filtered.kept_last_boundary = true;
    }

    // Synthesize the first addition with the old content (provides the newline)
    new_filtered.lines.insert(0, last_old_line.clone());
    new_filtered.first_line_num = Some(old_source.start + old_source.lines.len() as u32);
    new_filtered.kept_first_boundary = true;
}

/// The type of change after filtering
enum ChangeType {
    PureInsertion,
    PureDeletion,
    Mixed,
}

/// Determine what type of change the filtered result represents
fn change_type(old_filtered: &FilterResult, new_filtered: &FilterResult) -> ChangeType {
    match (old_filtered.is_empty(), new_filtered.is_empty()) {
        (true, false) => ChangeType::PureInsertion,
        (false, true) => ChangeType::PureDeletion,
        _ => ChangeType::Mixed,
    }
}

// Nom parser combinators for hunk parsing

fn header_marker(input: &str) -> IResult<&str, &str> {
    delimited(tag("@@ "), take_until(" @@"), tag(" @@")).parse(input)
}

fn header_range(input: &str) -> IResult<&str, u32> {
    map_res(
        pair(digit1, opt(preceded(tag(","), digit1))),
        |(start, _count): (&str, Option<&str>)| start.parse::<u32>(),
    )
    .parse(input)
}

fn hunk_header(input: &str) -> IResult<&str, (u32, u32)> {
    let (rest, inner) = header_marker(input)?;
    let (_, (old_start, new_start)) = separated_pair(
        preceded(tag("-"), header_range),
        tag(" "),
        preceded(tag("+"), header_range),
    )
    .parse(inner)?;
    Ok((rest, (old_start, new_start)))
}

fn deletion_line(input: &str) -> IResult<&str, &str> {
    preceded(tag("-"), terminated(not_line_ending, opt(line_ending))).parse(input)
}

fn addition_line(input: &str) -> IResult<&str, &str> {
    preceded(tag("+"), terminated(not_line_ending, opt(line_ending))).parse(input)
}

fn no_newline_marker(input: &str) -> IResult<&str, bool> {
    value(
        true,
        pair(tag("\\ No newline at end of file"), opt(line_ending)),
    )
    .parse(input)
}

fn parse_hunk(input: &str) -> IResult<&str, Hunk> {
    // Parse header
    let (rest, (old_start, new_start)) =
        terminated(hunk_header, pair(not_line_ending, line_ending)).parse(input)?;

    // Collect deletions
    let (rest, old_lines) = fold_many0(deletion_line, Vec::new, |mut acc, line| {
        acc.push(line.into());
        acc
    })
    .parse(rest)?;

    let (rest, old_no_newline) = opt(no_newline_marker).parse(rest)?;

    // Collect additions
    let (rest, new_lines) = fold_many0(addition_line, Vec::new, |mut acc, line| {
        acc.push(line.into());
        acc
    })
    .parse(rest)?;

    let (rest, new_no_newline) = opt(no_newline_marker).parse(rest)?;

    Ok((
        rest,
        Hunk {
            old: ModifiedLines {
                start: old_start,
                lines: old_lines,
                missing_final_newline: old_no_newline.unwrap_or(false),
            },
            new: ModifiedLines {
                start: new_start,
                lines: new_lines,
                missing_final_newline: new_no_newline.unwrap_or(false),
            },
        },
    ))
}

impl fmt::Display for Hunk {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Build header
        let old_part = match self.old.lines.len() {
            0 => format!("-{},0", self.old.start),
            1 => format!("-{}", self.old.start),
            n => format!("-{},{}", self.old.start, n),
        };

        let new_part = match self.new.lines.len() {
            0 => format!("+{},0", self.new.start),
            1 => format!("+{}", self.new.start),
            n => format!("+{},{}", self.new.start, n),
        };

        writeln!(f, "@@ {} {} @@", old_part, new_part)?;

        // Add deletion lines
        for line in &self.old.lines {
            writeln!(f, "-{}", line)?;
        }
        if self.old.missing_final_newline {
            writeln!(f, "\\ No newline at end of file")?;
        }

        // Add addition lines
        for line in &self.new.lines {
            writeln!(f, "+{}", line)?;
        }
        if self.new.missing_final_newline {
            writeln!(f, "\\ No newline at end of file")?;
        }

        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use similar_asserts::assert_eq;

    #[test]
    fn render_pure_insertion() {
        let hunk = Hunk {
            old: ModifiedLines {
                start: 10,
                lines: vec![],
                missing_final_newline: false,
            },
            new: ModifiedLines {
                start: 11,
                lines: vec!["new line here".to_string()],
                missing_final_newline: false,
            },
        };
        assert_eq!(hunk.to_string(), "@@ -10,0 +11 @@\n+new line here\n");
    }

    #[test]
    fn render_pure_deletion() {
        let hunk = Hunk {
            old: ModifiedLines {
                start: 10,
                lines: vec!["old line removed".to_string()],
                missing_final_newline: false,
            },
            new: ModifiedLines {
                start: 9,
                lines: vec![],
                missing_final_newline: false,
            },
        };
        assert_eq!(hunk.to_string(), "@@ -10 +9,0 @@\n-old line removed\n");
    }

    #[test]
    fn render_single_line_replacement() {
        let hunk = Hunk {
            old: ModifiedLines {
                start: 10,
                lines: vec!["old version".to_string()],
                missing_final_newline: false,
            },
            new: ModifiedLines {
                start: 10,
                lines: vec!["new version".to_string()],
                missing_final_newline: false,
            },
        };
        assert_eq!(
            hunk.to_string(),
            "@@ -10 +10 @@\n-old version\n+new version\n"
        );
    }

    #[test]
    fn render_multi_line_change() {
        let hunk = Hunk {
            old: ModifiedLines {
                start: 10,
                lines: vec!["first old line".to_string(), "second old line".to_string()],
                missing_final_newline: false,
            },
            new: ModifiedLines {
                start: 10,
                lines: vec![
                    "first new line".to_string(),
                    "second new line".to_string(),
                    "third new line".to_string(),
                ],
                missing_final_newline: false,
            },
        };
        assert_eq!(
            hunk.to_string(),
            "@@ -10,2 +10,3 @@\n-first old line\n-second old line\n+first new line\n+second new line\n+third new line\n"
        );
    }

    #[test]
    fn render_multiple_insertions() {
        let hunk = Hunk {
            old: ModifiedLines {
                start: 5,
                lines: vec![],
                missing_final_newline: false,
            },
            new: ModifiedLines {
                start: 6,
                lines: vec!["line one".to_string(), "line two".to_string()],
                missing_final_newline: false,
            },
        };
        assert_eq!(hunk.to_string(), "@@ -5,0 +6,2 @@\n+line one\n+line two\n");
    }

    #[test]
    fn render_multiple_deletions() {
        let hunk = Hunk {
            old: ModifiedLines {
                start: 15,
                lines: vec!["removed one".to_string(), "removed two".to_string()],
                missing_final_newline: false,
            },
            new: ModifiedLines {
                start: 14,
                lines: vec![],
                missing_final_newline: false,
            },
        };
        assert_eq!(
            hunk.to_string(),
            "@@ -15,2 +14,0 @@\n-removed one\n-removed two\n"
        );
    }

    #[test]
    fn parse_pure_insertion() {
        let input = "@@ -10,0 +11 @@\n+new line here";

        let expected = Hunk {
            old: ModifiedLines {
                start: 10,
                lines: vec![],
                missing_final_newline: false,
            },
            new: ModifiedLines {
                start: 11,
                lines: vec!["new line here".to_string()],
                missing_final_newline: false,
            },
        };

        let actual = Hunk::parse(input).unwrap();
        assert_eq!(actual, expected);
    }

    #[test]
    fn parse_pure_deletion() {
        let input = "@@ -10 +9,0 @@\n-old line removed";

        let expected = Hunk {
            old: ModifiedLines {
                start: 10,
                lines: vec!["old line removed".to_string()],
                missing_final_newline: false,
            },
            new: ModifiedLines {
                start: 9,
                lines: vec![],
                missing_final_newline: false,
            },
        };

        let actual = Hunk::parse(input).unwrap();
        assert_eq!(actual, expected);
    }

    #[test]
    fn parse_mixed_change() {
        let input =
            "@@ -10,2 +10,3 @@\n-first old\n-second old\n+first new\n+second new\n+third new";

        let expected = Hunk {
            old: ModifiedLines {
                start: 10,
                lines: vec!["first old".to_string(), "second old".to_string()],
                missing_final_newline: false,
            },
            new: ModifiedLines {
                start: 10,
                lines: vec![
                    "first new".to_string(),
                    "second new".to_string(),
                    "third new".to_string(),
                ],
                missing_final_newline: false,
            },
        };

        let actual = Hunk::parse(input).unwrap();
        assert_eq!(actual, expected);
    }

    #[test]
    fn roundtrip_pure_insertion() {
        let original = "@@ -10,0 +11 @@\n+new line here\n";
        let hunk = Hunk::parse(original).unwrap();
        assert_eq!(hunk.to_string(), original);
    }

    #[test]
    fn roundtrip_mixed_change() {
        let original =
            "@@ -10,2 +10,3 @@\n-first old\n-second old\n+first new\n+second new\n+third new\n";
        let hunk = Hunk::parse(original).unwrap();
        assert_eq!(hunk.to_string(), original);
    }

    #[test]
    fn retain_single_addition_from_mixed() {
        let hunk = Hunk {
            old: ModifiedLines {
                start: 10,
                lines: vec!["deleted one".to_string(), "deleted two".to_string()],
                missing_final_newline: false,
            },
            new: ModifiedLines {
                start: 10,
                lines: vec![
                    "added one".to_string(),
                    "added two".to_string(),
                    "added three".to_string(),
                ],
                missing_final_newline: false,
            },
        };

        let filtered = hunk.retain(|_| false, |n| n == 12).unwrap();

        // When filtering to only additions (no deletions), new_start is recalculated
        // as old_start + 1, since insertions appear right after the old position
        let expected = Hunk {
            old: ModifiedLines {
                start: 10,
                lines: vec![],
                missing_final_newline: false,
            },
            new: ModifiedLines {
                start: 11, // 10 + 1, not preserved 12
                lines: vec!["added three".to_string()],
                missing_final_newline: false,
            },
        };
        assert_eq!(filtered, expected);
        assert_eq!(filtered.to_string(), "@@ -10,0 +11 @@\n+added three\n");
    }

    #[test]
    fn retain_single_deletion_from_mixed() {
        let hunk = Hunk {
            old: ModifiedLines {
                start: 10,
                lines: vec!["deleted one".to_string(), "deleted two".to_string()],
                missing_final_newline: false,
            },
            new: ModifiedLines {
                start: 10,
                lines: vec!["added one".to_string(), "added two".to_string()],
                missing_final_newline: false,
            },
        };

        let filtered = hunk.retain(|o| o == 11, |_| false).unwrap();

        // When filtering to only deletions (no additions), new_start is recalculated
        // as old_start, since the gap appears at that position
        let expected = Hunk {
            old: ModifiedLines {
                start: 11,
                lines: vec!["deleted two".to_string()],
                missing_final_newline: false,
            },
            new: ModifiedLines {
                start: 11, // same as old_start for pure deletion
                lines: vec![],
                missing_final_newline: false,
            },
        };
        assert_eq!(filtered, expected);
        assert_eq!(filtered.to_string(), "@@ -11 +11,0 @@\n-deleted two\n");
    }

    #[test]
    fn retain_nothing_returns_none() {
        let hunk = Hunk {
            old: ModifiedLines {
                start: 10,
                lines: vec!["deleted".to_string()],
                missing_final_newline: false,
            },
            new: ModifiedLines {
                start: 10,
                lines: vec!["added".to_string()],
                missing_final_newline: false,
            },
        };

        let filtered = hunk.retain(|_| false, |_| false);
        assert!(filtered.is_none());
    }

    #[test]
    fn retain_subset_of_additions() {
        let hunk = Hunk {
            old: ModifiedLines {
                start: 9,
                lines: vec![],
                missing_final_newline: false,
            },
            new: ModifiedLines {
                start: 10,
                lines: vec![
                    "line ten".to_string(),
                    "line eleven".to_string(),
                    "line twelve".to_string(),
                ],
                missing_final_newline: false,
            },
        };

        let filtered = hunk.retain(|_| false, |n| n >= 11).unwrap();

        // Pure insertion: new_start = old_start + 1
        let expected = Hunk {
            old: ModifiedLines {
                start: 9,
                lines: vec![],
                missing_final_newline: false,
            },
            new: ModifiedLines {
                start: 10, // 9 + 1, not preserved 11
                lines: vec!["line eleven".to_string(), "line twelve".to_string()],
                missing_final_newline: false,
            },
        };
        assert_eq!(filtered, expected);
        assert_eq!(
            filtered.to_string(),
            "@@ -9,0 +10,2 @@\n+line eleven\n+line twelve\n"
        );
    }

    #[test]
    fn parse_insertion_at_file_start() {
        let input = "@@ -0,0 +1,2 @@\n+# Header\n+# Second line";

        let expected = Hunk {
            old: ModifiedLines {
                start: 0,
                lines: vec![],
                missing_final_newline: false,
            },
            new: ModifiedLines {
                start: 1,
                lines: vec!["# Header".to_string(), "# Second line".to_string()],
                missing_final_newline: false,
            },
        };

        let actual = Hunk::parse(input).unwrap();
        assert_eq!(actual, expected);
    }

    #[test]
    fn render_insertion_at_file_start() {
        let hunk = Hunk {
            old: ModifiedLines {
                start: 0,
                lines: vec![],
                missing_final_newline: false,
            },
            new: ModifiedLines {
                start: 1,
                lines: vec!["# First line".to_string()],
                missing_final_newline: false,
            },
        };
        assert_eq!(hunk.to_string(), "@@ -0,0 +1 @@\n+# First line\n");
    }

    #[test]
    fn parse_content_with_diff_markers() {
        let input = "@@ -5,0 +6,3 @@\n++++ this line starts with plus\n+--- this line starts with minus\n+@@ this looks like a header";

        let expected = Hunk {
            old: ModifiedLines {
                start: 5,
                lines: vec![],
                missing_final_newline: false,
            },
            new: ModifiedLines {
                start: 6,
                lines: vec![
                    "+++ this line starts with plus".to_string(),
                    "--- this line starts with minus".to_string(),
                    "@@ this looks like a header".to_string(),
                ],
                missing_final_newline: false,
            },
        };

        let actual = Hunk::parse(input).unwrap();
        assert_eq!(actual, expected);
    }

    #[test]
    fn parse_empty_line_content() {
        let input = "@@ -10,0 +11,3 @@\n+first\n+\n+third";

        let expected = Hunk {
            old: ModifiedLines {
                start: 10,
                lines: vec![],
                missing_final_newline: false,
            },
            new: ModifiedLines {
                start: 11,
                lines: vec!["first".to_string(), "".to_string(), "third".to_string()],
                missing_final_newline: false,
            },
        };

        let actual = Hunk::parse(input).unwrap();
        assert_eq!(actual, expected);
    }

    #[test]
    fn render_empty_line_content() {
        let hunk = Hunk {
            old: ModifiedLines {
                start: 10,
                lines: vec![],
                missing_final_newline: false,
            },
            new: ModifiedLines {
                start: 11,
                lines: vec!["first".to_string(), "".to_string(), "third".to_string()],
                missing_final_newline: false,
            },
        };
        assert_eq!(hunk.to_string(), "@@ -10,0 +11,3 @@\n+first\n+\n+third\n");
    }

    #[test]
    fn retain_non_contiguous_lines() {
        let hunk = Hunk {
            old: ModifiedLines {
                start: 9,
                lines: vec![],
                missing_final_newline: false,
            },
            new: ModifiedLines {
                start: 10,
                lines: vec![
                    "ten".to_string(),
                    "eleven".to_string(),
                    "twelve".to_string(),
                    "thirteen".to_string(),
                ],
                missing_final_newline: false,
            },
        };

        let filtered = hunk.retain(|_| false, |n| n == 10 || n == 12).unwrap();

        let expected = Hunk {
            old: ModifiedLines {
                start: 9,
                lines: vec![],
                missing_final_newline: false,
            },
            new: ModifiedLines {
                start: 10,
                lines: vec!["ten".to_string(), "twelve".to_string()],
                missing_final_newline: false,
            },
        };
        assert_eq!(filtered, expected);
    }

    #[test]
    fn retain_mixed_partial_selection() {
        let hunk = Hunk {
            old: ModifiedLines {
                start: 10,
                lines: vec![
                    "old one".to_string(),
                    "old two".to_string(),
                    "old three".to_string(),
                ],
                missing_final_newline: false,
            },
            new: ModifiedLines {
                start: 10,
                lines: vec![
                    "new one".to_string(),
                    "new two".to_string(),
                    "new three".to_string(),
                ],
                missing_final_newline: false,
            },
        };

        let filtered = hunk.retain(|o| o == 11, |n| n == 12).unwrap();

        let expected = Hunk {
            old: ModifiedLines {
                start: 11,
                lines: vec!["old two".to_string()],
                missing_final_newline: false,
            },
            new: ModifiedLines {
                start: 12,
                lines: vec!["new three".to_string()],
                missing_final_newline: false,
            },
        };
        assert_eq!(filtered, expected);
        assert_eq!(
            filtered.to_string(),
            "@@ -11 +12 @@\n-old two\n+new three\n"
        );
    }

    // =========================================================================
    // No newline at EOF tests
    // =========================================================================

    #[test]
    fn parse_old_missing_newline_only() {
        let input =
            "@@ -3 +3,2 @@\n-last line\n\\ No newline at end of file\n+last line\n+new final line";

        let expected = Hunk {
            old: ModifiedLines {
                start: 3,
                lines: vec!["last line".to_string()],
                missing_final_newline: true,
            },
            new: ModifiedLines {
                start: 3,
                lines: vec!["last line".to_string(), "new final line".to_string()],
                missing_final_newline: false,
            },
        };

        let actual = Hunk::parse(input).unwrap();
        assert_eq!(actual, expected);
    }

    #[test]
    fn parse_new_missing_newline_only() {
        // File originally had newline, new version removes it
        let input = "@@ -3 +3 @@\n-old line\n+old line\n\\ No newline at end of file";

        let expected = Hunk {
            old: ModifiedLines {
                start: 3,
                lines: vec!["old line".to_string()],
                missing_final_newline: false,
            },
            new: ModifiedLines {
                start: 3,
                lines: vec!["old line".to_string()],
                missing_final_newline: true,
            },
        };

        let actual = Hunk::parse(input).unwrap();
        assert_eq!(actual, expected);
    }

    #[test]
    fn parse_both_missing_newline() {
        // Both old and new lack trailing newline
        let input = "@@ -3 +3 @@\n-old version\n\\ No newline at end of file\n+new version\n\\ No newline at end of file";

        let expected = Hunk {
            old: ModifiedLines {
                start: 3,
                lines: vec!["old version".to_string()],
                missing_final_newline: true,
            },
            new: ModifiedLines {
                start: 3,
                lines: vec!["new version".to_string()],
                missing_final_newline: true,
            },
        };

        let actual = Hunk::parse(input).unwrap();
        assert_eq!(actual, expected);
    }

    #[test]
    fn roundtrip_old_missing_newline() {
        let original = "@@ -3 +3,2 @@\n-last line\n\\ No newline at end of file\n+last line\n+new final line\n";
        let hunk = Hunk::parse(original).unwrap();
        assert_eq!(hunk.to_string(), original);
    }

    #[test]
    fn roundtrip_new_missing_newline() {
        let original = "@@ -3 +3 @@\n-old line\n+old line\n\\ No newline at end of file\n";
        let hunk = Hunk::parse(original).unwrap();
        assert_eq!(hunk.to_string(), original);
    }

    #[test]
    fn roundtrip_both_missing_newline() {
        let original = "@@ -3 +3 @@\n-old version\n\\ No newline at end of file\n+new version\n\\ No newline at end of file\n";
        let hunk = Hunk::parse(original).unwrap();
        assert_eq!(hunk.to_string(), original);
    }

    #[test]
    fn retain_preserves_missing_newline_when_last_kept() {
        // Multiple additions, last one has no newline marker
        let text =
            "@@ -5,0 +6,2 @@\n+first addition\n+second addition\n\\ No newline at end of file";
        let hunk = Hunk::parse(text).unwrap();

        // Keep only last line (line 7)
        let filtered = hunk.retain(|_| false, |n| n == 7).unwrap();

        // Should preserve the no-newline marker since we kept the last line
        assert_eq!(
            filtered.to_string(),
            "@@ -5,0 +6 @@\n+second addition\n\\ No newline at end of file\n"
        );
    }

    #[test]
    fn retain_clears_missing_newline_when_last_filtered() {
        // Multiple additions, last one has no newline marker
        let text =
            "@@ -5,0 +6,2 @@\n+first addition\n+second addition\n\\ No newline at end of file";
        let hunk = Hunk::parse(text).unwrap();

        // Keep only first line (line 6), not the last
        let filtered = hunk.retain(|_| false, |n| n == 6).unwrap();

        // Should NOT have no-newline marker since we didn't keep the last line
        assert_eq!(filtered.to_string(), "@@ -5,0 +6 @@\n+first addition\n");
    }

    #[test]
    fn retain_mixed_with_old_missing_newline() {
        // Replacement where old line had no newline
        let text =
            "@@ -10 +10 @@\n-old content\n\\ No newline at end of file\n+new content with newline";
        let hunk = Hunk::parse(text).unwrap();

        // Keep only the addition
        let filtered = hunk.retain(|_| false, |n| n == 10).unwrap();

        // Old's no-newline marker should not appear since we filtered out deletions
        assert_eq!(
            filtered.to_string(),
            "@@ -10,0 +11 @@\n+new content with newline\n"
        );
    }
}

#[cfg(test)]
mod proptests {
    use super::*;
    use proptest::prelude::*;
    use std::collections::HashSet;

    /// Generate line content without newlines (diff format handles those)
    fn arb_line_content() -> impl Strategy<Value = String> {
        // Printable ASCII without newlines, reasonable length
        prop::collection::vec(prop::char::range(' ', '~'), 0..30)
            .prop_map(|chars| chars.into_iter().collect())
    }

    /// Generate a ModifiedLines struct
    fn arb_modified_lines() -> impl Strategy<Value = ModifiedLines> {
        (
            1..100u32,                                       // start
            prop::collection::vec(arb_line_content(), 0..5), // lines
            prop::bool::ANY,                                 // missing_final_newline
        )
            .prop_map(|(start, lines, missing_newline)| ModifiedLines {
                start,
                // Only meaningful if there are lines
                missing_final_newline: !lines.is_empty() && missing_newline,
                lines,
            })
    }

    /// Generate an arbitrary hunk
    fn arb_hunk() -> impl Strategy<Value = Hunk> {
        (arb_modified_lines(), arb_modified_lines()).prop_map(|(old, new)| Hunk { old, new })
    }

    /// Generate a set of line numbers to keep
    fn arb_line_set() -> impl Strategy<Value = HashSet<u32>> {
        prop::collection::hash_set(1..100u32, 0..15)
    }

    /// Generate a replacement hunk (both old and new have lines, same start)
    fn arb_replacement() -> impl Strategy<Value = Hunk> {
        (
            1..100u32,                                       // start
            prop::collection::vec(arb_line_content(), 1..4), // old_lines
            prop::collection::vec(arb_line_content(), 1..4), // new_lines
            prop::bool::ANY,                                 // old_missing_newline
            prop::bool::ANY,                                 // new_missing_newline
        )
            .prop_map(|(start, old_lines, new_lines, old_nl, new_nl)| Hunk {
                old: ModifiedLines {
                    start,
                    lines: old_lines,
                    missing_final_newline: old_nl,
                },
                new: ModifiedLines {
                    start,
                    lines: new_lines,
                    missing_final_newline: new_nl,
                },
            })
    }

    /// Generate realistic hunks using common patterns
    fn arb_realistic_hunk() -> impl Strategy<Value = Hunk> {
        prop_oneof![arb_pure_insertion(), arb_pure_deletion(), arb_replacement(),]
    }

    /// Generate a pure insertion hunk (no deletions)
    fn arb_pure_insertion() -> impl Strategy<Value = Hunk> {
        (
            1..100u32,                                       // old_start
            prop::collection::vec(arb_line_content(), 1..5), // new_lines (at least 1)
            prop::bool::ANY,                                 // missing_final_newline
        )
            .prop_map(|(old_start, new_lines, missing_newline)| Hunk {
                old: ModifiedLines {
                    start: old_start,
                    lines: vec![],
                    missing_final_newline: false,
                },
                new: ModifiedLines {
                    start: old_start + 1,
                    lines: new_lines,
                    missing_final_newline: missing_newline,
                },
            })
    }

    /// Generate a pure deletion hunk (no additions)
    fn arb_pure_deletion() -> impl Strategy<Value = Hunk> {
        (
            1..100u32,                                       // old_start
            prop::collection::vec(arb_line_content(), 1..5), // old_lines (at least 1)
            prop::bool::ANY,                                 // missing_final_newline
        )
            .prop_map(|(old_start, old_lines, missing_newline)| Hunk {
                old: ModifiedLines {
                    start: old_start,
                    lines: old_lines,
                    missing_final_newline: missing_newline,
                },
                new: ModifiedLines {
                    start: old_start,
                    lines: vec![],
                    missing_final_newline: false,
                },
            })
    }

    /// Generate a hunk that requires bridge synthesis:
    /// - Old has content with no trailing newline
    /// - New has the same content (bridge) followed by additions
    fn arb_bridge_scenario() -> impl Strategy<Value = Hunk> {
        (
            1..100u32,                                       // start
            arb_line_content(),                              // bridge content
            prop::collection::vec(arb_line_content(), 1..4), // new additions after bridge
        )
            .prop_map(|(start, bridge_content, new_additions)| {
                let mut new_lines = vec![bridge_content.clone()];
                new_lines.extend(new_additions);
                Hunk {
                    old: ModifiedLines {
                        start,
                        lines: vec![bridge_content],
                        missing_final_newline: true, // Key: no trailing newline
                    },
                    new: ModifiedLines {
                        start,
                        lines: new_lines,
                        missing_final_newline: false,
                    },
                }
            })
    }

    proptest! {
        /// Basic round-trip: any hunk must survive render → parse
        ///
        /// This validates:
        /// - The generators produce valid hunks
        /// - The parser and renderer are inverses
        #[test]
        fn hunk_roundtrips(hunk in arb_hunk()) {
            let rendered = hunk.to_string();
            let parsed = Hunk::parse(&rendered);

            prop_assert!(
                parsed.is_some(),
                "Failed to parse rendered hunk:\n{}\nOriginal: {:?}",
                rendered, hunk
            );

            prop_assert_eq!(
                parsed.unwrap(),
                hunk,
                "Round-trip failed for:\n{}",
                rendered
            );
        }

        /// The core property: filtered hunks must round-trip through render/parse
        ///
        /// This catches:
        /// - Line number recalculation bugs
        /// - Header generation bugs
        /// - No-newline marker handling bugs
        /// - Any state inconsistency between fields
        #[test]
        fn filtered_hunk_roundtrips(
            hunk in arb_hunk(),
            keep_old in arb_line_set(),
            keep_new in arb_line_set()
        ) {
            if let Some(filtered) = hunk.retain(
                |l| keep_old.contains(&l),
                |l| keep_new.contains(&l)
            ) {
                // The filtered hunk must render and parse back to itself
                let rendered = filtered.to_string();
                let parsed = Hunk::parse(&rendered);

                prop_assert!(
                    parsed.is_some(),
                    "Failed to parse rendered hunk:\n{}\nOriginal: {:?}\nFiltered: {:?}",
                    rendered, hunk, filtered
                );

                prop_assert_eq!(
                    parsed.unwrap(),
                    filtered,
                    "Round-trip failed for:\n{}",
                    rendered
                );
            }
        }

        /// Idempotence: retaining all lines of a mixed hunk must return the original
        ///
        /// This validates that retain doesn't accidentally modify mixed hunks
        /// when all lines are kept. Pure insertions/deletions are excluded because
        /// they correctly recalculate new_start for standalone patch format.
        #[test]
        fn retain_all_is_identity(hunk in arb_hunk()) {
            // Only test mixed hunks (both old and new have lines)
            prop_assume!(!hunk.old.lines.is_empty() && !hunk.new.lines.is_empty());

            let retained = hunk.retain(|_| true, |_| true);

            prop_assert!(
                retained.is_some(),
                "retain(true, true) returned None for non-empty hunk: {:?}",
                hunk
            );

            prop_assert_eq!(
                retained.unwrap(),
                hunk,
                "retain(true, true) modified the hunk"
            );
        }

        /// Empty filter: retaining nothing must return None
        #[test]
        fn retain_none_is_none(hunk in arb_hunk()) {
            let retained = hunk.retain(|_| false, |_| false);
            prop_assert!(
                retained.is_none(),
                "retain(false, false) returned Some for: {:?}",
                hunk
            );
        }

        /// Pure insertion recalculation: new_start must equal old_start + 1
        ///
        /// When a hunk has only additions (no deletions), the new content
        /// appears right after the old position, so new_start = old_start + 1.
        #[test]
        fn pure_insertion_new_start_is_old_start_plus_one(hunk in arb_pure_insertion()) {
            // Retain all additions
            let retained = hunk.retain(|_| false, |_| true).unwrap();

            prop_assert_eq!(
                retained.new.start,
                retained.old.start + 1,
                "Pure insertion should have new_start = old_start + 1, got {:?}",
                retained
            );
        }

        /// Pure deletion recalculation: new_start must equal old_start
        ///
        /// When a hunk has only deletions (no additions), the gap appears
        /// at the old position, so new_start = old_start.
        #[test]
        fn pure_deletion_new_start_is_old_start(hunk in arb_pure_deletion()) {
            // Retain all deletions
            let retained = hunk.retain(|_| true, |_| false).unwrap();

            prop_assert_eq!(
                retained.new.start,
                retained.old.start,
                "Pure deletion should have new_start = old_start, got {:?}",
                retained
            );
        }

        /// Mixed to pure insertion: filtering out deletions recalculates correctly
        #[test]
        fn mixed_to_pure_insertion_recalculates(hunk in arb_hunk()) {
            // Skip hunks that don't have both old and new lines
            prop_assume!(!hunk.old.lines.is_empty() && !hunk.new.lines.is_empty());

            // Filter to only additions (becomes pure insertion)
            let retained = hunk.retain(|_| false, |_| true).unwrap();

            prop_assert_eq!(
                retained.new.start,
                retained.old.start + 1,
                "Mixed→pure insertion should have new_start = old_start + 1, got {:?}",
                retained
            );
        }

        /// Mixed to pure deletion: filtering out additions recalculates correctly
        #[test]
        fn mixed_to_pure_deletion_recalculates(hunk in arb_hunk()) {
            // Skip hunks that don't have both old and new lines
            prop_assume!(!hunk.old.lines.is_empty() && !hunk.new.lines.is_empty());

            // Filter to only deletions (becomes pure deletion)
            let retained = hunk.retain(|_| true, |_| false).unwrap();

            prop_assert_eq!(
                retained.new.start,
                retained.old.start,
                "Mixed→pure deletion should have new_start = old_start, got {:?}",
                retained
            );
        }

        /// Bridge synthesis: when filtering additions after a no-newline line,
        /// the bridge content must be auto-included to provide line separation.
        ///
        /// Without bridge synthesis, git apply would concatenate lines.
        #[test]
        fn bridge_synthesis_includes_separator(hunk in arb_bridge_scenario()) {
            // Skip the first addition (the bridge), keep only subsequent additions
            // This should trigger bridge synthesis
            let first_new_line = hunk.new.start;
            let retained = hunk.retain(
                |_| false,
                |l| l > first_new_line
            );

            // Should have a result (bridge was synthesized)
            prop_assert!(
                retained.is_some(),
                "Bridge synthesis should produce a result for: {:?}",
                hunk
            );

            let retained = retained.unwrap();

            // The bridge must be included: old deletion + new addition of same content
            prop_assert!(
                !retained.old.lines.is_empty(),
                "Bridge synthesis must include old deletion: {:?}",
                retained
            );

            prop_assert!(
                !retained.new.lines.is_empty(),
                "Bridge synthesis must include new addition: {:?}",
                retained
            );

            // First line of new should be the bridge content (same as old)
            prop_assert_eq!(
                retained.old.lines.last(),
                retained.new.lines.first(),
                "Bridge content must match: old={:?}, new={:?}",
                retained.old, retained.new
            );

            // Result must still round-trip
            let rendered = retained.to_string();
            let parsed = Hunk::parse(&rendered);
            prop_assert!(parsed.is_some(), "Bridge result must parse: {}", rendered);
            prop_assert_eq!(parsed.unwrap(), retained);
        }

        /// Subset invariant: filtered result must only contain lines from original
        ///
        /// This catches data corruption or incorrect bridge synthesis content.
        /// Note: Bridge synthesis may copy old content to new, so new lines
        /// can come from either hunk.new OR hunk.old.
        #[test]
        fn filtered_is_subset_of_original(
            hunk in arb_hunk(),
            keep_old in arb_line_set(),
            keep_new in arb_line_set()
        ) {
            if let Some(filtered) = hunk.retain(
                |l| keep_old.contains(&l),
                |l| keep_new.contains(&l)
            ) {
                // Every line in filtered.old must exist in hunk.old
                for line in &filtered.old.lines {
                    prop_assert!(
                        hunk.old.lines.contains(line),
                        "Filtered old line {:?} not in original {:?}",
                        line, hunk.old.lines
                    );
                }

                // Every line in filtered.new must exist in hunk.new OR hunk.old
                // (bridge synthesis copies old content to new)
                for line in &filtered.new.lines {
                    prop_assert!(
                        hunk.new.lines.contains(line) || hunk.old.lines.contains(line),
                        "Filtered new line {:?} not in original new {:?} or old {:?}",
                        line, hunk.new.lines, hunk.old.lines
                    );
                }
            }
        }

        /// Header consistency: rendered header line counts must match actual content
        #[test]
        fn hunk_header_matches_content(hunk in arb_hunk()) {
            let rendered = hunk.to_string();

            // Parse the header line: @@ -OLD,COUNT +NEW,COUNT @@
            let header_line = rendered.lines().next().unwrap();

            // Extract counts from header (handles both "-X" and "-X,Y" formats)
            let parts: Vec<&str> = header_line
                .trim_start_matches("@@ ")
                .trim_end_matches(" @@")
                .split(' ')
                .collect();

            let old_part = parts[0].trim_start_matches('-');
            let new_part = parts[1].trim_start_matches('+');

            let old_count = if let Some((_, count)) = old_part.split_once(',') {
                count.parse::<usize>().unwrap()
            } else {
                if hunk.old.lines.is_empty() { 0 } else { 1 }
            };

            let new_count = if let Some((_, count)) = new_part.split_once(',') {
                count.parse::<usize>().unwrap()
            } else {
                if hunk.new.lines.is_empty() { 0 } else { 1 }
            };

            prop_assert_eq!(
                old_count,
                hunk.old.lines.len(),
                "Old line count mismatch in header: {}",
                header_line
            );

            prop_assert_eq!(
                new_count,
                hunk.new.lines.len(),
                "New line count mismatch in header: {}",
                header_line
            );
        }

        /// Realistic hunks should round-trip correctly
        ///
        /// Uses the realistic generator that produces structurally valid hunks
        /// (pure insertions, pure deletions, replacements) rather than arbitrary ones.
        #[test]
        fn realistic_hunk_roundtrips(hunk in arb_realistic_hunk()) {
            let rendered = hunk.to_string();
            let parsed = Hunk::parse(&rendered);

            prop_assert!(
                parsed.is_some(),
                "Failed to parse realistic hunk:\n{}\nOriginal: {:?}",
                rendered, hunk
            );

            prop_assert_eq!(
                parsed.unwrap(),
                hunk,
                "Round-trip failed for realistic hunk:\n{}",
                rendered
            );
        }

        /// Filtered realistic hunks should round-trip
        #[test]
        fn filtered_realistic_hunk_roundtrips(
            hunk in arb_realistic_hunk(),
            keep_old in arb_line_set(),
            keep_new in arb_line_set()
        ) {
            if let Some(filtered) = hunk.retain(
                |l| keep_old.contains(&l),
                |l| keep_new.contains(&l)
            ) {
                let rendered = filtered.to_string();
                let parsed = Hunk::parse(&rendered);

                prop_assert!(
                    parsed.is_some(),
                    "Failed to parse filtered realistic hunk:\n{}",
                    rendered
                );

                prop_assert_eq!(
                    parsed.unwrap(),
                    filtered,
                    "Round-trip failed for filtered realistic hunk"
                );
            }
        }
    }
}
