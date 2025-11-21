# Deletion Patches

This document specifies all valid ways to generate deletion-only patches.

## 2.1: Single Line Deletion

**Purpose**: Verify basic deletion of one line.

**Input Diff**:
```
  -15:       enableAutosuggestions = true;
```

**Command**: `git-stager stage file.nix:-15`

**Expected Patch**:
```diff
@@ -15 +14,0 @@
-      enableAutosuggestions = true;
```

## 2.2: Contiguous Range Deletion

**Purpose**: Verify range syntax produces multi-line deletion patch.

**Input Diff**:
```
  -15:       enableAutosuggestions = true;
  -16:       enableCompletion = true;
  -17:       enableSyntaxHighlighting = true;
```

**Command**: `git-stager stage file.nix:-15..-17`

**Expected Patch**:
```diff
@@ -15,3 +14,0 @@
-      enableAutosuggestions = true;
-      enableCompletion = true;
-      enableSyntaxHighlighting = true;
```

## 2.3: Non-Contiguous Deletion

**Purpose**: Verify comma-separated deletion from contiguous deletions.

**Input Diff**:
```
  -10:     # Old comment
  -11:     deprecated_setting = true;
  -12:     another_deprecated = true;
  -13:     # Another old comment
  -14:     legacy_feature = true;
```

**Command**: `git-stager stage file.nix:-11,-12,-14`

**Expected Patch**:
```diff
@@ -11,2 +10,0 @@
-    deprecated_setting = true;
-    another_deprecated = true;
@@ -14 +11,0 @@
-    legacy_feature = true;
```

## 2.4: Deletion from Mixed Hunk

**Purpose**: Verify extraction of deletions from hunk containing additions.

**Input Diff**:
```
  -25:     old_setting = true;
  -26:     deprecated = true;
  +25:     new_setting = false;
  +26:     modern = true;
  +27:     additional = true;
```

**Command**: `git-stager stage file.nix:-26`

**Expected Patch**:
```diff
@@ -26 +25,0 @@
-    deprecated = true;
```

## 2.5: Deletion at Start of File

**Purpose**: Verify deletion of first line(s) in file.

**Input Diff**:
```
  -1:      #!/usr/bin/env bash
  -2:      # Old header comment
```

**Command**: `git-stager stage file.sh:-1..-2`

**Expected Patch**:
```diff
@@ -1,2 +0,0 @@
-#!/usr/bin/env bash
-# Old header comment
```

## 2.6: Deletion at End of File

**Purpose**: Verify deletion of last line(s) in file.

**Input Diff**:
```
  -98:     last_setting = true;
  -99:     final_option = false;
  -100:    # EOF comment
```

**Command**: `git-stager stage file.nix:-98..-100`

**Expected Patch**:
```diff
@@ -98,3 +97,0 @@
-    last_setting = true;
-    final_option = false;
-    # EOF comment
```

## 2.7: Delete Only Line

**Purpose**: Verify deletion resulting in empty file.

**Input Diff**:
```
  -1:      only content
```

**Command**: `git-stager stage file.txt:-1`

**Expected Patch**:
```diff
@@ -1 +0,0 @@
-only content
```

## Implementation Requirements

### Critical Git Invariants

1. **Position Calculation for Pure Deletions**:
   - Formula: `new_start = old_start - 1`
   - Example: Deleting line 10 → `@@ -10,N +9,0 @@`
   - Special case: Deleting from line 1 → `@@ -1,N +0,0 @@`
   - Edge case: Deleting only line → `@@ -1,1 +0,0 @@`

2. **Patch Header Format**:
   - Structure: `@@ -old_start,old_count +new_start,new_count @@`
   - For pure deletions: `new_count = 0`
   - Count can be implicit when `count = 1`: `@@ -10 +9 @@` equals `@@ -10,1 +9,0 @@`
   - Negative line references (`-N`) refer to old file line numbers

3. **Cumulative Position Adjustment**:
   - When multiple hunks exist, later hunks must account for earlier changes
   - Delta calculation: `delta = new_count - old_count` for each prior hunk
   - For pure deletions: `delta = -old_count` (negative)
   - Second hunk position: `new_start = (old_start - 1) + cumulative_delta`
   - Example:
     ```
     First hunk:  @@ -10,2 +9,0 @@  (deletes 2 lines, delta = -2)
     Second hunk: @@ -30,1 +27,0 @@  (original line 30 → new line 27)
     ```

4. **Line Content Preservation**:
   - Each deletion line starts with `-` followed by the exact content
   - Empty lines are represented as bare `-` with no trailing spaces
   - Whitespace after `-` must be preserved exactly as in the source
   - No trailing newline on the last line of the patch content

5. **Git Apply Compatibility**:
   - Patches must work with `git apply --cached --unidiff-zero`
   - File must exist in the repository with matching content
   - Deleted content must match exactly or patch will fail
   - Line positions must be valid (not out of bounds)

6. **Multi-Hunk Requirements**:
   - Hunks must be ordered by `old_start` position (top to bottom)
   - Each hunk is independent and has its own `@@` header
   - Non-contiguous deletions create separate hunks
   - Position adjustments cascade through subsequent hunks

7. **Patch File Structure**:
   - Minimal valid patch needs only:
     ```
     --- a/filename
     +++ b/filename
     @@ -N,count +M,0 @@
     -content lines
     ```
   - The `diff --git` header is optional for `git apply`
   - Index line is optional

8. **Selection from Mixed Hunks**:
   - When selecting only deletions from a mixed add/delete hunk:
     - The patch contains ONLY the selected deletion lines
     - Position calculations are based on original file line numbers
     - new_start reflects cumulative effect of prior deletions

### Validation Checklist

Before generating a deletion patch, verify:
- [ ] Line references use `-N` syntax for old line numbers
- [ ] new_start = old_start - 1 for pure deletions
- [ ] new_count = 0 (no additions)
- [ ] old_count matches actual number of `-` lines
- [ ] Content preserves exact whitespace
- [ ] Hunks ordered by position (top to bottom)
- [ ] Cumulative adjustments applied for multi-hunk patches
- [ ] Special handling for deletions at line 1 (new_start = 0)