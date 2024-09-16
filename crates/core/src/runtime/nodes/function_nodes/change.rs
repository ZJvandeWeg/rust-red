use std::sync::Arc;

use regex::Regex;
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

    #[serde(default, rename = "fromRE", with = "crate::text::regex::serde_optional_regex")]
    pub from_regex: Option<Regex>,
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
                    node.apply_rules(&mut msg_guard).await;
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

    async fn apply_rules(&self, msg: &mut Msg) {
        for rule in self.config.rules.iter() {
            if let Err(err) = self.apply_rule(rule, msg).await {
                log::warn!("Failed to apply rule: {}", err);
            }
        }
    }

    async fn apply_rule(&self, rule: &Rule, msg: &mut Msg) -> crate::Result<()> {
        let to_value = self.get_to_value(rule, msg).await.ok();
        match rule.t {
            RuleKind::Set => self.apply_rule_set(rule, msg, to_value).await,
            RuleKind::Change => self.apply_rule_change(rule, msg, to_value).await,
            RuleKind::Delete => {
                self.apply_rule_delete(rule, msg).await?;
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
                    msg.set_nav_stripped(&rule.p, to_value, true)?;
                } else {
                    // Equals the `undefined` in JS
                    if msg.contains(&rule.p) {
                        // TODO remove by propex
                        msg.remove(&rule.p);
                    }
                }
                Ok(())
            }

            RedPropertyType::Global => {
                if let Some(to_value) = to_value {
                    let engine = self.get_flow().upgrade().and_then(|flow| flow.engine.upgrade()).unwrap(); // FIXME TODO

                    let ctx_prop = crate::runtime::context::parse_store(&rule.p)?;
                    engine.get_context().set_one(&ctx_prop, Some(to_value)).await
                } else {
                    Err(EdgelinkError::BadArguments("The target value is None".into()).into())
                }
            }

            RedPropertyType::Flow => {
                if let Some(to_value) = to_value {
                    let flow = self.get_flow().upgrade().unwrap(); // FIXME TODO
                    let fe = flow as Arc<dyn FlowsElement>;
                    let ctx_prop = crate::runtime::context::parse_store(&rule.p)?;
                    fe.context().set_one(&ctx_prop, Some(to_value)).await
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

        let to_value = match to_value {
            None => return Ok(()),
            Some(v) => v,
        };

        let from_value = match self.get_from_value(rule, msg).await {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };

        let current = match eval::evaluate_node_property(&rule.p, rule.pt, Some(self), None, Some(msg)).await {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };

        /*
        let mut target_object = match rule.pt {
            RedPropertyType::Msg => msg.as_variant_object_mut(),
            RedPropertyType::Flow | RedPropertyType::Global => todo!(),
            _ => {
                return Err(EdgelinkError::NotSupported(
                    "The 'change' node only allows modifying the 'msg' and global/flow context properties".into(),
                )
                .into())
            }
        };
        */

        match (&current, rule.fromt.unwrap()) {
            //FIXME unwrap
            (Variant::String(_), RedPropertyType::Num | RedPropertyType::Bool | RedPropertyType::Str)
                if current == from_value =>
            {
                // str representation of exact from number/boolean
                // only replace if they match exactly
                msg.set_nav_stripped(&rule.p, to_value, false)?;
            }

            (Variant::String(ref cs), RedPropertyType::Re) => {
                let replaced = rule.from_regex.as_ref().unwrap().replace_all(cs, to_value.to_string()?.as_str());
                let value_to_set = match (rule.tot, replaced.as_ref()) {
                    (Some(RedPropertyType::Bool), "true") => to_value,
                    (Some(RedPropertyType::Bool), "false") => to_value,
                    _ => Variant::String(replaced.into()),
                };
                msg.set_nav_stripped(&rule.p, value_to_set, false)?;
            }

            (Variant::String(ref cs), _) => {
                // Otherwise we search and replace
                let replaced = cs.replace(
                    from_value.to_string()?.as_str(), //TODO opti
                    to_value.to_string()?.as_str(),
                );
                msg.set_nav_stripped(&rule.p, replaced.into(), false)?;
            }

            (Variant::Integer(_) | Variant::Rational(_), RedPropertyType::Num) if from_value == current => {
                msg.set_nav_stripped(&rule.p, to_value, false)?;
            }

            (Variant::Bool(_), RedPropertyType::Bool) if from_value == current => {
                msg.set_nav_stripped(&rule.p, to_value, false)?;
            }

            _ => {
                // no rule matched
            }
        }

        Ok(())
    }

    async fn apply_rule_delete(&self, rule: &Rule, msg: &mut Msg) -> crate::Result<()> {
        assert!(rule.t == RuleKind::Delete);
        match rule.pt {
            RedPropertyType::Msg => {
                let _ = msg.remove_nav(&rule.p).ok_or(EdgelinkError::NotSupported(format!(
                    "Cannot remove the property '{}' in the msg",
                    rule.p
                )))?;
                Ok(())
            }

            RedPropertyType::Global => {
                // FIXME TODO
                // let csp = context::parse_context_store(&rule.p)?;
                // engine.get_context().set_one("memory", csp.key, to_value).await
                let engine = self.get_flow().upgrade().and_then(|flow| flow.engine.upgrade()).unwrap();
                let ctx_prop = crate::runtime::context::parse_store(&rule.p)?;
                engine.get_context().set_one(&ctx_prop, None).await
                // Setting it to "None" means to delete.
            }

            RedPropertyType::Flow => {
                // FIXME TODO
                // let csp = context::parse_context_store(&rule.p)?;
                // engine.get_context().set_one("memory", csp.key, to_value).await
                let flow = self.get_flow().upgrade().unwrap();
                let fe = flow as Arc<dyn FlowsElement>;
                let ctx_prop = crate::runtime::context::parse_store(&rule.p)?;
                fe.context().set_one(&ctx_prop, None).await
                // Setting it to "None" means to delete.
            }

            _ => Err(EdgelinkError::NotSupported(
                "The 'change' node only allows deleting the 'msg' and global/flow context propertie".into(),
            )
            .into()),
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

    let old_from_re_pattern = regex::Regex::new(r"[-\[\]{}()*+?.,\\^$|#\s]")?;
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

        if let (Some(t), Some(fromt), Some(from)) = (rule.get("t"), rule.get("fromt"), rule.get("from")) {
            if t == "change" && fromt != "msg" && fromt != "flow" && fromt != "global" {
                let from_str = from.as_str().unwrap_or("");
                let mut from_re = from_str.to_string();

                if fromt != "re" {
                    from_re = old_from_re_pattern.replace_all(&from_re, r"\$&").to_string();
                }

                match regex::Regex::new(&from_re) {
                    Ok(re) => {
                        rule["fromRE"] = Value::String(re.as_str().to_string());
                    }
                    Err(e) => {
                        log::error!("Invalid regexp: {}", e);
                        return Err(e.into());
                    }
                }
            }
        }

        /*
        // Preprocess the constants:
        let tot = rule.get("tot").and_then(Value::as_str).unwrap_or("");

        match tot {
            "num" => {
                if let Some(to_value) = rule.get("to").and_then(Value::as_str) {
                    if let Ok(number) = to_value.parse::<f64>() {
                        rule["to"] = Value::from(number);
                    }
                }
            }
            "json" | "bin" => {
                if let Some(to_value) = rule.get("to").and_then(Value::as_str) {
                    // Check if the string is valid JSON
                    if from_str::<Value>(to_value).is_err() {
                        log::error!("Error: invalid JSON");
                    }
                }
            }
            "bool" => {
                if let Some(to_value) = rule.get("to").and_then(Value::as_str) {
                    let re = Regex::new(r"^true$").unwrap();
                    let is_true = re.is_match(to_value);
                    rule["to"] = Value::from(is_true);
                }
            }
            "jsonata" =>
            {
                if let Some(to_value) = rule.get("to").and_then(Value::as_str) {
                    // Assuming `prepare_jsonata_expression` is a custom function to handle JSONata
                    match prepare_jsonata_expression(to_value, node) {
                        Ok(expression) => {
                            rule["to"] = Value::from(expression);
                        }
                        Err(e) => {
                            valid = false;
                            println!("Error: invalid JSONata expression: {}", e);
                        }
                    }
                }
            }
            "env" => {
                if let Some(to_value) = rule.get("to").and_then(Value::as_str) {
                    // Assuming `evaluate_node_property` is a custom function to evaluate environment variables
                    let result = evaluate_node_property(to_value, "env", node);
                    rule["to"] = Value::from(result);
                }
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
