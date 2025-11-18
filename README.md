# git-stager

Non-interactive line-level git staging tool for LLMs and automation.

## Overview

`git-stager` enables programmatic, line-level staging of git changes. It fills the gap left by `git add -p`, which requires interactive input and cannot be used by LLMs or automation tools.

When an LLM or automated system makes changes to code, those changes are often semantically distinct but physically interleaved in the same file. `git-stager` allows these systems to autonomously organize changes into clean, semantic commits without human intervention.

## The Problem

Git's interactive staging (`git add -p`) operates at the hunk level and requires a human at a terminal. This creates a limitation for LLMs:

- ✅ LLMs can write code
- ✅ LLMs can run `git commit`
- ❌ LLMs cannot use `git add -p` (interactive/TUI)

Result: LLMs can only stage entire files with `git add <file>`, losing the ability to create focused, semantic commits when multiple unrelated changes exist in the same file.

## The Solution

`git-stager` provides a non-interactive CLI for line-level staging:

```bash
# 1. View changes with line numbers
git-stager diff

# 2. Stage specific lines by number
git-stager stage flake.nix:137,142
git-stager stage config.nix:10..15
git-stager stage zsh.nix:-20,-21

# 3. Commit as usual
git commit -m "Add new dependencies"
```

This workflow is fully scriptable and requires no human interaction.

## Example: Semantic Commits from Mixed Changes

Consider these changes in one file:

```bash
$ git-stager diff vscode/default.nix
vscode/default.nix:
  +40:        # Allow Stylix to override terminal font
  +41:        "terminal.integrated.fontFamily" = lib.mkDefault "monospace";
  +42:        "direnv.restart.automatic" = true;
```

Git sees this as one atomic hunk. But semantically it's two features:
- Lines 40-41: Theme configuration (related to Stylix)
- Line 42: Direnv settings (unrelated)

With `git-stager`, an LLM can create two focused commits:

```bash
# Commit 1: Theme changes
git-stager stage vscode/default.nix:40..41
git commit -m "feat: add Stylix font override for terminal"

# Commit 2: Direnv changes
git-stager stage vscode/default.nix:42
git commit -m "feat: enable automatic direnv restart"
```

Each commit is self-contained and semantically coherent.

## Installation

### From crates.io

```bash
cargo install git-stager
```

### From source

```bash
git clone https://github.com/Omegaice/git-stager
cd git-stager
cargo install --path .
```

### Shell Completions

```bash
# Bash
git-stager completions bash > ~/.local/share/bash-completion/completions/git-stager

# Zsh
git-stager completions zsh > ~/.zfunc/_git-stager

# Fish
git-stager completions fish > ~/.config/fish/completions/git-stager.fish
```

## Usage

### Basic Workflow

```bash
# 1. Make changes to files (manually or via LLM)
# 2. View unstaged changes with line numbers
git-stager diff

# 3. Stage specific lines
git-stager stage file.nix:10,15,20

# 4. Create commit
git commit -m "Your commit message"
```

### Line Reference Syntax

```bash
# Single addition (new line 137)
git-stager stage flake.nix:137

# Range of additions (lines 10-15 inclusive)
git-stager stage config.nix:10..15

# Single deletion (old line 20)
git-stager stage zsh.nix:-20

# Range of deletions
git-stager stage file.nix:-10..-15

# Multiple selections (comma-separated)
git-stager stage config.nix:10,15,20

# Mixed operations
git-stager stage gtk.nix:-10,-11,12

# Multiple files in one command
git-stager stage flake.nix:137 gtk.nix:12 zsh.nix:-15
```

### Advanced Examples

**Splitting changes within a single hunk:**

```bash
$ git-stager diff config.nix
config.nix:
  +10:    feature_a_enabled = true;
  +11:    feature_a_timeout = 30;
  +12:    feature_b_enabled = true;

# Stage only feature A
$ git-stager stage config.nix:10,11
$ git commit -m "Enable feature A with 30s timeout"

# Later, stage feature B
$ git-stager stage config.nix:12
$ git commit -m "Enable feature B"
```

**Staging from multiple non-contiguous hunks:**

```bash
$ git-stager diff flake.nix
flake.nix:
  +7:       determinate.url = "github:DeterminateSystems/determinate";

  +137:       debug = true;

  +142:         ./flake-modules/home-manager.nix

# Stage lines from different hunks that are semantically related
$ git-stager stage flake.nix:7,142
$ git commit -m "Add determinate and home-manager modules"
```

**Selective staging from mixed additions and deletions:**

```bash
$ git-stager diff gtk.nix
gtk.nix:
  -10:    gtk.theme.name = "Adwaita";
  -11:    gtk.iconTheme.name = "Papirus";
  +10:    # Theme managed by Stylix
  +11:    gtk.iconTheme.name = "Papirus-Dark";
  +12:    gtk.cursorTheme.size = 24;

# Stage only the cursor size addition, ignore theme changes
$ git-stager stage gtk.nix:12
$ git commit -m "Set cursor size to 24"
```

## When to Use

### Use `git-stager` when:
- Multiple unrelated changes exist in the same file
- You need programmatic/scriptable staging (LLM workflows)
- Changes need to be organized into semantic commits
- `git add -p` hunks are too coarse

### Use regular `git add` when:
- Staging entire files
- All changes in a file are semantically related
- Changes are already separated by file boundaries

**Philosophy**: `git-stager` is a companion to git, not a replacement. Use it only when line-level precision is needed.

## How It Works

1. **Parse references**: `file.nix:10,15,-20` → structured line selections
2. **Fetch git diff**: Run `git diff -U0` to get changes with zero context
3. **Filter lines**: Extract only the requested lines from the diff
4. **Apply patch**: Feed the filtered patch to `git apply --cached`

Line numbers are always based on the output of `git-stager diff`, which shows the current state of unstaged changes.

## Use Cases

### LLM Coding Assistants
- Claude Code (Anthropic)
- GitHub Copilot
- Cursor
- Aider
- Custom LLM automation

LLMs can now create clean commit history autonomously without human intervention for staging.

### Human Workflows
- Code review: Stage reviewer suggestions one at a time
- Incremental refactoring: Separate style changes from logic changes
- Bug fixes: Commit discovered bugs separately from feature work
- Any scenario requiring scriptable line-level staging

## Design Principles

- **Non-interactive by design**: No prompts, no TUI, pure CLI
- **Minimal dependencies**: Only clap and error_set at runtime
- **Git-native**: Uses `git diff` and `git apply`, not libgit2
- **Line-level precision**: Finer granularity than hunks
- **Automation-first**: Built for programmatic use

## Limitations

- Requires `git` in PATH (uses CLI git commands)
- Works only on unstaged changes
- Line numbers are from `git diff` output (shift after partial staging)
- Does not handle interactive rebase or patch editing

## License

MIT

## Contributing

Issues and pull requests welcome at [github.com/Omegaice/git-stager](https://github.com/Omegaice/git-stager)

## Documentation

Full API documentation: [docs.rs/git-stager](https://docs.rs/git-stager)

---

Built to solve the line-level staging gap in LLM and automation workflows.
