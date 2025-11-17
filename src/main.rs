use clap::{Parser, Subcommand};
use git_stager::GitStager;

#[derive(Parser)]
#[command(name = "git-stager")]
#[command(version)]
#[command(about = "Non-interactive line-level git staging tool")]
#[command(long_about = concat!(
    "Non-interactive line-level git staging tool\n\n",
    "Stage specific lines from git diffs when hunks are too coarse.\n",
    "Use 'git-stager diff' to see line numbers, then 'git-stager stage' to select lines.\n\n",
    "Repository: ", env!("CARGO_PKG_REPOSITORY")
))]
struct Cli {
    /// Run as if git-stager was started in <path> instead of the current working directory
    #[arg(short = 'C', global = true)]
    path: Option<String>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Stage specific lines from unstaged changes
    ///
    /// Select individual changed lines to stage, even from within contiguous
    /// changes. Line numbers come from `git-stager diff` output.
    ///
    /// Syntax: FILE:REFS
    ///   N         stage addition at new line N
    ///   -N        stage deletion of old line N
    ///   N..M      stage range of additions
    ///   -N..-M    stage range of deletions
    ///   A,B,C     combine any of the above
    ///
    /// Basic:
    ///   file:137           single added line
    ///   file:-15           single deleted line
    ///   file:10..15        range of additions
    ///
    /// Advanced - skip lines within contiguous changes:
    ///   file:40..45,48     lines 40-45 and 48, skip 46-47
    ///   file:10,15,20      only specific lines, not 11-14 or 16-19
    ///   file:-10..-12,-15  delete 10-12 and 15, skip 13-14
    ///
    /// Multiple files:
    ///   a.nix:10 b.nix:20  stage from multiple files
    #[command(verbatim_doc_comment)]
    Stage {
        /// One or more FILE:REFS specifications
        file_refs: Vec<String>,
    },
    /// Show unstaged changes with line numbers for staging
    ///
    /// Output format:
    ///   +N:  added line (stage with N)
    ///   -N:  deleted line (stage with -N)
    ///
    /// Example output:
    ///   config.nix:
    ///     -10:    old_setting = true;
    ///     +10:    new_setting = false;
    ///     +11:    extra_setting = true;
    ///
    /// To stage only the replacement (skip +11):
    ///   git-stager stage config.nix:-10,10
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
