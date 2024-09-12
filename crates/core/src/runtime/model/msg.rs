use std::collections::BTreeMap;
use std::fmt;
use std::fmt::Write;
use std::ops::{Index, IndexMut};
use std::sync::Arc;

use serde::de;
use serde::ser::SerializeMap;
use tokio::sync::RwLock;

#[cfg(feature = "js")]
mod js {
    pub use rquickjs::{prelude::*, *};
}

use crate::runtime::model::json::deser::parse_red_id_str;
use crate::runtime::model::propex::*;
use crate::runtime::model::*;

pub mod wellknown {
    pub const MSG_ID_PROPERTY: &str = "_msgid";
    pub const LINK_SOURCE_PROPERTY: &str = "_linkSource";
}

#[derive(Debug)]
pub struct Envelope {
    pub port: usize,
    pub msg: Arc<RwLock<Msg>>,
}

pub type MsgBody = BTreeMap<String, Variant>;

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct LinkCallStackEntry {
    pub id: ElementId,
    pub link_call_node_id: ElementId,
}

#[derive(Debug)]
pub struct Msg {
    pub body: BTreeMap<String, Variant>,
    pub link_call_stack: Option<Vec<LinkCallStackEntry>>,
}

impl Msg {
    pub fn new_default() -> Arc<RwLock<Self>> {
        let msg = Msg {
            link_call_stack: None,
            body: BTreeMap::from([
                (wellknown::MSG_ID_PROPERTY.to_string(), Msg::generate_id_variant()),
                ("payload".to_string(), Variant::Null),
            ]),
        };
        Arc::new(RwLock::new(msg))
    }

    pub fn new_with_body(body: BTreeMap<String, Variant>) -> Arc<RwLock<Self>> {
        let msg = Msg { link_call_stack: None, body };
        Arc::new(RwLock::new(msg))
    }

    pub fn new_with_payload(payload: Variant) -> Arc<RwLock<Self>> {
        let msg = Msg {
            link_call_stack: None,
            body: BTreeMap::from([
                (wellknown::MSG_ID_PROPERTY.to_string(), Msg::generate_id_variant()),
                ("payload".to_string(), payload),
            ]),
        };
        Arc::new(RwLock::new(msg))
    }

    pub fn id(&self) -> Option<ElementId> {
        self.body.get(wellknown::MSG_ID_PROPERTY).and_then(|x| x.as_str()).and_then(parse_red_id_str)
    }

    pub fn generate_id() -> ElementId {
        ElementId::new()
    }

    pub fn generate_id_variant() -> Variant {
        Variant::String(ElementId::new().to_string())
    }

    pub fn contains_property(&self, prop: &str) -> bool {
        self.body.contains_key(prop)
    }

    pub fn get_property(&self, prop: &str) -> Option<&Variant> {
        self.body.get(prop)
    }

    pub fn get_property_mut(&mut self, prop: &str) -> Option<&mut Variant> {
        self.body.get_mut(prop)
    }

    /// Get the value of a navigation property
    ///
    /// The first level of the property expression for 'msg' must be a string, which means it must be
    /// `msg[msg.topic]` `msg['aaa']` or `msg.aaa`, and not `msg[12]`
    pub fn get_nav_property(&self, expr: &str) -> Option<&Variant> {
        let mut segs = propex::parse(expr).ok()?;
        self.normalize_segments(&mut segs).ok()?;
        self.get_property_by_segments_internal(&segs)
    }

    pub fn get_nav_property_mut(&mut self, expr: &str) -> Option<&mut Variant> {
        let mut segs = propex::parse(expr).ok()?;
        if segs.iter().any(|x| matches!(x, PropexSegment::Nested(_))) {
            // Do things
            self.normalize_segments(&mut segs).ok()?;
            let mut normalized = String::new();
            for seg in segs {
                write!(&mut normalized, "{}", seg).unwrap();
            }
            dbg!(&normalized);
            let segs = propex::parse(&normalized).ok()?;
            let segs = segs.clone();
            self.get_property_by_segments_internal_mut(&segs)
        } else {
            self.get_property_by_segments_internal_mut(&segs)
        }
    }

    pub fn get_trimmed_nav_property_mut(&mut self, expr: &str) -> Option<&mut Variant> {
        let trimmed_expr = expr.trim_ascii();
        if let Some(stripped_expr) = trimmed_expr.strip_prefix("msg.") {
            self.get_nav_property_mut(stripped_expr)
        } else {
            self.get_nav_property_mut(trimmed_expr)
        }
    }

    pub fn get_trimmed_nav_property(&self, expr: &str) -> Option<&Variant> {
        let trimmed_expr = expr.trim_ascii();
        if let Some(stripped_expr) = trimmed_expr.strip_prefix("msg.") {
            self.get_nav_property(stripped_expr)
        } else {
            self.get_nav_property(trimmed_expr)
        }
    }

    fn normalize_segments<'a>(&'a self, segs: &mut [PropexSegment<'a>]) -> crate::Result<()> {
        for seg in segs.iter_mut() {
            if let PropexSegment::Nested(nested_segs) = seg {
                if nested_segs.first() != Some(&PropexSegment::Property("msg")) {
                    return Err(EdgelinkError::BadArguments("The expression must contains `msg.`".into()).into());
                }
                *seg =
                    match self.get_property_by_segments_internal(&nested_segs[1..]).ok_or(EdgelinkError::OutOfRange)? {
                        Variant::String(str_index) => PropexSegment::Property(str_index.as_str()),
                        Variant::Integer(int_index) if *int_index >= 0 => PropexSegment::Index(*int_index as usize),
                        Variant::Rational(f64_index) if *f64_index >= 0.0 => {
                            PropexSegment::Index(f64_index.round() as usize)
                        }
                        _ => return Err(EdgelinkError::OutOfRange.into()), // We cannot found the nested property
                    };
            }
        }
        Ok(())
    }

    fn get_property_by_segments_internal(&self, segs: &[PropexSegment]) -> Option<&Variant> {
        match segs {
            [PropexSegment::Property(first_prop_name)] => self.body.get(*first_prop_name),
            [PropexSegment::Property(first_prop_name), ref rest @ ..] => {
                self.body.get(*first_prop_name)?.get_item_by_propex_segments(rest)
            }
            _ => None,
        }
    }

    fn get_property_by_segments_internal_mut(&mut self, segs: &[PropexSegment]) -> Option<&mut Variant> {
        match segs {
            [PropexSegment::Property(first_prop_name)] => self.get_property_mut(first_prop_name),
            [PropexSegment::Property(first_prop_name), ref rest @ ..] => {
                self.get_property_mut(first_prop_name)?.get_item_by_propex_segments_mut(rest)
            }
            _ => None,
        }
    }

    pub fn set_property(&mut self, prop: String, value: Variant) {
        let _ = self.body.insert(prop, value);
    }

    pub fn set_nav_property(&mut self, expr: &str, value: Variant, create_missing: bool) -> crate::Result<()> {
        if expr.is_empty() {
            return Err(crate::EdgelinkError::BadArguments("The argument expr cannot be empty".to_string()).into());
        }

        let segs = propex::parse(expr).map_err(|e| crate::EdgelinkError::BadArguments(e.to_string()))?;

        let first_prop_name = match segs.first() {
            Some(PropexSegment::Property(name)) => name,
            _ => {
                return Err(crate::EdgelinkError::BadArguments(format!(
                    "The first property to access must be a string, but got '{}'",
                    expr
                ))
                .into())
            }
        };

        // If create_missing is true and first_prop doesn't exist, we should create it here.
        let first_prop = match (self.get_property_mut(first_prop_name), create_missing, segs.len()) {
            (Some(prop), _, _) => prop,
            (None, true, 1) => {
                // Only one level of the property
                self.body.insert(expr.into(), value);
                return Ok(());
            }
            (None, true, _) => {
                let next_seg = segs.get(1);
                let var = match next_seg {
                    // the next level property is an object
                    Some(PropexSegment::Property(_)) => Variant::empty_object(),
                    Some(PropexSegment::Index(_)) => Variant::empty_array(),
                    _ => {
                        return Err(crate::EdgelinkError::BadArguments(format!(
                            "Not allowed to set first property: '{}'",
                            first_prop_name
                        ))
                        .into());
                    }
                };
                self.body.insert(first_prop_name.to_string(), var);
                self.get_property_mut(first_prop_name).unwrap()
            }
            (None, _, _) => {
                return Err(crate::EdgelinkError::BadArguments(format!(
                    "Failed to set first property: '{}'",
                    first_prop_name
                ))
                .into());
            }
        };

        if segs.len() == 1 {
            *first_prop = value;
            return Ok(());
        }

        match first_prop.get_item_by_propex_segments_mut(&segs[1..]) {
            Some(pv) => {
                *pv = value;
                Ok(())
            }
            None if create_missing => {
                first_prop.set_property_by_propex_segments(&segs[1..], value, true).map_err(Into::into)
            }
            None => Err(crate::EdgelinkError::InvalidOperation(
                "Unable to set property: missing intermediate segments".into(),
            )
            .into()),
        }
    }

    pub fn set_trimmed_nav_property(&mut self, expr: &str, value: Variant, create_missing: bool) -> crate::Result<()> {
        let trimmed_expr = expr.trim_ascii();
        if let Some(stripped_expr) = trimmed_expr.strip_prefix("msg.") {
            self.set_nav_property(stripped_expr, value, create_missing)
        } else {
            self.set_nav_property(trimmed_expr, value, create_missing)
        }
    }

    #[cfg(feature = "js")]
    pub fn as_js_object<'js>(&self, ctx: &js::context::Ctx<'js>) -> crate::Result<js::Object<'js>> {
        use js::IntoAtom;
        use rquickjs::IntoJs;
        let obj = js::Object::new(ctx.clone())?;
        for (k, v) in self.body.iter() {
            let prop_name = k.into_atom(ctx).map_err(|e| EdgelinkError::InvalidData(e.to_string()))?;

            let prop_value = v.as_js_value(ctx).map_err(|e| EdgelinkError::InvalidData(e.to_string()))?;

            obj.set(prop_name, prop_value).map_err(|e| EdgelinkError::InvalidData(e.to_string()))?;
        }

        {
            let link_source_atom = wellknown::LINK_SOURCE_PROPERTY.into_js(ctx)?;
            let link_source_buffer = js::ArrayBuffer::new(ctx.clone(), bincode::serialize(&self.link_call_stack)?)?;
            let link_source_value = link_source_buffer.into_js(ctx)?;

            //.map_err(|e| EdgelinkError::InvalidData(e.to_string()))?;
            obj.set(link_source_atom, link_source_value)?

            /*
            let msg_id_atom = "_msgid"
                .into_atom(ctx)
                .map_err(|e| EdgelinkError::InvalidData(e.to_string()))?;
            let msg_id_value = self
                .id
                .to_string()
                .into_js(ctx)
                .map_err(|e| EdgelinkError::InvalidData(e.to_string()))?;
            obj.set(msg_id_atom, msg_id_value)
                .map_err(|e| EdgelinkError::InvalidData(e.to_string()))?;
            */

            /*
            let link_source_atom = "_linkSource"
                .into_atom(ctx)
                .map_err(|e| EdgeLinkError::InvalidData(e.to_string()))?;
            let link_source_atom = self
                .id
                .to_string()
                .into_js(ctx)
                .map_err(|e| EdgeLinkError::InvalidData(e.to_string()))?;
            obj.set(msg_id_atom, msg_id_value)
                .map_err(|e| EdgeLinkError::InvalidData(e.to_string()))?;
            */
        }
        Ok(obj)
    }

    #[cfg(feature = "js")]
    pub fn as_js_value<'js>(&self, ctx: &js::Ctx<'js>) -> crate::Result<js::Value<'js>> {
        Ok(js::Value::from_object(self.as_js_object(ctx)?))
    }
}

impl Msg {
    pub fn push_link_source(&mut self, lse: LinkCallStackEntry) {
        if let Some(link_source) = &mut self.link_call_stack {
            link_source.push(lse);
        } else {
            self.link_call_stack = Some(vec![lse]);
        }
    }

    pub fn pop_link_source(&mut self) -> Option<LinkCallStackEntry> {
        if let Some(link_source) = &mut self.link_call_stack {
            let p = link_source.pop();
            if link_source.is_empty() {
                self.link_call_stack = None
            }
            p
        } else {
            None
        }
    }
}

impl Clone for Msg {
    fn clone(&self) -> Self {
        Self { link_call_stack: self.link_call_stack.clone(), body: self.body.clone() }
    }
}

impl Index<&str> for Msg {
    type Output = Variant;

    fn index(&self, key: &str) -> &Self::Output {
        &self.body[key]
    }
}

impl IndexMut<&str> for Msg {
    fn index_mut(&mut self, key: &str) -> &mut Self::Output {
        self.body.entry(key.to_string()).or_default()
    }
}

impl serde::Serialize for Msg {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut map = serializer.serialize_map(None)?;
        map.serialize_entry(wellknown::LINK_SOURCE_PROPERTY, &self.link_call_stack)?;
        for (k, v) in self.body.iter() {
            map.serialize_entry(k, v)?;
        }
        map.end()
    }
}

impl<'de> serde::Deserialize<'de> for Msg {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct MsgVisitor;

        impl<'de> serde::de::Visitor<'de> for MsgVisitor {
            type Value = Msg;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("struct Msg")
            }

            fn visit_map<V>(self, mut map: V) -> Result<Msg, V::Error>
            where
                V: serde::de::MapAccess<'de>,
            {
                let mut link_call_stack = None;
                let mut body: BTreeMap<String, Variant> = BTreeMap::new();

                while let Some(key) = map.next_key()? {
                    match key {
                        wellknown::LINK_SOURCE_PROPERTY => {
                            if link_call_stack.is_some() {
                                return Err(de::Error::duplicate_field(wellknown::LINK_SOURCE_PROPERTY));
                            }
                            link_call_stack = Some(map.next_value()?);
                        }
                        _ => {
                            let value = map.next_value()?;
                            body.insert(key.to_string(), value);
                        }
                    }
                }

                Ok(Msg { body, link_call_stack })
            }
        }

        deserializer.deserialize_map(MsgVisitor)
    }
}

#[cfg(feature = "js")]
impl<'js> js::FromJs<'js> for Msg {
    fn from_js(ctx: &js::Ctx<'js>, jv: js::Value<'js>) -> js::Result<Msg> {
        let mut link_call_stack: Option<Vec<LinkCallStackEntry>> = None;
        match jv.type_of() {
            js::Type::Object => {
                if let Some(jo) = jv.as_object() {
                    let mut body = BTreeMap::new();
                    // TODO _msgid check
                    for result in jo.props::<String, js::Value>() {
                        match result {
                            Ok((ref k, v)) => match k.as_str() {
                                wellknown::LINK_SOURCE_PROPERTY => {
                                    if let Some(bytes) =
                                        v.as_object().and_then(|x| x.as_array_buffer()).and_then(|x| x.as_bytes())
                                    {
                                        link_call_stack =
                                            bincode::deserialize(bytes).map_err(|_| js::Error::FromJs {
                                                from: wellknown::LINK_SOURCE_PROPERTY,
                                                to: "link_call_stack",
                                                message: Some(
                                                    "Failed to deserialize `_linkSource` property".to_string(),
                                                ),
                                            })?;
                                    }
                                }
                                _ => {
                                    body.insert(k.clone(), Variant::from_js(ctx, v)?);
                                }
                            },
                            Err(e) => {
                                eprintln!("Error occurred: {:?}", e);
                                panic!();
                            }
                        }
                    }
                    Ok(Msg { link_call_stack, body })
                } else {
                    Err(js::Error::FromJs { from: "JS object", to: "Variant::Object", message: None })
                }
            }
            _ => Err(js::Error::FromJs { from: "Unsupported JS type", to: "", message: None }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;
    use serde_json::json;

    #[test]
    fn test_get_nested_nav_property() {
        let jv = json!({"payload": "newValue", "lookup": {"a": 1, "b": 2}, "topic": "b"});
        let msg = Msg::deserialize(&jv).unwrap();
        {
            assert!(msg.contains_property("lookup"));
            assert!(msg.contains_property("topic"));
            assert_eq!(*msg.get_nav_property("lookup[msg.topic]").unwrap(), Variant::Integer(2));
        }
    }

    #[test]
    fn test_get_nested_nav_property_mut() {
        let jv = json!({"payload": "newValue", "lookup": {"a": 1, "b": 2}, "topic": "b"});
        let mut msg = Msg::deserialize(&jv).unwrap();
        {
            assert!(msg.contains_property("lookup"));
            assert!(msg.contains_property("topic"));
            let b = msg.get_nav_property_mut("lookup[msg.topic]").unwrap();
            *b = Variant::Integer(1701);
            assert_eq!(*msg.get_nav_property("lookup.b").unwrap(), Variant::Integer(1701));
        }
    }

    #[test]
    fn test_set_deep_msg_property() {
        let jv = json!( {"foo": {"bar": "foo"}, "name": "hello"});
        let mut msg = Msg::deserialize(&jv).unwrap();
        {
            let old_foo = msg.get_property("foo").unwrap();
            assert!(old_foo.is_object());
            assert_eq!(old_foo.as_object().unwrap()["bar"].as_str().unwrap(), "foo");
        }
        msg.set_property("name".into(), "world".into());
        assert_eq!(msg.get_property("name").unwrap().as_str().unwrap(), "world");

        msg.set_nav_property("foo.bar", "changed2".into(), false).unwrap();
        assert_eq!(
            msg.get_property("foo").unwrap().as_object().unwrap().get("bar").unwrap().as_str().unwrap(),
            "changed2"
        );

        assert!(msg.set_nav_property("foo.new_field", "new_value".into(), false).is_err());

        assert!(msg.set_nav_property("foo.new_new_field", "new_new_value".into(), true).is_ok());

        assert_eq!(
            msg.get_property("foo").unwrap().as_object().unwrap().get("new_new_field").unwrap().as_str().unwrap(),
            "new_new_value"
        );
    }

    #[test]
    fn should_be_ok_with_empty_object_variant() {
        let jv = json!({});
        let mut msg = Msg::deserialize(&jv).unwrap();

        msg.set_nav_property("foo.bar", "changed2".into(), true).unwrap();
        assert!(msg.contains_property("foo"));
        assert_eq!(
            msg.get_property("foo").unwrap().as_object().unwrap().get("bar").unwrap().as_str().unwrap(),
            "changed2"
        );

        assert!(msg.set_nav_property("foo.new_field", "new_value".into(), false).is_err());

        assert!(msg.set_nav_property("foo.new_new_field", "new_new_value".into(), true).is_ok());

        assert_eq!(
            msg.get_property("foo").unwrap().as_object().unwrap().get("new_new_field").unwrap().as_str().unwrap(),
            "new_new_value"
        );
    }
}
