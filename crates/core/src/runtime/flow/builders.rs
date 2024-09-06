use serde::Deserialize;
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

use super::{FlowArgs, FlowKind, FlowState, SubflowState};
use crate::red::eval;
use crate::red::json::{RedEnvEntry, RedPropertyType};

#[derive(Debug)]
pub struct FlowBuilder {
    pub id: ElementId,
    pub parent: Option<Weak<Self>>,
    pub label: String,
    pub disabled: bool,
    pub args: FlowArgs,

    pub engine: Weak<FlowEngine>,

    pub stop_token: CancellationToken,

    state: FlowState,
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
        let res = Self {
            id: flow_config.id,
            parent: None, //TODO FIXME
            engine: Arc::downgrade(&engine),
            label: flow_config.label.clone(),
            disabled: flow_config.disabled,
            args: FlowArgs::load(options)?,
            state: FlowState {
                groups: HashMap::new(),
                nodes: HashMap::new(),
                complete_nodes: HashMap::new(),
                complete_nodes_map: HashMap::new(),
                catch_nodes: HashMap::new(),
                nodes_ordering: Vec::new(),
                _context: Variant::new_empty_object(),
                node_tasks: JoinSet::new(),
            },

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
            // groups: HashMap::new(), //   flow_config.groups.iter().map(|g| Group::new_flow_group(config, flow))
        };
        Ok(res)
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
