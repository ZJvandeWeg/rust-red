use edgelink_core::runtime::engine::*;
use edgelink_core::runtime::registry::*;
use edgelink_core::Result;
use std::sync::Arc;
// use tokio_util::sync::CancellationToken;

/*
#[cfg(test)]
struct TestGlobalNode {
    base: BaseNode,
}

#[cfg(test)]
#[async_trait]
impl GlobalNodeBehavior for TestGlobalNode {}

#[async_trait]
impl NodeBehavior for TestGlobalNode {
    async fn start(&self) {}
    async fn stop(&self) {}
}
*/

#[tokio::test]
async fn can_create_flow_manually() -> Result<()> {
    // data::
    let reg_builder = RegistryBuilder::new();
    let reg = reg_builder.build()?;

    let engine = FlowEngine::new_with_flows_file(reg, "tests/data/flows.json").unwrap();

    let flow = engine
        .get_flow(&"dee0d1b0cfd62a6c".parse().unwrap())
        .unwrap();
    let inject_node = flow
        .get_node_by_id(&"bf843d35fe7cf583".parse().unwrap())
        .unwrap();
    assert_eq!(inject_node.get_node().type_, "inject");

    engine.start().await.unwrap();
    engine.stop().await.unwrap();

    // assert_eq!(engine.id(), 0xdee0d1b0cfd62a6cu64);
    // assert_eq!(flow.label(), "Flow 1");
    Ok(())
}
