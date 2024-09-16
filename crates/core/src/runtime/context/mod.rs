use std::{
    collections::HashMap,
    sync::{Arc, Weak},
};

use async_trait::async_trait;
use dashmap::DashMap;
use nom::Parser;
use serde;

use crate::*;
use runtime::model::*;

mod localfs;
mod memory;

pub const GLOBAL_STORE_NAME: &str = "global";
pub const DEFAULT_STORE_NAME: &str = "default";
pub const DEFAULT_STORE_NAME_ALIAS: &str = "_";

#[linkme::distributed_slice]
pub static __PROVIDERS: [ProviderMetadata];

type StoreFactoryFn = fn(name: String, options: Option<&ContextStoreOptions>) -> crate::Result<Box<dyn ContextStore>>;

#[derive(Debug)]
pub struct ProviderMetadata {
    pub type_: &'static str,
    pub factory: StoreFactoryFn,
}

#[derive(Debug, Clone, serde:: Deserialize)]
pub struct ContextStorageSettings {
    pub default: String,
    pub stores: HashMap<String, ContextStoreOptions>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ContextStoreOptions {
    pub provider: String,

    #[serde(flatten)]
    pub options: HashMap<String, config::Value>,
}

#[derive(Debug, Clone, Copy)]
pub struct ContextStoreProperty<'a> {
    pub store: Option<&'a str>,
    pub key: &'a str,
}

/// The API trait for a context storage plug-in
#[async_trait]
pub trait ContextStore: Send + Sync {
    fn metadata(&self) -> &'static ProviderMetadata;

    async fn name(&self) -> &str;

    async fn open(&self) -> Result<()>;
    async fn close(&self) -> Result<()>;

    async fn get_one(&self, scope: &str, key: &str) -> Result<Variant>;
    async fn get_many(&self, scope: &str, keys: &[&str]) -> Result<Vec<Variant>>;
    async fn get_keys(&self, scope: &str) -> Result<Vec<String>>;

    async fn set_one(&self, scope: &str, key: &str, value: Variant) -> Result<()>;
    async fn set_many(&self, scope: &str, pairs: &[(&str, &Variant)]) -> Result<()>;

    async fn remove_one(&self, scope: &str, key: &str) -> Result<Variant>;

    async fn delete(&self, scope: &str) -> Result<()>;
    async fn clean(&self, active_nodes: &[ElementId]) -> Result<()>;
}

/// A context instance, allowed to bind to a flows element
#[derive(Debug)]
pub struct Context {
    pub parent: Option<Weak<Context>>,
    pub manager: Weak<ContextManager>,
    pub scope: String,
}

pub struct ContextManager {
    default_store: Arc<dyn ContextStore>,
    stores: HashMap<String, Arc<dyn ContextStore>>,
    contexts: DashMap<String, Arc<Context>>,
}

pub struct ContextManagerBuilder {
    stores: HashMap<String, Arc<dyn ContextStore>>,
    default_store: String,
    settings: Option<ContextStorageSettings>,
}

impl Context {
    pub async fn get_one(&self, prop: &ContextStoreProperty<'_>) -> Option<Variant> {
        let store = if let Some(storage) = prop.store {
            self.manager.upgrade()?.get_context(storage)?
        } else {
            self.manager.upgrade()?.get_default()
        };
        // TODO FIXME change it to fixed length stack-allocated string
        store.get_one(&self.scope, prop.key).await.ok()
    }

    pub async fn set_one(&self, prop: &ContextStoreProperty<'_>, value: Option<Variant>) -> Result<()> {
        let store = if let Some(storage) = prop.store {
            self.manager
                .upgrade()
                .expect("The mananger cannot be released!")
                .get_context(storage)
                .ok_or(EdgelinkError::BadArguments(format!("Cannot found the storage: '{}'", storage)))?
        } else {
            self.manager.upgrade().expect("The manager cannot be released!").get_default()
        };
        // TODO FIXME change it to fixed length stack-allocated string
        if let Some(value) = value {
            store.set_one(&self.scope, prop.key, value).await
        } else {
            let _ = store.remove_one(&self.scope, prop.key).await?;
            Ok(())
        }
    }
}

impl Default for ContextManager {
    fn default() -> Self {
        let memory_metadata = __PROVIDERS.iter().find(|x| x.type_ == "memory").unwrap();
        let memory_store =
            (memory_metadata.factory)("memory".into(), None).expect("Create memory storage cannot go wrong.");
        let mut stores: HashMap<std::string::String, Arc<dyn ContextStore>> = HashMap::with_capacity(1);
        stores.insert("memory".to_string(), Arc::from(memory_store));
        Self { default_store: stores["memory"].clone(), contexts: DashMap::new(), stores }
    }
}

impl Default for ContextManagerBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl ContextManagerBuilder {
    pub fn new() -> Self {
        let stores = HashMap::with_capacity(__PROVIDERS.len());
        Self { stores, default_store: "memory".into(), settings: None }
    }

    pub fn load_default(&mut self) -> &mut Self {
        let memory_metadata = __PROVIDERS.iter().find(|x| x.type_ == "memory").unwrap();
        let memory_store =
            (memory_metadata.factory)("memory".into(), None).expect("Create memory storage cannot go wrong.");
        self.stores.clear();
        self.stores.insert("memory".to_string(), Arc::from(memory_store));
        self
    }

    pub fn with_config(&mut self, config: &config::Config) -> crate::Result<&mut Self> {
        let settings: ContextStorageSettings = config.get("runtime.context")?;
        self.stores.clear();
        for (store_name, store_options) in settings.stores.iter() {
            log::debug!("Initializing context store: name='{}', provider='{}' ...", store_name, store_options.provider);
            let meta =
                __PROVIDERS.iter().find(|x| x.type_ == store_options.provider).ok_or(EdgelinkError::Configuration)?;
            let store = (meta.factory)(store_name.into(), Some(store_options))?;
            self.stores.insert(store_name.clone(), Arc::from(store));
        }

        if !settings.stores.contains_key(&settings.default) {
            use anyhow::Context;
            return Err(EdgelinkError::Configuration).with_context(|| {
                format!(
                    "Cannot found the default context storage '{}', check your configuration file.",
                    settings.default
                )
            });
        }
        self.settings = Some(settings);
        Ok(self)
    }

    pub fn default_store(&mut self, default: String) -> &mut Self {
        self.default_store = default;
        self
    }

    pub fn build(&self) -> crate::Result<Arc<ContextManager>> {
        let cm = ContextManager {
            default_store: self.stores[&self.default_store].clone(),
            stores: self.stores.clone(),
            contexts: DashMap::new(),
        };
        Ok(Arc::new(cm))
    }
}

impl ContextManager {
    pub fn new_context(self: &Arc<Self>, parent: Option<&Arc<Context>>, scope: String) -> Arc<Context> {
        let c = Arc::new(Context {
            parent: parent.map(Arc::downgrade),
            manager: Arc::downgrade(self),
            scope: scope.clone(),
        });
        self.contexts.insert(scope, c.clone());
        c
    }

    pub fn new_global_context(self: &Arc<Self>) -> Arc<Context> {
        let c = Arc::new(Context { parent: None, manager: Arc::downgrade(self), scope: GLOBAL_STORE_NAME.to_string() });
        self.contexts.insert(GLOBAL_STORE_NAME.to_string(), c.clone());
        c
    }

    pub fn get_default(&self) -> Arc<dyn ContextStore> {
        self.default_store.clone()
    }

    pub fn get_context(&self, store_name: &str) -> Option<Arc<dyn ContextStore>> {
        self.stores.get(store_name).cloned()
    }
}

fn parse_store_expr(input: &str) -> nom::IResult<&str, &str, nom::error::VerboseError<&str>> {
    use crate::text::nom_parsers::*;
    use nom::{
        bytes::complete::tag,
        character::complete::{char, multispace0},
        sequence::delimited,
    };

    let (input, _) = tag("#:").parse(input)?;
    let (input, store) =
        delimited(char('('), delimited(multispace0, identifier, multispace0), char(')')).parse(input)?;
    let (input, _) = tag("::").parse(input)?;
    Ok((input, store))
}

fn context_store_parser(input: &str) -> nom::IResult<&str, ContextStoreProperty, nom::error::VerboseError<&str>> {
    // use crate::text::nom_parsers::*;
    use nom::combinator::{opt, rest};

    let (input, store) = opt(parse_store_expr).parse(input)?;
    let (input, key) = rest(input)?;

    Ok((input, ContextStoreProperty { store, key }))
}

/// Parses a context property string, as generated by the TypedInput, to extract
/// the store name if present.
///
/// # Examples
/// For example, `#:(file)::foo.bar` results in ` ParsedContextStoreProperty{ store: Some("file"), key: "foo.bar" }`.
/// ```
/// use edgelink_core::runtime::context::parse_store;
///
/// let res = parse_store("#:(file)::foo.bar").unwrap();
/// assert_eq!(Some("file"), res.store);
/// assert_eq!("foo.bar", res.key);
/// ```
/// @param  {String} key - the context property string to parse
/// @return {Object} The parsed property
/// @memberof @node-red/util_util
pub fn parse_store(key: &str) -> crate::Result<ContextStoreProperty> {
    match context_store_parser(key) {
        Ok(res) => Ok(res.1),
        Err(e) => Err(EdgelinkError::BadArguments(format!("Can not parse the key: '{0}'", e).to_owned()).into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_context_store() {
        let res = parse_store("#:(file1)::foo.bar").unwrap();
        assert_eq!(Some("file1"), res.store);
        assert_eq!("foo.bar", res.key);

        let res = parse_store("#:(memory1)::payload").unwrap();
        assert_eq!(Some("memory1"), res.store);
        assert_eq!("payload", res.key);

        let res = parse_store("foo.bar").unwrap();
        assert_eq!(None, res.store);
        assert_eq!("foo.bar", res.key);
    }
}
