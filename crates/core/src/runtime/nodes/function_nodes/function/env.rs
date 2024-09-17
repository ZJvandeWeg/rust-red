use std::sync::Arc;

use rquickjs::{class::Trace, Ctx, IntoJs, Result, Value};

use crate::runtime::env::*;

#[derive(Clone, Trace)]
#[rquickjs::class(frozen)]
pub(crate) struct EnvClass<'js> {
    #[qjs(skip_trace)]
    pub ctx: Ctx<'js>,

    #[qjs(skip_trace)]
    pub env_store: Arc<EnvStore>,
}

#[rquickjs::methods]
impl<'js> EnvClass<'js> {
    // All functions declared in this impl block will be defined on the prototype of the
    // class. This attributes allows you to skip certain functions.
    #[qjs(skip)]
    pub fn new(ctx: Ctx<'js>, env_store: Arc<EnvStore>) -> Self {
        EnvClass { ctx, env_store }
    }

    #[qjs()]
    fn get(&self, key: Value<'js>) -> Result<Value<'js>> {
        let key: String = key.get()?;
        let res: Value<'js> = match self.env_store.get_env(key.as_ref()) {
            Some(var) => var.into_js(&self.ctx)?,
            _ => Value::new_undefined(self.ctx.clone()),
        };
        Ok(res)
    }
}
