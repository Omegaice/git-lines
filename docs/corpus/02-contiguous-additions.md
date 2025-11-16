# Case 2: Contiguous Line Additions (Range)

## The Diff

```bash
$ git-stager diff flake.nix
flake.nix:
  +39:
  +40:	    stylix = {
  +41:	      url = "github:nix-community/stylix";
  +42:	      inputs.nixpkgs.follows = "nixpkgs";
  +43:	    };
```

## What to Stage

Lines 39-43 (all five lines).

## Command

```bash
git-stager stage flake.nix:39..43
```

## Expected Result

```bash
$ git diff --cached flake.nix
@@ -38,0 +39,5 @@
+
+    stylix = {
+      url = "github:nix-community/stylix";
+      inputs.nixpkgs.follows = "nixpkgs";
+    };
```

## Why This Matters

Range syntax. Proves:
- Parse range notation `N..M`
- Construct multi-line patch
- Correct line count in hunk header
