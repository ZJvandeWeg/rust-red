use std::io::Read;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use dashmap::DashMap;
use serde::Deserialize;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;

use super::context::{Context, ContextManager, ContextManagerBuilder};
use super::env::*;
use super::model::json::{RedFlowConfig, RedGlobalNodeConfig};
use super::model::*;
use super::nodes::FlowNodeBehavior;
use crate::runtime::flow::Flow;
use crate::runtime::model::Variant;
use crate::runtime::nodes::{GlobalNodeBehavior, NodeFactory};
use crate::runtime::registry::Registry;
use crate::EdgelinkError;

#[derive(Debug, Clone, Deserialize, Default)]
pub struct FlowEngineArgs {
    //node_msg_queue_capacity: usize,
}

impl FlowEngineArgs {
    pub fn load(cfg: Option<&config::Config>) -> crate::Result<Self> {
        match cfg {
            Some(cfg) => {
                let res = cfg.get::<Self>("runtime.engine")?;
                Ok(res)
            }
            _ => Ok(FlowEngineArgs::default()),
        }
    }
}

#[derive(Debug)]
pub(crate) struct FlowEngineState {
    _context: Variant,
    shutdown: AtomicBool,
    flows: DashMap<ElementId, Arc<Flow>>,
    global_nodes: DashMap<ElementId, Arc<dyn GlobalNodeBehavior>>,
    all_flow_nodes: DashMap<ElementId, Arc<dyn FlowNodeBehavior>>,
}

pub struct FlowEngine {
    pub(crate) state: FlowEngineState,

    stop_token: CancellationToken,
    _args: FlowEngineArgs,
    envs: Arc<EnvStore>,
    context_manager: Arc<ContextManager>,
    context: Arc<Context>,
}

impl FlowEngine {
    pub fn new_with_json(
        reg: Arc<dyn Registry>,
        json: &serde_json::Value,
        elcfg: Option<&config::Config>,
    ) -> crate::Result<Arc<FlowEngine>> {
        let json_values = json::deser::load_flows_json_value(json).map_err(|e| {
            log::error!("Failed to load NodeRED JSON value: {}", e);
            e
        })?;

        let envs = EnvStoreBuilder::default().with_process_env().build();

        let mut ctx_builder = ContextManagerBuilder::new();
        if let Some(cfg) = elcfg {
            let _ = ctx_builder.with_config(cfg)?; // Load the section in the configuration
        } else {
            let _ = ctx_builder.load_default();
        }
        let context_manager = ctx_builder.build()?;

        // let context_manager = Arc::new(ContextManager::default());
        let context = context_manager.new_context(None, "global".into());

        let engine = Arc::new(FlowEngine {
            stop_token: CancellationToken::new(),
            state: FlowEngineState {
                all_flow_nodes: DashMap::new(),
                global_nodes: DashMap::new(),
                flows: DashMap::new(),
                _context: Variant::empty_object(),
                shutdown: AtomicBool::new(true),
            },
            envs,
            _args: FlowEngineArgs::load(elcfg)?,
            context_manager,
            context,
        });

        engine.clone().load_flows(&json_values.flows, reg.clone(), elcfg)?;

        engine.clone().load_global_nodes(&json_values.global_nodes, reg.clone())?;

        Ok(engine)
    }

    pub fn new_with_flows_file(
        reg: Arc<dyn Registry>,
        flows_json_path: &str,
        elcfg: Option<&config::Config>,
    ) -> crate::Result<Arc<FlowEngine>> {
        let mut file = std::fs::File::open(flows_json_path)?;
        let mut json_str = String::new();
        file.read_to_string(&mut json_str)?;
        Self::new_with_json_string(reg, &json_str, elcfg)
    }

    pub fn new_with_json_string(
        reg: Arc<dyn Registry>,
        json_str: &str,
        elcfg: Option<&config::Config>,
    ) -> crate::Result<Arc<FlowEngine>> {
        let json: serde_json::Value = serde_json::from_str(json_str)?;
        Self::new_with_json(reg, &json, elcfg)
    }

    pub fn get_flow(&self, id: &ElementId) -> Option<Arc<Flow>> {
        self.state.flows.get(id).map(|x| x.value().clone())
    }

    fn load_flows(
        self: Arc<Self>,
        flow_configs: &[RedFlowConfig],
        reg: Arc<dyn Registry>,
        elcfg: Option<&config::Config>,
    ) -> crate::Result<()> {
        // load flows
        for flow_config in flow_configs.iter() {
            log::debug!("---- Loading flow/subflow: (id='{}', label='{}')...", flow_config.id, flow_config.label);
            let flow = Flow::new(self.clone(), flow_config, reg.clone(), elcfg)?;
            {
                // register all nodes
                for fnode in flow.get_all_flow_nodes().iter() {
                    if self.state.all_flow_nodes.contains_key(&fnode.id()) {
                        return Err(
                            EdgelinkError::InvalidData(format!("This flow node already existed: {}", fnode)).into()
                        );
                    }
                    self.state.all_flow_nodes.insert(fnode.id(), fnode.clone());
                }

                //register the flow
                self.state.flows.insert(flow.id, flow);
            }
            log::debug!(
                "---- The flow (id='{}', label='{}') has been loaded successfully.",
                flow_config.id,
                flow_config.label
            );
        }
        Ok(())
    }

    fn load_global_nodes(
        self: Arc<Self>,
        node_configs: &[RedGlobalNodeConfig],
        reg: Arc<dyn Registry>,
    ) -> crate::Result<()> {
        for global_config in node_configs.iter() {
            let node_type_name = global_config.type_name.as_str();
            let meta_node = if let Some(meta_node) = reg.get(node_type_name) {
                meta_node
            } else {
                log::warn!(
                    "Unknown global configuration node type: (type='{}', id='{}', name='{}')",
                    global_config.type_name,
                    global_config.id,
                    global_config.name
                );
                reg.get("unknown.global").unwrap()
            };

            let global_node = match meta_node.factory {
                NodeFactory::Global(factory) => factory(self.clone(), global_config)?,
                _ => {
                    return Err(EdgelinkError::NotSupported(format!(
                        "Must be a global node: Node(id={0}, type='{1}')",
                        global_config.id, global_config.type_name
                    ))
                    .into())
                }
            };

            self.state.global_nodes.insert(*global_node.id(), Arc::from(global_node));
        }
        Ok(())
    }

    pub async fn inject_msg_to_flow(
        &self,
        flow_id: &ElementId,
        msg: Arc<RwLock<Msg>>,
        cancel: CancellationToken,
    ) -> crate::Result<()> {
        let flow = self.state.flows.get(flow_id).as_deref().cloned();
        if let Some(flow) = flow {
            flow.inject_msg(msg, cancel.clone()).await?;
            Ok(())
        } else {
            Err(EdgelinkError::BadArguments(format!("Can not found flow_id: {}", flow_id)).into())
        }
    }

    pub async fn forward_msg_to_link_in(
        &self,
        link_in_id: &ElementId,
        msg: Arc<RwLock<Msg>>,
        cancel: CancellationToken,
    ) -> crate::Result<()> {
        let flow = { self.state.flows.get(link_in_id).as_deref().cloned() };
        if let Some(flow) = flow {
            flow.inject_msg(msg, cancel.clone()).await?;
            Ok(())
        } else {
            Err(EdgelinkError::BadArguments(format!("Can not found `link id`: {}", link_in_id)).into())
        }
    }

    pub async fn start(&self) -> crate::Result<()> {
        for f in self.state.flows.iter() {
            f.value().start().await?;
        }

        self.state.shutdown.store(false, std::sync::atomic::Ordering::Relaxed);

        log::info!("-- All flows started.");
        Ok(())
    }

    pub async fn stop(&self) -> crate::Result<()> {
        log::info!("-- Stopping all flows...");
        self.stop_token.cancel();

        for i in self.state.flows.iter() {
            i.value().stop().await?;
        }

        self.state.shutdown.store(true, std::sync::atomic::Ordering::Relaxed);
        //drop(self.stopped_tx);
        log::info!("-- All flows stopped.");
        Ok(())
    }

    pub fn find_flow_node_by_id(&self, id: &ElementId) -> Option<Arc<dyn FlowNodeBehavior>> {
        self.state.all_flow_nodes.get(id).map(|x| x.value().clone())
    }

    pub fn find_flow_node_by_name(&self, name: &str) -> crate::Result<Option<Arc<dyn FlowNodeBehavior>>> {
        for i in self.state.flows.iter() {
            let flow = i.value();
            let opt_node = flow.get_node_by_name(name)?;
            if opt_node.is_some() {
                return Ok(opt_node.clone());
            }
        }
        Ok(None)
    }

    pub async fn inject_msg(
        &self,
        flow_node_id: &ElementId,
        msg: Arc<RwLock<Msg>>,
        cancel: CancellationToken,
    ) -> crate::Result<()> {
        let node = self
            .find_flow_node_by_id(flow_node_id)
            .ok_or(EdgelinkError::BadArguments(format!("Cannot found the flow node, id='{}'", flow_node_id)))?;
        node.inject_msg(msg, cancel).await
    }

    pub fn get_envs(&self) -> Arc<EnvStore> {
        self.envs.clone()
    }

    pub fn get_env(&self, key: &str) -> Option<Variant> {
        self.envs.evalute_env(key)
    }

    pub fn get_context_manager(&self) -> Arc<ContextManager> {
        self.context_manager.clone()
    }

    pub fn get_context(&self) -> Arc<Context> {
        self.context.clone()
    }
}

impl std::fmt::Debug for FlowEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // TODO
        f.debug_struct("FlowEngine").finish()
    }
}
