use crate::runtime::model::*;

pub fn parse_red_id_str(id_str: &str) -> Option<ElementId> {
    id_str.parse().ok()
}

pub fn parse_red_id_value(id_value: &serde_json::Value) -> Option<ElementId> {
    id_value.as_str().and_then(|s| s.parse().ok())
}
