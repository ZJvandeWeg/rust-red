use std::sync::Arc;

use crate::runtime::flow::Flow;
use crate::runtime::nodes::*;
use crate::runtime::registry::*;
use edgelink_macro::*;

const UNKNOWN_GLOBAL_NODE_TYPE: &str = "unknown.global";

#[derive(Debug)]
#[global_node("unknown.global")]
struct UnknownGlobalNode {
    id: ElementId,
    name: String,
    type_: &'static str,
}

impl UnknownGlobalNode {
    fn build(_engine: Arc<FlowEngine>, _config: &RedGlobalNodeConfig) -> crate::Result<Box<dyn GlobalNodeBehavior>> {
        let node = UnknownGlobalNode { id: _config.id, name: _config.name.clone(), type_: UNKNOWN_GLOBAL_NODE_TYPE };
        Ok(Box::new(node))
    }
}

#[async_trait]
impl GlobalNodeBehavior for UnknownGlobalNode {
    fn id(&self) -> &ElementId {
        &self.id
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn type_name(&self) -> &'static str {
        self.type_
    }

    fn as_any(&self) -> &dyn ::std::any::Any {
        self
    }
}

#[flow_node("unknown.flow")]
struct UnknownFlowNode {
    state: FlowNode,
}

impl UnknownFlowNode {
    fn build(_flow: &Flow, base: FlowNode, _config: &RedFlowNodeConfig) -> crate::Result<Box<dyn FlowNodeBehavior>> {
        let node = UnknownFlowNode { state: base };
        Ok(Box::new(node))
    }
}

#[async_trait]
impl FlowNodeBehavior for UnknownFlowNode {
    fn get_node(&self) -> &FlowNode {
        &self.state
    }

    async fn run(self: Arc<Self>, stop_token: CancellationToken) {
        while !stop_token.is_cancelled() {
            stop_token.cancelled().await;
        }
    }
}
