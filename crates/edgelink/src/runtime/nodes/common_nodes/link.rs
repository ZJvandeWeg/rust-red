use std::sync::Arc;

use crate::define_builtin_flow_node;
use crate::runtime::flow::Flow;
use crate::runtime::nodes::*;

struct LinkInNode {
    state: FlowNodeState,
}

impl LinkInNode {
    fn create(
        _flow: Arc<Flow>,
        state: FlowNodeState,
        _config: &RedFlowNodeConfig,
    ) -> crate::Result<Arc<dyn FlowNodeBehavior>> {
        let node = LinkInNode { state };
        Ok(Arc::new(node))
    }
}

#[async_trait]
impl FlowNodeBehavior for LinkInNode {
    fn state(&self) -> &FlowNodeState {
        &self.state
    }

    async fn run(self: Arc<Self>, stop_token: CancellationToken) {
        while !stop_token.is_cancelled() {
            match self.wait_for_msg(stop_token.child_token()).await {
                Ok(msg) => {
                    let envelope = Envelope { port: 0, msg };
                    self.fan_out_one(&envelope, stop_token.child_token())
                        .await
                        .expect("Should be OK");
                }
                Err(ref err) => {
                    log::error!("Error: {:#?}", err);
                    break;
                }
            }
        }
    }
}

define_builtin_flow_node!("link in", LinkInNode::create);

struct LinkOutNode {
    state: FlowNodeState,
}

impl LinkOutNode {
    fn create(
        _flow: Arc<Flow>,
        state: FlowNodeState,
        _config: &RedFlowNodeConfig,
    ) -> crate::Result<Arc<dyn FlowNodeBehavior>> {
        let node = LinkOutNode { state };
        Ok(Arc::new(node))
    }
}

#[async_trait]
impl FlowNodeBehavior for LinkOutNode {
    fn state(&self) -> &FlowNodeState {
        &self.state
    }

    async fn run(self: Arc<Self>, stop_token: CancellationToken) {
        while !stop_token.is_cancelled() {
            match self.wait_for_msg(stop_token.child_token()).await {
                Ok(msg) => {
                    let envelope = Envelope { port: 0, msg };
                    self.fan_out_one(&envelope, stop_token.child_token())
                        .await
                        .expect("Should be OK");
                }
                Err(ref err) => {
                    log::error!("Error: {:#?}", err);
                    break;
                }
            }
        }
    }
}

define_builtin_flow_node!("link out", LinkOutNode::create);
