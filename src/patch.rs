use crate::diff::DiffLine;
use crate::parse::LineRef;

/// Build a patch containing only the selected lines
pub fn build_patch(
    file_path: &str,
    lines: &[DiffLine],
    refs: &[LineRef],
) -> Result<String, String> {
    // Filter diff lines to only include selected ones
    let selected_lines = select_diff_lines(lines, refs)?;
    if selected_lines.is_empty() {
        return Err("No matching lines found for selection".into());
    }

    // Build the patch
    let mut patch = String::new();

    // Add file headers
    patch.push_str(&format!("--- a/{}\n", file_path));
    patch.push_str(&format!("+++ b/{}\n", file_path));

    // Group contiguous lines into hunks
    let hunks = group_into_hunks(&selected_lines);

    for hunk in hunks {
        let hunk_header = build_hunk_header(&hunk);
        patch.push_str(&hunk_header);
        patch.push('\n');

        for line in &hunk {
            match line {
                DiffLine::Add { content, .. } => {
                    patch.push('+');
                    patch.push_str(content);
                    patch.push('\n');
                }
                DiffLine::Delete { content, .. } => {
                    patch.push('-');
                    patch.push_str(content);
                    patch.push('\n');
                }
            }
        }
    }

    Ok(patch)
}

/// Select only the diff lines that match the given references
fn select_diff_lines(lines: &[DiffLine], refs: &[LineRef]) -> Result<Vec<DiffLine>, String> {
    let selected: Vec<_> = lines
        .iter()
        .filter(|line| line_matches_refs(line, refs))
        .cloned()
        .collect();

    if selected.is_empty() && !refs.is_empty() {
        Err("No lines matched the selection criteria in the unstaged diff".into())
    } else {
        Ok(selected)
    }
}

/// Check if a diff line matches any of the given references
fn line_matches_refs(line: &DiffLine, refs: &[LineRef]) -> bool {
    refs.iter().any(|ref_item| match (line, ref_item) {
        (DiffLine::Add { new_line, .. }, LineRef::Add(n)) => new_line == n,
        (DiffLine::Add { new_line, .. }, LineRef::AddRange(start, end)) => {
            new_line >= start && new_line <= end
        }
        (DiffLine::Delete { old_line, .. }, LineRef::Delete(n)) => old_line == n,
        (DiffLine::Delete { old_line, .. }, LineRef::DeleteRange(start, end)) => {
            old_line >= start && old_line <= end
        }
        _ => false,
    })
}

/// Group diff lines into contiguous hunks
fn group_into_hunks(lines: &[DiffLine]) -> Vec<Vec<DiffLine>> {
    if lines.is_empty() {
        return vec![];
    }

    // For simplicity, put each contiguous block into its own hunk
    // Lines are contiguous if their line numbers are adjacent
    let mut hunks = vec![];
    let mut current_hunk = vec![lines[0].clone()];

    for i in 1..lines.len() {
        let prev = &lines[i - 1];
        let curr = &lines[i];

        let is_contiguous = match (prev, curr) {
            (
                DiffLine::Add {
                    new_line: prev_line,
                    ..
                },
                DiffLine::Add {
                    new_line: curr_line,
                    ..
                },
            ) => *curr_line == prev_line + 1,
            (
                DiffLine::Delete {
                    old_line: prev_line,
                    ..
                },
                DiffLine::Delete {
                    old_line: curr_line,
                    ..
                },
            ) => *curr_line == prev_line + 1,
            _ => false,
        };

        if is_contiguous {
            current_hunk.push(curr.clone());
        } else {
            hunks.push(current_hunk);
            current_hunk = vec![curr.clone()];
        }
    }

    if !current_hunk.is_empty() {
        hunks.push(current_hunk);
    }

    hunks
}

/// Build the hunk header for a set of contiguous lines
fn build_hunk_header(lines: &[DiffLine]) -> String {
    debug_assert!(
        !lines.is_empty(),
        "build_hunk_header called with empty lines"
    );

    let mut old_start = 0u32;
    let mut old_count = 0u32;
    let mut new_start = 0u32;
    let mut new_count = 0u32;

    for line in lines {
        match line {
            DiffLine::Add { new_line, .. } => {
                if new_start == 0 {
                    new_start = *new_line;
                    // For additions, old_start is the line before
                    old_start = new_line - 1;
                }
                new_count += 1;
            }
            DiffLine::Delete { old_line, .. } => {
                if old_start == 0 {
                    old_start = *old_line;
                    // For deletions, new_start is old_start - 1 (where insertion would happen)
                    new_start = old_line - 1;
                }
                old_count += 1;
            }
        }
    }

    // Format: @@ -old_start,old_count +new_start,new_count @@
    // Special cases for counts of 0 or 1
    let old_part = if old_count == 0 {
        format!("-{},0", old_start)
    } else if old_count == 1 {
        format!("-{}", old_start)
    } else {
        format!("-{},{}", old_start, old_count)
    };

    let new_part = if new_count == 0 {
        format!("+{},0", new_start)
    } else if new_count == 1 {
        format!("+{}", new_start)
    } else {
        format!("+{},{}", new_start, new_count)
    };

    format!("@@ {} {} @@", old_part, new_part)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diff::DiffLine;
    use crate::parse::LineRef;

    #[test]
    fn build_patch_single_addition() {
        let lines = vec![DiffLine::Add {
            new_line: 137,
            content: "      debug = true;".to_string(),
        }];
        let refs = vec![LineRef::Add(137)];

        let patch = build_patch("flake.nix", &lines, &refs).unwrap();

        // Patch should have proper headers and the single addition
        assert!(patch.contains("--- a/flake.nix"));
        assert!(patch.contains("+++ b/flake.nix"));
        assert!(patch.contains("@@ -136,0 +137 @@"));
        assert!(patch.contains("+      debug = true;"));
    }

    #[test]
    fn build_patch_contiguous_additions() {
        let lines = vec![
            DiffLine::Add {
                new_line: 39,
                content: "".to_string(),
            },
            DiffLine::Add {
                new_line: 40,
                content: "    stylix = {".to_string(),
            },
            DiffLine::Add {
                new_line: 41,
                content: "      url = \"github:nix-community/stylix\";".to_string(),
            },
            DiffLine::Add {
                new_line: 42,
                content: "      inputs.nixpkgs.follows = \"nixpkgs\";".to_string(),
            },
            DiffLine::Add {
                new_line: 43,
                content: "    };".to_string(),
            },
        ];
        let refs = vec![LineRef::AddRange(39, 43)];

        let patch = build_patch("flake.nix", &lines, &refs).unwrap();

        assert!(patch.contains("--- a/flake.nix"));
        assert!(patch.contains("+++ b/flake.nix"));
        assert!(patch.contains("@@ -38,0 +39,5 @@"));
        assert!(patch.contains("+"));
        assert!(patch.contains("+    stylix = {"));
        assert!(patch.contains("+    };"));
    }

    #[test]
    fn build_patch_partial_additions() {
        // Have 3 additions (lines 40, 41, 42), but only select 40 and 41
        let lines = vec![
            DiffLine::Add {
                new_line: 40,
                content: "        # Allow Stylix to override terminal font".to_string(),
            },
            DiffLine::Add {
                new_line: 41,
                content:
                    "        \"terminal.integrated.fontFamily\" = lib.mkDefault \"monospace\";"
                        .to_string(),
            },
            DiffLine::Add {
                new_line: 42,
                content: "        \"direnv.restart.automatic\" = true;".to_string(),
            },
        ];
        let refs = vec![LineRef::AddRange(40, 41)];

        let patch = build_patch("default.nix", &lines, &refs).unwrap();

        // Should only contain lines 40 and 41, not 42
        assert!(patch.contains("@@ -39,0 +40,2 @@"));
        assert!(patch.contains("+        # Allow Stylix to override terminal font"));
        assert!(patch.contains("+        \"terminal.integrated.fontFamily\""));
        assert!(!patch.contains("direnv.restart.automatic"));
    }

    #[test]
    fn build_patch_single_deletion() {
        let lines = vec![DiffLine::Delete {
            old_line: 15,
            content: "      enableAutosuggestions = true;".to_string(),
        }];
        let refs = vec![LineRef::Delete(15)];

        let patch = build_patch("zsh.nix", &lines, &refs).unwrap();

        assert!(patch.contains("--- a/zsh.nix"));
        assert!(patch.contains("+++ b/zsh.nix"));
        assert!(patch.contains("@@ -15 +14,0 @@"));
        assert!(patch.contains("-      enableAutosuggestions = true;"));
    }

    #[test]
    fn build_patch_selective_from_mixed() {
        // Mixed add/delete, but only select one addition
        let lines = vec![
            DiffLine::Delete {
                old_line: 10,
                content: "    gtk.theme.name = \"Adwaita\";".to_string(),
            },
            DiffLine::Delete {
                old_line: 11,
                content: "    gtk.iconTheme.name = \"Papirus\";".to_string(),
            },
            DiffLine::Add {
                new_line: 10,
                content: "    # Theme managed by Stylix".to_string(),
            },
            DiffLine::Add {
                new_line: 11,
                content: "    gtk.iconTheme.name = \"Papirus-Dark\";".to_string(),
            },
            DiffLine::Add {
                new_line: 12,
                content: "    gtk.cursorTheme.size = 24;".to_string(),
            },
        ];
        // Only select the cursor theme addition (line 12)
        let refs = vec![LineRef::Add(12)];

        let patch = build_patch("gtk.nix", &lines, &refs).unwrap();

        // Should only have the cursor size addition
        assert!(patch.contains("@@ -11,0 +12 @@"));
        assert!(patch.contains("+    gtk.cursorTheme.size = 24;"));
        // Should NOT have deletions or other additions
        assert!(!patch.contains("-    gtk.theme.name"));
        assert!(!patch.contains("-    gtk.iconTheme.name = \"Papirus\""));
        assert!(!patch.contains("+    # Theme managed by Stylix"));
        assert!(!patch.contains("+    gtk.iconTheme.name = \"Papirus-Dark\""));
    }

    #[test]
    fn build_patch_no_matching_lines() {
        let lines = vec![DiffLine::Add {
            new_line: 10,
            content: "something".to_string(),
        }];
        let refs = vec![LineRef::Add(99)]; // Line 99 doesn't exist

        let result = build_patch("file.nix", &lines, &refs);
        assert!(result.is_err());
    }
}
