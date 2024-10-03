use std::io::Read;
use std::sync::{Arc, Weak};

use dashmap::DashMap;
use runtime::flow::*;
use runtime::registry::RegistryHandle;
use serde::Deserialize;
use tokio_util::sync::CancellationToken;

use super::context::{Context, ContextManager, ContextManagerBuilder};
use super::env::*;
use super::model::json::{RedFlowConfig, RedGlobalNodeConfig};
use super::model::*;
use super::nodes::FlowNodeBehavior;
use crate::runtime::model::Variant;
use crate::runtime::nodes::{GlobalNodeBehavior, NodeFactory};
use crate::*;

#[derive(Debug, Clone, Deserialize, Default)]
pub struct EngineArgs {
    //node_msg_queue_capacity: usize,
}

impl EngineArgs {
    pub fn load(cfg: Option<&config::Config>) -> crate::Result<Self> {
        match cfg {
            Some(cfg) => match cfg.get::<Self>("runtime.engine") {
                Ok(res) => Ok(res),
                Err(config::ConfigError::NotFound(_)) => Ok(Self::default()),
                Err(e) => Err(e.into()),
            },
            _ => Ok(Self::default()),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Engine {
    inner: Arc<InnerEngine>,
}

#[derive(Debug, Clone)]
pub struct WeakEngine {
    inner: Weak<InnerEngine>,
}

impl WeakEngine {
    pub fn upgrade(&self) -> Option<Engine> {
        Weak::upgrade(&self.inner).map(|x| Engine { inner: x })
    }
}

struct InnerEngine {
    shutdown: tokio::sync::RwLock<bool>,
    stop_token: CancellationToken,
    _args: EngineArgs,
    envs: Arc<EnvStore>,
    context_manager: Arc<ContextManager>,
    context: Arc<Context>,

    _context: Variant,
    flows: DashMap<ElementId, Flow>,
    global_nodes: DashMap<ElementId, Arc<dyn GlobalNodeBehavior>>,
    all_flow_nodes: DashMap<ElementId, Arc<dyn FlowNodeBehavior>>,

    #[cfg(any(test, feature = "pymod"))]
    final_msgs_rx: MsgUnboundedReceiverHolder,

    #[cfg(any(test, feature = "pymod"))]
    final_msgs_tx: MsgUnboundedSender,
}

impl Engine {
    pub fn downgrade(&self) -> WeakEngine {
        WeakEngine { inner: Arc::downgrade(&self.inner) }
    }

    pub fn with_json(
        reg: &RegistryHandle,
        json: serde_json::Value,
        elcfg: Option<&config::Config>,
    ) -> crate::Result<Engine> {
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
        let context = context_manager.new_global_context();

        #[cfg(any(test, feature = "pymod"))]
        let final_msgs_channel = tokio::sync::mpsc::unbounded_channel();

        let engine = Self {
            inner: Arc::new(InnerEngine {
                shutdown: tokio::sync::RwLock::new(true),
                stop_token: CancellationToken::new(),
                all_flow_nodes: DashMap::new(),
                global_nodes: DashMap::new(),
                flows: DashMap::new(),
                _context: Variant::empty_object(),
                envs,
                _args: EngineArgs::load(elcfg)?,
                context_manager,
                context,

                #[cfg(any(test, feature = "pymod"))]
                final_msgs_rx: MsgUnboundedReceiverHolder::new(final_msgs_channel.1),

                #[cfg(any(test, feature = "pymod"))]
                final_msgs_tx: final_msgs_channel.0,
            }),
        };

        engine.clone().load_flows(json_values.flows, reg, elcfg)?;

        engine.clone().load_global_nodes(json_values.global_nodes, reg.clone())?;

        Ok(engine)
    }

    pub fn with_flows_file(
        reg: &RegistryHandle,
        flows_json_path: &str,
        elcfg: Option<&config::Config>,
    ) -> crate::Result<Engine> {
        let mut file = std::fs::File::open(flows_json_path)?;
        let mut json_str = String::new();
        file.read_to_string(&mut json_str)?;
        Self::with_json_string(reg, json_str, elcfg)
    }

    pub fn with_json_string(
        reg: &RegistryHandle,
        json_str: String,
        elcfg: Option<&config::Config>,
    ) -> crate::Result<Engine> {
        let json: serde_json::Value = serde_json::from_str(&json_str)?;
        Self::with_json(reg, json, elcfg)
    }

    pub fn get_flow(&self, id: &ElementId) -> Option<Flow> {
        self.inner.flows.get(id).map(|x| x.value().clone())
    }

    fn load_flows(
        &self,
        flow_cfg: Vec<RedFlowConfig>,
        reg: &RegistryHandle,
        elcfg: Option<&config::Config>,
    ) -> crate::Result<()> {
        // load flows
        for flow_config in flow_cfg.into_iter() {
            log::debug!("---- Loading flow/subflow: (id='{}', label='{}')...", flow_config.id, flow_config.label);
            let flow = Flow::new(self, flow_config, reg, elcfg)?;
            {
                // register all nodes
                for fnode in flow.get_all_flow_nodes().iter() {
                    if self.inner.all_flow_nodes.contains_key(&fnode.id()) {
                        return Err(EdgelinkError::InvalidOperation(format!(
                            "This flow node already existed: {}",
                            fnode
                        ))
                        .into());
                    }
                    self.inner.all_flow_nodes.insert(fnode.id(), fnode.clone());
                }

                log::debug!(
                    "---- The flow (id='{}', label='{}') has been loaded successfully.",
                    flow.id(),
                    flow.name()
                );
                //register the flow
                self.inner.flows.insert(flow.id(), flow);
            }
        }
        Ok(())
    }

    fn load_global_nodes(&self, node_configs: Vec<RedGlobalNodeConfig>, reg: RegistryHandle) -> crate::Result<()> {
        for global_config in node_configs.into_iter() {
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
                NodeFactory::Global(factory) => factory(self, &global_config)?,
                _ => {
                    return Err(EdgelinkError::NotSupported(format!(
                        "Must be a global node: Node(id={0}, type='{1}')",
                        global_config.id, global_config.type_name
                    ))
                    .into())
                }
            };

            self.inner.global_nodes.insert(global_node.id(), Arc::from(global_node));
        }
        Ok(())
    }

    pub async fn inject_msg_to_flow(
        &self,
        flow_id: ElementId,
        msg: MsgHandle,
        cancel: CancellationToken,
    ) -> crate::Result<()> {
        let flow = self.inner.flows.get(&flow_id).as_deref().cloned();
        if let Some(flow) = flow {
            flow.inject_msg(msg, cancel.clone()).await?;
            Ok(())
        } else {
            Err(EdgelinkError::BadArgument("flow_id")).with_context(|| format!("Can not found flow_id: {}", flow_id))
        }
    }

    pub async fn forward_msg_to_link_in(
        &self,
        link_in_id: &ElementId,
        msg: MsgHandle,
        cancel: CancellationToken,
    ) -> crate::Result<()> {
        let flow = { self.inner.flows.get(link_in_id).as_deref().cloned() };
        if let Some(flow) = flow {
            flow.inject_msg(msg, cancel.clone()).await?;
            Ok(())
        } else {
            Err(EdgelinkError::BadArgument("link_in_id"))
                .with_context(|| format!("Can not found `link id`: {}", link_in_id))
        }
    }

    pub async fn start(&self) -> crate::Result<()> {
        log::info!("-- Starting engine...");
        let mut shutdown_lock = self.inner.shutdown.try_write()?;
        if !(*shutdown_lock) {
            return Err(EdgelinkError::invalid_operation("already started."));
        }

        if self.inner.flows.is_empty() {
            return Err(EdgelinkError::invalid_operation("no flows loaded in the engine."));
        }
        for f in self.inner.flows.iter() {
            f.value().start().await?;
        }

        *shutdown_lock = false;

        log::info!("-- All flows started.");
        Ok(())
    }

    pub async fn stop(&self) -> crate::Result<()> {
        let mut shutdown_lock = self.inner.shutdown.try_write()?;
        if *shutdown_lock {
            return Err(EdgelinkError::invalid_operation("not started."));
        }
        log::info!("-- Stopping engine...");

        self.inner.stop_token.cancel();

        for i in self.inner.flows.iter() {
            i.value().stop().await?;
        }

        *shutdown_lock = true;
        //drop(self.stopped_tx);
        log::info!("-- Engine flows stopped.");
        Ok(())
    }

    #[cfg(any(test, feature = "pymod"))]
    pub async fn run_once_with_inject(
        &self,
        expected_msgs: usize,
        timeout: std::time::Duration,
        mut msgs_to_inject: Vec<(ElementId, Msg)>,
    ) -> crate::Result<Vec<Msg>> {
        self.start().await?;

        let mut count = 0;
        let mut received = Vec::new();

        // Clear the final_msgs channel
        {
            let mut rx = self.inner.final_msgs_rx.rx.lock().await;
            while rx.try_recv().is_ok() {}
        }

        let cancel = CancellationToken::new();
        for msg in msgs_to_inject.drain(..) {
            self.inject_msg(&msg.0, MsgHandle::new(msg.1), cancel.clone()).await?;
        }

        let result = tokio::time::timeout(timeout, async {
            while !cancel.is_cancelled() && count < expected_msgs {
                let msg = self.inner.final_msgs_rx.recv_msg(cancel.clone()).await?;
                count += 1;
                let msg = msg.unwrap().await;
                received.push(msg);
            }
            cancel.cancel();
            cancel.cancelled().await;
            Ok(())
        })
        .await;

        self.stop().await?;
        match result {
            Ok(Ok(())) => Ok(received),
            Ok(Err(e)) => Err(e),
            Err(_) => Err(EdgelinkError::Timeout.into()),
        }
    }

    #[cfg(any(test, feature = "pymod"))]
    pub async fn run_once(&self, expected_msgs: usize, timeout: std::time::Duration) -> crate::Result<Vec<Msg>> {
        self.run_once_with_inject(expected_msgs, timeout, Vec::with_capacity(0)).await
    }

    pub fn find_flow_node_by_id(&self, id: &ElementId) -> Option<Arc<dyn FlowNodeBehavior>> {
        self.inner.all_flow_nodes.get(id).map(|x| x.value().clone())
    }

    pub fn find_flow_node_by_name(&self, name: &str) -> crate::Result<Option<Arc<dyn FlowNodeBehavior>>> {
        for i in self.inner.flows.iter() {
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
        msg: MsgHandle,
        cancel: CancellationToken,
    ) -> crate::Result<()> {
        let node = self
            .find_flow_node_by_id(flow_node_id)
            .ok_or(EdgelinkError::BadArgument("flow_node_id"))
            .with_context(|| format!("Cannot found the flow node, id='{}'", flow_node_id))?;
        node.inject_msg(msg, cancel).await
    }

    pub fn get_envs(&self) -> Arc<EnvStore> {
        self.inner.envs.clone()
    }

    pub fn get_env(&self, key: &str) -> Option<Variant> {
        self.inner.envs.evalute_env(key)
    }

    pub fn get_context_manager(&self) -> &Arc<ContextManager> {
        &self.inner.context_manager
    }

    pub fn context(&self) -> Arc<Context> {
        self.inner.context.clone()
    }

    #[cfg(any(test, feature = "pymod"))]
    pub fn recv_final_msg(&self, msg: MsgHandle) -> crate::Result<()> {
        self.inner.final_msgs_tx.send(msg)?;
        Ok(())
    }
}

impl std::fmt::Debug for InnerEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // TODO
        f.debug_struct("FlowEngine").finish()
    }
}

#[cfg(test)]
pub fn build_test_engine(flows_json: serde_json::Value) -> crate::Result<Engine> {
    let registry = crate::runtime::registry::RegistryBuilder::default().build().unwrap();
    Engine::with_json(&registry, flows_json, None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::time::Duration;

    fn make_simple_flows_json() -> serde_json::Value {
        let flows_json = json!([
        { "id": "100", "type": "tab", "label": "Flow 1" },
        { "id": "1", "type": "inject", "z": "100", "name": "", "props": [
                { "p": "payload" },
                { "p": "topic", "vt": "str" },
                { "p": "target", "vt": "str", "v": "double payload" }
            ],
            "once": true, "onceDelay": 0, "repeat": "", "topic": "",
            "payload": "foo", "payloadType": "str",
            "wires": [ [ "2" ] ]
        },
        { "id": "2", "z": "100", "type": "test-once" }
        ]);
        flows_json
    }

    fn make_flows_json_that_contains_subflows() -> serde_json::Value {
        let flows_json = json!([
        { "id": "999", "type": "inject", "z": "100", "name": "", "props": [
                { "p": "payload" },
                { "p": "topic", "vt": "str" },
                { "p": "target", "vt": "str", "v": "double payload" }
            ],
            "repeat": "", "once": true, "onceDelay": 0, "topic": "",
            "payload": "123", "payloadType": "num",
            "wires": [ [ "5" ] ]
        },
        { "id": "100", "type": "tab", "label": "Flow 1" },
        { "id": "200", "type": "tab", "label": "Flow 2" },
        { "id": "1", "z": "100", "type": "link in", "name": "double payload", "wires": [ [ "3" ] ] },
        { "id": "2", "z": "200", "type": "link in", "name": "double payload", "wires": [ [ "3" ] ] },
        { "id": "3", "z": "100", "type": "function", "func": "msg.payload+=msg.payload;return msg;", "wires": [["4"]]},
        { "id": "4", "z": "100", "type": "link out", "mode": "return" },
        { "id": "5", "z": "100", "type": "link call", "linkType": "dynamic", "links": [], "wires": [ [ "6" ] ] },
        { "id": "6", "z": "100", "type": "test-once" }
        ]);
        flows_json
    }

    #[tokio::test]
    async fn test_it_should_able_to_inject_msgs() {
        let flows_json = serde_json::json!([
            { "id": "100", "type": "tab", "label": "Flow 1" },
            { "id": "1", "z": "100", "type": "test-once" }
        ]);
        let engine = build_test_engine(flows_json).unwrap();
        let msgs_to_inject_json = serde_json::json!([
            ["1", {"payload": "foo"}],
            ["1", {"payload": "bar"}],
        ]);
        let msgs_to_inject = Vec::<(ElementId, Msg)>::deserialize(msgs_to_inject_json).unwrap();
        let msgs = engine.run_once_with_inject(2, Duration::from_millis(200), msgs_to_inject).await.unwrap();

        assert_eq!(msgs.len(), 2);
        {
            let msg0 = msgs[0].as_variant_object();
            assert_eq!(msg0.get("payload").unwrap(), &Variant::from("foo"));
        }
        {
            let msg1 = msgs[1].as_variant_object();
            assert_eq!(msg1.get("payload").unwrap(), &Variant::from("bar"));
        }
    }

    #[tokio::test]
    async fn test_it_should_load_and_run_simple_json_without_configuration() {
        let flows_json = make_simple_flows_json();
        let engine = build_test_engine(flows_json).unwrap();
        let msgs = engine.run_once(1, Duration::from_millis(200)).await.unwrap();
        assert_eq!(msgs.len(), 1);
        let msg = msgs[0].as_variant_object();
        assert_eq!(msg.get("payload").unwrap(), &Variant::from("foo"));
    }

    #[tokio::test]
    async fn test_it_should_load_and_run_complex_json_without_configuration() {
        let flows_json = make_flows_json_that_contains_subflows();
        let engine = build_test_engine(flows_json).unwrap();
        let msgs = engine.run_once(1, Duration::from_millis(200)).await.unwrap();
        assert_eq!(msgs.len(), 1);
        let msg = msgs[0].as_variant_object();
        assert_eq!(msg.get("payload").unwrap(), &Variant::from(123 * 2));
    }

    #[tokio::test]
    async fn test_it_should_json_flows_multiple_times() {
        let flows_json = make_flows_json_that_contains_subflows();
        for _ in 0..10 {
            let res = build_test_engine(flows_json.clone());
            assert!(res.is_ok());
        }
    }
}
