use std::sync::Arc;

use serde::Deserialize;
use serde_json::Value;

use crate::red::eval::evaluate_node_property;
use crate::red::RedPropertyType;
use crate::runtime::flow::Flow;
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

    fn as_any(&self) -> &dyn ::std::any::Any {
        self
    }

    async fn run(self: Arc<Self>, stop_token: CancellationToken) {
        while !stop_token.is_cancelled() {
            let cancel = stop_token.clone();
            with_uow(
                self.as_ref(),
                cancel.child_token(),
                |node, msg| async move {
                    {
                        let mut msg_guard = msg.write().await;
                        node.apply_rules(&mut msg_guard)?;
                    }
                    node.fan_out_one(&Envelope { port: 0, msg }, cancel.clone())
                        .await
                },
            )
            .await;
        }
    }
}

impl ChangeNode {
    fn build(
        _flow: &Flow,
        state: FlowNode,
        config: &RedFlowNodeConfig,
    ) -> crate::Result<Box<dyn FlowNodeBehavior>> {
        let json = handle_legacy_json(config.json.clone())?;
        let change_config = ChangeNodeConfig::deserialize(&json)?;
        let node = ChangeNode {
            base: state,
            config: change_config,
        };
        Ok(Box::new(node))
    }

    fn get_to_value(&self, rule: &Rule, msg: &Msg) -> crate::Result<Variant> {
        if let (Some(tot), Some(to)) = (rule.tot.as_ref(), rule.to.as_ref()) {
            evaluate_node_property(to, tot, Some(self), Some(msg))
        } else {
            Err(
                EdgelinkError::BadFlowsJson("The `tot` and `to` in the rule cannot be None".into())
                    .into(),
            )
        }
    }

    fn get_from_value(&self, rule: &Rule, msg: &Msg) -> crate::Result<Variant> {
        if let (Some(fromt), Some(from)) = (rule.fromt.as_ref(), rule.from.as_ref()) {
            evaluate_node_property(from, fromt, Some(self), Some(msg))
        } else {
            Err(EdgelinkError::BadFlowsJson(
                "The `fromt` and `from` in the rule cannot be None".into(),
            )
            .into())
        }
    }

    fn apply_rules(&self, msg: &mut Msg) -> crate::Result<()> {
        for rule in self.config.rules.iter() {
            self.apply_rule(rule, msg)?;
        }
        Ok(())
    }

    fn apply_rule(&self, rule: &Rule, msg: &mut Msg) -> crate::Result<()> {
        match rule.t {
            RuleKind::Set => self.apply_set_rule(rule, msg),
            RuleKind::Change => Ok(()),
            RuleKind::Move => Ok(()),
            RuleKind::Delete => {
                /*
                msg.get_property_mut(, value, create_missing)
                msg.body.remove().with_context(|| format!())?
                */
                Ok(())
            }
        }
    }

    fn apply_set_rule(&self, rule: &Rule, msg: &mut Msg) -> crate::Result<()> {
        assert!(rule.t == RuleKind::Set);
        match rule.pt {
            RedPropertyType::Msg => {
                let to_value = self.get_to_value(rule, msg)?;
                msg.set_trimmed_nav_property(&rule.p, to_value, true)?;
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
    }
}

fn handle_legacy_json(n: Value) -> crate::Result<Value> {
    let mut rules: Vec<Value> = if let Some(Value::Array(existed_rules)) = n.get("rules") {
        existed_rules.iter().cloned().collect()
    } else {
        let mut rule = Value::Object(serde_json::Map::new());

        let action = n.get("action").and_then(Value::as_str).unwrap_or("");
        let property = n.get("property").and_then(Value::as_str).unwrap_or("");

        rule["t"] = match action {
            "replace" => Value::String("set".to_string()),
            _ => Value::String(action.to_string()),
        };

        rule["p"] = Value::String(property.to_string());

        match rule["t"].as_str().unwrap_or("") {
            "set" | "move" => {
                let to = n.get("to").and_then(Value::as_str).unwrap_or("");
                rule["to"] = Value::String(to.to_string());
            }
            "change" => {
                let from = n.get("from").and_then(Value::as_str).unwrap_or("");
                let to = n.get("to").and_then(Value::as_str).unwrap_or("");
                let reg = n.get("reg").and_then(Value::as_bool).unwrap_or(false);

                rule["from"] = Value::String(from.to_string());
                rule["to"] = Value::String(to.to_string());
                rule["re"] = Value::Bool(reg);
            }
            _ => {}
        }
        vec![rule]
    };

    for rule in rules.iter_mut() {
        // Migrate to type-aware rules
        if rule.get("pt").is_none() {
            rule["pt"] = "msg".into();
        }

        if let (Some("change"), Some(_)) = (rule.get("t").and_then(|t| t.as_str()), rule.get("re"))
        {
            rule["fromt"] = "re".into();
            rule.as_object_mut().unwrap().remove("re");
        }

        if let (Some("set"), None, Some(Value::String(to))) = (
            rule.get("t").and_then(|t| t.as_str()),
            rule.get("tot"),
            rule.get("to"),
        ) {
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
