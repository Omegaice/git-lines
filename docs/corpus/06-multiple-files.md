# Case 6: Multiple Files in Single Command

## The Diff

```bash
$ git-stager diff
flake.nix:
  +137:       debug = true;

zsh.nix:
  -15:       enableAutosuggestions = true;

gtk.nix:
  +12:     gtk.cursorTheme.size = 24;
```

## What to Stage

Line 137 from flake.nix and line 12 from gtk.nix (skip zsh.nix deletion).

## Command

```bash
git-stager stage flake.nix:137 gtk.nix:12
```

## Expected Result

```bash
$ git diff --cached flake.nix
@@ -136,0 +137 @@
+      debug = true;

$ git diff --cached gtk.nix
@@ -11,0 +12 @@
+    gtk.cursorTheme.size = 24;

$ git diff zsh.nix
@@ -15 +14,0 @@
-      enableAutosuggestions = true;
```

The zsh.nix deletion remains unstaged.

## Why This Matters

Batch operations across files. Proves:
- Multiple file:refs arguments processed sequentially
- Each file staged independently
- Useful for grouping related changes across files into single commit
