# Case 9: Out-of-Order Sequential Staging

## The Diff

```bash
$ git-stager diff config.nix
config.nix:
  +3:	# FIRST INSERTION

  +10:	# SECOND INSERTION
```

## What to Stage

Stage line 10 first (second hunk), then line 3 (first hunk) in a separate command.

## Commands

```bash
git-stager stage config.nix:10
git-stager stage config.nix:3
```

## Expected Result

After first command (line 10):
```bash
$ git diff --cached config.nix
@@ -8,0 +9 @@
+# SECOND INSERTION
```

After second command (line 3):
```bash
$ git diff --cached config.nix
@@ -2,0 +3 @@
+# FIRST INSERTION
@@ -8,0 +10 @@
+# SECOND INSERTION
```

## Why This Matters

Proves that:
- Line numbers in `git-stager diff` remain valid after partial staging
- Each stage command gets fresh diff of remaining unstaged changes
- Order of staging doesn't affect final result
- Position recalculation handles intervening hunks correctly

This is the natural LLM workflow - stage changes based on semantic grouping, not file position.
