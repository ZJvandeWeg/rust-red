use super::*;

#[cfg(feature = "js")]
mod js {
    pub use rquickjs::*;
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
                    let global = _ctx.globals();
                    let date_ctor: Constructor = global.get("Date")?;
                    let regexp_ctor: Constructor = global.get("RegExp")?;
                    if jo.is_instance_of(date_ctor) {
                        let st = jv.get::<SystemTime>()?;
                        Ok(Variant::Date(st))
                    } else if jo.is_instance_of(regexp_ctor) {
                        let to_string_fn: js::Function = jo.get("toString")?;
                        let re_str: String = to_string_fn.call((js::function::This(jv),))?;
                        match Regex::new(re_str.as_str()) {
                            Ok(re) => Ok(Variant::Regexp(re)),
                            Err(_) => Err(js::Error::FromJs {
                                from: "JS object",
                                to: "Variant::Regexp",
                                message: Some(format!("Failed to create Regex from: '{}'", re_str)),
                            }),
                        }
                    } else {
                        let mut map = VariantObjectMap::new();
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
                    }
                } else {
                    Err(js::Error::FromJs { from: "JS object", to: "Variant::Object", message: None })
                }
            }

            _ => Err(js::Error::FromJs { from: "Unknown JS type", to: "", message: None }),
        }
    }
}
