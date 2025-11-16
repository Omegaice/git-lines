# Case 4: Staging a Deletion

## The Diff

```bash
$ git diff -U0 home/terminal/shell/zsh.nix
@@ -15 +14,0 @@
-      enableAutosuggestions = true;
```

## What to Stage

The deletion of the line that was at old line 15.

## Command

```bash
git-stager stage home/terminal/shell/zsh.nix:-15
```

The `-15` references old line 15 (the deleted line), matching hunk header syntax `@@ -15 ... @@`.

## Expected Result

```bash
$ git diff --cached home/terminal/shell/zsh.nix
@@ -15 +14,0 @@
-      enableAutosuggestions = true;
```

## Why This Matters

Deletions don't have a "new line number". Proves:
- Reference scheme using `-N` for old line N (matches hunk header style)
- Construct deletion patch
- Handle hunk header for removals: `@@ -N,count +M,0 @@`
