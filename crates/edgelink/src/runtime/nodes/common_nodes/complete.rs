use std::sync::Arc;

use crate::define_builtin_flow_node;
use crate::runtime::flow::Flow;
use crate::runtime::nodes::*;

struct CompleteNode {
    state: FlowNodeState,
}

impl CompleteNode {
    fn create(
        _flow: Arc<Flow>,
        state: FlowNodeState,
        _config: &RedFlowNodeConfig,
    ) -> crate::Result<Arc<dyn FlowNodeBehavior>> {
        let node = CompleteNode { state };
        Ok(Arc::new(node))
    }
}

#[async_trait]
impl FlowNodeBehavior for CompleteNode {
    fn state(&self) -> &FlowNodeState {
        &self.state
    }

    async fn run(self: Arc<Self>, stop_token: CancellationToken) {
        while !stop_token.is_cancelled() {
            match self.wait_for_msg(stop_token.child_token()).await {
                Ok(msg) => {
                    self.fan_out_one(&Envelope { port: 0, msg }, stop_token.child_token())
                        .await
                        .unwrap(); //FIXME
                }
                Err(ref err) => {
                    log::error!("Error: {:#?}", err);
                    break;
                }
            }
        }
    }
}

define_builtin_flow_node!("complete", CompleteNode::create);
