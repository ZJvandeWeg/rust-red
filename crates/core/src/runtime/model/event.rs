use std::{future::Future, pin::Pin};

pub type AsyncCallback = Box<dyn Fn() -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync>;

pub struct AsyncEvent {
    pub subscribers: tokio::sync::RwLock<Vec<AsyncCallback>>,
}

impl AsyncEvent {
    pub fn new() -> Self {
        AsyncEvent {
            subscribers: tokio::sync::RwLock::new(Vec::new()),
        }
    }

    pub async fn subscribe<F>(&self, callback: F)
    where
        F: Fn() -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync + 'static,
    {
        self.subscribers.write().await.push(Box::new(callback));
    }

    pub async fn trigger(&self) {
        let subscribers = self.subscribers.read().await;
        for callback in subscribers.iter() {
            let fut = callback();
            tokio::spawn(fut);
        }
    }
}
