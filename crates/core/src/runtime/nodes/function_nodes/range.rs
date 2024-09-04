use core::f64;
use std::str::FromStr;
use std::sync::Arc;

use crate::runtime::flow::Flow;
use crate::runtime::model::*;
use crate::runtime::nodes::*;
use crate::runtime::registry::*;
use edgelink_macro::*;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
enum RangeAction {
    #[serde(rename = "scale")]
    Scale,

    #[serde(rename = "drop")]
    Drop,

    #[serde(rename = "clamp")]
    Clamp,

    #[serde(rename = "roll")]
    Roll,
}

impl Default for RangeAction {
    fn default() -> Self {
        RangeAction::Scale
    }
}

#[derive(Deserialize, Debug)]
struct RangeNodeConfig {
    action: RangeAction,

    #[serde(default)]
    round: bool,

    #[serde(deserialize_with = "crate::red::json::deser::deser_f64_or_string_nan")]
    minin: f64,

    #[serde(deserialize_with = "crate::red::json::deser::deser_f64_or_string_nan")]
    maxin: f64,

    #[serde(deserialize_with = "crate::red::json::deser::deser_f64_or_string_nan")]
    minout: f64,

    #[serde(deserialize_with = "crate::red::json::deser::deser_f64_or_string_nan")]
    maxout: f64,

    #[serde(default = "default_config_property")]
    property: String,
}

fn default_config_property() -> String {
    "payload".to_string()
}

#[derive(Debug)]
#[flow_node("range")]
struct RangeNode {
    base: FlowNode,
    config: RangeNodeConfig,
}

impl RangeNode {
    fn create(
        _flow: &Flow,
        base_node: FlowNode,
        config: &RedFlowNodeConfig,
    ) -> crate::Result<Arc<dyn FlowNodeBehavior>> {
        let range_config = RangeNodeConfig::deserialize(&config.json)?;
        let node = RangeNode {
            base: base_node,
            config: range_config,
        };
        Ok(Arc::new(node))
    }

    fn do_range(&self, msg: &mut Msg) {
        if let Some(value) = msg.get_trimmed_nav_property_mut(&self.config.property) {
            let mut n: f64 = match value {
                Variant::Rational(num_value) => *num_value,
                Variant::String(s) => s.parse::<f64>().unwrap(),
                _ => f64::NAN,
            };

            if !n.is_nan() {
                match self.config.action {
                    RangeAction::Drop => {
                        if n < self.config.minin || n > self.config.maxin {
                            return;
                        }
                    }

                    RangeAction::Clamp => n = n.clamp(self.config.minin, self.config.maxin),

                    RangeAction::Roll => {
                        let divisor = self.config.maxin - self.config.minin;
                        n = ((n - self.config.minin) % divisor + divisor) % divisor
                            + self.config.minin;
                    }

                    _ => {}
                };

                let mut new_value = ((n - self.config.minin)
                    / (self.config.maxin - self.config.minin)
                    * (self.config.maxout - self.config.minout))
                    + self.config.minout;
                if self.config.round {
                    new_value = new_value.round();
                }

                *value = Variant::Rational(new_value);
            }
        }
    }
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
