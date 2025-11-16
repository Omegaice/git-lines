# Git Hunk Staging - Test Cases

## Structural Changes

### File Operations
- File rename with content changes
- File rename without content changes
- File move to different directory
- File copy detection
- New file (untracked to staged)
- Deleted file (tracked to removed)
- File mode change only (e.g., adding executable permission)
- File mode change combined with content changes
- Symlink creation
- Symlink modification
- Symlink deletion

### Binary Files
- Binary file modification
- Binary file addition
- Binary file deletion
- Binary file that git incorrectly detects as text
- Text file that git incorrectly detects as binary
- Mixed binary and text changes in same commit

## Content Edge Cases

### Hunk Dependencies
- Overlapping context lines between adjacent hunks
- Non-adjacent hunks in same file
- Staging hunks out of order
- Staging middle hunk without first hunk
- Staging first and last hunk without middle
- Very long file with many small hunks
- Single line change creating minimal hunk
- Entire file replacement (one large hunk)

### Context Mismatches
- Working tree modified after diff generation
- File deleted after diff generation
- File created after diff generation
- Hunk context no longer matches due to external changes
- Concurrent modification by another process

### Whitespace Issues
- Whitespace-only changes
- Trailing whitespace additions
- Trailing whitespace removals
- Mixed tabs and spaces
- Line ending changes (LF to CRLF)
- Line ending changes (CRLF to LF)
- Mixed line endings in same file
- No newline at end of file (addition)
- No newline at end of file (removal)
- Blank line additions
- Blank line removals

### Encoding and Character Sets
- UTF-8 encoded files
- UTF-16 encoded files
- Latin-1 encoded files
- Files with BOM (Byte Order Mark)
- Multi-byte characters split across context boundary
- Non-printable characters in content
- Null bytes in text file
- Very long lines (>10000 characters)
- Files with mixed encodings

## Git-Specific Features

### Submodules
- Submodule pointer update
- Submodule addition
- Submodule removal
- Submodule with uncommitted changes
- Nested submodules

### Git LFS
- LFS pointer file modification
- LFS tracked file addition
- LFS tracked file removal
- Mixed LFS and regular files

### Git Attributes
- Files with custom diff drivers
- Files marked as binary in .gitattributes
- Files with text=auto attribute
- Files with specific encoding attributes
- Files with merge strategies defined

### Repository State
- Repository with merge in progress
- Repository with rebase in progress
- Repository with cherry-pick in progress
- Repository in detached HEAD state
- Bare repository (should fail gracefully)
- Shallow clone
- Sparse checkout
- Worktree (linked working tree)

## Error Conditions

### File System Issues
- Read-only file in working tree
- File permissions prevent reading
- File locked by another process
- Disk full during staging operation
- File path too long for OS
- Special characters in file path
- Unicode normalization differences in paths (macOS)

### Repository Issues
- Corrupted index file
- Missing objects in object database
- Invalid HEAD reference
- No commits in repository (initial commit scenario)
- Repository in inconsistent state

### Input Validation
- Invalid hunk ID format
- Non-existent hunk ID
- Hunk ID for already staged changes
- Duplicate hunk IDs in input
- Empty hunk ID list
- Malformed range syntax
- File pattern matching no files

## Performance Cases

### Scale Testing
- Repository with 1000+ modified files
- Single file with 100+ hunks
- Very large file (>100MB) with changes
- Many small files with single hunk each
- Deep directory nesting (>50 levels)
- Very large diff output (>10MB)

### Concurrency
- Multiple git-stager processes on same repo
- Git operations running concurrently
- Index lock contention
- Race conditions in hunk ID generation

## Output Validation

### JSON Output
- Valid JSON structure for empty results
- Valid JSON structure for single hunk
- Valid JSON structure for multiple hunks
- Special characters properly escaped in JSON
- Unicode properly encoded in JSON
- Large numbers handled correctly
- Nested file paths represented correctly

### Human-Readable Output
- Consistent formatting across different scenarios
- Proper alignment of columns
- Truncation of very long file paths
- Color output disabled when not TTY
- Progress indication for long operations

## Integration Scenarios

### Workflow Tests
- Stage single hunk from single file
- Stage multiple hunks from single file
- Stage hunks from multiple files
- Stage all hunks from one file, partial from another
- Unstage and re-stage same hunk
- Stage hunk, modify file, stage another hunk
- List hunks after partial staging
- Verify staged changes match intended hunks

### Compatibility
- Different git versions (2.20, 2.30, 2.40, latest)
- Case-sensitive vs case-insensitive file systems
- Different filesystem types (ext4, NTFS, APFS, etc.)
- Git configured with different diff algorithms
- Custom git configuration affecting diff output
