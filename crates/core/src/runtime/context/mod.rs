use std::{
    collections::HashMap,
    sync::{Arc, Weak},
};

use async_trait::async_trait;
use dashmap::DashMap;

use crate::*;
use runtime::model::*;

mod localfs;
mod memory;

#[linkme::distributed_slice]
pub static __STORES: [StoreMetadata];

type StoreFactoryFn = fn() -> crate::Result<Box<dyn ContextStore>>;

#[derive(Debug)]
pub struct StoreMetadata {
    pub type_: &'static str,
    pub factory: StoreFactoryFn,
}

#[derive(Debug)]
pub struct ContextStoreProperty<'a> {
    pub store: &'a str,
    pub key: &'a str,
}

/// The API trait for a context storage plug-in
#[async_trait]
pub trait ContextStore: Send + Sync {
    fn metadata(&self) -> &'static StoreMetadata;

    async fn open(&self) -> Result<()>;
    async fn close(&self) -> Result<()>;

    async fn get_one(&self, scope: &str, key: &str) -> Result<Variant>;
    async fn get_many(&self, scope: &str, keys: &[&str]) -> Result<Vec<Variant>>;
    async fn get_keys(&self, scope: &str) -> Result<Vec<String>>;

    async fn set_one(&self, scope: &str, key: &str, value: Variant) -> Result<()>;
    async fn set_many(&self, scope: &str, pairs: &[(&str, &Variant)]) -> Result<()>;

    async fn delete(&self, scope: &str) -> Result<()>;
    async fn clean(&self, active_nodes: &[ElementId]) -> Result<()>;
}

impl std::fmt::Debug for dyn ContextStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("TODO")?;
        Ok(())
    }
}

/// A context instance, allowed to bind to a flows element
#[derive(Debug)]
pub struct Context {
    pub parent: Option<Weak<Context>>,
    pub manager: Weak<ContextManager>,
    pub scope: String,
}

#[derive(Debug)]
pub struct ContextManager {
    stores: HashMap<&'static str, Arc<dyn ContextStore>>,
    contexts: DashMap<String, Arc<Context>>,
}

impl Context {
    pub async fn get_one(&self, storage: &str, key: &str) -> Option<Variant> {
        let store = self.manager.upgrade()?.get_context(storage)?;
        // TODO FIXME change it to fixed length stack-allocated string
        store.get_one(&self.scope, key).await.ok()
    }

    pub async fn set_one(&self, storage: &str, key: &str, value: Variant) -> Result<()> {
        let store = self
            .manager
            .upgrade()
            .expect("The mananger cannot be released!")
            .get_context(storage)
            .ok_or(EdgelinkError::BadArguments(format!("Cannot found the storage: '{}'", storage)))?;
        // TODO FIXME change it to fixed length stack-allocated string
        store.set_one(&self.scope, key, value).await
    }
}

impl Default for ContextManager {
    fn default() -> Self {
        let mut stores = HashMap::with_capacity(__STORES.len());
        for smd in __STORES.iter() {
            log::debug!("Initializing context storage provider: '{}'...", smd.type_);
            let store = (smd.factory)().unwrap(); // TODO FIXME
            stores.insert(store.metadata().type_, Arc::from(store));
        }

        Self { contexts: DashMap::new(), stores }
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

    pub fn get_context(&self, storage: &str) -> Option<Arc<dyn ContextStore>> {
        self.stores.get(storage).cloned()
    }
}

fn context_store_parser(input: &str) -> nom::IResult<&str, ContextStoreProperty<'_>, nom::error::VerboseError<&str>> {
    use crate::text::nom_parsers::*;
    use nom::{
        bytes::complete::tag,
        character::complete::{char, multispace0},
        combinator::rest,
        sequence::delimited,
    };

    let (input, _) = tag("#:")(input)?;

    let (input, store) = delimited(char('('), delimited(multispace0, identifier, multispace0), char(')'))(input)?;

    let (input, _) = tag("::")(input)?;
    let (input, key) = rest(input)?;

    Ok((input, ContextStoreProperty { store, key }))
}

/// Parses a context property string, as generated by the TypedInput, to extract
/// the store name if present.
///
/// # Examples
/// For example, `#:(file)::foo.bar` results in ` ParsedContextStoreProperty{ store: "file", key: "foo.bar" }`.
/// ```
/// use edgelink_core::runtime::context::parse_context_store;
///
/// let res = parse_context_store("#:(file)::foo.bar").unwrap();
/// assert_eq!("file", res.store);
/// assert_eq!("foo.bar", res.key);
/// ```
/// @param  {String} key - the context property string to parse
/// @return {Object} The parsed property
/// @memberof @node-red/util_util
pub fn parse_context_store(key: &str) -> crate::Result<ContextStoreProperty<'_>> {
    match context_store_parser(key) {
        Ok(res) => Ok(res.1),
        Err(e) => Err(EdgelinkError::BadArguments(format!("Can not parse the key: '{0}'", e).to_owned()).into()),
    }
}
