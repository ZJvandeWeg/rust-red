use crate::{runtime::model::*, EdgeLinkError};
use serde_json::Value as JsonValue;

pub mod deser;

pub struct RedTypeValue<'a> {
    red_type: &'a str,
    id: Option<ElementId>,
}

#[derive(serde::Deserialize, Debug, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub struct RedPortConfig {
    pub node_ids: Vec<ElementId>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct RedEnvEntry {
    pub name: String,

    pub value: String,

    #[serde(alias = "type")]
    pub type_name: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct RedGroupConfig {
    #[serde(deserialize_with = "deser::deser_red_id")]
    pub id: ElementId,

    #[serde(default)]
    pub name: String,

    #[serde(default, deserialize_with = "deser::deser_red_id_vec")]
    pub nodes: Vec<ElementId>,

    #[serde(deserialize_with = "deser::deser_red_id")]
    pub z: ElementId,

    #[serde(default, deserialize_with = "deser::deser_red_optional_id")]
    pub g: Option<ElementId>,

    #[serde(default)]
    pub env: Vec<RedEnvEntry>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct RedFlowConfig {
    #[serde(default)]
    pub disabled: bool,

    #[serde(deserialize_with = "deser::deser_red_id")]
    pub id: ElementId,

    #[serde(default)]
    pub info: String,

    #[serde(default)]
    pub label: String,

    #[serde(alias = "type")]
    pub type_name: String,

    #[serde(default)]
    pub env: Vec<RedEnvEntry>,

    #[serde(skip)]
    pub json: serde_json::Map<String, JsonValue>,

    #[serde(skip)]
    pub nodes: Vec<RedFlowNodeConfig>,

    #[serde(skip)]
    pub groups: Vec<RedGroupConfig>,

    #[serde(default, alias = "in")]
    pub in_ports: Vec<RedSubflowPort>,

    #[serde(default, alias = "out")]
    pub out_ports: Vec<RedSubflowPort>,

    #[serde(skip)]
    pub subflow_node_id: Option<ElementId>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct RedFlowNodeConfig {
    #[serde(deserialize_with = "deser::deser_red_id")]
    pub id: ElementId,

    #[serde(alias = "type")]
    pub type_name: String,

    #[serde(default)]
    pub name: String,

    #[serde(deserialize_with = "deser::deser_red_id")]
    pub z: ElementId,

    #[serde(default, deserialize_with = "deser::deser_red_optional_id")]
    pub g: Option<ElementId>,

    #[serde(default)]
    pub active: Option<bool>,

    #[serde(default, alias = "d")]
    pub disabled: bool,

    #[serde(default, deserialize_with = "deser::deserialize_wires")]
    pub wires: Vec<RedPortConfig>,

    #[serde(skip)]
    pub json: serde_json::Map<String, JsonValue>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct RedGlobalNodeConfig {
    #[serde(deserialize_with = "deser::deser_red_id")]
    pub id: ElementId,

    #[serde(alias = "type")]
    pub type_name: String,

    #[serde(default)]
    pub name: String,

    #[serde(default)]
    pub active: Option<bool>,

    #[serde(default)]
    pub disabled: bool,

    #[serde(skip)]
    pub json: serde_json::Map<String, JsonValue>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct RedSubflowPortWire {
    #[serde(deserialize_with = "deser::deser_red_id")]
    pub id: ElementId,

    #[serde(default)]
    pub port: usize,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct RedSubflowPort {
    // x: i32,
    // y: i32,
    #[serde(default)]
    pub wires: Vec<RedSubflowPortWire>,
}

#[derive(Debug, Clone)]
pub struct RedFlows {
    pub flows: Vec<RedFlowConfig>,
    pub global_nodes: Vec<RedGlobalNodeConfig>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RedPropertyType {
    Str,
    Num,
    Json,
    Re,
    Date,
    Bin,
    Msg,
    Flow,
    Global,
    Bool,
    Jsonata,
    Env,
}

#[derive(Debug, Clone)]
pub struct RedPropertyTriple {
    pub p: String,
    pub vt: RedPropertyType,
    pub v: String,
}

fn parse_property_triple(jv: &serde_json::Value) -> crate::Result<RedPropertyTriple> {
    Ok(RedPropertyTriple {
        vt: RedPropertyType::from(
            jv.get("vt")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("str"),
        )?,
        p: jv
            .get("p")
            .ok_or(EdgeLinkError::BadFlowsJson())?
            .as_str()
            .ok_or(EdgeLinkError::BadFlowsJson())?
            .to_string(),

        v: match jv.get("v").and_then(serde_json::Value::as_str) {
            Some(s) => s.to_string(),
            None => "".to_string(),
        },
    })
}

impl RedPropertyTriple {
    pub fn collection_from_json_value(
        jv: &serde_json::Value,
    ) -> crate::Result<Vec<RedPropertyTriple>> {
        if let Some(objects) = jv.as_array() {
            let entries: crate::Result<Vec<RedPropertyTriple>> =
                objects.iter().map(parse_property_triple).collect();
            entries
        } else {
            Err(EdgeLinkError::BadFlowsJson().into())
        }
    }
}
