use std::sync::Arc;

use serde::Deserialize;

use crate::define_builtin_flow_node;
use crate::runtime::flow::Flow;
use crate::runtime::nodes::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
enum LinkOutMode {
    #[serde(rename = "link")]
    Link = 0,

    #[serde(rename = "return")]
    Return = 1,
}

#[derive(Deserialize, Debug)]
struct LinkOutNodeConfig {
    mode: LinkOutMode,

    #[serde(deserialize_with = "crate::red::json::deser::deser_red_id_vec")]
    links: Vec<ElementId>,
}

#[derive(Debug)]
struct LinkOutNode {
    state: FlowNodeState,
    config: LinkOutNodeConfig,
    linked_nodes: Vec<Weak<dyn FlowNodeBehavior>>,
}

impl LinkOutNode {
    fn create(
        flow: &Flow,
        state: FlowNodeState,
        _config: &RedFlowNodeConfig,
    ) -> crate::Result<Arc<dyn FlowNodeBehavior>> {
        let link_out_config = LinkOutNodeConfig::deserialize(&_config.json)?;
        let engine = flow.engine.upgrade().expect("The engine must be created!");

        let mut linked_nodes = Vec::new();
        if link_out_config.mode == LinkOutMode::Link {
            for link_in_id in link_out_config.links.iter() {
                if let Some(link_in) = engine.find_flow_node(link_in_id) {
                    linked_nodes.push(Arc::downgrade(&link_in));
                } else {
                    log::error!(
                        "LinkOutNode: Cannot found the required `link in` node(id={})!",
                        link_in_id
                    );
                    return Err(EdgeLinkError::BadFlowsJson().into());
                }
            }
        }

        let node = LinkOutNode {
            state,
            config: link_out_config,
            linked_nodes,
        };
        Ok(Arc::new(node))
    }

    async fn uow(&self, msg: Arc<RwLock<Msg>>, cancel: CancellationToken) -> crate::Result<()> {
        match self.config.mode {
            LinkOutMode::Link => {
                for link_node in self.linked_nodes.iter() {
                    if let Some(link_node) = link_node.upgrade() {
                        link_node.inject_msg(msg.clone(), cancel.clone()).await?;
                    } else {
                        let err_msg = format!(
                            "The required `link in` was unavailable in `link out` node(id={})!",
                            self.id()
                        );
                        return Err(EdgeLinkError::InvalidOperation(err_msg).into());
                    }
                }
            }
            LinkOutMode::Return => {
                if let Some(engine) = self.state().flow.upgrade().and_then(|f| f.engine.upgrade()) {
                    let mut msg_guard = msg.write().await;
                    if let Some(link_source) = &mut msg_guard.link_source {
                        if link_source.len() > 0 {
                            if let Some(source_link) = link_source.pop() {
                                if let Some(target_node) =
                                    engine.find_flow_node(&source_link.link_call_node_id)
                                {
                                    target_node.inject_msg(msg.clone(), cancel.clone()).await?;
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(())
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
            with_uow(self.as_ref(), stop_token.clone(), |node, msg| {
                node.uow(msg, cancel.clone())
            })
            .await;
        }
    }
}

define_builtin_flow_node!("link out", LinkOutNode::create);
