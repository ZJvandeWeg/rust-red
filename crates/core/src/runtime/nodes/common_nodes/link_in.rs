use std::sync::Arc;

use crate::define_builtin_flow_node;
use crate::runtime::flow::Flow;
use crate::runtime::nodes::*;

#[derive(Debug)]
struct LinkInNode {
    state: FlowNodeState,
}

impl LinkInNode {
    fn create(
        _flow: &Flow,
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

define_builtin_flow_node!("link in", LinkInNode::create);
