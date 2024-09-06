use std::sync::Arc;

use serde::Deserialize;

use crate::runtime::flow::Flow;
use crate::runtime::nodes::*;
use crate::runtime::registry::*;
use edgelink_macro::*;

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
enum LinkOutMode {
    #[default]
    #[serde(rename = "link")]
    Link = 0,

    #[serde(rename = "return")]
    Return = 1,
}

#[derive(Deserialize, Debug)]
struct LinkOutNodeConfig {
    #[serde(default)]
    mode: LinkOutMode,

    #[serde(
        default,
        deserialize_with = "crate::red::json::deser::deser_red_id_vec"
    )]
    links: Vec<ElementId>,
}

#[derive(Debug)]
#[flow_node("link out")]
struct LinkOutNode {
    base: FlowNode,
    mode: LinkOutMode,
    linked_nodes: Vec<Weak<dyn FlowNodeBehavior>>,
}

impl LinkOutNode {
    fn build(
        flow: &Flow,
        state: FlowNode,
        _config: &RedFlowNodeConfig,
    ) -> crate::Result<Box<dyn FlowNodeBehavior>> {
        let link_out_config = LinkOutNodeConfig::deserialize(&_config.json)?;
        let engine = flow.engine.upgrade().expect("The engine must be created!");

        let mut linked_nodes = Vec::new();
        if link_out_config.mode == LinkOutMode::Link {
            for link_in_id in link_out_config.links.iter() {
                if let Some(link_in) = flow.get_node_by_id(link_in_id) {
                    linked_nodes.push(Arc::downgrade(&link_in));
                } else if let Some(link_in) = engine.find_flow_node_by_id(link_in_id) {
                    linked_nodes.push(Arc::downgrade(&link_in));
                } else {
                    log::error!(
                        "LinkOutNode: Cannot found the required `link in` node(id={})!",
                        link_in_id
                    );
                    return Err(EdgelinkError::BadFlowsJson(
                        "Cannot found the required `link in` node".to_string(),
                    )
                    .into());
                }
            }
        }

        let node = LinkOutNode {
            base: state,
            mode: link_out_config.mode,
            linked_nodes,
        };
        Ok(Box::new(node))
    }

    async fn uow(&self, msg: Arc<RwLock<Msg>>, cancel: CancellationToken) -> crate::Result<()> {
        match self.mode {
            LinkOutMode::Link => {
                for link_node in self.linked_nodes.iter() {
                    if let Some(link_node) = link_node.upgrade() {
                        link_node.inject_msg(msg.clone(), cancel.clone()).await?;
                    } else {
                        let err_msg = format!(
                            "The required `link in` was unavailable in `link out` node(id={})!",
                            self.id()
                        );
                        return Err(EdgelinkError::InvalidOperation(err_msg).into());
                    }
                }
            }
            LinkOutMode::Return => {
                let engine = self
                    .get_node()
                    .flow
                    .upgrade()
                    .and_then(|f| f.engine.upgrade())
                    .expect("The engine cannot be released");
                let mut msg_guard = msg.write().await;
                if let Some(link_call_stack) = &mut msg_guard.link_call_stack {
                    if let Some(source_link) = link_call_stack.last() {
                        if let Some(target_node) =
                            engine.find_flow_node_by_id(&source_link.link_call_node_id)
                        {
                            target_node.inject_msg(msg.clone(), cancel.clone()).await?;
                        } else {
                            return Err(EdgelinkError::InvalidData(format!(
                                "Cannot found the `link call` node by id='{}'",
                                source_link.link_call_node_id
                            ))
                            .into());
                        }
                    } else {
                        return Err(EdgelinkError::InvalidData(format!(
                            "The `link call stack` is empty for msg: {:?}",
                            msg
                        ))
                        .into());
                    }
                } else {
                    return Err(EdgelinkError::InvalidOperation(format!(
                        "The `link call stack` is empty for msg {:?}",
                        msg
                    ))
                    .into());
                }
            }
        }

        Ok(())
    }
}

#[async_trait]
impl FlowNodeBehavior for LinkOutNode {
    fn get_node(&self) -> &FlowNode {
        &self.base
    }

    fn as_any(&self) -> &dyn ::std::any::Any {
        self
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
