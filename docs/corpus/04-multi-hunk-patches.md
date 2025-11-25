# Multi-Hunk Patches (Single File)

This document specifies patches with multiple hunks within a single file.

## 4.1: Two Separate Additions

**Purpose**: Verify multiple addition hunks with position adjustment.

**Input Diff**:
```
  +7:      first_addition = true;

  +45:     second_addition = true;

  +120:    third_addition = true;
```

**Command**: `git-lines stage file.nix:7,45,120`

**Expected Patch**:
```diff
@@ -6,0 +7 @@
+     first_addition = true;
@@ -44,0 +46 @@
+    second_addition = true;
@@ -119,0 +122 @@
+    third_addition = true;
```

## 4.2: Mixed Operations in Different Hunks

**Purpose**: Verify combination of additions, deletions, and replacements.

**Input Diff**:
```
  +10:     added_line = true;

  -30:     deleted_line = false;

  -50:     old_value = 1;
  +49:     new_value = 2;
```

**Command**: `git-lines stage file.nix:10,-30,-50,49`

**Expected Patch**:
```diff
@@ -9,0 +10 @@
+    added_line = true;
@@ -30 +30,0 @@
-    deleted_line = false;
@@ -50 +49 @@
-    old_value = 1;
+    new_value = 2;
```

## 4.3: Non-Contiguous Selection Creating Multiple Hunks

**Purpose**: Verify that non-contiguous selections from single diff create separate hunks.

**Input Diff**:
```
  +20:     line_20 = true;
  +21:     line_21 = true;
  +22:     line_22 = true;
  +23:     line_23 = true;
  +24:     line_24 = true;
```

**Command**: `git-lines stage file.nix:20,22,24`

**Expected Patch**:
```diff
@@ -19,0 +20 @@
+    line_20 = true;
@@ -20,0 +22 @@
+    line_22 = true;
@@ -21,0 +24 @@
+    line_24 = true;
```

## 4.4: Cumulative Position Tracking

**Purpose**: Verify correct position adjustment through multiple operations.

**Input Diff**:
```
  +10:     // Add 2 lines here
  +11:     first_new_line();

  -30:     // Delete 3 lines
  -31:     old_line_one();
  -32:     old_line_two();

  +50:     // Add 1 line (originally line 52)
```

**Command**: `git-lines stage file.js:10,11,-30..-32,50`

**Expected Patch**:
```diff
@@ -9,0 +10,2 @@
+    // Add 2 lines here
+    first_new_line();
@@ -30,3 +31,0 @@
-    // Delete 3 lines
-    old_line_one();
-    old_line_two();
@@ -49,0 +48 @@
+    // Add 1 line (originally line 52)
```

## 4.5: Many Hunks Performance Test

**Purpose**: Verify handling of many separate changes.

**Input Diff**:
```
  +5:      change_1();
  +15:     change_2();
  +25:     change_3();
  +35:     change_4();
  +45:     change_5();
  +55:     change_6();
  +65:     change_7();
  +75:     change_8();
  +85:     change_9();
  +95:     change_10();
```

**Command**: `git-lines stage file.js:5,15,25,35,45,55,65,75,85,95`

**Expected Patch**:
```diff
@@ -4,0 +5 @@
+     change_1();
@@ -14,0 +16 @@
+    change_2();
@@ -24,0 +27 @@
+    change_3();
@@ -34,0 +38 @@
+    change_4();
@@ -44,0 +49 @@
+    change_5();
@@ -54,0 +60 @@
+    change_6();
@@ -64,0 +71 @@
+    change_7();
@@ -74,0 +82 @@
+    change_8();
@@ -84,0 +93 @@
+    change_9();
@@ -94,0 +104 @@
+    change_10();
```

## 4.6: Order Independence

**Purpose**: Verify that staging order doesn't affect final patch.

**Input Diff**:
```
  +3:      early_addition();

  +50:     late_addition();
```

**Command**: `git-lines stage file.nix:50,3`

**Expected Patch**:
```diff
@@ -2,0 +3 @@
+     early_addition();
@@ -49,0 +51 @@
+    late_addition();
```

## Implementation Requirements

### Critical Git Invariants

1. **Hunk Ordering**:
   - Hunks MUST be ordered by `old_start` position (top to bottom)
   - This is independent of staging order
   - Git requires hunks in file position order for apply to work

2. **Position Adjustment Formula**:
   - Each hunk affects positions of all subsequent hunks
   - Delta for hunk i: `delta_i = new_count_i - old_count_i`
   - Cumulative delta: `cumulative_delta = sum(delta_1..delta_n)`
   - Hunk n+1 positions:
     - `old_start` unchanged (references original file)
     - `new_start = old_start + cumulative_delta + position_offset`
   - Position offset depends on operation type:
     - Addition: `+1`
     - Deletion: `-1`
     - Replacement: `0`

3. **Cumulative Adjustment Examples**:
   ```
   Hunk 1: @@ -10,0 +11,2 @@  (adds 2, delta = +2)
   Hunk 2: @@ -30,1 +33,0 @@  (30 + 2 + 1 = 33 for addition)
   Hunk 3: @@ -50,1 +52,1 @@  (50 + 2 - 0 = 52 for replacement)
   ```

4. **Hunk Separation Requirements**:
   - Each hunk has its own `@@` header
   - No blank lines between header and content
   - Hunks separated by next `@@` header
   - File header (`--- a/file` and `+++ b/file`) appears once

5. **Non-Contiguous Selection Rules**:
   - Hunk content MUST be contiguous (consecutive lines from the file)
   - Git rejects patches where a single hunk claims non-consecutive lines
   - Skipping lines in a contiguous block creates separate hunks
   - Each selected region becomes its own hunk
   - Position calculations must account for all prior hunks

6. **Mixed Operation Ordering**:
   - Within each hunk: deletions before additions
   - Across hunks: file position order (top to bottom)
   - Operation type doesn't affect hunk ordering

7. **Performance Considerations**:
   - No practical limit on number of hunks
   - Each hunk independently validated
   - Position calculations are O(n) where n = number of hunks

8. **Staging Command Behavior**:
   - Multiple line references can be in any order
   - Parser must sort and deduplicate
   - Final patch always in canonical order

### Validation Checklist

Before generating a multi-hunk patch, verify:
- [ ] Hunks sorted by old_start position
- [ ] Each hunk has correct cumulative adjustment
- [ ] new_start calculations account for all prior deltas
- [ ] No overlapping hunks
- [ ] Each hunk has valid `@@` header
- [ ] File headers appear exactly once
- [ ] Position offset applied correctly per operation type
- [ ] All line references resolved to correct hunks