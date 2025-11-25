# Replacement Patches

This document specifies all valid ways to generate replacement patches (delete + add at same location).

## 3.1: Simple Single-Line Replacement

**Purpose**: Verify basic replacement of one line.

**Input Diff**:
```
  -10:     old_value = "deprecated";
  +10:     new_value = "modern";
```

**Command**: `git-lines stage file.nix:-10,10`

**Expected Patch**:
```diff
@@ -10 +10 @@
-    old_value = "deprecated";
+    new_value = "modern";
```

## 3.2: Multi-Line Replacement

**Purpose**: Verify replacement of multiple consecutive lines.

**Input Diff**:
```
  -20:     # Old implementation
  -21:     legacy_function() {
  -22:       old_code();
  -23:     }
  +20:     # New implementation
  +21:     modern_function() {
  +22:       new_code();
  +23:       extra_feature();
  +24:     }
```

**Command**: `git-lines stage file.js:-20..-23,20..24`

**Expected Patch**:
```diff
@@ -20,4 +20,5 @@
-    # Old implementation
-    legacy_function() {
-      old_code();
-    }
+    # New implementation
+    modern_function() {
+      new_code();
+      extra_feature();
+    }
```

## 3.3: Partial Replacement from Mixed Hunk

**Purpose**: Verify selective replacement within larger change.

**Input Diff**:
```
  -10:     setting_a = true;
  -11:     setting_b = false;
  -12:     setting_c = "old";
  +10:     setting_a = false;
  +11:     setting_b = false;
  +12:     setting_c = "new";
  +13:     setting_d = true;
```

**Command**: `git-lines stage file.nix:-10,10,-12,12`

**Expected Patch**:
```diff
@@ -10 +10 @@
-    setting_a = true;
+    setting_a = false;
@@ -12 +12 @@
-    setting_c = "old";
+    setting_c = "new";
```

## 3.4: Asymmetric Replacement

**Purpose**: Verify replacement with different line counts.

**Input Diff**:
```
  -30:     verbose_old_style_config();
  +30:     cfg();
```

**Command**: `git-lines stage file.nix:-30,30`

**Expected Patch**:
```diff
@@ -30 +30 @@
-    verbose_old_style_config();
+    cfg();
```

## 3.5: Multiple Separate Replacements

**Purpose**: Verify multiple replacements in different locations.

**Input Diff**:
```
  -5:      const OLD_CONSTANT = 42;
  +5:      const NEW_CONSTANT = 100;

  -25:     deprecatedMethod() {}
  +25:     modernMethod() {}

  -80:     // Old comment
  +80:     // Updated comment
```

**Command**: `git-lines stage file.js:-5,5,-25,25`

**Expected Patch**:
```diff
@@ -5 +5 @@
-const OLD_CONSTANT = 42;
+const NEW_CONSTANT = 100;
@@ -25 +25 @@
-    deprecatedMethod() {}
+    modernMethod() {}
```

## 3.6: Complex Mixed Selection

**Purpose**: Verify replacement combined with pure additions/deletions.

**Input Diff**:
```
  -10:     # Header to remove
  -11:     old_setting = true;
  +10:     new_setting = false;
  +11:     added_setting = true;
```

**Command**: `git-lines stage file.nix:-11,10`

**Expected Patch**:
```diff
@@ -11 +10 @@
-    old_setting = true;
+    new_setting = false;
```

## 3.7: Replacement at Start of File

**Purpose**: Verify replacement when modifying line 1 (boundary condition).

**Initial File**:
```
old_first_line
line 2
line 3
...
```

**Input Diff**:
```
  -1:      old_first_line
  +1:      new_first_line
```

**Command**: `git-lines stage file.nix:-1,1`

**Expected Patch**:
```diff
@@ -1 +1 @@
-old_first_line
+new_first_line
```

**Note**: First line uses `old_start = 1`. This is the minimum valid line number.

## 3.8: Replacement at End of File

**Purpose**: Verify replacement when modifying the last line (boundary condition).

**Initial File** (10 lines):
```
line 1
...
line 9
old_last_line
```

**Input Diff**:
```
  -10:     old_last_line
  +10:     new_last_line
```

**Command**: `git-lines stage file.nix:-10,10`

**Expected Patch**:
```diff
@@ -10 +10 @@
-old_last_line
+new_last_line
```

**Note**: Last line replacement. Position remains unchanged as there are no prior hunks affecting cumulative delta.

## 3.9: Replacement at Start with No-Newline

**Purpose**: Verify replacement of first line when file lacks trailing newline.

**Initial File** (no trailing newline):
```
old_first_line
line 2
line 3
```

**Input Diff**:
```
  -1:      old_first_line
  +1:      new_first_line
```

**Command**: `git-lines stage file.nix:-1,1`

**Expected Patch**:
```diff
@@ -1 +1 @@
-old_first_line
+new_first_line
```

**Note**: First line replacement is unaffected by trailing newline status of file.

## Implementation Requirements

### Critical Git Invariants

1. **Position Calculation for Replacements**:
   - Formula: `new_start = old_start` (same position)
   - Replacements occur at the same logical position
   - Line counts may differ (asymmetric replacement)
   - Example: Replace line 10 → `@@ -10,1 +10,1 @@`

2. **Patch Header Format**:
   - Structure: `@@ -old_start,old_count +new_start,new_count @@`
   - For replacements: both counts are non-zero
   - old_count = number of deleted lines
   - new_count = number of added lines
   - Asymmetric allowed: `@@ -10,3 +10,1 @@` (3 lines → 1 line)

3. **Content Ordering**:
   - All deletion lines (`-`) must appear first
   - All addition lines (`+`) must appear after deletions
   - No interleaving of `-` and `+` lines within a hunk
   - Example:
     ```diff
     @@ -10,2 +10,2 @@
     -old line 1
     -old line 2
     +new line 1
     +new line 2
     ```

4. **Cumulative Position Adjustment**:
   - Delta calculation: `delta = new_count - old_count`
   - Can be positive (more lines added), negative (more deleted), or zero
   - Second hunk position: `new_start = old_start + cumulative_delta`
   - Example:
     ```
     First hunk:  @@ -10,2 +10,3 @@  (2→3 lines, delta = +1)
     Second hunk: @@ -30,1 +31,1 @@  (position shifts by +1)
     ```

5. **Staging Syntax**:
   - Use `-N,M` to stage the replacement of old line N with new line M
   - Can combine: `-10..-12,10..14` for multi-line replacements
   - Order matters for clarity but not function: `-10,10` preferred over `10,-10`

6. **Mixed Hunk Selection**:
   - Can select specific replacements from larger mixed hunks
   - Unselected changes remain in working directory
   - Each selected replacement becomes a separate hunk in the patch

7. **Git Apply Compatibility**:
   - Old content must match exactly for patch to apply
   - Both deletion and addition parts must be complete
   - Cannot stage only half of a replacement (either deletion or addition alone)

8. **Special Cases**:
   - Empty line replacement: `-` followed by `+` (both bare)
   - Whitespace changes: Preserve exact spacing in both parts
   - Line ending changes: Part of the content, must be preserved

### Validation Checklist

Before generating a replacement patch, verify:
- [ ] Deletions staged with corresponding additions (`-N,M` syntax)
- [ ] new_start = old_start for all replacements
- [ ] All `-` lines appear before `+` lines in each hunk
- [ ] old_count matches number of `-` lines
- [ ] new_count matches number of `+` lines
- [ ] Content preserves exact whitespace
- [ ] Cumulative adjustments applied for multi-hunk patches
- [ ] Complete replacement staged (not partial)