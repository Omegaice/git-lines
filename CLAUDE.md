# CLAUDE.md

## Tool Purpose

git-stager enables line-level staging when git's hunks are too coarse. It fills the gap left by `git add -p` which Claude cannot use interactively.

**Critical context**: This is a git companion tool, not a replacement. Claude uses git directly for everything else (staging whole files, committing, etc.). Only reach for git-stager when multiple unrelated changes exist in the same file and need separate commits.

Typical workflow:
1. `git-stager diff` to see available line numbers
2. `git-stager stage file:N` to stage specific lines
3. `git commit` as normal

## Architecture

### Module Responsibilities
- **parse.rs** - Input syntax parsing (`file:refs` format). Owns `ParseError`.
- **diff.rs** - Git diff parsing and formatting. Owns `DiffError`.
- **patch.rs** - Patch construction from selected lines. Owns `PatchError`.
- **lib.rs** - `GitStager` orchestration and git command execution. Owns `GitCommandError`, composes `GitStagerError`.
- **main.rs** - Thin CLI wrapper. Argument parsing and output display only.

### Error Handling Pattern
Each module defines its own error type using `error_set!`. The top-level `GitStagerError` composes them via tuple variants:
```rust
GitStagerError := {
    NoChanges { file: String },
    ParseError(ParseError),
    DiffError(DiffError),
    PatchError(PatchError),
} || GitCommandError
```
This keeps modules self-contained while allowing automatic error conversion with `?`.

### Dependency Philosophy
- **Minimal runtime dependencies** - Only clap and error_set
- **git2 in dev-dependencies only** - Used for e2e test fixtures, not production code
- **Use CLI git commands** - `git diff` and `git apply --cached` instead of libgit2. The `git apply` operation has no good libgit2 equivalent and CLI is battle-tested.

## Development Tooling

- **Formatting**: Use `nix fmt`, NOT `cargo fmt` (treefmt-nix configured)
- **Snapshot testing**: `cargo insta accept --all` to accept new snapshots
- **Pre-commit**: treefmt hook runs automatically on commit
- **Dependency docs**: Use Context7 MCP server (`mcp__context7__resolve-library-id` and `mcp__context7__get-library-docs`) to look up crate documentation before adding new dependencies

## Test Organization

- `docs/corpus/` contains canonical test case documentation
- E2E tests in `tests/e2e_test.rs` mirror corpus cases 1:1
- Snapshot tests capture both diff parsing (unit) and git operations (e2e)

When adding new functionality: document in corpus first, then implement test.

## Known Gotchas

### Line numbers shift after insertions
When staging from multiple hunks, earlier insertions shift later line numbers:
```
Insert after line 6 → line 137 becomes 138, line 142 becomes 143
```
The diff output shows correct numbers; just don't use stale numbers from before a partial stage.

### Diff header lines look like deletions
`--- a/file` starts with `-` but is not a deletion line. Must check for `--- a/` prefix before treating as deletion content. Same for `+++ b/` and additions.

### error_set! cannot reference external types directly
Each `error_set!` block is self-contained. To compose errors from different modules, use tuple variants like `ParseError(ParseError)` instead of `|| ParseError`. The `||` operator only works within the same `error_set!` block.
