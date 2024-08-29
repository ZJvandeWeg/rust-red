use log;
use std::collections::HashMap;
use std::sync::Arc;

use crate::runtime::nodes::MetaNode;
use inventory;

use crate::runtime::nodes::BuiltinNodeDescriptor;

pub trait Registry: Send + Sync {
    fn all(&self) -> &HashMap<&'static str, MetaNode>;
    fn get(&self, type_name: &str) -> Option<&MetaNode>;
}

#[derive(Default)]
pub struct RegistryImpl {
    meta_nodes: Arc<HashMap<&'static str, MetaNode>>,
}

impl RegistryImpl {
    pub fn new() -> Self {
        let mut nodes = HashMap::new();
        for bnd in inventory::iter::<BuiltinNodeDescriptor> {
            log::debug!("Found builtin Node: '{}'", bnd.meta.type_name);
            nodes.insert(bnd.meta.type_name, bnd.meta);
        }

        RegistryImpl {
            meta_nodes: Arc::new(nodes),
        }
    }
}

impl Registry for RegistryImpl {
    fn all(&self) -> &HashMap<&'static str, MetaNode> {
        &self.meta_nodes
    }

    fn get(&self, type_name: &str) -> Option<&MetaNode> {
        self.meta_nodes.get(type_name)
    }
}
