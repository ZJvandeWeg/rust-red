use edgelink_core::runtime::engine::*;
use edgelink_core::runtime::model::ElementId;
use edgelink_core::runtime::nodes::*;
use edgelink_core::runtime::registry;
use edgelink_core::runtime::registry::*;
use edgelink_core::*;
use serde_json;
use std::sync::Arc;

pub struct TestHelper {
    pub registry: Arc<dyn Registry>,
    pub engine: Arc<FlowEngine>,
}

impl TestHelper {
    fn default_registry() -> Arc<dyn Registry> {
        let builder = RegistryBuilder::new();
        builder.with_builtins();
        builder.build()
    }

    pub fn with_flows_file(flows_path: &str) -> Result<Self> {
        let registry = TestHelper::default_registry();
        let engine = FlowEngine::new_with_flows_file(registry.clone(), flows_path)?;
        Ok(Self { registry, engine })
    }

    pub fn with_json(json: &serde_json::Value) -> Result<Self> {
        let registry = TestHelper::default_registry();
        let engine = FlowEngine::new_with_json(registry.clone(), json).unwrap();
        Ok(Self { registry, engine })
    }

    pub fn with_json_text(json_text: &str) -> Result<Self> {
        let registry = TestHelper::default_registry();
        let engine = FlowEngine::new_with_json_string(registry.clone(), json_text).unwrap();
        Ok(Self { registry, engine })
    }

    pub async fn start_engine(&self) -> Result<()> {
        self.engine.start().await
    }

    pub async fn stop_engine(&self) -> Result<()> {
        self.engine.stop().await
    }

    pub fn get_node(&self, id: &ElementId) -> Option<Arc<dyn FlowNodeBehavior>> {
        self.engine.find_flow_node_by_id(id)
    }
}
