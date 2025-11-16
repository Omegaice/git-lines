use std::path::PathBuf;
use std::process::Command;

mod diff;
mod parse;
mod patch;

pub use diff::format_diff_output;

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
        let parsed = parse::parse_file_refs(file_ref)?;
        self.stage_lines(&parsed)
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
        diff::format_diff_output(&self.get_raw_diff(files)?)
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

        args.extend(files.iter().map(|s| s.as_str()));

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

    /// Stage specific lines from a file
    fn stage_lines(&self, file_refs: &parse::FileLineRefs) -> Result<(), String> {
        let diff_output = self.get_git_diff_for_file(&file_refs.file)?;

        if diff_output.trim().is_empty() {
            return Err(format!("No changes found in {}", file_refs.file));
        }

        self.apply_patch(&patch::build_patch(
            &file_refs.file,
            &diff::parse_diff(&diff_output)?.lines,
            &file_refs.refs,
        )?)?;

        Ok(())
    }

    /// Get git diff output for a specific file
    fn get_git_diff_for_file(&self, file: &str) -> Result<String, String> {
        let output = Command::new("git")
            .args([
                "-C",
                self.repo_path.to_str().unwrap_or("."),
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

        String::from_utf8(output.stdout)
            .map_err(|e| format!("Invalid UTF-8 in git diff output: {}", e))
    }

    /// Apply a patch to the git index
    fn apply_patch(&self, patch: &str) -> Result<(), String> {
        let mut child = Command::new("git")
            .args([
                "-C",
                self.repo_path.to_str().unwrap_or("."),
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
}
