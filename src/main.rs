use clap::{Parser, Subcommand};
use git_stager::{GitStager, format_diff_output};

#[derive(Parser)]
#[command(name = "git-stager")]
#[command(about = "Non-interactive line-level git staging tool")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Stage specific lines (additions or deletions) from unstaged changes
    ///
    /// Syntax: FILE:REFS where REFS uses:
    ///   N       addition (new line number)
    ///   N..M    range of additions
    ///   -N      deletion (old line number)
    ///   ,       separator for multiple refs
    #[command(verbatim_doc_comment)]
    Stage {
        /// Examples: flake.nix:137  config.nix:10..15  zsh.nix:-15  gtk.nix:12,-10
        file_refs: Vec<String>,
    },
    /// Show diff with zero context (use to find line numbers for staging)
    ///
    /// Displays git diff output in the format that git-stager uses internally.
    /// Line numbers shown in the hunk headers (e.g., @@ -10 +11 @@) are the
    /// exact numbers to use with the 'stage' command.
    Diff {
        /// Files to show diff for (defaults to all changed files)
        files: Vec<String>,
    },
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let stager = GitStager::new(".");

    match cli.command {
        Commands::Stage { file_refs } => {
            for file_ref in &file_refs {
                stager
                    .stage(file_ref)
                    .map_err(|e| format!("Failed to stage '{}': {}", file_ref, e))?;
            }
        }
        Commands::Diff { files } => {
            let raw_diff = run_git_diff(&files)?;
            let formatted = format_diff_output(&raw_diff)
                .map_err(|e| format!("Failed to format diff: {}", e))?;
            print!("{}", formatted);
        }
    }

    Ok(())
}

/// Run git diff with zero context lines
fn run_git_diff(files: &[String]) -> Result<String, Box<dyn std::error::Error>> {
    let mut args = vec!["diff", "--no-ext-diff", "-U0", "--no-color"];

    // Add file arguments
    let file_refs: Vec<&str> = files.iter().map(|s| s.as_str()).collect();
    args.extend(file_refs);

    let output = std::process::Command::new("git")
        .args(&args)
        .output()
        .map_err(|e| format!("Failed to run git diff: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("git diff failed: {}", stderr).into());
    }

    Ok(String::from_utf8(output.stdout)
        .map_err(|e| format!("Invalid UTF-8 in git diff output: {}", e))?)
}
