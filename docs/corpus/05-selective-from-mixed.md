# Case 5: Selective Staging from Mixed Add/Delete

## The Diff

```bash
$ git-stager diff home/gtk.nix
home/gtk.nix:
  -10:	    gtk.theme.name = "Adwaita";
  -11:	    gtk.iconTheme.name = "Papirus";
  +10:	    # Theme managed by Stylix
  +11:	    gtk.iconTheme.name = "Papirus-Dark";
  +12:	    gtk.cursorTheme.size = 24;
```

## What to Stage

Only the cursor size addition (line 12), not the theme deletion or icon theme modification.

## Command

```bash
git-stager stage home/gtk.nix:12
```

## Expected Result

```bash
$ git diff --cached home/gtk.nix
@@ -11,0 +12 @@
+    gtk.cursorTheme.size = 24;

$ git diff home/gtk.nix
@@ -10,2 +10,2 @@
-    gtk.theme.name = "Adwaita";
-    gtk.iconTheme.name = "Papirus";
+    # Theme managed by Stylix
+    gtk.iconTheme.name = "Papirus-Dark";
```

## Why This Matters

Most complex case. Mixed adds and deletes, stage only specific adds. Proves:
- Can extract additions while ignoring deletions in same region
- Patch construction handles partial selection from mixed operations
- Line numbers recalculate correctly for remaining changes

## Alternate: Stage Only the Deletions

```bash
git-stager stage home/gtk.nix:-10,-11
```

Or stage a range of deletions:
```bash
git-stager stage home/gtk.nix:-10..-11
```

Or stage the replacement (delete + add as a unit):
```bash
git-stager stage home/gtk.nix:-11,11
```

Where `-11` is the deletion of old line 11, and `11` is the addition of new line 11.
