use std::sync::Arc;

use crate::runtime::flow::Flow;
use crate::runtime::nodes::*;
use crate::runtime::registry::*;
use edgelink_macro::*;

#[derive(Debug)]
#[flow_node("link in")]
struct LinkInNode {
    base: FlowNode,
}

impl LinkInNode {
    fn build(
        _flow: &Flow,
        state: FlowNode,
        _config: &RedFlowNodeConfig,
    ) -> crate::Result<Box<dyn FlowNodeBehavior>> {
        let node = LinkInNode { base: state };
        Ok(Box::new(node))
    }
}

#[async_trait]
impl FlowNodeBehavior for LinkInNode {
    fn get_node(&self) -> &FlowNode {
        &self.base
    }

    fn as_any(&self) -> &dyn ::std::any::Any {
        self
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
