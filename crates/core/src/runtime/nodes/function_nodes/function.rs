use std::sync::Arc;

use serde::Deserialize;
use smallvec::SmallVec;

mod js {
    pub use rquickjs::*;
}
use js::CatchResultExt;
use js::FromJs;
use js::IntoJs;

use crate::runtime::flow::Flow;
use crate::runtime::model::*;
use crate::runtime::nodes::*;
use crate::runtime::registry::*;
use edgelink_macro::*;

const OUTPUT_MSGS_CAP: usize = 4;

pub type OutputMsgs = smallvec::SmallVec<[(usize, Msg); OUTPUT_MSGS_CAP]>;

#[derive(Deserialize, Debug)]
struct FunctionNodeConfig {
    func: String,

    #[serde(default)]
    initialize: String,

    #[serde(default)]
    finalize: String,

    #[serde(default)]
    outputs: usize,
}

#[derive(Debug)]
#[flow_node("function")]
struct FunctionNode {
    base: FlowNode,
    config: FunctionNodeConfig,
}

const JS_PRELUDE_SCRIPT: &str = include_str!("./function.prelude.js");

#[async_trait]
impl FlowNodeBehavior for FunctionNode {
    fn get_node(&self) -> &FlowNode {
        &self.base
    }

    async fn run(self: Arc<Self>, stop_token: CancellationToken) {
        let js_rt = js::AsyncRuntime::new().unwrap();
        let js_ctx = js::AsyncContext::full(&js_rt).await.unwrap();
        let mut resolver = js::loader::BuiltinResolver::default();
        resolver.add_module("console");
        let loaders = (js::loader::ScriptLoader::default(), js::loader::ModuleLoader::default());
        js_rt.set_loader(resolver, loaders).await;

        let _ = self.init_async(&js_ctx).await;

        while !stop_token.is_cancelled() {
            let sub_ctx = &js_ctx;
            let cancel = stop_token.child_token();
            with_uow(self.as_ref(), cancel.child_token(), |node, msg| async move {
                let res = {
                    let msg_guard = msg.write().await;
                    node.filter_msg(msg_guard.clone(), sub_ctx).await // This gonna eat the msg and produce a new one
                };
                match res {
                    Ok(changed_msgs) => {
                        // Pack the new messages
                        let envelopes = changed_msgs
                            .into_iter()
                            .map(|x| Envelope { port: x.0, msg: Arc::new(RwLock::new(x.1)) })
                            .collect::<SmallVec<[Envelope; OUTPUT_MSGS_CAP]>>();

                        node.fan_out_many(&envelopes, cancel.child_token()).await?;
                    }
                    Err(e) => {
                        return Err(e);
                    }
                };
                Ok(())
            })
            .await;
        }

        //let _ = js_ctx.eval(js::Source::from_bytes(&self1.config.initialize));
        let _ = self.finalize_async(&js_ctx).await;

        log::debug!("DebugNode process() task has been terminated.");
    }
}

impl FunctionNode {
    fn build(
        _flow: &Flow,
        base_node: FlowNode,
        _config: &RedFlowNodeConfig,
    ) -> crate::Result<Box<dyn FlowNodeBehavior>> {
        let mut function_config = FunctionNodeConfig::deserialize(&_config.json)?;
        if function_config.outputs == 0 {
            function_config.outputs = 1;
        }

        let node = FunctionNode { base: base_node, config: function_config };
        Ok(Box::new(node))
    }

    async fn filter_msg(&self, msg: Msg, js_ctx: &js::AsyncContext) -> crate::Result<OutputMsgs> {
        let eval_result: js::Result<OutputMsgs> = js::async_with!(js_ctx => |ctx| {
            let user_func : js::Function = ctx.globals().get("__el_user_func")?;
            let js_msg = msg.into_js(&ctx).unwrap(); // FIXME
            let args =(js::Value::new_null(ctx.clone()), js_msg);
            let js_res_value: js::Result<js::Value> = user_func.call(args);
            match js_res_value.catch(&ctx) {
                Ok(js_result) => self.convert_return_value(&ctx , js_result),
                Err(e) => {
                    log::error!("Javascript user function exception: {:?}", e);
                    Err(js::Error::Exception)
                }
            }
        })
        .await;

        match eval_result {
            Ok(msgs) => Ok(msgs),
            Err(e) => {
                log::warn!("Failed to invoke user func: {}", e);
                Err(EdgelinkError::InvalidData(e.to_string()).into())
            }
        }
    }

    fn convert_return_value<'js>(&self, ctx: &js::Ctx<'js>, js_result: js::Value<'js>) -> js::Result<OutputMsgs> {
        let mut items = OutputMsgs::new();
        match js_result.type_of() {
            js::Type::Object => {
                // Returns single Msg
                let item = (0, Msg::from_js(ctx, js_result)?);
                items.push(item);
            }
            js::Type::Array => {
                // Returns an array of Msgs
                for (port, ele) in js_result.as_array().unwrap().iter::<js::Value>().enumerate() {
                    match ele {
                        Ok(ele) => {
                            if ele.is_object() {
                                items.push((port, Msg::from_js(ctx, ele)?));
                            } else if let Some(subarr) = ele.as_array() {
                                for subele in subarr.iter() {
                                    match subele {
                                        Ok(obj) => {
                                            items.push((port, Msg::from_js(ctx, obj)?));
                                        }
                                        Err(ref e) => {
                                            log::warn!("Bad array item: \n{:#?}", e);
                                        }
                                    }
                                }
                            } else {
                                log::warn!("Bad array item: \n{:#?}", ele);
                            }
                        }
                        Err(ref e) => {
                            log::warn!("Bad array item: \n{:#?}", e);
                        }
                    }
                }
            }
            _ => {
                log::warn!("Wrong type of the return values: Javascript type={}", js_result.type_of());
            }
        }
        Ok(items)
    }

    async fn init_async(&self, js_ctx: &js::AsyncContext) -> crate::Result<()> {
        let user_func = &self.config.func;
        let user_script = format!(
            r#"
function __el_user_func(context, msg) {{
    {user_func}
}}
"#
        );
        let user_script_ref = &user_script;

        js::async_with!(js_ctx => |ctx| {

            // crate::runtime::red::js::red::register_red_object(&ctx).unwrap();

            ctx.globals().set("console", crate::runtime::js::console::Console::new())?;


            match ctx.eval::<(), _>(JS_PRELUDE_SCRIPT) {
                Err(e) => {
                    log::error!("Failed to evaluate the prelude script: {}", e);
                    panic!();
                }
                _ =>{
                    log::info!("The evulation of the prelude script has been succeed.");
                }
            }

            if !self.config.initialize.is_empty() {
                match ctx.eval::<(),_>(self.config.initialize.as_bytes()) {
                    Ok(()) => (),
                    Err(e) => {
                        log::error!("Failed to evaluate the initialization script code: {}", e);
                        return Err(EdgelinkError::InvalidData(e.to_string()).into())
                    }
                }
            }

            match ctx.eval::<(),_>(user_script_ref.as_bytes()) {
                Ok(()) => Ok(()),
                Err(e) => {
                    log::error!("Failed to evaluate the user function definition code: {}", e);
                    return Err(EdgelinkError::InvalidData(e.to_string()).into())
                }
            }
        })
        .await
    }

    async fn finalize_async(&self, js_ctx: &js::AsyncContext) -> crate::Result<()> {
        js::async_with!(js_ctx => |ctx| {
            if !self.config.finalize.is_empty() {
                match ctx.eval::<(),_>(self.config.finalize.as_bytes()) {
                    Ok(()) => Ok(()),
                    Err(e) => {
                        log::error!("Failed to evaluate the finalization script code: {}", e);
                        Err(EdgelinkError::InvalidData(e.to_string()).into())
                    }
                }
            }
            else {
                Ok(())
            }
        })
        .await
    }
}
