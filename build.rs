use std::process::Command;

fn main() {
    // Only re-run if build.rs itself changes
    println!("cargo:rerun-if-changed=build.rs");

    // Set git hooks path to .githooks/ so pre-commit hook is automatically active
    let _ = Command::new("git")
        .args(["config", "core.hooksPath", ".githooks"])
        .status();
}
