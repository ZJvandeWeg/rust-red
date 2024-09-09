import json
import pytest
import time

from tests import *


# 0001 should load node with defaults
# 0002 should load defaults if set to replace
# 0003 should load defaults if set to change
# 0004 should no-op if there are no rules

@pytest.mark.asyncio
async def test_0005():
    '''sets the value of the message property'''
    flows = [
        {"id": "100", "type": "tab"},  # flow 1
        {"id": "1", "type": "change", "z": "100", "action": "replace", "property": "payload",
            "from": "", "to": "changed", "reg": False, "name": "changeNode", "wires": [["2"]]},
        {"id": "2", "z": "100", "type": "console-json"}
    ]
    injections = [
        {"nid": "1", "msg":  {'payload': 'changeMe'}},
    ]
    msgs = await run_flow_with_msgs_ntimes(flows, injections, 1)
    assert msgs[0]["payload"] == 'changed'

# 0006 sets the value of global context property
# 0007 sets the value of persistable global context property


@pytest.mark.asyncio
async def test_0008():
    '''sets the value and type of the message property'''
    flows = [
        {"id": "100", "type": "tab"},  # flow 1
        {"id": "1", "type": "change", "z": "100", "rules": [
            {"t": "set", "p": "payload", "pt": "msg", "to": "12345", "tot": "num"}
        ],
            "reg": False, "name": "changeNode", "wires": [["2"]]},
        {"id": "2", "z": "100", "type": "console-json"}
    ]
    injections = [
        {"nid": "1", "msg":  {'payload': 'changeMe'}},
    ]
    msgs = await run_flow_with_msgs_ntimes(flows, injections, 1)
    payload = msgs[0]['payload']
    assert isinstance(payload, float) or isinstance(payload, int)
    assert payload == 12345


@pytest.mark.asyncio
async def test_0009():
    '''sets the value of an already set multi-level message property'''
    flows = [
        {"id": "100", "type": "tab"},  # flow 1
        {"id": "1", "type": "change", "z": "100", "action": "replace", "property": "foo.bar",
         "from": "", "to": "bar", "reg": False, "name": "changeNode", "wires": [["2"]]},
        {"id": "2", "z": "100", "type": "console-json"}
    ]
    injections = [
        {"nid": "1", "msg":  {"foo": {"bar": "foo"}}},
    ]
    msgs = await run_flow_with_msgs_ntimes(flows, injections, 1)
    assert msgs[0]['foo']['bar'] == "bar"


@pytest.mark.asyncio
async def test_0010():
    '''sets the value of an empty multi-level message property'''
    flows = [
        {"id": "100", "type": "tab"},  # flow 1
        {"id": "1", "type": "change", "z": "100", "action": "replace", "property": "foo.bar",
         "from": "", "to": "bar", "reg": False, "name": "changeNode", "wires": [["2"]]},
        {"id": "2", "z": "100", "type": "console-json"}
    ]
    injections = [
        {"nid": "1", "msg":  {}},
    ]
    msgs = await run_flow_with_msgs_ntimes(flows, injections, 1)
    assert msgs[0]['foo']['bar'] == "bar"


@pytest.mark.asyncio
async def test_0011():
    '''sets the value of a message property to another message property'''
    flows = [
        {"id": "100", "type": "tab"},  # flow 1
        {"id": "1", "type": "change", "z": "100", "action": "replace", "property": "foo",
         "from": "", "to": "msg.fred", "reg": False, "name": "changeNode", "wires": [["2"]]},
        {"id": "2", "z": "100", "type": "console-json"}
    ]
    injections = [
        {"nid": "1", "msg": {"fred": "bar"}},
    ]
    msgs = await run_flow_with_msgs_ntimes(flows, injections, 1)
    assert msgs[0]['foo'] == "bar"


@pytest.mark.asyncio
async def test_0012():
    '''sets the value of a multi-level message property to another multi-level message property'''
    flows = [
        {"id": "100", "type": "tab"},  # flow 1
        {"id": "1", "type": "change", "z": "100", "action": "replace", "property": "foo.bar",
         "from": "", "to": "msg.fred.red", "reg": False, "name": "changeNode", "wires": [["2"]]},
        {"id": "2", "z": "100", "type": "console-json"}
    ]
    injections = [
        {"nid": "1", "msg": {"fred": {"red": "bar"}}},
    ]
    msgs = await run_flow_with_msgs_ntimes(flows, injections, 1)
    assert msgs[0]['foo']['bar'] == "bar"


@pytest.mark.asyncio
async def test_0013():
    '''doesn't set the value of a message property when the 'to' message property does not exist'''
    flows = [
        {"id": "100", "type": "tab"},  # flow 1
        {"id": "1", "type": "change", "z": "100", "action": "replace", "property": "foo.bar",
         "from": "", "to": "msg.fred.red", "reg": False, "name": "changeNode", "wires": [["2"]]},
        {"id": "2", "z": "100", "type": "console-json"}
    ]
    injections = [
        {"nid": "1", "msg": {}},
    ]
    msgs = await run_flow_with_msgs_ntimes(flows, injections, 1)
    assert 'foo' not in msgs[0]


@pytest.mark.asyncio
async def test_0014():
    '''overrides the value of a message property when the 'to' message property does not exist'''
    flows = [
        {"id": "100", "type": "tab"},  # flow 1
        {"id": "1", "type": "change", "z": "100", "action": "replace", "property": "payload",
         "from": "", "to": "msg.foo", "reg": False, "name": "changeNode", "wires": [["2"]]},
        {"id": "2", "z": "100", "type": "console-json"}
    ]
    injections = [
        {"nid": "1", "msg": {"payload": "Hello"}},
    ]
    msgs = await run_flow_with_msgs_ntimes(flows, injections, 1)
    assert 'payload' not in msgs[0]


@pytest.mark.asyncio
async def test_0015():
    '''sets the message property to null when the 'to' message property equals null'''
    flows = [
        {"id": "100", "type": "tab"},  # flow 1
        {"id": "1", "type": "change", "z": "100", "action": "replace", "property": "payload",
         "from": "", "to": "msg.foo", "reg": False, "name": "changeNode", "wires": [["2"]]},
        {"id": "2", "z": "100", "type": "console-json"}
    ]
    injections = [
        {"nid": "1", "msg": {"payload": "Hello", "foo": None}},
    ]
    msgs = await run_flow_with_msgs_ntimes(flows, injections, 1)
    assert 'payload' in msgs[0]
    assert msgs[0]['payload'] == None


@pytest.mark.asyncio
async def test_0016():
    '''does not set other properties using = inside to property'''
    flows = [
        {"id": "100", "type": "tab"},  # flow 1
        {"id": "1", "type": "change", "z": "100", "action": "replace", "property": "payload",
         "from": "", "to": "msg.otherProp=10", "reg": False, "name": "changeNode", "wires": [["2"]]},
        {"id": "2", "z": "100", "type": "console-json"}
    ]
    injections = [
        {"nid": "1", "msg": {"payload": "changeMe"}},
    ]
    msgs = await run_flow_with_msgs_ntimes(flows, injections, 1)
    assert 'payload' not in msgs[0]


@pytest.mark.asyncio
async def test_0017():
    '''splits dot delimited properties into objects'''
    flows = [
        {"id": "100", "type": "tab"},  # flow 1
        {"id": "1", "type": "change", "z": "100", "action": "replace", "property": "pay.load",
         "from": "", "to": "10", "reg": False, "name": "changeNode", "wires": [["2"]]},
        {"id": "2", "z": "100", "type": "console-json"}
    ]
    injections = [
        {"nid": "1", "msg": {"pay": {"load": "changeMe"}}},
    ]
    msgs = await run_flow_with_msgs_ntimes(flows, injections, 1)
    assert msgs[0]['pay']['load'] == "10"


# 0018 changes the value to flow context property
# 0019 changes the value to persistable flow context property
# 0020 changes the value to global context property
# 0021 changes the value to persistable global context property


@pytest.mark.asyncio
async def test_0022():
    '''changes the value to a number'''
    flows = [
        {"id": "100", "type": "tab"},  # flow 1
        {"id": "1", "type": "change", "z": "100",
         "rules": [{"t": "set", "p": "payload", "to": "123", "tot": "num"}], "name": "changeNode", "wires": [["2"]]},
        {"id": "2", "z": "100", "type": "console-json"}
    ]
    injections = [
        {"nid": "1", "msg": {"payload": ""}},
    ]
    msgs = await run_flow_with_msgs_ntimes(flows, injections, 1)
    assert msgs[0]['payload'] == 123


@pytest.mark.asyncio
async def test_0023():
    '''changes the value to a boolean value'''
    flows = [
        {"id": "100", "type": "tab"},  # flow 1
        {"id": "1", "type": "change", "z": "100",
         "rules": [{"t": "set", "p": "payload", "to": "true", "tot": "bool"}], "name": "changeNode", "wires": [["2"]]},
        {"id": "2", "z": "100", "type": "console-json"}
    ]
    injections = [
        {"nid": "1", "msg": {"payload": ""}},
    ]
    msgs = await run_flow_with_msgs_ntimes(flows, injections, 1)
    assert msgs[0]['payload'] == True


@pytest.mark.asyncio
async def test_0024():
    '''changes the value to a js object'''
    flows = [
        {"id": "100", "type": "tab"},  # flow 1
        {"id": "1", "type": "change", "z": "100",
         "rules": [{"t": "set", "p": "payload", "to": '{"a":123}', "tot": "json"}], "name": "changeNode", "wires": [["2"]]},
        {"id": "2", "z": "100", "type": "console-json"}
    ]
    injections = [
        {"nid": "1", "msg": {"payload": ""}},
    ]
    msgs = await run_flow_with_msgs_ntimes(flows, injections, 1)
    assert msgs[0]['payload'] == {"a": 123}


@pytest.mark.asyncio
async def test_0025():
    '''changes the value to a buffer object'''
    flows = [
        {"id": "100", "type": "tab"},  # flow 1
        {"id": "1", "type": "change", "z": "100",
         "rules": [{"t": "set", "p": "payload", "to": '[72,101,108,108,111,32,87,111,114,108,100]', "tot": "bin"}], "name": "changeNode", "wires": [["2"]]},
        {"id": "2", "z": "100", "type": "console-json"}
    ]
    injections = [
        {"nid": "1", "msg": {"payload": ""}},
    ]
    msgs = await run_flow_with_msgs_ntimes(flows, injections, 1)
    assert msgs[0]['payload'] == [72, 101, 108,
                                  108, 111, 32, 87, 111, 114, 108, 100]


@pytest.mark.asyncio
async def test_0026():
    '''sets the value of the message property to the current timestamp'''
    flows = [
        {"id": "100", "type": "tab"},  # flow 1
        {"id": "1", "type": "change", "z": "100",
         "rules": [{"t": "set", "p": "ts", "pt": "msg", "to": "", "tot": "date"}], "name": "changeNode", "wires": [["2"]]},
        {"id": "2", "z": "100", "type": "console-json"}
    ]
    injections = [
        {"nid": "1", "msg": {"payload": time.time_ns() / 1000_000.0}},
    ]
    msgs = await run_flow_with_msgs_ntimes(flows, injections, 1)
    assert ((time.time_ns() / 1000_000.0) - msgs[0]['ts']) < 50000.0


@pytest.mark.asyncio
async def test_0027():
    '''sets the value using env property'''
    flows = [
        {"id": "100", "type": "tab"},  # flow 1
        {"id": "1", "type": "change", "z": "100",
         "rules": [{"t": "set", "p": "payload", "pt": "msg", "to": "NR_TEST_A", "tot": "env"}], "name": "changeNode", "wires": [["2"]]},
        {"id": "2", "z": "100", "type": "console-json"}
    ]
    injections = [
        {"nid": "1", "msg": {"payload": "123", "topic": "ABC"}},
    ]
    os.environ["NR_TEST_A"] = "foo"
    msgs = await run_flow_with_msgs_ntimes(flows, injections, 1)
    del os.environ["NR_TEST_A"]
    assert msgs[0]["payload"] == "foo"


# 0028 sets the value using env property from tab
# 0029 sets the value using env property from group
# 0030 sets the value using env property from nested group
# 0031 changes the value using jsonata
# 0032 reports invalid jsonata expression
# 0033 changes the value using flow context with jsonata
# 0034 changes the value using persistable flow context with jsonata
# 0035 changes the value using persistable global context with jsonata
# 0036 sets the value of a message property using a nested property
# 0037 sets the value of a nested message property using a message property
# 0038 sets the value of a message property using a nested property in flow context
# 0039 sets the value of a message property using a nested property in flow context
# 0040 sets the value of a nested flow context property using a message property
# 0041 deep copies the property if selected

@pytest.mark.asyncio
async def test_0042():
    '''changes the value of the message property'''
    flows = [
        {"id": "100", "type": "tab"},  # flow 1
        {"id": "1", "type": "change", "z": "100", "action": "change", "property": "payload", "from": "Hello",
            "to": "Goodbye", "reg": False, "name": "changeNode", "wires": [["2"]]},
        {"id": "2", "z": "100", "type": "console-json"}
    ]
    injections = [
        {"nid": "1", "msg": {"payload": "Hello World!"}},
    ]
    msgs = await run_flow_with_msgs_ntimes(flows, injections, 1)
    assert msgs[0]["payload"] == "Goodbye World!"


@pytest.mark.asyncio
async def test_0043():
    '''changes the value and doesnt change type of the message property for partial match'''
    flows = [
        {"id": "100", "type": "tab"},  # flow 1
        {"id": "1", "type": "change", "z": "100", "rules": [
            {"t": "change", "p": "payload", "pt": "msg", "from": "123",
             "fromt": "str", "to": "456", "tot": "num"}], "reg": False, "name": "changeNode", "wires": [["2"]]},
        {"id": "2", "z": "100", "type": "console-json"}
    ]
    injections = [
        {"nid": "1", "msg": {"payload": "Change123Me"}},
    ]
    msgs = await run_flow_with_msgs_ntimes(flows, injections, 1)
    assert msgs[0]["payload"] == "Change456Me"


@pytest.mark.asyncio
async def test_0044():
    '''changes the value and type of the message property if a complete match - number'''
    flows = [
        {"id": "100", "type": "tab"},  # flow 1
        {"id": "1", "type": "change", "z": "100",
         "rules": [
             {"t": "change", "p": "payload", "pt": "msg", "from": "123", "fromt": "str", "to": "456", "tot": "num"}],
         "reg": False, "name": "changeNode", "wires": [["2"]]},
        {"id": "2", "z": "100", "type": "console-json"}
    ]
    injections = [
        {"nid": "1", "msg": {"payload": "123"}},
    ]
    msgs = await run_flow_with_msgs_ntimes(flows, injections, 1)
    assert msgs[0]["payload"] == 456


@pytest.mark.asyncio
async def test_0045():
    '''changes the value and type of the message property if a complete match - boolean'''
    flows = [
        {"id": "100", "type": "tab"},  # flow 1
        {"id": "1", "type": "change", "z": "100",
         "rules": [
             {"t": "change", "p": "payload.a", "pt": "msg", "from": "123",
                 "fromt": "str", "to": "true", "tot": "bool"},
             {"t": "change", "p": "payload.b", "pt": "msg", "from": "456",
                 "fromt": "str", "to": "false", "tot": "bool"}
         ],
         "reg": False, "name": "changeNode", "wires": [["2"]]},
        {"id": "2", "z": "100", "type": "console-json"}
    ]
    injections = [
        {"nid": "1", "msg": {"payload": {"a": "123", "b": "456"}}},
    ]
    msgs = await run_flow_with_msgs_ntimes(flows, injections, 1)
    assert msgs[0]["payload"]["a"] == True
    assert msgs[0]["payload"]["b"] == False


@pytest.mark.asyncio
async def test_0046():
    '''changes the value of a multi-level message property'''
    flows = [
        {"id": "100", "type": "tab"},  # flow 1
        {"id": "1", "type": "change", "action": "change", "z": "100",
         "property": "foo.bar", "from": "Hello",
            "to": "Goodbye", "reg": False, "name": "changeNode", "wires": [["2"]]},
        {"id": "2", "z": "100", "type": "console-json"}
    ]
    injections = [
        {"nid": "1", "msg": {"foo": {"bar": "Hello World!"}}},
    ]
    msgs = await run_flow_with_msgs_ntimes(flows, injections, 1)
    assert msgs[0]["foo"]["bar"] == "Goodbye World!"


@pytest.mark.asyncio
async def test_0047():
    '''sends unaltered message if the changed message property does not exist'''
    flows = [
        {"id": "100", "type": "tab"},  # flow 1
        {"id": "1", "type": "change", "z": "100", "action": "change", "property": "foo", "from": "Hello",
            "to": "Goodbye", "reg": False, "name": "changeNode", "wires": [["2"]]},
        {"id": "2", "z": "100", "type": "console-json"}
    ]
    injections = [
        {"nid": "1", "msg": {"payload": "Hello World!"}},
    ]
    msgs = await run_flow_with_msgs_ntimes(flows, injections, 1)
    assert msgs[0]["payload"] == "Hello World!"


@pytest.mark.asyncio
async def test_0048():
    '''sends unaltered message if a changed multi-level message property does not exist'''
    flows = [
        {"id": "100", "type": "tab"},  # flow 1
        {"id": "1", "type": "change", "z": "100", "action": "change", "property": "foo.bar", "from": "Hello",
            "to": "Goodbye", "reg": False, "name": "changeNode", "wires": [["2"]]},

        {"id": "2", "z": "100", "type": "console-json"}
    ]
    injections = [
        {"nid": "1", "msg": {"payload": "Hello World!"}},
    ]
    msgs = await run_flow_with_msgs_ntimes(flows, injections, 1)
    assert msgs[0]["payload"] == "Hello World!"

# 0049 changes the value of the message property based on a regex
# 0050 supports regex groups
# 0051 reports invalid regex
# 0052 supports regex groups - new rule format
# 0053 changes the value using msg property
# 0054 changes the value using flow context property
# 0055 changes the value using persistable flow context property
# 0056 changes the value using global context property
# 0057 changes the value using persistable global context property
# 0058 changes the number using global context property
# 0059 changes the number using persistable global context property
# 0060 changes the value using number - string payload
# 0061 changes the value using number - number payload
# 0062 changes the value using boolean - string payload
# 0063 changes the value using boolean - boolean payload
# 0064 changes the value of the global context