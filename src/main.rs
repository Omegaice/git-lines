use clap::{Parser, Subcommand};

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

    match cli.command {
        Commands::Stage { file_refs } => {
            eprintln!("Staging: {:?}", file_refs);
            eprintln!("(Not yet implemented)");
        }
    }

    Ok(())
}
