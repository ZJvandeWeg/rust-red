use rquickjs::{self as js, prelude::*};
use std::collections::BTreeMap;

use std::sync::Arc;

use tokio::sync::RwLock;

use crate::runtime::model::*;
use crate::EdgeLinkError;

use super::{
    propex::{self, PropexSegment},
    ElementId,
};

pub struct Envelope {
    pub port: usize,
    pub msg: Arc<RwLock<Msg>>,
}

pub type MsgBody = BTreeMap<String, Variant>;

#[derive(Debug, Clone)]
pub struct LinkSourceEntry {
    pub id: ElementId,
    pub link_call_node_id: ElementId,
}

#[derive(Debug)]
pub struct Msg {
    pub id: ElementId,
    pub birth_place: ElementId,
    pub body: BTreeMap<String, Variant>,
    pub link_source: Option<Vec<LinkSourceEntry>>,
}

impl Msg {
    pub fn new_default(birth_place: ElementId) -> Arc<RwLock<Self>> {
        let msg = Msg {
            id: Msg::generate_id(),
            birth_place,
            link_source: None,
            body: BTreeMap::from([("payload".to_string(), Variant::Null)]),
        };
        Arc::new(RwLock::new(msg))
    }

    pub fn new_with_body(
        birth_place: ElementId,
        body: BTreeMap<String, Variant>,
    ) -> Arc<RwLock<Self>> {
        let msg = Msg {
            id: Msg::generate_id(),
            birth_place,
            link_source: None,
            body,
        };
        Arc::new(RwLock::new(msg))
    }

    pub fn new_with_payload(birth_place: ElementId, payload: Variant) -> Arc<RwLock<Self>> {
        let msg = Msg {
            id: Msg::generate_id(),
            birth_place,
            link_source: None,
            body: BTreeMap::from([("payload".to_string(), payload)]),
        };
        Arc::new(RwLock::new(msg))
    }

    pub fn generate_id() -> ElementId {
        ElementId::new()
    }

    pub fn get_property(&self, prop: &str) -> Option<&Variant> {
        self.body.get(prop)
    }

    pub fn get_property_mut(&mut self, prop: &str) -> Option<&mut Variant> {
        self.body.get_mut(prop)
    }

    pub fn get_nav_property(&self, expr: &str) -> Option<&Variant> {
        if let Ok(segs) = propex::parse(expr) {
            match segs[0] {
                // The first level of the property expression for 'msg' must be a string, which means it must be
                // `msg['aaa']` or `msg.aaa`, and not `msg[12]`
                PropexSegment::StringIndex(first_prop_name) => {
                    if let Some(first_prop) = self.get_property(first_prop_name) {
                        if segs.len() == 1 {
                            Some(first_prop)
                        } else {
                            first_prop.get_item_by_propex_segments(&segs[1..])
                        }
                    } else {
                        None
                    }
                }
                _ => None,
            }
        } else {
            None
        }
    }

    pub fn get_nav_property_mut(&mut self, expr: &str) -> Option<&mut Variant> {
        if let Ok(segs) = propex::parse(expr) {
            match segs[0] {
                // The first level of the property expression for 'msg' must be a string, which means it must be
                // `msg['aaa']` or `msg.aaa`, and not `msg[12]`
                PropexSegment::StringIndex(first_prop_name) => {
                    if let Some(first_prop) = self.get_property_mut(first_prop_name) {
                        if segs.len() == 1 {
                            Some(first_prop)
                        } else {
                            first_prop.get_item_by_propex_segments_mut(&segs[1..])
                        }
                    } else {
                        None
                    }
                }
                _ => None,
            }
        } else {
            None
        }
    }

    pub fn get_trimmed_nav_property_mut(&mut self, expr: &str) -> Option<&mut Variant> {
        let trimmed_expr = expr.trim();
        if let Some(stripped_expr) = trimmed_expr.strip_prefix("msg.") {
            self.get_nav_property_mut(stripped_expr)
        } else {
            self.get_nav_property_mut(trimmed_expr)
        }
    }

    pub fn get_trimmed_nav_property(&self, expr: &str) -> Option<&Variant> {
        let trimmed_expr = expr.trim();
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
        if let Ok(segs) = propex::parse(expr) {
            match segs[0] {
                // The first level of the property expression for 'msg' must be a string, which means it must be
                // `msg['aaa']` or `msg.aaa`, and not `msg[12]`
                PropexSegment::StringIndex(first_prop_name) => {
                    if let Some(first_prop) = self.get_property_mut(first_prop_name) {
                        if segs.len() == 1 {
                            *first_prop = value;
                            Ok(())
                        } else if let Some(pv) =
                            first_prop.get_item_by_propex_segments_mut(&segs[1..])
                        {
                            Ok(pv.set_property_by_propex_segments(
                                &segs[1..],
                                value,
                                create_missing,
                            )?)
                        } else {
                            Err(crate::EdgeLinkError::BadArguments("Bad propex".into()).into())
                        }
                    } else {
                        Err(crate::EdgeLinkError::BadArguments(
                            "The first property must be a string".into(),
                        )
                        .into())
                    }
                }
                _ => Err(crate::EdgeLinkError::BadArguments(
                    "The first property must be a string".into(),
                )
                .into()),
            }
        } else {
            Err(
                crate::EdgeLinkError::BadArguments("The first property must be a string".into())
                    .into(),
            )
        }
    }

    pub fn set_trimmed_nav_property(
        &mut self,
        expr: &str,
        value: Variant,
        create_missing: bool,
    ) -> crate::Result<()> {
        let trimmed_expr = expr.trim();
        if let Some(stripped_expr) = trimmed_expr.strip_prefix("msg.") {
            self.set_nav_property(stripped_expr, value, create_missing)
        } else {
            self.set_nav_property(trimmed_expr, value, create_missing)
        }
    }

    pub fn as_js_object<'js>(&self, ctx: &js::context::Ctx<'js>) -> crate::Result<js::Object<'js>> {
        let obj = js::Object::new(ctx.clone())?;
        for (k, v) in self.body.iter() {
            let prop_name = k
                .into_atom(ctx)
                .map_err(|e| EdgeLinkError::InvalidData(e.to_string()))?;

            let prop_value = v
                .as_js_value(ctx)
                .map_err(|e| EdgeLinkError::InvalidData(e.to_string()))?;

            obj.set(prop_name, prop_value)
                .map_err(|e| EdgeLinkError::InvalidData(e.to_string()))?;
        }

        {
            let msg_id_atom = "_msgid"
                .into_atom(ctx)
                .map_err(|e| EdgeLinkError::InvalidData(e.to_string()))?;
            let msg_id_value = self
                .id
                .to_string()
                .into_js(ctx)
                .map_err(|e| EdgeLinkError::InvalidData(e.to_string()))?;
            obj.set(msg_id_atom, msg_id_value)
                .map_err(|e| EdgeLinkError::InvalidData(e.to_string()))?;

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

    pub fn as_js_value<'js>(&self, ctx: &js::Ctx<'js>) -> crate::Result<js::Value<'js>> {
        Ok(js::Value::from_object(self.as_js_object(ctx)?))
    }
}

impl Msg {
    pub fn push_link_source(&mut self, lse: LinkSourceEntry) {
        if let Some(link_source) = &mut self.link_source {
            link_source.push(lse);
        } else {
            self.link_source = Some(vec![lse]);
        }
    }

    pub fn pop_link_source(&mut self) -> Option<LinkSourceEntry> {
        if let Some(link_source) = &mut self.link_source {
            link_source.pop()
        } else {
            None
        }
    }
}

impl Clone for Msg {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            birth_place: self.birth_place,
            link_source: self.link_source.clone(),
            body: self.body.clone(),
        }
    }
}

impl<'js> From<&js::Object<'js>> for Msg {
    fn from(jo: &js::Object<'js>) -> Self {
        let mut map = BTreeMap::new();
        let mut birth_place = None;
        let mut msg_id = None;
        let mut link_source = None;
        for result in jo.props::<String, js::Value>() {
            match result {
                Ok((k, v)) => match k.as_ref() {
                    "_msgid" => msg_id = v.as_string().and_then(|x| x.to_string().ok()),
                    "_birth_place" => {
                        birth_place = v
                            .as_string()
                            .and_then(|x| x.to_string().ok())
                            .and_then(|x| x.parse().ok())
                    }
                    "_linkSource" => {
                        todo!()
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
            id: msg_id
                .and_then(|hex_str| hex_str.parse().ok())
                .unwrap_or(ElementId::new()),

            birth_place: birth_place.unwrap_or(ElementId::empty()),
            body: map,
            link_source,
        }
    }
}
