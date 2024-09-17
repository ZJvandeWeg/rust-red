use std::sync::{Arc, Weak};

use rquickjs::{class::Trace, prelude::Opt, Ctx, FromJs, IntoJs, Value};
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;

use super::{EdgelinkError, Envelope, FlowNodeBehavior, FunctionNode, Msg};

#[derive(Clone, Trace)]
#[rquickjs::class(frozen)]
pub(super) struct NodeClass {
    #[qjs(skip_trace)]
    node: Weak<FunctionNode>,
}

#[rquickjs::methods]
impl NodeClass {
    // All functions declared in this impl block will be defined on the prototype of the
    // class. This attributes allows you to skip certain functions.
    #[qjs(skip)]
    pub fn new(node: &Arc<FunctionNode>) -> Self {
        NodeClass { node: Arc::downgrade(node) }
    }

    #[qjs(get, rename = "id")]
    fn get_id(&self) -> rquickjs::Result<String> {
        let node = self.node.upgrade().clone().ok_or(rquickjs::Error::UnrelatedRuntime)?;
        Ok(node.base.id.to_string())
    }

    #[qjs(get, rename = "name")]
    fn get_name<'js>(&self, ctx: Ctx<'js>) -> rquickjs::Result<Value<'js>> {
        let node = self.node.upgrade().clone().ok_or(rquickjs::Error::UnrelatedRuntime)?;
        node.base.name.clone().into_js(&ctx)
    }

    #[qjs(get, rename = "outputCount")]
    fn get_output_count(&self) -> rquickjs::Result<usize> {
        let node = self.node.upgrade().clone().ok_or(rquickjs::Error::UnrelatedRuntime)?;
        Ok(node.config.output_count)
    }

    #[qjs(rename = "send")]
    fn send<'js>(self, msgs: Value<'js>, _cloning: Opt<bool>, ctx: Ctx<'js>) -> rquickjs::Result<()> {
        let _cloning = _cloning.unwrap_or(false);
        let async_ctx = ctx.clone();
        ctx.spawn(async move {
            if let Err(err) = self._send_msgs(async_ctx, msgs, _cloning).await {
                // TODO report error
                log::warn!("Failed to send msg(s): {}", err);
            }
        });
        Ok(())
    }

    #[qjs(skip)]
    async fn _send_msgs<'js>(&self, ctx: Ctx<'js>, msgs: rquickjs::Value<'js>, _cloning: bool) -> crate::Result<()> {
        match msgs.type_of() {
            rquickjs::Type::Array => {
                /* TODO
                if let Some(msgs) = msgs.as_array() {
                    let msgs_to_send = Vec::with_capacity(msgs.len());
                    for msg_ele in msgs.iter() {
                        let msg = msg_ele?;
                        let msg_to_send = Arc::new(RwLock::new(Msg::from_js(&ctx, msg)?));
                        msgs_to_send.push(msg_to_send);
                        let node = self.node.upgrade().clone().ok_or(rquickjs::Error::UnrelatedRuntime)?
                            as Arc<dyn FlowNodeBehavior>;
                        let envelope = Envelope { port: 0, msg: msg_to_send };
                        let cancel = CancellationToken::new();
                        match node.fan_out_one(&envelope, cancel).await {
                            Ok(_) => {}
                            Err(err) => log::error!("Failed to send msg in function node: {}", err),
                        }
                    }
                } else {
                    unreachable!();
                }
                */
            }

            rquickjs::Type::Object => {
                let msg_to_send = Arc::new(RwLock::new(Msg::from_js(&ctx, msgs)?));
                let node =
                    self.node.upgrade().clone().ok_or(rquickjs::Error::UnrelatedRuntime)? as Arc<dyn FlowNodeBehavior>;
                let envelope = Envelope { port: 0, msg: msg_to_send };
                // FIXME
                let cancel = CancellationToken::new();
                match node.fan_out_one(&envelope, cancel).await {
                    Ok(_) => {}
                    Err(err) => log::error!("Failed to send msg in function node: {}", err),
                }
            }

            _ => {
                return Err(EdgelinkError::InvalidOperation(format!("Unsupported: {:?}", msgs.type_of())).into());
            }
        }
        Ok(())
    }
}
