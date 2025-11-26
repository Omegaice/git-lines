use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::{Shell, generate};
use clap_mangen::Man;
use git_lines::GitLines;
use std::io;

#[derive(Parser)]
#[command(name = "git-lines")]
#[command(version)]
#[command(about = "Non-interactive line-level git staging tool")]
#[command(long_about = concat!(
    "Non-interactive line-level git staging tool\n\n",
    "Stage specific lines from git diffs when hunks are too coarse.\n",
    "Use 'git lines diff' to see line numbers, then 'git lines stage' to select lines.\n\n",
    "Repository: ", env!("CARGO_PKG_REPOSITORY")
))]
struct Cli {
    /// Run as if git-lines was started in <path> instead of the current working directory
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
    /// changes. Line numbers come from `git lines diff` output.
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

        /// Suppress output showing what was staged
        #[arg(short, long)]
        quiet: bool,
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
    ///   git lines stage config.nix:-10,10
    #[command(verbatim_doc_comment)]
    Diff {
        /// Files to show diff for (defaults to all changed files)
        files: Vec<String>,
    },
    /// Generate shell completion scripts
    ///
    /// Install completions for your shell:
    ///
    /// Bash:
    ///   git-lines completions bash > ~/.local/share/bash-completion/completions/git-lines
    ///
    /// Zsh:
    ///   git-lines completions zsh > ~/.zfunc/_git-lines
    ///   (and add ~/.zfunc to your $fpath in .zshrc)
    ///
    /// Fish:
    ///   git-lines completions fish > ~/.config/fish/completions/git-lines.fish
    ///
    /// PowerShell:
    ///   git-lines completions powershell > git-lines.ps1
    #[command(verbatim_doc_comment)]
    Completions {
        /// The shell to generate completions for
        shell: Shell,
    },
    /// Generate man page
    ///
    /// Install the man page:
    ///
    ///   git-lines man > git-lines.1
    ///   sudo mv git-lines.1 /usr/local/share/man/man1/
    ///   sudo mandb
    ///
    /// Then view with:
    ///   man git-lines
    #[command(verbatim_doc_comment)]
    Man,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Completions { shell } => {
            let mut cmd = Cli::command();
            generate(shell, &mut cmd, "git-lines", &mut io::stdout());
        }
        Commands::Man => {
            let cmd = Cli::command();
            let man = Man::new(cmd);
            man.render(&mut io::stdout())?;
        }
        Commands::Stage { file_refs, quiet } => {
            let repo_path = cli.path.as_deref().unwrap_or(".");
            let stager = GitLines::new(repo_path);
            for file_ref in &file_refs {
                let staged = stager
                    .stage(file_ref)
                    .map_err(|e| format!("Failed to stage '{}': {}", file_ref, e))?;
                if !quiet {
                    print!("Staged:\n{}", staged);
                }
            }
        }
        Commands::Diff { files } => {
            let repo_path = cli.path.as_deref().unwrap_or(".");
            let stager = GitLines::new(repo_path);
            let output = stager
                .diff(&files)
                .map_err(|e| format!("Failed to get diff: {}", e))?;
            print!("{}", output);
        }
    }

    Ok(())
}
