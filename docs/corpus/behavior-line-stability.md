# Behavioral Test: Line Number Stability

This document specifies the critical behavior that line numbers remain stable after partial staging.

## Purpose

Verify that `git-stager diff` line numbers remain valid after partial staging operations, enabling sequential staging workflows.

## Test Scenario

**Initial Diff**:
```
config.nix:
  +3:  # FIRST INSERTION

  +10: # SECOND INSERTION
```

## Sequential Staging Test

### Step 1: Stage Later Hunk First
**Command**: `git-stager stage config.nix:10`

**Result After Step 1**:
```bash
$ git diff --cached config.nix
@@ -9,0 +10 @@
+# SECOND INSERTION

$ git-stager diff config.nix
config.nix:
  +3:  # FIRST INSERTION
```

### Step 2: Stage Earlier Hunk
**Command**: `git-stager stage config.nix:3`

**Result After Step 2**:
```bash
$ git diff --cached config.nix
@@ -2,0 +3 @@
+# FIRST INSERTION
@@ -9,0 +11 @@
+# SECOND INSERTION
```

## Critical Invariant

**Line numbers in `git-stager diff` must remain stable**: After staging line 10, line 3 is still referenced as line 3, not adjusted for the prior staging.

## Why This Matters

1. **Enables AI workflows**: AI assistants can stage changes based on semantic grouping without recalculating line numbers
2. **User-friendly**: Users don't need to mentally adjust line numbers after each staging operation
3. **Composable operations**: Multiple staging commands can be issued based on initial diff output
4. **Order independence**: Final result is the same regardless of staging order

## Implementation Requirement

The diff parser must always work from the current working directory state, not from accumulated staging state. Each `git-stager diff` call should produce line numbers that are valid for the next `git-stager stage` command.

## Related Tests

- See `verify-invariants.sh` test: "Staging order commutativity"
- See corpus test 4.6: "Order Independence" (for single command with multiple hunks)