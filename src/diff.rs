/// A single line change from a diff
#[derive(Debug, Clone, PartialEq)]
pub enum DiffLine {
    /// Added line with new line number and content
    Add { new_line: u32, content: String },
    /// Deleted line with old line number and content
    Delete { old_line: u32, content: String },
}

/// Parsed diff for a single file
#[derive(Debug, Clone, PartialEq)]
pub struct FileDiff {
    pub file_path: String,
    pub lines: Vec<DiffLine>,
}

/// Parse a unified diff (git diff -U0 output) into structured data
pub fn parse_diff(diff_output: &str) -> Result<FileDiff, String> {
    if diff_output.is_empty() {
        return Err("Empty diff".to_string());
    }

    let mut lines_iter = diff_output.lines().peekable();
    let mut file_path = String::new();
    let mut diff_lines = Vec::new();

    // Parse header to get file path
    for line in lines_iter.by_ref() {
        if line.starts_with("+++ b/") {
            file_path = line[6..].to_string();
            break;
        }
    }

    if file_path.is_empty() {
        return Err("Could not find file path in diff".to_string());
    }

    // Parse hunks
    let mut current_old_line = 0u32;
    let mut current_new_line = 0u32;

    for line in lines_iter {
        if line.starts_with("@@ ") {
            // Parse hunk header: @@ -old,count +new,count @@
            let (old_start, new_start) = parse_hunk_header(line)?;
            current_old_line = old_start;
            current_new_line = new_start;
        } else if let Some(content) = line.strip_prefix('+') {
            diff_lines.push(DiffLine::Add {
                new_line: current_new_line,
                content: content.to_string(),
            });
            current_new_line += 1;
        } else if let Some(content) = line.strip_prefix('-') {
            diff_lines.push(DiffLine::Delete {
                old_line: current_old_line,
                content: content.to_string(),
            });
            current_old_line += 1;
        }
        // Ignore context lines (shouldn't have any with -U0)
    }

    Ok(FileDiff {
        file_path,
        lines: diff_lines,
    })
}

/// Parse hunk header to extract old and new line numbers
/// Format: @@ -old_start,old_count +new_start,new_count @@ optional context
pub fn parse_hunk_header(header: &str) -> Result<(u32, u32), String> {
    // Find the @@ markers
    let header = header
        .strip_prefix("@@ ")
        .ok_or("Invalid hunk header format")?;
    let end_idx = header.find(" @@").ok_or("Invalid hunk header format")?;
    let range_part = &header[..end_idx];

    // Split into old and new parts: "-old,count +new,count"
    let parts: Vec<&str> = range_part.split(' ').collect();
    if parts.len() != 2 {
        return Err(format!("Invalid hunk header: {}", header));
    }

    let old_part = parts[0];
    let new_part = parts[1];

    let old_start = parse_range_start(old_part.strip_prefix('-').unwrap_or(old_part))?;
    let new_start = parse_range_start(new_part.strip_prefix('+').unwrap_or(new_part))?;

    Ok((old_start, new_start))
}

/// Parse the start line number from a range like "136,0" or "137"
fn parse_range_start(range: &str) -> Result<u32, String> {
    let num_str = if let Some(idx) = range.find(',') {
        &range[..idx]
    } else {
        range
    };

    num_str
        .parse::<u32>()
        .map_err(|_| format!("Invalid line number in range: {}", range))
}

/// Format git diff output for human-readable display with explicit line numbers
///
/// Transforms raw git diff -U0 output into a format where each changed line
/// is prefixed with its line number, making it easy to reference for staging.
///
/// Example output:
/// ```text
/// flake.nix:
///   +137:       debug = true;
///
///   +142:         ./flake-modules/home-manager.nix
/// ```
pub fn format_diff_output(diff_output: &str) -> Result<String, String> {
    if diff_output.trim().is_empty() {
        return Ok(String::new());
    }

    let mut result = String::new();
    let mut lines_iter = diff_output.lines().peekable();
    let mut current_old_line = 0u32;
    let mut current_new_line = 0u32;
    let mut first_hunk_in_file = true;
    let mut in_header = true; // Track if we're still in diff header section

    while let Some(line) = lines_iter.next() {
        if line.starts_with("diff --git") {
            // New file starting
            first_hunk_in_file = true;
            in_header = true;
        } else if line.starts_with("--- a/") {
            // Old file header - skip it
            continue;
        } else if line.starts_with("+++ b/") {
            // Extract file name and print header
            let current_file = &line[6..];
            if !result.is_empty() {
                result.push('\n'); // Blank line between files
            }
            result.push_str(current_file);
            result.push_str(":\n");
        } else if line.starts_with("@@ ") {
            // Parse hunk header to get line numbers
            let (old_start, new_start) = parse_hunk_header(line)?;
            current_old_line = old_start;
            current_new_line = new_start;
            in_header = false;

            // Add blank line between non-contiguous hunks (but not before first hunk)
            if !first_hunk_in_file {
                result.push('\n');
            }
            first_hunk_in_file = false;
        } else if !in_header {
            // Only process +/- lines after we've seen a hunk header
            if let Some(content) = line.strip_prefix('+') {
                // Addition line
                result.push_str(&format!("  +{}:\t{}\n", current_new_line, content));
                current_new_line += 1;
            } else if let Some(content) = line.strip_prefix('-') {
                // Deletion line
                result.push_str(&format!("  -{}:\t{}\n", current_old_line, content));
                current_old_line += 1;
            }
        }
        // Ignore other lines (index lines, etc.)
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_diff_single_addition() {
        let diff = r#"diff --git a/flake.nix b/flake.nix
index abc1234..def5678 100644
--- a/flake.nix
+++ b/flake.nix
@@ -136,0 +137 @@
+      debug = true;
"#;
        let result = parse_diff(diff).unwrap();
        assert_eq!(result.file_path, "flake.nix");
        assert_eq!(result.lines.len(), 1);
        assert_eq!(
            result.lines[0],
            DiffLine::Add {
                new_line: 137,
                content: "      debug = true;".to_string()
            }
        );
    }

    #[test]
    fn parse_diff_contiguous_additions() {
        let diff = r#"diff --git a/flake.nix b/flake.nix
index 462392e..0b97d01 100644
--- a/flake.nix
+++ b/flake.nix
@@ -38,0 +39,5 @@ line 38
+
+    stylix = {
+      url = "github:nix-community/stylix";
+      inputs.nixpkgs.follows = "nixpkgs";
+    };
"#;
        let result = parse_diff(diff).unwrap();
        assert_eq!(result.file_path, "flake.nix");
        assert_eq!(result.lines.len(), 5);
        assert_eq!(
            result.lines[0],
            DiffLine::Add {
                new_line: 39,
                content: "".to_string()
            }
        );
        assert_eq!(
            result.lines[1],
            DiffLine::Add {
                new_line: 40,
                content: "    stylix = {".to_string()
            }
        );
        assert_eq!(
            result.lines[4],
            DiffLine::Add {
                new_line: 43,
                content: "    };".to_string()
            }
        );
    }

    #[test]
    fn parse_diff_single_deletion() {
        let diff = r#"diff --git a/zsh.nix b/zsh.nix
index 6f2e06d..110fff0 100644
--- a/zsh.nix
+++ b/zsh.nix
@@ -15 +14,0 @@ line 14
-      enableAutosuggestions = true;
"#;
        let result = parse_diff(diff).unwrap();
        assert_eq!(result.file_path, "zsh.nix");
        assert_eq!(result.lines.len(), 1);
        assert_eq!(
            result.lines[0],
            DiffLine::Delete {
                old_line: 15,
                content: "      enableAutosuggestions = true;".to_string()
            }
        );
    }

    #[test]
    fn parse_diff_mixed_operations() {
        let diff = r#"diff --git a/gtk.nix b/gtk.nix
index 2ce966d..93d8dbc 100644
--- a/gtk.nix
+++ b/gtk.nix
@@ -10,2 +10,3 @@ line 9
-    gtk.theme.name = "Adwaita";
-    gtk.iconTheme.name = "Papirus";
+    # Theme managed by Stylix
+    gtk.iconTheme.name = "Papirus-Dark";
+    gtk.cursorTheme.size = 24;
"#;
        let result = parse_diff(diff).unwrap();
        assert_eq!(result.file_path, "gtk.nix");
        assert_eq!(result.lines.len(), 5);

        // First two are deletions (old lines 10 and 11)
        assert_eq!(
            result.lines[0],
            DiffLine::Delete {
                old_line: 10,
                content: "    gtk.theme.name = \"Adwaita\";".to_string()
            }
        );
        assert_eq!(
            result.lines[1],
            DiffLine::Delete {
                old_line: 11,
                content: "    gtk.iconTheme.name = \"Papirus\";".to_string()
            }
        );

        // Next three are additions (new lines 10, 11, 12)
        assert_eq!(
            result.lines[2],
            DiffLine::Add {
                new_line: 10,
                content: "    # Theme managed by Stylix".to_string()
            }
        );
        assert_eq!(
            result.lines[3],
            DiffLine::Add {
                new_line: 11,
                content: "    gtk.iconTheme.name = \"Papirus-Dark\";".to_string()
            }
        );
        assert_eq!(
            result.lines[4],
            DiffLine::Add {
                new_line: 12,
                content: "    gtk.cursorTheme.size = 24;".to_string()
            }
        );
    }

    #[test]
    fn parse_diff_empty() {
        let diff = "";
        let result = parse_diff(diff);
        assert!(result.is_err());
    }

    #[test]
    fn format_diff_single_addition() {
        let diff = r#"diff --git a/flake.nix b/flake.nix
index abc1234..def5678 100644
--- a/flake.nix
+++ b/flake.nix
@@ -136,0 +137 @@
+      debug = true;
"#;
        let formatted = format_diff_output(diff).unwrap();
        insta::assert_snapshot!(formatted);
    }

    #[test]
    fn format_diff_contiguous_additions() {
        let diff = r#"diff --git a/flake.nix b/flake.nix
index 462392e..0b97d01 100644
--- a/flake.nix
+++ b/flake.nix
@@ -38,0 +39,5 @@ line 38
+
+    stylix = {
+      url = "github:nix-community/stylix";
+      inputs.nixpkgs.follows = "nixpkgs";
+    };
"#;
        let formatted = format_diff_output(diff).unwrap();
        insta::assert_snapshot!(formatted);
    }

    #[test]
    fn format_diff_non_contiguous_hunks() {
        let diff = r#"diff --git a/flake.nix b/flake.nix
index abc1234..def5678 100644
--- a/flake.nix
+++ b/flake.nix
@@ -136,0 +137 @@
+      debug = true;
@@ -140,0 +142 @@
+        ./flake-modules/home-manager.nix
"#;
        let formatted = format_diff_output(diff).unwrap();
        insta::assert_snapshot!(formatted);
    }

    #[test]
    fn format_diff_single_deletion() {
        let diff = r#"diff --git a/zsh.nix b/zsh.nix
index 6f2e06d..110fff0 100644
--- a/zsh.nix
+++ b/zsh.nix
@@ -15 +14,0 @@ line 14
-      enableAutosuggestions = true;
"#;
        let formatted = format_diff_output(diff).unwrap();
        insta::assert_snapshot!(formatted);
    }

    #[test]
    fn format_diff_mixed_operations() {
        let diff = r#"diff --git a/gtk.nix b/gtk.nix
index 2ce966d..93d8dbc 100644
--- a/gtk.nix
+++ b/gtk.nix
@@ -10,2 +10,3 @@ line 9
-    gtk.theme.name = "Adwaita";
-    gtk.iconTheme.name = "Papirus";
+    # Theme managed by Stylix
+    gtk.iconTheme.name = "Papirus-Dark";
+    gtk.cursorTheme.size = 24;
"#;
        let formatted = format_diff_output(diff).unwrap();
        insta::assert_snapshot!(formatted);
    }

    #[test]
    fn format_diff_multiple_files() {
        let diff = r#"diff --git a/flake.nix b/flake.nix
index abc1234..def5678 100644
--- a/flake.nix
+++ b/flake.nix
@@ -136,0 +137 @@
+      debug = true;
diff --git a/zsh.nix b/zsh.nix
index 6f2e06d..110fff0 100644
--- a/zsh.nix
+++ b/zsh.nix
@@ -15 +14,0 @@ line 14
-      enableAutosuggestions = true;
"#;
        let formatted = format_diff_output(diff).unwrap();
        insta::assert_snapshot!(formatted);
    }
}
