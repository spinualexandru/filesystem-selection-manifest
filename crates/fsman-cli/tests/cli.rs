use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT_TEMP_DIRECTORY: AtomicUsize = AtomicUsize::new(0);

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        let unique = NEXT_TEMP_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "fsman-cli-integration-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn create_file(&self, relative: &str) {
        let path = self.0.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, "test").unwrap();
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn fsman_cli() -> Command {
    Command::new(env!("CARGO_BIN_EXE_fsman-cli"))
}

#[test]
fn accepts_a_valid_manifest() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/basic.fsman");
    let output = fsman_cli().arg(&manifest).output().unwrap();

    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("is valid"));
    assert!(output.stderr.is_empty());
}

#[test]
fn rejects_an_invalid_manifest() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/invalid.fsman");
    let output = fsman_cli().arg(&manifest).output().unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("is not a valid fsman file"));
}

#[test]
fn requires_exactly_one_file() {
    let output = fsman_cli().output().unwrap();

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("usage: fsman-cli <FILE>"));
}

#[test]
fn resolves_a_manifest_as_a_tree() {
    let directory = TestDirectory::new();
    directory.create_file("config/one.txt");
    directory.create_file("config/nested/two.txt");
    directory.create_file("config/skip.txt");
    let manifest = directory.0.join("selection.fsman");
    fs::write(&manifest, "config {\n  **\n  !skip.txt\n}\n").unwrap();

    let output = fsman_cli()
        .args(["resolve", manifest.to_str().unwrap(), "--cwd"])
        .arg(&directory.0)
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        format!(
            "{}\n└── config\n    ├── nested\n    │   └── two.txt\n    └── one.txt\n",
            directory.0.display()
        )
    );
}

#[test]
fn limits_recursive_resolution_depth() {
    let directory = TestDirectory::new();
    directory.create_file("one/two/three.txt");
    let manifest = directory.0.join("selection.fsman");
    fs::write(&manifest, "**\n!selection.fsman\n").unwrap();

    let output = fsman_cli()
        .args([
            "resolve",
            manifest.to_str().unwrap(),
            "--depth",
            "2",
            "--cwd",
        ])
        .arg(&directory.0)
        .output()
        .unwrap();

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        format!("{}\n└── one\n    └── two\n", directory.0.display())
    );
}

#[test]
fn rejects_an_invalid_depth() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/basic.fsman");
    let output = fsman_cli()
        .args(["resolve", manifest.to_str().unwrap(), "--depth", "all"])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("invalid depth"));
}

#[test]
fn short_output_collapses_a_complete_recursive_selection() {
    let directory = TestDirectory::new();
    directory.create_file(".config/hypr/hyprland.conf");
    directory.create_file(".config/hypr/nested/rules.conf");
    let manifest = directory.0.join("selection.fsman");
    fs::write(&manifest, ".config {\n  hypr {\n    ***\n  }\n}\n").unwrap();

    let output = fsman_cli()
        .args(["resolve", manifest.to_str().unwrap(), "--short", "--cwd"])
        .arg(&directory.0)
        .output()
        .unwrap();

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        format!("{}/\n└── .config/hypr/\n", directory.0.display())
    );
}

#[test]
fn short_output_lists_immediate_paths_when_recursion_has_an_exclusion() {
    let directory = TestDirectory::new();
    directory.create_file(".config/hypr/AGENTS.md");
    directory.create_file(".config/hypr/hyprland.conf");
    directory.create_file(".config/hypr/nested/rules.conf");
    let manifest = directory.0.join("selection.fsman");
    fs::write(
        &manifest,
        ".config {\n  hypr {\n    ***\n    !AGENTS.md\n  }\n}\n",
    )
    .unwrap();

    let output = fsman_cli()
        .args(["resolve", manifest.to_str().unwrap(), "--cwd"])
        .arg(&directory.0)
        .arg("--short")
        .output()
        .unwrap();

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        format!(
            "{}/\n├── .config/hypr/hyprland.conf\n└── .config/hypr/nested/\n",
            directory.0.display()
        )
    );
}

#[test]
fn short_output_keeps_a_depth_limited_selection_expanded() {
    let directory = TestDirectory::new();
    directory.create_file(".config/hypr/one/two/rules.conf");
    let manifest = directory.0.join("selection.fsman");
    fs::write(&manifest, ".config {\n  hypr {\n    ***\n  }\n}\n").unwrap();

    let output = fsman_cli()
        .args([
            "resolve",
            manifest.to_str().unwrap(),
            "--short",
            "--depth",
            "2",
            "--cwd",
        ])
        .arg(&directory.0)
        .output()
        .unwrap();

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        format!("{}/\n└── .config/hypr/one/\n", directory.0.display())
    );
}

#[test]
fn flat_output_prints_compact_absolute_paths_without_tree_glyphs() {
    let directory = TestDirectory::new();
    directory.create_file(".config/hypr/AGENTS.md");
    directory.create_file(".config/hypr/hyprland.conf");
    directory.create_file(".config/hypr/nested/rules.conf");
    let manifest = directory.0.join("selection.fsman");
    fs::write(
        &manifest,
        ".config {\n  hypr {\n    ***\n    !AGENTS.md\n  }\n}\n",
    )
    .unwrap();

    let output = fsman_cli()
        .args(["resolve", manifest.to_str().unwrap(), "--flat", "--cwd"])
        .arg(&directory.0)
        .output()
        .unwrap();

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        format!(
            "{}/.config/hypr/hyprland.conf\n{}/.config/hypr/nested/\n",
            directory.0.display(),
            directory.0.display()
        )
    );
}

#[test]
fn json_output_preserves_the_resolved_hierarchy() {
    let directory = TestDirectory::new();
    directory.create_file("folder/nested/file.txt");
    let manifest = directory.0.join("selection.fsman");
    fs::write(&manifest, "folder {\n  **\n}\n").unwrap();

    let output = fsman_cli()
        .args(["resolve", manifest.to_str().unwrap(), "--json", "--cwd"])
        .arg(&directory.0)
        .output()
        .unwrap();

    assert!(output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["path"], directory.0.to_string_lossy().as_ref());
    assert_eq!(value["type"], "directory");
    assert_eq!(
        value["children"][0]["children"][0]["children"][0]["path"],
        directory
            .0
            .join("folder/nested/file.txt")
            .to_string_lossy()
            .as_ref()
    );
}

#[test]
fn json_flat_output_is_an_array_of_compact_paths() {
    let directory = TestDirectory::new();
    directory.create_file("folder/keep.txt");
    directory.create_file("folder/nested/file.txt");
    directory.create_file("folder/skip.txt");
    let manifest = directory.0.join("selection.fsman");
    fs::write(&manifest, "folder {\n  ***\n  !skip.txt\n}\n").unwrap();

    let output = fsman_cli()
        .args([
            "resolve",
            manifest.to_str().unwrap(),
            "--json",
            "--flat",
            "--cwd",
        ])
        .arg(&directory.0)
        .output()
        .unwrap();

    assert!(output.status.success());
    let paths: Vec<String> = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        paths,
        vec![
            directory.0.join("folder/keep.txt").display().to_string(),
            format!("{}/", directory.0.join("folder/nested").display()),
        ]
    );
}

#[test]
fn git_aware_recursion_respects_local_and_global_git_ignores() {
    let directory = TestDirectory::new();
    let workspace = directory.0.join("workspace");
    let home = directory.0.join("home");
    fs::create_dir_all(workspace.join(".git")).unwrap();
    fs::create_dir_all(home.join(".config/git")).unwrap();
    fs::write(workspace.join(".gitignore"), "local.txt\n").unwrap();
    fs::write(home.join(".config/git/ignore"), "global.txt\n").unwrap();
    fs::write(workspace.join("local.txt"), "test").unwrap();
    fs::write(workspace.join("global.txt"), "test").unwrap();
    fs::write(workspace.join("keep.txt"), "test").unwrap();
    let manifest = directory.0.join("selection.fsman");
    fs::write(&manifest, "**\n").unwrap();

    let output = fsman_cli()
        .args(["resolve", manifest.to_str().unwrap(), "--flat", "--cwd"])
        .arg(&workspace)
        .env("HOME", &home)
        .env_remove("XDG_CONFIG_HOME")
        .env_remove("GIT_CONFIG_GLOBAL")
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("keep.txt"));
    assert!(stdout.contains(".gitignore"));
    assert!(!stdout.contains("local.txt"));
    assert!(!stdout.contains("global.txt"));
    assert!(!stdout.contains("/.git/"));
}
