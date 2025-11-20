use super::hunk::Hunk;
use std::fmt;

/// A complete diff for a single file.
///
/// Contains all hunks (change blocks) for one file from a git diff.
#[derive(Debug, PartialEq, Eq)]
pub struct FileDiff {
    /// File path (extracted from `+++ b/path` header)
    pub path: String,
    /// All hunks for this file
    pub hunks: Vec<Hunk>,
}

impl FileDiff {
    /// Parse a single-file diff from git diff output.
    ///
    /// Expects input starting with `diff --git` and containing `+++ b/path` header.
    ///
    /// Returns `None` if the file path cannot be extracted.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        // Extract path from +++ b/... header
        let path = text
            .lines()
            .find_map(|line| line.strip_prefix("+++ b/"))
            .filter(|p| !p.is_empty())?
            .to_string();

        // Find first hunk marker
        let first_hunk_pos = text.find("\n@@ ").map(|i| i + 1)?;

        // Find all subsequent hunk markers
        let mut indices = vec![first_hunk_pos];
        let mut search_start = first_hunk_pos + 1;

        while let Some(pos) = text[search_start..].find("\n@@ ") {
            let abs_pos = search_start + pos + 1; // +1 to skip the newline
            indices.push(abs_pos);
            search_start = abs_pos + 1;
        }

        // Parse each hunk section
        let hunks = indices
            .iter()
            .enumerate()
            .filter_map(|(i, &start)| {
                let end = indices.get(i + 1).copied().unwrap_or(text.len());
                Hunk::parse(&text[start..end])
            })
            .collect();

        Some(FileDiff { path, hunks })
    }

    /// Filter lines across all hunks, returning a new FileDiff with only matching lines.
    ///
    /// Applies the predicates to every line across all hunks in the file.
    ///
    /// # Parameters
    ///
    /// - `keep_old`: Predicate for deletions (old line numbers)
    /// - `keep_new`: Predicate for additions (new line numbers)
    ///
    /// # Returns
    ///
    /// - `Some(FileDiff)` containing only hunks with matching lines
    /// - `None` if no lines matched in any hunk
    #[must_use]
    pub fn retain<F, G>(&self, mut keep_old: F, mut keep_new: G) -> Option<Self>
    where
        F: FnMut(u32) -> bool,
        G: FnMut(u32) -> bool,
    {
        let mut filtered_hunks = Vec::new();
        let mut cumulative_additions: i32 = 0;
        let mut cumulative_deletions: i32 = 0;

        for hunk in &self.hunks {
            if let Some(mut filtered) = hunk.retain(&mut keep_old, &mut keep_new) {
                // Adjust new_start by cumulative effect of previous filtered hunks
                let adjustment = cumulative_additions - cumulative_deletions;
                filtered.new.start = (filtered.new.start as i32 + adjustment) as u32;

                // Update cumulatives for next hunk
                cumulative_additions += filtered.new.lines.len() as i32;
                cumulative_deletions += filtered.old.lines.len() as i32;

                filtered_hunks.push(filtered);
            }
        }

        if filtered_hunks.is_empty() {
            None
        } else {
            Some(FileDiff {
                path: self.path.clone(),
                hunks: filtered_hunks,
            })
        }
    }
}

impl fmt::Display for FileDiff {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "diff --git a/{} b/{}", self.path, self.path)?;
        writeln!(f, "--- a/{}", self.path)?;
        writeln!(f, "+++ b/{}", self.path)?;

        for hunk in &self.hunks {
            write!(f, "{}", hunk)?;
        }

        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::diff::hunk::ModifiedLines;
    use similar_asserts::assert_eq;

    #[test]
    fn parse_single_hunk() {
        let diff = r#"diff --git a/flake.nix b/flake.nix
index abc1234..def5678 100644
--- a/flake.nix
+++ b/flake.nix
@@ -136,0 +137 @@
+      debug = true;
"#;
        let file_diff = FileDiff::parse(diff).unwrap();
        assert_eq!(file_diff.path, "flake.nix");
        assert_eq!(file_diff.hunks.len(), 1);
        assert_eq!(file_diff.hunks[0].old.start, 136);
        assert_eq!(file_diff.hunks[0].new.start, 137);
        assert_eq!(file_diff.hunks[0].new.lines, vec!["      debug = true;"]);
    }

    #[test]
    fn parse_multiple_hunks() {
        let diff = r#"diff --git a/config.nix b/config.nix
index fa2da6e..41114ff 100644
--- a/config.nix
+++ b/config.nix
@@ -2,0 +3 @@ line 2
+# FIRST INSERTION
@@ -8,0 +10 @@ line 8
+# SECOND INSERTION
"#;
        let file_diff = FileDiff::parse(diff).unwrap();
        assert_eq!(file_diff.path, "config.nix");
        assert_eq!(file_diff.hunks.len(), 2);

        assert_eq!(file_diff.hunks[0].old.start, 2);
        assert_eq!(file_diff.hunks[0].new.start, 3);
        assert_eq!(file_diff.hunks[0].new.lines, vec!["# FIRST INSERTION"]);

        assert_eq!(file_diff.hunks[1].old.start, 8);
        assert_eq!(file_diff.hunks[1].new.start, 10);
        assert_eq!(file_diff.hunks[1].new.lines, vec!["# SECOND INSERTION"]);
    }

    #[test]
    fn render_single_hunk() {
        let file_diff = FileDiff {
            path: "test.nix".to_string(),
            hunks: vec![Hunk {
                old: ModifiedLines {
                    start: 10,
                    lines: vec![],
                    missing_final_newline: false,
                },
                new: ModifiedLines {
                    start: 11,
                    lines: vec!["new line".to_string()],
                    missing_final_newline: false,
                },
            }],
        };

        assert_eq!(
            file_diff.to_string(),
            "diff --git a/test.nix b/test.nix\n--- a/test.nix\n+++ b/test.nix\n@@ -10,0 +11 @@\n+new line\n"
        );
    }

    #[test]
    fn render_multiple_hunks() {
        let file_diff = FileDiff {
            path: "config.nix".to_string(),
            hunks: vec![
                Hunk {
                    old: ModifiedLines {
                        start: 2,
                        lines: vec![],
                        missing_final_newline: false,
                    },
                    new: ModifiedLines {
                        start: 3,
                        lines: vec!["# FIRST".to_string()],
                        missing_final_newline: false,
                    },
                },
                Hunk {
                    old: ModifiedLines {
                        start: 8,
                        lines: vec![],
                        missing_final_newline: false,
                    },
                    new: ModifiedLines {
                        start: 10,
                        lines: vec!["# SECOND".to_string()],
                        missing_final_newline: false,
                    },
                },
            ],
        };

        assert_eq!(
            file_diff.to_string(),
            "diff --git a/config.nix b/config.nix\n--- a/config.nix\n+++ b/config.nix\n@@ -2,0 +3 @@\n+# FIRST\n@@ -8,0 +10 @@\n+# SECOND\n"
        );
    }

    #[test]
    fn roundtrip_single_hunk() {
        let file_diff = FileDiff {
            path: "test.nix".to_string(),
            hunks: vec![Hunk {
                old: ModifiedLines {
                    start: 10,
                    lines: vec![],
                    missing_final_newline: false,
                },
                new: ModifiedLines {
                    start: 11,
                    lines: vec!["new line".to_string()],
                    missing_final_newline: false,
                },
            }],
        };

        let rendered = file_diff.to_string();
        let reparsed = FileDiff::parse(&rendered).unwrap();

        assert_eq!(reparsed.path, file_diff.path);
        assert_eq!(reparsed.hunks.len(), 1);
        assert_eq!(reparsed.hunks[0].old.start, 10);
        assert_eq!(reparsed.hunks[0].new.start, 11);
        assert_eq!(reparsed.hunks[0].new.lines, vec!["new line"]);
    }

    #[test]
    fn roundtrip_multiple_hunks() {
        let file_diff = FileDiff {
            path: "config.nix".to_string(),
            hunks: vec![
                Hunk {
                    old: ModifiedLines {
                        start: 2,
                        lines: vec![],
                        missing_final_newline: false,
                    },
                    new: ModifiedLines {
                        start: 3,
                        lines: vec!["# FIRST".to_string()],
                        missing_final_newline: false,
                    },
                },
                Hunk {
                    old: ModifiedLines {
                        start: 8,
                        lines: vec![],
                        missing_final_newline: false,
                    },
                    new: ModifiedLines {
                        start: 10,
                        lines: vec!["# SECOND".to_string()],
                        missing_final_newline: false,
                    },
                },
            ],
        };

        let rendered = file_diff.to_string();
        let reparsed = FileDiff::parse(&rendered).unwrap();

        assert_eq!(reparsed.path, file_diff.path);
        assert_eq!(reparsed.hunks.len(), 2);
        assert_eq!(reparsed.hunks[0].old.start, 2);
        assert_eq!(reparsed.hunks[0].new.start, 3);
        assert_eq!(reparsed.hunks[1].old.start, 8);
        assert_eq!(reparsed.hunks[1].new.start, 10);
    }

    #[test]
    fn retain_second_hunk_only() {
        let file_diff = FileDiff {
            path: "config.nix".to_string(),
            hunks: vec![
                Hunk {
                    old: ModifiedLines {
                        start: 2,
                        lines: vec![],
                        missing_final_newline: false,
                    },
                    new: ModifiedLines {
                        start: 3,
                        lines: vec!["# FIRST".to_string()],
                        missing_final_newline: false,
                    },
                },
                Hunk {
                    old: ModifiedLines {
                        start: 8,
                        lines: vec![],
                        missing_final_newline: false,
                    },
                    new: ModifiedLines {
                        start: 10,
                        lines: vec!["# SECOND".to_string()],
                        missing_final_newline: false,
                    },
                },
            ],
        };

        let filtered = file_diff.retain(|_| false, |n| n == 10).unwrap();

        assert_eq!(filtered.path, "config.nix");
        assert_eq!(filtered.hunks.len(), 1);
        assert_eq!(filtered.hunks[0].old.start, 8);
        // new_start is recalculated: old_start + 1 = 8 + 1 = 9
        // This is the FIX for the out-of-order staging bug!
        assert_eq!(filtered.hunks[0].new.start, 9);
        assert_eq!(filtered.hunks[0].new.lines, vec!["# SECOND"]);

        assert_eq!(
            filtered.to_string(),
            "diff --git a/config.nix b/config.nix\n--- a/config.nix\n+++ b/config.nix\n@@ -8,0 +9 @@\n+# SECOND\n"
        );
    }

    #[test]
    fn retain_from_multiple_hunks_adjusts_line_numbers() {
        // When filtering lines from multiple hunks, later hunks' new_start positions
        // must account for the net line changes from earlier filtered hunks.
        //
        // Scenario: Two insertion hunks in a file
        // - Hunk 1: Inserts 2 lines after old line 3 (new lines 4-5)
        // - Hunk 2: Inserts 2 lines after old line 7 (new lines 10-11)
        //   Note: new_start is 10, not 8, because hunk 1 added 2 lines
        //
        // When we keep only line 4 from hunk 1 and line 10 from hunk 2:
        // - Hunk 1 now adds 1 line instead of 2 (net change: -1)
        // - Hunk 2's new_start must adjust: 10 - 1 = 9
        let file_diff = FileDiff {
            path: "test.txt".to_string(),
            hunks: vec![
                Hunk {
                    old: ModifiedLines {
                        start: 3,
                        lines: vec![],
                        missing_final_newline: false,
                    },
                    new: ModifiedLines {
                        start: 4,
                        lines: vec!["NEW 1".to_string(), "NEW 2".to_string()],
                        missing_final_newline: false,
                    },
                },
                Hunk {
                    old: ModifiedLines {
                        start: 7,
                        lines: vec![],
                        missing_final_newline: false,
                    },
                    new: ModifiedLines {
                        start: 10,
                        lines: vec!["NEW 3".to_string(), "NEW 4".to_string()],
                        missing_final_newline: false,
                    },
                },
            ],
        };

        let filtered = file_diff.retain(|_| false, |n| n == 4 || n == 10).unwrap();

        // Expected result: Both hunks filtered, with hunk 2's new_start adjusted
        // to account for the reduced line count from hunk 1
        let expected = FileDiff {
            path: "test.txt".to_string(),
            hunks: vec![
                Hunk {
                    old: ModifiedLines {
                        start: 3,
                        lines: vec![],
                        missing_final_newline: false,
                    },
                    new: ModifiedLines {
                        start: 4,
                        lines: vec!["NEW 1".to_string()],
                        missing_final_newline: false,
                    },
                },
                Hunk {
                    old: ModifiedLines {
                        start: 7,
                        lines: vec![],
                        missing_final_newline: false,
                    },
                    new: ModifiedLines {
                        start: 9, // Adjusted from 10: accounts for 1 line from hunk 1 instead of 2
                        lines: vec!["NEW 3".to_string()],
                        missing_final_newline: false,
                    },
                },
            ],
        };

        assert_eq!(filtered, expected);
    }

    #[test]
    fn retain_nothing_returns_none() {
        let file_diff = FileDiff {
            path: "test.nix".to_string(),
            hunks: vec![Hunk {
                old: ModifiedLines {
                    start: 10,
                    lines: vec![],
                    missing_final_newline: false,
                },
                new: ModifiedLines {
                    start: 11,
                    lines: vec!["line".to_string()],
                    missing_final_newline: false,
                },
            }],
        };

        let filtered = file_diff.retain(|_| false, |_| false);
        assert!(filtered.is_none());
    }

    #[test]
    fn parse_no_newline_at_eof_marker() {
        let diff = r#"diff --git a/config.nix b/config.nix
index 79e51de..88ee0b1 100644
--- a/config.nix
+++ b/config.nix
@@ -3 +3,2 @@ line 2
-no newline
\ No newline at end of file
+no newline
+new line
\ No newline at end of file
"#;
        let file_diff = FileDiff::parse(diff).unwrap();
        assert_eq!(file_diff.path, "config.nix");
        assert_eq!(file_diff.hunks.len(), 1);

        // The hunk should preserve the "no newline" information
        // Currently this fails: the marker is stripped and lost
        assert_eq!(
            file_diff.to_string(),
            "diff --git a/config.nix b/config.nix\n--- a/config.nix\n+++ b/config.nix\n@@ -3 +3,2 @@\n-no newline\n\\ No newline at end of file\n+no newline\n+new line\n\\ No newline at end of file\n"
        );
    }
}

#[cfg(test)]
mod proptests {
    use super::*;
    use crate::diff::hunk::ModifiedLines;
    use proptest::prelude::*;
    use std::collections::HashSet;

    /// Generate line content
    fn arb_line_content() -> impl Strategy<Value = String> {
        prop::collection::vec(prop::char::range(' ', '~'), 0..20)
            .prop_map(|chars| chars.into_iter().collect())
    }

    /// Generate a pure insertion hunk at a given position
    fn arb_insertion_hunk(old_start: u32, num_lines: usize) -> impl Strategy<Value = Hunk> {
        prop::collection::vec(arb_line_content(), num_lines..=num_lines).prop_map(
            move |new_lines| Hunk {
                old: ModifiedLines {
                    start: old_start,
                    lines: vec![],
                    missing_final_newline: false,
                },
                new: ModifiedLines {
                    start: old_start + 1,
                    lines: new_lines,
                    missing_final_newline: false,
                },
            },
        )
    }

    /// Generate a FileDiff with multiple non-overlapping insertion hunks
    fn arb_multi_hunk_file() -> impl Strategy<Value = FileDiff> {
        // Generate 2-3 hunks at different positions
        (
            arb_insertion_hunk(5, 2),  // 2 lines after line 5
            arb_insertion_hunk(15, 3), // 3 lines after line 15
            arb_insertion_hunk(30, 2), // 2 lines after line 30
        )
            .prop_map(|(h1, h2, h3)| FileDiff {
                path: "test.txt".to_string(),
                hunks: vec![h1, h2, h3],
            })
    }

    /// Generate a set of line numbers to keep
    fn arb_line_set() -> impl Strategy<Value = HashSet<u32>> {
        prop::collection::hash_set(1..50u32, 0..10)
    }

    /// Generate a pure deletion hunk at a given position
    fn arb_deletion_hunk(old_start: u32, num_lines: usize) -> impl Strategy<Value = Hunk> {
        prop::collection::vec(arb_line_content(), num_lines..=num_lines).prop_map(
            move |old_lines| Hunk {
                old: ModifiedLines {
                    start: old_start,
                    lines: old_lines,
                    missing_final_newline: false,
                },
                new: ModifiedLines {
                    start: old_start,
                    lines: vec![],
                    missing_final_newline: false,
                },
            },
        )
    }

    /// Generate a replacement hunk at a given position
    fn arb_replacement_hunk(start: u32) -> impl Strategy<Value = Hunk> {
        (
            prop::collection::vec(arb_line_content(), 1..3),
            prop::collection::vec(arb_line_content(), 1..3),
        )
            .prop_map(move |(old_lines, new_lines)| Hunk {
                old: ModifiedLines {
                    start,
                    lines: old_lines,
                    missing_final_newline: false,
                },
                new: ModifiedLines {
                    start,
                    lines: new_lines,
                    missing_final_newline: false,
                },
            })
    }

    /// Generate a FileDiff with mixed hunk types (insertions, deletions, replacements)
    fn arb_mixed_hunk_file() -> impl Strategy<Value = FileDiff> {
        (
            arb_insertion_hunk(5, 2), // insertion
            arb_deletion_hunk(15, 2), // deletion
            arb_replacement_hunk(30), // replacement
        )
            .prop_map(|(h1, h2, h3)| FileDiff {
                path: "mixed.txt".to_string(),
                hunks: vec![h1, h2, h3],
            })
    }

    proptest! {
        /// FileDiff round-trip: any multi-hunk file must survive render → parse
        #[test]
        fn file_diff_roundtrips(file_diff in arb_multi_hunk_file()) {
            let rendered = file_diff.to_string();
            let parsed = FileDiff::parse(&rendered);

            prop_assert!(
                parsed.is_some(),
                "Failed to parse rendered FileDiff:\n{}",
                rendered
            );

            let parsed = parsed.unwrap();
            prop_assert_eq!(parsed.path, file_diff.path);
            prop_assert_eq!(parsed.hunks.len(), file_diff.hunks.len());
        }

        /// Filtered FileDiff must round-trip
        #[test]
        fn filtered_file_diff_roundtrips(
            file_diff in arb_multi_hunk_file(),
            keep_new in arb_line_set()
        ) {
            if let Some(filtered) = file_diff.retain(
                |_| false,
                |l| keep_new.contains(&l)
            ) {
                let rendered = filtered.to_string();
                let parsed = FileDiff::parse(&rendered);

                prop_assert!(
                    parsed.is_some(),
                    "Failed to parse filtered FileDiff:\n{}\nOriginal: {:?}",
                    rendered, file_diff
                );
            }
        }

        /// Cumulative adjustment: when filtering multiple hunks, later hunks
        /// must have their new_start adjusted for the net effect of earlier hunks
        #[test]
        fn cumulative_adjustment_maintains_order(
            file_diff in arb_multi_hunk_file(),
            keep_new in arb_line_set()
        ) {
            if let Some(filtered) = file_diff.retain(
                |_| false,
                |l| keep_new.contains(&l)
            ) {
                // Verify hunks are still ordered by position
                for window in filtered.hunks.windows(2) {
                    prop_assert!(
                        window[0].old.start < window[1].old.start,
                        "Hunks not ordered by old_start: {:?}",
                        filtered.hunks
                    );
                }

                // Verify new_start values are monotonically increasing
                for window in filtered.hunks.windows(2) {
                    prop_assert!(
                        window[0].new.start < window[1].new.start,
                        "Hunks not ordered by new_start: {:?}",
                        filtered.hunks
                    );
                }
            }
        }

        /// Mixed hunk file (insertions, deletions, replacements) should round-trip
        #[test]
        fn mixed_hunk_file_roundtrips(file_diff in arb_mixed_hunk_file()) {
            let rendered = file_diff.to_string();
            let parsed = FileDiff::parse(&rendered);

            prop_assert!(
                parsed.is_some(),
                "Failed to parse mixed hunk file:\n{}",
                rendered
            );

            let parsed = parsed.unwrap();
            prop_assert_eq!(parsed.path, file_diff.path);
            prop_assert_eq!(parsed.hunks.len(), file_diff.hunks.len());
        }

        /// Filtered mixed hunk file should round-trip
        #[test]
        fn filtered_mixed_hunk_file_roundtrips(
            file_diff in arb_mixed_hunk_file(),
            keep_old in arb_line_set(),
            keep_new in arb_line_set()
        ) {
            if let Some(filtered) = file_diff.retain(
                |l| keep_old.contains(&l),
                |l| keep_new.contains(&l)
            ) {
                let rendered = filtered.to_string();
                let parsed = FileDiff::parse(&rendered);

                prop_assert!(
                    parsed.is_some(),
                    "Failed to parse filtered mixed hunk file:\n{}",
                    rendered
                );
            }
        }
    }
}
