use std::sync::Arc;

use serde::Deserialize;
use serde_json::Value;

use crate::runtime::eval;
use crate::runtime::flow::Flow;
use crate::runtime::model::*;
use crate::runtime::nodes::*;
use crate::runtime::registry::*;
use edgelink_macro::*;

#[derive(Debug)]
#[flow_node("change")]
struct ChangeNode {
    base: FlowNode,
    config: ChangeNodeConfig,
}

#[derive(Debug, Clone, Deserialize, PartialEq, PartialOrd)]
enum RuleKind {
    #[serde(rename = "set")]
    Set,

    #[serde(rename = "change")]
    Change,

    #[serde(rename = "delete")]
    Delete,

    #[serde(rename = "move")]
    Move,
}

#[derive(Debug, Clone, Deserialize)]
struct Rule {
    pub t: RuleKind,

    pub p: String,
    pub pt: RedPropertyType,

    #[serde(default)]
    pub to: Option<String>,

    #[serde(default)]
    pub tot: Option<RedPropertyType>,

    #[serde(default)]
    pub from: Option<String>,

    #[serde(default)]
    pub fromt: Option<RedPropertyType>,
    /*
    #[serde(default, rename = "dc")]
    pub deep_clone: bool,
    */
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct ChangeNodeConfig {
    #[serde(default)]
    rules: Vec<Rule>,
}

#[async_trait]
impl FlowNodeBehavior for ChangeNode {
    fn get_node(&self) -> &FlowNode {
        &self.base
    }

    async fn run(self: Arc<Self>, stop_token: CancellationToken) {
        while !stop_token.is_cancelled() {
            let cancel = stop_token.clone();
            with_uow(self.as_ref(), cancel.child_token(), |node, msg| async move {
                {
                    let mut msg_guard = msg.write().await;
                    // We always relay the message, regardless of whether the rules are followed or not.
                    if let Err(e) = node.apply_rules(&mut msg_guard).await {
                        // TODO Report Error to flow
                        log::error!("Failed to apply rules: {}", e);
                    }
                }
                node.fan_out_one(&Envelope { port: 0, msg }, cancel.clone()).await
            })
            .await;
        }
    }
}

impl ChangeNode {
    fn build(_flow: &Flow, state: FlowNode, config: &RedFlowNodeConfig) -> crate::Result<Box<dyn FlowNodeBehavior>> {
        let json = handle_legacy_json(config.json.clone())?;
        let change_config = ChangeNodeConfig::deserialize(&json)?;
        let node = ChangeNode { base: state, config: change_config };
        Ok(Box::new(node))
    }

    async fn get_to_value(&self, rule: &Rule, msg: &Msg) -> crate::Result<Variant> {
        if let (Some(tot), Some(to)) = (rule.tot, rule.to.as_ref()) {
            eval::evaluate_node_property(to, tot, Some(self), None, Some(msg)).await
        } else {
            Err(EdgelinkError::BadFlowsJson("The `tot` and `to` in the rule cannot be None".into()).into())
        }
    }

    async fn get_from_value(&self, rule: &Rule, msg: &Msg) -> crate::Result<Variant> {
        if let (Some(fromt), Some(from)) = (rule.fromt, rule.from.as_ref()) {
            eval::evaluate_node_property(from, fromt, Some(self), None, Some(msg)).await
        } else {
            Err(EdgelinkError::BadFlowsJson("The `fromt` and `from` in the rule cannot be None".into()).into())
        }
    }

    async fn apply_rules(&self, msg: &mut Msg) -> crate::Result<()> {
        for rule in self.config.rules.iter() {
            self.apply_rule(rule, msg).await?;
        }
        Ok(())
    }

    async fn apply_rule(&self, rule: &Rule, msg: &mut Msg) -> crate::Result<()> {
        let to_value = self.get_to_value(rule, msg).await.ok();
        match rule.t {
            RuleKind::Set => self.apply_rule_set(rule, msg, to_value).await,
            RuleKind::Change => self.apply_rule_change(rule, msg, to_value).await,
            RuleKind::Delete => {
                let _ = self.apply_rule_delete(rule, msg).await?;
                Ok(())
            }
            RuleKind::Move => Ok(()),
        }
    }

    async fn apply_rule_set(&self, rule: &Rule, msg: &mut Msg, to_value: Option<Variant>) -> crate::Result<()> {
        assert!(rule.t == RuleKind::Set);
        match rule.pt {
            RedPropertyType::Msg => {
                if let Some(to_value) = to_value {
                    msg.set_trimmed_nav_property(&rule.p, to_value, true)?;
                } else {
                    // Equals the `undefined` in JS
                    if msg.body.contains_key(&rule.p) {
                        // TODO remove by propex
                        msg.body.remove(&rule.p);
                    }
                }
                Ok(())
            }
            RedPropertyType::Global => {
                if let Some(to_value) = to_value {
                    let engine = self.get_flow().upgrade().and_then(|flow| flow.engine.upgrade()).unwrap(); // FIXME TODO
                                                                                                            // let csp = context::parse_context_store(&rule.p)?;
                                                                                                            // engine.get_context().set_one("memory", csp.key, to_value).await
                    engine.get_context().set_one("memory", &rule.p, to_value).await
                } else {
                    Err(EdgelinkError::BadArguments("The target value is None".into()).into())
                }
            }
            RedPropertyType::Flow => {
                if let Some(to_value) = to_value {
                    let flow = self.get_flow().upgrade().unwrap(); // FIXME TODO
                                                                   // let csp = context::parse_context_store(&rule.p)?;
                                                                   // engine.get_context().set_one("memory", csp.key, to_value).await
                    let fe = flow as Arc<dyn FlowsElement>;
                    fe.context().set_one("memory", &rule.p, to_value).await
                } else {
                    Err(EdgelinkError::BadArguments("The target value is None".into()).into())
                }
            }
            _ => Err(EdgelinkError::NotSupported(
                "We only support to set message property and flow/global context variables".into(),
            )
            .into()),
        }
    }

    async fn apply_rule_change(&self, rule: &Rule, msg: &mut Msg, to_value: Option<Variant>) -> crate::Result<()> {
        assert!(rule.t == RuleKind::Change);
        match rule.pt {
            RedPropertyType::Msg => {
                if let (Some(to_value), Ok(from_value), Ok(current)) = (
                    to_value,
                    self.get_from_value(rule, msg).await,
                    eval::evaluate_node_property(&rule.p, rule.pt, Some(self), None, Some(msg)).await,
                ) {
                    match current {
                        Variant::String(ref cs) => match from_value {
                            Variant::Integer(_) | Variant::Rational(_) | Variant::Bool(_) | Variant::String(_)
                                if current == from_value =>
                            {
                                // str representation of exact from number/boolean
                                // only replace if they match exactly
                                msg.set_trimmed_nav_property(&rule.p, to_value, false)?;
                            }
                            Variant::Regexp(ref from_value_re) => {
                                let replaced = from_value_re.replace_all(cs, to_value.to_string()?.as_str());
                                let value_to_set = match (rule.tot, replaced.as_ref()) {
                                    (Some(RedPropertyType::Bool), "true") => to_value,
                                    (Some(RedPropertyType::Bool), "false") => to_value,
                                    _ => Variant::String(replaced.into()),
                                };
                                msg.set_trimmed_nav_property(&rule.p, value_to_set, false)?;
                            }
                            _ => {
                                let replaced = cs.replace(
                                    from_value.to_string()?.as_str(), //TODO opti
                                    to_value.to_string()?.as_str(),
                                );
                                if rule.tot == Some(RedPropertyType::Bool) && current == to_value {
                                    // If the target type is boolean, and the replace call has resulted in "true"/"false",
                                    // convert to boolean type (which 'value' already is)
                                    msg.set_trimmed_nav_property(&rule.p, to_value, false)?;
                                } else {
                                    msg.set_trimmed_nav_property(&rule.p, Variant::String(replaced), false)?;
                                }
                            }
                        },
                        _ => todo!(),
                    }
                } else {
                    // Equals the `undefined` in JS
                    if msg.body.contains_key(&rule.p) {
                        // TODO remove by propex
                        msg.body.remove(&rule.p);
                    }
                }
                Ok(())
            }
            RedPropertyType::Flow | RedPropertyType::Global => {
                //
                todo!()
            }
            _ => Err(EdgelinkError::NotSupported(
                "We only support to set message property and flow/global context variables".into(),
            )
            .into()),
        }
    } // apply_rule_change

    async fn apply_rule_delete(&self, rule: &Rule, msg: &mut Msg) -> crate::Result<Variant> {
        assert!(rule.t == RuleKind::Delete);
        match rule.pt {
            RedPropertyType::Msg => msg.body.remove_nav_property(&rule.p).ok_or(
                EdgelinkError::NotSupported(format!("Cannot remove the property '{}' in the msg", rule.p)).into(),
            ),
            _ => Err(EdgelinkError::NotSupported("Only support to remove message property".into()).into()),
        }
    } // apply_rule_delete
}

fn handle_legacy_json(n: Value) -> crate::Result<Value> {
    let mut rules: Vec<Value> = if let Some(Value::Array(existed_rules)) = n.get("rules") {
        existed_rules.to_vec()
    } else {
        let mut rule = serde_json::json!({
            "t": if n["action"] == "replace" {
                "set"
            } else {
                n["action"].as_str().unwrap_or("")
            },
            "p": n["property"].as_str().unwrap_or("")
        });

        // Check if "set" or "move" action, and add "to" field
        if rule["t"] == "set" || rule["t"] == "move" {
            rule["to"] = n.get("to").cloned().unwrap_or(Value::String("".to_string()));
        }
        // If "change" action, add "from", "to" and "re" fields
        else if rule["t"] == "change" {
            rule["from"] = n.get("from").cloned().unwrap_or("".into());
            rule["to"] = n.get("to").cloned().unwrap_or("".into());
            rule["re"] = n.get("reg").cloned().unwrap_or(Value::Bool(true));
        }
        vec![rule]
    };

    for rule in rules.iter_mut() {
        // Migrate to type-aware rules
        if rule.get("pt").is_none() {
            rule["pt"] = "msg".into();
        }

        if let (Some("change"), Some(true)) =
            (rule.get("t").and_then(|t| t.as_str()), rule.get("re").and_then(|x| x.as_bool()))
        {
            rule["fromt"] = "re".into();
            rule.as_object_mut().unwrap().remove("re");
        }

        if let (Some("set"), None, Some(Value::String(to))) =
            (rule.get("t").and_then(|t| t.as_str()), rule.get("tot"), rule.get("to"))
        {
            if to.starts_with("msg.") {
                rule["to"] = to.trim_start_matches("msg.").into();
                rule["tot"] = "msg".into();
            }
        }

        if rule.get("tot").is_none() {
            rule["tot"] = "str".into();
        }

        if rule.get("fromt").is_none() {
            rule["fromt"] = "str".into();
        }

        /*
        if let Some("change") = rule.get("t").and_then(|t| t.as_str()) {
            let fromt = rule.get("fromt").and_then(|f| f.as_str());
            if fromt != Some("msg") && fromt != Some("flow") && fromt != Some("global") {
                {
                    // Simple copy as there's no regex
                    if let Some(from) = rule.get("from").cloned() {
                        rule["fromRE"] = from;
                    }
                }
                {
                    if fromt != Some("re") {
                        // Escaping meta characters
                        if let Some(from_value) = rule.get("from").and_then(|f| f.as_str()) {
                            let mut from = from_value.to_string(); // Copy the string

                            // Escape meta characters
                            for ch in &[
                                '-', '[', ']', '{', '}', '(', ')', '*', '+', '?', '.', '\\', '^',
                                '$', '|', '#', ' ',
                            ] {
                                from = from.replace(*ch, "\\");
                            }

                            rule["fromRE"] = from.into(); // Insert after all borrowing is done
                        }
                    }
                }
            }
        }
        */

        // `tot` handling
        /*
        match rule.get("tot").and_then(|t| t.as_str()) {
            Some("num") => {
                if let Some(to) = rule.get("to").and_then(|t| t.as_str()) {
                    rule["to"] = json!(to.parse::<f64>().unwrap_or(0.0));
                }
            }
            Some("json") | Some("bin") => {
                if rule.get("to").is_some() {
                    if serde_json::from_str::<Value>(rule.get("to").unwrap().as_str().unwrap_or(""))
                        .is_err()
                    {
                        return Err(EdgelinkError::BadFlowsJson("Invalid JSON".to_string()).into());
                    }
                }
            }
            Some("bool") => {
                if let Some(to) = rule.get("to").and_then(|t| t.as_str()) {
                    rule["to"] = json!(to.eq_ignore_ascii_case("true"));
                }
            }
            Some("jsonata") => {
                // Placeholder for expression preparation
                // rule["to"] = json!("prepared JSONata expression");
                return Err(EdgelinkError::NotSupported(
                    "We are not supported the JSONata at this moment!".to_string(),
                )
                .into());
            }
            Some("env") => {
                // Placeholder for environment evaluation
                // TODO FIXME
                rule["to"] = json!("evaluated environment variable");
            }
            _ => {}
        }
        */
    }

    let mut changed = n.clone();
    //rules = Value::Array(vec![rule]);
    changed["rules"] = Value::Array(rules);
    Ok(changed)
}
