use std::sync::Arc;

use serde::Deserialize;

use crate::define_builtin_flow_node;
use crate::runtime::flow::Flow;
use crate::runtime::nodes::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
enum LinkType {
    #[serde(rename = "dynamic")]
    Dynamic,

    #[serde(rename = "static")]
    Static,
}

#[derive(Deserialize, Debug)]
struct LinkCallNodeConfig {
    mode: LinkType,

    #[serde(deserialize_with = "crate::red::json::deser::str_to_option_f64")]
    timeout: Option<f64>,
}

#[derive(Debug)]
struct LinkCallNode {
    state: FlowNodeState,

    config: LinkCallNodeConfig,
}

impl LinkCallNode {
    fn create(
        _flow: &Flow,
        state: FlowNodeState,
        config: &RedFlowNodeConfig,
    ) -> crate::Result<Arc<dyn FlowNodeBehavior>> {
        let link_call_config = LinkCallNodeConfig::deserialize(&config.json)?;
        let node = LinkCallNode {
            state,
            config: link_call_config,
        };
        Ok(Arc::new(node))
    }
}

#[async_trait]
impl FlowNodeBehavior for LinkCallNode {
    fn state(&self) -> &FlowNodeState {
        &self.state
    }

    async fn run(self: Arc<Self>, stop_token: CancellationToken) {
        while !stop_token.is_cancelled() {
            let cancel = stop_token.clone();
            with_uow(
                self.as_ref(),
                cancel.child_token(),
                |node, msg| async move {
                    node.fan_out_one(&Envelope { port: 0, msg }, cancel.clone())
                        .await
                },
            )
            .await;
        }
    }
}

define_builtin_flow_node!("link call", LinkCallNode::create);
