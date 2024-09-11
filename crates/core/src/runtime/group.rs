use std::sync::Arc;
use std::sync::Weak;

use super::env::*;
use super::flow::*;
use super::model::json::*;
use super::model::*;

#[derive(Debug)]
pub enum GroupParent {
    Flow(Weak<Flow>),
    Group(Weak<Group>),
}

#[derive(Debug)]
pub struct Group {
    pub id: ElementId,
    pub name: String,
    pub flow: Weak<Flow>,
    pub parent: GroupParent,
    pub envs: Arc<EnvStore>,
}

impl Group {
    pub(crate) fn new_flow_group(config: &RedGroupConfig, flow: &Arc<Flow>) -> crate::Result<Self> {
        let envs_builder = EnvStoreBuilder::default().with_parent(&flow.get_envs());

        let group = Group {
            id: config.id,
            name: config.name.clone(),
            flow: Arc::downgrade(flow),
            parent: GroupParent::Flow(Arc::downgrade(flow)),
            envs: build_envs(envs_builder, config),
        };
        Ok(group)
    }

    pub(crate) fn new_subgroup(
        config: &RedGroupConfig,
        flow: &Arc<Flow>,
        parent: &Arc<Group>,
    ) -> crate::Result<Self> {
        let envs_builder = EnvStoreBuilder::default().with_parent(&parent.envs);

        let group = Group {
            id: config.id,
            name: config.name.clone(),
            flow: Arc::downgrade(flow),
            parent: GroupParent::Group(Arc::downgrade(parent)),
            envs: build_envs(envs_builder, config),
        };
        Ok(group)
    }

    pub fn get_envs(&self) -> Arc<EnvStore> {
        self.envs.clone()
    }

    pub fn get_env(&self, key: &str) -> Option<Variant> {
        self.envs.evalute_env(key)
    }
}

fn build_envs(mut envs_builder: EnvStoreBuilder, config: &RedGroupConfig) -> Arc<EnvStore> {
    if let Some(env_json) = config.json.get("env") {
        envs_builder = envs_builder.load_json(env_json);
    }
    envs_builder
        .extends([
            ("NR_GROUP_ID".into(), Variant::String(config.id.to_string())),
            ("NR_GROUP_NAME".into(), Variant::String(config.name.clone())),
        ])
        .build()
}
