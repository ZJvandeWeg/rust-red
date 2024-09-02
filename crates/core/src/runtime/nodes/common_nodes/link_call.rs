use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use serde::Deserialize;
use tokio::sync::Mutex;
use tokio::task::JoinSet;

use crate::define_builtin_flow_node;
use crate::red::json::deser::parse_red_id_str;
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
    base: FlowNode,
    config: LinkCallNodeConfig,
    linked_nodes: Vec<Weak<dyn FlowNodeBehavior>>,
    event_id_atomic: AtomicU64,
    mut_state: Mutex<LinkCallMutState>,
}

impl LinkCallNode {
    fn create(
        flow: &Flow,
        state: FlowNode,
        config: &RedFlowNodeConfig,
    ) -> crate::Result<Arc<dyn FlowNodeBehavior>> {
        let link_call_config = LinkCallNodeConfig::deserialize(&config.json)?;
        let engine = flow.engine.upgrade().expect("The engine must be created!");

        let mut linked_nodes = Vec::new();
        if link_call_config.link_type == LinkType::Static {
            for link_in_id in link_call_config.links.iter() {
                if let Some(link_in) = engine.find_flow_node_by_id(link_in_id) {
                    linked_nodes.push(Arc::downgrade(&link_in));
                } else {
                    log::error!(
                        "LinkCallNode: Cannot found the required `link in` node(id={})!",
                        link_in_id
                    );
                    return Err(EdgelinkError::BadFlowsJson(
                        "Cannot found the required `link in`".to_string(),
                    )
                    .into());
                }
            }
        }

        let node = LinkCallNode {
            base: state,
            config: link_call_config,
            event_id_atomic: AtomicU64::new(1),
            linked_nodes,
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
                stack.last().copied()
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
                let target_node = {
                    let locked_msg = msg.read().await;
                    self.get_dynamic_target_node(&locked_msg)?
                };
                if let Some(target_node) = target_node {
                    // Now we got the dynamic target
                    target_node.inject_msg(msg.clone(), cancel.clone()).await?;
                } else {
                    let err_msg = "Cannot found node by msg.target";
                    return Err(EdgelinkError::InvalidOperation(err_msg.to_string()).into());
                }
            }
        }
        Ok(())
    }

    fn get_dynamic_target_node(
        &self,
        msg: &Msg,
    ) -> crate::Result<Option<Arc<dyn FlowNodeBehavior>>> {
        let target_field = msg.body.get("target").ok_or(EdgelinkError::InvalidData(
            "There are no `target` field in the msg!".to_string(),
        ))?;

        let result = match target_field {
            Variant::String(target_name) => {
                let engine = self.get_engine().expect("The engine must be instanced!");
                // Firstly, we are looking into the node ids
                if let Some(parsed_id) = parse_red_id_str(target_name) {
                    let found = engine.find_flow_node_by_id(&parsed_id);
                    if found.is_some() {
                        found
                    } else {
                        None
                    }
                } else {
                    // Secondly, we are looking into the node names
                    // Otherwises, there is no such target node
                    engine.find_flow_node_by_name(target_name)?
                }
            }
            _ => {
                let err_msg = format!(
                    "Unsupported dynamic target in `msg.target`: {:?}",
                    target_field
                );
                return Err(EdgelinkError::InvalidOperation(err_msg).into());
            }
        };
        if let Some(node) = &result {
            let flow = node
                .get_node()
                .flow
                .upgrade()
                .ok_or(EdgelinkError::InvalidOperation(
                    "The flow cannot be released".to_string(),
                ))?;
            if flow.is_subflow() {
                return Err(EdgelinkError::InvalidData(
                    "A `link call` cannot call a `link in` node inside a subflow".to_string(),
                )
                .into());
            }
        }
        Ok(result)
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
    fn get_node(&self) -> &FlowNode {
        &self.base
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
