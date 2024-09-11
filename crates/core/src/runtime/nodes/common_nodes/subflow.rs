use std::sync::Arc;

use crate::runtime::flow::Flow;
use crate::runtime::model::json::helpers;
use crate::runtime::nodes::*;
use crate::runtime::registry::*;
use edgelink_macro::*;

#[derive(Debug)]
#[flow_node("subflow")]
struct SubflowNode {
    base: FlowNode,
    subflow_id: ElementId,
}

impl SubflowNode {
    fn build(_flow: &Flow, state: FlowNode, config: &RedFlowNodeConfig) -> crate::Result<Box<dyn FlowNodeBehavior>> {
        let subflow_id = config
            .json
            .get("type")
            .and_then(|s| s.as_str())
            .and_then(|s| s.split_once(':'))
            .and_then(|p| helpers::parse_red_id_str(p.1))
            .ok_or(EdgelinkError::BadFlowsJson(
                "Cannot get or parse the `type` property with the format `subflow:xxx`".to_string(),
            ))?;

        //let subflow = flow.engine.upgrade().unwrap().flows
        let node = SubflowNode { base: state, subflow_id };
        Ok(Box::new(node))
    }
}

#[async_trait]
impl FlowNodeBehavior for SubflowNode {
    fn get_node(&self) -> &FlowNode {
        &self.base
    }

    fn as_any(&self) -> &dyn ::std::any::Any {
        self
    }

    async fn run(self: Arc<Self>, stop_token: CancellationToken) {
        while !stop_token.is_cancelled() {
            let cancel = stop_token.clone();
            with_uow(self.as_ref(), stop_token.clone(), |node, msg| async move {
                if let Some(engine) = node.get_node().flow.upgrade().and_then(|f| f.engine.upgrade()) {
                    engine.inject_msg_to_flow(&node.subflow_id, msg, cancel.clone()).await?;
                }

                Ok(())
            })
            .await;
        }
    }
}
