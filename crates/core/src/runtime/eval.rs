use std::borrow::Cow;
use std::sync::Arc;

use regex::Regex;
use serde::Deserialize;

use crate::runtime::flow::*;
use crate::runtime::model::*;
use crate::runtime::nodes::*;
use crate::utils;
use crate::*;

/// Get value of environment variable.
fn evaluate_env_property(name: &str, node: Option<&dyn FlowNodeBehavior>, flow: Option<&Arc<Flow>>) -> Option<Variant> {
    if let Some(node) = node {
        if let Some(var) = node.get_env(name) {
            return Some(var);
        }
    }

    if let Some(flow_ref) = flow {
        if let Some(node) = node {
            if let Some(ref group) = node.group().clone().and_then(|g| g.upgrade()) {
                return group.get_env(name);
            }
        }

        return flow_ref.get_env(name);
    }

    // TODO FIXME
    // We should use the snapshot in the FlowEngine
    Some(std::env::var(name).map(Variant::String).unwrap_or(Variant::Null))
}

/// Evaluates a property value according to its type.
///
/// # Arguments
///
/// * `value`       - the raw value
///
/// # Returns
/// The evaluated result
pub async fn evaluate_node_property(
    value: &str,
    _type: RedPropertyType,
    node: Option<&dyn FlowNodeBehavior>,
    flow: Option<&Arc<Flow>>,
    msg: Option<&Msg>,
) -> crate::Result<Variant> {
    match _type {
        RedPropertyType::Str => Ok(Variant::String(value.into())),

        RedPropertyType::Num | RedPropertyType::Json => {
            let jv: serde_json::Value = serde_json::from_str(value)?;
            Ok(Variant::deserialize(jv)?)
        }

        RedPropertyType::Re => Ok(Variant::Regexp(Regex::new(value)?)),

        RedPropertyType::Date => match value {
            "object" => Ok(Variant::now()),
            "iso" => Ok(Variant::String(utils::time::iso_now())),
            _ => Ok(Variant::Number(utils::time::unix_now().into())),
        },

        RedPropertyType::Bin => {
            let jv: serde_json::Value = serde_json::from_str(value)?;
            let arr = Variant::deserialize(&jv)?;
            let bytes = arr
                .to_bytes()
                .ok_or(EdgelinkError::InvalidData(format!("Expected an array of bytes, got: {:?}", value)))?;
            Ok(Variant::from(bytes))
        }

        RedPropertyType::Msg => {
            if let Some(msg) = msg {
                if let Some(pv) = msg.get_nav_stripped(value) {
                    Ok(pv.clone())
                } else {
                    Err(EdgelinkError::BadArguments(format!("Cannot get the property(s) from `msg`: {}", value)).into())
                }
            } else {
                Err(EdgelinkError::BadArguments("`msg` is not existed!".to_string()).into())
            }
        }

        RedPropertyType::Global => {
            /*
            let csp = context::parse_context_store(value)?;
            // TODO normalize propex
            let engine = node.and_then(|n| n.get_engine()).or(flow.and_then(|f| f.engine.upgrade())).unwrap();
            if let Some(ctx_value) = engine.get_context().get_one(csp.store, csp.key).await {
                Ok(ctx_value)
            } else {
                Err(EdgelinkError::OutOfRange.into())
            }
            */
            let engine = node.and_then(|n| n.get_engine()).or(flow.and_then(|f| f.engine.upgrade())).unwrap();
            let ctx_prop = crate::runtime::context::parse_store(value)?;
            if let Some(ctx_value) = engine.get_context().get_one(&ctx_prop).await {
                Ok(ctx_value)
            } else {
                Err(EdgelinkError::OutOfRange.into())
            }
        }

        RedPropertyType::Flow => {
            let flow = node.and_then(|n| n.get_flow().upgrade()).unwrap();
            let fe = flow as Arc<dyn FlowsElement>;
            let ctx_prop = crate::runtime::context::parse_store(value)?;
            if let Some(ctx_value) = fe.context().get_one(&ctx_prop).await {
                Ok(ctx_value)
            } else {
                Err(EdgelinkError::OutOfRange.into())
            }
        }

        RedPropertyType::Bool => Ok(Variant::Bool(value.trim_ascii().parse::<bool>()?)),

        RedPropertyType::Jsonata => todo!(),

        RedPropertyType::Env => match evaluate_env_property(value, node, flow) {
            Some(ev) => Ok(ev),
            _ => Err(EdgelinkError::InvalidData(format!("Cannot found the environment variable: '{}'", value)).into()),
        },
    }
}

/// Evaluates a property variant according to its type.
pub fn evaluate_node_property_variant<'a>(
    value: &'a Variant,
    type_: &'a RedPropertyType,
    node: Option<&'a dyn FlowNodeBehavior>,
    flow: Option<&'a Arc<Flow>>,
    msg: Option<&'a Msg>,
) -> crate::Result<Cow<'a, Variant>> {
    let res = match (type_, value) {
        (RedPropertyType::Str, Variant::String(_)) => Cow::Borrowed(value),
        (RedPropertyType::Re, Variant::Regexp(_)) => Cow::Borrowed(value),
        (RedPropertyType::Num, Variant::Number(_)) => Cow::Borrowed(value),
        (RedPropertyType::Bool, Variant::Bool(_)) => Cow::Borrowed(value),
        (RedPropertyType::Bin, Variant::Bytes(_)) => Cow::Borrowed(value),
        (RedPropertyType::Date, Variant::Date(_)) => Cow::Borrowed(value),
        (RedPropertyType::Json, Variant::Object(_) | Variant::Array(_)) => Cow::Borrowed(value),

        (RedPropertyType::Bin, Variant::Array(array)) => Cow::Owned(Variant::bytes_from_vec(array)?),

        (RedPropertyType::Num | RedPropertyType::Json, Variant::String(s)) => {
            let jv: serde_json::Value = serde_json::from_str(s)?;
            Cow::Owned(Variant::deserialize(jv)?)
        }

        (RedPropertyType::Re, Variant::String(re)) => Cow::Owned(Variant::Regexp(Regex::new(re)?)),

        (RedPropertyType::Date, Variant::String(s)) => match s.as_str() {
            "object" => Cow::Owned(Variant::now()),
            "iso" => Cow::Owned(Variant::String(utils::time::iso_now())),
            _ => Cow::Owned(Variant::Number(utils::time::unix_now().into())),
        },

        (RedPropertyType::Bin, Variant::String(s)) => {
            let jv: serde_json::Value = serde_json::from_str(s.as_str())?;
            let arr = Variant::deserialize(&jv)?;
            let bytes = arr
                .to_bytes()
                .ok_or(EdgelinkError::InvalidData(format!("Expected an array of bytes, got: {:?}", value)))?;
            Cow::Owned(Variant::from(bytes))
        }

        (RedPropertyType::Msg, Variant::String(prop)) => {
            if let Some(msg) = msg {
                if let Some(pv) = msg.get_nav_stripped(prop.as_str()) {
                    Cow::Owned(pv.clone())
                } else {
                    return Err(EdgelinkError::BadArguments(format!(
                        "Cannot get the property(s) from `msg`: {}",
                        prop.as_str()
                    ))
                    .into());
                }
            } else {
                return Err(EdgelinkError::BadArguments("`msg` is not existed!".to_string()).into());
            }
        }

        // process the context variables
        (RedPropertyType::Flow | RedPropertyType::Global, _) => todo!(),

        (RedPropertyType::Bool, Variant::String(s)) => Cow::Owned(Variant::Bool(s.trim_ascii().parse::<bool>()?)),

        (RedPropertyType::Jsonata, _) => todo!(),

        (RedPropertyType::Env, Variant::String(s)) => match evaluate_env_property(s, node, flow) {
            Some(ev) => Cow::Owned(ev),
            _ => {
                return Err(
                    EdgelinkError::InvalidData(format!("Cannot found the environment variable: '{}'", s)).into()
                );
            }
        },

        (_, _) => {
            return Err(EdgelinkError::BadArguments(format!("Unable to evaluate property value: {:?}", value)).into());
        }
    };

    Ok(res)
}

#[cfg(test)]
mod tests {
    // use super::*;
}
