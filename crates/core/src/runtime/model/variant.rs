use std::borrow::BorrowMut;
use std::collections::BTreeMap;
use std::convert::TryInto;
use std::fmt;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::ser::{SerializeMap, SerializeSeq};
use serde::{self, de, Deserializer};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::runtime::model::propex;
use crate::EdgelinkError;

use super::propex::PropexSegment;

#[cfg(feature = "js")]
mod js {
    pub use rquickjs::*;
}

#[derive(Error, Clone, Debug, PartialEq, PartialOrd)]
pub enum VariantError {
    #[error("Wrong type")]
    WrongType,

    #[error("Out of range error")]
    OutOfRange,

    #[error("Casting error")]
    BadCast,
}

/// A versatile enum that can represent various types of data.
///
/// This enum is designed to be a flexible container for different kinds of data,
/// including null values, numbers, strings, booleans, byte arrays, arrays of `Variant`
/// values, and key-value mappings.
///
/// # Examples
///
/// ```rust
/// use std::collections::BTreeMap;
/// use edgelink_core::runtime::model::Variant;
///
/// // Create a null variant
/// let null_variant = Variant::Null;
///
/// // Create a rational variant
/// let rational_variant = Variant::Rational(3.14);
///
/// // Create an integer variant
/// let integer_variant = Variant::Integer(42);
/// assert_eq!(integer_variant.as_integer().unwrap(), 42);
/// ```
#[non_exhaustive]
#[derive(Default, Clone, Debug, PartialEq, PartialOrd)]
pub enum Variant {
    /// Represents a null value.
    #[default]
    Null,

    /// Represents a floating-point number.
    Rational(f64),

    /// Represents a 32-bit signed integer.
    Integer(i32),

    /// Represents a string of characters.
    String(String),

    /// Represents a boolean value (true or false).
    Bool(bool),

    /// Represents a Date value (timestamp inside).
    Date(SystemTime),

    /// Represents a sequence of bytes.
    Bytes(Vec<u8>),

    /// Represents an array of `Variant` values.
    Array(Vec<Variant>),

    /// Represents a key-value mapping of strings to `Variant` values.
    Object(BTreeMap<String, Variant>),
}

impl Variant {
    pub fn empty_object() -> Variant {
        Variant::Object(BTreeMap::new())
    }

    pub fn empty_array() -> Variant {
        Variant::Array(Vec::<Variant>::new())
    }

    pub fn now() -> Variant {
        Variant::Date(SystemTime::now())
    }

    pub fn bytes_from_json_value(jv: &serde_json::Value) -> crate::Result<Variant> {
        match jv {
            serde_json::Value::Array(array) => {
                let mut bytes = Vec::with_capacity(array.len());
                for e in array.iter() {
                    if let Some(byte) = e.as_i64() {
                        if !(0..=0xFF).contains(&byte) {
                            return Err(EdgelinkError::NotSupported(
                                "Invalid byte value".to_owned(),
                            )
                            .into());
                        }
                        bytes.push(byte as u8)
                    } else {
                        return Err(EdgelinkError::NotSupported(
                            "Invalid byte JSON value type".to_owned(),
                        )
                        .into());
                    }
                }
                Ok(Variant::Bytes(bytes))
            }
            serde_json::Value::String(string) => Ok(Variant::from(string.as_bytes())),
            _ => Err(EdgelinkError::NotSupported("Invalid byte JSON Value".to_owned()).into()),
        }
    }

    pub fn bytes_from_vec(vec: &[Variant]) -> crate::Result<Variant> {
        let mut bytes: Vec<u8> = Vec::with_capacity(vec.len());
        for v in vec.iter() {
            match v {
                Variant::Rational(f) if *f >= 0.0 && *f <= 255.0 => bytes.push(*f as u8),
                Variant::Integer(i) if *i >= 0 && *i <= 255 => bytes.push(*i as u8),
                _ => {
                    return Err(
                        EdgelinkError::NotSupported("Unsupported Variant type".into()).into(),
                    );
                }
            }
        }
        Ok(Variant::Bytes(bytes))
    }

    pub fn is_bytes(&self) -> bool {
        matches!(self, Variant::Bytes(..))
    }

    pub fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            Variant::Bytes(ref bytes) => Some(bytes),
            Variant::String(ref s) => Some(s.as_bytes()),
            _ => None,
        }
    }

    pub fn to_bytes(&self) -> Option<Vec<u8>> {
        match self {
            Variant::Bytes(ref bytes) => Some(bytes.clone()),
            Variant::String(ref s) => Some(s.bytes().collect()),
            Variant::Array(ref arr) => arr
                .iter()
                .flat_map(|x| x.to_u8())
                .next()
                .map(|_| arr.iter().filter_map(|x| x.to_u8()).collect()),
            Variant::Rational(f) => Some(f.to_string().bytes().collect()),
            Variant::Integer(i) => Some(i.to_string().bytes().collect()),
            _ => None,
        }
    }

    pub fn as_bytes_mut(&mut self) -> Option<&mut Vec<u8>> {
        match self {
            Variant::Bytes(ref mut bytes) => Some(bytes),
            _ => None,
        }
    }

    pub fn into_bytes(self) -> Result<Vec<u8>, Self> {
        match self {
            Variant::Bytes(vec) => Ok(vec),
            other => Err(other),
        }
    }

    pub fn to_u8(&self) -> Option<u8> {
        match self {
            Variant::Rational(value) => {
                if value.is_nan() || *value < u8::MIN as f64 || *value > u8::MAX as f64 {
                    None
                } else {
                    let rounded = value.round();
                    if rounded < u8::MIN as f64 || rounded > u8::MAX as f64 {
                        None
                    } else {
                        Some(rounded as u8)
                    }
                }
            }
            Variant::Integer(ivalue) => (*ivalue).try_into().ok(),
            _ => None,
        }
    }

    pub fn is_rational(&self) -> bool {
        matches!(self, Variant::Rational(..) | Variant::Integer(..))
    }

    pub fn as_rational(&self) -> Option<f64> {
        match *self {
            Variant::Rational(f) => Some(f),
            Variant::Integer(f) => Some(f as f64),
            _ => None,
        }
    }

    pub fn into_number(self) -> Result<f64, Self> {
        match self {
            Variant::Rational(f) => Ok(f),
            Variant::Integer(f) => Ok(f as f64),
            other => Err(other),
        }
    }

    pub fn is_integer(&self) -> bool {
        matches!(self, Variant::Integer(..))
    }

    pub fn as_integer(&self) -> Option<i32> {
        match *self {
            Variant::Integer(f) => Some(f),
            _ => None,
        }
    }

    pub fn into_integer(self) -> Result<i32, Self> {
        match self {
            Variant::Integer(v) => Ok(v),
            other => Err(other),
        }
    }

    pub fn is_string(&self) -> bool {
        matches!(self, Variant::String(..))
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Variant::String(ref s) => Some(s),
            _ => None,
        }
    }

    pub fn as_str_mut(&mut self) -> Option<&mut String> {
        match self {
            Variant::String(ref mut s) => Some(s),
            _ => None,
        }
    }

    pub fn to_string(&self) -> Result<String, VariantError> {
        match self {
            Variant::String(s) => Ok(s.clone()),
            Variant::Rational(f) => Ok(f.to_string()),
            Variant::Integer(i) => Ok(i.to_string()),
            _ => Err(VariantError::WrongType),
        }
    }

    pub fn is_bool(&self) -> bool {
        matches!(self, Variant::Bool(..))
    }

    pub fn as_bool(&self) -> Option<bool> {
        match *self {
            Variant::Bool(b) => Some(b),
            _ => None,
        }
    }

    pub fn into_bool(self) -> Result<bool, Self> {
        match self {
            Variant::Bool(b) => Ok(b),
            other => Err(other),
        }
    }

    pub fn is_null(&self) -> bool {
        matches!(self, Variant::Null)
    }

    pub fn is_array(&self) -> bool {
        matches!(self, Variant::Array(..))
    }

    pub fn as_array(&self) -> Option<&Vec<Variant>> {
        match self {
            Variant::Array(ref array) => Some(array),
            _ => None,
        }
    }

    pub fn as_array_mut(&mut self) -> Option<&mut Vec<Variant>> {
        match self {
            Variant::Array(ref mut list) => Some(list),
            _ => None,
        }
    }

    pub fn into_array(self) -> Result<Vec<Variant>, Self> {
        match self {
            Variant::Array(vec) => Ok(vec),
            other => Err(other),
        }
    }

    pub fn is_object(&self) -> bool {
        matches!(self, Variant::Object(..))
    }

    pub fn as_object(&self) -> Option<&BTreeMap<String, Variant>> {
        match self {
            Variant::Object(ref object) => Some(object),
            _ => None,
        }
    }

    pub fn as_object_mut(&mut self) -> Option<&mut BTreeMap<String, Variant>> {
        match self {
            Variant::Object(ref mut object) => Some(object),
            _ => None,
        }
    }

    pub fn into_object(self) -> Result<BTreeMap<String, Variant>, Self> {
        match self {
            Variant::Object(object) => Ok(object),
            other => Err(other),
        }
    }

    pub fn len(&self) -> usize {
        match self {
            Variant::Null => 0,
            Variant::Object(object) => object.len(),
            Variant::Array(array) => array.len(),
            Variant::Bytes(bytes) => bytes.len(),
            Variant::String(s) => s.len(),
            _ => 1,
        }
    }

    pub fn is_empty(&self) -> bool {
        match self {
            Variant::Null => true,
            Variant::Object(object) => object.is_empty(),
            Variant::Array(array) => array.is_empty(),
            Variant::Bytes(bytes) => bytes.is_empty(),
            Variant::String(s) => s.is_empty(),
            Variant::Rational(f) => f.is_nan(),
            _ => false,
        }
    }

    pub fn get_item_by_propex_segment(&self, pseg: &PropexSegment) -> Option<&Variant> {
        match pseg {
            PropexSegment::IntegerIndex(index) => self.get_array_item(*index),
            PropexSegment::StringIndex(prop) => self.get_object_property(prop),
        }
    }

    pub fn get_item_by_propex_segment_mut(&mut self, pseg: &PropexSegment) -> Option<&mut Variant> {
        match pseg {
            PropexSegment::IntegerIndex(index) => self.get_array_item_mut(*index),
            PropexSegment::StringIndex(prop) => self.get_object_property_mut(prop),
        }
    }

    pub fn get_item_by_propex_segments(&self, psegs: &[PropexSegment]) -> Option<&Variant> {
        match psegs.len() {
            0 => None,
            1 => self.get_item_by_propex_segment(&psegs[0]),
            _ => {
                let mut prev = self;
                for pseg in psegs {
                    if let Some(cur) = prev.get_item_by_propex_segment(pseg) {
                        prev = cur;
                    } else {
                        return None;
                    }
                }
                Some(prev)
            }
        }
    }

    pub fn get_item_by_propex_segments_mut(
        &mut self,
        psegs: &[PropexSegment],
    ) -> Option<&mut Variant> {
        match psegs.len() {
            0 => None,
            1 => self.get_item_by_propex_segment_mut(&psegs[0]),
            _ => {
                let mut prev = self;
                for pseg in psegs {
                    if let Some(cur) = prev.get_item_by_propex_segment_mut(pseg) {
                        prev = cur;
                    } else {
                        return None;
                    }
                }
                Some(prev)
            }
        }
    }

    pub fn get_object_property(&self, prop: &str) -> Option<&Variant> {
        match self {
            Variant::Object(obj) => obj.get(prop),
            _ => None,
        }
    }

    pub fn get_object_property_mut(&mut self, prop: &str) -> Option<&mut Variant> {
        match self {
            Variant::Object(obj) => obj.get_mut(prop),
            _ => None,
        }
    }

    pub fn get_array_item(&self, index: usize) -> Option<&Variant> {
        match self {
            Variant::Array(arr) => arr.get(index),
            _ => None,
        }
    }

    pub fn get_array_item_mut(&mut self, index: usize) -> Option<&mut Variant> {
        match self {
            Variant::Array(arr) => arr.get_mut(index),
            _ => None,
        }
    }

    pub fn get_object_nav_property(&self, expr: &str) -> Option<&Variant> {
        let prop_segs = propex::parse(expr).ok()?;
        self.get_item_by_propex_segments(&prop_segs)
    }

    pub fn set_object_property(
        &mut self,
        prop: String,
        value: Variant,
    ) -> Result<(), VariantError> {
        match self {
            Variant::Object(ref mut this_obj) => {
                this_obj.insert(prop, value);
                Ok(())
            }
            _ => {
                log::warn!(
                    "Only an object variant can be set the property '{}' to '{:?}', instead this variant is:\n{:?}",
                    prop, value, self);
                Err(VariantError::WrongType)
            }
        }
    }

    pub fn set_array_item(&mut self, index: usize, value: Variant) -> Result<(), VariantError> {
        match self {
            Variant::Array(ref mut this_arr) => {
                if let Some(existed) = this_arr.get_mut(index) {
                    *existed = value;
                    Ok(())
                } else if index == this_arr.len() {
                    // insert to tail
                    this_arr.push(value);
                    Ok(())
                } else {
                    Err(VariantError::OutOfRange)
                }
            }
            Variant::Bytes(ref mut this_bytes) => {
                if let Some(existed) = this_bytes.get_mut(index) {
                    *existed = value.to_u8().ok_or(VariantError::BadCast)?;
                    Ok(())
                } else if index == this_bytes.len() {
                    // insert to tail
                    this_bytes.push(value.to_u8().ok_or(VariantError::BadCast)?);
                    Ok(())
                } else {
                    Err(VariantError::OutOfRange)
                }
            }
            _ => Err(VariantError::WrongType),
        }
    }

    pub fn set_property_by_propex_segment(
        &mut self,
        pseg: &PropexSegment,
        value: Variant,
    ) -> Result<(), VariantError> {
        match pseg {
            PropexSegment::IntegerIndex(index) => self.set_array_item(*index, value),
            PropexSegment::StringIndex(prop) => self.set_object_property(prop.to_string(), value),
        }
    }

    pub fn set_property_by_propex_segments(
        &mut self,
        psegs: &[PropexSegment],
        value: Variant,
        create_missing: bool,
    ) -> Result<(), VariantError> {
        if psegs.is_empty() {
            return Err(VariantError::OutOfRange);
        }

        if psegs.len() == 1 {
            self.set_property_by_propex_segment(&psegs[0], value)?;
            return Ok(());
        }

        for nlevel in 0..psegs.len() - 1 {
            let psegs_slice = &psegs[..=nlevel];
            let pseg = &psegs[nlevel];

            {
                let cur = self.get_item_by_propex_segments(psegs_slice);
                if cur.is_some() {
                    continue;
                }
            }

            if create_missing {
                if let Some(next_pseg) = psegs.get(nlevel + 1) {
                    let mut prev = self.borrow_mut();
                    if nlevel > 0 {
                        prev = self
                            .get_item_by_propex_segments_mut(&psegs[0..=nlevel - 1])
                            .ok_or(VariantError::OutOfRange)?;
                    }
                    match next_pseg {
                        PropexSegment::StringIndex(_) => {
                            prev.set_property_by_propex_segment(pseg, Variant::empty_object())?
                        }
                        PropexSegment::IntegerIndex(_) => {
                            prev.set_property_by_propex_segment(pseg, Variant::empty_array())?
                        }
                    }
                } else {
                    return Err(VariantError::OutOfRange);
                };
            } else {
                return Err(VariantError::OutOfRange);
            }
        }

        // 如果最后一个属性存在
        if let Some(terminal_obj) = self.get_item_by_propex_segments_mut(psegs) {
            *terminal_obj = value;
            Ok(())
        } else if let Some(parent_obj) =
            self.get_item_by_propex_segments_mut(&psegs[0..=psegs.len() - 2])
        {
            // 没有就获取上一个属性
            parent_obj
                .set_property_by_propex_segment(psegs.last().expect("We're so over"), value)?;
            Ok(())
        } else {
            Err(VariantError::OutOfRange)
        }
    }

    pub fn set_object_nav_property(
        &mut self,
        expr: &str,
        value: Variant,
        create_missing: bool,
    ) -> Result<(), VariantError> {
        if let Ok(prop_segs) = propex::parse(expr) {
            self.set_property_by_propex_segments(&prop_segs, value, create_missing)
        } else {
            Err(VariantError::OutOfRange)
        }
    }

    #[cfg(feature = "js")]
    pub fn as_js_value<'js>(&self, ctx: &js::context::Ctx<'js>) -> crate::Result<js::Value<'js>> {
        use js::IntoJs;
        match self {
            Variant::Array(_) => Ok(js::Value::from_array(self.as_js_array(ctx)?)),

            Variant::Bool(b) => Ok(js::Value::new_bool(ctx.clone(), *b)),

            Variant::Bytes(bytes) => {
                Ok(js::ArrayBuffer::new_copy(ctx.clone(), bytes)?.into_value())
            }

            Variant::Integer(i) => Ok(js::Value::new_int(ctx.clone(), *i)),

            Variant::Null => Ok(js::Value::new_null(ctx.clone())),

            Variant::Object(_) => Ok(js::Value::from_object(self.as_js_object(ctx)?)),

            Variant::String(s) => s.into_js(ctx).map_err(|e| e.into()),

            Variant::Rational(f) => f.into_js(ctx).map_err(|e| e.into()),

            Variant::Date(t) => t.into_js(ctx).map_err(|e| e.into()),
        }
    }

    #[cfg(feature = "js")]
    pub fn as_js_array<'js>(&self, ctx: &js::Ctx<'js>) -> crate::Result<js::Array<'js>> {
        use js::FromIteratorJs;
        if let Variant::Array(items) = self {
            let iter = items.iter().map(|e| e.as_js_value(ctx).unwrap()); // TODO FIXME
            js::Array::from_iter_js(ctx, iter)
                .map_err(|e| EdgelinkError::InvalidData(e.to_string()).into())
        } else {
            Err(crate::EdgelinkError::InvalidOperation("Bad variant type".to_string()).into())
        }
    }

    #[cfg(feature = "js")]
    pub fn as_js_object<'js>(&self, ctx: &js::context::Ctx<'js>) -> crate::Result<js::Object<'js>> {
        use js::IntoAtom;
        if let Variant::Object(map) = self {
            let obj = js::Object::new(ctx.clone())?;
            for (k, v) in map {
                let prop_name = k
                    .into_atom(ctx)
                    .map_err(|e| EdgelinkError::InvalidData(e.to_string()))?;

                let prop_value = v
                    .as_js_value(ctx)
                    .map_err(|e| EdgelinkError::InvalidData(e.to_string()))?;

                obj.set(prop_name, prop_value)
                    .map_err(|e| EdgelinkError::InvalidData(e.to_string()))?;
            }
            Ok(obj)
        } else {
            Err(crate::EdgelinkError::InvalidOperation("Bad variant type".to_string()).into())
        }
    }
} // struct Variant

impl From<&Variant> for String {
    #[inline]
    fn from(var: &Variant) -> Self {
        match var {
            Variant::Integer(i) => i.to_string(),
            Variant::Rational(f) => f.to_string(),
            Variant::String(s) => s.clone(),
            _ => "".to_string(),
        }
    }
}

macro_rules! implfrom {
    ($($v:ident($t:ty)),+ $(,)?) => {
        $(
            impl From<$t> for Variant {
                #[inline]
                fn from(value: $t) -> Self {
                    Self::$v(value.into())
                }
            }
        )+
    };
}

implfrom! {
    Integer(i32),
    Integer(u16),
    Integer(i16),
    Integer(u8),
    Integer(i8),

    Bytes(Vec<u8>),

    Rational(f32),
    Rational(f64),

    String(String),
    String(&str),

    Bool(bool),

    Array(&[Variant]),
    Array(Vec<Variant>),

    // Object(&[(String, Variant)]),
    // Object(&[(&str, Variant)]),
    Object(BTreeMap<String, Variant>),
    // Object(&BTreeMap<String, Variant>),
    // Object(BTreeMap<&str, Variant>),
}

impl From<char> for Variant {
    #[inline]
    fn from(value: char) -> Self {
        Variant::String(value.to_string())
    }
}

impl From<&[(String, Variant)]> for Variant {
    #[inline]
    fn from(value: &[(String, Variant)]) -> Self {
        let map: BTreeMap<String, Variant> =
            value.iter().map(|x| (x.0.clone(), x.1.clone())).collect();
        Variant::Object(map)
    }
}

impl<const N: usize> From<[(&str, Variant); N]> for Variant {
    #[inline]
    fn from(value: [(&str, Variant); N]) -> Self {
        let map: BTreeMap<String, Variant> = value
            .iter()
            .map(|x| (x.0.to_string(), x.1.clone()))
            .collect();
        Variant::Object(map)
    }
}

impl From<&[u8]> for Variant {
    fn from(array: &[u8]) -> Self {
        Variant::Bytes(array.to_vec())
    }
}

impl Serialize for Variant {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Variant::Null => serializer.serialize_none(),
            Variant::Rational(v) => serializer.serialize_f64(*v),
            Variant::Integer(v) => serializer.serialize_i32(*v),
            Variant::String(v) => serializer.serialize_str(v),
            Variant::Bool(v) => serializer.serialize_bool(*v),
            Variant::Bytes(v) => serializer.serialize_bytes(v),
            Variant::Date(v) => {
                let ts = v
                    .duration_since(UNIX_EPOCH)
                    .map_err(serde::ser::Error::custom)?;
                serializer.serialize_u64(ts.as_millis() as u64)
            }
            Variant::Array(v) => {
                let mut seq = serializer.serialize_seq(Some(v.len()))?;
                for item in v {
                    seq.serialize_element(item)?;
                }
                seq.end()
            }
            Variant::Object(v) => {
                let mut map = serializer.serialize_map(Some(v.len()))?;
                for (k, v) in v {
                    map.serialize_entry(k, v)?;
                }
                map.end()
            }
        }
    }
}

impl From<serde_json::Value> for Variant {
    fn from(jv: serde_json::Value) -> Self {
        match jv {
            serde_json::Value::Null => Variant::Null,
            serde_json::Value::Bool(boolean) => Variant::from(boolean),
            serde_json::Value::Number(number) => {
                //FIXME TODO
                Variant::Rational(number.as_f64().unwrap_or(f64::NAN))
            }
            serde_json::Value::String(string) => Variant::String(string.to_owned()),
            serde_json::Value::Array(array) => {
                Variant::Array(array.iter().map(Variant::from).collect())
            }
            serde_json::Value::Object(object) => {
                let new_map: BTreeMap<String, Variant> = object
                    .iter()
                    .map(|(k, v)| (k.to_owned(), Variant::from(v)))
                    .collect();
                Variant::Object(new_map)
            }
        }
    }
}

impl From<&serde_json::Value> for Variant {
    fn from(jv: &serde_json::Value) -> Self {
        match jv {
            serde_json::Value::Null => Variant::Null,
            serde_json::Value::Bool(boolean) => Variant::from(*boolean),
            serde_json::Value::Number(number) => {
                // FIXME TODO
                Variant::Rational(number.as_f64().unwrap_or(f64::NAN))
            }
            serde_json::Value::String(string) => Variant::String(string.clone()),
            serde_json::Value::Array(array) => {
                Variant::Array(array.iter().map(Variant::from).collect())
            }
            serde_json::Value::Object(object) => {
                let new_map: BTreeMap<String, Variant> = object
                    .iter()
                    .map(|(k, v)| (k.clone(), Variant::from(v)))
                    .collect();
                Variant::Object(new_map)
            }
        }
    }
}

impl<'de> Deserialize<'de> for Variant {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct VariantVisitor;

        impl<'de> de::Visitor<'de> for VariantVisitor {
            type Value = Variant;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                write!(formatter, "a valid Variant value")
            }

            fn visit_unit<E>(self) -> Result<Variant, E>
            where
                E: de::Error,
            {
                Ok(Variant::Null)
            }

            fn visit_bool<E>(self, value: bool) -> Result<Variant, E>
            where
                E: de::Error,
            {
                Ok(Variant::Bool(value))
            }

            fn visit_i64<E>(self, value: i64) -> Result<Variant, E>
            where
                E: de::Error,
            {
                if value > i32::MAX.into() || value < i32::MIN.into() {
                    Ok(Variant::Rational(value as f64))
                } else {
                    Ok(Variant::Integer(value as i32))
                }
            }

            fn visit_u64<E>(self, value: u64) -> Result<Variant, E>
            where
                E: de::Error,
            {
                if value > (i32::MAX as u64) {
                    Ok(Variant::Rational(value as f64))
                } else {
                    Ok(Variant::Integer(value as i32))
                }
            }

            fn visit_f64<E>(self, value: f64) -> Result<Variant, E>
            where
                E: de::Error,
            {
                Ok(Variant::Rational(value))
            }

            fn visit_str<E>(self, value: &str) -> Result<Variant, E>
            where
                E: de::Error,
            {
                Ok(Variant::String(value.to_owned()))
            }

            fn visit_bytes<E>(self, value: &[u8]) -> Result<Variant, E>
            where
                E: de::Error,
            {
                Ok(Variant::Bytes(value.to_vec()))
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Variant, A::Error>
            where
                A: de::SeqAccess<'de>,
            {
                let mut vec = Vec::new();
                while let Some(item) = seq.next_element()? {
                    vec.push(item);
                }
                Ok(Variant::Array(vec))
            }

            fn visit_map<A>(self, mut map: A) -> Result<Variant, A::Error>
            where
                A: de::MapAccess<'de>,
            {
                let mut btreemap = BTreeMap::new();
                while let Some((key, value)) = map.next_entry()? {
                    btreemap.insert(key, value);
                }
                Ok(Variant::Object(btreemap))
            }
        }

        deserializer.deserialize_any(VariantVisitor)
    }
}

#[cfg(feature = "js")]
impl<'js> js::FromJs<'js> for Variant {
    fn from_js(_ctx: &js::Ctx<'js>, jv: js::Value<'js>) -> js::Result<Variant> {
        match jv.type_of() {
            js::Type::Undefined => Ok(Variant::Null),

            js::Type::Null => Ok(Variant::Null),

            js::Type::Bool => Ok(Variant::Bool(jv.get()?)),

            js::Type::Int => Ok(Variant::Integer(jv.get()?)),

            js::Type::Float => Ok(Variant::Rational(jv.get()?)),

            js::Type::String => Ok(Variant::String(jv.get()?)),

            js::Type::Symbol => Ok(Variant::String(jv.get()?)),

            js::Type::Array => {
                if let Some(arr) = jv.as_array() {
                    if let Some(buf) = arr.as_array_buffer() {
                        Ok(Variant::Bytes(buf.as_slice()?.into()))
                    } else {
                        let mut vec: Vec<Variant> = Vec::with_capacity(arr.len());
                        for item in arr.iter() {
                            match item {
                                Ok(v) => vec.push(Variant::from_js(_ctx, v)?),
                                Err(err) => {
                                    return Err(err);
                                }
                            }
                        }
                        Ok(Variant::Array(vec))
                    }
                } else {
                    Ok(Variant::Null)
                }
            }

            js::Type::Object => {
                if let Some(jo) = jv.as_object() {
                    let mut map = BTreeMap::new();
                    for result in jo.props::<String, js::Value>() {
                        match result {
                            Ok((ref k, v)) => {
                                map.insert(k.clone(), Variant::from_js(_ctx, v)?);
                            }
                            Err(e) => {
                                eprintln!("Error occurred: {:?}", e);
                                panic!();
                            }
                        }
                    }
                    Ok(Variant::Object(map))
                } else {
                    Err(js::Error::FromJs {
                        from: "JS object",
                        to: "Variant::Object",
                        message: None,
                    })
                }
            }

            _ => Err(js::Error::FromJs {
                from: "Unknown JS type",
                to: "",
                message: None,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::*;

    #[test]
    fn variant_clone_should_be_ok() {
        let var1 = Variant::Array(vec![
            Variant::Integer(123),
            Variant::Integer(333),
            Variant::Array(vec![Variant::Integer(901), Variant::Integer(902)]),
        ]);
        let mut var2 = var1.clone();

        let inner_array = var2.as_array_mut().unwrap()[2].as_array_mut().unwrap();
        inner_array[0] = Variant::Integer(999);

        let value1 = var1.as_array().unwrap()[2].as_array().unwrap()[0]
            .as_integer()
            .unwrap();
        let value2 = var2.as_array().unwrap()[2].as_array().unwrap()[0]
            .as_integer()
            .unwrap();

        assert_eq!(value1, 901);
        assert_eq!(value2, 999);
        assert_ne!(value1, value2);
    }

    #[test]
    fn variant_propex_readonly_accessing_should_be_ok() {
        /*
        let obj = Variant::Object(vec![
            Variant::Integer(123),
            Variant::Integer(333),
            Variant::Array(vec![Variant::Integer(901), Variant::Integer(902)]),
        ]);


        */
        let obj1 = Variant::from([
            ("value1", Variant::Integer(123)),
            ("value2", Variant::Rational(123.0)),
            (
                "value3",
                Variant::from([
                    ("aaa", Variant::Integer(333)),
                    ("bbb", Variant::Integer(444)),
                    ("ccc", Variant::Integer(555)),
                    ("ddd", Variant::Integer(999)),
                ]),
            ),
        ]);

        let value1 = obj1
            .get_object_nav_property("value1")
            .unwrap()
            .as_integer()
            .unwrap();
        assert_eq!(value1, 123);

        let ccc_1 = obj1
            .get_object_nav_property("value3.ccc")
            .unwrap()
            .as_integer()
            .unwrap();
        assert_eq!(ccc_1, 555);

        let ccc_2 = obj1
            .get_object_nav_property("['value3'].ccc")
            .unwrap()
            .as_integer()
            .unwrap();
        assert_eq!(ccc_2, 555);

        let ccc_3 = obj1
            .get_object_nav_property("['value3'][\"ccc\"]")
            .unwrap()
            .as_integer()
            .unwrap();
        assert_eq!(ccc_3, 555);

        let ddd_1 = obj1
            .get_object_nav_property("value3.ddd")
            .unwrap()
            .as_integer()
            .unwrap();
        assert_eq!(ddd_1, 999);
    }

    #[test]
    fn variant_propex_set_nav_property_with_empty_object_should_be_ok() {
        let mut obj1 = Variant::empty_object();

        obj1.set_object_nav_property("address.country", Variant::String("US".to_string()), true)
            .unwrap();
        obj1.set_object_nav_property("address.zip", Variant::String("12345".to_string()), true)
            .unwrap();

        obj1.set_object_nav_property("array_field[0]", Variant::String("11111".to_string()), true)
            .unwrap();
        obj1.set_object_nav_property("array_field[1]", Variant::String("22222".to_string()), true)
            .unwrap();

        let obj_address = obj1.get_object_property("address").unwrap();

        assert!(obj_address.is_object());
        assert_eq!(
            obj_address
                .get_object_property("country")
                .unwrap()
                .as_str()
                .unwrap(),
            "US"
        );
        assert_eq!(
            obj_address
                .get_object_property("zip")
                .unwrap()
                .as_str()
                .unwrap(),
            "12345"
        );

        assert_eq!(obj_address.len(), 2);
    }

    #[test]
    fn variant_can_deserialize_from_json_value() {
        let json = json!(null);
        let var = Variant::deserialize(&json).unwrap();
        assert!(var.is_null());

        let json = json!(3.14);
        let var = Variant::deserialize(&json).unwrap();
        assert!(var.is_rational());
        assert_eq!((var.as_rational().unwrap() * 100.0) as i64, 314);

        let json = json!(123);
        let var = Variant::deserialize(&json).unwrap();
        assert!(var.is_integer());
        assert_eq!(var.as_integer().unwrap(), 123);

        let json = json!("text");
        let var = Variant::deserialize(&json).unwrap();
        assert!(var.is_string());
        assert_eq!(var.as_str().unwrap(), "text");

        let json = json!("text");
        let var = Variant::deserialize(&json).unwrap();
        assert!(var.is_string());
        assert_eq!(var.as_str().unwrap(), "text");

        let json = json!(true);
        let var = Variant::deserialize(&json).unwrap();
        assert!(var.is_bool());
        assert!(var.as_bool().unwrap());

        // JSON does not supporting the ArrayBuffer
        let json = json!([1, 2, 3, 4, 5]);
        let var = Variant::deserialize(&json).unwrap();
        let var = Variant::from(var.to_bytes().unwrap());
        assert!(var.is_bytes());
        assert_eq!(var.as_bytes().unwrap(), &[1, 2, 3, 4, 5]);

        let json = json!(
            [0, 1, 2,
                { "p0": null, "p1": "a", "p2": 123, "p3": true, "p4": [100, 200.0] },
            4, 5]
        );
        let var = Variant::deserialize(&json).unwrap();
        assert!(var.is_array());
        let var = var.as_array().unwrap();
        assert_eq!(var.len(), 6);
        assert_eq!(var[0].as_integer().unwrap(), 0);
        assert_eq!(var[1].as_integer().unwrap(), 1);
        assert_eq!(var[2].as_integer().unwrap(), 2);
        let inner_obj = var[3].as_object().unwrap();
        assert_eq!(inner_obj.len(), 5);
        assert!(inner_obj["p0"].is_null());
        assert_eq!(inner_obj["p1"].as_str().unwrap(), "a");
        assert_eq!(inner_obj["p2"].as_integer().unwrap(), 123);
        assert!(inner_obj["p3"].as_bool().unwrap());
        let inner_arr = inner_obj["p4"].as_array().unwrap();
        assert_eq!(inner_arr[0].as_integer().unwrap(), 100);
        assert_eq!(inner_arr[1].as_rational().unwrap(), 200.0);
    }
}
