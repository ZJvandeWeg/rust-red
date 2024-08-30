use async_trait::async_trait;
use std::any::Any;
use std::fmt;
use std::sync::{Arc, Weak};
use tokio::select;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;

use crate::runtime::engine::FlowEngine;
use crate::runtime::flow::Flow;
use crate::runtime::model::*;
use crate::runtime::red::json::{RedFlowNodeConfig, RedGlobalNodeConfig};
use crate::EdgeLinkError;

use super::group::Group;
use super::model::{ElementId, Envelope, Msg, MsgReceiverHolder};

mod common_nodes;
mod function_nodes;
mod network_nodes;

#[derive(Debug, Clone, Copy)]
pub enum NodeState {
    Starting = 0,
    Idle,
    Busy,
    Stopping,
    Stopped,
}

#[derive(Debug, Clone, Copy)]
pub enum NodeKind {
    Flow = 0,
    Global = 1,
}

impl fmt::Display for NodeKind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            NodeKind::Flow => write!(f, "GlobalNode"),
            NodeKind::Global => write!(f, "FlowoNode"),
        }
    }
}

type GlobalNodeFactoryFn =
    fn(Arc<FlowEngine>, &RedGlobalNodeConfig) -> crate::Result<Arc<dyn GlobalNodeBehavior>>;

type FlowNodeFactoryFn =
    fn(&Flow, FlowNodeState, &RedFlowNodeConfig) -> crate::Result<Arc<dyn FlowNodeBehavior>>;

#[derive(Debug, Clone, Copy)]
pub enum NodeFactory {
    Global(GlobalNodeFactoryFn),
    Flow(FlowNodeFactoryFn),
}

#[derive(Debug, Clone, Copy)]
pub struct MetaNode {
    /// The tag of the element
    pub kind: NodeKind,
    pub type_name: &'static str,
    pub factory: NodeFactory,
}

#[derive(Debug)]
pub struct FlowNodeState {
    pub id: ElementId,
    pub name: String,
    pub type_name: String,
    pub disabled: bool,
    pub flow: Weak<Flow>,
    pub msg_tx: MsgSender,
    pub msg_rx: MsgReceiverHolder,
    pub ports: Vec<Port>,
    pub group: Weak<Group>,
}

impl FlowNodeState {
    //
}

#[async_trait]
pub trait GlobalNodeBehavior: Any + Send + Sync {
    fn id(&self) -> &ElementId;
    fn name(&self) -> &str;
}

#[async_trait]
pub trait FlowNodeBehavior: Any + Send + Sync {
    fn state(&self) -> &FlowNodeState;

    fn id(&self) -> &ElementId {
        &self.state().id
    }

    fn name(&self) -> &str {
        &self.state().name
    }

    fn group(&self) -> &Weak<Group> {
        &self.state().group
    }

    async fn run(self: Arc<Self>, stop_token: CancellationToken);

    async fn inject_msg(
        &self,
        msg: Arc<RwLock<Msg>>,
        cancel: CancellationToken,
    ) -> crate::Result<()> {
        select! {
            result = self.state().msg_tx.send(msg) => {
                result.map_err(|e| e.into())
            }

            _ = cancel.cancelled() => {
                // The token was cancelled
                Err(EdgeLinkError::TaskCancelled.into())
            }
        }
    }

    async fn wait_for_msg(&self, stop_token: CancellationToken) -> crate::Result<Arc<RwLock<Msg>>> {
        self.state().msg_rx.wait_for_msg(stop_token).await
    }

    async fn notify_uow_completed(&self, msg: &Msg, cancel: CancellationToken) {
        let (node_id, flow) = { (*self.id(), self.state().flow.upgrade()) };
        if let Some(flow) = flow {
            flow.notify_node_uow_completed(&node_id, msg, cancel).await;
        } else {
            todo!();
        }
    }

    async fn fan_out_one(
        &self,
        envelope: &Envelope,
        cancel: CancellationToken,
    ) -> crate::Result<()> {
        if self.state().ports.is_empty() {
            log::warn!("No output wires in this node: Node(id='{}')", self.id());
            return Ok(());
        }
        if envelope.port >= self.state().ports.len() {
            return Err(crate::EdgeLinkError::BadArguments(format!(
                "Invalid port index {}",
                envelope.port
            ))
            .into());
        }

        let port = &self.state().ports[envelope.port];

        let mut msg_sent = false;
        for wire in port.wires.iter() {
            let msg_to_send = if msg_sent {
                // other msg
                let to_clone = envelope.msg.read().await;
                Arc::new(RwLock::new(to_clone.clone()))
            } else {
                envelope.msg.clone() // First time
            };

            wire.tx(msg_to_send, cancel.child_token()).await?;
            msg_sent = true;
        }
        Ok(())
    }

    async fn fan_out_many(
        &self,
        envelopes: &[Envelope],
        cancel: CancellationToken,
    ) -> crate::Result<()> {
        if self.state().ports.is_empty() {
            log::warn!("No output wires in this node: Node(id='{}')", self.id());
            return Ok(());
        }

        for e in envelopes.iter() {
            self.fan_out_one(e, cancel.child_token()).await?;
        }
        Ok(())
    }
}

async fn with_uow<'a, B, F, T>(node: &'a B, cancel: CancellationToken, proc: F)
where
    B: FlowNodeBehavior,
    F: FnOnce(&'a B, Arc<RwLock<Msg>>) -> T,
    T: std::future::Future<Output = crate::Result<()>>,
{
    match node.wait_for_msg(cancel.child_token()).await {
        Ok(msg) => {
            if let Err(ref err) = proc(node, msg.clone()).await {
                // TODO report error
                log::warn!("Failed to commit uow job: {}", err.to_string())
            }
            // Report the completion
            {
                let msg_guard = msg.read().await;
                node.notify_uow_completed(&msg_guard, cancel.child_token())
                    .await;
            }
        }
        Err(ref err) => {
            if let Some(EdgeLinkError::TaskCancelled) = err.downcast_ref::<EdgeLinkError>() {
                return;
            }
            log::warn!("Error: {:#?}", err);
        }
    }
}

#[macro_export]
macro_rules! define_builtin_flow_node {
    ($type_name:literal, $factory:expr) => {
        inventory::submit! {
            BuiltinNodeDescriptor {
                meta: MetaNode {
                    kind: NodeKind::Flow,
                    type_name: $type_name,
                    factory: NodeFactory::Flow($factory),
                },
            }
        }
    };
}

#[macro_export]
macro_rules! define_builtin_global_node {
    ($type_name:literal, $factory:expr) => {
        inventory::submit! {
            BuiltinNodeDescriptor {
                meta: MetaNode {
                    kind: NodeKind::Global,
                    type_name: $type_name,
                    factory: NodeFactory::Global($factory),
                },
            }
        }
    };
}

pub(crate) struct BuiltinNodeDescriptor {
    pub(crate) meta: MetaNode,
}

impl BuiltinNodeDescriptor {}

inventory::collect!(BuiltinNodeDescriptor);
