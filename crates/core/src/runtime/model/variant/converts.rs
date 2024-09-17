use std::borrow::Cow;

use super::*;

impl From<&Variant> for String {
    #[inline]
    fn from(var: &Variant) -> Self {
        match var {
            Variant::Number(f) => f.to_string(),
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
    Bytes(Vec<u8>),

    String(String),
    String(&str),

    Bool(bool),

    Array(&[Variant]),
    Array(Vec<Variant>),

    // Object(&[(String, Variant)]),
    // Object(&[(&str, Variant)]),
    Object(VariantObjectMap),
    // Object(&BTreeMap<String, Variant>),
    // Object(BTreeMap<&str, Variant>),
}

impl From<f32> for Variant {
    fn from(f: f32) -> Self {
        serde_json::Number::from_f64(f as f64).map_or(Variant::Null, Variant::Number)
    }
}

impl From<f64> for Variant {
    fn from(f: f64) -> Self {
        serde_json::Number::from_f64(f).map_or(Variant::Null, Variant::Number)
    }
}

impl From<i64> for Variant {
    fn from(f: i64) -> Self {
        Variant::Number(serde_json::Number::from(f))
    }
}

impl From<u64> for Variant {
    fn from(f: u64) -> Self {
        Variant::Number(serde_json::Number::from(f))
    }
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
        let map: VariantObjectMap = value.iter().map(|x| (x.0.clone(), x.1.clone())).collect();
        Variant::Object(map)
    }
}

impl<const N: usize> From<[(&str, Variant); N]> for Variant {
    #[inline]
    fn from(value: [(&str, Variant); N]) -> Self {
        let map: VariantObjectMap = value.iter().map(|x| (x.0.to_string(), x.1.clone())).collect();
        Variant::Object(map)
    }
}

impl From<&[u8]> for Variant {
    fn from(array: &[u8]) -> Self {
        Variant::Bytes(array.to_vec())
    }
}

impl<'a> From<Cow<'a, str>> for Variant {
    fn from(f: Cow<'a, str>) -> Self {
        Variant::String(f.into_owned())
    }
}

impl From<serde_json::Number> for Variant {
    fn from(f: serde_json::Number) -> Self {
        Variant::Number(f)
    }
}

impl From<()> for Variant {
    fn from((): ()) -> Self {
        Variant::Null
    }
}

impl<T> From<Option<T>> for Variant
where
    T: Into<Variant>,
{
    fn from(opt: Option<T>) -> Self {
        match opt {
            None => Variant::Null,
            Some(value) => Into::into(value),
        }
    }
}
