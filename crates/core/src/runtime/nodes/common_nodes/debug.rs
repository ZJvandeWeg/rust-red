use serde;
use serde::Deserialize;
use std::sync::Arc;

use crate::runtime::flow::Flow;
use crate::runtime::model::json::RedFlowNodeConfig;
use crate::runtime::nodes::*;
use edgelink_macro::*;

#[derive(Deserialize, Debug)]
struct DebugNodeConfig {
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
    _config: DebugNodeConfig,
}

impl DebugNode {
    fn build(_flow: &Flow, state: FlowNode, config: &RedFlowNodeConfig) -> crate::Result<Box<dyn FlowNodeBehavior>> {
        let mut debug_config: DebugNodeConfig = DebugNodeConfig::deserialize(&config.rest)?;
        if debug_config.complete.is_empty() {
            debug_config.complete = "payload".to_string();
        }

        let node = DebugNode { base: state, _config: debug_config };
        Ok(Box::new(node))
    }
}

#[async_trait]
impl FlowNodeBehavior for DebugNode {
    fn get_node(&self) -> &FlowNode {
        &self.base
    }

    async fn run(self: Arc<Self>, stop_token: CancellationToken) {
        while !stop_token.is_cancelled() {
            if self.base.active {
                match self.recv_msg(stop_token.child_token()).await {
                    Ok(msg) => {
                        log::info!("[debug:{}] Message Received: \n{:#?}", self.name(), msg.as_ref())
                    }
                    Err(ref err) => {
                        log::error!("[debug:{}] Error: {:#?}", self.name(), err);
                        break;
                    }
                }
            } else {
                stop_token.cancelled().await;
            }
        }
    }
}
