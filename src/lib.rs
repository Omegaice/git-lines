#![allow(missing_docs)]

use error_set::error_set;
use std::process::Command;

mod diff;
mod parse;

pub use parse::ParseError;

error_set! {
    /// Top-level error for git-stager operations
    GitStagerError := {
        #[display("No changes found in {file}")]
        NoChanges { file: String },
        #[display("No matching lines found for {file}")]
        NoMatchingLines { file: String },
        ParseError(ParseError),
    } || GitCommandError

    /// Errors from git command execution
    GitCommandError := {
        #[display("Failed to run git diff: {message}")]
        DiffFailed { message: String },
        #[display("git diff failed: {stderr}")]
        DiffExitError { stderr: String },
        #[display("Invalid UTF-8 in git diff output: {message}")]
        InvalidUtf8 { message: String },
        #[display("Failed to spawn git apply: {message}")]
        ApplySpawnFailed { message: String },
        #[display("Failed to get stdin handle for git apply")]
        ApplyStdinFailed,
        #[display("Failed to write patch to git apply: {message}")]
        ApplyWriteFailed { message: String },
        #[display("Failed to wait for git apply: {message}")]
        ApplyWaitFailed { message: String },
        #[display("git apply failed: {stderr}")]
        ApplyExitError { stderr: String },
    }
}

/// Main interface for git-stager operations
pub struct GitStager<'a> {
    repo_path: &'a str,
}

impl<'a> GitStager<'a> {
    /// Create a new GitStager for the given repository path
    pub fn new(repo_path: &'a str) -> Self {
        Self { repo_path }
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
        let mut args = vec![
            "-C",
            self.repo_path,
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
                    parse::LineRef::Delete(n) => *n == old_line,
                    parse::LineRef::DeleteRange(start, end) => {
                        old_line >= *start && old_line <= *end
                    }
                    parse::LineRef::Add(_) | parse::LineRef::AddRange(_, _) => false,
                })
            },
            |_path, new_line| {
                file_refs.refs.iter().any(|r| match r {
                    parse::LineRef::Add(n) => *n == new_line,
                    parse::LineRef::AddRange(start, end) => new_line >= *start && new_line <= *end,
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

        let mut child = Command::new("git")
            .args([
                "-C",
                self.repo_path,
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
