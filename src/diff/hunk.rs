use std::fmt;

/// Lines modified in the old or new version
pub struct ModifiedLines(pub u32, pub Vec<String>);

/// A single hunk from a git diff
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

        // Parse content lines
        for line in lines {
            if let Some(content) = line.strip_prefix('-') {
                old_lines.push(content.to_string());
            } else if let Some(content) = line.strip_prefix('+') {
                new_lines.push(content.to_string());
            }
            // Ignore context lines (shouldn't have any with -U0)
        }

        Some(Hunk {
            old: ModifiedLines(old_start, old_lines),
            new: ModifiedLines(new_start, new_lines),
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
        for (i, line) in self.old.1.iter().enumerate() {
            let line_num = self.old.0 + i as u32;
            if keep_old(line_num) {
                if new_old_start.is_none() {
                    new_old_start = Some(line_num);
                }
                new_old_lines.push(line.clone());
            }
        }

        // Filter new (addition) lines
        let mut new_new_lines = Vec::new();
        let mut new_new_start = None;
        for (i, line) in self.new.1.iter().enumerate() {
            let line_num = self.new.0 + i as u32;
            if keep_new(line_num) {
                if new_new_start.is_none() {
                    new_new_start = Some(line_num);
                }
                new_new_lines.push(line.clone());
            }
        }

        // If nothing matched, return None
        if new_old_lines.is_empty() && new_new_lines.is_empty() {
            return None;
        }

        // Determine final start positions
        // Key insight: preserve original positions from the hunk when possible
        let final_old_start = match new_old_start {
            Some(old) => old,
            None => self.old.0, // No deletions kept, use original position
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
                None => self.new.0,
            }
        };

        Some(Hunk {
            old: ModifiedLines(final_old_start, new_old_lines),
            new: ModifiedLines(final_new_start, new_new_lines),
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
        let old_part = match self.old.1.len() {
            0 => format!("-{},0", self.old.0),
            1 => format!("-{}", self.old.0),
            n => format!("-{},{}", self.old.0, n),
        };

        let new_part = match self.new.1.len() {
            0 => format!("+{},0", self.new.0),
            1 => format!("+{}", self.new.0),
            n => format!("+{},{}", self.new.0, n),
        };

        writeln!(f, "@@ {} {} @@", old_part, new_part)?;

        // Add deletion lines
        for line in &self.old.1 {
            writeln!(f, "-{}", line)?;
        }

        // Add addition lines
        for line in &self.new.1 {
            writeln!(f, "+{}", line)?;
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
            old: ModifiedLines(10, vec![]),
            new: ModifiedLines(11, vec!["new line here".to_string()]),
        };
        assert_eq!(hunk.to_string(), "@@ -10,0 +11 @@\n+new line here\n");
    }

    #[test]
    fn render_pure_deletion() {
        let hunk = Hunk {
            old: ModifiedLines(10, vec!["old line removed".to_string()]),
            new: ModifiedLines(9, vec![]),
        };
        assert_eq!(hunk.to_string(), "@@ -10 +9,0 @@\n-old line removed\n");
    }

    #[test]
    fn render_single_line_replacement() {
        let hunk = Hunk {
            old: ModifiedLines(10, vec!["old version".to_string()]),
            new: ModifiedLines(10, vec!["new version".to_string()]),
        };
        assert_eq!(
            hunk.to_string(),
            "@@ -10 +10 @@\n-old version\n+new version\n"
        );
    }

    #[test]
    fn render_multi_line_change() {
        let hunk = Hunk {
            old: ModifiedLines(
                10,
                vec!["first old line".to_string(), "second old line".to_string()],
            ),
            new: ModifiedLines(
                10,
                vec![
                    "first new line".to_string(),
                    "second new line".to_string(),
                    "third new line".to_string(),
                ],
            ),
        };
        assert_eq!(
            hunk.to_string(),
            "@@ -10,2 +10,3 @@\n-first old line\n-second old line\n+first new line\n+second new line\n+third new line\n"
        );
    }

    #[test]
    fn render_multiple_insertions() {
        let hunk = Hunk {
            old: ModifiedLines(5, vec![]),
            new: ModifiedLines(6, vec!["line one".to_string(), "line two".to_string()]),
        };
        assert_eq!(hunk.to_string(), "@@ -5,0 +6,2 @@\n+line one\n+line two\n");
    }

    #[test]
    fn render_multiple_deletions() {
        let hunk = Hunk {
            old: ModifiedLines(
                15,
                vec!["removed one".to_string(), "removed two".to_string()],
            ),
            new: ModifiedLines(14, vec![]),
        };
        assert_eq!(
            hunk.to_string(),
            "@@ -15,2 +14,0 @@\n-removed one\n-removed two\n"
        );
    }

    #[test]
    fn parse_pure_insertion() {
        let text = "@@ -10,0 +11 @@\n+new line here";
        let hunk = Hunk::parse(text).unwrap();
        assert_eq!(hunk.old.0, 10);
        assert_eq!(hunk.old.1, Vec::<String>::new());
        assert_eq!(hunk.new.0, 11);
        assert_eq!(hunk.new.1, vec!["new line here"]);
    }

    #[test]
    fn parse_pure_deletion() {
        let text = "@@ -10 +9,0 @@\n-old line removed";
        let hunk = Hunk::parse(text).unwrap();
        assert_eq!(hunk.old.0, 10);
        assert_eq!(hunk.old.1, vec!["old line removed"]);
        assert_eq!(hunk.new.0, 9);
        assert_eq!(hunk.new.1, Vec::<String>::new());
    }

    #[test]
    fn parse_mixed_change() {
        let text =
            "@@ -10,2 +10,3 @@\n-first old\n-second old\n+first new\n+second new\n+third new";
        let hunk = Hunk::parse(text).unwrap();
        assert_eq!(hunk.old.0, 10);
        assert_eq!(hunk.old.1, vec!["first old", "second old"]);
        assert_eq!(hunk.new.0, 10);
        assert_eq!(hunk.new.1, vec!["first new", "second new", "third new"]);
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
            old: ModifiedLines(
                10,
                vec!["deleted one".to_string(), "deleted two".to_string()],
            ),
            new: ModifiedLines(
                10,
                vec![
                    "added one".to_string(),
                    "added two".to_string(),
                    "added three".to_string(),
                ],
            ),
        };

        let filtered = hunk.retain(|_| false, |n| n == 12).unwrap();

        // When filtering to only additions (no deletions), new_start is recalculated
        // as old_start + 1, since insertions appear right after the old position
        assert_eq!(filtered.old.0, 10);
        assert_eq!(filtered.old.1, Vec::<String>::new());
        assert_eq!(filtered.new.0, 11); // 10 + 1, not preserved 12
        assert_eq!(filtered.new.1, vec!["added three"]);
        assert_eq!(filtered.to_string(), "@@ -10,0 +11 @@\n+added three\n");
    }

    #[test]
    fn retain_single_deletion_from_mixed() {
        let hunk = Hunk {
            old: ModifiedLines(
                10,
                vec!["deleted one".to_string(), "deleted two".to_string()],
            ),
            new: ModifiedLines(10, vec!["added one".to_string(), "added two".to_string()]),
        };

        let filtered = hunk.retain(|o| o == 11, |_| false).unwrap();

        // When filtering to only deletions (no additions), new_start is recalculated
        // as old_start, since the gap appears at that position
        assert_eq!(filtered.old.0, 11);
        assert_eq!(filtered.old.1, vec!["deleted two"]);
        assert_eq!(filtered.new.0, 11); // same as old_start for pure deletion
        assert_eq!(filtered.new.1, Vec::<String>::new());
        assert_eq!(filtered.to_string(), "@@ -11 +11,0 @@\n-deleted two\n");
    }

    #[test]
    fn retain_nothing_returns_none() {
        let hunk = Hunk {
            old: ModifiedLines(10, vec!["deleted".to_string()]),
            new: ModifiedLines(10, vec!["added".to_string()]),
        };

        let filtered = hunk.retain(|_| false, |_| false);
        assert!(filtered.is_none());
    }

    #[test]
    fn retain_subset_of_additions() {
        let hunk = Hunk {
            old: ModifiedLines(9, vec![]),
            new: ModifiedLines(
                10,
                vec![
                    "line ten".to_string(),
                    "line eleven".to_string(),
                    "line twelve".to_string(),
                ],
            ),
        };

        let filtered = hunk.retain(|_| false, |n| n >= 11).unwrap();

        // Pure insertion: new_start = old_start + 1
        assert_eq!(filtered.old.0, 9);
        assert_eq!(filtered.new.0, 10); // 9 + 1, not preserved 11
        assert_eq!(filtered.new.1, vec!["line eleven", "line twelve"]);
        assert_eq!(
            filtered.to_string(),
            "@@ -9,0 +10,2 @@\n+line eleven\n+line twelve\n"
        );
    }

    #[test]
    fn parse_insertion_at_file_start() {
        let text = "@@ -0,0 +1,2 @@\n+# Header\n+# Second line";
        let hunk = Hunk::parse(text).unwrap();
        assert_eq!(hunk.old.0, 0);
        assert_eq!(hunk.old.1, Vec::<String>::new());
        assert_eq!(hunk.new.0, 1);
        assert_eq!(hunk.new.1, vec!["# Header", "# Second line"]);
    }

    #[test]
    fn render_insertion_at_file_start() {
        let hunk = Hunk {
            old: ModifiedLines(0, vec![]),
            new: ModifiedLines(1, vec!["# First line".to_string()]),
        };
        assert_eq!(hunk.to_string(), "@@ -0,0 +1 @@\n+# First line\n");
    }

    #[test]
    fn parse_content_with_diff_markers() {
        let text = "@@ -5,0 +6,3 @@\n++++ this line starts with plus\n+--- this line starts with minus\n+@@ this looks like a header";
        let hunk = Hunk::parse(text).unwrap();
        assert_eq!(hunk.new.1.len(), 3);
        assert_eq!(hunk.new.1[0], "+++ this line starts with plus");
        assert_eq!(hunk.new.1[1], "--- this line starts with minus");
        assert_eq!(hunk.new.1[2], "@@ this looks like a header");
    }

    #[test]
    fn parse_empty_line_content() {
        let text = "@@ -10,0 +11,3 @@\n+first\n+\n+third";
        let hunk = Hunk::parse(text).unwrap();
        assert_eq!(hunk.new.1, vec!["first", "", "third"]);
    }

    #[test]
    fn render_empty_line_content() {
        let hunk = Hunk {
            old: ModifiedLines(10, vec![]),
            new: ModifiedLines(
                11,
                vec!["first".to_string(), "".to_string(), "third".to_string()],
            ),
        };
        assert_eq!(hunk.to_string(), "@@ -10,0 +11,3 @@\n+first\n+\n+third\n");
    }

    #[test]
    fn retain_non_contiguous_lines() {
        let hunk = Hunk {
            old: ModifiedLines(9, vec![]),
            new: ModifiedLines(
                10,
                vec![
                    "ten".to_string(),
                    "eleven".to_string(),
                    "twelve".to_string(),
                    "thirteen".to_string(),
                ],
            ),
        };

        let filtered = hunk.retain(|_| false, |n| n == 10 || n == 12).unwrap();

        assert_eq!(filtered.old.0, 9);
        assert_eq!(filtered.new.0, 10);
        assert_eq!(filtered.new.1, vec!["ten", "twelve"]);
    }

    #[test]
    fn retain_mixed_partial_selection() {
        let hunk = Hunk {
            old: ModifiedLines(
                10,
                vec![
                    "old one".to_string(),
                    "old two".to_string(),
                    "old three".to_string(),
                ],
            ),
            new: ModifiedLines(
                10,
                vec![
                    "new one".to_string(),
                    "new two".to_string(),
                    "new three".to_string(),
                ],
            ),
        };

        let filtered = hunk.retain(|o| o == 11, |n| n == 12).unwrap();

        assert_eq!(filtered.old.0, 11);
        assert_eq!(filtered.old.1, vec!["old two"]);
        assert_eq!(filtered.new.0, 12);
        assert_eq!(filtered.new.1, vec!["new three"]);
        assert_eq!(
            filtered.to_string(),
            "@@ -11 +12 @@\n-old two\n+new three\n"
        );
    }
}
