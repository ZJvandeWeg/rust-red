use serde::Deserialize;
use std::collections::{BTreeMap, HashMap};
use std::io::Read;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;

use crate::runtime::flow::Flow;
use crate::runtime::model::Variant;
use crate::runtime::nodes::{GlobalNodeBehavior, NodeFactory};
use crate::runtime::registry::Registry;
use crate::EdgelinkError;

use super::model::{ElementId, Msg};
use super::nodes::FlowNodeBehavior;
use crate::red::json::{RedFlowConfig, RedGlobalNodeConfig};

#[derive(Debug, Clone, Deserialize, Default)]
pub struct FlowEngineArgs {
    //node_msg_queue_capacity: usize,
}

impl FlowEngineArgs {
    pub fn load(cfg: Option<&config::Config>) -> crate::Result<Self> {
        match cfg {
            Some(cfg) => {
                let res = cfg.get::<Self>("engine")?;
                Ok(res)
            }
            _ => Ok(FlowEngineArgs::default()),
        }
    }
}

#[derive(Debug)]
pub(crate) struct FlowEngineState {
    flows: HashMap<ElementId, Arc<Flow>>,
    global_nodes: HashMap<ElementId, Arc<dyn GlobalNodeBehavior>>,
    all_flow_nodes: HashMap<ElementId, Arc<dyn FlowNodeBehavior>>,
    env_vars: BTreeMap<String, Variant>,
    _context: Variant,
    _shutdown: bool,
}

pub struct FlowEngine {
    pub(crate) state: std::sync::RwLock<FlowEngineState>,

    stop_token: CancellationToken,

    _args: FlowEngineArgs,
}

impl FlowEngine {
    pub fn new_with_json(
        reg: Arc<dyn Registry>,
        json: &serde_json::Value,
        elcfg: Option<&config::Config>,
    ) -> crate::Result<Arc<FlowEngine>> {
        let json_values = crate::red::json::deser::load_flows_json_value(json).map_err(|e| {
            log::error!("Failed to load NodeRED JSON value: {}", e);
            e
        })?;

        let engine = Arc::new(FlowEngine {
            stop_token: CancellationToken::new(),
            state: std::sync::RwLock::new(FlowEngineState {
                flows: HashMap::new(),
                global_nodes: HashMap::new(),
                all_flow_nodes: HashMap::new(),
                env_vars: BTreeMap::from_iter(FlowEngine::get_env_vars()),
                _context: Variant::new_empty_object(),
                _shutdown: false,
            }),
            _args: FlowEngineArgs::load(elcfg)?,
        });

        engine
            .clone()
            .load_flows(&json_values.flows, reg.clone(), elcfg)?;

        engine
            .clone()
            .load_global_nodes(&json_values.global_nodes, reg.clone())?;

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
        self.state.read().ok()?.flows.get(id).cloned()
    }

    fn load_flows(
        self: Arc<Self>,
        flow_configs: &[RedFlowConfig],
        reg: Arc<dyn Registry>,
        elcfg: Option<&config::Config>,
    ) -> crate::Result<()> {
        // load flows
        for flow_config in flow_configs.iter() {
            log::debug!(
                "---- Loading flow/subflow: (id='{}', label='{}')...",
                flow_config.id,
                flow_config.label
            );
            let flow = Flow::new(self.clone(), flow_config, reg.clone(), elcfg)?;
            {
                let mut state = self.state.write().unwrap();

                // register all nodes
                for fnode in flow.get_all_flow_nodes().iter() {
                    if state.all_flow_nodes.contains_key(&fnode.id()) {
                        return Err(EdgelinkError::InvalidData(format!(
                            "This flow node already existed: {}",
                            fnode
                        ))
                        .into());
                    }
                    state.all_flow_nodes.insert(fnode.id(), fnode.clone());
                }

                //register the flow
                state.flows.insert(flow.id, flow);
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

            let mut state = self.state.write().unwrap();
            state.global_nodes.insert(*global_node.id(), global_node);
        }
        Ok(())
    }

    pub async fn inject_msg_to_flow(
        &self,
        flow_id: &ElementId,
        msg: Arc<RwLock<Msg>>,
        cancel: CancellationToken,
    ) -> crate::Result<()> {
        let flow = {
            let state = self.state.read().unwrap();
            let flows = &state.flows;
            flows.get(flow_id).cloned()
        };
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
        let flow = {
            let state = self.state.read().unwrap();
            let flows = &state.flows;
            flows.get(link_in_id).cloned()
        };
        if let Some(flow) = flow {
            flow.inject_msg(msg, cancel.clone()).await?;
            Ok(())
        } else {
            Err(
                EdgelinkError::BadArguments(format!("Can not found `link id`: {}", link_in_id))
                    .into(),
            )
        }
    }

    pub async fn start(&self) -> crate::Result<()> {
        let flows: Vec<_> = {
            let mut state = self.state.write().unwrap();
            state.env_vars.clear();
            state.env_vars.extend(FlowEngine::get_env_vars());

            state.flows.values().cloned().collect()
        };
        for flow in flows {
            flow.start().await?;
        }

        log::info!("-- All flows started.");
        Ok(())
    }

    pub async fn stop(&self) -> crate::Result<()> {
        log::info!("-- Stopping all flows...");
        self.stop_token.cancel();

        let flows: Vec<_> = {
            let state = self.state.read().unwrap();
            state.flows.values().cloned().collect()
        };
        for flow in flows {
            flow.clone().stop().await?;
        }

        //drop(self.stopped_tx);
        log::info!("-- All flows stopped.");
        Ok(())
    }

    pub fn find_flow_node_by_id(&self, id: &ElementId) -> Option<Arc<dyn FlowNodeBehavior>> {
        let nodes = &self.state.read().ok()?.all_flow_nodes;
        nodes.get(id).cloned()
    }

    pub fn find_flow_node_by_name(
        &self,
        name: &str,
    ) -> crate::Result<Option<Arc<dyn FlowNodeBehavior>>> {
        let state = &self.state.read().expect("The state must be available!");
        for (_, flow) in state.flows.iter() {
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
            .ok_or(EdgelinkError::BadArguments(format!(
                "Cannot found the flow node, id='{}'",
                flow_node_id
            )))?;
        node.inject_msg(msg, cancel).await
    }

    fn get_env_vars() -> impl Iterator<Item = (String, Variant)> {
        std::env::vars().map(|(k, v)| (k.to_string(), Variant::String(v)))
    }
}
