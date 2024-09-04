use std::sync::Arc;

use crate::runtime::flow::Flow;
use crate::runtime::nodes::*;
use crate::runtime::registry::*;
use edgelink_macro::*;

const UNKNOWN_GLOBAL_NODE_TYPE: &'static str = "unknown.global";

#[derive(Debug)]
#[global_node("unknown.global")]
struct UnknownGlobalNode {
    id: ElementId,
    name: String,
    type_: &'static str,
}

impl UnknownGlobalNode {
    fn create(
        _engine: Arc<FlowEngine>,
        _config: &RedGlobalNodeConfig,
    ) -> crate::Result<Arc<dyn GlobalNodeBehavior>> {
        let node = UnknownGlobalNode {
            id: _config.id,
            name: _config.name.clone(),
            type_: UNKNOWN_GLOBAL_NODE_TYPE,
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

    fn type_name(&self) -> &'static str {
        self.type_
    }
}

#[flow_node("unknown.flow")]
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
