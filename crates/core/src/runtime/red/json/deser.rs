use std::collections::{HashMap, HashSet};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::str::FromStr;

use serde::de;
use serde::Deserialize;
use serde::Deserializer;
use serde_json::Map as JsonMap;
use serde_json::Value as JsonValue;
use topological_sort::TopologicalSort;

use crate::runtime::model::ElementId;
use crate::runtime::red::json::*;
use crate::EdgeLinkError;

pub fn load_flows_json_value(root_jv: &JsonValue) -> crate::Result<JsonValues> {
    let processed = preprocess_root(root_jv)?;
    let all_values = processed.as_array().ok_or(EdgeLinkError::BadFlowsJson())?;

    let mut flows = HashMap::new();
    let mut groups = HashMap::new();
    let mut flow_nodes = HashMap::new();
    let mut global_nodes = Vec::new();

    let mut flow_topo_sort = TopologicalSort::<ElementId>::new();
    let mut group_topo_sort = TopologicalSort::<ElementId>::new();
    let mut node_topo_sort = TopologicalSort::<ElementId>::new();

    // Classify the JSON objects
    for jobject in all_values.iter() {
        if let Some(obj) = jobject.as_object() {
            if let (Some(ele_id), Some(type_value)) = (
                obj.get("id").and_then(parse_red_id_value),
                obj.get("type")
                    .and_then(|x| x.as_str())
                    .map(|x| parse_red_type_value(x)),
            ) {
                match type_value.red_type {
                    "tab" => {
                        let deps = obj.get_flow_dependencies(&all_values);
                        if deps.is_empty() {
                            flow_topo_sort.insert(ele_id);
                        } else {
                            deps.iter()
                                .for_each(|d| flow_topo_sort.add_dependency(*d, ele_id));
                        }
                        flows.insert(ele_id, jobject.clone());
                    }

                    "subflow" => {
                        if type_value.id.is_some() {
                            // "subflow:aabbccddee" We got a node that links to the subflow
                            let deps = obj.get_flow_node_dependencies();
                            if deps.is_empty() {
                                node_topo_sort.insert(ele_id);
                            } else {
                                deps.iter()
                                    .for_each(|d| node_topo_sort.add_dependency(*d, ele_id));
                            }
                            flow_nodes.insert(ele_id, jobject.clone());
                        } else {
                            // We got the "subflow" itself
                            let deps = obj.get_subflow_dependencies(&all_values);
                            if deps.is_empty() {
                                flow_topo_sort.insert(ele_id);
                            } else {
                                deps.iter()
                                    .for_each(|d| flow_topo_sort.add_dependency(*d, ele_id));
                            }
                            flows.insert(ele_id, jobject.clone());
                        }
                    }

                    "group" => match obj.get("z") {
                        Some(_) => {
                            let g: RedGroupConfig = serde_json::from_value(jobject.clone())?;
                            if let Some(parent_id) = &g.g {
                                group_topo_sort.add_dependency(*parent_id, ele_id);
                            } else {
                                group_topo_sort.insert(ele_id);
                            }
                            groups.insert(ele_id, g);
                        }
                        None => {
                            return Err(EdgeLinkError::BadFlowsJson().into());
                        }
                    },

                    "comment" => (),

                    // Dynamic nodes
                    _ => match obj.get("z") {
                        Some(_) => {
                            let deps = obj.get_flow_node_dependencies();
                            if deps.is_empty() {
                                node_topo_sort.insert(ele_id);
                            } else {
                                for &dep in deps.iter() {
                                    node_topo_sort.add_dependency(dep, ele_id);
                                }
                            }
                            flow_nodes.insert(ele_id, jobject.clone());
                        }
                        None => {
                            let mut global_config: RedGlobalNodeConfig =
                                serde_json::from_value(jobject.clone())?;
                            global_config.json = obj.clone();
                            global_nodes.push(global_config);
                        }
                    },
                }
            }
        } else {
            return Err(EdgeLinkError::BadFlowsJson().into());
        }
    }

    let mut sorted_flows = Vec::new();
    while let Some(flow_id) = flow_topo_sort.pop() {
        let flow = flows.remove(&flow_id).expect("No fucking way!!!");
        sorted_flows.push(flow);
    }

    let mut sorted_flow_groups = Vec::new();
    while let Some(group_id) = group_topo_sort.pop() {
        let group = groups.remove(&group_id).expect("No fucking way!!!");
        sorted_flow_groups.push(group);
    }

    let mut sorted_flow_nodes = Vec::new();
    while let Some(node_id) = node_topo_sort.pop() {
        // We check for cycle errors before usage
        let node = flow_nodes[&node_id].clone();
        log::debug!(
            "\t -- node.id={}, node.name={}, node.type={}",
            node_id,
            node.get("name").unwrap().as_str().unwrap(),
            node.get("type").unwrap().as_str().unwrap()
        );
        sorted_flow_nodes.push(node);
    }

    let mut flow_configs = Vec::with_capacity(flows.len());
    for flow in sorted_flows.iter() {
        let mut flow_config: RedFlowConfig = serde_json::from_value(flow.clone())?;

        flow_config.subflow_node_id = if flow_config.type_name == "subflow" {
            let key_type = format!("subflow:{}", flow_config.id);
            let node = all_values.iter().find(|x| {
                x.get("type")
                    .and_then(|y| y.as_str())
                    .is_some_and(|y| y == key_type)
            });
            node.and_then(|x| x.get("id")).and_then(parse_red_id_value)
        } else {
            None
        };

        flow_config.json = flow.as_object().unwrap().clone();
        flow_config.groups = sorted_flow_groups
            .iter()
            .filter(|x| x.z == flow_config.id)
            .cloned()
            .collect();

        let owned_node_jvs = sorted_flow_nodes.iter().filter(|x| {
            x.get("z")
                .and_then(parse_red_id_value)
                .map_or(false, |z| z == flow_config.id)
        });

        for flow_node_jv in owned_node_jvs.into_iter() {
            let mut node_config: RedFlowNodeConfig = serde_json::from_value(flow_node_jv.clone())?;
            node_config.json = flow_node_jv.as_object().unwrap().clone();
            flow_config.nodes.push(node_config);
        }

        flow_configs.push(flow_config);
    }

    Ok(JsonValues {
        flows: flow_configs,
        global_nodes,
    })
}

fn preprocess_root(jv_root: &JsonValue) -> crate::Result<JsonValue> {
    let elements = jv_root.as_array().unwrap();
    let mut ids_to_delete = HashSet::new();

    #[derive(Debug)]
    struct SubflowPack<'a> {
        subflow_node: &'a JsonValue,
        subflow: &'a JsonValue,
        children: Vec<&'a JsonValue>,
    }

    let mut subflow_packs = Vec::new();

    // Find out all of subflow related elements
    for jv in elements.iter() {
        if let Some((_, subflow_id)) = jv
            .get("type")
            .and_then(|x| x.as_str())
            .and_then(|x| x.split_once(':'))
        {
            ids_to_delete.insert(jv["id"].as_str().unwrap());
            ids_to_delete.insert(subflow_id);

            let pack = SubflowPack {
                subflow_node: jv,
                subflow: elements
                    .iter()
                    .find(|x| x.get("id").unwrap().as_str().unwrap() == subflow_id)
                    .unwrap(),
                children: elements
                    .iter()
                    .filter(|x| {
                        x.get("z")
                            .and_then(|y| y.as_str())
                            .is_some_and(|y| y == subflow_id)
                    })
                    .collect(),
            };

            ids_to_delete.extend(
                pack.children
                    .iter()
                    .map(|x| x.get("id").unwrap().as_str().unwrap()),
            );

            subflow_packs.push(pack);
        }
    }

    let mut new_elements = Vec::new();
    let mut id_map: HashMap<String, String> = HashMap::new();

    for pack in subflow_packs.iter() {
        let subflow_new_id = ElementId::new();

        // TODO code refactoring

        // "subflow" element
        {
            let mut new_subflow = pack.subflow.clone();
            id_map.insert(
                new_subflow["id"].as_str().unwrap().to_string(),
                format!("{}", subflow_new_id),
            );
            new_subflow["id"] = JsonValue::String(format!("{}", subflow_new_id));

            for in_item in new_subflow["in"].as_array_mut().unwrap() {
                for wires_item in in_item["wires"].as_array_mut().unwrap() {
                    id_map.insert(
                        wires_item["id"].as_str().unwrap().to_string(),
                        generate_new_xored_id(subflow_new_id, &wires_item["id"])
                            .as_str()
                            .unwrap()
                            .to_string(),
                    );
                    wires_item["id"] = generate_new_xored_id(subflow_new_id, &wires_item["id"]);
                }
            }

            for out_item in new_subflow["out"].as_array_mut().unwrap() {
                for wires_item in out_item["wires"].as_array_mut().unwrap() {
                    id_map.insert(
                        wires_item["id"].as_str().unwrap().to_string(),
                        generate_new_xored_id(subflow_new_id, &wires_item["id"])
                            .as_str()
                            .unwrap()
                            .to_string(),
                    );
                    wires_item["id"] = generate_new_xored_id(subflow_new_id, &wires_item["id"]);
                }
            }
            new_elements.push(new_subflow);
        }

        // "subflow:xxxxxx" node
        {
            let mut new_subflow_node = pack.subflow_node.clone();
            new_subflow_node["type"] = JsonValue::String(format!("subflow:{}", subflow_new_id));
            new_elements.push(new_subflow_node);
        }

        // normals in subflow
        {
            for node in pack.children.iter() {
                let mut node = (*node).clone();

                node["id"] = generate_new_xored_id(subflow_new_id, &node["id"]);
                node["z"] = JsonValue::String(format!("{}", subflow_new_id));

                if let Some(gid) = node.get_mut("g") {
                    *gid = generate_new_xored_id(subflow_new_id, gid);
                }

                if let Some(wires) = node.get_mut("wires").and_then(|x| x.as_array_mut()) {
                    for wire in wires {
                        let wire = wire.as_array_mut().unwrap();
                        for id in wire {
                            let new_id = generate_new_xored_id(subflow_new_id, id);
                            *id = new_id;
                        }
                    }
                }

                // TODO CHECK TYPE: complete/catch/status
                if let Some(scope) = node.get_mut("scope").and_then(|x| x.as_array_mut()) {
                    for id in scope {
                        let new_id = generate_new_xored_id(subflow_new_id, id);
                        *id = new_id;
                    }
                }

                // TODO node.XYZ old property

                new_elements.push(node);
            }
        }
    }

    new_elements.extend(
        elements
            .iter()
            .filter(|x| !ids_to_delete.contains(x["id"].as_str().unwrap()))
            .cloned(),
    );

    Ok(JsonValue::Array(new_elements))
}

fn generate_new_xored_id(subflow_id: ElementId, old_id_value: &JsonValue) -> JsonValue {
    let old_id = parse_red_id_value(old_id_value).unwrap();
    JsonValue::String(format!("{}", (subflow_id ^ old_id)))
}

pub fn parse_red_type_value(t: &str) -> RedTypeValue {
    match t.split_once(':') {
        Some((x, y)) => RedTypeValue {
            red_type: x,
            id: parse_red_id_str(y),
        },
        None => RedTypeValue {
            red_type: t,
            id: None,
        },
    }
}

pub fn parse_red_id_str(id_str: &str) -> Option<ElementId> {
    id_str.parse().ok()
}

pub fn parse_red_id_value(id_value: &serde_json::Value) -> Option<ElementId> {
    id_value.as_str().and_then(|s| s.parse().ok())
}

pub trait RedFlowJsonObject {
    fn get_flow_dependencies(&self, elements: &[JsonValue]) -> HashSet<ElementId>;
    fn get_subflow_dependencies(&self, elements: &[JsonValue]) -> HashSet<ElementId>;
}

impl RedFlowJsonObject for JsonMap<String, JsonValue> {
    fn get_flow_dependencies(&self, elements: &[JsonValue]) -> HashSet<ElementId> {
        let this_id = self.get("id");
        let link_out_type: JsonValue = "link out".into();

        let related_link_in_ids = elements
            .iter()
            .filter_map(|x| {
                if x.get("z") == this_id && x.get("type") == Some(&link_out_type) {
                    x.get("links").and_then(|y| y.as_array())
                } else {
                    None
                }
            })
            .flat_map(|array| array.iter())
            .collect::<HashSet<&JsonValue>>();

        elements
            .iter()
            .filter(|x| {
                x.get("id")
                    .map_or(false, |id| related_link_in_ids.contains(id))
            })
            .filter_map(|x| x.get("z"))
            .filter_map(parse_red_id_value)
            .collect::<HashSet<ElementId>>()
    }

    fn get_subflow_dependencies(&self, elements: &[JsonValue]) -> HashSet<ElementId> {
        let subflow_id = self
            .get("id")
            .and_then(|x| x.as_str())
            .expect("Must have `id`");

        elements
            .iter()
            .filter_map(|x| x.as_object())
            .filter(|o| {
                o.get("type")
                    .and_then(|x| x.as_str())
                    .and_then(|x| x.split_once(':'))
                    .is_some_and(|x| x.0 == "subflow" && x.1 == subflow_id)
            })
            .filter_map(|o| o.get("z"))
            .filter_map(parse_red_id_value)
            .collect::<HashSet<ElementId>>()
    }
}

pub trait RedFlowNodeJsonObject {
    fn get_flow_node_dependencies(&self) -> HashSet<ElementId>;
}

impl RedFlowNodeJsonObject for JsonMap<String, JsonValue> {
    fn get_flow_node_dependencies(&self) -> HashSet<ElementId> {
        let mut result = HashSet::new();

        // Add wires
        if let Some(wires) = self
            .get("wires")
            .and_then(|wires_value| wires_value.as_array())
        {
            let iter = wires
                .iter()
                .filter_map(|port| port.as_array())
                .flatten()
                .filter_map(parse_red_id_value);
            result.extend(iter);
        }

        // Add scope
        if let Some(scopes) = self
            .get("scope")
            .and_then(|wires_value| wires_value.as_array())
        {
            let iter = scopes
                .iter()
                .filter_map(|port| port.as_array())
                .flatten()
                .filter_map(parse_red_id_value);
            result.extend(iter);
        }

        result
    }
}

pub fn deser_red_id<'de, D>(deserializer: D) -> Result<ElementId, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    s.parse().map_err(serde::de::Error::custom)
}

pub fn deser_red_optional_id<'de, D>(deserializer: D) -> Result<Option<ElementId>, D::Error>
where
    D: Deserializer<'de>,
{
    let s: Option<String> = Option::deserialize(deserializer)?;
    match s {
        Some(s) => {
            if s.is_empty() {
                Ok(None)
            } else {
                s.parse().map_err(serde::de::Error::custom).map(Some)
            }
        }
        None => Ok(None),
    }
}

pub fn deser_red_id_vec<'de, D>(deserializer: D) -> Result<Vec<ElementId>, D::Error>
where
    D: Deserializer<'de>,
{
    let str_ids: Vec<String> = Vec::deserialize(deserializer)?;
    let mut ids = Vec::with_capacity(str_ids.capacity());
    for str_id in str_ids.iter() {
        ids.push(str_id.parse().map_err(serde::de::Error::custom)?);
    }
    Ok(ids)
}

pub(crate) fn deserialize_wires<'de, D>(deserializer: D) -> Result<Vec<RedPortConfig>, D::Error>
where
    D: Deserializer<'de>,
{
    struct WiresVisitor;

    impl<'de> de::Visitor<'de> for WiresVisitor {
        type Value = Vec<RedPortConfig>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a list of list of strings representing hex-encoded u64 values")
        }

        fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
        where
            A: de::SeqAccess<'de>,
        {
            let mut wires = Vec::new();

            while let Some(inner_seq) = seq.next_element::<Vec<String>>()? {
                let mut node_ids = Vec::new();

                for hex_str in inner_seq {
                    let node_id = parse_red_id_str(&hex_str)
                        .ok_or("Bad ID string")
                        .map_err(de::Error::custom)?;
                    node_ids.push(node_id);
                }

                wires.push(RedPortConfig { node_ids });
            }

            Ok(wires)
        }
    }

    deserializer.deserialize_seq(WiresVisitor)
}

impl RedPropertyType {
    pub fn from(ptype: &str) -> crate::Result<RedPropertyType> {
        match ptype {
            "str" => Ok(RedPropertyType::Str),
            "num" => Ok(RedPropertyType::Num),
            "json" => Ok(RedPropertyType::Json),
            "re" => Ok(RedPropertyType::Re),
            "date" => Ok(RedPropertyType::Date),
            "bin" => Ok(RedPropertyType::Bin),
            "msg" => Ok(RedPropertyType::Msg),
            "flow" => Ok(RedPropertyType::Flow),
            "global" => Ok(RedPropertyType::Global),
            "bool" => Ok(RedPropertyType::Bool),
            "jsonata" => Ok(RedPropertyType::Jsonata),
            "env" => Ok(RedPropertyType::Env),
            _ => Err(EdgeLinkError::BadFlowsJson().into()),
        }
    }
}

pub fn string_to_option_u16<'de, D>(deserializer: D) -> Result<Option<u16>, D::Error>
where
    D: Deserializer<'de>,
{
    let value: Option<String> = Option::deserialize(deserializer)?;
    match value {
        Some(s) => {
            if s.is_empty() {
                Ok(None)
            } else {
                s.parse::<u16>().map(Some).map_err(|_| {
                    de::Error::invalid_value(de::Unexpected::Str(&s), &"An invalid u16")
                })
            }
        }
        None => Ok(None),
    }
}

pub fn string_to_ipaddr<'de, D>(deserializer: D) -> Result<Option<IpAddr>, D::Error>
where
    D: Deserializer<'de>,
{
    let value: Option<String> = Option::deserialize(deserializer)?;
    match value {
        Some(s) => {
            if s.is_empty() {
                Ok(None)
            }
            // Try parsing as IPv4
            else if let Ok(ipv4) = s.parse::<Ipv4Addr>() {
                Ok(Some(IpAddr::V4(ipv4)))
            }
            // Try parsing as IPv6
            else if let Ok(ipv6) = s.parse::<Ipv6Addr>() {
                Ok(Some(IpAddr::V6(ipv6)))
            }
            // If neither, return an error
            else {
                Err(de::Error::invalid_value(
                    de::Unexpected::Str(&s),
                    &"a valid IP address",
                ))
            }
        }
        None => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_red_property_triple_should_be_ok() {
        let data = r#"[
        {
            "p": "timestamp",
            "v": "",
            "vt": "date"
        }
    ]"#;

        // Parse the string of data into serde_json::Value.
        let v: serde_json::Value = serde_json::from_str(data).unwrap();
        let triples = RedPropertyTriple::collection_from_json_value(&v).unwrap();
        assert_eq!(1, triples.len());
        assert_eq!("timestamp", triples[0].p);
        assert_eq!(RedPropertyType::Date, triples[0].vt);
    }
}
