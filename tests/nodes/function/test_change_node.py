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


# 0023 changes the value to a js object

# changes the value to a buffer object
# sets the value of the message property to the current timestamp
# sets the value using env property
# sets the value using env property from tab
# sets the value using env property from group
# sets the value using env property from nested group
# changes the value using jsonata
# reports invalid jsonata expression
# changes the value using flow context with jsonata
# changes the value using persistable flow context with jsonata
# changes the value using persistable global context with jsonata
# sets the value of a message property using a nested property
# sets the value of a nested message property using a message property
# sets the value of a message property using a nested property in flow context
# sets the value of a message property using a nested property in flow context
# sets the value of a nested flow context property using a message property
# deep copies the property if selected
# changes the value of the message property
# changes the value and doesnt change type of the message property for partial match
# changes the value and type of the message property if a complete match - number