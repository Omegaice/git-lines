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

/// Result of filtering a hunk's content.
///
/// This is an intermediate representation that separates the "what lines were kept"
/// question from the "how to structure output hunks" question. The caller (FileDiff)
/// decides how to build hunks from this content.
///
/// # Key Insight
///
/// Additions and deletions have different position semantics:
/// - **Deletions** reference specific positions in the old file. Each deletion
///   has a unique position that must be preserved.
/// - **Additions** share a single insertion point. All additions in a hunk are
///   inserted at the same place (after `insertion_point`).
///
/// This asymmetry means:
/// - Non-contiguous deletions may need multiple hunks (different old positions)
/// - Non-contiguous additions stay together (same insertion point)
#[derive(Debug, PartialEq, Eq)]
pub struct FilteredContent {
    /// The insertion point for additions (original hunk's old.start).
    /// All additions are inserted "after this line" in the old file.
    pub insertion_point: u32,

    /// Kept deletions with their original OLD line positions.
    /// Each deletion references a specific line in the old file.
    pub deletions: Vec<(u32, String)>,

    /// Kept additions (content only - position is implicit via insertion_point).
    /// All additions go to the same place, so we don't need individual positions.
    pub additions: Vec<String>,

    /// Whether the original old content's last line lacked a trailing newline
    pub old_missing_newline: bool,

    /// Whether the filtered additions' last line should lack a trailing newline
    pub new_missing_newline: bool,
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

    /// Filter lines in the hunk, returning the filtered content.
    ///
    /// This method only filters - it does NOT decide how to structure output hunks.
    /// The caller (FileDiff) is responsible for building hunks from the returned
    /// FilteredContent, which allows proper handling of the addition/deletion asymmetry.
    ///
    /// # Parameters
    ///
    /// - `keep_old`: Predicate for old lines (deletions). Called with old line number.
    /// - `keep_new`: Predicate for new lines (additions). Called with new line number.
    ///
    /// # Returns
    ///
    /// - `Some(FilteredContent)` with the kept lines
    /// - `None` if no lines matched either predicate
    ///
    /// # No-Newline Bridge Synthesis
    ///
    /// If the old lines had no trailing newline and you're keeping additions after it,
    /// the method automatically includes the old deletion to provide the required
    /// newline separator. This prevents corrupted git index state.
    #[must_use]
    pub fn filter<F, G>(&self, keep_old: F, keep_new: G) -> Option<FilteredContent>
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

        // Track no-newline state
        let old_missing_newline = old_filtered.kept_last_boundary && self.old.missing_final_newline;
        let new_missing_newline = new_filtered.kept_last_boundary && self.new.missing_final_newline;

        Some(FilteredContent {
            insertion_point: self.old.start,
            deletions: old_filtered.lines,
            additions: new_filtered.lines.into_iter().map(|(_, c)| c).collect(),
            old_missing_newline,
            new_missing_newline,
        })
    }
}

/// Result of filtering lines, tracking boundary alignment with the original
struct FilterResult {
    /// Each kept line with its original line number
    lines: Vec<(u32, String)>,
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
        kept_first_boundary: false,
        kept_last_boundary: false,
    };

    let last_idx = source.lines.len().saturating_sub(1);

    for (i, line) in source.lines.iter().enumerate() {
        let line_num = source.start + i as u32;
        if keep(line_num) {
            result.lines.push((line_num, line.clone()));
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

        old_filtered
            .lines
            .push((last_line_num, last_old_line.clone()));
        old_filtered.kept_last_boundary = true;
    }

    // Synthesize the first addition with the old content (provides the newline)
    let synth_line_num = old_source.start + old_source.lines.len() as u32;
    new_filtered
        .lines
        .insert(0, (synth_line_num, last_old_line.clone()));
    new_filtered.kept_first_boundary = true;
}

/// A contiguous group of lines
pub(crate) struct ContiguousGroup {
    pub first_line_num: u32,
    pub lines: Vec<(u32, String)>,
}

/// Group lines into contiguous runs
///
/// When there are gaps in line numbers (e.g., lines 3, 4, 6), this splits
/// them into separate groups (e.g., [3, 4] and [6]).
pub(crate) fn group_contiguous_lines(lines: &[(u32, String)]) -> Vec<ContiguousGroup> {
    if lines.is_empty() {
        return vec![];
    }

    let mut groups: Vec<ContiguousGroup> = Vec::new();
    let mut current_group: Vec<(u32, String)> = Vec::new();

    for (line_num, content) in lines {
        if current_group.is_empty() {
            // Start first group
            current_group.push((*line_num, content.clone()));
        } else {
            let last_num = current_group.last().unwrap().0;
            if *line_num == last_num + 1 {
                // Contiguous - add to current group
                current_group.push((*line_num, content.clone()));
            } else {
                // Gap detected - finalize current group and start new one
                let first = current_group[0].0;
                groups.push(ContiguousGroup {
                    first_line_num: first,
                    lines: current_group,
                });
                current_group = vec![(*line_num, content.clone())];
            }
        }
    }

    // Don't forget the last group
    if !current_group.is_empty() {
        let first = current_group[0].0;
        groups.push(ContiguousGroup {
            first_line_num: first,
            lines: current_group,
        });
    }

    groups
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
    fn filter_single_addition_from_mixed() {
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

        let filtered = hunk.filter(|_| false, |n| n == 12).unwrap();

        // When filtering to only additions, deletions should be empty
        // and additions should contain only the selected line
        assert!(filtered.deletions.is_empty());
        assert_eq!(filtered.additions, vec!["added three".to_string()]);
        assert_eq!(filtered.insertion_point, 10);
    }

    #[test]
    fn filter_single_deletion_from_mixed() {
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

        let filtered = hunk.filter(|o| o == 11, |_| false).unwrap();

        // When filtering to only deletions, additions should be empty
        // and deletions should contain the selected line with its position
        assert!(filtered.additions.is_empty());
        assert_eq!(filtered.deletions, vec![(11, "deleted two".to_string())]);
    }

    #[test]
    fn filter_nothing_returns_none() {
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

        let filtered = hunk.filter(|_| false, |_| false);
        assert!(filtered.is_none());
    }

    #[test]
    fn filter_subset_of_additions() {
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

        let filtered = hunk.filter(|_| false, |n| n >= 11).unwrap();

        // Should keep the last two additions
        assert!(filtered.deletions.is_empty());
        assert_eq!(
            filtered.additions,
            vec!["line eleven".to_string(), "line twelve".to_string()]
        );
        assert_eq!(filtered.insertion_point, 9);
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
    fn filter_non_contiguous_additions() {
        // Non-contiguous selection of additions should return all selected
        // additions in FilteredContent. The hunk-building (single vs multiple)
        // is now handled by FileDiff::retain.
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

        let filtered = hunk.filter(|_| false, |n| n == 10 || n == 12).unwrap();

        // Should have both selected additions
        assert!(filtered.deletions.is_empty());
        assert_eq!(
            filtered.additions,
            vec!["ten".to_string(), "twelve".to_string()]
        );
        // Insertion point is the original old.start
        assert_eq!(filtered.insertion_point, 9);
    }

    #[test]
    fn filter_mixed_partial_selection() {
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

        let filtered = hunk.filter(|o| o == 11, |n| n == 12).unwrap();

        // Should have one deletion at position 11 and one addition
        assert_eq!(filtered.deletions, vec![(11, "old two".to_string())]);
        assert_eq!(filtered.additions, vec!["new three".to_string()]);
        assert_eq!(filtered.insertion_point, 10);
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
    fn filter_preserves_missing_newline_when_last_kept() {
        // Multiple additions, last one has no newline marker
        let text =
            "@@ -5,0 +6,2 @@\n+first addition\n+second addition\n\\ No newline at end of file";
        let hunk = Hunk::parse(text).unwrap();

        // Keep only last line (line 7)
        let filtered = hunk.filter(|_| false, |n| n == 7).unwrap();

        // Should preserve the no-newline flag since we kept the last line
        assert_eq!(filtered.additions, vec!["second addition".to_string()]);
        assert!(filtered.new_missing_newline);
    }

    #[test]
    fn filter_clears_missing_newline_when_last_filtered() {
        // Multiple additions, last one has no newline marker
        let text =
            "@@ -5,0 +6,2 @@\n+first addition\n+second addition\n\\ No newline at end of file";
        let hunk = Hunk::parse(text).unwrap();

        // Keep only first line (line 6), not the last
        let filtered = hunk.filter(|_| false, |n| n == 6).unwrap();

        // Should NOT have no-newline flag since we didn't keep the last line
        assert_eq!(filtered.additions, vec!["first addition".to_string()]);
        assert!(!filtered.new_missing_newline);
    }

    #[test]
    fn filter_mixed_with_old_missing_newline() {
        // Replacement where old line had no newline
        let text =
            "@@ -10 +10 @@\n-old content\n\\ No newline at end of file\n+new content with newline";
        let hunk = Hunk::parse(text).unwrap();

        // Keep only the addition
        let filtered = hunk.filter(|_| false, |n| n == 10).unwrap();

        // Old's no-newline should not be set since we filtered out deletions
        assert!(filtered.deletions.is_empty());
        assert!(!filtered.old_missing_newline);
        assert_eq!(
            filtered.additions,
            vec!["new content with newline".to_string()]
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

        /// Filter all: filtering with (true, true) must return all content
        #[test]
        fn filter_all_returns_all_content(hunk in arb_hunk()) {
            prop_assume!(!hunk.old.lines.is_empty() || !hunk.new.lines.is_empty());

            let filtered = hunk.filter(|_| true, |_| true);

            prop_assert!(
                filtered.is_some(),
                "filter(true, true) returned None for non-empty hunk: {:?}",
                hunk
            );

            let filtered = filtered.unwrap();
            prop_assert_eq!(
                filtered.deletions.len(),
                hunk.old.lines.len(),
                "Should keep all deletions"
            );
            prop_assert_eq!(
                filtered.additions.len(),
                hunk.new.lines.len(),
                "Should keep all additions"
            );
        }

        /// Empty filter: filtering nothing must return None
        #[test]
        fn filter_none_returns_none(hunk in arb_hunk()) {
            let filtered = hunk.filter(|_| false, |_| false);
            prop_assert!(
                filtered.is_none(),
                "filter(false, false) returned Some for: {:?}",
                hunk
            );
        }

        /// Filter preserves insertion point
        #[test]
        fn filter_preserves_insertion_point(hunk in arb_hunk()) {
            prop_assume!(!hunk.new.lines.is_empty());

            let filtered = hunk.filter(|_| false, |_| true).unwrap();

            prop_assert_eq!(
                filtered.insertion_point,
                hunk.old.start,
                "Insertion point should be original old.start"
            );
        }

        /// Bridge synthesis: when filtering additions after a no-newline line,
        /// the bridge content must be auto-included to provide line separation.
        #[test]
        fn bridge_synthesis_includes_separator(hunk in arb_bridge_scenario()) {
            // Skip the first addition (the bridge), keep only subsequent additions
            let first_new_line = hunk.new.start;
            let filtered = hunk.filter(|_| false, |l| l > first_new_line);

            prop_assert!(
                filtered.is_some(),
                "Bridge synthesis should produce a result for: {:?}",
                hunk
            );

            let filtered = filtered.unwrap();

            // The bridge must be included: deletion of old line
            prop_assert!(
                !filtered.deletions.is_empty(),
                "Bridge synthesis must include deletion: {:?}",
                filtered
            );

            // First addition should be the bridge content (same as the deletion)
            let bridge_content = &filtered.deletions.last().unwrap().1;
            prop_assert!(
                filtered.additions.first() == Some(bridge_content),
                "Bridge content must match: deletions={:?}, additions={:?}",
                filtered.deletions, filtered.additions
            );
        }

        /// Subset invariant: filtered result must only contain lines from original
        #[test]
        fn filtered_is_subset_of_original(
            hunk in arb_hunk(),
            keep_old in arb_line_set(),
            keep_new in arb_line_set()
        ) {
            if let Some(filtered) = hunk.filter(
                |l| keep_old.contains(&l),
                |l| keep_new.contains(&l)
            ) {
                // Every deletion must exist in hunk.old
                for (_, line) in &filtered.deletions {
                    prop_assert!(
                        hunk.old.lines.contains(line),
                        "Filtered deletion {:?} not in original {:?}",
                        line, hunk.old.lines
                    );
                }

                // Every addition must exist in hunk.new OR hunk.old (bridge synthesis)
                for line in &filtered.additions {
                    prop_assert!(
                        hunk.new.lines.contains(line) || hunk.old.lines.contains(line),
                        "Filtered addition {:?} not in original new {:?} or old {:?}",
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
    }
}
