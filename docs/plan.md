# Git Line-Level Staging Tool - Planning Document

## Purpose

Provide Claude with the ability to stage semantically related changes at **line granularity**, bypassing Git's hunk-based limitations where adjacent but unrelated changes get merged together.

## Workflow

1. **Claude uses `git diff`** (or `git diff -U0`) to see changes
2. **Claude reasons about the diff** and identifies lines belonging to a logical commit
3. **Claude uses `git-stager`** to stage specific lines/ranges
4. **Verify with `git diff --cached`**
5. **Commit the staged changes**

The tool is a **staging mechanism**, not a diff viewer.

---

## Language Choice: Rust

### Why Rust?

1. **Type safety** - Patch construction requires careful handling; Rust prevents memory errors
2. **Excellent CLI ecosystem** - `clap` for argument parsing, `serde` for JSON
3. **Reliable error handling** - Explicit Result types for all edge cases
4. **Performance** - Fast execution with reasonable binary sizes
5. **Cross-platform** - Works consistently across Linux, macOS, Windows

---

## Command Interface

### Primary Command: `stage`

```bash
# Stage specific line (added line in new file)
git-stager stage flake.nix:144

# Stage multiple lines
git-stager stage flake.nix:144,149

# Stage a range
git-stager stage flake.nix:40-42

# Stage range plus individual lines
git-stager stage flake.nix:40-42,149

# Cross-file staging (future optimization)
git-stager stage flake.nix:40-42 packages/default.nix:45-53
```

### Secondary Command: `list`

```bash
# List all stageable lines (for verification)
git-stager list flake.nix

# JSON output
git-stager list flake.nix --json
```

Output:
```json
{
  "file": "flake.nix",
  "lines": [
    {"number": 7, "type": "add", "content": "    determinate.url = \"...\";"},
    {"number": 144, "type": "add", "content": "      debug = true;"},
    {"number": 149, "type": "add", "content": "        ./flake-modules/home-manager.nix"}
  ]
}
```

---

## Core Architecture

### Data Flow

1. Parse `git diff -U0` output for the target file
2. Extract line-level changes (additions, deletions, modifications)
3. User specifies which lines to stage (by line number)
4. Construct a valid unified diff patch containing only those lines
5. Apply patch to index via `git apply --cached`

### Key Insight

Line numbers refer to the **new file version** (working tree), which is intuitive for the user looking at `git diff` output.

For deletions, we need to handle both:
- Old line numbers (what was removed)
- New line numbers (where it was removed from)

---

## Implementation Phases

### Phase 1: Core Diff Parsing
- Parse unified diff format
- Extract individual line changes
- Map new file line numbers to changes
- Handle additions, deletions, and context

### Phase 2: Patch Construction
- Given selected line numbers, construct valid patch
- Ensure proper hunk headers (`@@ -old,count +new,count @@`)
- Add minimal context for patch application
- Handle adjacent changes correctly

### Phase 3: Staging via git apply
- Write patch to temp file
- Execute `git apply --cached <patch>`
- Handle errors (patch doesn't apply, conflicts)
- Clean up temp files

### Phase 4: Validation & Edge Cases
- Verify staged changes match expectations
- Handle file renames
- Handle binary files (reject gracefully)
- Handle deleted files
- Handle new files

### Phase 5: Cross-File Staging (Optimization)
- Allow staging lines from multiple files in one command
- Construct multi-file patch
- Atomic application (all or nothing)

---

## Technical Challenges

### 1. Patch Context
`git apply` needs context lines to locate where to apply changes. With `-U0` diffs, there's no context. Solutions:
- Re-diff with context just for the selected lines
- Trust line numbers and let git figure it out
- Use `--unidiff-zero` flag with `git apply`

### 2. Line Number Mapping
When staging partial changes, line numbers in the patch must be adjusted. If we stage lines 144 and 149 but not 145-148, the patch needs correct offsets.

### 3. Deletions
Deleted lines don't have a "new line number". Options:
- Refer to them by old line number with special syntax: `flake.nix:-45`
- Refer to position in new file: "line 44 had deletion"
- Show context in list output

### 4. Modifications
A "modified" line is actually a deletion + addition. Need to handle as atomic unit or allow staging just the add/delete.

---

## Project Structure

```
git-stager/
├── Cargo.toml
├── src/
│   ├── main.rs           # CLI entry point
│   ├── lib.rs            # Public API
│   ├── diff/
│   │   ├── mod.rs
│   │   ├── parser.rs     # Parse unified diff
│   │   └── types.rs      # LineChange, FileDiff structs
│   ├── patch/
│   │   ├── mod.rs
│   │   ├── builder.rs    # Construct patches from selected lines
│   │   └── apply.rs      # git apply --cached wrapper
│   └── cli.rs            # Argument parsing
└── tests/
    ├── integration_test.rs
    ├── fixtures/         # Programmatic repo builders
    └── snapshots/        # insta snapshots
```

---

## Success Criteria

1. **Stage individual lines** from a file with multiple unrelated changes
2. **Construct valid patches** that `git apply --cached` accepts
3. **Handle edge cases** gracefully (binary files, renames, conflicts)
4. **Clear error messages** when operations fail
5. **Fast execution** - should feel instantaneous
6. **Correct behavior** verified by comprehensive test suite

---

## Example Session

```bash
# Claude examines changes
$ git diff -U0 flake.nix
@@ -136,0 +137 @@
+      debug = true;
@@ -140,0 +142 @@
+        ./flake-modules/home-manager.nix

# Claude stages only the home-manager import
$ git-stager stage flake.nix:142

# Claude verifies
$ git diff --cached flake.nix
@@ -140,0 +142 @@
+        ./flake-modules/home-manager.nix

# Claude commits this logical change
$ git commit -m "feat: add home-manager flake module"

# Then stages the debug flag separately
$ git-stager stage flake.nix:137
$ git commit -m "chore: enable flake-parts debug mode"
```

---

## Open Questions

1. **Syntax for deletions** - How to refer to deleted lines by number?
2. **Atomic modifications** - Force staging both add+delete for modified lines, or allow partial?
3. **Context lines** - How many context lines needed for reliable patching?
4. **Error recovery** - What if patch partially applies?
