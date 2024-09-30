use std::sync::Arc;

use crate::runtime::flow::Flow;
use crate::runtime::nodes::*;
use edgelink_macro::*;

const UNKNOWN_GLOBAL_NODE_TYPE: &str = "unknown.global";

#[derive(Debug)]
#[global_node("unknown.global")]
struct UnknownGlobalNode {
    base: GlobalNode,
}

impl UnknownGlobalNode {
    fn build(engine: Arc<Engine>, config: &RedGlobalNodeConfig) -> crate::Result<Box<dyn GlobalNodeBehavior>> {
        let context = engine.get_context_manager().new_context(&engine.context(), config.id.to_string());
        let node = Self {
            base: GlobalNode {
                id: config.id,
                name: config.name.clone(),
                type_str: UNKNOWN_GLOBAL_NODE_TYPE,
                ordering: config.ordering,
                disabled: config.disabled,
                context,
            },
        };
        Ok(Box::new(node))
    }
}

impl GlobalNodeBehavior for UnknownGlobalNode {
    fn get_node(&self) -> &GlobalNode {
        &self.base
    }
}

#[flow_node("unknown.flow")]
struct UnknownFlowNode {
    base: FlowNode,
}

impl UnknownFlowNode {
    fn build(_flow: &Flow, base: FlowNode, _config: &RedFlowNodeConfig) -> crate::Result<Box<dyn FlowNodeBehavior>> {
        let node = UnknownFlowNode { base };
        Ok(Box::new(node))
    }
}

#[async_trait]
impl FlowNodeBehavior for UnknownFlowNode {
    fn get_node(&self) -> &FlowNode {
        &self.base
    }

    async fn run(self: Arc<Self>, stop_token: CancellationToken) {
        while !stop_token.is_cancelled() {
            stop_token.cancelled().await;
        }
    }
}
