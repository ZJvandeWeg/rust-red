use std::env;
use std::fs;
use std::path::Path;
use std::process::Command;

fn main() {
    set_git_revision_hash();
    check_patch();
    gen_use_plugins_file();
}

fn gen_use_plugins_file() {
    let out_dir = env::var_os("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("__use_node_plugins.rs");

    let plugins_dir = Path::new("node-plugins");
    let mut plugin_names = Vec::new();

    if plugins_dir.is_dir() {
        for entry in fs::read_dir(plugins_dir).unwrap() {
            let entry = entry.unwrap();
            if entry.path().is_dir() {
                let plugin_name = entry.file_name().to_string_lossy().replace("-", "_");
                plugin_names.push(plugin_name);
            }
        }
    }

    let mut file_content = String::new();
    for plugin_name in plugin_names {
        file_content.push_str(&format!("extern crate {};\n", plugin_name));
    }

    fs::write(&dest_path, file_content).unwrap();

    println!("cargo:rerun-if-changed=node-plugins");
}

/// Make the current git hash available to the build as the environment
/// variable `EDGELINK_BUILD_GIT_HASH`.
fn set_git_revision_hash() {
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

fn check_patch() {
    if env::consts::OS == "windows" {
        let output = Command::new("patch.exe")
            .arg("--version")
            .output()
            .expect("Failed to execute `patch.exe --version`, the GNU Patch program is required to build this project");

        if output.status.success() {
            let version_info = String::from_utf8_lossy(&output.stdout);
            let first_line = version_info.lines().next().unwrap_or("Unknown version");
            if !first_line.to_lowercase().contains("patch") {
                eprintln!("Error: The Patch program is required to build this project, but got: {}", first_line);
                std::process::exit(1);
            }
        } else {
            let error_info = String::from_utf8_lossy(&output.stderr);
            eprintln!("Error: Failed to get patch.exe version: {}", error_info);
            std::process::exit(1);
        }
    }
}
