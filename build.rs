use std::env;
use std::fs;
use std::path::Path;

fn main() {
    set_git_revision_hash();
    gen_use_plugins_file();
}

fn gen_use_plugins_file() {
    // 获取项目根目录
    let out_dir = env::var_os("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("__use_node_plugins.rs");

    // 扫描 plugins 目录
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

    // 生成 use_plugins.rs 文件内容
    let mut file_content = String::new();
    for plugin_name in plugin_names {
        file_content.push_str(&format!("extern crate {};\n", plugin_name));
    }

    // 写入文件
    fs::write(&dest_path, file_content).unwrap();

    println!("cargo:rerun-if-changed=node-plugins");
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
