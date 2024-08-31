use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use serde::Deserialize;
use tokio::sync::Mutex;
use tokio::task::JoinSet;

use crate::define_builtin_flow_node;
use crate::runtime::flow::Flow;
use crate::runtime::nodes::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
enum LinkType {
    #[serde(rename = "dynamic")]
    Dynamic,

    #[serde(rename = "static")]
    Static,
}

#[derive(Deserialize, Debug)]
struct LinkCallNodeConfig {
    #[serde(rename = "linkType")]
    link_type: LinkType,

    #[serde(deserialize_with = "crate::red::json::deser::deser_red_id_vec")]
    links: Vec<ElementId>,

    #[serde(deserialize_with = "crate::red::json::deser::str_to_option_f64")]
    timeout: Option<f64>,
}

#[derive(Debug)]
struct LinkCallMutState {
    recv_tasks: JoinSet<()>,
    msg_events: HashMap<ElementId, Arc<RwLock<Msg>>>,
}

#[derive(Debug)]
struct LinkCallNode {
    state: FlowNodeState,
    config: LinkCallNodeConfig,
    linked_nodes: Vec<Weak<dyn FlowNodeBehavior>>,
    event_id_atomic: AtomicU64,
    mut_state: Mutex<LinkCallMutState>,
}

impl LinkCallNode {
    fn create(
        flow: &Flow,
        state: FlowNodeState,
        config: &RedFlowNodeConfig,
    ) -> crate::Result<Arc<dyn FlowNodeBehavior>> {
        let link_call_config = LinkCallNodeConfig::deserialize(&config.json)?;
        let engine = flow.engine.upgrade().expect("The engine must be created!");

        let mut linked_nodes = Vec::new();
        if link_call_config.link_type == LinkType::Static {
            for link_in_id in link_call_config.links.iter() {
                if let Some(link_in) = engine.find_flow_node(link_in_id) {
                    linked_nodes.push(Arc::downgrade(&link_in));
                } else {
                    log::error!(
                        "LinkCallNode: Cannot found the required `link in` node(id={})!",
                        link_in_id
                    );
                    return Err(EdgeLinkError::BadFlowsJson().into());
                }
            }
        }

        let node = LinkCallNode {
            state,
            config: link_call_config,
            event_id_atomic: AtomicU64::new(1),
            linked_nodes: linked_nodes,
            mut_state: Mutex::new(LinkCallMutState {
                msg_events: HashMap::new(),
                recv_tasks: JoinSet::new(),
            }),
        };
        Ok(Arc::new(node))
    }

    async fn uow(
        self: Arc<Self>,
        msg: Arc<RwLock<Msg>>,
        cancel: CancellationToken,
    ) -> crate::Result<()> {
        let cloned_msg;
        let entry_id;
        {
            let mut locked_msg = msg.write().await;
            entry_id = ElementId::with_u64(self.event_id_atomic.fetch_add(1, Ordering::Relaxed));
            locked_msg.push_link_source(LinkSourceEntry {
                id: entry_id,
                link_call_node_id: self.id(),
            });
            cloned_msg = Arc::new(RwLock::new(locked_msg.clone()));
        }

        let mut mut_state = self.mut_state.lock().await;
        mut_state.msg_events.insert(entry_id, cloned_msg);

        let node = self.clone();
        let task_cancel = cancel.child_token();
        mut_state
            .recv_tasks
            .spawn(async move { node.wait_msg_task(entry_id, task_cancel).await });

        self.fan_out_linked_msg(msg, cancel.clone()).await?;
        Ok(())
    }

    async fn fan_out_linked_msg(
        &self,
        msg: Arc<RwLock<Msg>>,
        cancel: CancellationToken,
    ) -> crate::Result<()> {
        match self.config.link_type {
            LinkType::Static => {
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
            LinkType::Dynamic => {
                todo!()
            }
        }
        Ok(())
    }

    async fn wait_msg_task(&self, event_id: ElementId, cancel: CancellationToken) {
        //  let mut mut_state = self.mut_state.lock().await;
        let timeout_result = tokio::time::timeout(
            std::time::Duration::from_secs_f64(self.config.timeout.unwrap_or(30.0)),
            async move {},
        )
        .await;

        match timeout_result {
            Ok(_) => println!("Message  processed {}", 0),
            Err(_) => println!("Message  timed out {}", 1),
        }
    }
}

#[async_trait]
impl FlowNodeBehavior for LinkCallNode {
    fn state(&self) -> &FlowNodeState {
        &self.state
    }

    async fn run(self: Arc<Self>, stop_token: CancellationToken) {
        while !stop_token.is_cancelled() {
            let cancel = stop_token.clone();
            let node = self.clone();
            with_uow(self.as_ref(), cancel.clone(), |_, msg| async move {
                LinkCallNode::uow(node, msg, cancel).await
            })
            .await;
        }

        {
            let mut mut_state = self.mut_state.lock().await;
            if !mut_state.recv_tasks.is_empty() {
                while mut_state.recv_tasks.join_next().await.is_some() {
                    //
                }
            }
        }
    }
}

define_builtin_flow_node!("link call", LinkCallNode::create);
