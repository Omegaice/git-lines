//! Git diff parsing and manipulation.
//!
//! This module provides types for parsing git diff output and filtering it
//! to select specific lines for staging.
//!
//! # Structure
//!
//! A git diff is organized hierarchically:
//!
//! - [`Diff`] (full) - Contains multiple files
//!   - [`FileDiff`](file::FileDiff) - One file's changes
//!     - [`Hunk`](hunk::Hunk) - One contiguous block of changes
//!       - [`ModifiedLines`](hunk::ModifiedLines) - Additions or deletions
//!
//! # Workflow
//!
//! 1. Parse raw `git diff` output into [`Diff`]
//! 2. Filter with `.retain()` to keep only selected lines
//! 3. Render back to patch format with `.to_string()`
//! 4. Apply with `git apply --cached`
//!
//! # Example
//!
//! ```
//! use git_stager::diff::Diff;
//!
//! let raw_diff = r#"diff --git a/file.txt b/file.txt
//! --- a/file.txt
//! +++ b/file.txt
//! @@ -10,0 +11,2 @@
//! +new line 11
//! +new line 12
//! "#;
//!
//! let diff = Diff::parse(raw_diff);
//! let filtered = diff.retain(|_, _| false, |_, line| line == 11);
//! // filtered now contains only line 11
//! ```

pub mod file;
pub mod full;
pub mod hunk;

pub use full::Diff;

/// Format a git diff for user display with explicit line numbers.
///
/// Converts a parsed diff into a human-readable format showing line numbers
/// for each addition and deletion, suitable for `git-stager diff` output.
///
/// # Format
///
/// ```text
/// file.nix:
///   -10:    deleted line
///   +10:    added line
///   +11:    another addition
/// ```
///
/// The numbers indicate:
/// - `-N` - Old line number (for deletions)
/// - `+N` - New line number (for additions)
pub fn format_diff(diff: &Diff) -> String {
    let mut result = String::new();

    for file_diff in &diff.files {
        result.push_str(&file_diff.path);
        result.push_str(":\n");

        for hunk in &file_diff.hunks {
            // Show deletions
            for (i, line) in hunk.old.lines.iter().enumerate() {
                let line_num = hunk.old.start + i as u32;
                result.push_str(&format!("  -{}:\t{}\n", line_num, line));
            }

            // Show additions
            for (i, line) in hunk.new.lines.iter().enumerate() {
                let line_num = hunk.new.start + i as u32;
                result.push_str(&format!("  +{}:\t{}\n", line_num, line));
            }

            result.push('\n');
        }
    }

    // Remove trailing newline if present
    if result.ends_with("\n\n") {
        result.pop();
    }

    result
}
