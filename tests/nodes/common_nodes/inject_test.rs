fn basic_test(type_: &str, val: &str, rval: &str) {
    let flow = json!([
        { "id":"flow", "type":"tab" },
        {
            "id": "01", "type": "inject", "topic": "t1", "payload": val, "payloadType": type_,
            "wires": [["02"]],
            "z": "flow"
        },
        { "id": "02", "type": "helper", "z":"flow" }
    ]);
    /*
    helper.load(injectNode, flow, function () {
        var n1 = helper.getNode("n1");
        var n2 = helper.getNode("n2");
        n2.on("input", function (msg) {
            try {
                msg.should.have.property("topic", "t1");
                if (rval) {
                    msg.should.have.property("payload");
                    should.deepEqual(msg.payload, rval);
                }
                else {
                    msg.should.have.property("payload", val);
                }
                done();
            } catch (err) {
                done(err);
            }
        });
        n1.receive({});
    });
    */
}

#[test]
fn test_crate1_integration() {
    // setup();
    // let result = crate1::some_function();
    assert_eq!(33, 42);
}
