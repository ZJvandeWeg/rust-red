use std::sync::Arc;

use rquickjs::{class::Trace, Ctx, IntoJs, Result, Value};

use crate::runtime::env::*;

#[derive(Clone, Trace)]
#[rquickjs::class(frozen)]
pub(super) struct EnvClass {
    #[qjs(skip_trace)]
    pub env_store: Arc<EnvStore>,
}

#[allow(non_snake_case)]
#[rquickjs::methods]
impl<'js> EnvClass {
    #[qjs(skip)]
    pub fn new(env_store: Arc<EnvStore>) -> Self {
        EnvClass { env_store }
    }

    #[qjs()]
    fn get(&self, key: Value<'js>, ctx: Ctx<'js>) -> Result<Value<'js>> {
        let key: String = key.get()?;
        let res: Value<'js> = match self.env_store.evalute_env(key.as_ref()) {
            Some(var) => var.into_js(&ctx)?,
            _ => Value::new_undefined(ctx),
        };
        Ok(res)
    }
}
