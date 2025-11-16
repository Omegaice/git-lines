use git2::{Repository, Signature};
use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

/// Test fixture for a git repository
struct Fixture {
    dir: TempDir,
    repo: Repository,
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

        Self { dir, repo }
    }

    fn path(&self) -> &Path {
        self.dir.path()
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

    /// Get git diff output (unstaged changes)
    fn git_diff(&self, file: &str) -> String {
        let output = Command::new("git")
            .args([
                "-C",
                self.path().to_str().unwrap(),
                "diff",
                "--no-ext-diff", // Force standard diff, ignore external tools
                "-U0",
                "--no-color",
                file,
            ])
            .output()
            .expect("Failed to run git diff");
        String::from_utf8(output.stdout).unwrap()
    }

    /// Get git diff --cached output (staged changes)
    fn git_diff_cached(&self, file: &str) -> String {
        let output = Command::new("git")
            .args([
                "-C",
                self.path().to_str().unwrap(),
                "diff",
                "--cached",
                "--no-ext-diff", // Force standard diff, ignore external tools
                "-U0",
                "--no-color",
                file,
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
    let fixture = Fixture::new();

    // Create initial file with specific line count
    // We want line 137 to be the added line
    let initial_lines: Vec<String> = (1..=136).map(|i| format!("line {}", i)).collect();
    let initial_content = initial_lines.join("\n") + "\n";
    fixture.write_file("flake.nix", &initial_content);
    fixture.stage_file("flake.nix");
    fixture.commit("initial");

    // Add a single line (becomes line 137)
    let modified_content = initial_content.clone() + "      debug = true;\n";
    fixture.write_file("flake.nix", &modified_content);

    // Verify the diff shows what we expect
    let diff = fixture.git_diff("flake.nix");
    insta::assert_snapshot!("case_01_initial_diff", diff);

    // TODO: Run git-stager stage flake.nix:137
    // TODO: Verify staged diff
    // TODO: Verify remaining unstaged diff
}

// =============================================================================
// Case 2: Contiguous Line Additions (Range)
// =============================================================================

#[test]
fn case_02_contiguous_additions() {
    let fixture = Fixture::new();

    // Create initial file
    let initial_lines: Vec<String> = (1..=38).map(|i| format!("line {}", i)).collect();
    let initial_content = initial_lines.join("\n") + "\n";
    fixture.write_file("flake.nix", &initial_content);
    fixture.stage_file("flake.nix");
    fixture.commit("initial");

    // Add 5 lines (39-43)
    let addition = r#"
    stylix = {
      url = "github:nix-community/stylix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
"#;
    let modified_content = initial_content.clone() + addition;
    fixture.write_file("flake.nix", &modified_content);

    let diff = fixture.git_diff("flake.nix");
    insta::assert_snapshot!("case_02_initial_diff", diff);

    // TODO: Run git-stager stage flake.nix:39..43
}

// =============================================================================
// Case 3: Non-Contiguous Additions (Skip Some Lines)
// =============================================================================

#[test]
fn case_03_non_contiguous_additions() {
    let fixture = Fixture::new();

    // Create initial file with 39 lines
    let initial_lines: Vec<String> = (1..=39).map(|i| format!("        line {}", i)).collect();
    let initial_content = initial_lines.join("\n") + "\n";
    fixture.write_file("default.nix", &initial_content);
    fixture.stage_file("default.nix");
    fixture.commit("initial");

    // Add 3 adjacent lines (40, 41, 42)
    // Line 40-41: Stylix (want to stage)
    // Line 42: Direnv (want to skip)
    let additions = r#"        # Allow Stylix to override terminal font
        "terminal.integrated.fontFamily" = lib.mkDefault "monospace";
        "direnv.restart.automatic" = true;
"#;
    let modified_content = initial_content.clone() + additions;
    fixture.write_file("default.nix", &modified_content);

    let diff = fixture.git_diff("default.nix");
    insta::assert_snapshot!("case_03_initial_diff", diff);

    // TODO: Run git-stager stage default.nix:40..41
    // Should stage lines 40-41, leave line 42 unstaged
}

// =============================================================================
// Case 4: Deletion
// =============================================================================

#[test]
fn case_04_deletion() {
    let fixture = Fixture::new();

    // Create initial file with a line we'll delete
    let initial_lines: Vec<String> = (1..=20)
        .map(|i| {
            if i == 15 {
                "      enableAutosuggestions = true;".to_string()
            } else {
                format!("line {}", i)
            }
        })
        .collect();
    let initial_content = initial_lines.join("\n") + "\n";
    fixture.write_file("zsh.nix", &initial_content);
    fixture.stage_file("zsh.nix");
    fixture.commit("initial");

    // Delete line 15
    let modified_lines: Vec<String> = (1..=20)
        .filter(|&i| i != 15)
        .map(|i| {
            if i == 15 {
                "      enableAutosuggestions = true;".to_string()
            } else {
                format!("line {}", i)
            }
        })
        .collect();
    let modified_content = modified_lines.join("\n") + "\n";
    fixture.write_file("zsh.nix", &modified_content);

    let diff = fixture.git_diff("zsh.nix");
    insta::assert_snapshot!("case_04_initial_diff", diff);

    // TODO: Run git-stager stage zsh.nix:-15
}

// =============================================================================
// Case 5: Selective Staging from Mixed Add/Delete
// =============================================================================

#[test]
fn case_05_selective_from_mixed() {
    let fixture = Fixture::new();

    // Create initial file
    let initial_lines = vec![
        "line 1",
        "line 2",
        "line 3",
        "line 4",
        "line 5",
        "line 6",
        "line 7",
        "line 8",
        "line 9",
        "    gtk.theme.name = \"Adwaita\";", // line 10 - will delete
        "    gtk.iconTheme.name = \"Papirus\";", // line 11 - will modify
        "line 12",
    ];
    let initial_content = initial_lines.join("\n") + "\n";
    fixture.write_file("gtk.nix", &initial_content);
    fixture.stage_file("gtk.nix");
    fixture.commit("initial");

    // Modify: delete line 10, change line 11, add line 12 (cursor size)
    let modified_lines = vec![
        "line 1",
        "line 2",
        "line 3",
        "line 4",
        "line 5",
        "line 6",
        "line 7",
        "line 8",
        "line 9",
        // line 10 deleted
        "    # Theme managed by Stylix",              // new line 10
        "    gtk.iconTheme.name = \"Papirus-Dark\";", // line 11 (modified)
        "    gtk.cursorTheme.size = 24;",             // line 12 (new)
        "line 12",                                    // line 13 (was 12)
    ];
    let modified_content = modified_lines.join("\n") + "\n";
    fixture.write_file("gtk.nix", &modified_content);

    let diff = fixture.git_diff("gtk.nix");
    insta::assert_snapshot!("case_05_initial_diff", diff);

    // TODO: Run git-stager stage gtk.nix:12
    // Should stage only the cursor size line, not deletions or other additions
}
