fn main() {
    set_git_revision_hash();
}

/// Make the current git hash available to the build as the environment
/// variable `EDGELINK_BUILD_GIT_HASH`.
fn set_git_revision_hash() {
    use std::process::Command;

    let args = &["rev-parse", "--short=10", "HEAD"];
    let Ok(output) = Command::new("git").args(args).output() else {
        return;
    };
    let rev = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if rev.is_empty() {
        return;
    }
    println!("cargo:rustc-env=EDGELINK_BUILD_GIT_HASH={}", rev);
}
