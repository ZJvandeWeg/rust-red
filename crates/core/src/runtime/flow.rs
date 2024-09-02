use smallvec::SmallVec;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Weak};
use tokio::sync::RwLock;
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;

use crate::red::json::*;
use crate::runtime::engine::FlowEngine;
use crate::runtime::model::*;
use crate::runtime::nodes::*;
use crate::runtime::registry::Registry;
use crate::EdgelinkError;

use super::group::Group;
use crate::red::eval;
use crate::red::json::{RedEnvEntry, RedPropertyType};

const NODE_MSG_CHANNEL_CAPACITY: usize = 32;

pub type FlowNodeTask = tokio::task::JoinHandle<()>;

struct SubflowOutputPort {
    index: usize,
    owner: Weak<Flow>,
    msg_tx: MsgSender,
    msg_rx: MsgReceiverHolder,
}

struct SubflowState {
    instance_node: Option<Arc<dyn FlowNodeBehavior>>,
    in_nodes: Vec<Arc<dyn FlowNodeBehavior>>,
    tx_tasks: JoinSet<()>,
    tx_ports: Vec<Arc<SubflowOutputPort>>,
}

pub(crate) struct FlowState {
    pub(crate) groups: HashMap<ElementId, Arc<Group>>,
    pub(crate) nodes: HashMap<ElementId, Arc<dyn FlowNodeBehavior>>,
    pub(crate) complete_nodes: HashMap<ElementId, Arc<dyn FlowNodeBehavior>>,
    pub(crate) complete_nodes_map: HashMap<ElementId, HashSet<ElementId>>,
    pub(crate) catch_nodes: HashMap<ElementId, Arc<dyn FlowNodeBehavior>>,
    pub(crate) nodes_ordering: Vec<ElementId>,
    pub(crate) _context: Variant,
    pub(crate) node_tasks: JoinSet<()>,
}

#[derive(Debug, Clone)]
enum FlowKind {
    GlobalFlow,
    Subflow,
}

pub struct Flow {
    pub id: ElementId,
    pub parent: Option<Weak<Self>>,
    pub label: String,
    pub disabled: bool,

    pub engine: Weak<FlowEngine>,

    pub stop_token: CancellationToken,

    state: std::sync::RwLock<FlowState>,
    subflow_state: Option<std::sync::RwLock<SubflowState>>,
}

impl GraphElement for Flow {
    fn parent(&self) -> Option<Weak<Flow>> {
        self.parent.clone()
    }

    fn parent_ref(&self) -> Option<Weak<dyn GraphElement>> {
        self.parent
            .as_ref()
            .and_then(|weak_parent| weak_parent.upgrade())
            .map(|arc| Arc::downgrade(&(arc as Arc<dyn GraphElement>)))
    }
}

impl SubflowOutputPort {
    async fn tx_task(&self, stop_token: CancellationToken) {
        while !stop_token.is_cancelled() {
            match self.msg_rx.recv_msg(stop_token.clone()).await {
                Ok(msg) => {
                    // Find out the subflow:xxx node
                    let instance_node = {
                        // 升级 Weak<Flow> 到 Arc<Flow>
                        let flow = self
                            .owner
                            .upgrade()
                            .expect("The owner of this sub-flow node has been released already!!!");

                        // 直接借用 flow，避免使用 clone
                        let subflow_state = flow
                            .subflow_state
                            .as_ref()
                            .expect("Subflow must have a subflow_state!");

                        // 获取读锁定的引用
                        let subflow_state_guard = subflow_state
                            .read()
                            .expect("Cannot acquire the lock of field `subflow_state`!!!");

                        // 克隆 instance_node，因为需要返回 Option<T> 的克隆
                        subflow_state_guard.instance_node.clone()
                    };

                    if let Some(instance_node) = instance_node {
                        let instance_node = instance_node.clone();
                        let envelope = Envelope {
                            port: self.index,
                            msg,
                        };
                        if let Err(e) = instance_node
                            .fan_out_one(&envelope, stop_token.clone())
                            .await
                        {
                            log::warn!("Failed to fan-out message: {:?}", e);
                        }
                    } else {
                        log::warn!("The sub-flow does not have a subflow node");
                    }
                }

                Err(e) => {
                    log::error!("Failed to receive msg in subflow_tx_task: {:?}", e);
                }
            }
        }
    }
}

impl SubflowState {
    fn populate_in_nodes(
        &mut self,
        flow_state: &FlowState,
        flow_config: &RedFlowConfig,
    ) -> crate::Result<()> {
        // this is a subflow with in ports
        for wire_obj in flow_config.in_ports.iter().flat_map(|x| x.wires.iter()) {
            if let Some(node) = flow_state.nodes.get(&wire_obj.id) {
                self.in_nodes.push(node.clone());
            } else {
                log::warn!("Can not found node(id='{}')", wire_obj.id);
            }
        }
        Ok(())
    }

    fn start_tx_tasks(&mut self, stop_token: CancellationToken) -> crate::Result<()> {
        for tx_port in self.tx_ports.iter() {
            let child_stop_token = stop_token.clone();
            let port_cloned = tx_port.clone();
            self.tx_tasks.spawn(async move {
                port_cloned.tx_task(child_stop_token.clone()).await;
            });
        }
        Ok(())
    }

    async fn stop_tx_tasks(&mut self) -> crate::Result<()> {
        while self.tx_tasks.join_next().await.is_some() {
            //
        }
        Ok(())
    }
}

impl FlowState {
    fn start_nodes(&mut self, stop_token: CancellationToken) -> crate::Result<()> {
        for node_id in self.nodes_ordering.iter() {
            let node = self.nodes[node_id].clone();

            if node.get_node().disabled {
                log::warn!("------ Skipping disabled node {}.", node);
                continue;
            }

            // Start the async-task of each flow node
            log::info!("------ Starting node {}...", node,);

            let child_stop_token = stop_token.clone();
            self.node_tasks.spawn(async move {
                let node_ref = node.as_ref();
                let _ = node.clone().run(child_stop_token.child_token()).await;
                log::info!("------ {} has been stopped.", node_ref,);
            });
        }

        Ok(())
    }

    pub async fn stop_nodes(&mut self) -> crate::Result<()> {
        while self.node_tasks.join_next().await.is_some() {
            //
        }
        Ok(())
    }
}

impl Flow {
    pub(crate) fn new(
        engine: Arc<FlowEngine>,
        flow_config: &RedFlowConfig,
        reg: Arc<dyn Registry>,
    ) -> crate::Result<Arc<Self>> {
        let flow_kind = match flow_config.type_name.as_str() {
            "tab" => FlowKind::GlobalFlow,
            "subflow" => FlowKind::Subflow,
            _ => {
                return Err(EdgelinkError::BadFlowsJson("Unsupported flow type".to_string()).into())
            }
        };

        let flow: Arc<Flow> = Arc::new(Flow {
            id: flow_config.id,
            parent: None, //TODO FIXME
            engine: Arc::downgrade(&engine),
            label: flow_config.label.clone(),
            disabled: flow_config.disabled,
            state: std::sync::RwLock::new(FlowState {
                groups: HashMap::new(),
                nodes: HashMap::new(),
                complete_nodes: HashMap::new(),
                complete_nodes_map: HashMap::new(),
                catch_nodes: HashMap::new(),
                nodes_ordering: Vec::new(),
                _context: Variant::new_empty_object(),
                node_tasks: JoinSet::new(),
            }),

            subflow_state: match flow_kind {
                FlowKind::Subflow => Some(std::sync::RwLock::new(SubflowState {
                    instance_node: None,
                    in_nodes: Vec::new(),
                    tx_tasks: JoinSet::new(),
                    tx_ports: Vec::new(),
                })),
                FlowKind::GlobalFlow => None,
            },
            stop_token: CancellationToken::new(),
            // groups: HashMap::new(), //   flow_config.groups.iter().map(|g| Group::new_flow_group(config, flow))
        });

        // Add empty subflow forward ports
        if let Some(subflow_state) = &flow.subflow_state {
            let mut subflow_state = subflow_state.write().unwrap();

            if let Some(subflow_node_id) = flow_config.subflow_node_id {
                subflow_state.instance_node = engine.find_flow_node_by_id(&subflow_node_id);
            }

            for (index, _) in flow_config.out_ports.iter().enumerate() {
                let (msg_root_tx, msg_rx) = tokio::sync::mpsc::channel(NODE_MSG_CHANNEL_CAPACITY);

                subflow_state.tx_ports.push(Arc::new(SubflowOutputPort {
                    index,
                    owner: Arc::downgrade(&flow),
                    msg_tx: msg_root_tx.clone(),
                    msg_rx: MsgReceiverHolder::new(msg_rx),
                }));
            }
        }

        {
            let flow = flow.clone();
            let mut state = flow.state.write().unwrap();

            flow.clone().populate_groups(&mut state, flow_config)?;

            flow.clone().populate_nodes(&mut state, flow_config, reg)?;
        }

        if let Some(subflow_state) = &flow.subflow_state {
            let flow_state = flow.state.write().unwrap();
            let mut subflow_state = subflow_state.write().unwrap();

            subflow_state.populate_in_nodes(&flow_state, flow_config)?;
        }

        Ok(flow)
    }

    fn populate_groups(
        self: Arc<Self>,
        state: &mut FlowState,
        flow_config: &RedFlowConfig,
    ) -> crate::Result<()> {
        if !state.groups.is_empty() {
            state.groups.clear();
        }
        // Adding root groups
        let root_group_configs = flow_config.groups.iter().filter(|gc| gc.z == *self.id());
        for gc in root_group_configs {
            let group = match &gc.g {
                // Subgroup
                Some(parent_id) => Group::new_subgroup(
                    gc,
                    Arc::downgrade(&self),
                    Arc::downgrade(&state.groups[parent_id]),
                )?,

                // Root group
                None => Group::new_flow_group(gc, Arc::downgrade(&self))?,
            };
            state.groups.insert(group.id, Arc::new(group));
        }
        Ok(())
    }

    fn populate_nodes(
        self: Arc<Self>,
        state: &mut FlowState,
        flow_config: &RedFlowConfig,
        reg: Arc<dyn Registry>,
    ) -> crate::Result<()> {
        // Adding nodes
        for node_config in flow_config.nodes.iter() {
            let meta_node = if let Some(meta_node) = reg.get(&node_config.type_name) {
                meta_node
            } else if node_config.type_name.starts_with("subflow:") {
                reg.get("subflow")
                    .expect("The `subflow` node must be existed")
            } else {
                log::warn!(
                    "Unknown flow node type: (type='{}', id='{}')",
                    node_config.type_name,
                    node_config.id
                );
                reg.get("unknown.flow")
                    .expect("The `unknown.flow` node must be existed")
            };

            let node = match meta_node.factory {
                NodeFactory::Flow(factory) => {
                    let mut node_state = self
                        .clone()
                        .new_flow_node_state(state, node_config)
                        .map_err(|e| {
                            log::error!(
                                "Failed to create flow node(id='{}'): {:?}",
                                node_config.id,
                                e
                            );
                            e
                        })?;

                    // Redirect all the output node wires in the subflow to the output port of the subflow.
                    if let Some(subflow_state) = &self.subflow_state {
                        let subflow_state = subflow_state.read().unwrap();
                        for (subflow_port_index, red_port) in
                            flow_config.out_ports.iter().enumerate()
                        {
                            let red_wires = red_port.wires.iter().filter(|x| x.id == node_state.id);
                            for red_wire in red_wires {
                                if let Some(node_port) = node_state.ports.get_mut(red_wire.port) {
                                    let subflow_tx_port =
                                        &subflow_state.tx_ports[subflow_port_index];
                                    let node_wire = PortWire {
                                        msg_sender: subflow_tx_port.msg_tx.clone(),
                                    };
                                    node_port.wires.push(node_wire)
                                } else {
                                    return Err(EdgelinkError::BadFlowsJson(format!(
                                        "Bad port for subflow: {}",
                                        red_wire.port
                                    ))
                                    .into());
                                }
                            }
                        }
                    }

                    factory(&self, node_state, node_config)?
                }
                NodeFactory::Global(_) => {
                    return Err(EdgelinkError::NotSupported(format!(
                        "Must be a flow node: Node(id={0}, type='{1}')",
                        flow_config.id, flow_config.type_name
                    ))
                    .into())
                }
            };

            state.nodes_ordering.push(node.id());
            state.nodes.insert(node_config.id, node.clone());

            log::debug!("------ {} has been loaded!", node);

            self.register_internal_node(node, state, node_config)?;
        }
        Ok(())
    }

    fn register_internal_node(
        &self,
        node: Arc<dyn FlowNodeBehavior>,
        state: &mut FlowState,
        node_config: &RedFlowNodeConfig,
    ) -> crate::Result<()> {
        match node.get_node().type_.as_str() {
            "complete" => {
                state.complete_nodes.insert(node_config.id, node.clone());

                if let Some(scope) = node_config.json.get("scope").and_then(|x| x.as_array()) {
                    for src_id in scope {
                        if let Some(src_id) = helpers::parse_red_id_value(src_id) {
                            if let Some(set) = state.complete_nodes_map.get_mut(&src_id) {
                                set.insert(node.id());
                            } else {
                                state
                                    .complete_nodes_map
                                    .insert(src_id, HashSet::from([node.id()]));
                            }
                        }
                    }
                }
            }
            "catch" => {
                state.catch_nodes.insert(node_config.id, node.clone());
            }
            // ignore normal nodes
            &_ => {}
        }
        Ok(())
    }

    pub fn id(&self) -> &ElementId {
        &self.id
    }

    pub fn parent(&self) -> &Option<Weak<Flow>> {
        &self.parent
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn disabled(&self) -> bool {
        self.disabled
    }

    pub fn is_subflow(&self) -> bool {
        self.subflow_state.is_some()
    }

    pub fn get_all_flow_nodes(&self) -> Vec<Arc<dyn FlowNodeBehavior>> {
        self.state.read().expect("Must be readable").nodes.values().cloned().collect()
    }

    pub fn get_node_by_id(&self, id: &ElementId) -> Option<Arc<dyn FlowNodeBehavior>> {
        self.state.read().ok()?.nodes.get(id).cloned()
    }

    pub fn get_node_by_name(&self, name: &str) -> crate::Result<Option<Arc<dyn FlowNodeBehavior>>> {
        let state = self.state.read().expect("The state must be available!");
        let iter = state.nodes.values().filter(move |&val| val.name() == name);
        let nfound = iter.clone().count();
        if nfound == 1 {
            Ok(iter.clone().next().cloned())
        } else if nfound == 0 {
            Ok(None)
        } else {
            Err(EdgelinkError::InvalidOperation(format!(
                "There are multiple node with name '{}'",
                name
            ))
            .into())
        }
    }

    pub fn get_setting(&self, key: &str) -> Variant {
        match key {
            "NR_FLOW_NAME" => Variant::String(self.label.clone()),
            "NR_FLOW_ID" => Variant::String(self.id.clone().to_string()),
            _ => {
                if let Some(pkey) = key.strip_prefix("$parent.") {
                    match self.parent() {
                        Some(parent) => match parent.upgrade() {
                            Some(parent) => parent.get_setting(pkey),
                            None => Variant::Null,
                        },
                        None => Variant::Null,
                    }
                } else {
                    Variant::Null
                }
            }
        }
    }

    pub fn eval_env_entries(
        &self,
        env_entries: &[RedEnvEntry],
    ) -> crate::Result<HashMap<String, Variant>> {
        let mut evaluated_entries: HashMap<String, Variant> = HashMap::new();

        // preprocessing
        for e in env_entries.iter().filter(|&x| x.name != "env") {
            let parsed_value = match e.type_name.as_str() {
                "bool" => Variant::Bool(e.value.parse::<bool>().unwrap_or(false)),
                "jsonata" => {
                    todo!();
                }
                _ => eval::evaluate_node_property(
                    &e.value,
                    &RedPropertyType::from(&e.type_name)?,
                    None,
                    None,
                )?,
            };
            evaluated_entries.insert(e.name.clone(), parsed_value);
        }

        // TODO JSONATA

        for e in env_entries.iter().filter(|&x| x.name == "env") {
            let env_value_text = if e.name == e.value {
                format!("$parent.${}", e.name)
            } else {
                e.value.clone()
            };
            let mut parsed_value =
                if let Some(existed_value) = evaluated_entries.get(&env_value_text) {
                    existed_value.clone()
                } else {
                    eval::evaluate_node_property(
                        &env_value_text,
                        &RedPropertyType::from(&e.type_name)?,
                        None,
                        None,
                    )?
                };
            parsed_value = if let Some(parsed_obj) = parsed_value.as_object() {
                if !parsed_obj.contains_key("__clone__") {
                    Variant::from([("value", parsed_value), ("__clone__", Variant::Bool(true))])
                } else {
                    parsed_value
                }
            } else {
                parsed_value
            };
            evaluated_entries.insert(e.name.clone(), parsed_value);
        }

        Ok(evaluated_entries)
    }

    pub async fn start(&self) -> crate::Result<()> {
        // let mut state = self.shared.state.write().await;

        if self.is_subflow() {
            log::info!("---- Starting Subflow (id={})...", self.id);
        } else {
            log::info!("---- Starting Flow (id={})...", self.id);
        }

        if let Some(subflow_state) = &self.subflow_state {
            log::info!("------ Starting the forward tasks of the subflow...");
            let mut subflow_state = subflow_state.write().unwrap();
            subflow_state.start_tx_tasks(self.stop_token.clone())?;
        }

        {
            let mut state = self.state.write().unwrap();
            state.start_nodes(self.stop_token.clone())?;
        }

        Ok(())
    }

    pub async fn stop(&self) -> crate::Result<()> {
        if self.is_subflow() {
            log::info!("---- Stopping Subflow (id={})...", self.id);
        } else {
            log::info!("---- Stopping Flow (id={})...", self.id);
        }

        self.stop_token.cancel();

        // Wait all subflow senders to stop
        if let Some(ss) = &self.subflow_state {
            let mut ss = ss.write().unwrap();
            ss.stop_tx_tasks().await?;
        }

        // Wait all nodes
        {
            let mut state = self.state.write().unwrap();
            state.stop_nodes().await?;
        }
        log::info!(
            "---- All node in flow/subflow(id='{}') has been stopped.",
            self.id
        );

        Ok(())
    }

    pub async fn notify_node_uow_completed(
        &self,
        emitter_id: &ElementId,
        msg: &Msg,
        cancel: CancellationToken,
    ) {
        if let Some(complete_nodes) = self.get_complete_nodes_by_emitter(emitter_id) {
            for complete_node in complete_nodes.iter() {
                let to_send = Arc::new(RwLock::new(msg.clone()));
                match complete_node
                    .inject_msg(to_send, cancel.child_token())
                    .await
                {
                    Ok(()) => {}
                    Err(err) => {
                        log::warn!(
                            "Failed to inject msg in notify_node_completed(): {}",
                            err.to_string()
                        );
                    }
                }
            }
        }
    }

    fn get_complete_nodes_by_emitter(
        &self,
        emitter_id: &ElementId,
    ) -> Option<SmallVec<[Arc<dyn FlowNodeBehavior>; 8]>> {
        let state = self.state.read().unwrap();
        state
            .complete_nodes_map
            .get(emitter_id)
            .map(|complete_nids| {
                complete_nids
                    .iter()
                    .filter_map(|k| state.complete_nodes.get(k))
                    .cloned()
                    .collect()
            })
    }

    pub async fn inject_msg(
        &self,
        msg: Arc<RwLock<Msg>>,
        cancel: CancellationToken,
    ) -> crate::Result<()> {
        tokio::select! {
            result = self.inject_msg_internal(msg, cancel.clone()) => result,

            _ = cancel.cancelled() => {
                // The token was cancelled
                Err(EdgelinkError::TaskCancelled.into())
            }
        }
    }

    async fn inject_msg_internal(
        &self,
        msg: Arc<RwLock<Msg>>,
        cancel: CancellationToken,
    ) -> crate::Result<()> {
        if let Some(subflow_state) = &self.subflow_state {
            let in_nodes = {
                let subflow_state = subflow_state.read().unwrap();
                subflow_state.in_nodes.clone()
            };
            let mut msg_sent = false;
            for node in in_nodes {
                if !msg_sent {
                    node.inject_msg(msg.clone(), cancel.clone()).await?;
                } else {
                    let to_clone = msg.read().await;
                    node.inject_msg(Arc::new(RwLock::new(to_clone.clone())), cancel.clone())
                        .await?;
                }
                msg_sent = true;
            }
            Ok(())
        } else {
            Err(EdgelinkError::InvalidOperation("This is not a subflow!".into()).into())
        }
    }

    fn new_flow_node_state(
        self: Arc<Self>,
        state: &FlowState,
        node_config: &RedFlowNodeConfig,
    ) -> crate::Result<FlowNode> {
        let mut ports = Vec::new();
        let (tx_root, rx) = tokio::sync::mpsc::channel(NODE_MSG_CHANNEL_CAPACITY);
        // Convert the Node-RED wires elements to ours
        for red_port in node_config.wires.iter() {
            let mut wires = Vec::new();
            for nid in red_port.node_ids.iter() {
                let node_entry = state.nodes.get(nid).ok_or(EdgelinkError::InvalidData(format!(
                    "Referenced node not found [this_node.id='{}' this_node.name='{}', referenced_node.id='{}']",
                    node_config.id,
                    node_config.name,
                    nid
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

        let group_ref = match &node_config.g {
            Some(gid) => match state.groups.get(gid) {
                Some(g) => Arc::downgrade(g),
                None => {
                    return Err(EdgelinkError::InvalidData(format!(
                        "Can not found the group id in groups: id='{}'",
                        gid
                    ))
                    .into());
                }
            },
            None => Weak::new(),
        };

        Ok(FlowNode {
            id: node_config.id,
            name: node_config.name.clone(),
            type_: node_config.type_name.clone(),
            disabled: node_config.disabled,
            flow: Arc::downgrade(&self),
            msg_tx: tx_root,
            msg_rx: MsgReceiverHolder::new(rx),
            ports,
            group: group_ref,
            on_received: MsgEventSender::new(1),
            on_completed: MsgEventSender::new(1),
            on_error: MsgEventSender::new(1),
        })
    }
}
