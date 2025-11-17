use std::fmt;

/// Lines modified in the old or new version
#[derive(Debug, PartialEq, Eq)]
pub struct ModifiedLines {
    pub start: u32,
    pub lines: Vec<String>,
    pub missing_final_newline: bool,
}

/// A single hunk from a git diff
#[derive(Debug, PartialEq, Eq)]
pub struct Hunk {
    pub old: ModifiedLines,
    pub new: ModifiedLines,
}

impl Hunk {
    /// Parse a hunk from diff text (header + content lines)
    pub fn parse(text: &str) -> Option<Self> {
        let mut lines = text.lines();

        // Parse header
        let header = lines.next()?;
        let (old_start, new_start) = Self::parse_header(header)?;

        let mut old_lines = Vec::new();
        let mut new_lines = Vec::new();
        let mut old_missing_newline = false;
        let mut new_missing_newline = false;

        // Track what type of line we last saw (for "\ No newline" marker)
        enum LastLineType {
            None,
            Old,
            New,
        }
        let mut last_line_type = LastLineType::None;

        // Parse content lines
        for line in lines {
            if line.starts_with("\\ No newline at end of file") {
                // This marker applies to whichever line type we saw last
                match last_line_type {
                    LastLineType::Old => old_missing_newline = true,
                    LastLineType::New => new_missing_newline = true,
                    LastLineType::None => {}
                }
            } else if let Some(content) = line.strip_prefix('-') {
                old_lines.push(content.to_string());
                last_line_type = LastLineType::Old;
            } else if let Some(content) = line.strip_prefix('+') {
                new_lines.push(content.to_string());
                last_line_type = LastLineType::New;
            }
            // Ignore context lines (shouldn't have any with -U0)
        }

        Some(Hunk {
            old: ModifiedLines {
                start: old_start,
                lines: old_lines,
                missing_final_newline: old_missing_newline,
            },
            new: ModifiedLines {
                start: new_start,
                lines: new_lines,
                missing_final_newline: new_missing_newline,
            },
        })
    }

    /// Filter lines in the hunk, returning a new valid hunk with only matching lines.
    /// Returns None if no lines match.
    pub fn retain<F, G>(&self, mut keep_old: F, mut keep_new: G) -> Option<Self>
    where
        F: FnMut(u32) -> bool,
        G: FnMut(u32) -> bool,
    {
        // Filter old (deletion) lines
        let mut new_old_lines = Vec::new();
        let mut new_old_start = None;
        let mut kept_last_old = false;
        let old_last_idx = self.old.lines.len().saturating_sub(1);
        for (i, line) in self.old.lines.iter().enumerate() {
            let line_num = self.old.start + i as u32;
            if keep_old(line_num) {
                if new_old_start.is_none() {
                    new_old_start = Some(line_num);
                }
                new_old_lines.push(line.clone());
                if i == old_last_idx {
                    kept_last_old = true;
                }
            }
        }

        // Filter new (addition) lines
        let mut new_new_lines = Vec::new();
        let mut new_new_start = None;
        let mut kept_last_new = false;
        let mut kept_first_new = false;
        let new_last_idx = self.new.lines.len().saturating_sub(1);
        for (i, line) in self.new.lines.iter().enumerate() {
            let line_num = self.new.start + i as u32;
            if keep_new(line_num) {
                if new_new_start.is_none() {
                    new_new_start = Some(line_num);
                }
                new_new_lines.push(line.clone());
                if i == 0 {
                    kept_first_new = true;
                }
                if i == new_last_idx {
                    kept_last_new = true;
                }
            }
        }

        // If nothing matched, return None
        if new_old_lines.is_empty() && new_new_lines.is_empty() {
            return None;
        }

        // Handle no-newline bridge: if old line had no trailing newline and we're
        // adding lines after it, we must include the old deletion and synthesize
        // the first addition to provide the \n separator
        if self.old.missing_final_newline
            && !new_new_lines.is_empty()
            && !kept_first_new
            && !self.old.lines.is_empty()
        {
            // Force-include the last old deletion
            let last_old_line = self.old.lines.last().unwrap().clone();
            let last_old_line_num = self.old.start + old_last_idx as u32;
            if !kept_last_old {
                new_old_lines.push(last_old_line.clone());
                if new_old_start.is_none() {
                    new_old_start = Some(last_old_line_num);
                }
                kept_last_old = true;
            }

            // Synthesize first addition as copy of old content (provides \n)
            new_new_lines.insert(0, last_old_line);
            new_new_start = Some(self.new.start);
        }

        // Determine final start positions
        // Key insight: preserve original positions from the hunk when possible
        let final_old_start = match new_old_start {
            Some(old) => old,
            None => self.old.start, // No deletions kept, use original position
        };

        // For new_start: if we have no deletions, we must recalculate
        // because the original new_start assumed other changes happened
        let final_new_start = if new_old_lines.is_empty() && !new_new_lines.is_empty() {
            // Pure insertion: new content appears right after old_start
            final_old_start + 1
        } else if !new_old_lines.is_empty() && new_new_lines.is_empty() {
            // Pure deletion: result line number is where the gap appears
            final_old_start
        } else {
            // Mixed or empty: use the actual new line number
            match new_new_start {
                Some(new) => new,
                None => self.new.start,
            }
        };

        // Preserve missing_final_newline only if we kept the original last line
        let old_missing = kept_last_old && self.old.missing_final_newline;
        let new_missing = kept_last_new && self.new.missing_final_newline;

        Some(Hunk {
            old: ModifiedLines {
                start: final_old_start,
                lines: new_old_lines,
                missing_final_newline: old_missing,
            },
            new: ModifiedLines {
                start: final_new_start,
                lines: new_new_lines,
                missing_final_newline: new_missing,
            },
        })
    }

    /// Parse hunk header to extract old and new start positions
    fn parse_header(header: &str) -> Option<(u32, u32)> {
        let header = header.strip_prefix("@@ ")?;
        let end_idx = header.find(" @@")?;
        let range_part = &header[..end_idx];

        let parts: Vec<&str> = range_part.split(' ').collect();
        if parts.len() != 2 {
            return None;
        }

        let old_start = Self::parse_range_start(parts[0].strip_prefix('-').unwrap_or(parts[0]))?;
        let new_start = Self::parse_range_start(parts[1].strip_prefix('+').unwrap_or(parts[1]))?;

        Some((old_start, new_start))
    }

    /// Parse the start line number from a range like "136,0" or "137"
    fn parse_range_start(range: &str) -> Option<u32> {
        let num_str = if let Some(idx) = range.find(',') {
            &range[..idx]
        } else {
            range
        };

        num_str.parse::<u32>().ok()
    }
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
mod tests {
    use super::*;

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
