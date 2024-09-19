use crate::EdgelinkError;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

pub async fn delay(dur: Duration, cancel: CancellationToken) -> crate::Result<()> {
    tokio::select! {
        _ = cancel.cancelled() => {
            // 取消 sleep_task 任务
            Err(EdgelinkError::TaskCancelled.into())
        }
        _ = tokio::time::sleep(dur)=> {
            // Long work has completed
            Ok(())
        }
    }
}

pub async fn delay_secs_f64(secs: f64, cancel: CancellationToken) -> crate::Result<()> {
    delay(Duration::from_secs_f64(secs), cancel).await
}

pub async fn delay_millis(millis: u64, cancel: CancellationToken) -> crate::Result<()> {
    delay(Duration::from_millis(millis), cancel).await
}

pub trait SyncWaitableFuture: std::future::Future {
    fn wait(self) -> Self::Output
    where
        Self: Sized + Send + 'static,
        Self::Output: Send + 'static,
    {
        let handle = tokio::runtime::Handle::current();
        let task = handle.spawn(self);
        tokio::task::block_in_place(|| tokio::runtime::Handle::current().block_on(task).unwrap())
    }
}

impl<F> SyncWaitableFuture for F where F: std::future::Future {}
