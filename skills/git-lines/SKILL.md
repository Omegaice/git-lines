---
name: git-lines
description: Stages specific lines from git diffs when hunk-level staging is too coarse. Use when a file contains multiple unrelated changes that need separate semantic commits and git's hunks group them together.
---

# Line-Level Git Staging

## When to Use

Reach for git-lines **only** after deciding a file's changes need splitting:

1. You've reviewed changes with `git diff` (normal git)
2. You see multiple unrelated changes in one file
3. Git's hunks group changes that belong in separate commits

For everything else—viewing changes, staging whole files, committing—use normal git commands.

## Workflow

```
git diff                        # review changes (normal git)
↓ decide: "need to split this file"
git lines diff config.rs        # get line numbers
git lines stage config.rs:10    # stage first change
git commit -m "feat: first change"
git lines stage config.rs:25    # stage second change
git commit -m "fix: second change"
```

## Commands

Can be invoked as `git-lines` or `git lines` (if in PATH).

**View line numbers** (only when you intend to stage):
```bash
git lines diff              # all changed files
git lines diff file.rs      # specific file
```

Output format: `+N:` additions, `-N:` deletions.

**Stage specific lines**:
```bash
git lines stage file.rs:10         # single addition
git lines stage file.rs:-10        # single deletion
git lines stage file.rs:10..20     # range
git lines stage file.rs:10,15,-20  # multiple (mixed)
git lines stage a.rs:5 b.rs:10     # multiple files
```

## Line Numbers Stay Stable

Line numbers always refer to working tree positions, which don't change until you edit the file. You can run multiple `git lines stage` commands using line numbers from the same initial `git lines diff` output.
