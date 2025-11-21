# Git-Stager Corpus Documentation

This directory contains the complete specification for git-stager's patch generation capabilities.

## Structure

The corpus is organized by patch type, with each document specifying all valid ways to generate that type of patch:

1. **[01-addition-patches.md](01-addition-patches.md)** - Pure addition patches
2. **[02-deletion-patches.md](02-deletion-patches.md)** - Pure deletion patches
3. **[03-replacement-patches.md](03-replacement-patches.md)** - Replacement patches (delete + add at same location)
4. **[04-multi-hunk-patches.md](04-multi-hunk-patches.md)** - Multiple hunks within a single file
5. **[05-multi-file-patches.md](05-multi-file-patches.md)** - Patches spanning multiple files
6. **[06-no-newline-patches.md](06-no-newline-patches.md)** - Edge cases for files without trailing newlines

## Purpose

Each document serves as both:
- **Specification** - Defines expected behavior for implementers
- **Test Plan** - Each case maps directly to a test case
- **Reference** - Shows exact patch format git requires

## Format

Each test case includes:
- **Purpose** - What aspect is being tested
- **Input Diff** - The diff format git-stager receives
- **Command** - The exact git-stager command to run
- **Expected Patch** - The exact patch that must be generated

Each document concludes with:
- **Implementation Requirements** - Critical git invariants and formulas
- **Validation Checklist** - Quick reference for correctness

## Usage

For implementers:
1. Start with document 01 and implement each case
2. Use Implementation Requirements for technical details
3. Validate against Expected Patch output
4. Run verify-invariants.sh to confirm git compatibility

For test writers:
1. Each case should become an automated test
2. Use snapshot testing to verify Expected Patch
3. Test both the git-stager output and git apply success

## Related Files

- `../../scripts/verify-invariants.sh` - Verifies git diff/apply behavior
- `../../tests/e2e_test.rs` - End-to-end tests implementing these cases