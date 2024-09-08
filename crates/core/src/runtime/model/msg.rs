use std::collections::BTreeMap;
use std::fmt;
use std::ops::{Index, IndexMut};
use std::sync::Arc;

use serde::de;

use serde::ser::SerializeMap;
use tokio::sync::RwLock;

#[cfg(feature = "js")]
mod js {
    pub use rquickjs::{prelude::*, *};
}

use crate::red::json::deser::parse_red_id_str;
use crate::runtime::model::propex::*;
use crate::runtime::model::*;

pub mod wellknown {
    pub const MSG_ID_PROPERTY: &str = "_msgid";
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
    pub birth_place: ElementId,

    pub body: BTreeMap<String, Variant>,

    pub link_call_stack: Option<Vec<LinkCallStackEntry>>,
}

impl Msg {
    pub fn new_default(birth_place: ElementId) -> Arc<RwLock<Self>> {
        let msg = Msg {
            birth_place,
            link_call_stack: None,
            body: BTreeMap::from([
                (
                    wellknown::MSG_ID_PROPERTY.to_string(),
                    Msg::generate_id_variant(),
                ),
                ("payload".to_string(), Variant::Null),
            ]),
        };
        Arc::new(RwLock::new(msg))
    }

    pub fn new_with_body(
        birth_place: ElementId,
        body: BTreeMap<String, Variant>,
    ) -> Arc<RwLock<Self>> {
        let msg = Msg {
            birth_place,
            link_call_stack: None,
            body,
        };
        Arc::new(RwLock::new(msg))
    }

    pub fn new_with_payload(birth_place: ElementId, payload: Variant) -> Arc<RwLock<Self>> {
        let msg = Msg {
            birth_place,
            link_call_stack: None,
            body: BTreeMap::from([
                (
                    wellknown::MSG_ID_PROPERTY.to_string(),
                    Msg::generate_id_variant(),
                ),
                ("payload".to_string(), payload),
            ]),
        };
        Arc::new(RwLock::new(msg))
    }

    pub fn id(&self) -> Option<ElementId> {
        self.body
            .get(wellknown::MSG_ID_PROPERTY)
            .and_then(|x| x.as_str())
            .and_then(parse_red_id_str)
    }

    pub fn generate_id() -> ElementId {
        ElementId::new()
    }

    pub fn generate_id_variant() -> Variant {
        Variant::String(ElementId::new().to_string())
    }

    pub fn get_property(&self, prop: &str) -> Option<&Variant> {
        self.body.get(prop)
    }

    pub fn get_property_mut(&mut self, prop: &str) -> Option<&mut Variant> {
        self.body.get_mut(prop)
    }

    pub fn get_nav_property(&self, expr: &str) -> Option<&Variant> {
        let segs = propex::parse(expr).ok()?;
        match segs[0] {
            // The first level of the property expression for 'msg' must be a string, which means it must be
            // `msg['aaa']` or `msg.aaa`, and not `msg[12]`
            PropexSegment::StringIndex(first_prop_name) => {
                let first_prop = self.get_property(first_prop_name)?;
                if segs.len() == 1 {
                    Some(first_prop)
                } else {
                    first_prop.get_item_by_propex_segments(&segs[1..])
                }
            }
            _ => None,
        }
    }

    pub fn get_nav_property_mut(&mut self, expr: &str) -> Option<&mut Variant> {
        let segs = propex::parse(expr).ok()?;
        match segs[0] {
            // The first level of the property expression for 'msg' must be a string, which means it must be
            // `msg['aaa']` or `msg.aaa`, and not `msg[12]`
            PropexSegment::StringIndex(first_prop_name) => {
                let first_prop = self.get_property_mut(first_prop_name)?;
                if segs.len() == 1 {
                    Some(first_prop)
                } else {
                    first_prop.get_item_by_propex_segments_mut(&segs[1..])
                }
            }
            _ => None,
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

    pub fn set_property(&mut self, prop: String, value: Variant) {
        let _ = self.body.insert(prop, value);
    }

    pub fn set_nav_property(
        &mut self,
        expr: &str,
        value: Variant,
        create_missing: bool,
    ) -> crate::Result<()> {
        let segs = propex::parse(expr)?;
        if segs.is_empty() {
            return Err(crate::EdgelinkError::BadArguments(format!(
                "Cannot parse the property accessing expression: {}",
                expr
            ))
            .into());
        }

        // The first level of the property expression for 'msg' must be a string, which means it must be
        // `msg['aaa']` or `msg.aaa`, and not `msg[12]`
        if let Some(PropexSegment::StringIndex(first_prop_name)) = segs.first() {
            if let Some(first_prop) = self.get_property_mut(first_prop_name) {
                match segs.len() {
                    1 => {
                        *first_prop = value;
                        Ok(())
                    }
                    _ => {
                        match (
                            first_prop.get_item_by_propex_segments_mut(&segs[1..]),
                            create_missing,
                        ) {
                            (Some(pv), _) => {
                                *pv = value;
                                Ok(())
                            }
                            (None, true) => first_prop
                                .set_property_by_propex_segments(&segs[1..], value, true)
                                .map_err(|e| e.into()),
                            (None, false) => Err(crate::EdgelinkError::InvalidOperation(
                                "Failed to set property".into(),
                            )
                            .into()),
                        }
                    }
                }
            } else {
                Err(crate::EdgelinkError::BadArguments(
                    "The first property must be a string".into(),
                )
                .into())
            }
        } else {
            Err(crate::EdgelinkError::BadArguments(format!(
                "The first property to access `Msg` must be a string, got '{}'",
                expr
            ))
            .into())
        }
    }

    pub fn set_trimmed_nav_property(
        &mut self,
        expr: &str,
        value: Variant,
        create_missing: bool,
    ) -> crate::Result<()> {
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
            let prop_name = k
                .into_atom(ctx)
                .map_err(|e| EdgelinkError::InvalidData(e.to_string()))?;

            let prop_value = v
                .as_js_value(ctx)
                .map_err(|e| EdgelinkError::InvalidData(e.to_string()))?;

            obj.set(prop_name, prop_value)
                .map_err(|e| EdgelinkError::InvalidData(e.to_string()))?;
        }

        {
            let link_source_atom = "_linkSource".into_js(ctx)?;
            let link_source_bytes = bincode::serialize(&self.link_call_stack)?;
            let link_source_value = link_source_bytes
                .into_js(ctx)
                .map_err(|e| EdgelinkError::InvalidData(e.to_string()))?;
            obj.set(link_source_atom, link_source_value)
                .map_err(|e| EdgelinkError::InvalidData(e.to_string()))?;

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
        Self {
            birth_place: self.birth_place,
            link_call_stack: self.link_call_stack.clone(),
            body: self.body.clone(),
        }
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

#[cfg(feature = "js")]
impl<'js> From<&js::Object<'js>> for Msg {
    fn from(jo: &js::Object<'js>) -> Self {
        let mut map = BTreeMap::new();
        let mut birth_place = None;
        let mut link_call_stack = None;
        for result in jo.props::<String, js::Value>() {
            match result {
                Ok((k, v)) => match k.as_ref() {
                    "_birth_place" => {
                        birth_place = v
                            .as_string()
                            .and_then(|x| x.to_string().ok())
                            .and_then(|x| x.parse().ok())
                    }
                    "_linkSource" => {
                        let bytes: Vec<u8> = v.get().unwrap();
                        link_call_stack =
                            bincode::deserialize::<Option<Vec<LinkCallStackEntry>>>(&bytes)
                                .unwrap();
                    }
                    _ => {
                        map.insert(k, Variant::from(&v));
                    }
                },
                Err(e) => {
                    eprintln!("Error occurred: {:?}", e);
                    panic!();
                }
            }
        }

        Msg {
            /*
            id: msg_id
                .and_then(|hex_str| hex_str.parse().ok())
                .unwrap_or(ElementId::new()),
                */
            birth_place: birth_place.unwrap_or(ElementId::empty()),
            body: map,
            link_call_stack,
        }
    }
}

impl serde::Serialize for Msg {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut map = serializer.serialize_map(None)?;
        map.serialize_entry("_birth_place", &self.birth_place)?;
        map.serialize_entry("_linkSource", &self.link_call_stack)?;
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
                let mut birth_place = None;
                let mut link_call_stack = None;
                let mut body: BTreeMap<String, Variant> = BTreeMap::new();

                while let Some(key) = map.next_key()? {
                    match key {
                        "_birth_place" => {
                            if birth_place.is_some() {
                                return Err(de::Error::duplicate_field("_birth_place"));
                            }
                            birth_place = Some(map.next_value()?);
                        }
                        "_linkSource" => {
                            if link_call_stack.is_some() {
                                return Err(de::Error::duplicate_field("_linkSource"));
                            }
                            link_call_stack = Some(map.next_value()?);
                        }
                        _ => {
                            let value = map.next_value()?;
                            body.insert(key.to_string(), value);
                        }
                    }
                }

                Ok(Msg {
                    birth_place: birth_place.unwrap_or_default(),
                    body,
                    link_call_stack,
                })
            }
        }

        deserializer.deserialize_map(MsgVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;
    use serde_json::json;

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

        msg.set_nav_property("foo.bar", "changed2".into(), false)
            .unwrap();
        assert_eq!(
            msg.get_property("foo")
                .unwrap()
                .as_object()
                .unwrap()
                .get("bar")
                .unwrap()
                .as_str()
                .unwrap(),
            "changed2"
        );

        assert!(msg
            .set_nav_property("foo.new_field", "new_value".into(), false)
            .is_err());

        assert!(msg
            .set_nav_property("foo.new_new_field", "new_new_value".into(), true)
            .is_ok());

        assert_eq!(
            msg.get_property("foo")
                .unwrap()
                .as_object()
                .unwrap()
                .get("new_new_field")
                .unwrap()
                .as_str()
                .unwrap(),
            "new_new_value"
        );
    }
}
