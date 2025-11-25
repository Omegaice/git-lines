# Addition Patches

This document specifies all valid ways to generate addition-only patches.

## 1.1: Single Line Addition

**Purpose**: Verify basic addition of one line.

**Input Diff**:
```
  +137:       debug = true;
```

**Command**: `git-lines stage file.nix:137`

**Expected Patch**:
```diff
@@ -136,0 +137 @@
+      debug = true;
```

## 1.2: Contiguous Range Addition

**Purpose**: Verify range syntax produces multi-line addition patch.

**Input Diff**:
```
  +39:
  +40:     stylix = {
  +41:       url = "github:danth/stylix";
  +42:       inputs.nixpkgs.follows = "nixpkgs";
  +43:     };
```

**Command**: `git-lines stage file.nix:39..43`

**Expected Patch**:
```diff
@@ -38,0 +39,5 @@
+
+    stylix = {
+      url = "github:danth/stylix";
+      inputs.nixpkgs.follows = "nixpkgs";
+    };
```

## 1.3: Non-Contiguous Selection

**Purpose**: Verify comma-separated selection from contiguous additions.

**Input Diff**:
```
  +10:     # TODO: Remove after testing
  +11:     debug.enable = true;
  +12:     debug.verbose = true;
  +13:     # Another comment
  +14:     feature.enable = true;
```

**Command**: `git-lines stage file.nix:11,12,14`

**Expected Patch**:
```diff
@@ -9,0 +10,3 @@
+    debug.enable = true;
+    debug.verbose = true;
+    feature.enable = true;
```

## 1.4: Addition from Mixed Hunk

**Purpose**: Verify extraction of additions from hunk containing deletions.

**Input Diff**:
```
  -25:     old_setting = true;
  -26:     deprecated = true;
  +25:     new_setting = false;
  +26:     modern = true;
  +27:     additional = true;
```

**Command**: `git-lines stage file.nix:27`

**Expected Patch**:
```diff
@@ -26,0 +27 @@
+    additional = true;
```

## 1.5: Multiple Separate Additions

**Purpose**: Verify selection from multiple addition-only hunks.

**Input Diff**:
```
  +7:      first_addition = true;

  +45:     second_addition = true;

  +120:    third_addition = true;
```

**Command**: `git-lines stage file.nix:7,45`

**Expected Patch**:
```diff
@@ -6,0 +7 @@
+     first_addition = true;
@@ -44,0 +45 @@
+    second_addition = true;
```

## 1.6: Complex Range and Individual Mix

**Purpose**: Verify combined range and individual selection syntax.

**Input Diff**:
```
  +30:     line_30 = true;
  +31:     line_31 = true;
  +32:     line_32 = true;
  +33:     line_33 = true;
  +34:     line_34 = true;
  +35:     line_35 = true;
```

**Command**: `git-lines stage file.nix:30,32..34`

**Expected Patch**:
```diff
@@ -29,0 +30,4 @@
+    line_30 = true;
+    line_32 = true;
+    line_33 = true;
+    line_34 = true;
```

## 1.7: Non-Contiguous Selection (Mid-File Insertion)

**Purpose**: Verify non-contiguous selection when additions are inserted in the middle of a file (content exists both before and after the insertion point).

**Initial File** (10 lines):
```
line 1
line 2
line 3
...
line 10
```

**Input Diff** (insertions after line 2, before line 3):
```
  +3:     addition_a = true;
  +4:     addition_b = true;
  +5:     addition_c = true;
  +6:     addition_d = true;
```

**Command**: `git-lines stage file.nix:3,5`

**Expected Patch**:
```diff
@@ -2,0 +3,2 @@
+    addition_a = true;
+    addition_c = true;
```

**Note**: All selected lines must remain consecutive in the output patch, anchored to the original insertion point (after line 2). The gap in selection (skipping line 4) does not create separate hunks.

## 1.8: Non-Contiguous Selection (Start-of-File Insertion)

**Purpose**: Verify non-contiguous selection when additions are inserted at the start of a file (content exists only after the insertion point).

**Initial File** (5 lines):
```
line 1
line 2
...
line 5
```

**Input Diff** (insertions before line 1):
```
  +1:     addition_a = true;
  +2:     addition_b = true;
  +3:     addition_c = true;
  +4:     addition_d = true;
```

**Command**: `git-lines stage file.nix:1,3`

**Expected Patch**:
```diff
@@ -0,0 +1,2 @@
+    addition_a = true;
+    addition_c = true;
```

**Note**: Start-of-file insertions use `old_start = 0` in the hunk header. Selected lines remain consecutive regardless of gaps in selection.

## Implementation Requirements

### Critical Git Invariants

1. **Position Calculation for Pure Additions**:
   - Formula: `new_start = old_start + 1`
   - Example: Inserting after line 10 → `@@ -10,0 +11,N @@`
   - Special case: Inserting at file start → `@@ -0,0 +1,N @@`

2. **Patch Header Format**:
   - Structure: `@@ -old_start,old_count +new_start,new_count @@`
   - For pure additions: `old_count = 0`
   - Count can be implicit when `count = 1`: `@@ -10 +11 @@` equals `@@ -10,1 +11,1 @@`
   - Line numbers are 1-indexed (first line is 1, not 0)

3. **Cumulative Position Adjustment**:
   - When multiple hunks exist, later hunks must account for earlier changes
   - Delta calculation: `delta = new_count - old_count` for each prior hunk
   - Second hunk position: `new_start = old_start + cumulative_delta + 1` (for pure additions)
   - Example:
     ```
     First hunk:  @@ -10,0 +11,2 @@  (adds 2 lines, delta = +2)
     Second hunk: @@ -30,0 +33,1 @@  (original line 30 → new line 33)
     ```

4. **Line Content Preservation**:
   - Each addition line starts with `+` followed by the exact content
   - Empty lines are represented as bare `+` with no trailing spaces
   - Whitespace after `+` must be preserved exactly as in the source
   - No trailing newline on the last line of the patch content

5. **Git Apply Compatibility**:
   - Patches must work with `git apply --cached --unidiff-zero`
   - The `--unidiff-zero` flag is required for `-U0` context patches
   - File must exist in the repository for the patch to apply
   - Line positions must be valid (not out of bounds)

6. **Multi-Hunk Requirements**:
   - Hunks must be ordered by `old_start` position (top to bottom)
   - Each hunk is independent and has its own `@@` header
   - No blank lines between hunk header and content
   - Hunks are separated by the next `@@` header or end of patch

7. **Patch File Structure**:
   - Minimal valid patch needs only:
     ```
     --- a/filename
     +++ b/filename
     @@ -N,0 +M,count @@
     +content lines
     ```
   - The `diff --git` header is optional for `git apply`
   - Index line is optional

8. **Selection from Mixed Hunks**:
   - When selecting only additions from a mixed add/delete hunk:
     - The patch contains ONLY the selected addition lines
     - Position calculations must account for skipped deletions
     - old_start in patch header reflects position after deletions are considered
     - Example: If selecting line +27 from a hunk with deletions at 25-26, patch header: `@@ -26,0 +27 @@`

### Validation Checklist

Before generating an addition patch, verify:
- [ ] Line numbers are 1-indexed
- [ ] new_start = old_start + 1 for pure additions
- [ ] old_count = 0 (no deletions)
- [ ] new_count matches actual number of `+` lines
- [ ] Content preserves exact whitespace
- [ ] Hunks ordered by position (top to bottom)
- [ ] Cumulative adjustments applied for multi-hunk patches