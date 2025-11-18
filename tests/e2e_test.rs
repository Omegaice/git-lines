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
    stager: GitStager<'static>,
}

impl Fixture {
    /// Create a new empty repo with deterministic config
    fn new(_test_name: &str) -> Self {
        let dir = TempDir::new().expect("Failed to create temp dir");
        let repo = Repository::init(dir.path()).expect("Failed to init repo");

        // Deterministic config
        let mut config = repo.config().unwrap();
        config.set_str("user.name", "Test User").unwrap();
        config.set_str("user.email", "test@example.com").unwrap();

        // Leak the path string to get 'static lifetime
        let path_str: &'static str =
            Box::leak(dir.path().to_str().unwrap().to_string().into_boxed_str());
        let stager = GitStager::new(path_str);

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
    fn git_diff_cached_all(&self) -> String {
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
}

// =============================================================================
// Case 1: Single Line Addition
// =============================================================================

#[test]
fn case_01_single_addition() {
    let fixture = Fixture::new("case_01");
    let initial_content = (1..=136)
        .map(|i| format!("line {}", i))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    fixture.write_file("flake.nix", &initial_content);
    fixture.stage_file("flake.nix");
    fixture.commit("initial");

    // Modifications
    fixture.write_file("flake.nix", &(initial_content + "      debug = true;\n"));

    // Snapshot, stage, verify
    insta::assert_snapshot!(
        "case_01_diff",
        fixture.stager.diff(&["flake.nix".to_string()]).unwrap()
    );
    fixture.stager.stage("flake.nix:137").unwrap();
    insta::assert_snapshot!("case_01_staged", fixture.git_diff_cached_all());
}

// =============================================================================
// Case 2: Contiguous Line Additions (Range)
// =============================================================================

#[test]
fn case_02_contiguous_additions() {
    let fixture = Fixture::new("case_02");
    let initial_content = (1..=38)
        .map(|i| format!("line {}", i))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    fixture.write_file("flake.nix", &initial_content);
    fixture.stage_file("flake.nix");
    fixture.commit("initial");

    // Modifications
    let addition = r#"
    stylix = {
      url = "github:nix-community/stylix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
"#;
    fixture.write_file("flake.nix", &(initial_content + addition));

    // Snapshot, stage, verify
    insta::assert_snapshot!(
        "case_02_diff",
        fixture.stager.diff(&["flake.nix".to_string()]).unwrap()
    );
    fixture.stager.stage("flake.nix:39..43").unwrap();
    insta::assert_snapshot!("case_02_staged", fixture.git_diff_cached_all());
}

// =============================================================================
// Case 3: Non-Contiguous Additions (Skip Some Lines)
// =============================================================================

#[test]
fn case_03_non_contiguous_additions() {
    let fixture = Fixture::new("case_03");
    let initial_content = (1..=39)
        .map(|i| format!("        line {}", i))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    fixture.write_file("default.nix", &initial_content);
    fixture.stage_file("default.nix");
    fixture.commit("initial");

    // Modifications
    let additions = r#"        # Allow Stylix to override terminal font
        "terminal.integrated.fontFamily" = lib.mkDefault "monospace";
        "direnv.restart.automatic" = true;
"#;
    fixture.write_file("default.nix", &(initial_content + additions));

    // Snapshot, stage, verify
    insta::assert_snapshot!(
        "case_03_diff",
        fixture.stager.diff(&["default.nix".to_string()]).unwrap()
    );
    fixture.stager.stage("default.nix:40..41").unwrap();
    insta::assert_snapshot!("case_03_staged", fixture.git_diff_cached_all());
}

// =============================================================================
// Case 4: Deletion
// =============================================================================

#[test]
fn case_04_deletion() {
    let fixture = Fixture::new("case_04");
    let initial_content = (1..=20)
        .map(|i| {
            if i == 15 {
                "      enableAutosuggestions = true;".to_string()
            } else {
                format!("line {}", i)
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    fixture.write_file("zsh.nix", &initial_content);
    fixture.stage_file("zsh.nix");
    fixture.commit("initial");

    // Modifications
    let modified_content = (1..=20)
        .filter(|&i| i != 15)
        .map(|i| format!("line {}", i))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    fixture.write_file("zsh.nix", &modified_content);

    // Snapshot, stage, verify
    insta::assert_snapshot!(
        "case_04_diff",
        fixture.stager.diff(&["zsh.nix".to_string()]).unwrap()
    );
    fixture.stager.stage("zsh.nix:-15").unwrap();
    insta::assert_snapshot!("case_04_staged", fixture.git_diff_cached_all());
}

// =============================================================================
// Case 5: Selective Staging from Mixed Add/Delete
// =============================================================================

#[test]
fn case_05_selective_from_mixed() {
    let fixture = Fixture::new("case_05");
    let initial_content = [
        "line 1",
        "line 2",
        "line 3",
        "line 4",
        "line 5",
        "line 6",
        "line 7",
        "line 8",
        "line 9",
        "    gtk.theme.name = \"Adwaita\";",
        "    gtk.iconTheme.name = \"Papirus\";",
        "line 12",
    ]
    .join("\n")
        + "\n";
    fixture.write_file("gtk.nix", &initial_content);
    fixture.stage_file("gtk.nix");
    fixture.commit("initial");

    // Modifications
    let modified_content = [
        "line 1",
        "line 2",
        "line 3",
        "line 4",
        "line 5",
        "line 6",
        "line 7",
        "line 8",
        "line 9",
        "    # Theme managed by Stylix",
        "    gtk.iconTheme.name = \"Papirus-Dark\";",
        "    gtk.cursorTheme.size = 24;",
        "line 12",
    ]
    .join("\n")
        + "\n";
    fixture.write_file("gtk.nix", &modified_content);

    // Snapshot, stage, verify
    insta::assert_snapshot!(
        "case_05_diff",
        fixture.stager.diff(&["gtk.nix".to_string()]).unwrap()
    );
    fixture.stager.stage("gtk.nix:12").unwrap();
    insta::assert_snapshot!("case_05_staged", fixture.git_diff_cached_all());
}

// =============================================================================
// Case 6: Multiple Files in Single Command
// =============================================================================

#[test]
fn case_06_multiple_files() {
    let fixture = Fixture::new("case_06");
    let flake_initial = (1..=136)
        .map(|i| format!("line {}", i))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    let gtk_initial = (1..=11)
        .map(|i| format!("line {}", i))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    fixture.write_file("flake.nix", &flake_initial);
    fixture.write_file("gtk.nix", &gtk_initial);
    fixture.stage_file("flake.nix");
    fixture.stage_file("gtk.nix");
    fixture.commit("initial");

    // Modifications
    fixture.write_file("flake.nix", &(flake_initial + "      debug = true;\n"));
    fixture.write_file(
        "gtk.nix",
        &(gtk_initial + "    gtk.cursorTheme.size = 24;\n"),
    );

    // Snapshot, stage, verify
    insta::assert_snapshot!("case_06_diff", fixture.stager.diff(&[]).unwrap());
    fixture.stager.stage("flake.nix:137").unwrap();
    fixture.stager.stage("gtk.nix:12").unwrap();
    insta::assert_snapshot!("case_06_staged", fixture.git_diff_cached_all());
}

// =============================================================================
// Case 7: Deletion Range
// =============================================================================

#[test]
fn case_07_deletion_range() {
    let fixture = Fixture::new("case_07");
    let initial_content = (1..=20)
        .map(|i| match i {
            15 => "      enableAutosuggestions = true;".to_string(),
            16 => "      enableCompletion = true;".to_string(),
            17 => "      enableSyntaxHighlighting = true;".to_string(),
            _ => format!("line {}", i),
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    fixture.write_file("zsh.nix", &initial_content);
    fixture.stage_file("zsh.nix");
    fixture.commit("initial");

    // Modifications
    let modified_content = (1..=20)
        .filter(|&i| !(15..=17).contains(&i))
        .map(|i| format!("line {}", i))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    fixture.write_file("zsh.nix", &modified_content);

    // Snapshot, stage, verify
    insta::assert_snapshot!(
        "case_07_diff",
        fixture.stager.diff(&["zsh.nix".to_string()]).unwrap()
    );
    fixture.stager.stage("zsh.nix:-15..-17").unwrap();
    insta::assert_snapshot!("case_07_staged", fixture.git_diff_cached_all());
}

// =============================================================================
// Case 8: Multiple Hunks in Same File
// =============================================================================

#[test]
fn case_08_multiple_hunks_same_file() {
    let fixture = Fixture::new("case_08");
    let initial_content = (1..=141)
        .map(|i| format!("line {}", i))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    fixture.write_file("flake.nix", &initial_content);
    fixture.stage_file("flake.nix");
    fixture.commit("initial");

    // Modifications
    let mut modified_lines: Vec<String> = Vec::new();
    for i in 1..=141 {
        modified_lines.push(format!("line {}", i));
        if i == 6 {
            modified_lines.push(
                "    determinate.url = \"github:DeterminateSystems/determinate\";".to_string(),
            );
        }
        if i == 136 {
            modified_lines.push("      debug = true;".to_string());
        }
        if i == 140 {
            modified_lines.push("        ./flake-modules/home-manager.nix".to_string());
        }
    }
    let modified_content = modified_lines.join("\n") + "\n";
    fixture.write_file("flake.nix", &modified_content);

    // Snapshot, stage, verify
    insta::assert_snapshot!(
        "case_08_diff",
        fixture.stager.diff(&["flake.nix".to_string()]).unwrap()
    );
    fixture.stager.stage("flake.nix:7,143").unwrap();
    insta::assert_snapshot!("case_08_staged", fixture.git_diff_cached_all());
}

// =============================================================================
// Case 9: Out-of-Order Sequential Staging (Later Hunk First)
// =============================================================================

#[test]
fn case_09_out_of_order_sequential_staging() {
    let fixture = Fixture::new("case_09");
    let initial_content = (1..=10)
        .map(|i| format!("line {}", i))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    fixture.write_file("config.nix", &initial_content);
    fixture.stage_file("config.nix");
    fixture.commit("initial");

    // Modifications
    let mut modified_lines: Vec<String> = Vec::new();
    for i in 1..=10 {
        modified_lines.push(format!("line {}", i));
        if i == 2 {
            modified_lines.push("# FIRST INSERTION".to_string());
        }
        if i == 8 {
            modified_lines.push("# SECOND INSERTION".to_string());
        }
    }
    let modified_content = modified_lines.join("\n") + "\n";
    fixture.write_file("config.nix", &modified_content);

    // Snapshot, stage (later first), then stage (earlier)
    insta::assert_snapshot!(
        "case_09_diff",
        fixture.stager.diff(&["config.nix".to_string()]).unwrap()
    );
    fixture.stager.stage("config.nix:10").unwrap();
    fixture.stager.stage("config.nix:3").unwrap();
    insta::assert_snapshot!("case_09_staged", fixture.git_diff_cached_all());
}

// =============================================================================
// Case 10: Single Command vs Sequential (Comparison)
// =============================================================================

#[test]
fn case_10_single_command_multiple_hunks() {
    let fixture = Fixture::new("case_10");
    let initial_content = (1..=10)
        .map(|i| format!("line {}", i))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    fixture.write_file("config.nix", &initial_content);
    fixture.stage_file("config.nix");
    fixture.commit("initial");

    // Modifications
    let mut modified_lines: Vec<String> = Vec::new();
    for i in 1..=10 {
        modified_lines.push(format!("line {}", i));
        if i == 2 {
            modified_lines.push("# FIRST INSERTION".to_string());
        }
        if i == 8 {
            modified_lines.push("# SECOND INSERTION".to_string());
        }
    }
    let modified_content = modified_lines.join("\n") + "\n";
    fixture.write_file("config.nix", &modified_content);

    // Snapshot, stage, verify
    insta::assert_snapshot!(
        "case_10_diff",
        fixture.stager.diff(&["config.nix".to_string()]).unwrap()
    );
    fixture.stager.stage("config.nix:10,3").unwrap();
    insta::assert_snapshot!("case_10_staged", fixture.git_diff_cached_all());
}

// =============================================================================
// Case 11: Files Without Trailing Newline
// =============================================================================

#[test]
fn case_11_a_partial_stage() {
    let fixture = Fixture::new("case_11_a");
    // File without trailing newline
    fixture.write_file("config.nix", "line 1\nline 2\nno newline");
    fixture.stage_file("config.nix");
    fixture.commit("initial");

    // Add lines after the no-newline line
    fixture.write_file("config.nix", "line 1\nline 2\nno newline\nnew line");

    // Stage only the new line - git-stager auto-synthesizes bridge (-3,+3)
    insta::assert_snapshot!(
        "case_11_a_diff",
        fixture.stager.diff(&["config.nix".to_string()]).unwrap()
    );
    fixture.stager.stage("config.nix:4").unwrap();
    insta::assert_snapshot!("case_11_a_staged", fixture.git_diff_cached_all());
}

#[test]
fn case_11_b_stage_all() {
    let fixture = Fixture::new("case_11_b");
    fixture.write_file("config.nix", "line 1\nline 2\nno newline");
    fixture.stage_file("config.nix");
    fixture.commit("initial");

    fixture.write_file("config.nix", "line 1\nline 2\nno newline\nnew line");

    insta::assert_snapshot!(
        "case_11_b_diff",
        fixture.stager.diff(&["config.nix".to_string()]).unwrap()
    );
    // Stage everything explicitly: -3 (delete old), +3 (add with \n), +4 (add new line)
    fixture.stager.stage("config.nix:-3,3,4").unwrap();
    insta::assert_snapshot!("case_11_b_staged", fixture.git_diff_cached_all());
}

#[test]
fn case_11_c_delete() {
    let fixture = Fixture::new("case_11_c");
    fixture.write_file("config.nix", "line 1\nline 2\nno newline");
    fixture.stage_file("config.nix");
    fixture.commit("initial");

    // Delete the last line (which had no newline)
    fixture.write_file("config.nix", "line 1\nline 2\n");

    insta::assert_snapshot!(
        "case_11_c_diff",
        fixture.stager.diff(&["config.nix".to_string()]).unwrap()
    );
    fixture.stager.stage("config.nix:-3").unwrap();
    insta::assert_snapshot!("case_11_c_staged", fixture.git_diff_cached_all());
}

#[test]
fn case_11_d_modify_content() {
    let fixture = Fixture::new("case_11_d");
    fixture.write_file("config.nix", "line 1\nline 2\nold content");
    fixture.stage_file("config.nix");
    fixture.commit("initial");

    // Change content but keep no trailing newline
    fixture.write_file("config.nix", "line 1\nline 2\nnew content");

    insta::assert_snapshot!(
        "case_11_d_diff",
        fixture.stager.diff(&["config.nix".to_string()]).unwrap()
    );
    fixture.stager.stage("config.nix:-3,3").unwrap();
    insta::assert_snapshot!("case_11_d_staged", fixture.git_diff_cached_all());
}

#[test]
fn case_11_e_bridge_only() {
    let fixture = Fixture::new("case_11_e");
    fixture.write_file("config.nix", "line 1\nline 2\nno newline");
    fixture.stage_file("config.nix");
    fixture.commit("initial");

    // Add multiple lines after no-newline line
    fixture.write_file(
        "config.nix",
        "line 1\nline 2\nno newline\nfourth line\nfifth line",
    );

    insta::assert_snapshot!(
        "case_11_e_diff",
        fixture.stager.diff(&["config.nix".to_string()]).unwrap()
    );
    // Stage only the bridge (gives no-newline line its \n) but not the actual additions
    fixture.stager.stage("config.nix:-3,3").unwrap();
    insta::assert_snapshot!("case_11_e_staged", fixture.git_diff_cached_all());
}

#[test]
fn case_11_f_skip_middle() {
    let fixture = Fixture::new("case_11_f");
    fixture.write_file("config.nix", "line 1\nline 2\nno newline");
    fixture.stage_file("config.nix");
    fixture.commit("initial");

    // Add multiple lines after no-newline line
    fixture.write_file(
        "config.nix",
        "line 1\nline 2\nno newline\nfourth line\nfifth line",
    );

    insta::assert_snapshot!(
        "case_11_f_diff",
        fixture.stager.diff(&["config.nix".to_string()]).unwrap()
    );
    // Stage only line 5, skipping line 4 - git-stager auto-synthesizes bridge
    fixture.stager.stage("config.nix:5").unwrap();
    insta::assert_snapshot!("case_11_f_staged", fixture.git_diff_cached_all());
}

#[test]
fn case_11_g_add_trailing_newline() {
    let fixture = Fixture::new("case_11_g");
    fixture.write_file("config.nix", "line 1\nline 2\nno newline");
    fixture.stage_file("config.nix");
    fixture.commit("initial");

    // Add trailing newline to file (no new lines, just the \n)
    fixture.write_file("config.nix", "line 1\nline 2\nno newline\n");

    insta::assert_snapshot!(
        "case_11_g_diff",
        fixture.stager.diff(&["config.nix".to_string()]).unwrap()
    );
    fixture.stager.stage("config.nix:-3,3").unwrap();
    insta::assert_snapshot!("case_11_g_staged", fixture.git_diff_cached_all());
}
