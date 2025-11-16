use std::path::{Path, PathBuf};
use std::process::Command;

/// Main interface for git-stager operations
pub struct GitStager {
    repo_path: PathBuf,
}

impl GitStager {
    /// Create a new GitStager for the given repository path
    pub fn new(repo_path: impl Into<PathBuf>) -> Self {
        Self {
            repo_path: repo_path.into(),
        }
    }

    /// Stage specific lines from a file
    ///
    /// # Examples
    /// ```no_run
    /// # use git_stager::GitStager;
    /// let stager = GitStager::new(".");
    /// stager.stage("flake.nix:137").unwrap();
    /// stager.stage("file.nix:10..15").unwrap();
    /// stager.stage("config.nix:-10,-11,12").unwrap();
    /// ```
    pub fn stage(&self, file_ref: &str) -> Result<(), String> {
        let parsed = parse_file_refs(file_ref)?;
        stage_lines(&self.repo_path, &parsed)
    }

    /// Get formatted diff output for specified files (or all files if empty)
    ///
    /// Returns diff output formatted with explicit line numbers for easy staging.
    ///
    /// # Examples
    /// ```no_run
    /// # use git_stager::GitStager;
    /// let stager = GitStager::new(".");
    /// let diff = stager.diff(&[]).unwrap(); // all files
    /// let diff = stager.diff(&["flake.nix".to_string()]).unwrap(); // specific file
    /// ```
    pub fn diff(&self, files: &[String]) -> Result<String, String> {
        let raw_diff = self.get_raw_diff(files)?;
        format_diff_output(&raw_diff)
    }

    /// Get raw git diff output with zero context lines
    fn get_raw_diff(&self, files: &[String]) -> Result<String, String> {
        let mut args = vec![
            "-C",
            self.repo_path.to_str().unwrap_or("."),
            "diff",
            "--no-ext-diff",
            "-U0",
            "--no-color",
        ];

        let file_refs: Vec<&str> = files.iter().map(|s| s.as_str()).collect();
        args.extend(file_refs);

        let output = Command::new("git")
            .args(&args)
            .output()
            .map_err(|e| format!("Failed to run git diff: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("git diff failed: {}", stderr));
        }

        String::from_utf8(output.stdout)
            .map_err(|e| format!("Invalid UTF-8 in git diff output: {}", e))
    }
}

/// A reference to specific lines to stage
#[derive(Debug, Clone, PartialEq)]
pub enum LineRef {
    /// Addition at new line number
    Add(u32),
    /// Addition range (inclusive)
    AddRange(u32, u32),
    /// Deletion at old line number
    Delete(u32),
    /// Deletion range (inclusive)
    DeleteRange(u32, u32),
}

/// Parsed file reference with line selections
#[derive(Debug, Clone, PartialEq)]
pub struct FileLineRefs {
    pub file: String,
    pub refs: Vec<LineRef>,
}

/// Parse a file:refs string into structured data
/// Examples:
/// - "flake.nix:137" -> FileLineRefs { file: "flake.nix", refs: [Add(137)] }
/// - "file.nix:10..15" -> FileLineRefs { file: "file.nix", refs: [AddRange(10, 15)] }
/// - "file.nix:10,15,-20" -> FileLineRefs { file: "file.nix", refs: [Add(10), Add(15), Delete(20)] }
fn parse_file_refs(input: &str) -> Result<FileLineRefs, String> {
    let parts: Vec<&str> = input.splitn(2, ':').collect();
    if parts.len() != 2 {
        return Err(format!("Invalid format '{}': expected 'file:refs'", input));
    }

    let file = parts[0].trim();
    if file.is_empty() {
        return Err(format!(
            "Invalid format '{}': file name cannot be empty",
            input
        ));
    }

    let refs_str = parts[1];
    let refs = parse_line_refs(refs_str)?;

    Ok(FileLineRefs {
        file: file.to_string(),
        refs,
    })
}

/// Parse the line references part (after the colon)
/// Examples: "137", "10..15", "10,15,-20"
fn parse_line_refs(input: &str) -> Result<Vec<LineRef>, String> {
    let mut refs = Vec::new();

    for part in input.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }

        let line_ref = parse_single_ref(part)?;
        refs.push(line_ref);
    }

    if refs.is_empty() {
        return Err("No line references provided".to_string());
    }

    Ok(refs)
}

/// Parse a single line reference (could be single number, range, or deletion)
fn parse_single_ref(input: &str) -> Result<LineRef, String> {
    // Check for range syntax (N..M or -N..-M)
    if let Some(idx) = input.find("..") {
        let start_str = &input[..idx];
        let end_str = &input[idx + 2..];

        // Determine if it's a deletion range
        let is_delete = start_str.starts_with('-');

        if is_delete {
            let start = parse_delete_number(start_str)?;
            let end = parse_delete_number(end_str)?;
            Ok(LineRef::DeleteRange(start, end))
        } else {
            let start = parse_add_number(start_str)?;
            let end = parse_add_number(end_str)?;
            Ok(LineRef::AddRange(start, end))
        }
    } else if input.starts_with('-') {
        // Single deletion
        let num = parse_delete_number(input)?;
        Ok(LineRef::Delete(num))
    } else {
        // Single addition
        let num = parse_add_number(input)?;
        Ok(LineRef::Add(num))
    }
}

/// Parse a positive line number (for additions)
fn parse_add_number(input: &str) -> Result<u32, String> {
    input
        .parse::<u32>()
        .map_err(|_| format!("Invalid line number '{}'", input))
}

/// Parse a negative line number (for deletions)
fn parse_delete_number(input: &str) -> Result<u32, String> {
    if !input.starts_with('-') {
        return Err(format!(
            "Delete reference must start with '-', got '{}'",
            input
        ));
    }
    input[1..]
        .parse::<u32>()
        .map_err(|_| format!("Invalid delete line number '{}'", input))
}

/// Stage specific lines from a file
/// This is the internal implementation
fn stage_lines(repo_path: &Path, file_refs: &FileLineRefs) -> Result<(), String> {
    // 1. Get git diff for the file
    let diff_output = get_git_diff(repo_path, &file_refs.file)?;

    if diff_output.trim().is_empty() {
        return Err(format!("No changes found in {}", file_refs.file));
    }

    // 2. Parse diff to extract line changes
    let diff = parse_diff(&diff_output)?;

    // 3. Construct patch with only selected lines
    let patch = build_patch(&file_refs.file, &diff, &file_refs.refs)?;

    // 4. Apply patch via git apply --cached
    apply_patch(repo_path, &patch)?;

    Ok(())
}

/// Get git diff output for a specific file
fn get_git_diff(repo_path: &Path, file: &str) -> Result<String, String> {
    let output = Command::new("git")
        .args([
            "-C",
            repo_path.to_str().unwrap_or("."),
            "diff",
            "--no-ext-diff",
            "-U0",
            "--no-color",
            file,
        ])
        .output()
        .map_err(|e| format!("Failed to run git diff: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("git diff failed: {}", stderr));
    }

    String::from_utf8(output.stdout).map_err(|e| format!("Invalid UTF-8 in git diff output: {}", e))
}

/// Apply a patch to the git index
fn apply_patch(repo_path: &Path, patch: &str) -> Result<(), String> {
    let mut child = Command::new("git")
        .args([
            "-C",
            repo_path.to_str().unwrap_or("."),
            "apply",
            "--cached",
            "--unidiff-zero",
            "-",
        ])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to spawn git apply: {}", e))?;

    // Write patch to stdin
    if let Some(mut stdin) = child.stdin.take() {
        use std::io::Write;
        stdin
            .write_all(patch.as_bytes())
            .map_err(|e| format!("Failed to write patch to git apply: {}", e))?;
    }

    let output = child
        .wait_with_output()
        .map_err(|e| format!("Failed to wait for git apply: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("git apply failed: {}", stderr));
    }

    Ok(())
}

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
fn parse_diff(diff_output: &str) -> Result<FileDiff, String> {
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
fn parse_hunk_header(header: &str) -> Result<(u32, u32), String> {
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

/// Build a patch containing only the selected lines
fn build_patch(file_path: &str, diff: &FileDiff, refs: &[LineRef]) -> Result<String, String> {
    // Filter diff lines to only include selected ones
    let selected_lines = select_diff_lines(&diff.lines, refs)?;

    if selected_lines.is_empty() {
        return Err("No matching lines found for selection".to_string());
    }

    // Build the patch
    let mut patch = String::new();

    // Add file headers
    patch.push_str(&format!("--- a/{}\n", file_path));
    patch.push_str(&format!("+++ b/{}\n", file_path));

    // Group contiguous lines into hunks
    let hunks = group_into_hunks(&selected_lines);

    for hunk in hunks {
        let hunk_header = build_hunk_header(&hunk);
        patch.push_str(&hunk_header);
        patch.push('\n');

        for line in &hunk {
            match line {
                DiffLine::Add { content, .. } => {
                    patch.push('+');
                    patch.push_str(content);
                    patch.push('\n');
                }
                DiffLine::Delete { content, .. } => {
                    patch.push('-');
                    patch.push_str(content);
                    patch.push('\n');
                }
            }
        }
    }

    Ok(patch)
}

/// Select only the diff lines that match the given references
fn select_diff_lines(lines: &[DiffLine], refs: &[LineRef]) -> Result<Vec<DiffLine>, String> {
    let mut selected = Vec::new();

    for line in lines {
        if line_matches_refs(line, refs) {
            selected.push(line.clone());
        }
    }

    if selected.is_empty() && !refs.is_empty() {
        return Err("No lines matched the selection criteria in the unstaged diff".to_string());
    }

    Ok(selected)
}

/// Check if a diff line matches any of the given references
fn line_matches_refs(line: &DiffLine, refs: &[LineRef]) -> bool {
    for ref_item in refs {
        match (line, ref_item) {
            (DiffLine::Add { new_line, .. }, LineRef::Add(n)) => {
                if new_line == n {
                    return true;
                }
            }
            (DiffLine::Add { new_line, .. }, LineRef::AddRange(start, end)) => {
                if new_line >= start && new_line <= end {
                    return true;
                }
            }
            (DiffLine::Delete { old_line, .. }, LineRef::Delete(n)) => {
                if old_line == n {
                    return true;
                }
            }
            (DiffLine::Delete { old_line, .. }, LineRef::DeleteRange(start, end)) => {
                if old_line >= start && old_line <= end {
                    return true;
                }
            }
            _ => {}
        }
    }
    false
}

/// Group diff lines into contiguous hunks
fn group_into_hunks(lines: &[DiffLine]) -> Vec<Vec<DiffLine>> {
    if lines.is_empty() {
        return vec![];
    }

    // For simplicity, put each contiguous block into its own hunk
    // Lines are contiguous if their line numbers are adjacent
    let mut hunks = vec![];
    let mut current_hunk = vec![lines[0].clone()];

    for i in 1..lines.len() {
        let prev = &lines[i - 1];
        let curr = &lines[i];

        let is_contiguous = match (prev, curr) {
            (
                DiffLine::Add {
                    new_line: prev_line,
                    ..
                },
                DiffLine::Add {
                    new_line: curr_line,
                    ..
                },
            ) => *curr_line == prev_line + 1,
            (
                DiffLine::Delete {
                    old_line: prev_line,
                    ..
                },
                DiffLine::Delete {
                    old_line: curr_line,
                    ..
                },
            ) => *curr_line == prev_line + 1,
            _ => false,
        };

        if is_contiguous {
            current_hunk.push(curr.clone());
        } else {
            hunks.push(current_hunk);
            current_hunk = vec![curr.clone()];
        }
    }

    if !current_hunk.is_empty() {
        hunks.push(current_hunk);
    }

    hunks
}

/// Build the hunk header for a set of contiguous lines
fn build_hunk_header(lines: &[DiffLine]) -> String {
    if lines.is_empty() {
        return "@@ -0,0 +0,0 @@".to_string();
    }

    let mut old_start = 0u32;
    let mut old_count = 0u32;
    let mut new_start = 0u32;
    let mut new_count = 0u32;

    for line in lines {
        match line {
            DiffLine::Add { new_line, .. } => {
                if new_start == 0 {
                    new_start = *new_line;
                    // For additions, old_start is the line before
                    old_start = new_line - 1;
                }
                new_count += 1;
            }
            DiffLine::Delete { old_line, .. } => {
                if old_start == 0 {
                    old_start = *old_line;
                    // For deletions, new_start is old_start - 1 (where insertion would happen)
                    new_start = old_line - 1;
                }
                old_count += 1;
            }
        }
    }

    // Format: @@ -old_start,old_count +new_start,new_count @@
    // Special cases for counts of 0 or 1
    let old_part = if old_count == 0 {
        format!("-{},0", old_start)
    } else if old_count == 1 {
        format!("-{}", old_start)
    } else {
        format!("-{},{}", old_start, old_count)
    };

    let new_part = if new_count == 0 {
        format!("+{},0", new_start)
    } else if new_count == 1 {
        format!("+{}", new_start)
    } else {
        format!("+{},{}", new_start, new_count)
    };

    format!("@@ {} {} @@", old_part, new_part)
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

    // =========================================================================
    // Line reference parsing tests
    // =========================================================================

    #[test]
    fn parse_single_addition() {
        let result = parse_file_refs("flake.nix:137").unwrap();
        assert_eq!(result.file, "flake.nix");
        assert_eq!(result.refs, vec![LineRef::Add(137)]);
    }

    #[test]
    fn parse_addition_range() {
        let result = parse_file_refs("flake.nix:39..43").unwrap();
        assert_eq!(result.file, "flake.nix");
        assert_eq!(result.refs, vec![LineRef::AddRange(39, 43)]);
    }

    #[test]
    fn parse_multiple_additions() {
        let result = parse_file_refs("default.nix:40,41").unwrap();
        assert_eq!(result.file, "default.nix");
        assert_eq!(result.refs, vec![LineRef::Add(40), LineRef::Add(41)]);
    }

    #[test]
    fn parse_single_deletion() {
        let result = parse_file_refs("zsh.nix:-15").unwrap();
        assert_eq!(result.file, "zsh.nix");
        assert_eq!(result.refs, vec![LineRef::Delete(15)]);
    }

    #[test]
    fn parse_deletion_range() {
        let result = parse_file_refs("gtk.nix:-10..-11").unwrap();
        assert_eq!(result.file, "gtk.nix");
        assert_eq!(result.refs, vec![LineRef::DeleteRange(10, 11)]);
    }

    #[test]
    fn parse_mixed_refs() {
        let result = parse_file_refs("gtk.nix:-10,-11,12").unwrap();
        assert_eq!(result.file, "gtk.nix");
        assert_eq!(
            result.refs,
            vec![LineRef::Delete(10), LineRef::Delete(11), LineRef::Add(12)]
        );
    }

    #[test]
    fn parse_range_with_deletion() {
        let result = parse_file_refs("file.nix:10..15,-20").unwrap();
        assert_eq!(result.file, "file.nix");
        assert_eq!(
            result.refs,
            vec![LineRef::AddRange(10, 15), LineRef::Delete(20)]
        );
    }

    #[test]
    fn parse_invalid_format() {
        assert!(parse_file_refs("no_colon").is_err());
    }

    #[test]
    fn parse_empty_refs() {
        assert!(parse_file_refs("file.nix:").is_err());
    }

    #[test]
    fn parse_empty_file_name() {
        let result = parse_file_refs(":10");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("empty"));
    }

    #[test]
    fn parse_empty_file_with_range() {
        let result = parse_file_refs(":10..15");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("empty"));
    }

    #[test]
    fn parse_whitespace_file_name() {
        let result = parse_file_refs("  :10");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("empty"));
    }

    #[test]
    fn parse_just_colon() {
        let result = parse_file_refs(":");
        assert!(result.is_err());
    }

    // =========================================================================
    // Diff parsing tests
    // =========================================================================

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

    // =========================================================================
    // Patch construction tests
    // =========================================================================

    #[test]
    fn build_patch_single_addition() {
        let diff = FileDiff {
            file_path: "flake.nix".to_string(),
            lines: vec![DiffLine::Add {
                new_line: 137,
                content: "      debug = true;".to_string(),
            }],
        };
        let refs = vec![LineRef::Add(137)];

        let patch = build_patch("flake.nix", &diff, &refs).unwrap();

        // Patch should have proper headers and the single addition
        assert!(patch.contains("--- a/flake.nix"));
        assert!(patch.contains("+++ b/flake.nix"));
        assert!(patch.contains("@@ -136,0 +137 @@"));
        assert!(patch.contains("+      debug = true;"));
    }

    #[test]
    fn build_patch_contiguous_additions() {
        let diff = FileDiff {
            file_path: "flake.nix".to_string(),
            lines: vec![
                DiffLine::Add {
                    new_line: 39,
                    content: "".to_string(),
                },
                DiffLine::Add {
                    new_line: 40,
                    content: "    stylix = {".to_string(),
                },
                DiffLine::Add {
                    new_line: 41,
                    content: "      url = \"github:nix-community/stylix\";".to_string(),
                },
                DiffLine::Add {
                    new_line: 42,
                    content: "      inputs.nixpkgs.follows = \"nixpkgs\";".to_string(),
                },
                DiffLine::Add {
                    new_line: 43,
                    content: "    };".to_string(),
                },
            ],
        };
        let refs = vec![LineRef::AddRange(39, 43)];

        let patch = build_patch("flake.nix", &diff, &refs).unwrap();

        assert!(patch.contains("--- a/flake.nix"));
        assert!(patch.contains("+++ b/flake.nix"));
        assert!(patch.contains("@@ -38,0 +39,5 @@"));
        assert!(patch.contains("+"));
        assert!(patch.contains("+    stylix = {"));
        assert!(patch.contains("+    };"));
    }

    #[test]
    fn build_patch_partial_additions() {
        // Have 3 additions (lines 40, 41, 42), but only select 40 and 41
        let diff = FileDiff {
            file_path: "default.nix".to_string(),
            lines: vec![
                DiffLine::Add {
                    new_line: 40,
                    content: "        # Allow Stylix to override terminal font".to_string(),
                },
                DiffLine::Add {
                    new_line: 41,
                    content:
                        "        \"terminal.integrated.fontFamily\" = lib.mkDefault \"monospace\";"
                            .to_string(),
                },
                DiffLine::Add {
                    new_line: 42,
                    content: "        \"direnv.restart.automatic\" = true;".to_string(),
                },
            ],
        };
        let refs = vec![LineRef::AddRange(40, 41)];

        let patch = build_patch("default.nix", &diff, &refs).unwrap();

        // Should only contain lines 40 and 41, not 42
        assert!(patch.contains("@@ -39,0 +40,2 @@"));
        assert!(patch.contains("+        # Allow Stylix to override terminal font"));
        assert!(patch.contains("+        \"terminal.integrated.fontFamily\""));
        assert!(!patch.contains("direnv.restart.automatic"));
    }

    #[test]
    fn build_patch_single_deletion() {
        let diff = FileDiff {
            file_path: "zsh.nix".to_string(),
            lines: vec![DiffLine::Delete {
                old_line: 15,
                content: "      enableAutosuggestions = true;".to_string(),
            }],
        };
        let refs = vec![LineRef::Delete(15)];

        let patch = build_patch("zsh.nix", &diff, &refs).unwrap();

        assert!(patch.contains("--- a/zsh.nix"));
        assert!(patch.contains("+++ b/zsh.nix"));
        assert!(patch.contains("@@ -15 +14,0 @@"));
        assert!(patch.contains("-      enableAutosuggestions = true;"));
    }

    #[test]
    fn build_patch_selective_from_mixed() {
        // Mixed add/delete, but only select one addition
        let diff = FileDiff {
            file_path: "gtk.nix".to_string(),
            lines: vec![
                DiffLine::Delete {
                    old_line: 10,
                    content: "    gtk.theme.name = \"Adwaita\";".to_string(),
                },
                DiffLine::Delete {
                    old_line: 11,
                    content: "    gtk.iconTheme.name = \"Papirus\";".to_string(),
                },
                DiffLine::Add {
                    new_line: 10,
                    content: "    # Theme managed by Stylix".to_string(),
                },
                DiffLine::Add {
                    new_line: 11,
                    content: "    gtk.iconTheme.name = \"Papirus-Dark\";".to_string(),
                },
                DiffLine::Add {
                    new_line: 12,
                    content: "    gtk.cursorTheme.size = 24;".to_string(),
                },
            ],
        };
        // Only select the cursor theme addition (line 12)
        let refs = vec![LineRef::Add(12)];

        let patch = build_patch("gtk.nix", &diff, &refs).unwrap();

        // Should only have the cursor size addition
        assert!(patch.contains("@@ -11,0 +12 @@"));
        assert!(patch.contains("+    gtk.cursorTheme.size = 24;"));
        // Should NOT have deletions or other additions
        assert!(!patch.contains("-    gtk.theme.name"));
        assert!(!patch.contains("-    gtk.iconTheme.name = \"Papirus\""));
        assert!(!patch.contains("+    # Theme managed by Stylix"));
        assert!(!patch.contains("+    gtk.iconTheme.name = \"Papirus-Dark\""));
    }

    #[test]
    fn build_patch_no_matching_lines() {
        let diff = FileDiff {
            file_path: "file.nix".to_string(),
            lines: vec![DiffLine::Add {
                new_line: 10,
                content: "something".to_string(),
            }],
        };
        let refs = vec![LineRef::Add(99)]; // Line 99 doesn't exist

        let result = build_patch("file.nix", &diff, &refs);
        assert!(result.is_err());
    }

    // =========================================================================
    // Diff formatting tests
    // =========================================================================

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
