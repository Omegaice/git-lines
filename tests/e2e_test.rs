#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(missing_docs)]

use git_stager::GitStager;
use git2::{Repository, Signature};
use std::{fs, path::Path, process::Command};
use tempfile::TempDir;

/// Test fixture for a git repository
struct Fixture {
    dir: TempDir,
    repo: Repository,
    stager: GitStager,
}

impl Fixture {
    /// Create a new empty repo with deterministic config
    fn new() -> Self {
        let dir = TempDir::new().expect("Failed to create temp dir");
        let repo = Repository::init(dir.path()).expect("Failed to init repo");

        // Deterministic config
        let mut config = repo.config().unwrap();
        config.set_str("user.name", "Test User").unwrap();
        config.set_str("user.email", "test@example.com").unwrap();

        let stager = GitStager::new(dir.path());

        Self { dir, repo, stager }
    }

    /// Write a file to the repo
    fn write_file(&self, name: &str, content: &str) {
        let path = self.dir.path().join(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, content).unwrap();
    }

    /// Stage a file
    fn stage_file(&self, name: &str) {
        let mut index = self.repo.index().unwrap();
        index.add_path(Path::new(name)).unwrap();
        index.write().unwrap();
    }

    /// Create a commit
    fn commit(&self, message: &str) {
        let sig = Signature::new(
            "Test User",
            "test@example.com",
            &git2::Time::new(1234567890, 0),
        )
        .unwrap();
        let tree_id = self.repo.index().unwrap().write_tree().unwrap();
        let tree = self.repo.find_tree(tree_id).unwrap();

        if self.repo.head().is_ok() {
            let parent = self.repo.head().unwrap().peel_to_commit().unwrap();
            self.repo
                .commit(Some("HEAD"), &sig, &sig, message, &tree, &[&parent])
                .unwrap();
        } else {
            self.repo
                .commit(Some("HEAD"), &sig, &sig, message, &tree, &[])
                .unwrap();
        }
    }

    /// Get git diff --cached output for all staged changes
    fn git_diff_cached(&self) -> String {
        let output = Command::new("git")
            .args([
                "-C",
                self.dir.path().to_str().unwrap(),
                "diff",
                "--cached",
                "--no-ext-diff",
                "-U0",
                "--no-color",
            ])
            .output()
            .expect("Failed to run git diff --cached");
        String::from_utf8(output.stdout).unwrap()
    }

    /// Helper to create a file with N numbered lines
    fn numbered_lines(n: usize) -> String {
        (1..=n)
            .map(|i| format!("line {}", i))
            .collect::<Vec<_>>()
            .join("\n")
            + "\n"
    }
}

// =============================================================================
// 01: Addition Patches
// =============================================================================
mod addition {
    use super::*;

    /// 1.1: Single Line Addition
    #[test]
    fn single_line() {
        let f = Fixture::new();
        let initial = Fixture::numbered_lines(136);
        f.write_file("file.nix", &initial);
        f.stage_file("file.nix");
        f.commit("initial");

        f.write_file("file.nix", &(initial + "      debug = true;\n"));

        insta::assert_snapshot!(
            "addition__single_line__diff",
            f.stager.diff(&["file.nix".to_string()]).unwrap()
        );
        f.stager.stage("file.nix:137").unwrap();
        insta::assert_snapshot!("addition__single_line__staged", f.git_diff_cached());
    }

    /// 1.2: Contiguous Range Addition
    #[test]
    fn contiguous_range() {
        let f = Fixture::new();
        let initial = Fixture::numbered_lines(38);
        f.write_file("file.nix", &initial);
        f.stage_file("file.nix");
        f.commit("initial");

        let addition = r#"
    stylix = {
      url = "github:danth/stylix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
"#;
        f.write_file("file.nix", &(initial + addition));

        insta::assert_snapshot!(
            "addition__contiguous_range__diff",
            f.stager.diff(&["file.nix".to_string()]).unwrap()
        );
        f.stager.stage("file.nix:39..43").unwrap();
        insta::assert_snapshot!("addition__contiguous_range__staged", f.git_diff_cached());
    }

    /// 1.3: Non-Contiguous Selection
    #[test]
    fn non_contiguous_selection() {
        let f = Fixture::new();
        let initial = Fixture::numbered_lines(9);
        f.write_file("file.nix", &initial);
        f.stage_file("file.nix");
        f.commit("initial");

        let additions = "    # TODO: Remove after testing\n    debug.enable = true;\n    debug.verbose = true;\n    # Another comment\n    feature.enable = true;\n";
        f.write_file("file.nix", &(initial + additions));

        insta::assert_snapshot!(
            "addition__non_contiguous_selection__diff",
            f.stager.diff(&["file.nix".to_string()]).unwrap()
        );
        f.stager.stage("file.nix:11,12,14").unwrap();
        insta::assert_snapshot!(
            "addition__non_contiguous_selection__staged",
            f.git_diff_cached()
        );
    }

    /// 1.4: Addition from Mixed Hunk
    #[test]
    fn from_mixed_hunk() {
        let f = Fixture::new();
        let mut lines: Vec<String> = (1..=30)
            .map(|i| {
                if i == 25 {
                    "    old_setting = true;".to_string()
                } else if i == 26 {
                    "    deprecated = true;".to_string()
                } else {
                    format!("line {}", i)
                }
            })
            .collect();
        let initial = lines.join("\n") + "\n";
        f.write_file("file.nix", &initial);
        f.stage_file("file.nix");
        f.commit("initial");

        // Replace lines 25-26 with three new lines
        lines[24] = "    new_setting = false;".to_string();
        lines[25] = "    modern = true;".to_string();
        lines.insert(26, "    additional = true;".to_string());
        let modified = lines.join("\n") + "\n";
        f.write_file("file.nix", &modified);

        insta::assert_snapshot!(
            "addition__from_mixed_hunk__diff",
            f.stager.diff(&["file.nix".to_string()]).unwrap()
        );
        f.stager.stage("file.nix:27").unwrap();
        insta::assert_snapshot!("addition__from_mixed_hunk__staged", f.git_diff_cached());
    }

    /// 1.5: Multiple Separate Additions
    #[test]
    fn multiple_separate() {
        let f = Fixture::new();
        let initial = Fixture::numbered_lines(119);
        f.write_file("file.nix", &initial);
        f.stage_file("file.nix");
        f.commit("initial");

        // Insert at positions 7, 45, and 120
        let mut lines: Vec<String> = (1..=119).map(|i| format!("line {}", i)).collect();
        lines.insert(6, "     first_addition = true;".to_string());
        lines.insert(44, "    second_addition = true;".to_string());
        lines.push("    third_addition = true;".to_string());
        let modified = lines.join("\n") + "\n";
        f.write_file("file.nix", &modified);

        insta::assert_snapshot!(
            "addition__multiple_separate__diff",
            f.stager.diff(&["file.nix".to_string()]).unwrap()
        );
        f.stager.stage("file.nix:7,45").unwrap();
        insta::assert_snapshot!("addition__multiple_separate__staged", f.git_diff_cached());
    }

    /// 1.6: Complex Range and Individual Mix
    #[test]
    fn complex_range_mix() {
        let f = Fixture::new();
        let initial = Fixture::numbered_lines(29);
        f.write_file("file.nix", &initial);
        f.stage_file("file.nix");
        f.commit("initial");

        let additions = "    line_30 = true;\n    line_31 = true;\n    line_32 = true;\n    line_33 = true;\n    line_34 = true;\n    line_35 = true;\n";
        f.write_file("file.nix", &(initial + additions));

        insta::assert_snapshot!(
            "addition__complex_range_mix__diff",
            f.stager.diff(&["file.nix".to_string()]).unwrap()
        );
        f.stager.stage("file.nix:30,32..34").unwrap();
        insta::assert_snapshot!("addition__complex_range_mix__staged", f.git_diff_cached());
    }
}

// =============================================================================
// 02: Deletion Patches
// =============================================================================
mod deletion {
    use super::*;

    /// 2.1: Single Line Deletion
    #[test]
    fn single_line() {
        let f = Fixture::new();
        let lines: Vec<String> = (1..=20)
            .map(|i| {
                if i == 15 {
                    "      enableAutosuggestions = true;".to_string()
                } else {
                    format!("line {}", i)
                }
            })
            .collect();
        let initial = lines.join("\n") + "\n";
        f.write_file("file.nix", &initial);
        f.stage_file("file.nix");
        f.commit("initial");

        let modified: String = lines
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != 14)
            .map(|(_, s)| s.as_str())
            .collect::<Vec<_>>()
            .join("\n")
            + "\n";
        f.write_file("file.nix", &modified);

        insta::assert_snapshot!(
            "deletion__single_line__diff",
            f.stager.diff(&["file.nix".to_string()]).unwrap()
        );
        f.stager.stage("file.nix:-15").unwrap();
        insta::assert_snapshot!("deletion__single_line__staged", f.git_diff_cached());
    }

    /// 2.2: Contiguous Range Deletion
    #[test]
    fn contiguous_range() {
        let f = Fixture::new();
        let lines: Vec<String> = (1..=20)
            .map(|i| match i {
                15 => "      enableAutosuggestions = true;".to_string(),
                16 => "      enableCompletion = true;".to_string(),
                17 => "      enableSyntaxHighlighting = true;".to_string(),
                _ => format!("line {}", i),
            })
            .collect();
        let initial = lines.join("\n") + "\n";
        f.write_file("file.nix", &initial);
        f.stage_file("file.nix");
        f.commit("initial");

        let modified: String = lines
            .iter()
            .enumerate()
            .filter(|(i, _)| !(14..=16).contains(i))
            .map(|(_, s)| s.as_str())
            .collect::<Vec<_>>()
            .join("\n")
            + "\n";
        f.write_file("file.nix", &modified);

        insta::assert_snapshot!(
            "deletion__contiguous_range__diff",
            f.stager.diff(&["file.nix".to_string()]).unwrap()
        );
        f.stager.stage("file.nix:-15..-17").unwrap();
        insta::assert_snapshot!("deletion__contiguous_range__staged", f.git_diff_cached());
    }

    /// 2.3: Non-Contiguous Deletion
    #[test]
    fn non_contiguous() {
        let f = Fixture::new();
        let lines: Vec<String> = (1..=20)
            .map(|i| match i {
                10 => "    # Old comment".to_string(),
                11 => "    deprecated_setting = true;".to_string(),
                12 => "    another_deprecated = true;".to_string(),
                13 => "    # Another old comment".to_string(),
                14 => "    legacy_feature = true;".to_string(),
                _ => format!("line {}", i),
            })
            .collect();
        let initial = lines.join("\n") + "\n";
        f.write_file("file.nix", &initial);
        f.stage_file("file.nix");
        f.commit("initial");

        // Delete lines 10-14
        let modified: String = lines
            .iter()
            .enumerate()
            .filter(|(i, _)| !(9..=13).contains(i))
            .map(|(_, s)| s.as_str())
            .collect::<Vec<_>>()
            .join("\n")
            + "\n";
        f.write_file("file.nix", &modified);

        insta::assert_snapshot!(
            "deletion__non_contiguous__diff",
            f.stager.diff(&["file.nix".to_string()]).unwrap()
        );
        // Stage only lines 11, 12, and 14 (skipping 10 and 13)
        f.stager.stage("file.nix:-11..-12,-14").unwrap();
        insta::assert_snapshot!("deletion__non_contiguous__staged", f.git_diff_cached());
    }

    /// 2.4: Deletion from Mixed Hunk
    #[test]
    fn from_mixed_hunk() {
        let f = Fixture::new();
        let mut lines: Vec<String> = (1..=30)
            .map(|i| {
                if i == 25 {
                    "    old_setting = true;".to_string()
                } else if i == 26 {
                    "    deprecated = true;".to_string()
                } else {
                    format!("line {}", i)
                }
            })
            .collect();
        let initial = lines.join("\n") + "\n";
        f.write_file("file.nix", &initial);
        f.stage_file("file.nix");
        f.commit("initial");

        // Replace lines 25-26 with three new lines
        lines[24] = "    new_setting = false;".to_string();
        lines[25] = "    modern = true;".to_string();
        lines.insert(26, "    additional = true;".to_string());
        let modified = lines.join("\n") + "\n";
        f.write_file("file.nix", &modified);

        insta::assert_snapshot!(
            "deletion__from_mixed_hunk__diff",
            f.stager.diff(&["file.nix".to_string()]).unwrap()
        );
        f.stager.stage("file.nix:-26").unwrap();
        insta::assert_snapshot!("deletion__from_mixed_hunk__staged", f.git_diff_cached());
    }

    /// 2.5: Deletion at Start of File
    #[test]
    fn at_start() {
        let f = Fixture::new();
        let lines = vec![
            "#!/usr/bin/env bash".to_string(),
            "# Old header comment".to_string(),
        ];
        let rest: Vec<String> = (3..=10).map(|i| format!("line {}", i)).collect();
        let initial = [lines.clone(), rest.clone()].concat().join("\n") + "\n";
        f.write_file("file.sh", &initial);
        f.stage_file("file.sh");
        f.commit("initial");

        let modified = rest.join("\n") + "\n";
        f.write_file("file.sh", &modified);

        insta::assert_snapshot!(
            "deletion__at_start__diff",
            f.stager.diff(&["file.sh".to_string()]).unwrap()
        );
        f.stager.stage("file.sh:-1..-2").unwrap();
        insta::assert_snapshot!("deletion__at_start__staged", f.git_diff_cached());
    }

    /// 2.6: Deletion at End of File
    #[test]
    fn at_end() {
        let f = Fixture::new();
        let initial = Fixture::numbered_lines(10);
        f.write_file("file.txt", &initial);
        f.stage_file("file.txt");
        f.commit("initial");

        let modified = Fixture::numbered_lines(9);
        f.write_file("file.txt", &modified);

        insta::assert_snapshot!(
            "deletion__at_end__diff",
            f.stager.diff(&["file.txt".to_string()]).unwrap()
        );
        f.stager.stage("file.txt:-10").unwrap();
        insta::assert_snapshot!("deletion__at_end__staged", f.git_diff_cached());
    }

    /// 2.7: Delete Only Line
    #[test]
    fn only_line() {
        let f = Fixture::new();
        f.write_file("file.txt", "only content\n");
        f.stage_file("file.txt");
        f.commit("initial");

        f.write_file("file.txt", "");

        insta::assert_snapshot!(
            "deletion__only_line__diff",
            f.stager.diff(&["file.txt".to_string()]).unwrap()
        );
        f.stager.stage("file.txt:-1").unwrap();
        insta::assert_snapshot!("deletion__only_line__staged", f.git_diff_cached());
    }
}

// =============================================================================
// 03: Replacement Patches
// =============================================================================
mod replacement {
    use super::*;

    /// 3.1: Simple Single-Line Replacement
    #[test]
    fn simple_single_line() {
        let f = Fixture::new();
        let mut lines: Vec<String> = (1..=15).map(|i| format!("line {}", i)).collect();
        lines[9] = "    old_value = \"deprecated\";".to_string();
        let initial = lines.join("\n") + "\n";
        f.write_file("file.nix", &initial);
        f.stage_file("file.nix");
        f.commit("initial");

        lines[9] = "    new_value = \"modern\";".to_string();
        let modified = lines.join("\n") + "\n";
        f.write_file("file.nix", &modified);

        insta::assert_snapshot!(
            "replacement__simple_single_line__diff",
            f.stager.diff(&["file.nix".to_string()]).unwrap()
        );
        f.stager.stage("file.nix:-10,10").unwrap();
        insta::assert_snapshot!(
            "replacement__simple_single_line__staged",
            f.git_diff_cached()
        );
    }

    /// 3.2: Multi-Line Replacement
    #[test]
    fn multi_line() {
        let f = Fixture::new();
        let mut lines: Vec<String> = (1..=30).map(|i| format!("line {}", i)).collect();
        lines[19] = "    # Old implementation".to_string();
        lines[20] = "    legacy_function() {".to_string();
        lines[21] = "      old_code();".to_string();
        lines[22] = "    }".to_string();
        let initial = lines.join("\n") + "\n";
        f.write_file("file.js", &initial);
        f.stage_file("file.js");
        f.commit("initial");

        // Replace 4 lines with 5 lines
        lines[19] = "    # New implementation".to_string();
        lines[20] = "    modern_function() {".to_string();
        lines[21] = "      new_code();".to_string();
        lines[22] = "      extra_feature();".to_string();
        lines.insert(23, "    }".to_string());
        let modified = lines.join("\n") + "\n";
        f.write_file("file.js", &modified);

        insta::assert_snapshot!(
            "replacement__multi_line__diff",
            f.stager.diff(&["file.js".to_string()]).unwrap()
        );
        f.stager.stage("file.js:-20..-23,20..24").unwrap();
        insta::assert_snapshot!("replacement__multi_line__staged", f.git_diff_cached());
    }

    /// 3.3: Partial Replacement from Mixed Hunk
    #[test]
    fn partial_from_mixed() {
        let f = Fixture::new();
        let mut lines: Vec<String> = (1..=15).map(|i| format!("line {}", i)).collect();
        lines[9] = "    setting_a = true;".to_string();
        lines[10] = "    setting_b = false;".to_string();
        lines[11] = "    setting_c = \"old\";".to_string();
        let initial = lines.join("\n") + "\n";
        f.write_file("file.nix", &initial);
        f.stage_file("file.nix");
        f.commit("initial");

        // Change all three and add a fourth
        lines[9] = "    setting_a = false;".to_string();
        lines[10] = "    setting_b = false;".to_string();
        lines[11] = "    setting_c = \"new\";".to_string();
        lines.insert(12, "    setting_d = true;".to_string());
        let modified = lines.join("\n") + "\n";
        f.write_file("file.nix", &modified);

        insta::assert_snapshot!(
            "replacement__partial_from_mixed__diff",
            f.stager.diff(&["file.nix".to_string()]).unwrap()
        );
        f.stager.stage("file.nix:-10,10,-12,12").unwrap();
        insta::assert_snapshot!(
            "replacement__partial_from_mixed__staged",
            f.git_diff_cached()
        );
    }

    /// 3.4: Asymmetric Replacement
    #[test]
    fn asymmetric() {
        let f = Fixture::new();
        let mut lines: Vec<String> = (1..=35).map(|i| format!("line {}", i)).collect();
        lines[29] = "    verbose_old_style_config();".to_string();
        let initial = lines.join("\n") + "\n";
        f.write_file("file.nix", &initial);
        f.stage_file("file.nix");
        f.commit("initial");

        lines[29] = "    cfg();".to_string();
        let modified = lines.join("\n") + "\n";
        f.write_file("file.nix", &modified);

        insta::assert_snapshot!(
            "replacement__asymmetric__diff",
            f.stager.diff(&["file.nix".to_string()]).unwrap()
        );
        f.stager.stage("file.nix:-30,30").unwrap();
        insta::assert_snapshot!("replacement__asymmetric__staged", f.git_diff_cached());
    }

    /// 3.5: Multiple Separate Replacements
    #[test]
    fn multiple_separate() {
        let f = Fixture::new();
        let mut lines: Vec<String> = (1..=85).map(|i| format!("line {}", i)).collect();
        lines[4] = "     const OLD_CONSTANT = 42;".to_string();
        lines[24] = "    deprecatedMethod() {}".to_string();
        lines[79] = "    // Old comment".to_string();
        let initial = lines.join("\n") + "\n";
        f.write_file("file.js", &initial);
        f.stage_file("file.js");
        f.commit("initial");

        lines[4] = "     const NEW_CONSTANT = 100;".to_string();
        lines[24] = "    modernMethod() {}".to_string();
        lines[79] = "    // Updated comment".to_string();
        let modified = lines.join("\n") + "\n";
        f.write_file("file.js", &modified);

        insta::assert_snapshot!(
            "replacement__multiple_separate__diff",
            f.stager.diff(&["file.js".to_string()]).unwrap()
        );
        f.stager.stage("file.js:-5,5,-25,25").unwrap();
        insta::assert_snapshot!(
            "replacement__multiple_separate__staged",
            f.git_diff_cached()
        );
    }

    /// 3.6: Complex Mixed Selection
    #[test]
    fn complex_mixed() {
        let f = Fixture::new();
        let mut lines: Vec<String> = (1..=15).map(|i| format!("line {}", i)).collect();
        lines[9] = "    # Header to remove".to_string();
        lines[10] = "    old_setting = true;".to_string();
        let initial = lines.join("\n") + "\n";
        f.write_file("file.nix", &initial);
        f.stage_file("file.nix");
        f.commit("initial");

        // Replace both lines with two new lines
        lines[9] = "    new_setting = false;".to_string();
        lines[10] = "    added_setting = true;".to_string();
        let modified = lines.join("\n") + "\n";
        f.write_file("file.nix", &modified);

        insta::assert_snapshot!(
            "replacement__complex_mixed__diff",
            f.stager.diff(&["file.nix".to_string()]).unwrap()
        );
        // Stage deletion of old line 11 and addition of new line 10
        f.stager.stage("file.nix:-11,10").unwrap();
        insta::assert_snapshot!("replacement__complex_mixed__staged", f.git_diff_cached());
    }
}

// =============================================================================
// 04: Multi-Hunk Patches
// =============================================================================
mod multi_hunk {
    use super::*;

    /// 4.1: Two Separate Additions
    #[test]
    fn two_separate_additions() {
        let f = Fixture::new();
        let initial = Fixture::numbered_lines(119);
        f.write_file("file.nix", &initial);
        f.stage_file("file.nix");
        f.commit("initial");

        let mut lines: Vec<String> = (1..=119).map(|i| format!("line {}", i)).collect();
        lines.insert(6, "     first_addition = true;".to_string());
        lines.insert(44, "    second_addition = true;".to_string());
        lines.push("    third_addition = true;".to_string());
        let modified = lines.join("\n") + "\n";
        f.write_file("file.nix", &modified);

        insta::assert_snapshot!(
            "multi_hunk__two_separate_additions__diff",
            f.stager.diff(&["file.nix".to_string()]).unwrap()
        );
        f.stager.stage("file.nix:7,45,120").unwrap();
        insta::assert_snapshot!(
            "multi_hunk__two_separate_additions__staged",
            f.git_diff_cached()
        );
    }

    /// 4.2: Mixed Operations in Different Hunks
    #[test]
    fn mixed_operations() {
        let f = Fixture::new();
        let mut lines: Vec<String> = (1..=55).map(|i| format!("line {}", i)).collect();
        lines[29] = "    deleted_line = false;".to_string();
        lines[49] = "    old_value = 1;".to_string();
        let initial = lines.join("\n") + "\n";
        f.write_file("file.nix", &initial);
        f.stage_file("file.nix");
        f.commit("initial");

        // Add line at 10, delete line 30, replace line 50
        let mut modified_lines: Vec<String> = Vec::new();
        for (i, line) in lines.iter().enumerate() {
            if i == 9 {
                modified_lines.push(line.clone());
                modified_lines.push("    added_line = true;".to_string());
            } else if i == 29 {
                // Skip - deletion
            } else if i == 49 {
                modified_lines.push("    new_value = 2;".to_string());
            } else {
                modified_lines.push(line.clone());
            }
        }
        let modified = modified_lines.join("\n") + "\n";
        f.write_file("file.nix", &modified);

        insta::assert_snapshot!(
            "multi_hunk__mixed_operations__diff",
            f.stager.diff(&["file.nix".to_string()]).unwrap()
        );
        f.stager.stage("file.nix:10,-30,-50,49").unwrap();
        insta::assert_snapshot!("multi_hunk__mixed_operations__staged", f.git_diff_cached());
    }

    /// 4.3: Non-Contiguous Selection Creating Multiple Hunks
    #[test]
    fn non_contiguous_hunks() {
        let f = Fixture::new();
        let initial = Fixture::numbered_lines(19);
        f.write_file("file.nix", &initial);
        f.stage_file("file.nix");
        f.commit("initial");

        let additions = "    line_20 = true;\n    line_21 = true;\n    line_22 = true;\n    line_23 = true;\n    line_24 = true;\n";
        f.write_file("file.nix", &(initial + additions));

        insta::assert_snapshot!(
            "multi_hunk__non_contiguous_hunks__diff",
            f.stager.diff(&["file.nix".to_string()]).unwrap()
        );
        f.stager.stage("file.nix:20,22,24").unwrap();
        insta::assert_snapshot!(
            "multi_hunk__non_contiguous_hunks__staged",
            f.git_diff_cached()
        );
    }

    /// 4.4: Cumulative Position Tracking
    #[test]
    fn cumulative_tracking() {
        let f = Fixture::new();
        let initial = Fixture::numbered_lines(52);
        f.write_file("file.js", &initial);
        f.stage_file("file.js");
        f.commit("initial");

        let mut lines: Vec<String> = (1..=52).map(|i| format!("line {}", i)).collect();
        // Insert 2 lines after line 9
        lines.insert(9, "    // Add 2 lines here".to_string());
        lines.insert(10, "    first_new_line();".to_string());
        // Delete lines 30-32 (now at indices 32-34 due to insertions)
        lines[32] = "".to_string(); // Mark for deletion
        lines[33] = "".to_string();
        lines[34] = "".to_string();
        let mut modified_lines: Vec<String> = lines.into_iter().filter(|s| !s.is_empty()).collect();
        // Add line at end (was line 52, now different position)
        modified_lines.push("    // Add 1 line".to_string());
        let modified = modified_lines.join("\n") + "\n";
        f.write_file("file.js", &modified);

        insta::assert_snapshot!(
            "multi_hunk__cumulative_tracking__diff",
            f.stager.diff(&["file.js".to_string()]).unwrap()
        );
        // Stage the additions and deletions
        f.stager.stage("file.js:10,11,-30..-32,50").unwrap();
        insta::assert_snapshot!(
            "multi_hunk__cumulative_tracking__staged",
            f.git_diff_cached()
        );
    }

    /// 4.5: Many Hunks Performance Test
    #[test]
    fn many_hunks() {
        let f = Fixture::new();
        let initial = Fixture::numbered_lines(94);
        f.write_file("file.js", &initial);
        f.stage_file("file.js");
        f.commit("initial");

        // Insert lines at 5, 15, 25, 35, 45, 55, 65, 75, 85, 95
        let mut lines: Vec<String> = (1..=94).map(|i| format!("line {}", i)).collect();
        let insertions = [4, 14, 24, 34, 44, 54, 64, 74, 84];
        let mut offset = 0;
        for (idx, pos) in insertions.iter().enumerate() {
            lines.insert(pos + offset, format!("     change_{}();", idx + 1));
            offset += 1;
        }
        lines.push("     change_10();".to_string());
        let modified = lines.join("\n") + "\n";
        f.write_file("file.js", &modified);

        insta::assert_snapshot!(
            "multi_hunk__many_hunks__diff",
            f.stager.diff(&["file.js".to_string()]).unwrap()
        );
        f.stager
            .stage("file.js:5,15,25,35,45,55,65,75,85,95")
            .unwrap();
        insta::assert_snapshot!("multi_hunk__many_hunks__staged", f.git_diff_cached());
    }

    /// 4.6: Order Independence
    #[test]
    fn order_independence() {
        let f = Fixture::new();
        let initial = Fixture::numbered_lines(49);
        f.write_file("file.nix", &initial);
        f.stage_file("file.nix");
        f.commit("initial");

        let mut lines: Vec<String> = (1..=49).map(|i| format!("line {}", i)).collect();
        lines.insert(2, "     early_addition();".to_string());
        lines.push("    late_addition();".to_string());
        let modified = lines.join("\n") + "\n";
        f.write_file("file.nix", &modified);

        insta::assert_snapshot!(
            "multi_hunk__order_independence__diff",
            f.stager.diff(&["file.nix".to_string()]).unwrap()
        );
        // Stage in reverse order
        f.stager.stage("file.nix:50,3").unwrap();
        insta::assert_snapshot!(
            "multi_hunk__order_independence__staged",
            f.git_diff_cached()
        );
    }
}

// =============================================================================
// 05: Multi-File Patches
// =============================================================================
mod multi_file {
    use super::*;

    /// 5.1: Two Files Simple Addition
    #[test]
    fn two_files_simple() {
        let f = Fixture::new();
        let flake_initial = Fixture::numbered_lines(136);
        let config_initial = Fixture::numbered_lines(41);
        f.write_file("flake.nix", &flake_initial);
        f.write_file("config.nix", &config_initial);
        f.stage_file("flake.nix");
        f.stage_file("config.nix");
        f.commit("initial");

        f.write_file("flake.nix", &(flake_initial + "    debug = true;\n"));
        f.write_file(
            "config.nix",
            &(config_initial + "    feature.enable = true;\n"),
        );

        insta::assert_snapshot!(
            "multi_file__two_files_simple__diff",
            f.stager.diff(&[]).unwrap()
        );
        f.stager.stage("flake.nix:137").unwrap();
        f.stager.stage("config.nix:42").unwrap();
        insta::assert_snapshot!("multi_file__two_files_simple__staged", f.git_diff_cached());
    }

    /// 5.2: Mixed Operations Across Files
    #[test]
    fn mixed_across_files() {
        let f = Fixture::new();
        let main_initial = Fixture::numbered_lines(15);
        let mut utils_lines: Vec<String> = (1..=30).map(|i| format!("line {}", i)).collect();
        utils_lines[24] = "    deprecated_helper();".to_string();
        let utils_initial = utils_lines.join("\n") + "\n";
        let mut config_lines: Vec<String> = (1..=10).map(|i| format!("line {}", i)).collect();
        config_lines[4] = "    OLD_VERSION = \"1.0\";".to_string();
        let config_initial = config_lines.join("\n") + "\n";

        f.write_file("src/main.js", &main_initial);
        f.write_file("src/utils.js", &utils_initial);
        f.write_file("src/config.js", &config_initial);
        f.stage_file("src/main.js");
        f.stage_file("src/utils.js");
        f.stage_file("src/config.js");
        f.commit("initial");

        // Add to main.js
        let mut main_lines: Vec<String> = (1..=15).map(|i| format!("line {}", i)).collect();
        main_lines.insert(9, "    new_function();".to_string());
        let main_modified = main_lines.join("\n") + "\n";
        f.write_file("src/main.js", &main_modified);

        // Delete from utils.js
        utils_lines.remove(24);
        let utils_modified = utils_lines.join("\n") + "\n";
        f.write_file("src/utils.js", &utils_modified);

        // Replace in config.js
        config_lines[4] = "    NEW_VERSION = \"2.0\";".to_string();
        let config_modified = config_lines.join("\n") + "\n";
        f.write_file("src/config.js", &config_modified);

        insta::assert_snapshot!(
            "multi_file__mixed_across_files__diff",
            f.stager.diff(&[]).unwrap()
        );
        f.stager.stage("src/main.js:10").unwrap();
        f.stager.stage("src/utils.js:-25").unwrap();
        f.stager.stage("src/config.js:-5,5").unwrap();
        insta::assert_snapshot!(
            "multi_file__mixed_across_files__staged",
            f.git_diff_cached()
        );
    }

    /// 5.3: Multiple Hunks in Multiple Files
    #[test]
    fn multi_hunk_multi_file() {
        let f = Fixture::new();
        let core_initial = Fixture::numbered_lines(100);
        let helpers_initial = Fixture::numbered_lines(100);
        let test_initial = Fixture::numbered_lines(25);

        f.write_file("lib/core.py", &core_initial);
        f.write_file("lib/helpers.py", &helpers_initial);
        f.write_file("tests/test_core.py", &test_initial);
        f.stage_file("lib/core.py");
        f.stage_file("lib/helpers.py");
        f.stage_file("tests/test_core.py");
        f.commit("initial");

        // core.py: add at 10 and 50
        let mut core_lines: Vec<String> = (1..=100).map(|i| format!("line {}", i)).collect();
        core_lines.insert(9, "    import new_module".to_string());
        core_lines.insert(50, "    use_new_module()".to_string());
        let core_modified = core_lines.join("\n") + "\n";
        f.write_file("lib/core.py", &core_modified);

        // helpers.py: delete at 5, add at 100
        let mut helpers_lines: Vec<String> = (1..=100).map(|i| format!("line {}", i)).collect();
        helpers_lines[4] = "# Old header".to_string();
        helpers_lines.remove(4);
        helpers_lines.push("    # New footer".to_string());
        let helpers_modified = helpers_lines.join("\n") + "\n";
        f.write_file("lib/helpers.py", &helpers_modified);

        // test_core.py: add at 20-21
        let mut test_lines: Vec<String> = (1..=25).map(|i| format!("line {}", i)).collect();
        test_lines.insert(19, "    def test_new_feature():".to_string());
        test_lines.insert(20, "        assert True".to_string());
        let test_modified = test_lines.join("\n") + "\n";
        f.write_file("tests/test_core.py", &test_modified);

        insta::assert_snapshot!(
            "multi_file__multi_hunk_multi_file__diff",
            f.stager.diff(&[]).unwrap()
        );
        f.stager.stage("lib/core.py:10,51").unwrap();
        f.stager.stage("lib/helpers.py:-5,100").unwrap();
        f.stager.stage("tests/test_core.py:20,21").unwrap();
        insta::assert_snapshot!(
            "multi_file__multi_hunk_multi_file__staged",
            f.git_diff_cached()
        );
    }

    /// 5.4: Deep Directory Structure
    #[test]
    fn deep_directories() {
        let f = Fixture::new();
        let header_initial = Fixture::numbered_lines(20);
        let footer_initial = Fixture::numbered_lines(35);
        let mut format_lines: Vec<String> = (1..=15).map(|i| format!("line {}", i)).collect();
        format_lines[9] = "    oldFormat(data)".to_string();
        let format_initial = format_lines.join("\n") + "\n";

        f.write_file("src/components/Header.jsx", &header_initial);
        f.write_file("src/components/Footer.jsx", &footer_initial);
        f.write_file("src/utils/helpers/format.js", &format_initial);
        f.stage_file("src/components/Header.jsx");
        f.stage_file("src/components/Footer.jsx");
        f.stage_file("src/utils/helpers/format.js");
        f.commit("initial");

        // Add to Header
        let mut header_lines: Vec<String> = (1..=20).map(|i| format!("line {}", i)).collect();
        header_lines.insert(14, "    <NewElement />".to_string());
        let header_modified = header_lines.join("\n") + "\n";
        f.write_file("src/components/Header.jsx", &header_modified);

        // Add to Footer
        let mut footer_lines: Vec<String> = (1..=35).map(|i| format!("line {}", i)).collect();
        footer_lines.insert(29, "    <Copyright year={2024} />".to_string());
        let footer_modified = footer_lines.join("\n") + "\n";
        f.write_file("src/components/Footer.jsx", &footer_modified);

        // Replace in format.js
        format_lines[9] = "    newFormat(data)".to_string();
        let format_modified = format_lines.join("\n") + "\n";
        f.write_file("src/utils/helpers/format.js", &format_modified);

        insta::assert_snapshot!(
            "multi_file__deep_directories__diff",
            f.stager.diff(&[]).unwrap()
        );
        f.stager.stage("src/components/Header.jsx:15").unwrap();
        f.stager.stage("src/components/Footer.jsx:30").unwrap();
        f.stager
            .stage("src/utils/helpers/format.js:-10,10")
            .unwrap();
        insta::assert_snapshot!("multi_file__deep_directories__staged", f.git_diff_cached());
    }

    /// 5.5: Many Files
    #[test]
    fn many_files() {
        let f = Fixture::new();
        for i in 1..=5 {
            let initial = Fixture::numbered_lines(i);
            f.write_file(&format!("file{}.txt", i), &initial);
            f.stage_file(&format!("file{}.txt", i));
        }
        f.commit("initial");

        // Add one line to each file
        for i in 1..=5 {
            let mut lines: Vec<String> = (1..=i).map(|j| format!("line {}", j)).collect();
            lines.push(format!("change{}", i));
            let modified = lines.join("\n") + "\n";
            f.write_file(&format!("file{}.txt", i), &modified);
        }

        insta::assert_snapshot!("multi_file__many_files__diff", f.stager.diff(&[]).unwrap());
        f.stager.stage("file1.txt:2").unwrap();
        f.stager.stage("file2.txt:3").unwrap();
        f.stager.stage("file3.txt:4").unwrap();
        f.stager.stage("file4.txt:5").unwrap();
        f.stager.stage("file5.txt:6").unwrap();
        insta::assert_snapshot!("multi_file__many_files__staged", f.git_diff_cached());
    }
}

// =============================================================================
// 06: No-Newline Patches
// =============================================================================
mod no_newline {
    use super::*;

    /// 6.1: Adding After No-Newline (Auto-Bridge)
    #[test]
    fn adding_after() {
        let f = Fixture::new();
        f.write_file("config.nix", "line 1\nline 2\nno newline");
        f.stage_file("config.nix");
        f.commit("initial");

        f.write_file("config.nix", "line 1\nline 2\nno newline\nnew line");

        insta::assert_snapshot!(
            "no_newline__adding_after__diff",
            f.stager.diff(&["config.nix".to_string()]).unwrap()
        );
        f.stager.stage("config.nix:4").unwrap();
        insta::assert_snapshot!("no_newline__adding_after__staged", f.git_diff_cached());
    }

    /// 6.2: Staging Complete Change
    #[test]
    fn complete_change() {
        let f = Fixture::new();
        f.write_file("config.nix", "line 1\nline 2\nno newline");
        f.stage_file("config.nix");
        f.commit("initial");

        f.write_file("config.nix", "line 1\nline 2\nno newline\nnew line");

        insta::assert_snapshot!(
            "no_newline__complete_change__diff",
            f.stager.diff(&["config.nix".to_string()]).unwrap()
        );
        f.stager.stage("config.nix:-3,3,4").unwrap();
        insta::assert_snapshot!("no_newline__complete_change__staged", f.git_diff_cached());
    }

    /// 6.3: Bridge Only (Add Trailing Newline)
    #[test]
    fn bridge_only() {
        let f = Fixture::new();
        f.write_file("config.nix", "line 1\nline 2\nno newline");
        f.stage_file("config.nix");
        f.commit("initial");

        f.write_file("config.nix", "line 1\nline 2\nno newline\n");

        insta::assert_snapshot!(
            "no_newline__bridge_only__diff",
            f.stager.diff(&["config.nix".to_string()]).unwrap()
        );
        f.stager.stage("config.nix:-3,3").unwrap();
        insta::assert_snapshot!("no_newline__bridge_only__staged", f.git_diff_cached());
    }

    /// 6.4: Delete No-Newline Line
    #[test]
    fn delete() {
        let f = Fixture::new();
        f.write_file("config.nix", "line 1\nline 2\nno newline");
        f.stage_file("config.nix");
        f.commit("initial");

        f.write_file("config.nix", "line 1\nline 2\n");

        insta::assert_snapshot!(
            "no_newline__delete__diff",
            f.stager.diff(&["config.nix".to_string()]).unwrap()
        );
        f.stager.stage("config.nix:-3").unwrap();
        insta::assert_snapshot!("no_newline__delete__staged", f.git_diff_cached());
    }

    /// 6.5: Modify Content (Stays No-Newline)
    #[test]
    fn modify_stays() {
        let f = Fixture::new();
        f.write_file("config.nix", "line 1\nline 2\nold content");
        f.stage_file("config.nix");
        f.commit("initial");

        f.write_file("config.nix", "line 1\nline 2\nnew content");

        insta::assert_snapshot!(
            "no_newline__modify_stays__diff",
            f.stager.diff(&["config.nix".to_string()]).unwrap()
        );
        f.stager.stage("config.nix:-3,3").unwrap();
        insta::assert_snapshot!("no_newline__modify_stays__staged", f.git_diff_cached());
    }

    /// 6.6: Skip Middle Line
    #[test]
    fn skip_middle() {
        let f = Fixture::new();
        f.write_file("config.nix", "line 1\nline 2\nno newline");
        f.stage_file("config.nix");
        f.commit("initial");

        f.write_file(
            "config.nix",
            "line 1\nline 2\nno newline\nfourth line\nfifth line",
        );

        insta::assert_snapshot!(
            "no_newline__skip_middle__diff",
            f.stager.diff(&["config.nix".to_string()]).unwrap()
        );
        f.stager.stage("config.nix:5").unwrap();
        insta::assert_snapshot!("no_newline__skip_middle__staged", f.git_diff_cached());
    }

    /// 6.7: Complex Multi-Line After No-Newline
    #[test]
    fn complex_multi_line() {
        let f = Fixture::new();
        f.write_file(
            "file.txt",
            "line 1\nline 2\nline 3\nline 4\nline 5\nline 6\nline 7\nline 8\nline 9\nlast line",
        );
        f.stage_file("file.txt");
        f.commit("initial");

        f.write_file("file.txt", "line 1\nline 2\nline 3\nline 4\nline 5\nline 6\nline 7\nline 8\nline 9\nlast line\nadded one\nadded two\nadded three");

        insta::assert_snapshot!(
            "no_newline__complex_multi_line__diff",
            f.stager.diff(&["file.txt".to_string()]).unwrap()
        );
        f.stager.stage("file.txt:11,13").unwrap();
        insta::assert_snapshot!(
            "no_newline__complex_multi_line__staged",
            f.git_diff_cached()
        );
    }

    /// 6.8: No-Newline in Middle of Changes
    #[test]
    fn middle_of_changes() {
        let f = Fixture::new();
        let mut lines: Vec<String> = (1..=25).map(|i| format!("line {}", i)).collect();
        lines[19] = "middle line".to_string();
        // File ends with newline initially
        let initial = lines.join("\n") + "\n";
        f.write_file("file.txt", &initial);
        f.stage_file("file.txt");
        f.commit("initial");

        // Add at 5, replace at 20 (changing to no-newline context), add at 30
        let mut modified_lines: Vec<String> = Vec::new();
        for (i, line) in lines.iter().enumerate() {
            if i == 4 {
                modified_lines.push(line.clone());
                modified_lines.push("early addition".to_string());
            } else if i == 19 {
                modified_lines.push("middle line".to_string());
                modified_lines.push("after middle".to_string());
            } else {
                modified_lines.push(line.clone());
            }
        }
        modified_lines.push("late addition".to_string());
        let modified = modified_lines.join("\n") + "\n";
        f.write_file("file.txt", &modified);

        insta::assert_snapshot!(
            "no_newline__middle_of_changes__diff",
            f.stager.diff(&["file.txt".to_string()]).unwrap()
        );
        f.stager.stage("file.txt:6,22,28").unwrap();
        insta::assert_snapshot!("no_newline__middle_of_changes__staged", f.git_diff_cached());
    }
}

// =============================================================================
// Behavioral Tests
// =============================================================================
mod behavior {
    use super::*;

    /// Line Number Stability: Verify line numbers remain valid after partial staging
    #[test]
    fn line_number_stability() {
        let f = Fixture::new();
        let initial = Fixture::numbered_lines(10);
        f.write_file("config.nix", &initial);
        f.stage_file("config.nix");
        f.commit("initial");

        let mut lines: Vec<String> = (1..=10).map(|i| format!("line {}", i)).collect();
        lines.insert(2, "# FIRST INSERTION".to_string());
        lines.insert(9, "# SECOND INSERTION".to_string());
        let modified = lines.join("\n") + "\n";
        f.write_file("config.nix", &modified);

        insta::assert_snapshot!(
            "behavior__line_number_stability__diff",
            f.stager.diff(&["config.nix".to_string()]).unwrap()
        );

        // Stage later hunk first, then earlier
        f.stager.stage("config.nix:10").unwrap();
        f.stager.stage("config.nix:3").unwrap();
        insta::assert_snapshot!(
            "behavior__line_number_stability__staged",
            f.git_diff_cached()
        );
    }
}
