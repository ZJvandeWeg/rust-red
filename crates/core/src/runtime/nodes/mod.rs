use std::fmt;
use std::sync::{Arc, Weak};

use async_trait::async_trait;
use runtime::engine::Engine;
use runtime::group::{Group, WeakGroup};
use tokio::select;
use tokio_util::sync::CancellationToken;

use super::context::Context;
use crate::runtime::env::*;
use crate::runtime::flow::*;
use crate::runtime::model::json::{RedFlowNodeConfig, RedGlobalNodeConfig};
use crate::runtime::model::*;
use crate::EdgelinkError;
use crate::*;

pub(crate) mod common_nodes;
mod function_nodes;

#[cfg(feature = "net")]
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

type GlobalNodeFactoryFn = fn(&Engine, &RedGlobalNodeConfig) -> crate::Result<Box<dyn GlobalNodeBehavior>>;

type FlowNodeFactoryFn = fn(&Flow, FlowNode, &RedFlowNodeConfig) -> crate::Result<Box<dyn FlowNodeBehavior>>;

#[derive(Debug, Clone, Copy)]
pub enum NodeFactory {
    Global(GlobalNodeFactoryFn),
    Flow(FlowNodeFactoryFn),
}

#[derive(Debug)]
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
    pub type_str: &'static str,
    pub ordering: usize,
    pub disabled: bool,
    pub active: bool,
    pub flow: WeakFlow,
    pub msg_tx: MsgSender,
    pub msg_rx: MsgReceiverHolder,
    pub ports: Vec<Port>,
    pub group: Option<WeakGroup>,
    pub envs: Arc<EnvStore>,
    pub context: Arc<Context>,

    pub on_received: MsgEventSender,
    pub on_completed: MsgEventSender,
    pub on_error: MsgEventSender,
}

#[derive(Debug)]
pub struct GlobalNode {
    pub id: ElementId,
    pub name: String,
    pub type_str: &'static str,
    pub ordering: usize,
    pub context: Arc<Context>,
    pub disabled: bool,
}

#[async_trait]
pub trait GlobalNodeBehavior: Send + Sync + FlowsElement {
    fn get_node(&self) -> &GlobalNode;
}

#[async_trait]
pub trait FlowNodeBehavior: Send + Sync + FlowsElement {
    fn get_node(&self) -> &FlowNode;

    async fn run(self: Arc<Self>, stop_token: CancellationToken);

    fn group(&self) -> Option<Group> {
        self.get_node().group.clone().and_then(|x| x.upgrade())
    }

    fn get_flow(&self) -> &WeakFlow {
        &self.get_node().flow
    }

    fn get_envs(&self) -> Arc<EnvStore> {
        self.get_node().envs.clone()
    }

    fn get_env(&self, key: &str) -> Option<Variant> {
        self.get_node().envs.evalute_env(key)
    }

    fn get_context(&self) -> Arc<Context> {
        self.get_node().context.clone()
    }

    fn get_engine(&self) -> Option<Engine> {
        let flow = self.get_node().flow.upgrade()?;
        flow.engine()
    }

    async fn inject_msg(&self, msg: MsgHandle, cancel: CancellationToken) -> crate::Result<()> {
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

    async fn recv_msg(&self, stop_token: CancellationToken) -> crate::Result<MsgHandle> {
        let msg = self.get_node().msg_rx.recv_msg(stop_token).await?;
        if self.get_node().on_received.receiver_count() > 0 {
            self.get_node().on_received.send(msg.clone())?;
        }
        Ok(msg)
    }

    async fn notify_uow_completed(&self, msg: MsgHandle, cancel: CancellationToken) {
        let (node_id, flow) = { (self.id(), self.get_node().flow.upgrade()) };
        if let Some(flow) = flow {
            flow.notify_node_uow_completed(&node_id, msg, cancel).await;
        } else {
            todo!();
        }
    }

    async fn fan_out_one(&self, envelope: Envelope, cancel: CancellationToken) -> crate::Result<()> {
        if self.get_node().ports.is_empty() {
            log::warn!("No output wires in this node: Node(id='{}', name='{}')", self.id(), self.name());
            return Ok(());
        }
        if envelope.port >= self.get_node().ports.len() {
            return Err(crate::EdgelinkError::BadArgument("envelope"))
                .with_context(|| format!("Invalid port index {}", envelope.port));
        }

        let port = &self.get_node().ports[envelope.port];

        let mut msg_sent = false;
        for wire in port.wires.iter() {
            let msg_to_send = if msg_sent { envelope.msg.deep_clone(true).await } else { envelope.msg.clone() };

            wire.tx(msg_to_send, cancel.clone()).await?;
            msg_sent = true;
        }
        Ok(())
    }

    async fn report_error(&self, log_message: String, msg: MsgHandle, cancel: CancellationToken) {
        let handled = if let Some(flow) = self.get_flow().upgrade() {
            let node = self.as_any().downcast_ref::<Arc<dyn FlowNodeBehavior>>().unwrap(); // FIXME
            flow.handle_error(node.as_ref(), &log_message, Some(&msg), None, cancel).await.unwrap_or(false)
        } else {
            false
        };
        if !handled {
            log::error!("[{}:{}] {}", self.type_str(), self.name(), log_message);
        }
    }

    // events
    fn on_loaded(&self) {}
    async fn on_starting(&self) {}
}

impl dyn GlobalNodeBehavior {
    pub fn type_id(&self) -> ::std::any::TypeId {
        self.as_any().type_id()
    }
}

impl fmt::Debug for dyn GlobalNodeBehavior {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!(
            "GlobalNode(id='{}', type='{}', name='{}')",
            self.id(),
            self.get_node().type_str,
            self.name(),
        ))
    }
}

impl fmt::Display for dyn GlobalNodeBehavior {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!(
            "GlobalNode(id='{}', type='{}', name='{}')",
            self.id(),
            self.get_node().type_str,
            self.name(),
        ))
    }
}

impl dyn FlowNodeBehavior {
    pub fn type_id(&self) -> ::std::any::TypeId {
        self.as_any().type_id()
    }

    async fn fan_out_many(
        &self,
        envelopes: impl IntoIterator<Item = Envelope>,
        cancel: CancellationToken,
    ) -> crate::Result<()> {
        if self.get_node().ports.is_empty() {
            log::warn!("No output wires in this node: Node(id='{}')", self.id());
            return Ok(());
        }

        for e in envelopes.into_iter() {
            self.fan_out_one(e, cancel.child_token()).await?;
        }
        Ok(())
    }
}

impl fmt::Debug for dyn FlowNodeBehavior {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!("FlowNode(id='{}', type='{}', name='{}')", self.id(), self.type_str(), self.name(),))
    }
}

impl fmt::Display for dyn FlowNodeBehavior {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!("FlowNode(id='{}', type='{}', name='{}')", self.id(), self.type_str(), self.name(),))
    }
}

pub async fn with_uow<'a, B, F, T>(node: &'a B, cancel: CancellationToken, proc: F)
where
    B: FlowNodeBehavior,
    F: FnOnce(&'a B, MsgHandle) -> T,
    T: std::future::Future<Output = crate::Result<()>>,
{
    match node.recv_msg(cancel.clone()).await {
        Ok(msg) => {
            if let Err(ref err) = proc(node, msg.clone()).await {
                let flow = node.get_flow().upgrade().expect("flow");
                let error_message = err.to_string();
                match flow.handle_error(node, &error_message, Some(&msg), None, cancel.clone()).await {
                    Ok(_) => (),
                    Err(e) => {
                        log::error!("Failed to handle error: {:?}", e)
                    }
                }
            }
            // Report the completion
            node.notify_uow_completed(msg, cancel.clone()).await;
        }
        Err(ref err) => {
            if let Some(EdgelinkError::TaskCancelled) = err.downcast_ref::<EdgelinkError>() {
                return;
            }
            log::warn!(
                "with_uow() Error: Node(id='{}', name='{}', type='{}')\n{:#?}",
                node.id(),
                node.name(),
                node.type_str(),
                err
            );
        }
    }
}

#[async_trait]
pub trait LinkCallNodeBehavior: Send + Sync + FlowNodeBehavior {
    /// Receive the returning message
    async fn return_msg(
        &self,
        msg: MsgHandle,
        stack_id: ElementId,
        return_from_node_id: ElementId,
        return_from_flow_id: ElementId,
        cancel: CancellationToken,
    ) -> crate::Result<()>;
}
