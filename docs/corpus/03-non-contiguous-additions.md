# Case 3: Partial Selection from Single Hunk

## The Diff

```bash
$ git diff -U0 home/programs/vscode/default.nix
@@ -39,0 +40,3 @@
+        # Allow Stylix to override terminal font
+        "terminal.integrated.fontFamily" = lib.mkDefault "monospace";
+        "direnv.restart.automatic" = true;
```

## What to Stage

Lines 40-41 only (Stylix changes), skip line 42 (direnv).

## Command

```bash
git-stager stage home/programs/vscode/default.nix:40..41
```

OR equivalently:

```bash
git-stager stage home/programs/vscode/default.nix:40,41
```

## Expected Result

```bash
$ git diff --cached home/programs/vscode/default.nix
@@ -39,0 +40,2 @@
+        # Allow Stylix to override terminal font
+        "terminal.integrated.fontFamily" = lib.mkDefault "monospace";

$ git diff home/programs/vscode/default.nix
@@ -41,0 +42 @@
+        "direnv.restart.automatic" = true;
```

## Why This Matters

Selective extraction from single hunk. Proves:
- Can skip lines within Git's atomic hunk
- Constructs patch with only selected lines
- Remaining unstaged lines still available
- Line numbers adjust correctly in remaining diff
