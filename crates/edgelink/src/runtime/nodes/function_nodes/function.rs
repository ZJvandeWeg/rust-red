use rquickjs as js;
use serde::Deserialize;
use std::sync::Arc;

use log;

use crate::define_builtin_flow_node;
use crate::runtime::flow::Flow;
use crate::runtime::model::*;
use crate::runtime::nodes::*;

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

struct FunctionNode {
    state: FlowNodeState,
    config: FunctionNodeConfig,
}

const JS_PRELUDE_SCRIPT: &str = include_str!("./function.prelude.js");

#[async_trait]
impl FlowNodeBehavior for FunctionNode {
    fn state(&self) -> &FlowNodeState {
        &self.state
    }

    async fn run(self: Arc<Self>, stop_token: CancellationToken) {
        let js_rt = js::AsyncRuntime::new().unwrap();
        let js_ctx = js::AsyncContext::full(&js_rt).await.unwrap();
        let resolver = (
            js::loader::BuiltinResolver::default(), // .with_module(path)
                                                    // FileResolver::default().with_path(app_path),
        );
        let loader = js::loader::ScriptLoader::default();
        js_rt.set_loader(resolver, loader).await;

        let _ = self.init_async(&js_ctx).await;

        while !stop_token.is_cancelled() {
            let node = self.clone();
            let sub_ctx = &js_ctx;
            let cancel = stop_token.child_token();
            with_uow(self.as_ref(), cancel.child_token(), |msg| async move {
                let res = {
                    let mut msg_guard = msg.write().await;
                    node.clone().filter_msg(&mut msg_guard, sub_ctx).await
                };
                match res {
                    Ok(changed_msgs) => {
                        let envelopes = changed_msgs
                            .into_iter()
                            .map(|x| Envelope {
                                port: x.0,
                                msg: Arc::new(RwLock::new(x.1)),
                            })
                            .collect::<Vec<Envelope>>();

                        node.clone()
                            .fan_out_many(&envelopes, cancel.child_token())
                            .await?;
                    }
                    Err(e) => {
                        log::warn!("Failed to filter message: {}", e);
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
    fn create(
        _flow: Arc<Flow>,
        base_node: FlowNodeState,
        _config: &RedFlowNodeConfig,
    ) -> crate::Result<Arc<dyn FlowNodeBehavior>> {
        let mut function_config: FunctionNodeConfig =
            serde_json::from_value(serde_json::Value::Object(_config.json.clone()))?;
        if function_config.outputs == 0 {
            function_config.outputs = 1;
        }

        let node = FunctionNode {
            state: base_node,

            config: function_config,
        };
        Ok(Arc::new(node))
    }

    async fn filter_msg(
        &self,
        msg: &mut Msg,
        js_ctx: &js::AsyncContext,
    ) -> crate::Result<Vec<(usize, Msg)>> {
        let origin_msg = &msg;
        let eval_result: js::Result<Vec<(usize, Msg)>> = js::async_with!(js_ctx => |ctx| {
            let user_func : js::Function = ctx.globals().get("__el_user_func")?;
            let js_msg = origin_msg.as_js_object(&ctx).unwrap(); // FIXME
            let args =(js::Value::new_null(ctx.clone()), js_msg);
            let js_result: js::Value = user_func.call(args)?;

            let items = self.convert_return_value(&js_result);
            Ok(items)
        })
        .await;
        match eval_result {
            Ok(msgs) => Ok(msgs),
            Err(e) => {
                log::warn!("Failed to invoke user func: {}", e);
                Err(EdgeLinkError::InvalidData(e.to_string()).into())
            }
        }
    }

    fn convert_return_value(&self, js_result: &js::Value) -> Vec<(usize, Msg)> {
        let mut items = Vec::new();
        if let Some(obj) = js_result.as_object() {
            // Returns single Msg
            let item = (0, Msg::from(obj));
            items.push(item);
        } else if let Some(arr) = js_result.as_array() {
            // Returns an array of Msgs
            for (port, ele) in arr.iter::<js::Value>().enumerate() {
                match ele {
                    Ok(ref ele) => {
                        if let Some(obj) = ele.as_object() {
                            items.push((port, Msg::from(obj)));
                        } else if let Some(subarr) = ele.as_array() {
                            for subele in subarr.iter::<js::Object>() {
                                match subele {
                                    Ok(ref obj) => {
                                        items.push((port, Msg::from(obj)));
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
        } else {
            log::warn!("Wrong type of the return values");
        }
        items
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

            crate::runtime::red::js::red::register_red_object(&ctx).unwrap();

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
                        return Err(EdgeLinkError::InvalidData(e.to_string()).into())
                    }
                }
            }

            match ctx.eval::<(),_>(user_script_ref.as_bytes()) {
                Ok(()) => Ok(()),
                Err(e) => {
                    log::error!("Failed to evaluate the user function definition code: {}", e);
                    return Err(EdgeLinkError::InvalidData(e.to_string()).into())
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
                        Err(EdgeLinkError::InvalidData(e.to_string()).into())
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

define_builtin_flow_node!("function", FunctionNode::create);
