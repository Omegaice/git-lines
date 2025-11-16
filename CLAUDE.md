## git-stager

A line-level staging tool for when you need to commit unrelated changes separately from the same file.

**The problem:** When a file contains multiple unrelated changes (e.g., a bug fix on line 45 and a new feature on line 120), git groups them into hunks. `git add -p` can split hunks interactively, but you cannot use interactive commands.

**When to use git-stager:**
- You have multiple logical changes in a single file that belong in separate commits
- Git's hunks are too coarse-grained for your needs

**When NOT to use it:**
- Staging entire files → `git add file.rs`
- Staging all changes → `git add .`
- New/untracked files → `git add newfile.rs`
- Discarding changes → `git restore file.rs`

**Workflow:**
```bash
# 1. See what's available with line numbers
git-stager diff src/lib.rs

# 2. Stage specific lines (numbers from step 1)
git-stager stage src/lib.rs:45,46    # additions
git-stager stage src/lib.rs:-30      # deletion
git-stager stage src/lib.rs:10..15   # range

# 3. Commit, then repeat for remaining changes
git commit -m "fix: ..."
git-stager stage src/lib.rs:120..125
git commit -m "feat: ..."
```

This is supplemental to git, not a replacement. Reach for standard git commands first.
