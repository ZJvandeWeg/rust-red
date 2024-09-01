use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

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
struct MsgEvent {
    msg: Arc<RwLock<Msg>>,
    timeout_handle: tokio::task::AbortHandle,
}

impl Drop for MsgEvent {
    fn drop(&mut self) {
        if !self.timeout_handle.is_finished() {
            self.timeout_handle.abort();
        }
    }
}

#[derive(Debug)]
struct LinkCallMutState {
    timeout_tasks: JoinSet<()>,
    msg_events: HashMap<ElementId, MsgEvent>,
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
                    return Err(EdgelinkError::BadFlowsJson().into());
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
                timeout_tasks: JoinSet::new(),
            }),
        };
        Ok(Arc::new(node))
    }

    async fn uow(
        &self,
        node: Arc<Self>,
        msg: Arc<RwLock<Msg>>,
        cancel: CancellationToken,
    ) -> crate::Result<()> {
        let stack_top_entry = {
            let locked_msg = msg.read().await;
            if let Some(stack) = &locked_msg.link_call_stack {
                stack.last().map(|x| x.clone())
            } else {
                None
            }
        };

        match stack_top_entry {
            Some(stack_top_entry) if stack_top_entry.link_call_node_id == self.id() => {
                // We've got a `return msg`.
                // And yes, we are not allowed the recursive call rightnow.
                let mut locked_msg = msg.write().await;
                if let Some(p) = locked_msg.pop_link_source() {
                    assert!(p.link_call_node_id == self.id());
                    let mut mut_state = self.mut_state.lock().await;
                    if let Some(event) = mut_state.msg_events.remove(&p.id) {
                        self.fan_out_one(
                            &Envelope {
                                msg: event.msg.clone(),
                                port: 0,
                            },
                            cancel,
                        )
                        .await?;
                        drop(event);
                    }
                } else {
                    return Err(EdgelinkError::InvalidOperation(format!(
                        "Cannot pop link call stack in msg!: {:?}",
                        msg
                    ))
                    .into());
                }
            }
            _ =>
            // Fresh incoming msg
            {
                self.forward_call_msg(node.clone(), msg, cancel).await?
            }
        }

        Ok(())
    }

    async fn forward_call_msg(
        &self,
        node: Arc<Self>,
        msg: Arc<RwLock<Msg>>,
        cancel: CancellationToken,
    ) -> crate::Result<()> {
        let entry_id;
        let cloned_msg;
        {
            let mut locked_msg = msg.write().await;
            entry_id = ElementId::with_u64(self.event_id_atomic.fetch_add(1, Ordering::Relaxed));
            locked_msg.push_link_source(LinkSourceEntry {
                id: entry_id,
                link_call_node_id: self.id(),
            });
            cloned_msg = Arc::new(RwLock::new(locked_msg.clone()));
        }
        {
            let mut mut_state = self.mut_state.lock().await;
            let timeout_handle = mut_state
                .timeout_tasks
                .spawn(async move { node.timeout_task(entry_id).await });
            mut_state.msg_events.insert(
                entry_id,
                MsgEvent {
                    msg: cloned_msg,
                    timeout_handle,
                },
            );
        }
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
                        return Err(EdgelinkError::InvalidOperation(err_msg).into());
                    }
                }
            }
            LinkType::Dynamic => {
                // get the target_node_id from msg
                todo!()
            }
        }
        Ok(())
    }

    async fn timeout_task(&self, event_id: ElementId) {
        tokio::time::sleep(Duration::from_secs_f64(self.config.timeout.unwrap_or(30.0))).await;
        log::warn!("LinkCallNode: flow timed out, event_id={}", event_id);
        let mut mut_state = self.mut_state.lock().await;
        if let Some(event) = mut_state.msg_events.remove(&event_id) {
            drop(event);
        // TODO report the msg
        } else {
            log::warn!("LinkCallNode: Cannot found the event_id={}", event_id);
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
                node.uow(node.clone(), msg, cancel).await
            })
            .await;
        }

        {
            let mut mut_state = self.mut_state.lock().await;
            if !mut_state.timeout_tasks.is_empty() {
                mut_state.timeout_tasks.abort_all();
            }
        }
    }
}

define_builtin_flow_node!("link call", LinkCallNode::create);
