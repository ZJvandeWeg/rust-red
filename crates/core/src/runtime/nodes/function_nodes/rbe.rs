use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;

use crate::runtime::flow::Flow;
use crate::runtime::model::*;
use crate::runtime::nodes::*;
use crate::runtime::registry::*;
use edgelink_macro::*;

#[derive(Debug, PartialEq, PartialOrd, Eq)]
enum RbeFunc {
    Rbe,
    Rbei,
    Narrowband,
    NarrowbandEq,
    DeadBand,
    DeadbandEq,
}

impl FromStr for RbeFunc {
    type Err = ();

    #[allow(clippy::match_str_case_mismatch)]
    fn from_str(input: &str) -> Result<RbeFunc, Self::Err> {
        match input.to_lowercase().as_str() {
            "rbe" => Ok(RbeFunc::Rbe),
            "rbei" => Ok(RbeFunc::Rbei),
            "narrowband" => Ok(RbeFunc::Narrowband),
            "narrowbandEq" => Ok(RbeFunc::NarrowbandEq),
            "deadband" => Ok(RbeFunc::DeadBand),
            "deadbandEq" => Ok(RbeFunc::DeadbandEq),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Eq)]
enum Inout {
    In,
    Out,
}

impl FromStr for Inout {
    type Err = ();

    fn from_str(input: &str) -> Result<Inout, Self::Err> {
        match input.to_lowercase().as_str() {
            "in" => Ok(Inout::In),
            "out" => Ok(Inout::Out),
            _ => Err(()),
        }
    }
}

#[derive(Debug)]
struct RbeNodeState {
    gap: f64,
    prev: HashMap<String, Variant>,
}

#[derive(Debug)]
#[flow_node("rbe")]
struct RbeNode {
    base: FlowNode,
    func: RbeFunc,
    gap: f64,
    start_value: Option<f64>,
    inout: Inout,
    percent: bool,
    property: String,
    topic: String,
    sep_topics: bool,
    // scope: BTreeMap<ElementId, Arc<dyn FlowNodeBehavior>>,
}

impl RbeNode {
    fn create(
        _flow: &Flow,
        base_node: FlowNode,
        _config: &RedFlowNodeConfig,
    ) -> crate::Result<Arc<dyn FlowNodeBehavior>> {
        let node = RbeNode {
            base: base_node,
            // scope: BTreeMap::new(),
            func: _config
                .json
                .get("func")
                .and_then(|jv| jv.as_str())
                .and_then(|value| RbeFunc::from_str(value).ok())
                .unwrap_or(RbeFunc::Rbe),

            gap: _config
                .json
                .get("gap")
                .and_then(|jv| jv.as_str())
                .and_then(|s| s.strip_suffix("%"))
                .and_then(|value| value.parse::<f64>().ok())
                .unwrap_or(0.0),

            start_value: _config
                .json
                .get("start")
                .and_then(|jv| jv.as_str())
                .and_then(|s| s.strip_suffix("%"))
                .and_then(|value| value.parse::<f64>().ok()),

            inout: _config
                .json
                .get("inout")
                .and_then(|jv| jv.as_str())
                .and_then(|value| Inout::from_str(value).ok())
                .unwrap_or(Inout::Out),

            percent: _config
                .json
                .get("gap")
                .and_then(|jv| jv.as_str())
                .map(|s| {
                    if let Some(c) = s.chars().last() {
                        c == '%'
                    } else {
                        false
                    }
                })
                .expect("No way"),

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

            topic: _config
                .json
                .get("topi")
                .and_then(|jv| jv.as_str())
                .and_then(|v| {
                    if v.is_empty() {
                        None
                    } else {
                        Some(v.to_string())
                    }
                })
                .unwrap_or("topic".to_string()),

            sep_topics: _config
                .json
                .get("septopics")
                .and_then(|jv| jv.as_bool())
                .unwrap_or(false),
        };
        Ok(Arc::new(node))
    }

    fn do_filter(&self, msg: &mut Msg, state: &mut RbeNodeState) -> bool {
        // If the `topic` cannot be retrieved or is an empty string, convert it to `None`.
        let topic = match msg.get_trimmed_nav_property(&self.topic) {
            Some(Variant::String(topic)) => {
                if !topic.is_empty() {
                    Some(topic)
                } else {
                    None
                }
            }
            _ => None,
        };

        // reset previous storage
        if msg.get_property("reset").is_some() {
            match (self.sep_topics, topic) {
                (true, Some(topic_value)) => {
                    state.prev.remove(topic_value);
                }
                (_, _) => {
                    state.prev.clear();
                }
            };
        }

        // TODO: The following code is directly copied from Node-RED JS and needs to be rewritten and optimized.

        if let Some(prop_value) = msg.get_trimmed_nav_property(&self.property) {
            let t = match (self.sep_topics, topic) {
                (true, Some(topic)) => topic,
                _ => "_no_topic",
            };

            //let mut prev_value = state.prev.entry(t.to_string()).or_insert(Variant::Null);

            match self.func {
                RbeFunc::Rbe | RbeFunc::Rbei => {
                    let do_send = self.func != RbeFunc::Rbei || state.prev.contains_key(t);
                    match prop_value {
                        Variant::Object(_) => todo!(),
                        Variant::Null => panic!("DEBUG ME!"),
                        _ => {
                            if *prop_value != *state.prev.get(t).unwrap_or(&Variant::Null) {
                                state.prev.insert(t.to_string(), prop_value.clone());
                                if do_send {
                                    return true;
                                }
                            }
                        }
                    }
                }
                _ => {
                    if let Some(n) = prop_value.as_number().filter(|x| !x.is_nan()) {
                        if let (false, RbeFunc::Narrowband | RbeFunc::NarrowbandEq) =
                            (state.prev.contains_key(t), &self.func)
                        {
                            let value_to_insert = match self.start_value {
                                None => Variant::Rational(n),
                                Some(sv) => Variant::Rational(sv),
                            };
                            state.prev.insert(t.to_string(), value_to_insert);
                        }

                        // process percent
                        state.gap = if self.percent {
                            f64::abs(state.prev[t].as_number().unwrap_or(0.0) * self.gap / 100.0)
                        } else {
                            state.gap
                        };

                        if !state.prev.contains_key(t) && self.func == RbeFunc::NarrowbandEq {
                            state.prev.insert(t.to_string(), Variant::Rational(n));
                        }

                        if !state.prev.contains_key(t) {
                            state
                                .prev
                                .insert(t.to_string(), Variant::Rational(n - state.gap - 1.0));
                        }

                        let _gap_delta =
                            f64::abs(n - state.prev[t].as_number().unwrap_or(f64::NAN));

                        let do_send = match (&self.func, &self.inout) {
                            (RbeFunc::DeadbandEq | RbeFunc::Narrowband, Inout::Out)
                                if _gap_delta == state.gap =>
                            {
                                state.prev.insert(t.to_string(), Variant::Rational(n));
                                true
                            }

                            (RbeFunc::DeadBand | RbeFunc::DeadbandEq, Inout::Out)
                                if _gap_delta > state.gap =>
                            {
                                state.prev.insert(t.to_string(), Variant::Rational(n));
                                true
                            }

                            (RbeFunc::Narrowband | RbeFunc::NarrowbandEq, Inout::Out)
                                if _gap_delta < state.gap =>
                            {
                                state.prev.insert(t.to_string(), Variant::Rational(n));
                                true
                            }

                            (_, _) => false,
                        };

                        if self.inout == Inout::In {
                            state.prev.insert(t.to_string(), Variant::Rational(n));
                        }

                        return do_send;
                    } else {
                        log::warn!("rbe.warn.nonumber");
                    }

                    todo!()
                }
            }
        }
        false
    }
}

#[async_trait]
impl FlowNodeBehavior for RbeNode {
    fn get_node(&self) -> &FlowNode {
        &self.base
    }

    async fn run(self: Arc<Self>, stop_token: CancellationToken) {
        let mut state = RbeNodeState {
            gap: self.gap,
            prev: HashMap::new(),
        };

        while !stop_token.is_cancelled() {
            let cancel = stop_token.clone();
            let sub_state = &mut state;
            with_uow(
                self.as_ref(),
                cancel.child_token(),
                |node, msg| async move {
                    let can_send = {
                        let mut msg_guard = msg.write().await;
                        node.do_filter(&mut msg_guard, sub_state)
                    };
                    if can_send {
                        node.fan_out_one(&Envelope { port: 0, msg }, cancel.child_token())
                            .await?;
                    }
                    Ok(())
                },
            )
            .await;
        }

        log::debug!("DebugNode process() task has been terminated.");
    }
}
