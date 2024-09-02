use std::sync::Arc;

use crate::runtime::flow::Flow;
use crate::runtime::nodes::*;
use crate::{define_builtin_flow_node, define_builtin_global_node};

struct UnknownGlobalNode {
    id: ElementId,
    name: String,
}

impl UnknownGlobalNode {
    fn create(
        _engine: Arc<FlowEngine>,
        _config: &RedGlobalNodeConfig,
    ) -> crate::Result<Arc<dyn GlobalNodeBehavior>> {
        let node = UnknownGlobalNode {
            id: _config.id,
            name: _config.name.clone(),
        };
        Ok(Arc::new(node))
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
}

define_builtin_global_node!("unknown.global", UnknownGlobalNode::create);

struct UnknownFlowNode {
    state: FlowNode,
}

impl UnknownFlowNode {
    fn create(
        _flow: &Flow,
        base: FlowNode,
        _config: &RedFlowNodeConfig,
    ) -> crate::Result<Arc<dyn FlowNodeBehavior>> {
        let node = UnknownFlowNode { state: base };
        Ok(Arc::new(node))
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

define_builtin_flow_node!("unknown.flow", UnknownFlowNode::create);
