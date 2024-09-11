use serde::Deserialize;

use crate::runtime::flow::*;
use crate::runtime::model::*;
use crate::runtime::nodes::*;
use crate::utils;
use crate::*;

/**
 * Get value of environment variable.
 * @param {Node} node - accessing node
 * @param {String} name - name of variable
 * @return {String} value of env var
 */
fn evaluate_env_property(name: &str, node: Option<&dyn FlowNodeBehavior>, flow: Option<&Flow>) -> Option<Variant> {
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

/**
 * Evaluates a property value according to its type.
 *
 * @param  {String}   value    - the raw value
 * @param  {String}   _type     - the type of the value
 * @param  {Node}     node     - the node evaluating the property
 * @param  {Object}   msg      - the message object to evaluate against
 * @param  {Function} callback - (optional) called when the property is evaluated
 * @return {any} The evaluted property, if no `callback` is provided
 */
pub fn evaluate_node_property(
    value: &str,
    _type: RedPropertyType,
    node: Option<&dyn FlowNodeBehavior>,
    flow: Option<&Flow>,
    msg: Option<&Msg>,
) -> crate::Result<Variant> {
    match _type {
        RedPropertyType::Str => Ok(Variant::String(value.into())),

        RedPropertyType::Num | RedPropertyType::Json => {
            let jv: serde_json::Value = serde_json::from_str(value)?;
            Ok(Variant::deserialize(jv)?)
        }

        RedPropertyType::Re => Ok(Variant::Regexp(value.into())),

        RedPropertyType::Date => match value {
            "object" => Ok(Variant::now()),
            "iso" => Ok(Variant::String(utils::time::iso_now())),
            _ => Ok(Variant::Rational(utils::time::unix_now() as f64)),
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
                if let Some(pv) = msg.get_trimmed_nav_property(value) {
                    Ok(pv.clone())
                } else {
                    Err(EdgelinkError::BadArguments(format!("Cannot get the property(s) from `msg`: {}", value)).into())
                }
            } else {
                Err(EdgelinkError::BadArguments("`msg` is not existed!".to_string()).into())
            }
        }

        RedPropertyType::Flow | RedPropertyType::Global => todo!(),

        RedPropertyType::Bool => Ok(Variant::Bool(value.trim_ascii().parse::<bool>()?)),

        RedPropertyType::Jsonata => todo!(),

        RedPropertyType::Env => match evaluate_env_property(value, node, flow) {
            Some(ev) => Ok(ev),
            _ => Err(EdgelinkError::InvalidData(format!("Cannot found the environment variable: '{}'", value)).into()),
        },
    }
}

/**
 * Evaluates a property variant according to its type.
 *
 * @param  {Variant}   value    - the raw variant
 * @param  {String}   _type     - the type of the value
 * @param  {Node}     node     - the node evaluating the property
 * @param  {Object}   msg      - the message object to evaluate against
 * @param  {Function} callback - (optional) called when the property is evaluated
 * @return {any} The evaluted property, if no `callback` is provided
 */
pub fn evaluate_node_property_variant(
    value: &Variant,
    type_: &RedPropertyType,
    node: Option<&dyn FlowNodeBehavior>,
    flow: Option<&Flow>,
    msg: Option<&Msg>,
) -> crate::Result<Variant> {
    match (type_, value) {
        (RedPropertyType::Str, Variant::String(_)) => Ok(value.clone()),
        (RedPropertyType::Str, _) => Ok(Variant::String(value.to_string()?)),

        (RedPropertyType::Num | RedPropertyType::Json, Variant::String(s)) => {
            let jv: serde_json::Value = serde_json::from_str(s)?;
            Ok(Variant::deserialize(jv)?)
        }

        (RedPropertyType::Re, _) => todo!(), // TODO FIXME

        (RedPropertyType::Date, Variant::String(s)) => match s.as_str() {
            "object" => Ok(Variant::now()),
            "iso" => Ok(Variant::String(utils::time::iso_now())),
            _ => Ok(Variant::Rational(utils::time::unix_now() as f64)),
        },

        (RedPropertyType::Bin, Variant::String(s)) => {
            let jv: serde_json::Value = serde_json::from_str(s.as_str())?;
            let arr = Variant::deserialize(&jv)?;
            let bytes = arr
                .to_bytes()
                .ok_or(EdgelinkError::InvalidData(format!("Expected an array of bytes, got: {:?}", value)))?;
            Ok(Variant::from(bytes))
        }
        (RedPropertyType::Bin, Variant::Bytes(_)) => Ok(value.clone()),
        (RedPropertyType::Bin, Variant::Array(array)) => Variant::bytes_from_vec(array),

        (RedPropertyType::Msg, Variant::String(prop)) => {
            if let Some(msg) = msg {
                if let Some(pv) = msg.get_trimmed_nav_property(prop.as_str()) {
                    Ok(pv.clone())
                } else {
                    Err(EdgelinkError::BadArguments(format!(
                        "Cannot get the property(s) from `msg`: {}",
                        prop.as_str()
                    ))
                    .into())
                }
            } else {
                Err(EdgelinkError::BadArguments("`msg` is not existed!".to_string()).into())
            }
        }

        // process the context variables
        (RedPropertyType::Flow | RedPropertyType::Global, _) => todo!(),

        (RedPropertyType::Bool, Variant::String(s)) => Ok(Variant::Bool(s.trim_ascii().parse::<bool>()?)),
        (RedPropertyType::Bool, Variant::Bool(_)) => Ok(value.clone()), // TODO javascript rules

        (RedPropertyType::Jsonata, _) => todo!(),

        (RedPropertyType::Env, Variant::String(s)) => match evaluate_env_property(s, node, flow) {
            Some(ev) => Ok(ev),
            _ => Err(EdgelinkError::InvalidData(format!("Cannot found the environment variable: '{}'", s)).into()),
        },

        (_, _) => Err(EdgelinkError::BadArguments(format!("Unable to evaluate property value: {:?}", value)).into()),
    }
}

#[cfg(test)]
mod tests {
    // use super::*;
}
