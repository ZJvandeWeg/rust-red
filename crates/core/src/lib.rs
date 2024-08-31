use thiserror::Error;

pub mod red;
pub mod runtime;
pub mod utils;

/// The `PluginRegistrar` is defined by the application and passed to `plugin_entry`. It's used
/// for a plugin module to register itself with the application.
pub trait PluginRegistrar {
    fn register_plugin(&mut self, plugin: Box<dyn Plugin>);
}

/// `Plugin` is implemented by a plugin library for one or more types. As you need additional
/// callbacks, they can be defined here. These are first class Rust trait objects, so you have the
/// full flexibility of that system. The main thing you'll lose access to is generics, but that's
/// expected with a plugin system
pub trait Plugin {
    /// This is a callback routine implemented by the plugin.
    fn callback1(&self);
    /// Callbacks can take arguments and return values
    fn callback2(&self, i: i32) -> i32;
}

#[derive(Error, Debug)]
pub enum EdgeLinkError {
    #[error("Permission Denied")]
    PermissionDenied,

    #[error("Invalid 'flows.json'")]
    BadFlowsJson(),

    #[error("Unsupported 'flows.json' format: {0}")]
    UnsupportedFlowsJsonFormat(String),

    #[error("Not supported: {0}")]
    NotSupported(String),

    #[error("Invalid arguments: {0}")]
    BadArguments(String),

    #[error("Task cancelled")]
    TaskCancelled,

    #[error("Invalidate operation")]
    InvalidateOperation(String),

    #[error("{0}")]
    InvalidOperation(String),

    #[error("{0}")]
    InvalidData(String),

    #[error("IO error")]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Other(#[from] crate::Error), // source and Display delegate to anyhow::Error
}

pub type Error = Box<dyn std::error::Error + Send + Sync>;

pub type Result<T> = std::result::Result<T, Error>;
