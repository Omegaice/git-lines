# Case 8: Staging from Multiple Hunks in Same File

## The Diff

```bash
$ git-stager diff flake.nix
flake.nix:
  +7:       determinate.url = "github:DeterminateSystems/determinate";

  +137:       debug = true;

  +142:         ./flake-modules/home-manager.nix
```

Three separate hunks in the same file.

## What to Stage

Lines 7 and 142 (determinate input and home-manager import), skip line 137 (debug flag).

## Command

```bash
git-stager stage flake.nix:7,142
```

## Expected Result

```bash
$ git diff --cached flake.nix
@@ -6,0 +7 @@
+    determinate.url = "github:DeterminateSystems/determinate";
@@ -140,0 +142 @@
+        ./flake-modules/home-manager.nix

$ git diff flake.nix
@@ -136,0 +137 @@
+      debug = true;
```

The debug flag remains unstaged.

## Why This Matters

Non-contiguous hunks from same file. Proves:
- Can select lines from different hunks
- Patch contains multiple hunk headers
- Line references span across file regions
- Useful for grouping related changes scattered through a file
