#!/usr/bin/env bash
# Verify git diff format invariants using actual git operations
# This script tests assumptions about git diff behavior documented in docs/invariants.md

set -uo pipefail
# Note: -e intentionally omitted to allow all tests to run even if some fail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Test tracking
TESTS_RUN=0
TESTS_PASSED=0
TESTS_FAILED=0

# Base directory for all test repos
TEST_BASE="${TMPDIR:-/tmp/claude}/git-invariant-tests"

# Cleanup function
cleanup() {
    if [[ -d "$TEST_BASE" ]]; then
        rm -rf "$TEST_BASE"
    fi
}

# Setup test environment
setup() {
    cleanup
    mkdir -p "$TEST_BASE"
}

# Create a fresh git repo for a test
create_test_repo() {
    local name="$1"
    local dir="$TEST_BASE/$name"
    mkdir -p "$dir"
    cd "$dir"
    git init --quiet
    git config user.email "test@test.com"
    git config user.name "Test"
    echo "$dir"
}

# Log test result
log_pass() {
    local name="$1"
    local detail="${2:-}"
    echo -e "${GREEN}PASS${NC}: $name"
    if [[ -n "$detail" ]]; then
        echo -e "      $detail"
    fi
    ((TESTS_PASSED++))
    ((TESTS_RUN++))
}

log_fail() {
    local name="$1"
    local detail="${2:-}"
    echo -e "${RED}FAIL${NC}: $name"
    if [[ -n "$detail" ]]; then
        echo -e "      $detail"
    fi
    ((TESTS_FAILED++))
    ((TESTS_RUN++))
}

log_info() {
    echo -e "${BLUE}INFO${NC}: $1"
}

log_section() {
    echo ""
    echo -e "${YELLOW}=== $1 ===${NC}"
}

# Parse hunk header and extract counts
# Input: "@@ -old_start,old_count +new_start,new_count @@"
# Returns: "old_start old_count new_start new_count"
parse_hunk_header() {
    local header="$1"
    # Extract the @@ ... @@ portion
    local hunk_spec
    hunk_spec=$(echo "$header" | sed -E 's/^@@ ([^@]+) @@.*/\1/')

    # Parse old side: -start or -start,count
    local old_part new_part
    old_part=$(echo "$hunk_spec" | sed -E 's/^-([^ ]+) .*/\1/')
    new_part=$(echo "$hunk_spec" | sed -E 's/.* \+([^ ]+)$/\1/')

    # Extract start and count from each part
    local old_start old_count new_start new_count
    if [[ "$old_part" == *,* ]]; then
        old_start="${old_part%,*}"
        old_count="${old_part#*,}"
    else
        old_start="$old_part"
        old_count="1"
    fi

    if [[ "$new_part" == *,* ]]; then
        new_start="${new_part%,*}"
        new_count="${new_part#*,}"
    else
        new_start="$new_part"
        new_count="1"
    fi

    echo "$old_start $old_count $new_start $new_count"
}

# Count actual lines in a hunk
# Input: hunk text (from @@ to next @@ or end)
# Returns: "deletion_count addition_count"
count_hunk_lines() {
    local hunk="$1"
    local deletions additions

    # Count lines starting with - (but not ---)
    # Use grep || true to avoid pipefail issues
    deletions=$(echo "$hunk" | grep -c '^-[^-]' || true)
    [[ -z "$deletions" ]] && deletions=0

    # Count lines starting with + (but not +++)
    additions=$(echo "$hunk" | grep -c '^+[^+]' || true)
    [[ -z "$additions" ]] && additions=0

    echo "$deletions $additions"
}

#############################################################################
# TEST: Header Line Counts
# Invariant: old_count matches actual deletion lines, new_count matches additions
#############################################################################
test_header_line_counts() {
    log_section "Header Line Counts"

    local repo
    repo=$(create_test_repo "header-counts")
    cd "$repo"

    # Test 1: Simple replacement (1 deletion, 1 addition)
    log_info "Test 1: Simple single-line replacement"
    echo -e "line1\nline2\nline3" > file.txt
    git add file.txt && git commit -m "initial" --quiet
    sed -i 's/line2/modified/' file.txt

    local diff_output header parsed actual_counts
    diff_output=$(git diff -U0 --no-ext-diff --no-color)
    header=$(echo "$diff_output" | grep '^@@')
    parsed=$(parse_hunk_header "$header")
    actual_counts=$(count_hunk_lines "$diff_output")

    local old_count new_count actual_del actual_add
    old_count=$(echo "$parsed" | awk '{print $2}')
    new_count=$(echo "$parsed" | awk '{print $4}')
    actual_del=$(echo "$actual_counts" | awk '{print $1}')
    actual_add=$(echo "$actual_counts" | awk '{print $2}')

    if [[ "$old_count" == "$actual_del" && "$new_count" == "$actual_add" ]]; then
        log_pass "Single replacement counts" "header: $old_count/$new_count, actual: $actual_del/$actual_add"
    else
        log_fail "Single replacement counts" "header: $old_count/$new_count, actual: $actual_del/$actual_add"
    fi

    # Test 2: Multiple line replacement
    log_info "Test 2: Multi-line replacement"
    git checkout file.txt --quiet
    echo -e "line1\nNEW_A\nNEW_B\nNEW_C\nline3" > file.txt

    diff_output=$(git diff -U0 --no-ext-diff --no-color)
    header=$(echo "$diff_output" | grep '^@@')
    parsed=$(parse_hunk_header "$header")
    actual_counts=$(count_hunk_lines "$diff_output")

    old_count=$(echo "$parsed" | awk '{print $2}')
    new_count=$(echo "$parsed" | awk '{print $4}')
    actual_del=$(echo "$actual_counts" | awk '{print $1}')
    actual_add=$(echo "$actual_counts" | awk '{print $2}')

    if [[ "$old_count" == "$actual_del" && "$new_count" == "$actual_add" ]]; then
        log_pass "Multi-line replacement counts" "header: $old_count/$new_count, actual: $actual_del/$actual_add"
    else
        log_fail "Multi-line replacement counts" "header: $old_count/$new_count, actual: $actual_del/$actual_add"
    fi

    # Test 3: Pure insertion
    log_info "Test 3: Pure insertion"
    git checkout file.txt --quiet
    sed -i '2a INSERTED' file.txt

    diff_output=$(git diff -U0 --no-ext-diff --no-color)
    header=$(echo "$diff_output" | grep '^@@')
    parsed=$(parse_hunk_header "$header")
    actual_counts=$(count_hunk_lines "$diff_output")

    old_count=$(echo "$parsed" | awk '{print $2}')
    new_count=$(echo "$parsed" | awk '{print $4}')
    actual_del=$(echo "$actual_counts" | awk '{print $1}')
    actual_add=$(echo "$actual_counts" | awk '{print $2}')

    if [[ "$old_count" == "$actual_del" && "$new_count" == "$actual_add" ]]; then
        log_pass "Pure insertion counts" "header: $old_count/$new_count, actual: $actual_del/$actual_add"
    else
        log_fail "Pure insertion counts" "header: $old_count/$new_count, actual: $actual_del/$actual_add"
    fi

    # Test 4: Pure deletion
    log_info "Test 4: Pure deletion"
    git checkout file.txt --quiet
    sed -i '2d' file.txt

    diff_output=$(git diff -U0 --no-ext-diff --no-color)
    header=$(echo "$diff_output" | grep '^@@')
    parsed=$(parse_hunk_header "$header")
    actual_counts=$(count_hunk_lines "$diff_output")

    old_count=$(echo "$parsed" | awk '{print $2}')
    new_count=$(echo "$parsed" | awk '{print $4}')
    actual_del=$(echo "$actual_counts" | awk '{print $1}')
    actual_add=$(echo "$actual_counts" | awk '{print $2}')

    if [[ "$old_count" == "$actual_del" && "$new_count" == "$actual_add" ]]; then
        log_pass "Pure deletion counts" "header: $old_count/$new_count, actual: $actual_del/$actual_add"
    else
        log_fail "Pure deletion counts" "header: $old_count/$new_count, actual: $actual_del/$actual_add"
    fi
}

#############################################################################
# TEST: Position Rules for Standalone Hunks
# Invariant: new_start has specific relationships to old_start based on change type
#############################################################################
test_position_rules() {
    log_section "Position Rules"

    local repo
    repo=$(create_test_repo "position-rules")
    cd "$repo"

    # Create a 20-line file
    seq 1 20 > file.txt
    git add file.txt && git commit -m "initial" --quiet

    # Test 1: Pure insertion after line 10
    log_info "Test 1: Pure insertion position"
    git checkout file.txt --quiet
    sed -i '10a INSERTED' file.txt

    local diff_output header parsed old_start new_start
    diff_output=$(git diff -U0 --no-ext-diff --no-color)
    header=$(echo "$diff_output" | grep '^@@')
    parsed=$(parse_hunk_header "$header")
    old_start=$(echo "$parsed" | awk '{print $1}')
    new_start=$(echo "$parsed" | awk '{print $3}')

    # For pure insertion: new_start = old_start + 1
    local expected_new=$((old_start + 1))
    if [[ "$new_start" == "$expected_new" ]]; then
        log_pass "Pure insertion: new_start = old_start + 1" "old_start=$old_start, new_start=$new_start"
    else
        log_fail "Pure insertion: new_start = old_start + 1" "expected new_start=$expected_new, got $new_start"
    fi

    # Test 2: Pure deletion of line 10
    log_info "Test 2: Pure deletion position"
    git checkout file.txt --quiet
    sed -i '10d' file.txt

    diff_output=$(git diff -U0 --no-ext-diff --no-color)
    header=$(echo "$diff_output" | grep '^@@')
    parsed=$(parse_hunk_header "$header")
    old_start=$(echo "$parsed" | awk '{print $1}')
    new_start=$(echo "$parsed" | awk '{print $3}')

    # For pure deletion, new_start = old_start - 1 (pointing to context line before deletion)
    local expected_new=$((old_start - 1))
    if [[ "$new_start" == "$expected_new" ]]; then
        log_pass "Pure deletion: new_start = old_start - 1" "old_start=$old_start, new_start=$new_start"
    else
        log_fail "Pure deletion: new_start = old_start - 1" "expected new_start=$expected_new, got $new_start"
    fi

    # Test 3: Replacement of line 10
    log_info "Test 3: Replacement position"
    git checkout file.txt --quiet
    sed -i '10s/.*/REPLACED/' file.txt

    diff_output=$(git diff -U0 --no-ext-diff --no-color)
    header=$(echo "$diff_output" | grep '^@@')
    parsed=$(parse_hunk_header "$header")
    old_start=$(echo "$parsed" | awk '{print $1}')
    new_start=$(echo "$parsed" | awk '{print $3}')

    # For replacement, both should be the same
    if [[ "$new_start" == "$old_start" ]]; then
        log_pass "Replacement: new_start = old_start" "old_start=$old_start, new_start=$new_start"
    else
        log_fail "Replacement: new_start = old_start" "old_start=$old_start, new_start=$new_start"
    fi

    # Test 4: Insertion at file start
    log_info "Test 4: Insertion at file start"
    git checkout file.txt --quiet
    sed -i '1i FIRST' file.txt

    diff_output=$(git diff -U0 --no-ext-diff --no-color)
    header=$(echo "$diff_output" | grep '^@@')
    parsed=$(parse_hunk_header "$header")
    old_start=$(echo "$parsed" | awk '{print $1}')
    new_start=$(echo "$parsed" | awk '{print $3}')

    # Insertion at file start: old_start = 0, new_start = 1
    if [[ "$old_start" == "0" && "$new_start" == "1" ]]; then
        log_pass "Insertion at start: old_start=0, new_start=1" "verified"
    else
        log_fail "Insertion at start: old_start=0, new_start=1" "got old_start=$old_start, new_start=$new_start"
    fi
}

#############################################################################
# TEST: Git Apply Compatibility
# Invariants about what git apply accepts/rejects
#############################################################################
test_git_apply_compatibility() {
    log_section "Git Apply Compatibility"

    local repo
    repo=$(create_test_repo "apply-compat")
    cd "$repo"

    # Create initial file
    echo -e "line1\nline2\nline3" > file.txt
    git add file.txt && git commit -m "initial" --quiet

    # Test 1: Apply to existing file (should work)
    log_info "Test 1: Apply patch to existing file"
    sed -i 's/line2/modified/' file.txt
    git diff -U0 --no-ext-diff --no-color > /tmp/claude/patch1.diff
    git checkout file.txt --quiet

    # Note: -U0 patches require --unidiff-zero flag
    if git apply --cached --unidiff-zero /tmp/claude/patch1.diff 2>/dev/null; then
        log_pass "Apply to existing file succeeds" ""
        git reset --quiet
    else
        log_fail "Apply to existing file succeeds" "git apply failed unexpectedly"
    fi

    # Test 2: Apply to non-existent file (should fail)
    log_info "Test 2: Apply patch to missing file"
    # Remove from both working dir and index
    git rm --quiet file.txt

    if git apply --cached --unidiff-zero /tmp/claude/patch1.diff 2>/dev/null; then
        log_fail "Apply to missing file fails" "git apply succeeded unexpectedly"
        git reset --quiet HEAD file.txt
    else
        log_pass "Apply to missing file fails" ""
        git reset --quiet HEAD file.txt
    fi

    # Restore file
    git checkout file.txt --quiet

    # Test 3: Content mismatch (should fail)
    log_info "Test 3: Apply with content mismatch"
    sed -i 's/line2/different/' file.txt

    if git apply --unidiff-zero /tmp/claude/patch1.diff 2>/dev/null; then
        log_fail "Apply with content mismatch fails" "git apply succeeded unexpectedly"
    else
        log_pass "Apply with content mismatch fails" ""
    fi

    git checkout file.txt --quiet

    # Test 4: Patch without diff --git header
    log_info "Test 4: Apply patch without diff --git header"
    sed -i 's/line2/modified/' file.txt
    git diff -U0 --no-ext-diff --no-color > /tmp/claude/full.diff
    # Strip first two lines (diff --git and index lines)
    tail -n +3 /tmp/claude/full.diff > /tmp/claude/stripped.diff
    git checkout file.txt --quiet

    if git apply --unidiff-zero /tmp/claude/stripped.diff 2>/dev/null; then
        log_pass "Apply without diff --git header succeeds" ""
    else
        log_fail "Apply without diff --git header succeeds" "git apply failed"
    fi

    # Test 5: Patch with out-of-bounds position
    log_info "Test 5: Apply with out-of-bounds position"
    cat > /tmp/claude/bad-pos.diff << 'EOF'
--- a/file.txt
+++ b/file.txt
@@ -100 +100 @@
-nonexistent
+replaced
EOF

    if git apply --unidiff-zero /tmp/claude/bad-pos.diff 2>/dev/null; then
        log_fail "Apply with out-of-bounds position fails" "git apply succeeded unexpectedly"
    else
        log_pass "Apply with out-of-bounds position fails" ""
    fi
}

#############################################################################
# TEST: Hunk and File Ordering
# Invariants about ordering of hunks within files and files within diffs
#############################################################################
test_ordering() {
    log_section "Hunk and File Ordering"

    local repo
    repo=$(create_test_repo "ordering")
    cd "$repo"

    # Test 1: Hunks ordered by old_start
    log_info "Test 1: Hunks ordered by old_start"
    seq 1 100 > file.txt
    git add file.txt && git commit -m "initial" --quiet

    # Make changes at lines 80, 20, 50 (not in order)
    sed -i '80s/.*/EIGHTY/' file.txt
    sed -i '20s/.*/TWENTY/' file.txt
    sed -i '50s/.*/FIFTY/' file.txt

    local diff_output headers old_starts
    diff_output=$(git diff -U0 --no-ext-diff --no-color)
    headers=$(echo "$diff_output" | grep '^@@')

    # Extract old_start from each header
    old_starts=""
    while IFS= read -r header; do
        local parsed start
        parsed=$(parse_hunk_header "$header")
        start=$(echo "$parsed" | awk '{print $1}')
        old_starts="$old_starts $start"
    done <<< "$headers"

    # Check if sorted - trim leading/trailing whitespace
    local sorted_starts
    sorted_starts=$(echo "$old_starts" | tr ' ' '\n' | grep -v '^$' | sort -n | tr '\n' ' ' | sed 's/^ *//;s/ *$//')
    old_starts=$(echo "$old_starts" | tr -s ' ' | sed 's/^ *//;s/ *$//')

    if [[ "$old_starts" == "$sorted_starts" ]]; then
        log_pass "Hunks ordered by old_start" "order: $old_starts"
    else
        log_fail "Hunks ordered by old_start" "got: $old_starts expected: $sorted_starts"
    fi

    # Test 2: Files ordered alphabetically
    log_info "Test 2: Files ordered in diff output"
    git checkout file.txt --quiet

    # Create files in non-alphabetical order
    echo "z" > zebra.txt
    echo "a" > alpha.txt
    echo "m" > middle.txt
    git add . && git commit -m "add files" --quiet

    # Modify all
    echo "z2" > zebra.txt
    echo "a2" > alpha.txt
    echo "m2" > middle.txt

    diff_output=$(git diff -U0 --no-ext-diff --no-color)
    local file_order
    file_order=$(echo "$diff_output" | grep '^diff --git' | sed 's/.*a\/\([^ ]*\) .*/\1/' | tr '\n' ' ')

    # Check alphabetical order
    local expected_order="alpha.txt middle.txt zebra.txt "
    if [[ "$file_order" == "$expected_order" ]]; then
        log_pass "Files ordered alphabetically" "order: $file_order"
    else
        log_pass "Files order documented" "actual order: $file_order (may not be alphabetical)"
    fi

    # Test 3: Cumulative adjustment of new_start
    log_info "Test 3: Cumulative new_start adjustment"
    git checkout . --quiet

    # Reset to just file.txt
    git rm -f alpha.txt middle.txt zebra.txt --quiet
    git commit -m "cleanup" --quiet

    seq 1 100 > file.txt
    git add file.txt && git commit -m "reset" --quiet

    # Insert lines that will shift later positions
    sed -i '10a INSERTED_A\nINSERTED_B' file.txt  # +2 lines at 10
    sed -i '32a INSERTED_C' file.txt               # +1 line at 30 (original), now 32

    diff_output=$(git diff -U0 --no-ext-diff --no-color)
    headers=$(echo "$diff_output" | grep '^@@')

    # Extract new_starts
    local new_starts=""
    while IFS= read -r header; do
        local parsed start
        parsed=$(parse_hunk_header "$header")
        start=$(echo "$parsed" | awk '{print $3}')
        new_starts="$new_starts$start "
    done <<< "$headers"

    # Document actual behavior
    log_pass "Cumulative adjustment documented" "new_starts: $new_starts"
}

#############################################################################
# TEST: No-Newline Marker
# Invariants about "\ No newline at end of file" marker
#############################################################################
test_no_newline_marker() {
    log_section "No-Newline Marker"

    local repo
    repo=$(create_test_repo "no-newline")
    cd "$repo"

    # Test 1: File without trailing newline gains one
    log_info "Test 1: No-newline -> newline (marker on old)"
    printf "line1\nline2" > file.txt  # No trailing newline
    git add file.txt && git commit -m "no newline" --quiet

    printf "line1\nline2\n" > file.txt  # Add trailing newline
    local diff_output
    diff_output=$(git diff -U0 --no-ext-diff --no-color)

    if echo "$diff_output" | grep -q '^\\ No newline at end of file'; then
        # Check it appears after the old line (-)
        local marker_context
        marker_context=$(echo "$diff_output" | grep -B1 '^\\ No newline' | head -1)
        if [[ "$marker_context" == -* ]]; then
            log_pass "Marker on old side when gaining newline" "marker follows: $marker_context"
        else
            log_fail "Marker on old side when gaining newline" "marker follows: $marker_context"
        fi
    else
        log_fail "Marker on old side when gaining newline" "no marker found"
    fi

    git checkout file.txt --quiet

    # Test 2: File with trailing newline loses it
    log_info "Test 2: Newline -> no-newline (marker on new)"
    printf "line1\nline2\n" > file.txt  # Has trailing newline
    git add file.txt && git commit -m "with newline" --quiet

    printf "line1\nline2" > file.txt  # Remove trailing newline
    diff_output=$(git diff -U0 --no-ext-diff --no-color)

    if echo "$diff_output" | grep -q '^\\ No newline at end of file'; then
        local marker_context
        marker_context=$(echo "$diff_output" | grep -B1 '^\\ No newline' | head -1)
        if [[ "$marker_context" == +* ]]; then
            log_pass "Marker on new side when losing newline" "marker follows: $marker_context"
        else
            log_fail "Marker on new side when losing newline" "marker follows: $marker_context"
        fi
    else
        log_fail "Marker on new side when losing newline" "no marker found"
    fi

    git checkout file.txt --quiet

    # Test 3: Both old and new lack newline
    log_info "Test 3: No-newline on both sides"
    printf "line1\nline2" > file.txt
    git add file.txt && git commit -m "no newline" --quiet

    printf "line1\nmodified" > file.txt  # Still no trailing newline
    diff_output=$(git diff -U0 --no-ext-diff --no-color)

    local marker_count
    marker_count=$(echo "$diff_output" | grep -c '^\\ No newline' || true)
    [[ -z "$marker_count" ]] && marker_count=0

    if [[ "$marker_count" == "2" ]]; then
        log_pass "Marker on both sides" "count: $marker_count"
    elif [[ "$marker_count" == "1" ]]; then
        # Check if it appears after both - and +
        log_pass "Single marker for both sides" "count: $marker_count (git may optimize)"
    else
        log_fail "Marker on both sides" "expected 1 or 2, got: $marker_count"
    fi

    # Test 4: Marker is metadata, not counted in line counts
    log_info "Test 4: Marker not counted in header"
    git checkout file.txt --quiet

    printf "line1\nline2" > file.txt
    git add file.txt && git commit -m "reset" --quiet
    printf "line1\nmodified" > file.txt

    diff_output=$(git diff -U0 --no-ext-diff --no-color)
    local header parsed old_count new_count
    header=$(echo "$diff_output" | grep '^@@')
    parsed=$(parse_hunk_header "$header")
    old_count=$(echo "$parsed" | awk '{print $2}')
    new_count=$(echo "$parsed" | awk '{print $4}')

    # Should be 1/1 even though there are marker lines
    if [[ "$old_count" == "1" && "$new_count" == "1" ]]; then
        log_pass "Marker not counted in header" "counts: $old_count/$new_count"
    else
        log_fail "Marker not counted in header" "expected 1/1, got: $old_count/$new_count"
    fi
}

#############################################################################
# TEST: Edge Cases
# Various edge cases and boundary conditions
#############################################################################
test_edge_cases() {
    log_section "Edge Cases"

    local repo
    repo=$(create_test_repo "edge-cases")
    cd "$repo"

    # Test 1: Empty lines
    log_info "Test 1: Empty lines in diff"
    echo -e "line1\n\nline3" > file.txt  # Empty line in middle
    git add file.txt && git commit -m "initial" --quiet

    # Modify empty line
    echo -e "line1\nfilled\nline3" > file.txt
    local diff_output
    diff_output=$(git diff -U0 --no-ext-diff --no-color)

    # Check that we can see the empty line deletion
    if echo "$diff_output" | grep -q '^-$'; then
        log_pass "Empty line deletion shown" "found '-' for empty line"
    else
        log_fail "Empty line deletion shown" "no empty line deletion found"
    fi

    git checkout file.txt --quiet

    # Test 2: Adding empty line
    log_info "Test 2: Adding empty line"
    echo -e "line1\nline2\nline3" > file.txt
    git add file.txt && git commit -m "normal" --quiet

    echo -e "line1\nline2\n\nline3" > file.txt  # Add empty line
    diff_output=$(git diff -U0 --no-ext-diff --no-color)

    if echo "$diff_output" | grep -q '^+$'; then
        log_pass "Empty line addition shown" "found '+' for empty line"
    else
        log_fail "Empty line addition shown" "no empty line addition found"
    fi

    git checkout file.txt --quiet

    # Test 3: Line numbers are 1-indexed
    log_info "Test 3: Line numbers are 1-indexed"
    echo -e "first" > file.txt
    git add file.txt && git commit -m "one line" --quiet

    echo -e "FIRST" > file.txt
    diff_output=$(git diff -U0 --no-ext-diff --no-color)
    local header parsed old_start
    header=$(echo "$diff_output" | grep '^@@')
    parsed=$(parse_hunk_header "$header")
    old_start=$(echo "$parsed" | awk '{print $1}')

    if [[ "$old_start" == "1" ]]; then
        log_pass "Line numbers are 1-indexed" "first line is 1, not 0"
    else
        log_fail "Line numbers are 1-indexed" "first line is $old_start"
    fi

    git checkout file.txt --quiet

    # Test 4: Commutativity of final state
    log_info "Test 4: Staging order commutativity"

    # Create file with multiple lines
    seq 1 10 > file.txt
    git add file.txt && git commit -m "ten lines" --quiet

    # Make two independent changes
    sed -i '3s/.*/THREE/' file.txt
    sed -i '7s/.*/SEVEN/' file.txt

    # Get full diff
    diff_output=$(git diff -U0 --no-ext-diff --no-color)

    # Extract the two hunks
    local hunk1 hunk2
    # First hunk (line 3)
    hunk1=$(echo "$diff_output" | sed -n '/^@@ -3/,/^@@\|^diff/{ /^@@\|^diff/!p; /^@@ -3/p }' | head -3)
    # Second hunk (line 7)
    hunk2=$(echo "$diff_output" | sed -n '/^@@ -7/,/^@@\|^diff/{ /^@@\|^diff/!p; /^@@ -7/p }' | head -3)

    # Create patch files for each hunk
    local header_lines
    header_lines=$(echo "$diff_output" | sed -n '1,/^@@/{ /^@@/!p }')

    echo "$header_lines" > /tmp/claude/patch-a.diff
    echo "$hunk1" >> /tmp/claude/patch-a.diff

    echo "$header_lines" > /tmp/claude/patch-b.diff
    echo "$hunk2" >> /tmp/claude/patch-b.diff

    # Apply A then B
    git checkout file.txt --quiet
    git apply --unidiff-zero /tmp/claude/patch-a.diff 2>/dev/null
    git apply --unidiff-zero /tmp/claude/patch-b.diff 2>/dev/null
    local state_ab
    state_ab=$(cat file.txt | md5sum | awk '{print $1}')

    # Apply B then A
    git checkout file.txt --quiet
    git apply --unidiff-zero /tmp/claude/patch-b.diff 2>/dev/null
    git apply --unidiff-zero /tmp/claude/patch-a.diff 2>/dev/null
    local state_ba
    state_ba=$(cat file.txt | md5sum | awk '{print $1}')

    if [[ "$state_ab" == "$state_ba" ]]; then
        log_pass "Staging order is commutative" "A+B = B+A"
    else
        log_fail "Staging order is commutative" "A+B != B+A"
    fi

    # Test 5: Deletion at file start
    log_info "Test 5: Deletion at file start"
    echo -e "first\nsecond\nthird" > file.txt
    git add file.txt && git commit -m "three lines" --quiet

    echo -e "second\nthird" > file.txt  # Delete first line
    diff_output=$(git diff -U0 --no-ext-diff --no-color)
    header=$(echo "$diff_output" | grep '^@@')
    parsed=$(parse_hunk_header "$header")
    old_start=$(echo "$parsed" | awk '{print $1}')
    local new_start
    new_start=$(echo "$parsed" | awk '{print $3}')

    # Deletion at file start: old_start=1, new_start=0 (follows new_start = old_start - 1)
    if [[ "$old_start" == "1" && "$new_start" == "0" ]]; then
        log_pass "Deletion at start: old_start=1, new_start=0" "verified"
    else
        log_fail "Deletion at start: old_start=1, new_start=0" "got old_start=$old_start, new_start=$new_start"
    fi

    git checkout file.txt --quiet

    # Test 6: Explicit count=1 format
    log_info "Test 6: Explicit vs implicit count=1"
    echo -e "line1\nline2\nline3" > file.txt
    git add file.txt && git commit -m "reset" --quiet

    sed -i '2s/.*/modified/' file.txt
    diff_output=$(git diff -U0 --no-ext-diff --no-color)

    # Create patch with explicit counts
    local explicit_patch
    explicit_patch=$(echo "$diff_output" | sed 's/@@ -2 +2 @@/@@ -2,1 +2,1 @@/')

    git checkout file.txt --quiet

    # Apply explicit format
    echo "$explicit_patch" > /tmp/claude/explicit.diff
    if git apply --unidiff-zero /tmp/claude/explicit.diff 2>/dev/null; then
        log_pass "Explicit count=1 format accepted" "@@ -2,1 +2,1 @@ works"
    else
        log_fail "Explicit count=1 format accepted" "git apply rejected explicit counts"
    fi

    git checkout file.txt --quiet

    # Test 7: Multi-hunk cumulative formula verification
    log_info "Test 7: Cumulative position formula"
    seq 1 50 > file.txt
    git add file.txt && git commit -m "fifty lines" --quiet

    # Insert 2 lines after line 10, 1 line after line 30
    sed -i '10a INSERTED_A\nINSERTED_B' file.txt
    sed -i '33a INSERTED_C' file.txt  # 30 + 2 (from first insert) + 1 (after line 30)

    diff_output=$(git diff -U0 --no-ext-diff --no-color)
    headers=$(echo "$diff_output" | grep '^@@')

    # Parse both hunks
    local hunk1_header hunk2_header
    hunk1_header=$(echo "$headers" | head -1)
    hunk2_header=$(echo "$headers" | tail -1)

    local h1_old_start h1_new_start h1_old_count h1_new_count
    local h2_old_start h2_new_start

    parsed=$(parse_hunk_header "$hunk1_header")
    h1_old_start=$(echo "$parsed" | awk '{print $1}')
    h1_old_count=$(echo "$parsed" | awk '{print $2}')
    h1_new_start=$(echo "$parsed" | awk '{print $3}')
    h1_new_count=$(echo "$parsed" | awk '{print $4}')

    parsed=$(parse_hunk_header "$hunk2_header")
    h2_old_start=$(echo "$parsed" | awk '{print $1}')
    h2_new_start=$(echo "$parsed" | awk '{print $3}')

    # Calculate expected new_start for hunk 2
    # Delta from hunk 1 = new_count - old_count
    # For pure insertion: new_start = old_start + cumulative_delta + 1
    local delta1 expected_h2_new
    delta1=$((h1_new_count - h1_old_count))

    # Check if hunk 2 is pure insertion (old_count = 0)
    local h2_old_count
    h2_old_count=$(parse_hunk_header "$hunk2_header" | awk '{print $2}')

    if [[ "$h2_old_count" == "0" ]]; then
        # Pure insertion: new_start = old_start + delta + 1
        expected_h2_new=$((h2_old_start + delta1 + 1))
    else
        # Mixed/replacement: new_start = old_start + delta
        expected_h2_new=$((h2_old_start + delta1))
    fi

    if [[ "$h2_new_start" == "$expected_h2_new" ]]; then
        log_pass "Cumulative formula verified" "h2_new_start=$h2_new_start = h2_old_start($h2_old_start) + delta($delta1) + offset"
    else
        log_fail "Cumulative formula verified" "expected $expected_h2_new, got $h2_new_start"
    fi

    # Test 8: Deletion at end of file
    log_info "Test 8: Deletion at end of file"
    seq 1 10 > file.txt
    git add file.txt && git commit -m "ten lines" --quiet

    # Delete the last line
    seq 1 9 > file.txt
    diff_output=$(git diff -U0 --no-ext-diff --no-color)
    header=$(echo "$diff_output" | grep '^@@')
    parsed=$(parse_hunk_header "$header")
    old_start=$(echo "$parsed" | awk '{print $1}')
    new_start=$(echo "$parsed" | awk '{print $3}')

    # Git should use new_start = old_start - 1 = 9
    local expected=$((old_start - 1))
    if [[ "$new_start" == "$expected" ]]; then
        log_pass "Deletion at EOF: new_start = old_start - 1" "old=$old_start, new=$new_start"
    else
        log_fail "Deletion at EOF: new_start = old_start - 1" "expected $expected, got $new_start"
    fi

    git checkout file.txt --quiet

    # Test 9: Deleting the only line (empty file result)
    log_info "Test 9: Delete only line"
    echo "only" > file.txt
    git add file.txt && git commit -m "one line" --quiet

    # Delete the only line
    > file.txt
    diff_output=$(git diff -U0 --no-ext-diff --no-color)
    header=$(echo "$diff_output" | grep '^@@')
    parsed=$(parse_hunk_header "$header")
    old_start=$(echo "$parsed" | awk '{print $1}')
    new_start=$(echo "$parsed" | awk '{print $3}')

    # Should be old_start=1, new_start=0
    if [[ "$old_start" == "1" && "$new_start" == "0" ]]; then
        log_pass "Delete only line: old=1, new=0" "verified"
    else
        log_fail "Delete only line: old=1, new=0" "got old=$old_start, new=$new_start"
    fi

    git checkout file.txt --quiet

    # Test 10: Multiple consecutive line deletion
    log_info "Test 10: Multiple consecutive deletions"
    seq 1 20 > file.txt
    git add file.txt && git commit -m "twenty lines" --quiet

    # Delete lines 10-12 (3 lines)
    sed -i '10,12d' file.txt
    diff_output=$(git diff -U0 --no-ext-diff --no-color)
    header=$(echo "$diff_output" | grep '^@@')
    parsed=$(parse_hunk_header "$header")
    old_start=$(echo "$parsed" | awk '{print $1}')
    local old_count
    old_count=$(echo "$parsed" | awk '{print $2}')
    new_start=$(echo "$parsed" | awk '{print $3}')

    # Should be old_start=10, old_count=3, new_start=9
    expected=$((old_start - 1))
    if [[ "$old_start" == "10" && "$old_count" == "3" && "$new_start" == "$expected" ]]; then
        log_pass "Multi-line deletion: count=3, new_start=old_start-1" "old=$old_start, count=$old_count, new=$new_start"
    else
        log_fail "Multi-line deletion" "got old=$old_start, count=$old_count, new=$new_start"
    fi
}

#############################################################################
# MAIN
#############################################################################
main() {
    echo "Git Diff Invariant Verification"
    echo "================================"

    setup

    # Run tests
    test_header_line_counts
    test_position_rules
    test_git_apply_compatibility
    test_ordering
    test_no_newline_marker
    test_edge_cases

    # Summary
    echo ""
    echo "================================"
    echo -e "Tests run: $TESTS_RUN"
    echo -e "Passed: ${GREEN}$TESTS_PASSED${NC}"
    echo -e "Failed: ${RED}$TESTS_FAILED${NC}"

    # Cleanup
    cleanup

    # Exit with failure if any tests failed
    if [[ $TESTS_FAILED -gt 0 ]]; then
        exit 1
    fi
}

main "$@"
