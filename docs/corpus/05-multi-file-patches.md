# Multi-File Patches

This document specifies patches that span multiple files in a single operation.

## 5.1: Two Files Simple Addition

**Purpose**: Verify basic multi-file patch generation.

**Input Diff**:
```
flake.nix:
  +137:    debug = true;

config.nix:
  +42:     feature.enable = true;
```

**Command**: `git-lines stage flake.nix:137 config.nix:42`

**Expected Patch**:
```diff
diff --git a/flake.nix b/flake.nix
--- a/flake.nix
+++ b/flake.nix
@@ -136,0 +137 @@
+    debug = true;
diff --git a/config.nix b/config.nix
--- a/config.nix
+++ b/config.nix
@@ -41,0 +42 @@
+    feature.enable = true;
```

## 5.2: Mixed Operations Across Files

**Purpose**: Verify different operation types in different files.

**Input Diff**:
```
src/main.js:
  +10:     new_function();

src/utils.js:
  -25:     deprecated_helper();

src/config.js:
  -5:      OLD_VERSION = "1.0";
  +5:      NEW_VERSION = "2.0";
```

**Command**: `git-lines stage src/main.js:10 src/utils.js:-25 src/config.js:-5,5`

**Expected Patch**:
```diff
diff --git a/src/main.js b/src/main.js
--- a/src/main.js
+++ b/src/main.js
@@ -9,0 +10 @@
+    new_function();
diff --git a/src/utils.js b/src/utils.js
--- a/src/utils.js
+++ b/src/utils.js
@@ -25 +24,0 @@
-    deprecated_helper();
diff --git a/src/config.js b/src/config.js
--- a/src/config.js
+++ b/src/config.js
@@ -5 +5 @@
-OLD_VERSION = "1.0";
+NEW_VERSION = "2.0";
```

## 5.3: Multiple Hunks in Multiple Files

**Purpose**: Verify complex multi-file, multi-hunk patches.

**Input Diff**:
```
lib/core.py:
  +10:     import new_module
  +50:     use_new_module()

lib/helpers.py:
  -5:      # Old header
  +100:    # New footer

tests/test_core.py:
  +20:     def test_new_feature():
  +21:         assert True
```

**Command**: `git-lines stage lib/core.py:10,50 lib/helpers.py:-5,100 tests/test_core.py:20,21`

**Expected Patch**:
```diff
diff --git a/lib/core.py b/lib/core.py
--- a/lib/core.py
+++ b/lib/core.py
@@ -9,0 +10 @@
+    import new_module
@@ -49,0 +51 @@
+    use_new_module()
diff --git a/lib/helpers.py b/lib/helpers.py
--- a/lib/helpers.py
+++ b/lib/helpers.py
@@ -5 +4,0 @@
-# Old header
@@ -99,0 +99 @@
+    # New footer
diff --git a/tests/test_core.py b/tests/test_core.py
--- a/tests/test_core.py
+++ b/tests/test_core.py
@@ -19,0 +20,2 @@
+    def test_new_feature():
+        assert True
```

## 5.4: Deep Directory Structure

**Purpose**: Verify path handling for nested directories.

**Input Diff**:
```
src/components/Header.jsx:
  +15:     <NewElement />

src/components/Footer.jsx:
  +30:     <Copyright year={2024} />

src/utils/helpers/format.js:
  -10:     oldFormat(data)
  +10:     newFormat(data)
```

**Command**: `git-lines stage src/components/Header.jsx:15 src/components/Footer.jsx:30 src/utils/helpers/format.js:-10,10`

**Expected Patch**:
```diff
diff --git a/src/components/Header.jsx b/src/components/Header.jsx
--- a/src/components/Header.jsx
+++ b/src/components/Header.jsx
@@ -14,0 +15 @@
+    <NewElement />
diff --git a/src/components/Footer.jsx b/src/components/Footer.jsx
--- a/src/components/Footer.jsx
+++ b/src/components/Footer.jsx
@@ -29,0 +30 @@
+    <Copyright year={2024} />
diff --git a/src/utils/helpers/format.js b/src/utils/helpers/format.js
--- a/src/utils/helpers/format.js
+++ b/src/utils/helpers/format.js
@@ -10 +10 @@
-    oldFormat(data)
+    newFormat(data)
```

## 5.5: Many Files

**Purpose**: Verify handling of many files in single command.

**Input Diff**:
```
file1.txt: +1:  change1
file2.txt: +2:  change2
file3.txt: +3:  change3
file4.txt: +4:  change4
file5.txt: +5:  change5
```

**Command**: `git-lines stage file1.txt:1 file2.txt:2 file3.txt:3 file4.txt:4 file5.txt:5`

**Expected Patch**:
```diff
diff --git a/file1.txt b/file1.txt
--- a/file1.txt
+++ b/file1.txt
@@ -0,0 +1 @@
+change1
diff --git a/file2.txt b/file2.txt
--- a/file2.txt
+++ b/file2.txt
@@ -1,0 +2 @@
+change2
diff --git a/file3.txt b/file3.txt
--- a/file3.txt
+++ b/file3.txt
@@ -2,0 +3 @@
+change3
diff --git a/file4.txt b/file4.txt
--- a/file4.txt
+++ b/file4.txt
@@ -3,0 +4 @@
+change4
diff --git a/file5.txt b/file5.txt
--- a/file5.txt
+++ b/file5.txt
@@ -4,0 +5 @@
+change5
```

## Implementation Requirements

### Critical Git Invariants

1. **File Ordering**:
   - Files appear in the patch in the order git sorts them
   - Typically alphabetical by path
   - Not necessarily the order specified in the command

2. **Patch Structure**:
   - Each file section starts with `diff --git a/path b/path`
   - Followed by `--- a/path` and `+++ b/path` headers
   - Then one or more hunks with `@@` headers
   - No blank lines between file sections

3. **File Header Format**:
   ```diff
   diff --git a/filepath b/filepath
   --- a/filepath
   +++ b/filepath
   ```
   - The `diff --git` line is required for multi-file patches
   - Paths are relative to repository root
   - No leading `/` in paths after `a/` and `b/`

4. **Independent Position Tracking**:
   - Each file's hunks are independent
   - Position adjustments don't carry across files
   - Each file starts fresh with its own line numbering

5. **Command Syntax**:
   - Multiple file:ref arguments separated by spaces
   - Each file can have multiple line references
   - Order of file arguments doesn't affect output order

6. **Path Handling**:
   - Paths must be relative to repository root
   - Forward slashes (`/`) used regardless of OS
   - Paths with spaces must be quoted in command
   - Special characters in paths must be escaped

7. **Atomicity**:
   - All file changes staged together or none
   - If any file fails validation, entire operation fails
   - Partial staging across files not supported

8. **Performance**:
   - No practical limit on number of files
   - Each file processed independently
   - Memory usage proportional to largest single file

### Special Cases

1. **New File Creation**:
   - Shows as `/dev/null` in `--- a/` line for new files
   - Full file content appears as additions
   - Example: `--- /dev/null` and `+++ b/newfile.txt`

2. **File Deletion**:
   - Shows as `/dev/null` in `+++ b/` line for deletions
   - Full file content appears as deletions
   - Example: `--- a/oldfile.txt` and `+++ /dev/null`

3. **File Rename with Changes**:
   - Not supported by line-level staging
   - Use regular git commands for renames

### Validation Checklist

Before generating a multi-file patch, verify:
- [ ] Each file has complete header block
- [ ] Files ordered correctly (typically alphabetical)
- [ ] Path format is correct (relative, forward slashes)
- [ ] Each file's hunks are independently valid
- [ ] No position adjustment across file boundaries
- [ ] All file:ref arguments parsed correctly
- [ ] Special characters in paths handled properly
- [ ] Atomic operation - all or nothing