use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet},
    sync::{Arc, RwLock, Weak},
};

use dashmap::DashMap;
use itertools::Itertools;
use nom;
use serde::Deserialize;
use serde_json::Value as JsonValue;

use crate::runtime::model::{RedPropertyType, Variant};
use crate::*;

#[derive(Debug)]
pub struct EnvStore {
    pub parent: RwLock<Option<Weak<EnvStore>>>,
    pub envs: DashMap<String, Variant>,
}

impl EnvStore {
    pub fn evalute_env(&self, env_expr: &str) -> Option<Variant> {
        self.get_normalized(env_expr)
    }

    pub fn get_env(&self, key: &str) -> Option<Variant> {
        if let Some(value) = self.envs.get(key) {
            Some(value.clone())
        } else {
            let parent = self.parent.read().ok()?;
            parent.as_ref().and_then(|p| p.upgrade()).and_then(|p| p.evalute_env(key))
        }
    }

    fn get_normalized(&self, env_expr: &str) -> Option<Variant> {
        let trimmed = env_expr.trim();
        if trimmed.starts_with("${") && env_expr.ends_with("}") {
            // ${ENV_VAR}
            let to_match = &trimmed[2..(env_expr.len() - 1)];
            self.get_env(to_match)
        } else if !trimmed.contains("${") {
            // ENV_VAR
            self.get_env(trimmed)
        } else {
            // FOO${ENV_VAR}BAR
            Some(Variant::String(replace_vars(trimmed, |env_name| match self.get_env(env_name) {
                Some(v) => v.to_string().unwrap(), // FIXME
                _ => "".to_string(),
            })))
        }
    }
}

#[derive(Debug, Clone, serde::Deserialize)]
struct EnvEntry {
    pub name: String,

    pub value: String,

    #[serde(alias = "type")]
    pub type_: RedPropertyType,
}

#[derive(Debug, Default)]
pub struct EnvStoreBuilder {
    parent: Option<Weak<EnvStore>>,
    envs: HashMap<String, Variant>,
}

impl EnvStoreBuilder {
    pub fn with_parent(mut self, parent: &Arc<EnvStore>) -> Self {
        self.parent = Some(Arc::downgrade(parent));
        self
    }

    pub fn load_json(mut self, jv: &JsonValue) -> Self {
        if let Ok(mut entries) = Vec::<EnvEntry>::deserialize(jv) {
            // Remove duplicated by name, only keep the last one
            entries = {
                let mut seen = HashSet::new();
                entries
                    .into_iter()
                    .rev()
                    .unique_by(|e| e.name.clone())
                    .filter(|e| seen.insert(e.name.clone()))
                    .collect::<Vec<_>>()
                    .into_iter()
                    .rev()
                    .collect()
            };

            // TODO: Maybe dependency sorting? The Node-RED didn't have it.
            entries.sort_by(|a, b| match (a.type_, b.type_) {
                (RedPropertyType::Env, RedPropertyType::Env) => Ordering::Equal,
                (RedPropertyType::Env, _) => Ordering::Less,
                (_, RedPropertyType::Env) => Ordering::Greater,
                _ => Ordering::Equal,
            });

            for e in entries.iter() {
                if let Ok(var) = self.evaluate(&e.value, e.type_) {
                    if !self.envs.contains_key(&e.name) {
                        self.envs.insert(e.name.clone(), var);
                    }
                } else {
                    log::warn!("Failed to evaluate environment variable property: {:?}", e);
                }
            }
        } else {
            log::warn!("Failed to parse environment variables: \n{}", serde_json::to_string_pretty(&jv).unwrap());
        }
        self
    }

    pub fn with_process_env(mut self) -> Self {
        for (k, v) in std::env::vars() {
            self.envs.insert(k, Variant::String(v));
        }
        self
    }

    pub fn extends<T: IntoIterator<Item = (String, Variant)>>(mut self, iter: T) -> Self {
        for (k, v) in iter {
            self.envs.insert(k, v);
        }
        self
    }

    pub fn build(self) -> Arc<EnvStore> {
        let mut this = EnvStore { parent: RwLock::new(self.parent), envs: DashMap::with_capacity(self.envs.len()) };
        this.envs.extend(self.envs);

        Arc::new(this)
    }

    fn evaluate(&self, value: &str, type_: RedPropertyType) -> crate::Result<Variant> {
        match type_ {
            RedPropertyType::Str => Ok(Variant::String(value.into())),

            RedPropertyType::Num | RedPropertyType::Json => {
                let jv: serde_json::Value = serde_json::from_str(value)?;
                Ok(Variant::deserialize(jv)?)
            }

            RedPropertyType::Bool => Ok(Variant::Bool(value.trim_ascii().parse::<bool>()?)),

            RedPropertyType::Bin => {
                let jv: serde_json::Value = serde_json::from_str(value)?;
                let arr = Variant::deserialize(&jv)?;
                let bytes = arr
                    .to_bytes()
                    .ok_or(EdgelinkError::BadArgument("value"))
                    .with_context(|| format!("Expected an array of bytes, got: {:?}", value))?;
                Ok(Variant::from(bytes))
            }

            RedPropertyType::Jsonata => todo!(),

            RedPropertyType::Env => match self.normalized_and_get_existed(value) {
                Some(ev) => Ok(ev),
                _ => Err(EdgelinkError::BadArgument("value"))
                    .with_context(|| format!("Cannot found the environment variable: '{}'", value)),
            },

            _ => Err(EdgelinkError::BadArgument("type_"))
                .with_context(|| format!("Unsupported environment varibale type: '{}'", value)),
        }
    }

    fn get_existed(&self, env: &str) -> Option<Variant> {
        if let Some(value) = self.envs.get(env) {
            Some(value.clone())
        } else {
            self.parent.as_ref().and_then(|p| p.upgrade()).and_then(|p| p.evalute_env(env))
        }
    }

    fn normalized_and_get_existed(&self, value: &str) -> Option<Variant> {
        let trimmed = value.trim();
        if trimmed.starts_with("${") && value.ends_with("}") {
            // ${ENV_VAR}
            let to_match = &trimmed[2..(value.len() - 1)];
            self.get_existed(to_match)
        } else if !trimmed.contains("${") {
            // ENV_VAR
            self.get_existed(trimmed)
        } else {
            // FOO${ENV_VAR}BAR
            Some(Variant::String(replace_vars(trimmed, |env_name| {
                match self.get_existed(env_name) {
                    Some(v) => v.to_string().unwrap(), // FIXME
                    _ => "".to_string(),
                }
            })))
        }
    }
}

pub fn replace_vars<'a, F, R>(input: &'a str, converter: F) -> String
where
    F: Fn(&'a str) -> R,
    R: AsRef<str>,
{
    fn variable_name(input: &str) -> nom::IResult<&str, &str> {
        nom::sequence::delimited(
            nom::bytes::complete::tag("${"), // Starts with "${"
            nom::sequence::preceded(
                nom::character::complete::space0,
                nom::bytes::complete::take_while(|c: char| c.is_alphanumeric() || c == '_'),
            ),
            nom::sequence::preceded(nom::character::complete::space0, nom::bytes::complete::tag("}")), // Ends with "}"
        )(input)
    }

    let mut output = input.to_string();
    let mut remaining_input = input;

    // Continue the parsing until it end
    while let Ok((remaining, var)) = variable_name(remaining_input) {
        let replacement = converter(var);
        output = output.replace(&format!("${{{}}}", var.trim()), replacement.as_ref());
        remaining_input = remaining;
    }

    output
}

pub fn parse_complex_env(expr: &str) -> Option<&str> {
    match parse_complex_env_internal(expr) {
        Ok((_, x)) => Some(x),
        Err(_) => None,
    }
}

fn parse_complex_env_internal(input: &str) -> nom::IResult<&str, &str> {
    nom::sequence::delimited(
        nom::bytes::complete::tag("${"),
        nom::sequence::delimited(
            nom::character::complete::multispace0,
            nom::bytes::complete::take_while(|c: char| c.is_alphanumeric() || c == '_'),
            nom::character::complete::multispace0,
        ),
        nom::bytes::complete::tag("}"),
    )(input)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::EnvStoreBuilder;
    use crate::runtime::model::*;

    #[test]
    fn test_env_store_builder() {
        let json = json!([
            {
                "name": "FOO",
                "value": "foofoo",
                "type": "str"
            },
            {
                "name": "AGE",
                "value": "41",
                "type": "num"
            },
        ]);
        let global =
            EnvStoreBuilder::default().load_json(&json).extends([("FILE_SIZE".into(), Variant::from(123))]).build();
        assert_eq!(global.evalute_env("FOO").unwrap().as_str().unwrap(), "foofoo");
        assert_eq!(global.evalute_env("AGE").unwrap().as_i64().unwrap(), 41);

        let json = json!([
            {
                "name": "BAR",
                "value": "barbar",
                "type": "str"
            },
        ]);
        let flow = EnvStoreBuilder::default().with_parent(&global).load_json(&json).build();

        let json = json!([
            {
                "name": "MY_FOO",
                "value": "aaa",
                "type": "str"
            },
            {
                "name": "GLOBAL_FOO",
                "value": "FOO",
                "type": "env"
            },
            {
                "name": "PARENT_BAR",
                "value": "BAR",
                "type": "env"
            },
            {
                "name": "AGE",
                "value": "100",
                "type": "str"
            }
        ]);
        let node = EnvStoreBuilder::default().with_parent(&flow).load_json(&json).build();
        assert_eq!(node.evalute_env("MY_FOO").unwrap().as_str().unwrap(), "aaa");
        assert_eq!(node.evalute_env("${MY_FOO}").unwrap().as_str().unwrap(), "aaa");
        assert_eq!(node.evalute_env("GLOBAL_FOO").unwrap().as_str().unwrap(), "foofoo");
        assert_eq!(node.evalute_env("PARENT_BAR").unwrap().as_str().unwrap(), "barbar");
        assert_eq!(node.evalute_env("AGE").unwrap().as_str().unwrap(), "100");
        assert_eq!(node.evalute_env("FILE_SIZE").unwrap().as_i64().unwrap(), 123);
    }
}
