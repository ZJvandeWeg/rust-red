
use serde::Deserialize;
use smallvec::SmallVec;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Weak};
use tokio::sync::RwLock;
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;

use crate::red::json::*;
use crate::runtime::engine::FlowEngine;
use crate::runtime::group::Group;
use crate::runtime::model::*;
use crate::runtime::nodes::*;
use crate::runtime::registry::Registry;
use crate::EdgelinkError;

use super::{FlowArgs, FlowKind, FlowState, SubflowOutputPort, SubflowState};
use crate::red::eval;
use crate::red::json::{RedEnvEntry, RedPropertyType};

#[derive(Debug)]
pub(crate) struct FlowBuilder {
    pub id: ElementId,
    pub parent: Option<Weak<Self>>,
    pub label: String,
    pub disabled: bool,
    pub args: FlowArgs,

    pub engine: Weak<FlowEngine>,

    pub stop_token: CancellationToken,

    pub groups: HashMap<ElementId, Arc<Group>>,
    pub nodes: HashMap<ElementId, Arc<dyn FlowNodeBehavior>>,
    pub complete_nodes: HashMap<ElementId, Arc<dyn FlowNodeBehavior>>,
    pub complete_nodes_map: HashMap<ElementId, HashSet<ElementId>>,
    pub catch_nodes: HashMap<ElementId, Arc<dyn FlowNodeBehavior>>,
    pub nodes_ordering: Vec<ElementId>,
    pub _context: Variant,
    pub node_tasks: JoinSet<()>,

    pub subflow_state: Option<SubflowState>,
}

impl FlowBuilder {
    pub fn new(
        engine: Arc<FlowEngine>,
        flow_config: &RedFlowConfig,
        reg: Arc<dyn Registry>,
        options: Option<&config::Config>,
    ) -> crate::Result<Self> {
        let flow_kind = match flow_config.type_name.as_str() {
            "tab" => FlowKind::GlobalFlow,
            "subflow" => FlowKind::Subflow,
            _ => {
                return Err(EdgelinkError::BadFlowsJson("Unsupported flow type".to_string()).into())
            }
        };
        let mut flow = Self {
            id: flow_config.id,
            parent: None, //TODO FIXME
            engine: Arc::downgrade(&engine),
            label: flow_config.label.clone(),
            disabled: flow_config.disabled,
            args: FlowArgs::load(options)?,
            groups: HashMap::new(),
            nodes: HashMap::new(),
            complete_nodes: HashMap::new(),
            complete_nodes_map: HashMap::new(),
            catch_nodes: HashMap::new(),
            nodes_ordering: Vec::new(),
            _context: Variant::new_empty_object(),
            node_tasks: JoinSet::new(),

            subflow_state: match flow_kind {
                FlowKind::Subflow => Some(SubflowState {
                    instance_node: None,
                    in_nodes: Vec::new(),
                    tx_tasks: JoinSet::new(),
                    tx_ports: Vec::new(),
                }),
                FlowKind::GlobalFlow => None,
            },
            stop_token: CancellationToken::new(),
        };

        // Add empty subflow forward ports
        if let Some(ref mut subflow_state) = flow.subflow_state {
            if let Some(subflow_node_id) = flow_config.subflow_node_id {
                subflow_state.instance_node = engine.find_flow_node_by_id(&subflow_node_id);
            }

            for (index, _) in flow_config.out_ports.iter().enumerate() {
                let (msg_root_tx, msg_rx) =
                    tokio::sync::mpsc::channel(flow.args.node_msg_queue_capacity);

                subflow_state.tx_ports.push(Arc::new(SubflowOutputPort {
                    index,
                    owner: Weak::new(),
                    msg_tx: msg_root_tx.clone(),
                    msg_rx: MsgReceiverHolder::new(msg_rx),
                }));
            }
        }

        // flow.populate_groups(&mut state, flow_config)?;

        // flow.populate_nodes(&mut state, flow_config, reg)?;

        /*
        if let Some(ref mut subflow_state) = flow.subflow_state {
            subflow_state.populate_in_nodes(&flow.state, flow_config)?;
        }
        */

        Ok(flow)
    }

    fn populate_groups(mut self, flow_config: &RedFlowConfig) -> crate::Result<Self> {
        if !self.groups.is_empty() {
            self.groups.clear();
        }
        // Adding root groups
        let root_group_configs = flow_config.groups.iter().filter(|gc| gc.z == self.id);
        for gc in root_group_configs {
            let group = match &gc.g {
                // Subgroup
                Some(parent_id) => {
                    Group::new_subgroup(gc, Weak::new(), Arc::downgrade(&self.groups[parent_id]))?
                }

                // Root group
                None => Group::new_flow_group(gc, Weak::new())?,
            };
            self.groups.insert(group.id, Arc::new(group));
        }
        Ok(self)
    }

    fn populate_nodes(
        mut self,
        state: &mut FlowState,
        flow_config: &RedFlowConfig,
        reg: Arc<dyn Registry>,
    ) -> crate::Result<Self> {
        // Adding nodes
        for node_config in flow_config.nodes.iter() {
            let meta_node = if let Some(meta_node) = reg.get(&node_config.type_name) {
                meta_node
            } else if node_config.type_name.starts_with("subflow:") {
                reg.get("subflow")
                    .expect("The `subflow` node must be existed")
            } else {
                log::warn!(
                    "Unknown flow node type: (type='{}', id='{}', name='{}')",
                    node_config.type_name,
                    node_config.id,
                    node_config.name
                );
                reg.get("unknown.flow")
                    .expect("The `unknown.flow` node must be existed")
            };

            let node = match meta_node.factory {
                NodeFactory::Flow(factory) => {
                    let mut node_state =
                        self.new_flow_node_state(meta_node, node_config)
                            .map_err(|e| {
                                log::error!(
                                    "Failed to create flow node(id='{}'): {:?}",
                                    node_config.id,
                                    e
                                );
                                e
                            })?;

                    // Redirect all the output node wires in the subflow to the output port of the subflow.
                    if let Some(ref mut subflow_state) = self.subflow_state {
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

            let arc_node: Arc<dyn FlowNodeBehavior> = Arc::from(node);
            state.nodes_ordering.push(arc_node.id());
            state.nodes.insert(node_config.id, arc_node.clone());

            log::debug!("------ {} has been loaded!", arc_node);

            self.register_internal_node(arc_node.clone(), state, node_config)?;
        }
        Ok(self)
    }

    fn register_internal_node(
        &self,
        node: Arc<dyn FlowNodeBehavior>,
        state: &mut FlowState,
        node_config: &RedFlowNodeConfig,
    ) -> crate::Result<()> {
        match node.get_node().type_ {
            "complete" => self.register_complete_node(node, state, node_config)?,
            "catch" => {
                state.catch_nodes.insert(node_config.id, node.clone());
            }
            // ignore normal nodes
            &_ => {}
        }
        Ok(())
    }

    fn register_complete_node(
        &self,
        node: &dyn FlowNodeBehavior,
        state: &mut FlowState,
        node_config: &RedFlowNodeConfig,
    ) -> crate::Result<()> {
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
            state.complete_nodes.insert(node_config.id, node.clone());
            Ok(())
        } else {
            Err(EdgelinkError::BadFlowsJson(format!(
                "CompleteNode has no 'scope' property: {}",
                node
            ))
            .into())
        }
    }

    fn new_flow_node_state(
        &self,
        meta_node: &MetaNode,
        node_config: &RedFlowNodeConfig,
    ) -> crate::Result<FlowNode> {
        let mut ports = Vec::new();
        let (tx_root, rx) = tokio::sync::mpsc::channel(NODE_MSG_CHANNEL_CAPACITY);
        // Convert the Node-RED wires elements to ours
        for red_port in node_config.wires.iter() {
            let mut wires = Vec::new();
            for nid in red_port.node_ids.iter() {
                let node_entry = self.nodes.get(nid).ok_or(EdgelinkError::InvalidData(format!(
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
            Some(gid) => match self.groups.get(gid) {
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
            type_: meta_node.type_,
            disabled: node_config.disabled,
            flow: Weak::new(),
            msg_tx: tx_root,
            msg_rx: MsgReceiverHolder::new(rx),
            ports,
            group: group_ref,
            on_received: MsgEventSender::new(1),
            on_completed: MsgEventSender::new(1),
            on_error: MsgEventSender::new(1),
        })
    }
    /*
    pub fn register(mut self, meta_node: &'static MetaNode) -> Self {
        self.meta_nodes.insert(meta_node.type_, meta_node);
        self
    }

    pub fn with_builtins(mut self) -> Self {
        for meta in META_NODES.iter() {
            log::debug!("Found builtin Node: '{}'", meta.type_);
            self.meta_nodes.insert(meta.type_, meta);
        }
        self
    }

    pub fn build(self) -> crate::Result<Arc<dyn Registry>> {
        if self.meta_nodes.is_empty() {
            log::warn!("There are no meta node in the Registry!");
        }

        let result = Arc::new(RegistryImpl {
            meta_nodes: Arc::new(self.meta_nodes),
        });
        Ok(result)
    }
    */
}
