# Git Line-Level Staging - Test Cases

## Core Problem: No Non-Interactive Partial Staging

Git provides **no way to programmatically stage partial file changes**. The available tools are:

- `git add <file>` - Stages entire file (too coarse)
- `git add -p` - **Interactive only** (requires human input)
- `git add -e` - **Interactive only** (opens editor)
- `git apply --cached <patch>` - Non-interactive, but requires constructing a valid patch

**Claude's limitation:** Cannot respond to interactive prompts, cannot open editors, cannot use `git add -p`.

**This tool's purpose:** Provide a non-interactive interface to construct and apply partial patches, enabling Claude to stage specific lines programmatically.

---

## Primary Use Cases (What Claude Can't Do Without This Tool)

### Single File with Multiple Changes
Most common case - Claude made several changes to one file that should be separate commits:

- **Stage lines 10-15, ignore lines 20-25** - Select subset of changes
- **Stage one import, not another** - Different features need different imports
- **Stage function body change, not signature change** - Incremental refactoring
- **Stage new feature, not the debug code** - Clean up before commit

### Adjacent Line Changes
Changes next to each other that belong to different logical commits:

- **Two adjacent added lines, different features** - Line A adds feature X, line A+1 adds feature Y
- **Two adjacent deleted lines, different features** - Removing two unrelated things in sequence
- **Three or more adjacent additions** - Multiple features interleaved on consecutive lines
- **Adjacent addition and deletion** - One line deleted, immediately followed by unrelated addition
- **Modified line adjacent to unrelated change** - Line modified for feature X, next line added for feature Y

### Interleaved Semantic Changes
Pattern: A, B, A, B where A and B are different features:

- **Two features both touching same struct** - Field additions for different purposes
- **Error handling mixed with feature code** - try/catch for feature X, logging for feature Y
- **Documentation updates mixed with code changes** - Comment for X, code for Y, comment for Y
- **Refactoring mixed with new functionality** - Rename in line N, new code in line N+1

### Partial Modifications
A "modification" is delete+add; sometimes only one part belongs to a commit:

- **Rename within larger change** - Variable renamed (delete old, add new) but other changes unrelated
- **Formatting fix adjacent to logic change** - Indentation corrected on same line as feature change
- **Typo fix in modified line** - Original change plus typo correction should be separate commits

---

## Line Reference Scenarios

### Addition-Only Cases
- **Single line addition** - Stage one new line
- **Range of added lines** - Stage lines 10-15
- **Non-contiguous additions** - Stage lines 10, 15, 20 (skipping 11-14, 16-19)
- **First line of file added** - Edge case for line numbering
- **Last line of file added** - Edge case for patch construction
- **Addition in middle of existing hunk** - Select subset of an existing Git hunk

### Deletion-Only Cases
- **Single line deletion** - Remove one line
- **Range of deleted lines** - Remove lines 10-15
- **Non-contiguous deletions** - Remove lines 10, 15, 20
- **First line of file deleted** - Edge case
- **Last line of file deleted** - Edge case
- **All content deleted** - File becomes empty

### Mixed Operations
- **Stage addition, ignore adjacent deletion** - Within same Git hunk
- **Stage deletion, ignore adjacent addition** - Within same Git hunk
- **Multiple additions with one deletion between** - Select only the additions
- **Alternating add/delete pattern** - Stage only the adds, or only the deletes

---

## Patch Construction Challenges

### Line Number Accuracy
- **Staged line numbers after unstaged additions** - Line numbers shift
- **Staged line numbers after unstaged deletions** - Line numbers shift oppositely
- **Multiple unstaged changes before staged line** - Cumulative offset calculation
- **Staging affects subsequent line numbers** - Need to track as we stage

### Context Requirements
- **Zero context diff staging** - Use `--unidiff-zero` flag
- **Minimal context needed for git apply** - How much context ensures success
- **Context lines contain other changes** - Unstaged changes in context
- **Context at file boundaries** - Not enough lines above/below for full context

### Hunk Header Construction
- **Single line addition** - `@@ -N,0 +N,1 @@`
- **Single line deletion** - `@@ -N,1 +N,0 @@`
- **Range of additions** - Calculate correct counts
- **Non-contiguous selections** - Multiple hunks in constructed patch
- **Empty old or new section** - Header format edge cases

---

## Edge Cases for Line-Level Operations

### File State Transitions
- **New file, partial content staging** - File doesn't exist in index yet
- **Deleted file, partial restoration** - Remove some deletions, keep others
- **Renamed file with modifications** - Both rename and content changes
- **File mode change plus content** - Stage content only, not mode (or vice versa)

### Whitespace Sensitivity
- **Trailing whitespace on staged line** - Must preserve exactly
- **Line ending differences** - CRLF vs LF on specific lines
- **Tab vs space on staged line** - Exact character preservation
- **Empty line addition** - Line with only newline
- **Whitespace-only line modification** - Spaces changed to tabs

### Special Content
- **Line contains patch-like syntax** - `@@`, `+++`, `---` in actual code
- **Line starts with +, -, or space** - Could confuse naive parsers
- **Very long line (>10000 chars)** - Buffer handling
- **Line with null bytes** - Binary content in "text" file
- **Unicode combining characters** - Character boundary issues
- **Line contains only special chars** - `{}[]();,` etc.

---

## Error Conditions

### Invalid Input
- **Line number out of range** - Beyond file length
- **Line number references unchanged line** - Not a valid staging target
- **Line number references context line** - Not an actual change
- **Negative line number** - Invalid input
- **Zero line number** - Lines are 1-indexed
- **Non-numeric line reference** - Parse error
- **Range with start > end** - `flake.nix:50-40`
- **Overlapping ranges** - `flake.nix:10-20,15-25`
- **Duplicate line references** - `flake.nix:10,10`

### Git Apply Failures
- **Patch doesn't apply cleanly** - Context mismatch
- **File modified since diff** - Working tree changed
- **Index already has changes** - Conflict with staged content
- **Patch creates conflict markers** - Malformed patch
- **Permission denied on index** - Lock file issues

### State Validation
- **Line already staged** - Part of change already in index
- **File not in diff** - No changes to stage
- **Binary file** - Cannot do line-level staging
- **Submodule** - Not regular file content

---

## Integration with Git Workflow

### Sequential Staging
- **Stage line, verify, stage another line** - Multiple passes
- **Stage line, unstage, re-stage** - Undo and redo
- **Stage line, then stage adjacent line** - Building up commit
- **Stage lines, realize mistake, reset** - Error recovery

### Verification Steps
- **`git diff --cached` shows only staged lines** - Correct staging
- **`git diff` shows remaining unstaged lines** - Nothing lost
- **Staged patch applies cleanly to HEAD** - Valid patch
- **Line numbers in output match working tree** - Consistent reference

### Commit Workflow
- **Stage feature A lines, commit** - First commit
- **Stage feature B lines, commit** - Second commit
- **Verify both commits have correct content** - Clean separation
- **History shows logical atomic commits** - Goal achieved

---

## Real-World Scenarios (from nixpkgs example)

### Config File Refactoring
```nix
# Single hunk contains:
_module.args.local = {           # Structural refactor
  yaziPlugins = ...              # Structural refactor
  mkFHSSandboxExec = ...;        # NEW FEATURE
};
```
**Test:** Stage only `mkFHSSandboxExec` line, or only structural refactor lines

### Adjacent Feature Changes
```nix
style = "full";                  # Config improvement
# Theme managed by Stylix        # Stylix feature
```
**Test:** Stage config improvement without Stylix comment

### Unrelated Changes in Same Block
```nix
"terminal.integrated.fontFamily" = lib.mkDefault...  # Stylix
"direnv.restart.automatic" = true;                   # Direnv (UNRELATED)
```
**Test:** Stage direnv config without Stylix changes

### Flake Input Additions
```nix
debug = true;                              # Debug mode
imports = [
  ./flake-modules/home-manager.nix         # HM module
```
**Test:** Stage home-manager import, not debug flag (or vice versa)

---

## Performance Considerations

### Scale
- **File with 1000+ line changes** - Parse and select efficiently
- **100+ lines selected for staging** - Patch construction performance
- **Very large file (>1MB)** - Memory usage
- **Deep directory path** - Path handling

### Repeated Operations
- **Stage 100 lines one at a time** - Shouldn't degrade
- **List command called frequently** - Fast response
- **Large diff parsed repeatedly** - Consider caching

---

## Output Validation (for list command)

### JSON Format
- **Empty changes** - Valid empty array
- **Single line change** - Correct structure
- **Multiple changes** - Array of objects
- **Deletion representation** - How to show removed lines
- **Line content escaping** - Special chars in JSON strings
- **File path escaping** - Paths with spaces, quotes

### Human Readable
- **Clear line number display** - Easy to reference
- **Change type indicator** - Add/delete/modify
- **Content preview** - Enough to identify the line
- **Grouped by contiguity** - Show natural groupings
