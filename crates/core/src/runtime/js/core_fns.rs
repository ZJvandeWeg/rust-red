use std::time::Duration;

use rquickjs::class::Trace;
use rquickjs::prelude::*;
use rquickjs::Function;

#[derive(Clone, Trace, Default)]
#[rquickjs::class(frozen)]
pub struct CancellationTokenWrapper {
    #[qjs(skip_trace)]
    token: tokio_util::sync::CancellationToken,
}

impl Drop for CancellationTokenWrapper {
    fn drop(&mut self) {
        if !self.token.is_cancelled() {
            self.token.cancel();
        }
    }
}

pub fn register_all(ctx: &Ctx<'_>) -> rquickjs::Result<()> {
    ctx.globals().set("setTimeout", Func::from(self::set_timeout))?;
    ctx.globals().set("clearTimeout", Func::from(self::clear_timeout))?;
    ctx.globals().set("setInterval", Func::from(self::set_interval))?;
    ctx.globals().set("clearInterval", Func::from(self::clear_interval))?;
    Ok(())
}

pub fn set_interval<'js>(cb: Function<'js>, delay: Opt<f64>, ctx: Ctx<'js>) -> CancellationTokenWrapper {
    let delay = delay.0.unwrap_or(0.0);
    let duration = Duration::from_secs_f64(delay / 1000.0);
    let mut interval = tokio::time::interval(duration);
    let token = tokio_util::sync::CancellationToken::new();
    ctx.spawn({
        let token = token.clone();
        async move {
            loop {
                tokio::select! {
                    _ = token.cancelled() => break,
                    _ = interval.tick() => { let _ =  cb.call::<_, ()>(()); }
                }
            }
        }
    });
    CancellationTokenWrapper { token }
}

pub fn clear_interval(ctw: CancellationTokenWrapper) {
    ctw.token.cancel();
}

pub fn set_timeout<'js>(cb: Function<'js>, delay: Opt<f64>, ctx: Ctx<'js>) -> CancellationTokenWrapper {
    let delay = delay.0.unwrap_or(0.0);
    let duration = Duration::from_secs_f64(delay / 1000.0);
    let token = tokio_util::sync::CancellationToken::new();
    ctx.spawn({
        let token = token.clone();
        async move {
            tokio::select! {
                _ = token.cancelled() => { },
                _ = tokio::time::sleep(duration) => { let _ =  cb.call::<_, ()>(()); }
            }
        }
    });
    CancellationTokenWrapper { token }
}

pub fn clear_timeout(ctw: CancellationTokenWrapper) {
    ctw.token.cancel();
}
