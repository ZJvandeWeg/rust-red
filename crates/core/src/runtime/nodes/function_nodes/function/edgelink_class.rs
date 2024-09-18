use std::collections::HashMap;

use rquickjs::{class::Trace, function::Constructor, Ctx, IntoJs, Result, Value};

#[derive(Clone, Trace)]
#[rquickjs::class(frozen)]
pub(super) struct EdgelinkClass {}

impl Default for EdgelinkClass {
    fn default() -> Self {
        Self {}
    }
}

#[allow(non_snake_case)]
#[rquickjs::methods]
impl<'js> EdgelinkClass {

    /// Deep clone a JS object
    #[qjs(rename = "deepClone")]
    fn deep_clone(&self, obj: Value<'js>, ctx: Ctx<'js>) -> Result<Value<'js>> {
        if let Some(obj_ref) = obj.as_object() {
            let global = ctx.globals();
            let date_ctor: Constructor = global.get("Date")?;
            if obj_ref.is_instance_of(&date_ctor) {
                let get_time_fn: rquickjs::Function = obj_ref.get("getTime")?;
                let time: i64 = get_time_fn.call((rquickjs::function::This(&obj),))?;
                return date_ctor.construct((time,));
            }

            if let Some(src_arr) = obj_ref.as_array() {
                let mut arr_copy = Vec::with_capacity(src_arr.len());
                for item in src_arr.iter() {
                    let cloned = self.deep_clone(item?, ctx.clone())?;
                    arr_copy.push(cloned);
                }
                return arr_copy.into_js(&ctx);
            }

            {
                let mut obj_copy: HashMap<String, Value<'js>> = HashMap::with_capacity(obj_ref.len());
                let has_own_property_fn: rquickjs::Function = obj_ref.get("hasOwnProperty")?;
                for item in obj_ref.props::<String, Value<'js>>() {
                    let (k, v) = item?;
                    let has: bool = has_own_property_fn.call((rquickjs::function::This(&obj), k.as_str()))?;
                    if has {
                        obj_copy.insert(k, self.deep_clone(v, ctx.clone())?);
                    }
                }
                return obj_copy.into_js(&ctx);
            }
        } else {
            return Ok(obj);
        }
    }
}
