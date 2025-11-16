# Case 1: Single Line Addition

## The Diff

```bash
$ git diff -U0 flake.nix
@@ -136,0 +137 @@
+      debug = true;
```

## What to Stage

Line 137 only.

## Command

```bash
git-stager stage flake.nix:137
```

## Expected Result

```bash
$ git diff --cached flake.nix
@@ -136,0 +137 @@
+      debug = true;
```

## Why This Matters

Simplest case. Proves:
- Parse diff to identify added line
- Construct single-line patch with correct header
- Apply via `git apply --cached`
