# Case 10: Single Command Multiple Hunks

## The Diff

```bash
$ git-stager diff config.nix
config.nix:
  +3:	# FIRST INSERTION

  +10:	# SECOND INSERTION
```

## What to Stage

Both insertions in a single command.

## Command

```bash
git-stager stage config.nix:10,3
```

Or equivalently:
```bash
git-stager stage config.nix:3,10
```

## Expected Result

```bash
$ git diff --cached config.nix
@@ -2,0 +3 @@
+# FIRST INSERTION
@@ -8,0 +10 @@
+# SECOND INSERTION
```

## Why This Matters

Proves that:
- Multiple hunks can be staged in single command
- Order of line numbers in command doesn't matter
- Patch construction handles multiple non-adjacent regions
- Equivalent to case 9, but more efficient (one command vs two)

Useful when multiple unrelated changes should go in the same commit.
