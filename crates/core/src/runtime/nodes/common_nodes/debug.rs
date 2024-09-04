use serde;
use serde::Deserialize;
use std::sync::Arc;

use crate::red::json::RedFlowNodeConfig;
use crate::runtime::flow::Flow;
use crate::runtime::nodes::*;
use crate::runtime::registry::*;
use edgelink_macro::*;

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
#[flow_node("debug")]
struct DebugNode {
    base: FlowNode,
    config: DebugNodeConfig,
}

impl DebugNode {
    fn create(
        _flow: &Flow,
        state: FlowNode,
        _config: &RedFlowNodeConfig,
    ) -> crate::Result<Arc<dyn FlowNodeBehavior>> {
        let mut debug_config: DebugNodeConfig = DebugNodeConfig::deserialize(&_config.json)?;
        if debug_config.complete.is_empty() {
            debug_config.complete = "payload".to_string();
        }

        let node = DebugNode {
            base: state,
            config: debug_config,
        };
        Ok(Arc::new(node))
    }
}

#[async_trait]
impl FlowNodeBehavior for DebugNode {
    fn get_node(&self) -> &FlowNode {
        &self.base
    }

    async fn run(self: Arc<Self>, stop_token: CancellationToken) {
        while !stop_token.is_cancelled() {
            if self.config.active {
                match self.recv_msg(stop_token.child_token()).await {
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
