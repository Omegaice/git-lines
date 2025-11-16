# CLAUDE.md

## Tool Purpose

git-stager enables line-level staging when git's hunks are too coarse. It fills the gap left by `git add -p` which Claude cannot use interactively.

**Critical context**: This is a git companion tool, not a replacement. Claude uses git directly for everything else (staging whole files, committing, etc.). Only reach for git-stager when multiple unrelated changes exist in the same file and need separate commits.

Typical workflow:
1. `git-stager diff` to see available line numbers
2. `git-stager stage file:N` to stage specific lines
3. `git commit` as normal

## Development Tooling

- **Formatting**: Use `nix fmt`, NOT `cargo fmt` (treefmt-nix configured)
- **Snapshot testing**: `cargo insta accept --all` to accept new snapshots
- **Pre-commit**: treefmt hook runs automatically on commit

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

### Sandbox blocks heredocs in bash
Can't use `cat <<'EOF'` for commit messages due to sandbox restrictions. Use simple `-m "message"` instead.

### Empty string edge cases
`:10` (empty filename) and `file:` (empty refs) should error. Whitespace-only filenames should also be rejected.
