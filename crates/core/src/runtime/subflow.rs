use std::sync::{Arc, Weak};

use json::RedFlowConfig;
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;

use crate::runtime::model::*;
use super::{flow::{Flow, FlowState}, nodes::FlowNodeBehavior};

#[derive(Debug)]
pub(crate) struct SubflowOutputPort {
    pub index: usize,
    pub owner: Weak<Flow>,
    pub msg_tx: MsgSender,
    pub msg_rx: MsgReceiverHolder,
}

#[derive(Debug)]
pub(crate) struct SubflowState {
    pub instance_node: Option<Arc<dyn FlowNodeBehavior>>,
    pub in_nodes: Vec<Arc<dyn FlowNodeBehavior>>,
    pub tx_tasks: JoinSet<()>,
    pub tx_ports: Vec<Arc<SubflowOutputPort>>,
}

impl SubflowOutputPort {
    pub(crate) async fn tx_task(&self, stop_token: CancellationToken) {
        while !stop_token.is_cancelled() {
            match self.msg_rx.recv_msg(stop_token.clone()).await {
                Ok(msg) => {
                    // Find out the subflow:xxx node
                    let instance_node = {
                        let flow =
                            self.owner.upgrade().expect("The owner of this sub-flow node has been released already!!!");

                        let subflow_state = flow.subflow_state.as_ref().expect("Subflow must have a subflow_state!");

                        let subflow_state_guard =
                            subflow_state.read().expect("Cannot acquire the lock of field `subflow_state`!!!");

                        subflow_state_guard.instance_node.clone()
                    };

                    if let Some(instance_node) = instance_node {
                        let instance_node = instance_node.clone();
                        let envelope = Envelope { port: self.index, msg };
                        if let Err(e) = instance_node.fan_out_one(&envelope, stop_token.clone()).await {
                            log::warn!("Failed to fan-out message: {:?}", e);
                        }
                    } else {
                        log::warn!("The sub-flow does not have a subflow node");
                    }
                }

                Err(e) => {
                    log::error!("Failed to receive msg in subflow_tx_task: {:?}", e);
                }
            }
        }
    }
}

impl SubflowState {
    pub(crate) fn populate_in_nodes(
        &mut self,
        flow_state: &FlowState,
        flow_config: &RedFlowConfig,
    ) -> crate::Result<()> {
        // this is a subflow with in ports
        for wire_obj in flow_config.in_ports.iter().flat_map(|x| x.wires.iter()) {
            if let Some(node) = flow_state.nodes.get(&wire_obj.id) {
                self.in_nodes.push(node.clone());
            } else {
                log::warn!("Can not found node(id='{}')", wire_obj.id);
            }
        }
        Ok(())
    }

    pub(crate) fn start_tx_tasks(&mut self, stop_token: CancellationToken) -> crate::Result<()> {
        for tx_port in self.tx_ports.iter() {
            let child_stop_token = stop_token.clone();
            let port_cloned = tx_port.clone();
            self.tx_tasks.spawn(async move {
                port_cloned.tx_task(child_stop_token.clone()).await;
            });
        }
        Ok(())
    }

    /*
    async fn stop_tx_tasks(&mut self) -> crate::Result<()> {
        while self.tx_tasks.join_next().await.is_some() {
            //
        }
        Ok(())
    }
    */
}
