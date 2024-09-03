// use std::env;
use std::process;
use std::sync::Arc;

// 3rd-party libs
use clap::Parser;
use tokio_util::sync::CancellationToken;

// use libloading::Library;

use edgelink_core::runtime::engine::FlowEngine;
use edgelink_core::runtime::registry::{Registry, RegistryBuilder};
use edgelink_core::*;

mod cliargs;
mod consts;

pub use cliargs::*;

/*
use core::{Plugin, PluginRegistrar};

struct Registrar {
    plugins: Vec<Box<dyn Plugin>>,
}

impl PluginRegistrar for Registrar {
    fn register_plugin(&mut self, plugin: Box<dyn Plugin>) {
        self.plugins.push(plugin);
    }
}

fn main() {
    let mut registrar = Registrar {
        plugins: Vec::new(),
    };

    for path in std::env::args_os().skip(1) {
        // In this code, we never close the shared library - if you need to be able to unload the
        // library, that will require more work.
        let lib = Box::leak(Box::new(Library::new(path).unwrap()));
        // NOTE: You need to do something to ensure you're only loading "safe" code. Out of scope
        // for this code.
        unsafe {
            let func: libloading::Symbol<unsafe extern "C" fn(&mut dyn PluginRegistrar) -> ()> =
                lib.get(b"plugin_entry").unwrap();
            func(&mut registrar);
        }
    }

    for plugin in registrar.plugins {
        plugin.callback1();
        dbg!(plugin.callback2(7));
    }
}

*/
pub(crate) fn log_init(elargs: &CliArgs) {
    if let Some(ref log_path) = elargs.log_path {
        log4rs::init_file(log_path, Default::default()).unwrap();
    } else {
        let stderr = log4rs::append::console::ConsoleAppender::builder()
            .target(log4rs::append::console::Target::Stderr)
            .encoder(Box::new(log4rs::encode::pattern::PatternEncoder::new(
                "[{h({l})}]\t{m}{n}",
            )))
            .build();

        let level = match elargs.verbose {
            0 => log::LevelFilter::Off,
            1 => log::LevelFilter::Warn,
            2 => log::LevelFilter::Info,
            3 => log::LevelFilter::Debug,
            _ => log::LevelFilter::Trace,
        };

        let config = log4rs::Config::builder()
            .appender(log4rs::config::Appender::builder().build("stderr", Box::new(stderr)))
            .build(
                log4rs::config::Root::builder()
                    .appender("stderr")
                    .build(level),
            )
            .unwrap(); // TODO FIXME

        let _ = log4rs::init_config(config).unwrap();
    }
}

struct App {
    _registry: Arc<dyn Registry>,
    engine: Arc<FlowEngine>,
}

impl App {
    fn default(
        elargs: Arc<CliArgs>,
        app_config: Option<&config::Config>,
    ) -> edgelink_core::Result<Self> {
        log::info!("Loading node registry...");
        let reg = RegistryBuilder::default().build()?;

        log::info!("Loading flows file: {}", elargs.flows_path);
        let engine = FlowEngine::new_with_flows_file(reg.clone(), &elargs.flows_path, app_config)?;

        Ok(App {
            _registry: reg,
            engine,
        })
    }

    async fn main_flow_task(self: Arc<Self>, cancel: CancellationToken) -> crate::Result<()> {
        self.engine.start().await?;

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
        .expect("Cannot got the `~/home` directory");

    let edgelink_home_dir = cli_args
        .home
        .clone()
        .or(std::env::var("EDGELINK_HOME").ok())
        .or(Some(home_dir));

    let run_env = cli_args
        .env
        .clone()
        .and(std::env::var("EDGELINK_RUN_ENV").ok())
        .unwrap_or("dev".to_string());

    if cli_args.verbose > 0 {
        if let Some(ref x) = edgelink_home_dir {
            eprintln!("$EDGELINK_HOME={}", x);
        }
    }

    if let Some(md) = edgelink_home_dir
        .as_ref()
        .and_then(|x| std::fs::metadata(&x).ok())
    {
        if md.is_dir() {
            let cfg = config::Config::builder()
                .add_source(config::File::with_name("edgelinkd.toml"))
                .add_source(
                    config::File::with_name(&format!("edgelinkd.{}.toml", run_env)).required(false),
                )
                .set_override("home_dir", edgelink_home_dir)?
                .set_override("run_env", run_env)?
                .set_override("node.msg_queue_capacity", 1)?
                .build()?;
            return Ok(Some(cfg));
        }
    }
    if cli_args.verbose > 0 {
        eprintln!("The `$EDGELINK_HOME` does not existed!");
    }
    Ok(None)
}

async fn app_main(cli_args: Arc<CliArgs>) -> anyhow::Result<()> {
    if cli_args.verbose > 0 {
        eprintln!(
            "EdgeLink v{} - #{}\n",
            consts::APP_VERSION,
            consts::GIT_HASH
        );
        eprintln!("Loading configuration..");
    }
    let cfg = load_config(&cli_args)?;

    if cli_args.verbose > 0 {
        eprintln!("Initializing logging sub-system...\n");
    }
    log_init(&cli_args);
    if cli_args.verbose > 0 {
        eprintln!("Logging sub-system initialized.\n");
    }

    // let m = Modal {};
    // m.run().await;
    log::info!(
        "EdgeLink Version={}-#{}",
        consts::APP_VERSION,
        consts::GIT_HASH
    );
    log::info!("==========================================================\n");

    // That's right, a CancellationToken. I guess you could say that safely
    // I'm a C# lover.
    let cancel = CancellationToken::new();

    let ctrl_c_token = cancel.clone();
    tokio::task::spawn(async move {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to install CTRL+C signal handler");
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
