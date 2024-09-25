use std::sync::Arc;

use rquickjs::context::EvalOptions;
use serde::Deserialize;
use smallvec::SmallVec;

mod js {
    pub use rquickjs::prelude::*;
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

type OutputMsgs = smallvec::SmallVec<[(usize, Msg); OUTPUT_MSGS_CAP]>;

#[derive(Deserialize, Debug)]
struct FunctionNodeConfig {
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
    user_func: Vec<u8>,
}

const JS_PRELUDE_SCRIPT: &str = include_str!("./function.prelude.js");
// static JS_RUMTIME: ::tokio::sync::OnceCell<js::AsyncRuntime> = ::tokio::sync::OnceCell::const_new();

#[async_trait]
impl FlowNodeBehavior for FunctionNode {
    fn get_node(&self) -> &FlowNode {
        &self.base
    }

    async fn run(self: Arc<Self>, stop_token: CancellationToken) {
        // This is a workaround; ideally, all function nodes should share a runtime. However,
        // for some reason, if the runtime of rquickjs is used as a global variable,
        // the members of node and env will disappear upon the second load.

        //let js_rt = JS_RUMTIME
        //.get_or_init(|| async move {
        log::debug!("[FUNCTION_NODE] Initializing JavaScript AsyncRuntime...");
        let rt = js::AsyncRuntime::new().unwrap();
        let resolver = js::loader::BuiltinResolver::default();
        //resolver.add_module("console");
        let loaders = (js::loader::ScriptLoader::default(), js::loader::ModuleLoader::default());
        rt.set_loader(resolver, loaders).await;
        rt.idle().await;
        let js_rt = rt;
        //})
        //.await;

        let js_ctx = js::AsyncContext::full(&js_rt).await.unwrap();

        if let Err(e) = self.init_async(&js_ctx).await {
            // It's a fatal error
            log::error!("[FUNCTION NODE] Fatal error! Failed to initialize JavaScript environment: {:?}", e);

            stop_token.cancel();
            stop_token.cancelled().await;
        }
        js_rt.idle().await;

        while !stop_token.is_cancelled() {
            let sub_ctx = &js_ctx;
            let cancel = stop_token.child_token();
            let this_node = self.clone();
            with_uow(this_node.clone().as_ref(), cancel.child_token(), |_, msg| async move {
                let res = {
                    let msg_guard = msg.write().await;
                    // This gonna eat the msg and produce a new one
                    this_node.filter_msg(msg_guard.clone(), sub_ctx).await
                };
                match res {
                    Ok(changed_msgs) => {
                        // Pack the new messages
                        if !changed_msgs.is_empty() {
                            let envelopes = changed_msgs
                                .into_iter()
                                .map(|x| Envelope { port: x.0, msg: Arc::new(RwLock::new(x.1)) })
                                .collect::<SmallVec<[Envelope; OUTPUT_MSGS_CAP]>>();

                            this_node.fan_out_many(&envelopes, cancel.clone()).await?;
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
        js_ctx.runtime().idle().await;

        log::debug!("DebugNode process() task has been terminated.");
    }
}

impl FunctionNode {
    fn build(
        _flow: &Flow,
        base_node: FlowNode,
        _config: &RedFlowNodeConfig,
    ) -> crate::Result<Box<dyn FlowNodeBehavior>> {
        let mut function_config = FunctionNodeConfig::deserialize(&_config.rest)?;
        if function_config.output_count == 0 {
            function_config.output_count = 1;
        }

        let user_script_bytes = if let Some(user_script) = _config.rest.get("func").and_then(|x| x.as_str()) {
            let user_script = format!(
                r#"
                async function __el_user_func(msg) {{ 
                    var global = __edgelinkGlobalContext; 
                    var flow = __edgelinkFlowContext; 
                    var context = __edgelinkNodeContext; 
                    var __msgid__ = msg._msgid; 
                    context.flow = flow;
                    context.global = global;

                    {} 

                }}"#,
                user_script
            );
            user_script.as_bytes().to_vec()
        } else {
            return Err(EdgelinkError::BadFlowsJson(
                "The `func` property in function node cannot be null or empty".to_string(),
            )
            .into());
        };

        let node = FunctionNode { base: base_node, config: function_config, user_func: user_script_bytes };
        Ok(Box::new(node))
    }

    async fn filter_msg(self: &Arc<Self>, msg: Msg, js_ctx: &js::AsyncContext) -> crate::Result<OutputMsgs> {
        let origin_msg_id = msg.id();
        let eval_result: js::Result<OutputMsgs> = js::async_with!(js_ctx => |ctx| {
            self.prepare_js_ctx(&ctx).map_err(|_| js::Error::Exception)?;

            match ctx.eval_with_options::<(),_>(self.user_func.as_slice(), self.make_eval_options()).catch(&ctx) {
                Ok(()) => (),
                Err(e) => {
                    log::error!("Failed to evaluate the user function definition code: {:?}", e);
                    return Err(js::Error::Exception)
                }
            }

            let user_func : js::Function = ctx.globals().get("__el_user_func")?;
            let js_msg = msg.into_js(&ctx)?;
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
        js_ctx.runtime().idle().await;

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
        log::debug!("[FUNCTION_NODE] Initializing JavaScript context...");
        js::async_with!(js_ctx => |ctx| {
            self.prepare_js_ctx(&ctx)?;

            if !self.config.initialize.trim_ascii().is_empty() {
                let init_body = &self.config.initialize;
                let init_script = format!(
                    "
                    async function __el_init_func() {{ 
                        var global = __edgelinkGlobalContext; 
                        var flow = __edgelinkFlowContext; 
                        var context = __edgelinkNodeContext; 
                        context.flow = flow;
                        context.global = global;
                        \n{init_body}\n
                    }}
                    "
                );
                match ctx.eval_with_options::<(), _>(init_script.as_bytes(), self.make_eval_options()) {
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
            while ctx.execute_pending_job() {};

            Ok(())
        })
        .await
    }

    async fn finalize_async(self: &Arc<Self>, js_ctx: &js::AsyncContext) -> crate::Result<()> {
        let final_body = &self.config.finalize;
        let final_script = format!(
            "
            async function __el_finalize_func() {{ 
                var global = __edgelinkGlobalContext; 
                var flow = __edgelinkFlowContext; 
                var context = __edgelinkNodeContext; 
                context.flow = flow;
                context.global = global;
                \n{final_body}\n
            }}
            "
        );
        js::async_with!(js_ctx => |ctx| {
            self.prepare_js_ctx(&ctx)?;

            match ctx.eval_with_options::<(), _>(final_script.as_bytes(), self.make_eval_options()) {
                Err(e) => {
                    log::error!("Failed to evaluate the `finialize` script: {:?}", e);
                    return Err(EdgelinkError::InvalidData(e.to_string()).into());
                }
                _ =>{
                    log::debug!("[FUNCTION_NODE] The evulation of the `finalize` script has been succeed.");
                }
            }

            let final_func : js::Function = ctx.globals().get("__el_finalize_func")?;
            let promised = final_func.call::<_, rquickjs::Promise>(())?;
            match promised.into_future().await {
                Ok(()) => Ok(()),
                Err(e) => {
                    log::error!("Failed to invoke the `finialize` script code: {}", e);
                    return Err(EdgelinkError::InvalidData(e.to_string()).into());
                }
            }
        })
        .await
    }

    fn prepare_js_ctx(self: &Arc<Self>, ctx: &js::Ctx<'_>) -> crate::Result<()> {
        // crate::runtime::red::js::red::register_red_object(&ctx).unwrap();
        // js::Class::<node_class::NodeClass>::register(&ctx)?;
        // js::Class::<env_class::EnvClass>::register(&ctx)?;
        // js::Class::<edgelink_class::EdgelinkClass>::register(&ctx)?;

        ::rquickjs_extra::console::init(ctx)?;
        ctx.globals().set("__edgelink", edgelink_class::EdgelinkClass::default())?;

        /*
        {
            ::llrt_modules::timers::init_timers(&ctx)?;
            let (_module, module_eval) = js::Module::evaluate_def::<llrt_modules::timers::TimersModule, _>(ctx.clone(), "timers")?;
            module_eval.into_future().await?;
        }
        */
        ::rquickjs_extra::timers::init(ctx)?;

        ctx.globals().set("env", env_class::EnvClass::new(self.get_envs().clone()))?;
        ctx.globals().set("node", node_class::NodeClass::new(self))?;

        // Register the global-scoped context
        if let Some(global_context) = self.get_engine().map(|x| x.context()) {
            ctx.globals().set("__edgelinkGlobalContext", context_class::ContextClass::new(global_context))?;
        } else {
            return Err(EdgelinkError::InvalidOperation("Failed to get global context".into()))
                .with_context(|| "The engine cannot be released!");
        }

        // Register the flow-scoped context
        if let Some(flow_context) = self.get_flow().upgrade().map(|x| x.context()) {
            ctx.globals().set("__edgelinkFlowContext", context_class::ContextClass::new(flow_context))?;
        } else {
            return Err(EdgelinkError::InvalidOperation("Failed to get flow context".into()).into());
        }

        // Register the node-scoped context
        ctx.globals().set("__edgelinkNodeContext", context_class::ContextClass::new(self.context()))?;

        let mut eval_options = EvalOptions::default();
        eval_options.promise = true;
        eval_options.strict = true;
        if let Err(e) = ctx.eval_with_options::<(), _>(JS_PRELUDE_SCRIPT, eval_options).catch(ctx) {
            return Err(EdgelinkError::InvalidData(e.to_string()))
                .with_context(|| format!("Failed to evaluate the prelude script: {:?}", e));
        }
        Ok(())
    }

    fn make_eval_options(&self) -> EvalOptions {
        let mut eval_options = EvalOptions::default();
        eval_options.promise = false;
        eval_options.strict = false;
        eval_options
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_it_should_set_node_context_with_stress() {
        let flows_json = json!([
            {"id": "100", "type": "tab"},
            {"id": "1", "type": "function", "z": "100", "wires": [
                ["2"]], "func": "context.set('count','0'); msg.count=context.get('count'); node.send(msg);"},
            {"id": "2", "z": "100", "type": "test-once"},
        ]);
        let msgs_to_inject_json = json!([
            ["1", {"payload": "foo", "topic": "bar"}],
        ]);

        for i in 0..5 {
            let engine = crate::runtime::engine::build_test_engine(flows_json.clone()).unwrap();
            eprintln!("ROUND {}", i);
            let msgs_to_inject = Vec::<(ElementId, Msg)>::deserialize(msgs_to_inject_json.clone()).unwrap();
            let msgs =
                engine.run_once_with_inject(1, std::time::Duration::from_secs_f64(0.2), msgs_to_inject).await.unwrap();

            assert_eq!(msgs.len(), 1);
            let msg = &msgs[0];
            assert_eq!(msg["payload"], "foo".into());
            assert_eq!(msg["topic"], "bar".into());
            assert_eq!(msg["count"], "0".into());
        }
    }
}
