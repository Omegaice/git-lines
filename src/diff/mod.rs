pub mod file;
pub mod full;
pub mod hunk;

pub use full::Diff;

/// Format a git diff for user display with explicit line numbers
pub fn format_diff(diff: &Diff) -> String {
    let mut result = String::new();

    for file_diff in &diff.files {
        result.push_str(&file_diff.path);
        result.push_str(":\n");

        for hunk in &file_diff.hunks {
            // Show deletions
            for (i, line) in hunk.old.1.iter().enumerate() {
                let line_num = hunk.old.0 + i as u32;
                result.push_str(&format!("  -{}:\t{}\n", line_num, line));
            }

            // Show additions
            for (i, line) in hunk.new.1.iter().enumerate() {
                let line_num = hunk.new.0 + i as u32;
                result.push_str(&format!("  +{}:\t{}\n", line_num, line));
            }

            result.push('\n');
        }
    }

    // Remove trailing newline if present
    if result.ends_with("\n\n") {
        result.pop();
    }

    result
}
