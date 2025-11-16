# Git Hunk Staging Tool - Planning Document

## Language Choice: Rust

After careful consideration, I recommend **Rust** for the following reasons:

### Why Rust?

1. **git2-rs library** - Mature, well-maintained bindings to libgit2 providing direct access to Git internals (diffs, hunks, staging operations)

2. **Type safety for Git operations** - Rust's ownership model ensures we handle Git state transitions correctly (important when modifying the index)

3. **Excellent CLI ecosystem** - `clap` for argument parsing, `serde` for JSON output, `colored` for terminal output

4. **Reliable error handling** - Explicit Result types ensure we handle all edge cases (binary files, conflicts, permission issues)

5. **Performance and binary size** - Fast execution with reasonable binary sizes

6. **Cross-platform** - Works consistently across Linux, macOS, and Windows

---

## Architecture Plan

### Core Concepts

**Hunk Identification System:**
- Each hunk gets a unique ID: `<file_path>:<hunk_index>` (e.g., `src/main.rs:2`)
- Alternative: Short hash of hunk content for stability across runs
- Support both for flexibility

**Data Flow:**
1. Read Git repository state
2. Generate diff between HEAD/index and working tree
3. Parse diff into structured hunk objects
4. Present hunks with metadata
5. Apply selected hunks to index

---

## Command Structure

### Primary Commands

**1. `list` - Display available hunks**
- Show all unstaged hunks with IDs
- Display context (file, line numbers, change summary)
- Support JSON output for machine parsing
- Options for filtering by file pattern

**2. `stage` - Stage specific hunks**
- Accept hunk IDs as arguments
- Support ranges (`src/main.rs:1-3`)
- Support file-level staging (`src/main.rs:*`)
- Dry-run mode to preview what would be staged

**3. `show` - Display hunk content**
- Show full diff content for specific hunk(s)
- Syntax highlighting
- Context lines configuration

**4. `status` - Repository overview**
- Summary of files with pending changes
- Count of hunks per file
- Quick reference for planning

---

## Implementation Phases

### Phase 1: Core Foundation
- Repository detection and validation
- Diff generation using libgit2
- Hunk parsing and identification
- Basic data structures for hunks

### Phase 2: List Command
- Parse all unstaged changes
- Generate unique hunk IDs
- Format output (human-readable and JSON)
- Display hunk metadata (file, lines, type of change)

### Phase 3: Stage Command
- Parse hunk ID arguments
- Validate hunk existence
- Apply hunks to Git index using libgit2
- Error handling for conflicts/failures

### Phase 4: Show Command
- Retrieve specific hunk content
- Format with line numbers and context
- Optional syntax highlighting

### Phase 5: Polish & Edge Cases
- Binary file handling
- File rename/delete operations
- Empty hunks
- Permission denied scenarios
- Large file handling
- Submodule awareness

---

## Output Format Design

### Human-Readable (default)
```
File: src/lib.rs
  [src/lib.rs:0] Lines 10-15 (+3/-1) - Add error handling
  [src/lib.rs:1] Lines 45-52 (+8/-0) - New helper function

File: tests/test_main.rs
  [tests/test_main.rs:0] Lines 5-12 (+7/-2) - Update test cases
```

### JSON (for programmatic use)
```json
{
  "hunks": [
    {
      "id": "src/lib.rs:0",
      "file": "src/lib.rs",
      "old_start": 10,
      "new_start": 10,
      "lines_added": 3,
      "lines_removed": 1,
      "header": "@@ -10,4 +10,6 @@"
    }
  ]
}
```

---

## Key Technical Challenges

1. **Hunk application** - libgit2's `apply_to_index` needs careful handling of partial patches

2. **Hunk boundary detection** - Correctly splitting diffs at hunk boundaries

3. **Index manipulation** - Safely modifying Git's staging area without corruption

4. **Context sensitivity** - Hunks may depend on context that conflicts with other changes

5. **Performance** - Large repositories with many changes need efficient parsing

---

## Usage Workflow (for Claude)

1. Run `git-stager list --json` to see all available hunks
2. Analyze which hunks are relevant to the task
3. Run `git-stager stage <hunk-ids>` to stage specific changes
4. Verify with `git diff --cached`
5. Commit the staged changes

---

## Project Structure

```
git-stager/
├── Cargo.toml
├── src/
│   ├── main.rs           # CLI entry point
│   ├── lib.rs            # Core library
│   ├── commands/
│   │   ├── mod.rs
│   │   ├── list.rs       # List command
│   │   ├── stage.rs      # Stage command
│   │   └── show.rs       # Show command
│   ├── git/
│   │   ├── mod.rs
│   │   ├── diff.rs       # Diff parsing
│   │   ├── hunk.rs       # Hunk structures
│   │   └── index.rs      # Index manipulation
│   └── output/
│       ├── mod.rs
│       ├── json.rs       # JSON formatting
│       └── human.rs      # Human-readable formatting
└── tests/
    └── integration/      # End-to-end tests
```

---

## Success Criteria

1. Reliably list all unstaged hunks with stable IDs
2. Successfully stage individual hunks without affecting others
3. Handle all common edge cases gracefully
4. JSON output parseable for automated workflows
5. Fast enough for repositories with hundreds of changed files
6. Clear error messages when operations fail
