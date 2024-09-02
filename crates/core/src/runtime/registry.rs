use std::collections::HashMap;
use std::sync::Arc;

use crate::runtime::nodes::MetaNode;
use inventory;

use crate::runtime::nodes::BuiltinNodeDescriptor;

pub trait Registry: Send + Sync {
    fn all(&self) -> &HashMap<&'static str, &'static MetaNode>;
    fn get(&self, type_name: &str) -> Option<&'static MetaNode>;
}

#[derive(Debug)]
struct RegistryImpl {
    meta_nodes: Arc<HashMap<&'static str, &'static MetaNode>>,
}

#[derive(Debug)]
pub struct RegistryBuilder {
    meta_nodes: HashMap<&'static str, &'static MetaNode>,
}

impl RegistryBuilder {
    pub fn new() -> Self {
        Self {
            meta_nodes: HashMap::new(),
        }
    }

    pub fn default() -> Self {
        Self::new().with_builtins()
    }

    pub fn register(mut self, meta_node: &'static MetaNode) -> Self {
        self.meta_nodes.insert(meta_node.type_, &meta_node);
        self
    }

    pub fn with_builtins(mut self) -> Self {
        for bnd in inventory::iter::<BuiltinNodeDescriptor> {
            log::debug!("Found builtin Node: '{}'", bnd.meta.type_);
            self.meta_nodes.insert(bnd.meta.type_, &bnd.meta);
        }
        self
    }

    pub fn build(self) -> Arc<dyn Registry> {
        if self.meta_nodes.is_empty() {
            log::warn!("There are no meta node in the Registry!");
        }
        Arc::new(RegistryImpl {
            meta_nodes: Arc::new(self.meta_nodes),
        })
    }
}

impl RegistryImpl {}

impl Registry for RegistryImpl {
    fn all(&self) -> &HashMap<&'static str, &'static MetaNode> {
        &self.meta_nodes
    }

    fn get(&self, type_name: &str) -> Option<&'static MetaNode> {
        self.meta_nodes.get(type_name).map(|x| *x)
    }
}
