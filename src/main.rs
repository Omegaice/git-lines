use clap::{Parser, Subcommand};
use git_stager::GitStager;

#[derive(Parser)]
#[command(name = "git-stager")]
#[command(about = "Non-interactive line-level git staging tool")]
struct Cli {
    /// Run as if git-stager was started in <path> instead of the current working directory
    #[arg(short = 'C', global = true)]
    path: Option<String>,

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
    ///   -N..-M  range of deletions
    ///   ,       separator for multiple refs
    #[command(verbatim_doc_comment)]
    Stage {
        /// Examples: flake.nix:137  config.nix:10..15  zsh.nix:-15..-17  gtk.nix:12,-10
        file_refs: Vec<String>,
    },
    /// Show unstaged changes with line numbers for staging
    ///
    /// Example: -10: old line
    ///          +10: new line
    ///          Stage both with: file.nix:-10,10
    #[command(verbatim_doc_comment)]
    Diff {
        /// Files to show diff for (defaults to all changed files)
        files: Vec<String>,
    },
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let repo_path = cli.path.as_deref().unwrap_or(".");
    let stager = GitStager::new(repo_path);

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
