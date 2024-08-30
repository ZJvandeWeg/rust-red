// use std::env;
use std::process;
use std::sync::Arc;

// 3rd-party libs
use clap::Parser;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;

// use libloading::Library;

use edgelink_core::runtime::engine::FlowEngine;
use edgelink_core::runtime::model::EdgelinkConfig;
use edgelink_core::runtime::registry::{Registry, RegistryImpl};
use edgelink_core::Result;

mod consts;

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
pub(crate) fn log_init(elargs: &EdgelinkConfig) {
    if let Some(ref log_path) = elargs.log_path {
        log4rs::init_file(log_path, Default::default()).unwrap();
    } else {
        let stdout = log4rs::append::console::ConsoleAppender::builder()
            .encoder(Box::new(log4rs::encode::pattern::PatternEncoder::new(
                "[{h({l})}]\t{m}{n}",
            )))
            .build();

        let config = log4rs::Config::builder()
            .appender(log4rs::config::Appender::builder().build("stdout", Box::new(stdout)))
            .build(
                log4rs::config::Root::builder()
                    .appender("stdout")
                    .build(log::LevelFilter::Info),
            )
            .unwrap();

        let _ = log4rs::init_config(config).unwrap();
    }
}

struct Runtime {
    args: Arc<EdgelinkConfig>,
    registry: Arc<dyn Registry>,
    engine: RwLock<Option<Arc<FlowEngine>>>,
}

impl Runtime {
    fn new(elargs: Arc<EdgelinkConfig>) -> Self {
        Runtime {
            args: elargs.clone(),
            registry: Arc::new(RegistryImpl::new()),
            engine: RwLock::new(None),
        }
    }

    async fn main_flow_task(self: Arc<Self>, cancel: CancellationToken) -> crate::Result<()> {
        let mut engine_holder = self.engine.write().await;
        log::info!("Loading flows file: {}", &self.args.flows_path);
        let engine =
            match FlowEngine::new_with_flows_file(self.registry.clone(), &self.args.flows_path) {
                Ok(eng) => eng,
                Err(e) => {
                    log::error!("Failed to create engine: {}", e);
                    return Err(e);
                }
            };
        *engine_holder = Option::Some(engine.clone());
        engine.start().await?;

        cancel.cancelled().await;

        engine.stop().await?;
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

async fn run_main_task(
    elargs: Arc<EdgelinkConfig>,
    cancel: CancellationToken,
) -> crate::Result<()> {
    let rt = Arc::new(Runtime::new(elargs));
    rt.run(cancel.clone()).await
}

async fn app_main() -> edgelink_core::Result<()> {
    println!(
        "EdgeLink V{} - #{}\n",
        consts::APP_VERSION,
        consts::GIT_HASH
    );

    let elargs = Arc::new(EdgelinkConfig::parse());

    println!("Initializing logging subsystem...\n");
    log_init(&elargs);

    // let m = Modal {};
    // m.run().await;
    log::info!(
        "EdgeLink Version: {} - #{}",
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
        ctrl_c_token.cancel(); // 触发取消
    });

    let res = run_main_task(elargs.clone(), cancel.child_token()).await;

    tokio::time::timeout(tokio::time::Duration::from_secs(10), cancel.cancelled()).await?;
    log::info!("All done!");

    res
}

#[tokio::main]
async fn main() -> Result<()> {
    if let Err(ref err) = app_main().await {
        eprintln!("Application error: {}", err);
        process::exit(-1);
    }
    Ok(())
}
