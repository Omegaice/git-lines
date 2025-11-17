use super::hunk::Hunk;
use std::fmt;

/// A complete diff for a single file
pub struct FileDiff {
    pub path: String,
    pub hunks: Vec<Hunk>,
}

impl FileDiff {
    /// Parse a single-file diff from git diff output
    pub fn parse(text: &str) -> Option<Self> {
        let mut lines = text.lines().peekable();
        let mut path = String::new();

        // Find file path from +++ b/... header
        for line in lines.by_ref() {
            if let Some(p) = line.strip_prefix("+++ b/") {
                path = p.to_string();
                break;
            }
        }

        if path.is_empty() {
            return None;
        }

        // Parse hunks
        let mut hunks = Vec::new();
        let mut current_hunk_text = String::new();

        for line in lines {
            if line.starts_with("@@ ") {
                // Start of new hunk - save previous if exists
                if !current_hunk_text.is_empty()
                    && let Some(hunk) = Hunk::parse(&current_hunk_text)
                {
                    hunks.push(hunk);
                }
                current_hunk_text = line.to_string();
                current_hunk_text.push('\n');
            } else if line.starts_with('+') || line.starts_with('-') {
                // Content line
                current_hunk_text.push_str(line);
                current_hunk_text.push('\n');
            }
        }

        // Don't forget the last hunk
        if !current_hunk_text.is_empty()
            && let Some(hunk) = Hunk::parse(&current_hunk_text)
        {
            hunks.push(hunk);
        }

        Some(FileDiff { path, hunks })
    }

    /// Filter lines across all hunks, returning a new FileDiff with only matching lines.
    /// Returns None if no lines match in any hunk.
    pub fn retain<F, G>(&self, mut keep_old: F, mut keep_new: G) -> Option<Self>
    where
        F: FnMut(u32) -> bool,
        G: FnMut(u32) -> bool,
    {
        let filtered_hunks: Vec<Hunk> = self
            .hunks
            .iter()
            .filter_map(|hunk| hunk.retain(&mut keep_old, &mut keep_new))
            .collect();

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
        writeln!(f, "--- a/{}", self.path)?;
        writeln!(f, "+++ b/{}", self.path)?;

        for hunk in &self.hunks {
            write!(f, "{}", hunk)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diff::hunk::ModifiedLines;

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
        assert_eq!(file_diff.hunks[0].old.0, 136);
        assert_eq!(file_diff.hunks[0].new.0, 137);
        assert_eq!(file_diff.hunks[0].new.1, vec!["      debug = true;"]);
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

        assert_eq!(file_diff.hunks[0].old.0, 2);
        assert_eq!(file_diff.hunks[0].new.0, 3);
        assert_eq!(file_diff.hunks[0].new.1, vec!["# FIRST INSERTION"]);

        assert_eq!(file_diff.hunks[1].old.0, 8);
        assert_eq!(file_diff.hunks[1].new.0, 10);
        assert_eq!(file_diff.hunks[1].new.1, vec!["# SECOND INSERTION"]);
    }

    #[test]
    fn render_single_hunk() {
        let file_diff = FileDiff {
            path: "test.nix".to_string(),
            hunks: vec![Hunk {
                old: ModifiedLines(10, vec![]),
                new: ModifiedLines(11, vec!["new line".to_string()]),
            }],
        };

        assert_eq!(
            file_diff.to_string(),
            "--- a/test.nix\n+++ b/test.nix\n@@ -10,0 +11 @@\n+new line\n"
        );
    }

    #[test]
    fn render_multiple_hunks() {
        let file_diff = FileDiff {
            path: "config.nix".to_string(),
            hunks: vec![
                Hunk {
                    old: ModifiedLines(2, vec![]),
                    new: ModifiedLines(3, vec!["# FIRST".to_string()]),
                },
                Hunk {
                    old: ModifiedLines(8, vec![]),
                    new: ModifiedLines(10, vec!["# SECOND".to_string()]),
                },
            ],
        };

        assert_eq!(
            file_diff.to_string(),
            "--- a/config.nix\n+++ b/config.nix\n@@ -2,0 +3 @@\n+# FIRST\n@@ -8,0 +10 @@\n+# SECOND\n"
        );
    }

    #[test]
    fn roundtrip_single_hunk() {
        let file_diff = FileDiff {
            path: "test.nix".to_string(),
            hunks: vec![Hunk {
                old: ModifiedLines(10, vec![]),
                new: ModifiedLines(11, vec!["new line".to_string()]),
            }],
        };

        let rendered = file_diff.to_string();
        let reparsed = FileDiff::parse(&rendered).unwrap();

        assert_eq!(reparsed.path, file_diff.path);
        assert_eq!(reparsed.hunks.len(), 1);
        assert_eq!(reparsed.hunks[0].old.0, 10);
        assert_eq!(reparsed.hunks[0].new.0, 11);
        assert_eq!(reparsed.hunks[0].new.1, vec!["new line"]);
    }

    #[test]
    fn roundtrip_multiple_hunks() {
        let file_diff = FileDiff {
            path: "config.nix".to_string(),
            hunks: vec![
                Hunk {
                    old: ModifiedLines(2, vec![]),
                    new: ModifiedLines(3, vec!["# FIRST".to_string()]),
                },
                Hunk {
                    old: ModifiedLines(8, vec![]),
                    new: ModifiedLines(10, vec!["# SECOND".to_string()]),
                },
            ],
        };

        let rendered = file_diff.to_string();
        let reparsed = FileDiff::parse(&rendered).unwrap();

        assert_eq!(reparsed.path, file_diff.path);
        assert_eq!(reparsed.hunks.len(), 2);
        assert_eq!(reparsed.hunks[0].old.0, 2);
        assert_eq!(reparsed.hunks[0].new.0, 3);
        assert_eq!(reparsed.hunks[1].old.0, 8);
        assert_eq!(reparsed.hunks[1].new.0, 10);
    }

    #[test]
    fn retain_second_hunk_only() {
        let file_diff = FileDiff {
            path: "config.nix".to_string(),
            hunks: vec![
                Hunk {
                    old: ModifiedLines(2, vec![]),
                    new: ModifiedLines(3, vec!["# FIRST".to_string()]),
                },
                Hunk {
                    old: ModifiedLines(8, vec![]),
                    new: ModifiedLines(10, vec!["# SECOND".to_string()]),
                },
            ],
        };

        let filtered = file_diff.retain(|_| false, |n| n == 10).unwrap();

        assert_eq!(filtered.path, "config.nix");
        assert_eq!(filtered.hunks.len(), 1);
        assert_eq!(filtered.hunks[0].old.0, 8);
        // new_start is recalculated: old_start + 1 = 8 + 1 = 9
        // This is the FIX for the out-of-order staging bug!
        assert_eq!(filtered.hunks[0].new.0, 9);
        assert_eq!(filtered.hunks[0].new.1, vec!["# SECOND"]);

        assert_eq!(
            filtered.to_string(),
            "--- a/config.nix\n+++ b/config.nix\n@@ -8,0 +9 @@\n+# SECOND\n"
        );
    }

    #[test]
    fn retain_nothing_returns_none() {
        let file_diff = FileDiff {
            path: "test.nix".to_string(),
            hunks: vec![Hunk {
                old: ModifiedLines(10, vec![]),
                new: ModifiedLines(11, vec!["line".to_string()]),
            }],
        };

        let filtered = file_diff.retain(|_| false, |_| false);
        assert!(filtered.is_none());
    }
}
