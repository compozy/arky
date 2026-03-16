//! Regression tests for the workspace dependency graph policy.

use std::{
    path::PathBuf,
    process::Command,
};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .expect("crate manifest should sit under the repo root")
        .to_path_buf()
}

fn check_deps_script() -> PathBuf {
    repo_root().join("scripts").join("check-deps.sh")
}

fn fixture(name: &str) -> PathBuf {
    repo_root().join("scripts").join("fixtures").join(name)
}

#[test]
fn dependency_graph_script_should_accept_clean_fixture() {
    let output = Command::new("sh")
        .arg(check_deps_script())
        .arg("--metadata-file")
        .arg(fixture("deps-clean.json"))
        .output()
        .expect("dependency graph script should run");

    assert!(
        output.status.success(),
        "expected clean fixture to pass, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn dependency_graph_script_should_detect_intentional_cycle() {
    let output = Command::new("sh")
        .arg(check_deps_script())
        .arg("--metadata-file")
        .arg(fixture("deps-cycle.json"))
        .output()
        .expect("dependency graph script should run");

    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("dependency cycle detected"),
        "stderr should mention the cycle: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn dependency_graph_script_should_reject_leaf_violations() {
    let output = Command::new("sh")
        .arg(check_deps_script())
        .arg("--metadata-file")
        .arg(fixture("deps-leaf-violation.json"))
        .output()
        .expect("dependency graph script should run");

    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("disallowed internal crates"),
        "stderr should mention the violation: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
