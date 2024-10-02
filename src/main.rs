// use std::env;
use std::io::{self, Read};
use std::process;
use std::str::FromStr;
use std::sync::Arc;

// 3rd-party libs
use clap::Parser;
use runtime::engine::Engine;
use runtime::registry::RegistryHandle;
use serde::Deserialize;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

use edgelink_core::runtime::model::*;
use edgelink_core::runtime::registry::RegistryBuilder;
use edgelink_core::text::json_seq;
use edgelink_core::*;

include!(concat!(env!("OUT_DIR"), "/__use_node_plugins.rs"));

mod cliargs;
mod consts;
mod logging;

pub use cliargs::*;

// TODO move to debug.rs
#[derive(Debug, Clone)]
pub struct MsgInjectionEntry {
    pub nid: ElementId,

    pub msg: MsgHandle,
}

#[derive(Debug)]
struct App {
    _registry: RegistryHandle,
    engine: Engine,
    msgs_to_inject: Mutex<Vec<MsgInjectionEntry>>,
}

impl App {
    pub fn default(elargs: Arc<CliArgs>, app_config: Option<&config::Config>) -> edgelink_core::Result<Self> {
        log::info!("Discovering all nodes...");
        // edgelink_core::runtime::registry::collect_nodes();
        log::info!("Loading node registry...");
        let reg = RegistryBuilder::default().build()?;

        let mut msgs_to_inject = Vec::new();

        log::info!("Loading flows file: {}", elargs.flows_path);
        let engine = if elargs.stdin {
            let mut buffer = Vec::new();
            io::stdin().read_to_end(&mut buffer)?;

            // This is a flow JSON and following some messages to inject
            let flows_json_value = if !buffer.is_empty() && buffer[0] == json_seq::RS_CHAR {
                log::info!("Loading JSON sequences from stdin...");
                let mut start = 0;
                let mut in_braces = false;
                let mut is_first = false;
                let mut flows_value: serde_json::Value = serde_json::Value::Null;

                for (i, c) in buffer.iter().enumerate() {
                    if *c == json_seq::RS_CHAR {
                        if in_braces {
                            panic!("Nested braces are not supported");
                        }
                        in_braces = true;
                        start = i + 1;
                    } else if *c == json_seq::NL_CHAR {
                        if !in_braces {
                            panic!("Unmatched closing brace");
                        }
                        in_braces = false;
                        let json_entry_text = String::from_utf8_lossy(&buffer[start..i]);
                        let json_value = serde_json::Value::from_str(&json_entry_text)?;
                        if !is_first {
                            flows_value = json_value.clone();
                            is_first = true;
                        } else {
                            let entry = MsgInjectionEntry {
                                nid: edgelink_core::runtime::model::json::helpers::parse_red_id_value(
                                    &json_value["nid"],
                                )
                                .unwrap(),
                                msg: MsgHandle::new(Msg::deserialize(&json_value["msg"])?),
                            };
                            msgs_to_inject.push(entry);
                        }
                    }
                }
                flows_value
            } else {
                log::info!("Loading flows JSON stdin...");
                let json_str = String::from_utf8_lossy(&buffer);
                serde_json::from_str(&json_str)?
            };
            Engine::with_json(&reg, flows_json_value, app_config)?
        } else {
            Engine::with_flows_file(&reg, &elargs.flows_path, app_config)?
        };

        Ok(App { _registry: reg, engine, msgs_to_inject: Mutex::new(msgs_to_inject) })
    }

    async fn main_flow_task(self: Arc<Self>, cancel: CancellationToken) -> crate::Result<()> {
        self.engine.start().await?;

        // Inject msgs
        {
            let mut entries = self.msgs_to_inject.lock().await;
            for e in entries.iter() {
                self.engine.inject_msg(&e.nid, e.msg.clone(), cancel.clone()).await?;
            }
            entries.clear();
        }

        cancel.cancelled().await;

        self.engine.stop().await?;
        log::info!("The flows engine stopped.");
        Ok(())
    }

    async fn idle_task(self: Arc<Self>, cancel: CancellationToken) -> crate::Result<()> {
        loop {
            tokio::select! {
                _ = tokio::time::sleep(tokio::time::Duration::from_secs(1)) => {
                }
                _ = cancel.cancelled() => {
                    // The token was cancelled
                    log::info!("Cancelling the idle task...");
                    break;
                }
            }
        }
        Ok(())
    }

    pub async fn run(self: Arc<Self>, cancel: CancellationToken) -> crate::Result<()> {
        let (res1, res2) = tokio::join!(
            self.clone().main_flow_task(cancel.child_token()),
            self.clone().idle_task(cancel.child_token())
        );
        res1?;
        res2?;
        Ok(())
    }
}

fn load_config(cli_args: &CliArgs) -> anyhow::Result<Option<config::Config>> {
    // Load configuration from default, development, and production files
    let home_dir = dirs_next::home_dir()
        .map(|x| x.join(".edgelink").to_string_lossy().to_string())
        .expect("Cannot get the `~/home` directory");

    let edgelink_home_dir = cli_args.home.clone().or(std::env::var("EDGELINK_HOME").ok()).or(Some(home_dir));

    let run_env = cli_args.env.clone().or(std::env::var("EDGELINK_RUN_ENV").ok()).unwrap_or("dev".to_string());

    if cli_args.verbose > 0 {
        if let Some(ref x) = edgelink_home_dir {
            eprintln!("$EDGELINK_HOME={}", x);
        }
    }

    if let Some(md) = edgelink_home_dir.as_ref().and_then(|x| std::fs::metadata(x).ok()) {
        if md.is_dir() {
            let mut builder = config::Config::builder();

            builder = if let Some(hd) = edgelink_home_dir {
                builder
                    .add_source(config::File::with_name(&format!("{}/edgelinkd.toml", hd)).required(false))
                    .add_source(config::File::with_name(&format!("{}/edgelinkd.{}.toml", hd, run_env)).required(false))
                    .set_override("home_dir", hd)?
            } else {
                builder
            };

            builder = builder
                .set_override("run_env", run_env)? // override run_env
                .set_override("node.msg_queue_capacity", 1)?;
            let config = builder.build()?;
            return Ok(Some(config));
        }
    }
    if cli_args.verbose > 0 {
        eprintln!("The `$EDGELINK_HOME` directory does not exist!");
    }
    Ok(None)
}

async fn app_main(cli_args: Arc<CliArgs>) -> anyhow::Result<()> {
    if cli_args.verbose > 0 {
        eprintln!("EdgeLink v{} - #{}\n", consts::APP_VERSION, consts::GIT_HASH);
        eprintln!("Loading configuration..");
    }
    let cfg = load_config(&cli_args)?;

    if cli_args.verbose > 0 {
        eprintln!("Initializing logging sub-system...\n");
    }
    logging::log_init(&cli_args);
    if cli_args.verbose > 0 {
        eprintln!("Logging sub-system initialized.\n");
    }

    // let m = Modal {};
    // m.run().await;
    log::info!("EdgeLink Version={}-#{}", consts::APP_VERSION, consts::GIT_HASH);
    log::info!("==========================================================\n");

    // That's right, a CancellationToken. I guess you could say that safely
    // I'm a C# lover.
    let cancel = CancellationToken::new();

    let ctrl_c_token = cancel.clone();
    tokio::task::spawn(async move {
        tokio::signal::ctrl_c().await.expect("Failed to install CTRL+C signal handler");
        log::info!("CTRL+C pressed, cancelling tasks...");
        ctrl_c_token.cancel();
    });

    log::info!("Starting EdgeLink run-time engine...");
    log::info!("Press CTRL+C to terminate.");

    let app = Arc::new(App::default(cli_args, cfg.as_ref())?);
    let app_result = app.run(cancel.child_token()).await;

    tokio::time::timeout(tokio::time::Duration::from_secs(10), cancel.cancelled()).await?;
    log::info!("All done!");

    app_result
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Arc::new(CliArgs::parse());
    if let Err(ref err) = app_main(args).await {
        eprintln!("Application error: {}", err);
        process::exit(-1);
    }
    Ok(())
}
