//! Line-level git staging tool for fine-grained commit control.
//!
//! `git-stager` enables staging individual changed lines when git's hunks are too coarse.
//! This fills the gap left by `git add -p` which requires interactive input and cannot be
//! used by automation or LLMs.
//!
//! # Overview
//!
//! Git's interactive staging (`git add -p`) works at the hunk level - contiguous blocks of
//! changes. When multiple unrelated changes exist in the same hunk, you cannot separate them
//! for different commits. This tool allows line-level precision.
//!
//! # Workflow
//!
//! 1. Use [`GitStager::diff`] to see line numbers for all unstaged changes
//! 2. Use [`GitStager::stage`] to select specific lines by number
//! 3. Create commits with `git commit` as usual
//!
//! # Examples
//!
//! ```no_run
//! use git_stager::GitStager;
//!
//! let stager = GitStager::new(".");
//!
//! // View changes with line numbers
//! let diff = stager.diff(&[]).unwrap();
//! println!("{}", diff);
//!
//! // Stage specific lines
//! stager.stage("flake.nix:137").unwrap();           // Single addition
//! stager.stage("file.nix:10..15").unwrap();         // Range of additions
//! stager.stage("zsh.nix:-15").unwrap();             // Single deletion
//! stager.stage("config.nix:-10,-11,12").unwrap();   // Mixed operations
//! ```
//!
//! # Line Reference Syntax
//!
//! - `N` - Stage addition at new line N
//! - `-N` - Stage deletion of old line N
//! - `N..M` - Stage range of additions (inclusive)
//! - `-N..-M` - Stage range of deletions (inclusive)
//! - `A,B,C` - Combine multiple line references
//!
//! # Architecture
//!
//! The crate is organized into focused modules:
//!
//! - [`parse`] - Parse `file:refs` syntax into structured line references
//! - [`diff`] - Parse and manipulate git diff output
//! - [`GitStager`] - Main API for staging operations
//!
//! # Use Cases
//!
//! - **Semantic commits**: Group related changes scattered across a file
//! - **Incremental refactoring**: Stage bug fixes separately from style changes
//! - **LLM workflows**: Enable automated staging based on change semantics
//! - **Code review**: Stage reviewer suggestions line-by-line

use error_set::error_set;
use std::path::{Path, PathBuf};
use std::process::Command;

pub mod diff;
pub mod parse;

pub use parse::ParseError;

error_set! {
    /// Top-level error for git-stager operations
    GitStagerError := {
        /// No unstaged changes found in the specified file
        #[display("No changes found in {file}")]
        NoChanges { file: String },
        /// No lines matched the specified line references
        #[display("No matching lines found for {file}")]
        NoMatchingLines { file: String },
        /// Error parsing the file:refs syntax
        ParseError(ParseError),
    } || GitCommandError

    /// Errors from git command execution
    GitCommandError := {
        /// Failed to execute the git diff command
        #[display("Failed to run git diff: {message}")]
        DiffFailed { message: String },
        /// Git diff command exited with non-zero status
        #[display("git diff failed: {stderr}")]
        DiffExitError { stderr: String },
        /// Git diff output contained invalid UTF-8
        #[display("Invalid UTF-8 in git diff output: {message}")]
        InvalidUtf8 { message: String },
        /// Failed to spawn the git apply process
        #[display("Failed to spawn git apply: {message}")]
        ApplySpawnFailed { message: String },
        /// Failed to obtain stdin handle for git apply
        #[display("Failed to get stdin handle for git apply")]
        ApplyStdinFailed,
        /// Failed to write patch data to git apply stdin
        #[display("Failed to write patch to git apply: {message}")]
        ApplyWriteFailed { message: String },
        /// Failed to wait for git apply to complete
        #[display("Failed to wait for git apply: {message}")]
        ApplyWaitFailed { message: String },
        /// Git apply command exited with non-zero status
        #[display("git apply failed: {stderr}")]
        ApplyExitError { stderr: String },
    }
}

/// Main interface for git-stager operations
pub struct GitStager {
    repo_path: PathBuf,
}

impl GitStager {
    /// Create a new GitStager for the given repository path
    pub fn new(repo_path: impl AsRef<Path>) -> Self {
        Self {
            repo_path: repo_path.as_ref().to_path_buf(),
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
    pub fn stage(&self, file_ref: &str) -> Result<(), GitStagerError> {
        self.stage_lines(&parse::parse_file_refs(file_ref)?)
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
    pub fn diff(&self, files: &[String]) -> Result<String, GitStagerError> {
        let raw_diff = self.get_raw_diff(files)?;
        let parsed = diff::Diff::parse(&raw_diff);
        Ok(diff::format_diff(&parsed))
    }

    /// Get raw git diff output with zero context lines
    fn get_raw_diff(&self, files: &[String]) -> Result<String, GitCommandError> {
        let repo_path_str = self
            .repo_path
            .to_str()
            .expect("repo path should be valid UTF-8");
        let mut args = vec![
            "-C",
            repo_path_str,
            "diff",
            "--no-ext-diff",
            "-U0",
            "--no-color",
        ];

        args.extend(files.iter().map(|s| s.as_str()));

        let output =
            Command::new("git")
                .args(&args)
                .output()
                .map_err(|e| GitCommandError::DiffFailed {
                    message: e.to_string(),
                })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(GitCommandError::DiffExitError {
                stderr: stderr.into_owned(),
            });
        }

        String::from_utf8(output.stdout).map_err(|e| GitCommandError::InvalidUtf8 {
            message: e.to_string(),
        })
    }

    /// Stage specific lines from a file
    fn stage_lines(&self, file_refs: &parse::FileLineRefs) -> Result<(), GitStagerError> {
        let diff_output = self.get_raw_diff(std::slice::from_ref(&file_refs.file))?;

        if diff_output.trim().is_empty() {
            return Err(GitStagerError::NoChanges {
                file: file_refs.file.clone(),
            });
        }

        let full_diff = diff::Diff::parse(&diff_output);
        let filtered = full_diff.retain(
            |_path, old_line| {
                file_refs.refs.iter().any(|r| match r {
                    parse::LineRef::Delete(n) => n.get() == old_line,
                    parse::LineRef::DeleteRange(start, end) => {
                        old_line >= start.get() && old_line <= end.get()
                    }
                    parse::LineRef::Add(_) | parse::LineRef::AddRange(_, _) => false,
                })
            },
            |_path, new_line| {
                file_refs.refs.iter().any(|r| match r {
                    parse::LineRef::Add(n) => n.get() == new_line,
                    parse::LineRef::AddRange(start, end) => {
                        new_line >= start.get() && new_line <= end.get()
                    }
                    parse::LineRef::Delete(_) | parse::LineRef::DeleteRange(_, _) => false,
                })
            },
        );

        if filtered.files.is_empty() {
            return Err(GitStagerError::NoMatchingLines {
                file: file_refs.file.clone(),
            });
        }

        Ok(self.apply_patch(&filtered.to_string())?)
    }

    /// Apply a patch to the git index
    fn apply_patch(&self, patch: &str) -> Result<(), GitCommandError> {
        use std::io::Write;

        let repo_path_str = self
            .repo_path
            .to_str()
            .expect("repo path should be valid UTF-8");
        let mut child = Command::new("git")
            .args([
                "-C",
                repo_path_str,
                "apply",
                "--cached",
                "--unidiff-zero",
                "-",
            ])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| GitCommandError::ApplySpawnFailed {
                message: e.to_string(),
            })?;

        child
            .stdin
            .take()
            .ok_or(GitCommandError::ApplyStdinFailed)?
            .write_all(patch.as_bytes())
            .map_err(|e| GitCommandError::ApplyWriteFailed {
                message: e.to_string(),
            })?;

        let output = child
            .wait_with_output()
            .map_err(|e| GitCommandError::ApplyWaitFailed {
                message: e.to_string(),
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(GitCommandError::ApplyExitError {
                stderr: stderr.into_owned(),
            });
        }

        Ok(())
    }
}
