use rquickjs::{class::Trace, Result};

#[derive(Clone, Trace)]
#[rquickjs::class(frozen)]
pub struct Console {
    //
}

impl Console {
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for Console {
    fn default() -> Self {
        Self::new()
    }
}

#[rquickjs::methods]
impl Console {
    fn log(&self, text: rquickjs::Value<'_>) -> Result<()> {
        log::info!("[JS]: {:?}", text.as_string());
        Ok(())
    }

    fn warn(&self, text: rquickjs::Value<'_>) -> Result<()> {
        log::warn!("[JS]: {:?}", text.as_string());
        Ok(())
    }

    fn error(&self, text: rquickjs::Value<'_>) -> Result<()> {
        log::error!("[JS]: {:?}", text.as_string());
        Ok(())
    }

    fn debug(&self, text: rquickjs::Value<'_>) -> Result<()> {
        log::debug!("[JS]: {:?}", text.as_string());
        Ok(())
    }
}
