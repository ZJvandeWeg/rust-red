use serde_json::Value;

pub static EMPTY_ARRAY: Vec<serde_json::Value> = Vec::new();

pub fn value_equals_str(jv: &Value, target_string: &str) -> bool {
    match jv.as_str() {
        Some(s) => s == target_string,
        _ => false,
    }
}

pub fn option_value_equals_str(jv: &Option<&Value>, target_string: &str) -> bool {
    match jv {
        Some(s) => value_equals_str(s, target_string),
        _ => false,
    }
}
