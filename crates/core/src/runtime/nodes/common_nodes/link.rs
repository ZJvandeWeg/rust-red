use std::str::FromStr;
use std::sync::Arc;

use serde::Deserialize;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
enum LinkOutMode {
    #[serde(rename = "link")]
    Link = 0,

    #[serde(rename = "return")]
    Return = 1,
}

#[derive(Debug)]
struct LinkOutNode {
    state: FlowNodeState,
    config: LinkOutNodeConfig,
    linked_nodes: Vec<Arc<dyn FlowNodeBehavior>>,
}

#[derive(Deserialize, Debug)]
struct LinkOutNodeConfig {
    mode: LinkOutMode,

    #[serde(deserialize_with = "crate::red::json::deser::deser_red_id_vec")]
    links: Vec<ElementId>,
}

impl LinkOutNode {
    fn create(
        flow: &Flow,
        state: FlowNodeState,
        _config: &RedFlowNodeConfig,
    ) -> crate::Result<Arc<dyn FlowNodeBehavior>> {
        let link_out_config: LinkOutNodeConfig = LinkOutNodeConfig::deserialize(&_config.json)?;
        let engine = flow.engine.upgrade().expect("The engine must be created!");

        let mut linked_nodes = Vec::new();
        for link_in_id in link_out_config.links.iter() {
            if let Some(link_in) = engine.find_flow_node(&link_in_id) {
                linked_nodes.push(link_in.clone());
            }
        }

        let node = LinkOutNode {
            state,
            config: link_out_config,
            linked_nodes,
        };
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
            let cancel = stop_token.clone();
            with_uow(self.as_ref(), stop_token.clone(), |node, msg| async move {
                if let Some(engine) = node.state().flow.upgrade().and_then(|f| f.engine.upgrade()) {
                    /*
                    engine
                        .inject_msg_to_flow(&node., msg, cancel.clone())
                        .await?;
                    */
                }

                Ok(())
            })
            .await;
        }
    }
}

define_builtin_flow_node!("link out", LinkOutNode::create);
