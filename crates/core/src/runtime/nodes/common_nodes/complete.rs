use std::sync::Arc;

use crate::runtime::flow::Flow;
use crate::runtime::nodes::*;
use crate::runtime::registry::*;
use edgelink_macro::*;

#[derive(Debug)]
#[flow_node("complete")]
struct CompleteNode {
    base: FlowNode,
}

impl CompleteNode {
    fn build(_flow: &Flow, state: FlowNode, _config: &RedFlowNodeConfig) -> crate::Result<Box<dyn FlowNodeBehavior>> {
        let node = CompleteNode { base: state };
        Ok(Box::new(node))
    }
}

#[async_trait]
impl FlowNodeBehavior for CompleteNode {
    fn get_node(&self) -> &FlowNode {
        &self.base
    }

    fn as_any(&self) -> &dyn ::std::any::Any {
        self
    }

    async fn run(self: Arc<Self>, stop_token: CancellationToken) {
        while !stop_token.is_cancelled() {
            // We are not using the Unit of Work stuff here!
            match self.recv_msg(stop_token.clone()).await {
                Ok(msg) => match self.fan_out_one(&Envelope { port: 0, msg }, stop_token.clone()).await {
                    Ok(_) => {}
                    Err(err) => {
                        log::error!(
                            "Fatal error: failed to fan out message in CompleteNode(id='{}', name='{}'): {:?}",
                            self.id(),
                            self.name(),
                            err
                        );
                    }
                },
                Err(ref err) => {
                    log::error!("Error: {:#?}", err);
                    break;
                }
            }
        }
    }
}
