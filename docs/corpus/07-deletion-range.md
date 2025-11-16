# Case 7: Deletion Range

## The Diff

```bash
$ git-stager diff zsh.nix
zsh.nix:
  -15:       enableAutosuggestions = true;
  -16:       enableCompletion = true;
  -17:       enableSyntaxHighlighting = true;
```

## What to Stage

Delete lines 15-17 (remove three consecutive settings).

## Command

```bash
git-stager stage zsh.nix:-15..-17
```

OR equivalently:

```bash
git-stager stage zsh.nix:-15,-16,-17
```

## Expected Result

```bash
$ git diff --cached zsh.nix
@@ -15,3 +14,0 @@
-      enableAutosuggestions = true;
-      enableCompletion = true;
-      enableSyntaxHighlighting = true;
```

## Why This Matters

Range syntax for deletions. Proves:
- `-N..-M` syntax works for deletion ranges
- Mirrors addition range syntax (`N..M`) with `-` prefix
- Constructs multi-line deletion patch
- Correct hunk header for multiple removals
