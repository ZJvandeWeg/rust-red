use std::any::Any;
use std::sync::{Arc, Weak};
use tokio::sync::{Mutex, RwLock};
use tokio_util::sync::CancellationToken;

use tokio;
use tokio::sync::mpsc;

use crate::runtime::nodes::FlowNodeBehavior;
use crate::EdgeLinkError;

mod msg;

pub use msg::*;
pub mod propex;
mod variant;

pub use variant::Variant;
pub use variant::VariantError;

use super::flow::Flow;

pub type ElementId = u64;

#[derive(Debug)]
pub struct PortWire {
    // pub target_node_id: ElementId,
    // pub target_node: Weak<dyn FlowNodeBehavior>,
    pub msg_sender: tokio::sync::mpsc::Sender<Arc<RwLock<Msg>>>,
}

impl PortWire {
    pub async fn tx(&self, msg: Arc<RwLock<Msg>>, cancel: CancellationToken) -> crate::Result<()> {
        tokio::select! {

            send_result = self.msg_sender.send(msg) =>  send_result.map_err(|e|
                crate::EdgeLinkError::InvalidOperation(format!("Failed to transmit message: {}", e)).into()),

            _ = cancel.cancelled() =>
                Err(crate::EdgeLinkError::TaskCancelled.into()),
        }
    }
}

#[derive(Debug)]
pub struct Port {
    pub wires: Vec<PortWire>,
}

pub type MsgSender = mpsc::Sender<Arc<RwLock<Msg>>>;
pub type MsgReceiver = mpsc::Receiver<Arc<RwLock<Msg>>>;

#[derive(Debug)]
pub struct MsgReceiverHolder {
    pub rx: Mutex<MsgReceiver>,
}

impl MsgReceiverHolder {
    pub fn new(rx: MsgReceiver) -> Self {
        MsgReceiverHolder { rx: Mutex::new(rx) }
    }

    pub async fn wait_for_msg_forever(&self) -> crate::Result<Arc<RwLock<Msg>>> {
        let rx = &mut self.rx.lock().await;
        match rx.recv().await {
            Some(msg) => Ok(msg),
            None => {
                log::error!("Failed to receive message");
                Err(EdgeLinkError::TaskCancelled.into())
            }
        }
    }

    pub async fn wait_for_msg(
        &self,
        stop_token: CancellationToken,
    ) -> crate::Result<Arc<RwLock<Msg>>> {
        tokio::select! {
            result = self.wait_for_msg_forever() => {
                result
            }

            _ = stop_token.cancelled() => {
                // The token was cancelled
                Err(EdgeLinkError::TaskCancelled.into())
            }
        }
    }
}

pub trait GraphElement {
    fn parent(&self) -> Option<Weak<Self>>
    where
        Self: Sized;
    fn parent_ref(&self) -> Option<Weak<dyn GraphElement>>;
}

pub trait SettingHolder {
    fn get_setting<'a>(
        name: &'a str,
        node: Option<&'a dyn FlowNodeBehavior>,
        flow: Option<&'a Flow>,
    ) -> &'a Variant;
}

pub trait RuntimeElement: Any {
    fn as_any(&self) -> &dyn Any;
}

impl<T: RuntimeElement + Any> RuntimeElement for T {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

pub fn query_trait<T: RuntimeElement, U: 'static>(ele: &T) -> Option<&U> {
    ele.as_any().downcast_ref::<U>()
}
