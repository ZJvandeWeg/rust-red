use std::sync::{Arc, Weak};

use rquickjs::{class::Trace, Ctx, IntoJs, Value};

use super::FunctionNode;

#[derive(Clone, Trace)]
#[rquickjs::class(frozen)]
pub(super) struct NodeClass<'js> {
    #[qjs(skip_trace)]
    ctx: Ctx<'js>,

    #[qjs(skip_trace)]
    node: Weak<FunctionNode>,
}

#[rquickjs::methods]
impl<'js> NodeClass<'js> {
    // All functions declared in this impl block will be defined on the prototype of the
    // class. This attributes allows you to skip certain functions.
    #[qjs(skip)]
    pub fn new(ctx: Ctx<'js>, node: &Arc<FunctionNode>) -> Self {
        NodeClass { ctx, node: Arc::downgrade(node) }
    }

    #[qjs(get, rename = "id")]
    pub fn get_id(&self) -> rquickjs::Result<Value<'js>> {
        let node = self.node.upgrade().clone().ok_or(rquickjs::Error::UnrelatedRuntime)?;
        node.base.id.to_string().into_js(&self.ctx) // TODO FIXME
    }

    #[qjs(get, rename = "name")]
    pub fn get_name(&self) -> rquickjs::Result<Value<'js>> {
        let node = self.node.upgrade().clone().ok_or(rquickjs::Error::UnrelatedRuntime)?;
        node.base.name.clone().into_js(&self.ctx)
    }

    #[qjs(get, rename = "outputCount")]
    pub fn get_output_count(&self) -> rquickjs::Result<usize> {
        let node = self.node.upgrade().clone().ok_or(rquickjs::Error::UnrelatedRuntime)?;
        Ok(node.config.output_count)
    }
}
