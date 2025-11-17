# Case 11: Files Without Trailing Newline

## Background

Files without a trailing newline at EOF require special handling. When you add lines after such a file, git sees this as:
1. Delete the old last line (which had no `\n`)
2. Add the same content back (now WITH `\n` because more lines follow)
3. Add the new lines

## The Diff

```bash
$ git-stager diff config.nix
config.nix:
  -3:	no newline
  +3:	no newline
  +4:	new line
```

The `-3` and `+3` are the same content - but `+3` now has a trailing `\n` because line 4 follows it.

## Scenario A: Stage Only the New Line

### What to Stage

Line 4 only.

### Command

```bash
git-stager stage config.nix:4
```

### Expected Result

```bash
$ git diff --cached config.nix
@@ -3 +3,2 @@
-no newline
\ No newline at end of file
+no newline
+new line
\ No newline at end of file
```

**Important**: git-stager automatically includes `-3` and `+3` even though you only requested `+4`. This is required - you cannot add line 4 without giving line 3 its trailing `\n`.

## Scenario B: Stage the Bridge Only

### What to Stage

Just add the trailing newline to line 3, without adding new lines.

### Command

```bash
git-stager stage config.nix:-3,3
```

### Expected Result

```bash
$ git diff --cached config.nix
@@ -3 +3 @@
-no newline
\ No newline at end of file
+no newline
```

Line 3 now has a trailing newline.

## Scenario C: Delete the No-Newline Line

### Command

```bash
git-stager stage config.nix:-3
```

### Expected Result

```bash
$ git diff --cached config.nix
@@ -3 +2,0 @@
-no newline
\ No newline at end of file
```

## Scenario D: Modify Content (Stays No-Newline)

### The Diff

```bash
$ git-stager diff config.nix
config.nix:
  -3:	old content
  +3:	new content
```

### Command

```bash
git-stager stage config.nix:-3,3
```

### Expected Result

```bash
$ git diff --cached config.nix
@@ -3 +3 @@
-old content
\ No newline at end of file
+new content
\ No newline at end of file
```

Both old and new lines lack trailing newlines.

## Why This Matters

Proves that:
- `\ No newline at end of file` markers are preserved in patches
- Automatic bridge synthesis when staging additions after no-newline lines
- User doesn't need to understand the internal representation
- All combinations (add, delete, modify) work correctly

This edge case can corrupt the git index if not handled properly, producing concatenated content instead of separate lines.
