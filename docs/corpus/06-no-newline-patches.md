# No-Newline Patches

This document specifies patches involving files without trailing newlines.

## 6.1: Adding After No-Newline (Auto-Bridge)

**Purpose**: Verify automatic bridge synthesis when staging additions after no-newline.

**Input Diff**:
```
  -3:  no newline
  +3:  no newline
  +4:  new line
```

**Command**: `git-stager stage config.nix:4`

**Expected Patch**:
```diff
@@ -3 +3,2 @@
-no newline
\ No newline at end of file
+no newline
+new line
\ No newline at end of file
```

**Note**: Staging line 4 automatically includes the `-3,+3` bridge to give line 3 its required newline.

## 6.2: Staging Complete Change

**Purpose**: Verify explicit staging of bridge plus additions.

**Input Diff**:
```
  -3:  no newline
  +3:  no newline
  +4:  new line
```

**Command**: `git-stager stage config.nix:-3,3,4`

**Expected Patch**:
```diff
@@ -3 +3,2 @@
-no newline
\ No newline at end of file
+no newline
+new line
\ No newline at end of file
```

## 6.3: Bridge Only (Add Trailing Newline)

**Purpose**: Verify adding only a trailing newline to last line.

**Input Diff**:
```
  -3:  no newline
  +3:  no newline
```

**Command**: `git-stager stage config.nix:-3,3`

**Expected Patch**:
```diff
@@ -3 +3 @@
-no newline
\ No newline at end of file
+no newline
```

## 6.4: Delete No-Newline Line

**Purpose**: Verify deletion of line lacking trailing newline.

**Input Diff**:
```
  -3:  no newline
```

**Command**: `git-stager stage config.nix:-3`

**Expected Patch**:
```diff
@@ -3 +2,0 @@
-no newline
\ No newline at end of file
```

## 6.5: Modify Content (Stays No-Newline)

**Purpose**: Verify content change where both versions lack newline.

**Input Diff**:
```
  -3:  old content
  +3:  new content
```

**Command**: `git-stager stage config.nix:-3,3`

**Expected Patch**:
```diff
@@ -3 +3 @@
-old content
\ No newline at end of file
+new content
\ No newline at end of file
```

## 6.6: Skip Middle Line

**Purpose**: Verify staging line 5 while skipping line 4 after no-newline.

**Input Diff**:
```
  -3:  no newline
  +3:  no newline
  +4:  fourth line
  +5:  fifth line
```

**Command**: `git-stager stage config.nix:5`

**Expected Patch**:
```diff
@@ -3 +3,3 @@
-no newline
\ No newline at end of file
+no newline
+fourth line
+fifth line
\ No newline at end of file
```

**Note**: Bridge is auto-synthesized, and line 4 must be included to reach line 5.

## 6.7: Complex Multi-Line After No-Newline

**Purpose**: Verify selective staging from multiple additions after no-newline.

**Input Diff**:
```
  -10: last line
  +10: last line
  +11: added one
  +12: added two
  +13: added three
```

**Command**: `git-stager stage file.txt:11,13`

**Expected Patch**:
```diff
@@ -10 +10,3 @@
-last line
\ No newline at end of file
+last line
+added one
+added three
\ No newline at end of file
```

## 6.8: No-Newline in Middle of Changes

**Purpose**: Verify handling when no-newline appears between other changes.

**Input Diff**:
```
  +5:  early addition

  -20: middle line
  +20: middle line
  +21: after middle

  +30: late addition
```

**Command**: `git-stager stage file.txt:5,21,30`

**Expected Patch**:
```diff
@@ -4,0 +5 @@
+early addition
@@ -20 +21,2 @@
-middle line
\ No newline at end of file
+middle line
+after middle
@@ -29,0 +32 @@
+late addition
```

## Implementation Requirements

### Critical Git Invariants

1. **No-Newline Marker**:
   - Syntax: `\ No newline at end of file`
   - Appears immediately after the line it refers to
   - Is NOT counted in hunk header line counts
   - Is metadata, not content

2. **Marker Placement Rules**:
   - After `-` line: Indicates old version had no newline
   - After `+` line: Indicates new version has no newline
   - Can appear twice in same hunk (both old and new lack newline)

3. **Bridge Synthesis**:
   - When staging additions after no-newline line
   - Must include `-N` (remove no-newline version)
   - Must include `+N` (add version with newline)
   - Cannot add lines after no-newline without fixing it

4. **Auto-Bridge Triggers**:
   - Staging any `+M` where M > N and N lacks newline
   - Tool must detect and include bridge automatically
   - User doesn't need to know about bridge requirement

5. **Content Preservation**:
   - Line content before marker must be exact
   - No trailing whitespace after content before marker
   - Marker must be on its own line in patch

6. **Special Position Calculations**:
   - No-newline doesn't affect line numbering
   - Marker doesn't count in old_count or new_count
   - Position calculations ignore marker lines

7. **Common Patterns**:
   ```diff
   # Adding newline only:
   -content\ No newline at end of file
   +content

   # Keeping no-newline:
   -old\ No newline at end of file
   +new\ No newline at end of file

   # Removing no-newline line:
   -content\ No newline at end of file
   ```

8. **Edge Cases**:
   - Empty file to no-newline: Special handling
   - Multiple consecutive no-newline changes: Each handled independently
   - No-newline at specific line positions: Works at any position

### Bridge Synthesis Algorithm

When staging line `+N`:
1. Check if line `N-1` exists in diff as `-X` without newline
2. If yes, must include:
   - `-X` (remove no-newline version)
   - `+X` (add version with newline)
   - All lines from `+X` to `+N` inclusive
3. If line `N-1` already has newline, no bridge needed

### Validation Checklist

Before generating a no-newline patch, verify:
- [ ] Marker syntax exactly: `\ No newline at end of file`
- [ ] Marker not counted in hunk header counts
- [ ] Bridge synthesized when needed
- [ ] Marker placed immediately after relevant line
- [ ] No extra whitespace around marker
- [ ] Auto-bridge includes all required lines
- [ ] Position calculations ignore markers
- [ ] Content before marker preserved exactly