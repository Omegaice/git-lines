use clap::{Parser, Subcommand};
use git2::Repository;
use git_stager::list_hunks;

#[derive(Parser)]
#[command(name = "git-stager")]
#[command(about = "Non-interactive git hunk staging tool")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// List all unstaged hunks
    List {
        /// Output in JSON format
        #[arg(long)]
        json: bool,
    },
    /// Stage specific hunks by ID
    Stage {
        /// Hunk IDs to stage
        hunk_ids: Vec<String>,
    },
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    let repo = Repository::discover(".")?;
    eprintln!("Found repository at: {:?}", repo.path());

    match cli.command {
        Commands::List { json } => {
            let hunks = list_hunks(&repo)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&hunks)?);
            } else {
                for hunk in &hunks {
                    println!(
                        "[{}] {} lines {}-{} (+{}/-{})",
                        hunk.id,
                        hunk.file,
                        hunk.old_start,
                        hunk.old_start + hunk.lines_removed,
                        hunk.lines_added,
                        hunk.lines_removed
                    );
                }
                if hunks.is_empty() {
                    println!("No unstaged hunks found.");
                }
            }
        }
        Commands::Stage { hunk_ids } => {
            eprintln!("Staging hunks: {:?}", hunk_ids);
            eprintln!("(Not yet implemented)");
        }
    }

    Ok(())
}
