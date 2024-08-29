use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use tokio::io::AsyncReadExt;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;

use crate::runtime::flow::Flow;
use crate::runtime::model::Variant;
use crate::runtime::nodes::{GlobalNodeBehavior, NodeFactory};
use crate::runtime::registry::Registry;
use crate::EdgeLinkError;

use super::model::{ElementId, Msg};
use super::nodes::FlowNodeBehavior;
use super::red::json::{RedFlowConfig, RedGlobalNodeConfig};

pub(crate) struct FlowEngineState {
    flows: HashMap<ElementId, Arc<Flow>>,
    env_vars: BTreeMap<String, Variant>,
    global_nodes: HashMap<ElementId, Arc<dyn GlobalNodeBehavior>>,
    _context: Variant,
    _shutdown: bool,
}

pub struct FlowEngine {
    pub(crate) state: RwLock<FlowEngineState>,
    pub(crate) stop_token: CancellationToken,
}

impl FlowEngine {
    pub async fn new_with_json(
        reg: Arc<dyn Registry>,
        json: &serde_json::Value,
    ) -> crate::Result<Arc<FlowEngine>> {
        let json_values =
            crate::runtime::red::json::deser::load_flows_json_value(json).map_err(|e| {
                log::error!("Failed to load NodeRED JSON value: {}", e);
                e
            })?;

        let engine = Arc::new(FlowEngine {
            stop_token: CancellationToken::new(),
            state: RwLock::new(FlowEngineState {
                flows: HashMap::new(),
                global_nodes: HashMap::new(),
                env_vars: BTreeMap::from_iter(FlowEngine::get_env_vars()),
                _context: Variant::new_empty_object(),
                _shutdown: false,
            }),
        });

        engine
            .clone()
            .load_flows(&json_values.flows, reg.clone())
            .await?;

        engine
            .clone()
            .load_global_nodes(&json_values.global_nodes, reg.clone())
            .await?;

        Ok(engine)
    }

    pub async fn new_with_flows_file(
        reg: Arc<dyn Registry>,
        flows_json_path: &str,
    ) -> crate::Result<Arc<FlowEngine>> {
        let mut file = tokio::fs::File::open(flows_json_path).await?;
        let mut json_str = String::new();
        file.read_to_string(&mut json_str).await?;
        Self::new_with_json_string(reg, &json_str).await
    }

    pub async fn new_with_json_string(
        reg: Arc<dyn Registry>,
        json_str: &str,
    ) -> crate::Result<Arc<FlowEngine>> {
        let json: serde_json::Value = serde_json::from_str(json_str)?;
        Self::new_with_json(reg, &json).await
    }

    pub fn get_flow(&self, _id: &ElementId) -> Arc<Flow> {
        todo!()
    }

    async fn load_flows(
        self: Arc<Self>,
        flow_configs: &[RedFlowConfig],
        reg: Arc<dyn Registry>,
    ) -> crate::Result<()> {
        // load flows
        for flow_config in flow_configs.iter() {
            let flow = Flow::new(self.clone(), flow_config, reg.clone()).await?;
            {
                let mut state = self.state.write().await;
                state.flows.insert(flow.id, flow);
            }
        }
        Ok(())
    }

    async fn load_global_nodes(
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
                    "Unknown flow node type: (type='{}', id='{}')",
                    global_config.type_name,
                    global_config.id
                );
                reg.get("unknown.global").unwrap()
            };

            let global_node = match meta_node.factory {
                NodeFactory::Global(factory) => factory(self.clone(), global_config)?,
                _ => {
                    return Err(EdgeLinkError::NotSupported(format!(
                        "Must be a global node: Node(id={0}, type='{1}')",
                        global_config.id, global_config.type_name
                    ))
                    .into())
                }
            };

            let mut state = self.state.write().await;
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
        let state = self.state.read().await;
        if let Some(flow) = state.flows.get(flow_id) {
            flow.inject_msg(msg, cancel.clone()).await?;
            Ok(())
        } else {
            Err(EdgeLinkError::BadArguments(format!("Can not found flow_id: {}", flow_id)).into())
        }
    }

    pub async fn start(&self) -> crate::Result<()> {
        let mut state = self.state.write().await;
        state.env_vars.clear();
        state.env_vars.extend(FlowEngine::get_env_vars());
        for flow in state.flows.values() {
            flow.start().await?;
        }
        log::info!("-- All flows started.");
        Ok(())
    }

    pub async fn stop(&self) -> crate::Result<()> {
        log::info!("-- Stopping all flows...");
        self.stop_token.cancel();
        let state = self.state.write().await;
        for flow in state.flows.values() {
            flow.clone().stop().await?;
        }
        //drop(self.stopped_tx);
        log::info!("-- All flows stopped.");
        Ok(())
    }

    pub async fn find_flow_node_async(&self, id: &ElementId) -> Option<Arc<dyn FlowNodeBehavior>> {
        let flows = &self.state.read().await.flows;
        for (_, flow) in flows.iter() {
            if let Some(node) = flow.get_node_async(id).await {
                return Some(node.clone());
            }
        }
        None
    }

    fn get_env_vars() -> impl Iterator<Item = (String, Variant)> {
        std::env::vars().map(|(k, v)| (k.to_string(), Variant::String(v)))
    }
}
