use std::cmp::Ordering;
use std::sync::{Arc, Weak};

use common_nodes::catch::{CatchNode, CatchNodeScope};
use dashmap::DashMap;
use itertools::Itertools;
use serde::Deserialize;
use tokio::sync::{Mutex, RwLock};
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;

use super::context::Context;
use super::engine::{Engine, WeakEngine};
use super::group::{Group, GroupParent};
use super::registry::RegistryHandle;
use super::subflow::SubflowState;
use crate::runtime::env::*;
use crate::runtime::model::json::*;
use crate::runtime::model::*;
use crate::runtime::nodes::*;
use crate::runtime::registry::Registry;
use crate::EdgelinkError;

const NODE_MSG_CHANNEL_CAPACITY: usize = 32;

pub type FlowNodeTask = tokio::task::JoinHandle<()>;

#[derive(Debug, Clone, Deserialize)]
pub struct FlowArgs {
    pub node_msg_queue_capacity: usize,
}

impl FlowArgs {
    pub fn load(cfg: Option<&config::Config>) -> crate::Result<Self> {
        match cfg {
            Some(cfg) => match cfg.get::<Self>("runtime.flow") {
                Ok(res) => Ok(res),
                Err(config::ConfigError::NotFound(_)) => Ok(Self::default()),
                Err(e) => Err(e.into()),
            },
            _ => Ok(Self::default()),
        }
    }
}

impl Default for FlowArgs {
    fn default() -> Self {
        Self { node_msg_queue_capacity: 16 }
    }
}

#[derive(Debug, Clone)]
pub struct Flow {
    inner: Arc<InnerFlow>,
}

#[derive(Debug, Clone)]
pub struct WeakFlow {
    inner: Weak<InnerFlow>,
}

impl WeakFlow {
    pub fn upgrade(&self) -> Option<Flow> {
        Weak::upgrade(&self.inner).map(|x| Flow { inner: x })
    }
}

#[derive(Debug, Clone)]
pub enum FlowKind {
    GlobalFlow,
    Subflow,
}

#[derive(Debug)]
struct InnerFlow {
    id: ElementId,
    parent: Option<ElementId>,
    label: String,
    disabled: bool,
    _args: FlowArgs,
    ordering: usize,
    type_str: &'static str,

    engine: WeakEngine,

    stop_token: CancellationToken,

    pub(crate) groups: DashMap<ElementId, Group>,
    pub(crate) nodes: DashMap<ElementId, Arc<dyn FlowNodeBehavior>>,
    pub(crate) complete_nodes_map: DashMap<ElementId, Vec<Arc<dyn FlowNodeBehavior>>>,
    pub(crate) catch_nodes: std::sync::RwLock<Vec<Arc<dyn FlowNodeBehavior>>>,
    pub(crate) _context: RwLock<Variant>,
    pub(crate) node_tasks: Mutex<JoinSet<()>>,

    subflow_state: Option<SubflowState>,

    envs: Arc<EnvStore>,
    context: Arc<Context>,
}

impl FlowsElement for Flow {
    fn id(&self) -> ElementId {
        self.inner.id
    }

    fn name(&self) -> &str {
        &self.inner.label
    }

    fn type_str(&self) -> &'static str {
        self.inner.type_str
    }

    fn ordering(&self) -> usize {
        self.inner.ordering
    }

    fn parent_element(&self) -> Option<ElementId> {
        self.inner.parent
    }

    fn context(&self) -> Arc<Context> {
        self.inner.context.clone()
    }

    fn as_any(&self) -> &dyn ::std::any::Any {
        self
    }

    fn is_disabled(&self) -> bool {
        self.inner.disabled
    }

    fn get_path(&self) -> String {
        if let Some(parent_id) = self.parent_element() {
            self.inner.engine.upgrade().unwrap().find_flow_node_by_id(&parent_id).unwrap().get_path()
        } else {
            self.inner.id.to_string()
        }
    }
}

impl Flow {
    pub fn downgrade(&self) -> WeakFlow {
        WeakFlow { inner: Arc::downgrade(&self.inner) }
    }

    async fn start_nodes(&self, stop_token: CancellationToken) -> crate::Result<()> {
        let nodes_ordering =
            self.inner.nodes.iter().sorted_by(|a, b| a.ordering().cmp(&b.ordering())).map(|x| x.value().clone());

        for node in nodes_ordering.into_iter() {
            if node.get_node().disabled {
                log::warn!("------ Skipping disabled node {}.", node);
                continue;
            }

            // Start the async-task of each flow node
            log::info!("------ Starting node {}...", node,);

            let child_stop_token = stop_token.clone();
            node.on_starting().await;
            self.inner.node_tasks.lock().await.spawn(async move {
                let node_ref = node.as_ref();
                let _ = node.clone().run(child_stop_token.child_token()).await;
                log::info!("------ {} has been stopped.", node_ref,);
            });
        }

        Ok(())
    }

    async fn stop_nodes(&self) -> crate::Result<()> {
        while self.inner.node_tasks.lock().await.join_next().await.is_some() {
            //
        }
        Ok(())
    }

    pub(crate) fn new(
        engine: &Engine,
        flow_config: RedFlowConfig,
        reg: &RegistryHandle,
        options: Option<&config::Config>,
    ) -> crate::Result<Flow> {
        let flow_kind = match flow_config.type_name.as_str() {
            "tab" => FlowKind::GlobalFlow,
            "subflow" => FlowKind::Subflow,
            _ => return Err(EdgelinkError::BadFlowsJson("Unsupported flow type".to_string()).into()),
        };

        let subflow_instance = flow_config.subflow_node_id.and_then(|x| engine.find_flow_node_by_id(&x));

        let mut envs_builder = EnvStoreBuilder::default();
        envs_builder = match flow_kind {
            FlowKind::GlobalFlow => envs_builder.with_parent(&engine.get_envs()),
            FlowKind::Subflow => {
                if let Some(ref instance) = subflow_instance {
                    envs_builder.with_parent(&instance.get_envs())
                } else {
                    log::warn!("Cannot found the instance node of the subflow: id='{}'", flow_config.id);
                    envs_builder.with_parent(&engine.get_envs())
                }
            }
        };
        if let Some(env_json) = flow_config.rest.get("env") {
            envs_builder = envs_builder.load_json(env_json);
        }
        if let Some(ref instance) = subflow_instance {
            // merge from subflow instance
            envs_builder = envs_builder.update_with(instance.get_envs().as_ref());
        }

        envs_builder = match flow_kind {
            FlowKind::GlobalFlow => envs_builder.extends([
                ("NR_FLOW_ID".into(), flow_config.id.to_string().into()),
                ("NR_FLOW_NAME".into(), flow_config.label.clone().into()),
            ]),
            FlowKind::Subflow => {
                if subflow_instance.is_none() {
                    return Err(
                        EdgelinkError::BadFlowsJson("The ID of Sub-flow instance node is None".to_string()).into()
                    );
                }
                let subflow_instance = subflow_instance.as_ref().unwrap().clone();
                envs_builder.extends([
                    ("NR_SUBFLOW_ID".into(), subflow_instance.id().to_string().into()),
                    ("NR_SUBFLOW_NAME".into(), subflow_instance.name().into()),
                    (
                        "NR_SUBFLOW_PATH".into(),
                        format!("{}/{}", subflow_instance.get_flow().upgrade().unwrap().id(), subflow_instance.id())
                            .into(),
                    ),
                ])
            }
        };
        let envs = envs_builder.build();

        let context = engine.get_context_manager().new_context(&engine.context(), flow_config.id.to_string());
        let args = FlowArgs::load(options)?;

        let inner_flow = InnerFlow {
            id: flow_config.id,
            parent: subflow_instance.clone().map(|x| x.id()),
            engine: engine.downgrade(),
            label: flow_config.label.clone(),
            disabled: flow_config.disabled,
            ordering: flow_config.ordering,
            _args: args.clone(),
            type_str: match flow_kind {
                FlowKind::GlobalFlow => "flow",
                FlowKind::Subflow => "subflow",
            },
            groups: DashMap::new(),
            nodes: DashMap::new(),
            complete_nodes_map: DashMap::new(),
            catch_nodes: std::sync::RwLock::new(Vec::new()),
            _context: RwLock::new(Variant::empty_object()),
            node_tasks: Mutex::new(JoinSet::new()),

            subflow_state: match flow_kind {
                FlowKind::Subflow => Some(SubflowState::new(engine, &flow_config, &args)?),
                FlowKind::GlobalFlow => None,
            },
            envs,
            context,
            stop_token: CancellationToken::new(),
            // groups: HashMap::new(), //   flow_config.groups.iter().map(|g| Group::new_flow_group(config, flow))
        };
        let flow = Flow { inner: Arc::new(inner_flow) };

        flow.populate_groups(&flow_config)?;
        flow.populate_nodes(&flow_config, reg.as_ref(), engine)?;

        if let Some(subflow_state) = &flow.inner.subflow_state {
            subflow_state.populate_in_nodes(&flow, &flow_config)?;
        }

        Ok(flow)
    }

    fn populate_groups(&self, flow_config: &RedFlowConfig) -> crate::Result<()> {
        if !self.inner.groups.is_empty() {
            self.inner.groups.clear();
        }
        // Adding root groups
        let root_group_configs = flow_config.groups.iter().filter(|gc| gc.z == self.id());
        for gc in root_group_configs {
            let group = match &gc.g {
                // Subgroup
                Some(parent_id) => Group::new_subgroup(
                    gc,
                    &self.inner.groups.get(parent_id).map(|x| x.value().clone()).ok_or(
                        EdgelinkError::InvalidOperation(format!("cannot found parent group id `{}`", parent_id)),
                    )?,
                )?,

                // Root group
                None => Group::new_flow_group(gc, self)?,
            };
            self.inner.groups.insert(group.id(), group);
        }
        Ok(())
    }

    fn populate_nodes(&self, flow_config: &RedFlowConfig, reg: &dyn Registry, engine: &Engine) -> crate::Result<()> {
        // Adding nodes
        for node_config in flow_config.nodes.iter() {
            let meta_node = if let Some(meta_node) = reg.get(&node_config.type_name) {
                meta_node
            } else if node_config.type_name.starts_with("subflow:") {
                reg.get("subflow").expect("The `subflow` node must be existed")
            } else {
                log::warn!(
                    "Unknown flow node type: (type='{}', id='{}', name='{}')",
                    node_config.type_name,
                    node_config.id,
                    node_config.name
                );
                reg.get("unknown.flow").expect("The `unknown.flow` node must be existed")
            };

            let node = match meta_node.factory {
                NodeFactory::Flow(factory) => {
                    let mut node_state = self.new_flow_node_state(meta_node, node_config, engine).map_err(|e| {
                        log::error!("Failed to create flow node(id='{}'): {:?}", node_config.id, e);
                        e
                    })?;

                    // Redirect all the output node wires in the subflow to the output port of the subflow.
                    if let Some(subflow_state) = &self.inner.subflow_state {
                        for (subflow_port_index, red_port) in flow_config.out_ports.iter().enumerate() {
                            let red_wires = red_port.wires.iter().filter(|x| x.id == node_state.id);
                            for red_wire in red_wires {
                                // Makre sure the target node has one port at least!
                                if node_state.ports.is_empty() {
                                    node_state.ports.push(Port::empty());
                                }
                                if let Some(node_port) = node_state.ports.get_mut(red_wire.port) {
                                    let subflow_tx_port = {
                                        let tx_ports_lock =
                                            subflow_state.tx_ports.read().expect("read subflow tx_ports lock");
                                        tx_ports_lock[subflow_port_index].clone()
                                    };
                                    let node_wire = PortWire { msg_sender: subflow_tx_port.msg_tx.clone() };
                                    node_port.wires.push(node_wire)
                                } else {
                                    return Err(EdgelinkError::BadFlowsJson(format!(
                                        "Invalid port '{}' for subflow: {:?}",
                                        red_wire.port, subflow_state
                                    ))
                                    .into());
                                }
                            }
                        }
                    }

                    match factory(self, node_state, node_config) {
                        Ok(node) => {
                            log::debug!("------ The node {} has been built.", node);
                            node
                        }
                        Err(err) => {
                            log::error!("Failed to build node from {}: {}", node_config, err);
                            log::debug!("Node JSON:\n{}", node_config.rest);
                            return Err(err);
                        }
                    }
                }
                NodeFactory::Global(_) => {
                    return Err(EdgelinkError::NotSupported(format!(
                        "Must be a flow node: Node(id={0}, type='{1}')",
                        flow_config.id, flow_config.type_name
                    ))
                    .into())
                }
            };

            let arc_node: Arc<dyn FlowNodeBehavior> = Arc::from(node);
            arc_node.on_loaded();
            self.inner.nodes.insert(node_config.id, arc_node.clone());

            log::debug!("------ {} has been loaded!", arc_node);

            self.register_internal_node(arc_node, node_config)?;
        }

        // Sort the `catch` nodes
        {
            let mut catch_nodes = self.inner.catch_nodes.write().expect("`catch_nodes` write lock");
            catch_nodes.sort_by(|a, b| {
                let a = a.as_any().downcast_ref::<CatchNode>().unwrap();
                let b = b.as_any().downcast_ref::<CatchNode>().unwrap();
                if a.scope.as_bool() && !b.scope.as_bool() {
                    Ordering::Greater
                } else if !a.scope.as_bool() && b.scope.as_bool() {
                    Ordering::Less
                } else if a.scope.as_bool() && b.scope.as_bool() {
                    Ordering::Equal
                } else if a.uncaught && !b.uncaught {
                    Ordering::Greater
                } else if !a.uncaught && b.uncaught {
                    Ordering::Less
                } else {
                    Ordering::Equal
                }
            });
        }

        Ok(())
    }

    fn register_internal_node(
        &self,
        node: Arc<dyn FlowNodeBehavior>,
        node_config: &RedFlowNodeConfig,
    ) -> crate::Result<()> {
        match node.get_node().type_str {
            "complete" => self.register_complete_node(node, node_config)?,

            "catch" => {
                let mut catch_nodes = self.inner.catch_nodes.write().expect("`catch_nodes` write lock");
                catch_nodes.push(node.clone());
            }

            // ignore normal nodes
            &_ => {}
        }
        Ok(())
    }

    fn register_complete_node(
        &self,
        node: Arc<dyn FlowNodeBehavior>,
        node_config: &RedFlowNodeConfig,
    ) -> crate::Result<()> {
        if let Some(scope) = node_config.rest.get("scope").and_then(|x| x.as_array()) {
            for src_id in scope {
                if let Some(src_id) = helpers::parse_red_id_value(src_id) {
                    if let Some(ref mut complete_nodes) = self.inner.complete_nodes_map.get_mut(&src_id) {
                        if !complete_nodes.iter().any(|x| x.id() == node.id()) {
                            complete_nodes.push(node.clone());
                        } else {
                            return Err(EdgelinkError::InvalidOperation(format!(
                                "The connection of the {} to the `complete` node already existed!",
                                node
                            ))
                            .into());
                        }
                    } else {
                        self.inner.complete_nodes_map.insert(src_id, Vec::from([node.clone()]));
                    }
                }
            }
            Ok(())
        } else {
            Err(EdgelinkError::BadFlowsJson(format!("CompleteNode has no 'scope' property: {}", node)).into())
        }
    }

    pub fn is_subflow(&self) -> bool {
        self.inner.subflow_state.is_some()
    }

    pub fn get_all_flow_nodes(&self) -> Vec<Arc<dyn FlowNodeBehavior>> {
        self.inner.nodes.iter().map(|x| x.value().clone()).collect()
    }

    pub fn get_node_by_id(&self, id: &ElementId) -> Option<Arc<dyn FlowNodeBehavior>> {
        self.inner.nodes.get(id).map(|x| x.value().clone())
    }

    pub fn get_node_by_name(&self, name: &str) -> crate::Result<Option<Arc<dyn FlowNodeBehavior>>> {
        let iter = self.inner.nodes.iter().filter(|val| val.name() == name);
        let nfound = iter.clone().count();
        if nfound == 1 {
            Ok(iter.clone().next().map(|x| x.clone()))
        } else if nfound == 0 {
            Ok(None)
        } else {
            Err(EdgelinkError::InvalidOperation(format!("There are multiple node with name '{}'", name)).into())
        }
    }

    pub fn engine(&self) -> Option<Engine> {
        self.inner.engine.upgrade()
    }

    pub fn get_envs(&self) -> Arc<EnvStore> {
        self.inner.envs.clone()
    }

    pub fn get_env(&self, key: &str) -> Option<Variant> {
        self.inner.envs.evalute_env(key)
    }

    pub async fn start(&self) -> crate::Result<()> {
        // let mut state = self.shared.state.write().await;

        if self.is_subflow() {
            log::info!("---- Starting Subflow (id={})...", self.id());
        } else {
            log::info!("---- Starting Flow (id={})...", self.id());
        }

        if let Some(subflow_state) = &self.inner.subflow_state {
            log::info!("------ Starting the forward tasks of the subflow...");
            subflow_state.start_tx_tasks(self.inner.stop_token.clone()).await?;
        }

        {
            self.start_nodes(self.inner.stop_token.clone()).await?;
        }

        Ok(())
    }

    pub async fn stop(&self) -> crate::Result<()> {
        if self.is_subflow() {
            log::info!("---- Stopping Subflow (id={})...", self.id());
        } else {
            log::info!("---- Stopping Flow (id={})...", self.id());
        }

        self.inner.stop_token.cancel();

        // Wait all subflow senders to stop
        /*
        if let Some(ss) = &self.subflow_state {
            let mut ss = ss.write().unwrap();
            ss.stop_tx_tasks().await?;
        }
        */

        // Wait all nodes
        {
            self.stop_nodes().await?;
        }
        log::info!("---- All node in flow/subflow(id='{}') has been stopped.", self.id());

        Ok(())
    }

    pub async fn notify_node_uow_completed(&self, emitter_id: &ElementId, msg: MsgHandle, cancel: CancellationToken) {
        if let Some(complete_nodes) = self.inner.complete_nodes_map.get(emitter_id) {
            for complete_node in complete_nodes.iter() {
                let to_send = msg.deep_clone(true).await;
                match complete_node.inject_msg(to_send, cancel.child_token()).await {
                    Ok(()) => {}
                    Err(err) => {
                        log::warn!("Failed to inject msg in notify_node_completed(): {}", err.to_string());
                    }
                }
            }
        }
    }

    pub async fn inject_msg(&self, msg: MsgHandle, cancel: CancellationToken) -> crate::Result<()> {
        tokio::select! {
            result = self.inject_msg_internal(msg, cancel.clone()) => result,

            _ = cancel.cancelled() => {
                // The token was cancelled
                Err(EdgelinkError::TaskCancelled.into())
            }
        }
    }

    async fn inject_msg_internal(&self, msg: MsgHandle, cancel: CancellationToken) -> crate::Result<()> {
        if let Some(subflow_state) = &self.inner.subflow_state {
            let mut msg_sent = false;
            let in_nodes = subflow_state.in_nodes.read().expect("read in_nodes lock").clone();
            for node in in_nodes.iter() {
                if !msg_sent {
                    node.inject_msg(msg.clone(), cancel.clone()).await?;
                } else {
                    node.inject_msg(msg.deep_clone(true).await, cancel.clone()).await?;
                }
                msg_sent = true;
            }
            Ok(())
        } else {
            Err(EdgelinkError::InvalidOperation("This is not a subflow!".into()).into())
        }
    }

    fn new_flow_node_state(
        &self,
        meta_node: &MetaNode,
        node_config: &RedFlowNodeConfig,
        engine: &Engine,
    ) -> crate::Result<FlowNode> {
        let mut ports = Vec::new();
        let (tx_root, rx) = tokio::sync::mpsc::channel(NODE_MSG_CHANNEL_CAPACITY);
        // Convert the Node-RED wires elements to ours
        for red_port in node_config.wires.iter() {
            let mut wires = Vec::new();
            for nid in red_port.node_ids.iter() {
                // First we find the node in this flow
                let node_in_flow = self.inner.nodes.get(nid).map(|x| x.value().clone());
                // Next we find the node in the entire engine, otherwise there is an error
                let node_in_engine = engine.find_flow_node_by_id(nid);
                let node_entry = node_in_flow.or(node_in_engine).ok_or(EdgelinkError::InvalidOperation(format!(
                    "[flow:{}] Referenced node not found [this_node.id='{}' this_node.name='{}', referenced_node.id='{}']",
                    self.name(), node_config.id, node_config.name, nid
                )))?;
                let tx = node_entry.get_node().msg_tx.to_owned();
                let pw = PortWire {
                    // target_node_id: *nid,
                    // target_node: Arc::downgrade(node_entry),
                    msg_sender: tx,
                };
                wires.push(pw);
            }
            let port = Port { wires };
            ports.push(port);
        }

        let group = match &node_config.g {
            Some(gid) => match self.inner.groups.get(gid) {
                Some(g) => Some(g.value().clone()),
                None => {
                    return Err(EdgelinkError::InvalidOperation(format!(
                        "Can not found the group id in groups: id='{}'",
                        gid
                    ))
                    .into());
                }
            },
            None => None,
        };

        let mut envs_builder = EnvStoreBuilder::default();
        if let Some(ref g) = group {
            envs_builder = envs_builder.with_parent(&g.get_envs());
        } else {
            envs_builder = envs_builder.with_parent(&self.get_envs());
        }
        if let Some(env_json) = node_config.rest.get("env") {
            envs_builder = envs_builder.load_json(env_json);
        }
        let envs = envs_builder
            .extends([
                ("NR_NODE_ID".into(), Variant::String(node_config.id.to_string())),
                ("NR_NODE_NAME".into(), Variant::String(node_config.name.clone())),
                ("NR_NODE_PATH".into(), Variant::String(format!("{}/{}", self.get_path(), node_config.id))),
            ])
            .build();
        let context = engine.get_context_manager().new_context(&self.inner.context, node_config.id.to_string());

        Ok(FlowNode {
            id: node_config.id,
            name: node_config.name.clone(),
            type_str: meta_node.type_,
            ordering: node_config.ordering,
            disabled: node_config.disabled,
            active: node_config.active.unwrap_or(true),
            flow: self.downgrade(),
            msg_tx: tx_root,
            msg_rx: MsgReceiverHolder::new(rx),
            ports,
            group: group.map(|g| g.downgrade()),
            envs,
            context,
            on_received: MsgEventSender::new(1),
            on_completed: MsgEventSender::new(1),
            on_error: MsgEventSender::new(1),
        })
    }

    pub async fn handle_error(
        &self,
        node: &dyn FlowNodeBehavior,
        log_message: &str,
        msg: Option<&MsgHandle>,
        reporting_node: Option<&dyn FlowNodeBehavior>,
        cancel: CancellationToken,
    ) -> crate::Result<bool> {
        let reporting_node = if let Some(rn) = reporting_node { rn } else { node };

        // TODO: use SmallVec
        let mut candidates = Vec::new();
        {
            let catch_nodes = self.inner.catch_nodes.read().expect("`catch_nodes` read lock").clone();
            for catch_node_behavior in catch_nodes.iter() {
                let catch_node = catch_node_behavior.as_any().downcast_ref::<CatchNode>().expect("CatchNode");
                if catch_node.group().is_some()
                    && catch_node.scope == CatchNodeScope::Group
                    && reporting_node.group().is_none()
                {
                    // Catch node inside a group, reporting node not in a group - skip it
                    return Ok(true);
                }

                if let CatchNodeScope::Nodes(ref scope) = catch_node.scope {
                    // Catch node has a scope set and it doesn't include the reporting node
                    if !scope.contains(&reporting_node.id()) {
                        return Ok(true);
                    }
                }
                let mut distance: usize = 0;
                let catch_node_group_id = catch_node.group().map(|x| x.id());
                if let Some(ref reporting_node_group_id) = reporting_node.group().map(|x| x.id()) {
                    // Reporting node inside a group. Calculate the distance between it and the catch node
                    let mut containing_group_id = Some(*reporting_node_group_id);
                    while containing_group_id.is_some() && containing_group_id != catch_node_group_id {
                        distance += 1;
                        let containing_group_parent = self
                            .inner
                            .groups
                            .get(&containing_group_id.unwrap())
                            .map(|x| x.value().get_parent().clone());
                        containing_group_id = if let Some(GroupParent::Group(g)) = containing_group_parent {
                            Some(g.upgrade().expect("Group").id())
                        } else {
                            None
                        };
                    }
                    if containing_group_id.is_none()
                        && catch_node.group().is_some()
                        && catch_node.scope == CatchNodeScope::Group
                    {
                        // This catch node is in a group, but not in the same hierachy
                        // the reporting node is in
                        return Ok(true);
                    }
                }
                candidates.push((distance, catch_node_behavior.clone()))
            }
        }
        candidates.sort_by(|a, b| a.0.cmp(&b.0));

        let mut handled = false;
        let mut handled_by_uncaught = false;
        for candidate in candidates.iter() {
            let catch_node = candidate.1.as_any().downcast_ref::<CatchNode>().unwrap();
            if catch_node.uncaught && !handled_by_uncaught {
                if handled {
                    return Ok(true);
                }
                handled_by_uncaught = true;
            }
            let mut error_msg = if let Some(msg) = msg {
                let msg_lock = msg.read().await;
                msg_lock.clone()
            } else {
                Msg::default()
            };
            let error_object = Variant::from(serde_json::json!({
                "message": log_message.to_string(),
                "source": {
                    "id": node.id(),
                    "type": node.type_str().to_string(),
                    "name": node.name(),
                    "count": 1, // TODO
                }
            }));
            error_msg.set("error".into(), error_object);
            let error_msg = MsgHandle::new(error_msg);
            catch_node.inject_msg(error_msg, cancel.clone()).await?;

            handled = true;
        }
        Ok(handled)
    }
}
