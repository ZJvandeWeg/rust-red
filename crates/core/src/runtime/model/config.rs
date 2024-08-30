use clap::{Parser, Subcommand};

#[derive(Parser, Debug, Clone)]
#[command(version, about, long_about = None)]
pub struct EdgelinkConfig {
    /// Path of the 'flows.json' file
    #[arg(short, long, default_value_t = default_flows_path())]
    pub flows_path: String,

    /// Path of the log configuration file
    #[arg(short, long)]
    pub log_path: Option<String>,
}

fn default_flows_path() -> String {
    dirs_next::home_dir()
        .expect("Can not found the $HOME dir!!!")
        .join(".node-red")
        .join("flows.json")
        .to_string_lossy()
        .to_string()
}
