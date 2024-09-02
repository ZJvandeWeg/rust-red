use async_trait::async_trait;
use std::any::Any;
use std::fmt;
use std::sync::{Arc, Weak};
use tokio::select;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;

use crate::red::json::{RedFlowNodeConfig, RedGlobalNodeConfig};
use crate::runtime::engine::FlowEngine;
use crate::runtime::flow::*;
use crate::runtime::model::*;
use crate::EdgelinkError;

use super::group::Group;
use super::model::{ElementId, Envelope, Msg, MsgReceiverHolder};

mod common_nodes;
mod function_nodes;
mod network_nodes;

pub const NODE_MSG_CHANNEL_CAPACITY: usize = 16;

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
    fn(&Flow, FlowNode, &RedFlowNodeConfig) -> crate::Result<Arc<dyn FlowNodeBehavior>>;

#[derive(Debug, Clone, Copy)]
pub enum NodeFactory {
    Global(GlobalNodeFactoryFn),
    Flow(FlowNodeFactoryFn),
}

#[derive(Debug, Clone, Copy)]
pub struct MetaNode {
    /// The tag of the element
    pub kind: NodeKind,
    pub type_: &'static str,
    pub factory: NodeFactory,
}

#[derive(Debug)]
pub struct FlowNode {
    pub id: ElementId,
    pub name: String,
    pub type_: String,
    pub disabled: bool,
    pub flow: Weak<Flow>,
    pub msg_tx: MsgSender,
    pub msg_rx: MsgReceiverHolder,
    pub ports: Vec<Port>,
    pub group: Weak<Group>,

    pub on_received: MsgEventSender,
    pub on_completed: MsgEventSender,
    pub on_error: MsgEventSender,
}

impl FlowNode {}

#[async_trait]
pub trait GlobalNodeBehavior: Any + Send + Sync {
    fn id(&self) -> &ElementId;
    fn name(&self) -> &str;
}

#[async_trait]
pub trait FlowNodeBehavior: Any + Send + Sync {
    fn get_node(&self) -> &FlowNode;

    fn id(&self) -> ElementId {
        self.get_node().id
    }

    fn name(&self) -> &str {
        &self.get_node().name
    }

    fn type_name(&self) -> &str {
        &self.get_node().type_
    }

    fn group(&self) -> &Weak<Group> {
        &self.get_node().group
    }

    fn get_engine(&self) -> Option<Arc<FlowEngine>> {
        let flow = self.get_node().flow.upgrade()?;
        flow.engine.upgrade()
    }

    async fn run(self: Arc<Self>, stop_token: CancellationToken);

    async fn inject_msg(
        &self,
        msg: Arc<RwLock<Msg>>,
        cancel: CancellationToken,
    ) -> crate::Result<()> {
        select! {
            result = self.get_node().msg_tx.send(msg) => {
                result.map_err(|e| e.into())
            }

            _ = cancel.cancelled() => {
                // The token was cancelled
                Err(EdgelinkError::TaskCancelled.into())
            }
        }
    }

    async fn recv_msg(&self, stop_token: CancellationToken) -> crate::Result<Arc<RwLock<Msg>>> {
        let msg = self.get_node().msg_rx.recv_msg(stop_token).await?;
        if self.get_node().on_received.receiver_count() > 0 {
            self.get_node().on_received.send(msg.clone())?;
        }
        Ok(msg)
    }

    async fn notify_uow_completed(&self, msg: &Msg, cancel: CancellationToken) {
        let (node_id, flow) = { (self.id(), self.get_node().flow.upgrade()) };
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
        if self.get_node().ports.is_empty() {
            log::warn!(
                "No output wires in this node: Node(id='{}', name='{}')",
                self.id(),
                self.name()
            );
            return Ok(());
        }
        if envelope.port >= self.get_node().ports.len() {
            return Err(crate::EdgelinkError::BadArguments(format!(
                "Invalid port index {}",
                envelope.port
            ))
            .into());
        }

        let port = &self.get_node().ports[envelope.port];

        let mut msg_sent = false;
        for wire in port.wires.iter() {
            let msg_to_send = if msg_sent {
                // other msg
                let to_clone = envelope.msg.read().await;
                Arc::new(RwLock::new(to_clone.clone()))
            } else {
                envelope.msg.clone() // First time
            };

            wire.tx(msg_to_send, cancel.clone()).await?;
            msg_sent = true;
        }
        Ok(())
    }

    async fn fan_out_many(
        &self,
        envelopes: &[Envelope],
        cancel: CancellationToken,
    ) -> crate::Result<()> {
        if self.get_node().ports.is_empty() {
            log::warn!("No output wires in this node: Node(id='{}')", self.id());
            return Ok(());
        }

        for e in envelopes.iter() {
            self.fan_out_one(e, cancel.child_token()).await?;
        }
        Ok(())
    }
}

impl fmt::Debug for dyn FlowNodeBehavior {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!(
            "FlowNode(id='{}', type='{}', name='{}')",
            self.id(),
            self.type_name(),
            self.name(),
        ))
    }
}

impl fmt::Display for dyn FlowNodeBehavior {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!(
            "FlowNode(id='{}', type='{}', name='{}')",
            self.id(),
            self.type_name(),
            self.name(),
        ))
    }
}

async fn with_uow<'a, B, F, T>(node: &'a B, cancel: CancellationToken, proc: F)
where
    B: FlowNodeBehavior,
    F: FnOnce(&'a B, Arc<RwLock<Msg>>) -> T,
    T: std::future::Future<Output = crate::Result<()>>,
{
    match node.recv_msg(cancel.child_token()).await {
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
            if let Some(EdgelinkError::TaskCancelled) = err.downcast_ref::<EdgelinkError>() {
                return;
            }
            log::warn!(
                "with_uow() Error: Node(id='{}', name='{}', type='{}')\n{:#?}",
                node.id(),
                node.name(),
                node.type_name(),
                err
            );
        }
    }
}

#[macro_export]
macro_rules! define_builtin_flow_node {
    ($type_:literal, $factory:expr) => {
        inventory::submit! {
            BuiltinNodeDescriptor {
                meta: MetaNode {
                    kind: NodeKind::Flow,
                    type_: $type_,
                    factory: NodeFactory::Flow($factory),
                },
            }
        }
    };
}

#[macro_export]
macro_rules! define_builtin_global_node {
    ($type_:literal, $factory:expr) => {
        inventory::submit! {
            BuiltinNodeDescriptor {
                meta: MetaNode {
                    kind: NodeKind::Global,
                    type_: $type_,
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
