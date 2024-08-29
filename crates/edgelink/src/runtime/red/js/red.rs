use rquickjs::{self as js, IntoAtom};

pub fn register_red_object(js_ctx: &js::Ctx) -> Result<(), js::Error> {
    let red_obj = define_red_object(js_ctx)?;
    js_ctx.globals().set("RED", red_obj)?;
    Ok(())
}

pub fn define_red_object<'js>(js_ctx: &js::Ctx<'js>) -> Result<js::Object<'js>, js::Error> {
    let red_obj = js::Object::new(js_ctx.clone())?;

    let util_obj = define_util_object(js_ctx)?;
    red_obj.set("util".into_atom(js_ctx)?, util_obj)?;

    Ok(red_obj)
}

fn define_util_object<'js>(js_ctx: &js::Ctx<'js>) -> Result<js::Object<'js>, js::Error> {
    let util_obj = js::Object::new(js_ctx.clone())?;

    let func = js::Function::new(js_ctx.clone(), || {
        format!("{:016x}", crate::utils::generate_uid())
    })?;
    util_obj.set("generateId".into_atom(js_ctx)?, func)?;

    let func: js::Function = js_ctx.eval("(obj) => { return JSON.parse(JSON.stringify(obj)); }")?;
    util_obj.set("cloneMessage".into_atom(js_ctx)?, func)?;

    // TODO more functions

    Ok(util_obj)
}
