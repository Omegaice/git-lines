use super::file::FileDiff;

/// A complete git diff containing changes for multiple files.
///
/// This is the top-level structure representing the full output of `git diff`.
#[derive(Debug)]
pub struct Diff {
    /// All file diffs in this git diff
    pub files: Vec<FileDiff>,
}

impl Diff {
    /// Parse a complete git diff output into file diffs.
    ///
    /// Splits the input by `diff --git` markers and parses each section
    /// as a [`FileDiff`].
    ///
    /// Files that fail to parse are silently skipped.
    #[must_use]
    pub fn parse(text: &str) -> Self {
        let marker = "diff --git ";

        // Find all marker positions
        let indices: Vec<usize> = text.match_indices(marker).map(|(i, _)| i).collect();

        if indices.is_empty() {
            return Diff { files: Vec::new() };
        }

        // Parse each section between markers
        let files = indices
            .iter()
            .enumerate()
            .filter_map(|(i, &start)| {
                let end = indices.get(i + 1).copied().unwrap_or(text.len());
                FileDiff::parse(&text[start..end])
            })
            .collect();

        Diff { files }
    }

    /// Filter lines across all files, returning a new Diff with only matching lines.
    ///
    /// This is the main filtering method used by git-stager to select specific lines
    /// for staging.
    ///
    /// # Parameters
    ///
    /// - `keep_old`: Predicate for deletions. Called with `(file_path, old_line_number)`.
    /// - `keep_new`: Predicate for additions. Called with `(file_path, new_line_number)`.
    ///
    /// # Returns
    ///
    /// A new `Diff` containing only:
    /// - Files that had matching lines
    /// - Hunks within those files that had matching lines
    /// - Lines within those hunks that matched the predicates
    ///
    /// Files and hunks with no matches are omitted entirely.
    ///
    /// # Example
    ///
    /// ```
    /// use git_stager::diff::Diff;
    ///
    /// let diff = Diff::parse("...git diff output...");
    ///
    /// // Keep only line 137 from flake.nix
    /// let filtered = diff.retain(
    ///     |_, _| false,
    ///     |path, line| path == "flake.nix" && line == 137
    /// );
    /// ```
    #[must_use]
    pub fn retain<F, G>(&self, mut keep_old: F, mut keep_new: G) -> Self
    where
        F: FnMut(&str, u32) -> bool,
        G: FnMut(&str, u32) -> bool,
    {
        let filtered_files: Vec<FileDiff> = self
            .files
            .iter()
            .filter_map(|file_diff| {
                file_diff.retain(
                    |old| keep_old(&file_diff.path, old),
                    |new| keep_new(&file_diff.path, new),
                )
            })
            .collect();

        Diff {
            files: filtered_files,
        }
    }
}

impl std::fmt::Display for Diff {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for file_diff in &self.files {
            write!(f, "{}", file_diff)?;
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
    fn parse_empty_diff() {
        let diff = Diff::parse("");
        assert_eq!(diff.files.len(), 0);
    }

    #[test]
    fn parse_single_file() {
        let text = r#"diff --git a/flake.nix b/flake.nix
index abc1234..def5678 100644
--- a/flake.nix
+++ b/flake.nix
@@ -136,0 +137 @@
+      debug = true;
"#;
        let diff = Diff::parse(text);
        assert_eq!(diff.files.len(), 1);
        assert_eq!(diff.files[0].path, "flake.nix");
        assert_eq!(diff.files[0].hunks.len(), 1);
    }

    #[test]
    fn parse_multiple_files() {
        let text = r#"diff --git a/flake.nix b/flake.nix
index abc1234..def5678 100644
--- a/flake.nix
+++ b/flake.nix
@@ -136,0 +137 @@
+      debug = true;
diff --git a/gtk.nix b/gtk.nix
index 111..222 100644
--- a/gtk.nix
+++ b/gtk.nix
@@ -11,0 +12 @@
+    gtk.cursorTheme.size = 24;
"#;
        let diff = Diff::parse(text);
        assert_eq!(diff.files.len(), 2);
        assert_eq!(diff.files[0].path, "flake.nix");
        assert_eq!(diff.files[1].path, "gtk.nix");
    }

    #[test]
    fn retain_single_file() {
        let text = r#"diff --git a/config.nix b/config.nix
index fa2da6e..41114ff 100644
--- a/config.nix
+++ b/config.nix
@@ -2,0 +3 @@ line 2
+# FIRST INSERTION
@@ -8,0 +10 @@ line 8
+# SECOND INSERTION
"#;
        let diff = Diff::parse(text);

        // Keep only line 10 from config.nix
        let filtered = diff.retain(
            |_, _| false,
            |path, line| path == "config.nix" && line == 10,
        );

        assert_eq!(filtered.files.len(), 1);
        assert_eq!(filtered.files[0].hunks.len(), 1);
        assert_eq!(filtered.files[0].hunks[0].old.start, 8); // Preserves original position!
        // new_start is recalculated: 8 + 1 = 9 (fixes the out-of-order bug)
        assert_eq!(filtered.files[0].hunks[0].new.start, 9);
    }

    #[test]
    fn retain_from_multiple_files() {
        let text = r#"diff --git a/flake.nix b/flake.nix
index abc1234..def5678 100644
--- a/flake.nix
+++ b/flake.nix
@@ -136,0 +137 @@
+      debug = true;
diff --git a/gtk.nix b/gtk.nix
index 111..222 100644
--- a/gtk.nix
+++ b/gtk.nix
@@ -11,0 +12 @@
+    gtk.cursorTheme.size = 24;
"#;
        let diff = Diff::parse(text);

        // Keep line 137 from flake.nix and line 12 from gtk.nix
        let filtered = diff.retain(
            |_, _| false,
            |path, line| (path == "flake.nix" && line == 137) || (path == "gtk.nix" && line == 12),
        );

        assert_eq!(filtered.files.len(), 2);
        assert_eq!(filtered.files[0].path, "flake.nix");
        assert_eq!(filtered.files[1].path, "gtk.nix");
    }

    #[test]
    fn retain_filters_out_empty_files() {
        let text = r#"diff --git a/flake.nix b/flake.nix
index abc1234..def5678 100644
--- a/flake.nix
+++ b/flake.nix
@@ -136,0 +137 @@
+      debug = true;
diff --git a/gtk.nix b/gtk.nix
index 111..222 100644
--- a/gtk.nix
+++ b/gtk.nix
@@ -11,0 +12 @@
+    gtk.cursorTheme.size = 24;
"#;
        let diff = Diff::parse(text);

        // Keep only line 137 from flake.nix (gtk.nix should be filtered out)
        let filtered = diff.retain(
            |_, _| false,
            |path, line| path == "flake.nix" && line == 137,
        );

        assert_eq!(filtered.files.len(), 1);
        assert_eq!(filtered.files[0].path, "flake.nix");
    }

    #[test]
    fn render_multiple_files() {
        let text = r#"diff --git a/flake.nix b/flake.nix
index abc1234..def5678 100644
--- a/flake.nix
+++ b/flake.nix
@@ -136,0 +137 @@
+      debug = true;
diff --git a/gtk.nix b/gtk.nix
index 111..222 100644
--- a/gtk.nix
+++ b/gtk.nix
@@ -11,0 +12 @@
+    gtk.cursorTheme.size = 24;
"#;
        let diff = Diff::parse(text);
        let rendered = diff.to_string();

        assert!(rendered.contains("--- a/flake.nix"));
        assert!(rendered.contains("+++ b/flake.nix"));
        assert!(rendered.contains("@@ -136,0 +137 @@"));
        assert!(rendered.contains("+      debug = true;"));
        assert!(rendered.contains("--- a/gtk.nix"));
        assert!(rendered.contains("+++ b/gtk.nix"));
        assert!(rendered.contains("@@ -11,0 +12 @@"));
        assert!(rendered.contains("+    gtk.cursorTheme.size = 24;"));
    }
}

#[cfg(test)]
mod proptests {
    use super::*;
    use crate::diff::hunk::{Hunk, ModifiedLines};
    use proptest::prelude::*;
    use std::collections::HashSet;

    /// Generate line content
    fn arb_line_content() -> impl Strategy<Value = String> {
        prop::collection::vec(prop::char::range(' ', '~'), 0..15)
            .prop_map(|chars| chars.into_iter().collect())
    }

    /// Generate a simple FileDiff with one hunk
    fn arb_simple_file(name: &'static str, old_start: u32) -> impl Strategy<Value = FileDiff> {
        prop::collection::vec(arb_line_content(), 1..3).prop_map(move |lines| FileDiff {
            path: name.to_string(),
            hunks: vec![Hunk {
                old: ModifiedLines {
                    start: old_start,
                    lines: vec![],
                    missing_final_newline: false,
                },
                new: ModifiedLines {
                    start: old_start + 1,
                    lines,
                    missing_final_newline: false,
                },
            }],
        })
    }

    /// Generate a Diff with multiple files
    fn arb_multi_file_diff() -> impl Strategy<Value = Diff> {
        (
            arb_simple_file("file_a.txt", 10),
            arb_simple_file("file_b.txt", 20),
        )
            .prop_map(|(f1, f2)| Diff {
                files: vec![f1, f2],
            })
    }

    /// Generate a set of line numbers to keep
    fn arb_line_set() -> impl Strategy<Value = HashSet<u32>> {
        prop::collection::hash_set(1..50u32, 0..10)
    }

    proptest! {
        /// Diff with multiple files must round-trip
        #[test]
        fn diff_roundtrips(diff in arb_multi_file_diff()) {
            let rendered = diff.to_string();
            let parsed = Diff::parse(&rendered);

            prop_assert_eq!(
                parsed.files.len(),
                diff.files.len(),
                "File count mismatch after round-trip"
            );

            for (orig, parsed) in diff.files.iter().zip(parsed.files.iter()) {
                prop_assert_eq!(&parsed.path, &orig.path);
            }
        }

        /// Filtered multi-file diff must maintain consistency
        #[test]
        fn filtered_diff_maintains_files(
            diff in arb_multi_file_diff(),
            keep_new in arb_line_set()
        ) {
            let filtered = diff.retain(
                |_, _| false,
                |_, l| keep_new.contains(&l)
            );

            // Each remaining file should have at least one hunk
            for file in &filtered.files {
                prop_assert!(
                    !file.hunks.is_empty(),
                    "File with no hunks should be filtered out: {:?}",
                    file
                );
            }

            // File paths should be unique
            let paths: HashSet<_> = filtered.files.iter().map(|f| &f.path).collect();
            prop_assert_eq!(
                paths.len(),
                filtered.files.len(),
                "Duplicate file paths in filtered diff"
            );
        }
    }
}
