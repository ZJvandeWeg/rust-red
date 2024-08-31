use std::sync::Arc;

use crate::define_builtin_flow_node;
use crate::red::json::helpers;
use crate::runtime::flow::Flow;
use crate::runtime::nodes::*;

struct SubflowNode {
    state: FlowNodeState,
    subflow_id: ElementId,
}

impl SubflowNode {
    fn create(
        _flow: &Flow,
        state: FlowNodeState,
        config: &RedFlowNodeConfig,
    ) -> crate::Result<Arc<dyn FlowNodeBehavior>> {
        let subflow_id = config
            .json
            .get("type")
            .and_then(|s| s.as_str())
            .and_then(|s| s.split_once(':'))
            .and_then(|p| helpers::parse_red_id_str(p.1))
            .ok_or(EdgeLinkError::BadFlowsJson())?;

        //let subflow = flow.engine.upgrade().unwrap().flows
        let node = SubflowNode { state, subflow_id };
        Ok(Arc::new(node))
    }
}

#[async_trait]
impl FlowNodeBehavior for SubflowNode {
    fn state(&self) -> &FlowNodeState {
        &self.state
    }

    async fn run(self: Arc<Self>, stop_token: CancellationToken) {
        while !stop_token.is_cancelled() {
            let cancel = stop_token.clone();
            with_uow(self.as_ref(), stop_token.clone(), |node, msg| async move {
                if let Some(engine) = node.state().flow.upgrade().and_then(|f| f.engine.upgrade()) {
                    engine
                        .inject_msg_to_flow(&node.subflow_id, msg, cancel.clone())
                        .await?;
                }

                Ok(())
            })
            .await;
        }
    }
}

define_builtin_flow_node!("subflow", SubflowNode::create);
