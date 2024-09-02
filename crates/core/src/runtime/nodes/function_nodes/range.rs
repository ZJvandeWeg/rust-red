use std::str::FromStr;
use std::sync::Arc;

use crate::define_builtin_flow_node;
use crate::runtime::flow::Flow;
use crate::runtime::model::*;
use crate::runtime::nodes::*;

struct RangeNode {
    base: FlowNode,

    action: RangeAction,
    round: bool,
    minin: f64,
    maxin: f64,
    minout: f64,
    maxout: f64,
    property: String,
}

impl RangeNode {
    fn create(
        _flow: &Flow,
        base_node: FlowNode,
        _config: &RedFlowNodeConfig,
    ) -> crate::Result<Arc<dyn FlowNodeBehavior>> {
        let node = RangeNode {
            base: base_node,
            action: _config
                .json
                .get("action")
                .and_then(|jv| jv.as_str())
                .and_then(|value| RangeAction::from_str(value).ok())
                .ok_or(EdgelinkError::NotSupported(
                    "Bad range node action".to_string(),
                ))?,

            round: _config
                .json
                .get("round")
                .and_then(|jv| jv.as_bool())
                .unwrap_or(false),

            minin: _config
                .json
                .get("minin")
                .and_then(|jv| jv.as_str())
                .and_then(|value| value.parse::<f64>().ok())
                .unwrap_or(0.0),

            maxin: _config
                .json
                .get("maxin")
                .and_then(|jv| jv.as_str())
                .and_then(|value| value.parse::<f64>().ok())
                .unwrap_or(0.0),

            minout: _config
                .json
                .get("minout")
                .and_then(|jv| jv.as_str())
                .and_then(|value| value.parse::<f64>().ok())
                .unwrap_or(0.0),

            maxout: _config
                .json
                .get("maxout")
                .and_then(|jv| jv.as_str())
                .and_then(|value| value.parse::<f64>().ok())
                .unwrap_or(0.0),

            property: _config
                .json
                .get("property")
                .and_then(|jv| jv.as_str())
                .and_then(|v| {
                    if v.is_empty() {
                        None
                    } else {
                        Some(v.to_string())
                    }
                })
                .unwrap_or("payload".to_string()),
        };
        Ok(Arc::new(node))
    }

    fn do_range(&self, msg: &mut Msg) {
        if let Some(value) = msg.get_trimmed_nav_property_mut(&self.property) {
            let mut n: f64 = match value {
                Variant::Rational(num_value) => *num_value,
                Variant::String(s) => s.parse::<f64>().unwrap(),
                _ => f64::NAN,
            };

            if !n.is_nan() {
                match self.action {
                    RangeAction::Drop => {
                        if n < self.minin || n > self.maxin {
                            return;
                        }
                    }

                    RangeAction::Clamp => n = n.clamp(self.minin, self.maxin),

                    RangeAction::Roll => {
                        let divisor = self.maxin - self.minin;
                        n = ((n - self.minin) % divisor + divisor) % divisor + self.minin;
                    }

                    _ => {}
                };

                let mut new_value = ((n - self.minin) / (self.maxin - self.minin)
                    * (self.maxout - self.minout))
                    + self.minout;
                if self.round {
                    new_value = new_value.round();
                }

                *value = Variant::Rational(new_value);
            }
        }
    }
}

#[derive(Debug)]
enum RangeAction {
    Scale,
    Drop,
    Clamp,
    Roll,
}

impl FromStr for RangeAction {
    type Err = ();

    fn from_str(input: &str) -> Result<RangeAction, Self::Err> {
        match input.to_lowercase().as_str() {
            "scale" => Ok(RangeAction::Scale),
            "drop" => Ok(RangeAction::Drop),
            "clamp" => Ok(RangeAction::Clamp),
            "roll" => Ok(RangeAction::Roll),
            _ => Err(()),
        }
    }
}

#[async_trait]
impl FlowNodeBehavior for RangeNode {
    fn get_node(&self) -> &FlowNode {
        &self.base
    }

    async fn run(self: Arc<Self>, stop_token: CancellationToken) {
        while !stop_token.is_cancelled() {
            let cancel = stop_token.child_token();
            with_uow(
                self.as_ref(),
                cancel.child_token(),
                |node, msg| async move {
                    {
                        let mut msg_guard = msg.write().await;
                        node.do_range(&mut msg_guard);
                    }
                    node.fan_out_one(&Envelope { port: 0, msg }, cancel.child_token())
                        .await?;
                    Ok(())
                },
            )
            .await;
        }
    }
}

define_builtin_flow_node!("range", RangeNode::create);
