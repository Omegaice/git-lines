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
    /// Stage specific lines by reference (e.g., file.nix:10..15,-20)
    Stage {
        /// File and line references (e.g., "flake.nix:137" or "flake.nix:10..15")
        file_refs: Vec<String>,
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
    }

    Ok(())
}
