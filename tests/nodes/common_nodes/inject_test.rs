use std::time::Duration;

use crate::*;
use edgelink_core as el;
use edgelink_core::runtime::flow::*;
use edgelink_core::runtime::model::*;
use edgelink_core::runtime::nodes::*;
use edgelink_core::utils::async_util::delay;
use serde::ser::Serialize;
use serde_json::*;

async fn basic_test(type_: &str, val: Variant, rval: Option<&str>) -> el::Result<()> {
    let flow = json!([
        { "id":"0", "type":"tab" },
        {
            "id": "1", "type": "inject", "topic": "t1",
            "once": true, "onceDelay": 1,
            "payload": to_value(&val).unwrap(),
            "payloadType": type_,
            "wires": [
                ["2", ],
            ],
            "z": "0"
        },
        { "id": "2", "type": "debug", "z": "0" }
    ]);
    println!("json:\n{}", serde_json::to_string_pretty(&flow)?);

    let helper = TestHelper::with_json(&flow)?;
    let n1 = helper.get_node(&ElementId::with_u64(1)).unwrap();
    let n2 = helper.get_node(&ElementId::with_u64(2)).unwrap();
    let mut received_rx = n2.get_node().on_received.subscribe();

    println!("Starting engine...");
    helper.start_engine().await?;

    let x = tokio::spawn(async move {
        println!("Waiting broadcast...");
        let msg = received_rx.recv().await.unwrap();
        println!("Received! ...");
        let locked_msg = msg.read().await;
        assert_eq!(
            locked_msg
                .get_property("topic")
                .expect("has topic")
                .as_string()
                .expect(""),
            "t1"
        );
    });

    tokio::time::sleep(Duration::from_secs(1)).await;

    helper.stop_engine().await?;
    Ok(())
}

#[tokio::test]
async fn test_crate1_integration() -> el::Result<()> {
    // setup();
    // let result = crate1::some_function();
    basic_test("num", Variant::String("10".to_string()), None).await?;
    Ok(())
}
