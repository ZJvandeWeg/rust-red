use std::sync::Arc;

use rquickjs::context::EvalOptions;
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
use edgelink_macro::*;

mod context_class;
mod edgelink_class;
mod env_class;
mod node_class;

const OUTPUT_MSGS_CAP: usize = 4;

pub type OutputMsgs = smallvec::SmallVec<[(usize, Msg); OUTPUT_MSGS_CAP]>;

#[derive(Deserialize, Debug)]
struct FunctionNodeConfig {
    func: String,

    #[serde(default)]
    initialize: String,

    #[serde(default)]
    finalize: String,

    #[serde(default, rename = "outputs")]
    output_count: usize,
}

#[derive(Debug)]
#[flow_node("function")]
struct FunctionNode {
    base: FlowNode,
    config: FunctionNodeConfig,
}

const JS_PRELUDE_SCRIPT: &str = include_str!("./function.prelude.js");
static JS_RUMTIME: ::tokio::sync::OnceCell<js::AsyncRuntime> = ::tokio::sync::OnceCell::const_new();

#[async_trait]
impl FlowNodeBehavior for FunctionNode {
    fn get_node(&self) -> &FlowNode {
        &self.base
    }

    async fn run(self: Arc<Self>, stop_token: CancellationToken) {
        let js_rt = JS_RUMTIME
            .get_or_init(|| async move {
                log::debug!("[FUNCTION_NODE] Initializing JavaScript AsyncRuntime...");
                let rt = js::AsyncRuntime::new().unwrap();
                let mut resolver = js::loader::BuiltinResolver::default();

                resolver.add_module("console");

                let loaders = (js::loader::ScriptLoader::default(), js::loader::ModuleLoader::default());
                rt.set_loader(resolver, loaders).await;
                rt
            })
            .await;

        let js_ctx = js::AsyncContext::full(js_rt).await.unwrap();

        if let Err(e) = self.init_async(&js_ctx).await {
            // It's a fatal error
            log::error!("[FUNCTION NODE] Fatal error! Failed to initialize JavaScript environment: {}", e);

            stop_token.cancel();
            stop_token.cancelled().await;
        }
        JS_RUMTIME.get().unwrap().idle().await;

        while !stop_token.is_cancelled() {
            let sub_ctx = &js_ctx;
            let cancel = stop_token.child_token();
            let this_node = self.clone();
            with_uow(this_node.as_ref(), cancel.child_token(), |node, msg| async move {
                let res = {
                    let msg_guard = msg.write().await;
                    node.filter_msg(msg_guard.clone(), sub_ctx).await // This gonna eat the msg and produce a new one
                };
                match res {
                    Ok(changed_msgs) => {
                        // Pack the new messages
                        if !changed_msgs.is_empty() {
                            let envelopes = changed_msgs
                                .into_iter()
                                .map(|x| Envelope { port: x.0, msg: Arc::new(RwLock::new(x.1)) })
                                .collect::<SmallVec<[Envelope; OUTPUT_MSGS_CAP]>>();

                            node.fan_out_many(&envelopes, cancel.child_token()).await?;
                        }
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
        JS_RUMTIME.get().unwrap().idle().await;

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
        if function_config.output_count == 0 {
            function_config.output_count = 1;
        }

        let node = FunctionNode { base: base_node, config: function_config };
        Ok(Box::new(node))
    }

    async fn filter_msg(&self, msg: Msg, js_ctx: &js::AsyncContext) -> crate::Result<OutputMsgs> {
        let origin_msg_id = msg.id();
        let eval_result: js::Result<OutputMsgs> = js::async_with!(js_ctx => |ctx| {
            let user_func : js::Function = ctx.globals().get("__el_user_func")?;
            let js_msg = msg.into_js(&ctx).unwrap(); // FIXME
            let args = (js_msg,);
            let promised = user_func.call::<_, rquickjs::Promise>(args)?;
            let js_res_value: js::Result<js::Value> = promised.into_future().await;
            match js_res_value.catch(&ctx) {
                Ok(js_result) => self.convert_return_value(&ctx , js_result, origin_msg_id),
                Err(e) => {
                    log::error!("Javascript user function exception: {:?}", e);
                    Err(js::Error::Exception)
                }
            }
        })
        .await;

        // This is VERY IMPORTANT! Execute all spawned tasks.
        JS_RUMTIME.get().unwrap().idle().await;

        match eval_result {
            Ok(msgs) => Ok(msgs),
            Err(e) => {
                log::warn!("Failed to invoke user func: {}", e);
                Err(EdgelinkError::InvalidData(e.to_string()).into())
            }
        }
    }

    fn convert_return_value<'js>(
        &self,
        ctx: &js::Ctx<'js>,
        js_result: js::Value<'js>,
        origin_msg_id: Option<ElementId>,
    ) -> js::Result<OutputMsgs> {
        let mut items = OutputMsgs::new();
        match js_result.type_of() {
            // Returns an array of Msgs
            js::Type::Array => {
                for (port, ele) in js_result.as_array().unwrap().iter::<js::Value>().enumerate() {
                    match ele {
                        Ok(ele) => {
                            if let Some(subarr) = ele.as_array() {
                                for subele in subarr.iter() {
                                    let obj: js::Value = subele.unwrap();
                                    if obj.is_null() {
                                        continue;
                                    }
                                    let mut msg = Msg::from_js(ctx, obj)?;
                                    if let Some(org_id) = origin_msg_id {
                                        msg.set_id(org_id);
                                    }
                                    items.push((port, msg));
                                }
                            } else if ele.is_object() && !ele.is_null() {
                                let mut msg = Msg::from_js(ctx, ele)?;
                                if let Some(org_id) = origin_msg_id {
                                    msg.set_id(org_id);
                                }
                                items.push((port, msg));
                            } else if ele.is_null() {
                                continue;
                            } else {
                                log::warn!("Bad msg array item: \n{:#?}", ele);
                            }
                        }
                        Err(ref e) => {
                            log::warn!("Bad msg array item: \n{:#?}", e);
                        }
                    }
                }
            }

            // Returns single Msg
            js::Type::Object => {
                let item = (0, Msg::from_js(ctx, js_result)?);
                items.push(item);
            }

            js::Type::Null => {
                log::debug!("[FUNCTION_NODE] Skip `null`");
            }

            js::Type::Undefined => {
                log::debug!("[FUNCTION_NODE] No returned msg(s).");
            }

            _ => {
                log::warn!("Wrong type of the return values: Javascript type={}", js_result.type_of());
            }
        }
        Ok(items)
    }

    async fn init_async(self: &Arc<Self>, js_ctx: &js::AsyncContext) -> crate::Result<()> {
        let user_func = &self.config.func;
        let user_script = format!(
            r#"
            async function __el_user_func(msg) {{ 
                var global = __edgelinkGlobalContext; 
                var flow = __edgelinkFlowContext; 
                var context = __edgelinkNodeContext; 
                var __msgid__ = msg._msgid; 
                {user_func} 
            }}"#
        );
        let user_script_ref = &user_script;

        log::debug!("[FUNCTION_NODE] Initializing JavaScript context...");
        js::async_with!(js_ctx => |ctx| {

            // crate::runtime::red::js::red::register_red_object(&ctx).unwrap();

            ctx.globals().set("console", crate::runtime::js::console::Console::new())?;
            ctx.globals().set("__edgelink", edgelink_class::EdgelinkClass::default())?;
            ctx.globals().set("env", env_class::EnvClass::new(self.get_envs().clone()))?;
            ctx.globals().set("node", node_class::NodeClass::new(self))?;

            // Register the global-scoped context
            if let Some(global_context) = self.get_engine().map(|x| x.context()) {
                ctx.globals().set("__edgelinkGlobalContext", context_class::ContextClass::new(global_context))?;
            }
            else {
                return Err(EdgelinkError::InvalidData("Failed to get global context".into()).into());
            }

            // Register the flow-scoped context
            if let Some(flow_context) = self.get_flow().upgrade().map(|x| x.context()) {
                ctx.globals().set("__edgelinkFlowContext", context_class::ContextClass::new(flow_context))?;
            }
            else {
                return Err(EdgelinkError::InvalidData("Failed to get flow context".into()).into());
            }

            // Register the node-scoped context
            ctx.globals().set("__edgelinkNodeContext", context_class::ContextClass::new(self.context()))?;

            let mut eval_options = EvalOptions::default();
            eval_options.promise = true;
            eval_options.strict = true;
            match ctx.eval_with_options::<(), _>(JS_PRELUDE_SCRIPT, eval_options) {
                Err(e) => {
                    log::error!("Failed to evaluate the prelude script: {}", e);
                    return Err(EdgelinkError::InvalidData(e.to_string()).into());
                }
                _ =>{
                    log::debug!("[FUNCTION_NODE] The evulation of the prelude script has been succeed.");
                }
            }

            if !self.config.initialize.trim_ascii().is_empty() {
                let init_body = &self.config.initialize;
                let init_script = format!(
                    r#"
                    async function __el_init_func() {{ 
                        var global = __edgelinkGlobalContext; 
                        var flow = __edgelinkFlowContext; 
                        var context = __edgelinkNodeContext; 
                        {init_body} 
                    }}"#
                );
                let mut eval_options = EvalOptions::default();
                eval_options.promise = true;
                eval_options.strict = true;
                match ctx.eval_with_options::<(), _>(init_script.as_bytes(), eval_options) {
                    Err(e) => {
                        log::error!("Failed to evaluate the `initialize` script: {:?}", e);
                        return Err(EdgelinkError::InvalidData(e.to_string()).into());
                    }
                    _ =>{
                        log::debug!("[FUNCTION_NODE] The evulation of the `initialize` script has been succeed.");
                    }
                }

                let init_func : js::Function = ctx.globals().get("__el_init_func")?;
                let promised = init_func.call::<_, rquickjs::Promise>(())?;
                match promised.into_future().await {
                    Ok(()) => (),
                    Err(e) => {
                        log::error!("Failed to invoke the initialization script code: {}", e);
                        return Err(EdgelinkError::InvalidData(e.to_string()).into());
                    }
                }
            }

            let mut eval_options = EvalOptions::default();
            eval_options.promise = true;
            eval_options.strict = true;
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
