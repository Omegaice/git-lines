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
//! 2. Filter with `.filter()` to keep only selected lines
//! 3. Render back to patch format with `.to_patch()`
//! 4. Apply with `git apply --cached`
//!
//! # Example
//!
//! ```
//! use git_lines::diff::Diff;
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
//! let filtered = diff.filter(|_, _| false, |_, line| line == 11);
//! // filtered now contains only line 11
//!
//! // Human-readable display
//! println!("{}", filtered);
//!
//! // Patch format for git apply
//! let patch = filtered.to_patch();
//! ```

pub mod file;
pub mod full;
pub mod hunk;

pub use full::Diff;
