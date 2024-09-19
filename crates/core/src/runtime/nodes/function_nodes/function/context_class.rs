use std::sync::Arc;

use rquickjs::{class::Trace, Ctx, Function, IntoJs, Value};
use rquickjs::{prelude::*, Exception};

use crate::runtime::context::{Context as RedContext, ContextKey};

use super::Variant;

#[derive(Clone, Trace)]
#[rquickjs::class(frozen)]
pub(super) struct ContextClass {
    #[qjs(skip_trace)]
    pub red_ctx: Arc<RedContext>,
}

#[allow(non_snake_case)]
#[rquickjs::methods]
impl ContextClass {
    #[qjs(skip)]
    pub fn new(red_ctx: Arc<RedContext>) -> Self {
        ContextClass { red_ctx }
    }

    #[qjs(rename = "set")]
    fn set<'js>(
        self,
        keys: Value<'js>,
        values: Value<'js>,
        store: Opt<rquickjs::String<'js>>,
        cb: Opt<Function<'js>>,
        ctx: Ctx<'js>,
    ) -> rquickjs::Result<()> {
        let keys: String = keys.get()?;
        let values: Variant = values.get()?;

        let async_ctx = ctx.clone();
        if let Some(cb) = cb.0 {
            // User provides the callback, we do it in async
            ctx.spawn(async move {
                let store = store.0.and_then(|x| x.get::<String>().ok());
                let ctx_key = ContextKey { store: store.as_deref(), key: keys.as_ref() };
                match self.red_ctx.set_one(&ctx_key, Some(values)).await {
                    Ok(()) => {
                        let args = (Value::new_undefined(async_ctx.clone()),);
                        cb.call::<_, ()>(args).unwrap();
                    }
                    Err(_) => {
                        let args =
                            (Exception::from_message(async_ctx.clone(), "Failed to parse key").into_js(&async_ctx),);
                        cb.call::<_, ()>(args).unwrap();
                    }
                }
            });
        } else {
            // No callback, we do it in sync
            let store = store.0.and_then(|x| x.get::<String>().ok());
            let ctx_key = ContextKey { store: store.as_deref(), key: keys.as_ref() };
            self.red_ctx
                .set_one_sync(&ctx_key, Some(values))
                .map_err(|e| ctx.throw(format!("{}", e).into_js(&ctx).unwrap()))?;
        }
        Ok(())
    }
}
