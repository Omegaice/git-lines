use super::file::FileDiff;

/// A complete git diff containing changes for multiple files
pub struct Diff {
    pub files: Vec<FileDiff>,
}

impl Diff {
    /// Parse a complete git diff output into file diffs
    pub fn parse(text: &str) -> Self {
        let mut files = Vec::new();
        let mut current_file_text = String::new();

        for line in text.lines() {
            if line.starts_with("diff --git ") {
                // Start of new file diff - save previous if exists
                if !current_file_text.is_empty()
                    && let Some(file_diff) = FileDiff::parse(&current_file_text)
                {
                    files.push(file_diff);
                }
                current_file_text = line.to_string();
                current_file_text.push('\n');
            } else if !current_file_text.is_empty() {
                current_file_text.push_str(line);
                current_file_text.push('\n');
            }
        }

        // Don't forget the last file
        if !current_file_text.is_empty()
            && let Some(file_diff) = FileDiff::parse(&current_file_text)
        {
            files.push(file_diff);
        }

        Diff { files }
    }

    /// Filter lines across all files, returning a new Diff with only matching lines.
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
mod tests {
    use super::*;

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
