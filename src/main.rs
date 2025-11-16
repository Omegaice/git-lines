use clap::{Parser, Subcommand};
use git_stager::GitStager;

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
            let output = stager
                .diff(&files)
                .map_err(|e| format!("Failed to get diff: {}", e))?;
            print!("{}", output);
        }
    }

    Ok(())
}
