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
fn get_setting(
    name: &str,
    node: Option<&dyn FlowNodeBehavior>,
    flow: Option<&Flow>,
) -> Option<Variant> {
    if let Some(node) = node {
        match name {
            "NR_NODE_NAME" => return Some(Variant::String(node.name().into())),
            "NR_NODE_ID" => return Some(Variant::String(node.id().to_string())),
            "NR_NODE_PATH" => return None,
            &_ => (),
        };
    }

    if let Some(flow_ref) = flow {
        if let Some(node) = node {
            if let Some(group) = node.group().upgrade() {
                return group.get_setting(name);
            }
        }

        return flow_ref.get_setting(name);
    }

    // TODO FIXME
    // We should use the snapshot in the FlowEngine
    Some(
        std::env::var(name)
            .map(Variant::String)
            .unwrap_or(Variant::Null),
    )
}

/**
 * Checks if a String contains any Environment Variable specifiers and returns
 * it with their values substituted in place.
 *
 * For example, if the env var `WHO` is set to `Joe`, the string `Hello ${WHO}!`
 * will return `Hello Joe!`.
 * @param  {String} value - the string to parse
 * @param  {Node} node - the node evaluating the property
 * @return {String} The parsed string
 */
fn evaluate_env_property(value: &str, node: Option<&dyn FlowNodeBehavior>) -> Option<Variant> {
    let flow = node.and_then(|n| n.get_flow().upgrade());
    let flow_ref = flow.as_ref().map(|arc| arc.as_ref());
    let trimmed = value.trim();
    if trimmed.starts_with("${") && value.ends_with("}") {
        // ${ENV_VAR}
        let name = &trimmed[2..(value.len() - 1)];
        get_setting(name, node, flow_ref)
    } else if !trimmed.contains("${") {
        // ENV_VAR
        get_setting(trimmed, node, flow_ref)
    } else {
        // FOO${ENV_VAR}BAR
        Some(Variant::String(crate::runtime::model::replace_vars(
            trimmed,
            |env_name| match get_setting(env_name, node, flow_ref) {
                Some(Variant::String(v)) => v,
                _ => "".to_string(),
            },
        )))
    }
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
            let bytes = arr.to_bytes().ok_or(EdgelinkError::InvalidData(format!(
                "Expected an array of bytes, got: {:?}",
                value
            )))?;
            Ok(Variant::from(bytes))
        }

        RedPropertyType::Msg => {
            if let Some(msg) = msg {
                if let Some(pv) = msg.get_trimmed_nav_property(value) {
                    Ok(pv.clone())
                } else {
                    Err(EdgelinkError::BadArguments(format!(
                        "Cannot get the property(s) from `msg`: {}",
                        value
                    ))
                    .into())
                }
            } else {
                Err(EdgelinkError::BadArguments("`msg` is not existed!".to_string()).into())
            }
        }

        RedPropertyType::Flow | RedPropertyType::Global => todo!(),

        RedPropertyType::Bool => Ok(Variant::Bool(value.trim_ascii().parse::<bool>()?)),

        RedPropertyType::Jsonata => todo!(),

        RedPropertyType::Env => match evaluate_env_property(value, node) {
            Some(ev) => Ok(ev),
            _ => Err(EdgelinkError::InvalidData(format!(
                "Cannot found the environment variable: '{}'",
                value
            ))
            .into()),
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
            let bytes = arr.to_bytes().ok_or(EdgelinkError::InvalidData(format!(
                "Expected an array of bytes, got: {:?}",
                value
            )))?;
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

        (RedPropertyType::Bool, Variant::String(s)) => {
            Ok(Variant::Bool(s.trim_ascii().parse::<bool>()?))
        }
        (RedPropertyType::Bool, Variant::Bool(_)) => Ok(value.clone()), // TODO javascript rules

        (RedPropertyType::Jsonata, _) => todo!(),

        (RedPropertyType::Env, Variant::String(s)) => match evaluate_env_property(s, node) {
            Some(ev) => Ok(ev),
            _ => Err(EdgelinkError::InvalidData(format!(
                "Cannot found the environment variable: '{}'",
                s
            ))
            .into()),
        },

        (_, _) => Err(EdgelinkError::BadArguments(format!(
            "Unable to evaluate property value: {:?}",
            value
        ))
        .into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_evaluate_node_property_without_msg() {
        let triple = RedPropertyTriple {
            p: "payload".to_string(),
            vt: RedPropertyType::Num,
            v: "10".to_string(),
        };
        let evaluated = evaluate_node_property(&triple.v, triple.vt, None, None).unwrap();
        assert!(evaluated.is_integer());
        assert_eq!(evaluated.as_integer().unwrap(), 10);
    }
}
