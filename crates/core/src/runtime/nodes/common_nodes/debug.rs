use serde;
use serde::Deserialize;
use std::sync::Arc;

use crate::define_builtin_flow_node;
use crate::runtime::flow::Flow;
use crate::runtime::nodes::*;
use crate::runtime::red::json::RedFlowNodeConfig;

#[derive(Deserialize, Debug)]
struct DebugNodeConfig {
    #[serde(default)]
    active: bool,

    //#[serde(default)]
    //console: bool,
    //#[serde(default)]
    //target_type: String,
    #[serde(default)]
    complete: String,
}

#[derive(Debug)]
struct DebugNode {
    state: FlowNodeState,
    config: DebugNodeConfig,
}

impl DebugNode {
    fn create(
        _flow: Arc<Flow>,
        state: FlowNodeState,
        _config: &RedFlowNodeConfig,
    ) -> crate::Result<Arc<dyn FlowNodeBehavior>> {
        let mut debug_config: DebugNodeConfig =
            serde_json::from_value(serde_json::Value::Object(_config.json.clone()))?;
        if debug_config.complete.is_empty() {
            debug_config.complete = "payload".to_string();
        }

        let node = DebugNode {
            state,
            config: debug_config,
        };
        Ok(Arc::new(node))
    }
}

#[async_trait]
impl FlowNodeBehavior for DebugNode {
    fn state(&self) -> &FlowNodeState {
        &self.state
    }

    async fn run(self: Arc<Self>, stop_token: CancellationToken) {
        while !stop_token.is_cancelled() {
            if self.config.active {
                match self.wait_for_msg(stop_token.child_token()).await {
                    Ok(msg) => {
                        log::info!(
                            "Message Received [Node: {}] ：\n{:#?}",
                            self.name(),
                            msg.as_ref()
                        )
                    }
                    Err(ref err) => {
                        log::error!("Error: {:#?}", err);
                        break;
                    }
                }
            } else {
                stop_token.cancelled().await;
            }
        }
    }
}

define_builtin_flow_node!("debug", DebugNode::create);
