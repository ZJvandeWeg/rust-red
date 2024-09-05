import json
import pytest
import time

from tests import *


@pytest.mark.asyncio
async def test_0001():
    '''should be linked'''
    # We are not allowed that the `link in` and the 'link out' nodes in the same flow!
    flows = [
        {"id": "100", "type": "tab"},  # flow 1
        {"id": "200", "type": "tab"},  # flow 2
        {
            "id": "1", "z": "100", "type": "link out",
            "name": "link-out", "links": ["2"]
        },
        {
            "id": "2", "z": "200", "type": "link in",
            "name": "link-out", "wires": [["3"]]
        },
        {
            "id": "3", "z": "200", "type": "console-json"
        }
    ]
    injections = [
        {'payload': 'hello'},
    ]
    msgs = await run_flow_with_msgs_ntimes(flows, injections, 1)
    assert msgs[0]["payload"] == 'hello'


@pytest.mark.asyncio
async def test_0002():
    # '''should be linked to multiple nodes'''
    flows = [
        {"id": "100", "type": "tab"},  # flow 1
        {"id": "200", "type": "tab"},  # flow 2
        {
            "id": "1", "z": "100", "type": "link out",
            "name": "link-out", "links": ["2", "3"]
        },
        {
            "id": "2", "z": "200", "type": "link in",
            "name": "link-in0", "wires": [["4"]]
        },
        {
            "id": "3", "z": "200", "type": "link in",
            "name": "link-in1", "wires": [["4"]]
        },
        {
            "id": "4", "z": "200", "type": "console-json"
        }
    ]
    injections = [
        {'payload': 'hello'},
    ]
    msgs = await run_flow_with_msgs_ntimes(flows, injections, 2)
    assert msgs[0]["payload"] == 'hello'


@pytest.mark.asyncio
async def test_0003():
    # '''should be linked to multiple nodes'''
    flows = [
        {"id": "100", "type": "tab"},  # flow 1
        {"id": "200", "type": "tab"},  # flow 2
        {
            "id": "1", "z": "100", "type": "link out",
            "name": "link-out0", "links": ["3"]
        },
        {
            "id": "2", "z": "100", "type": "link out",
            "name": "link-out1", "links": ["3"]
        },
        {
            "id": "3", "z": "200", "type": "link in",
            "name": "link-in", "wires": [["4"]]
        },
        {
            "id": "4", "z": "200", "type": "console-json"
        }
    ]
    injections = [
        {"nid": "1", "msg": {'payload': 'hello'}},
        {"nid": "2", "msg": {'payload': 'hello'}},
    ]
    msgs = await run_flow_with_msgs_ntimes(flows, injections, 2)
    assert msgs[0]["payload"] == 'hello'
    assert msgs[1]["payload"] == 'hello'


@pytest.mark.asyncio
async def test_0004():
    # '''should call static link-in node and get response'''
    flows = [
        {"id": "100", "type": "tab"},  # flow 1
        {"id": "200", "type": "tab"},  # flow 2
        {
            "id": "1", "z": "100", "type": "link in",
            "wires": [["2"]]
        },
        {
            "id": "2", "z": "100", "type": "function",
            "func": 'msg.payload = "123"; return msg;',
            "wires": [["3"]]
        },
        {
            "id": "3", "z": "100", "type": "link out",
            "mode": "return"
        },
        {
            "id": "4", "z": "200", "type": "link call",
            "links": ["1"], "wires": [["5"]]
        },
        {
            "id": "5", "z": "200", "type": "console-json"
        }
    ]
    injections = [
        {"nid": "4", "msg": {'payload': 'hello'}},
    ]
    msgs = await run_flow_with_msgs_ntimes(flows, injections, 1)
    assert msgs[0]["payload"] == "123"


@pytest.mark.asyncio
async def test_0005():
    # '''should call link-in node by name and get response'''
    payload = float(time.time())
    flows = [
        {"id": "100", "type": "tab", "label": "Flow 1"},
        {"id": "200", "type": "tab", "label": "Flow 2"},
        {
            "id": "1", "z": "100", "type": "link in",
            "name": "double payload", "wires": [["3"]]},
        {
            "id": "2", "z": "200", "type": "link in",
            "name": "double payload", "wires": [["3"]]},
        {
            "id": "3", "z": "100", "type": "function",
            "func": 'msg.payload = msg.payload + msg.payload; return msg;',
            "wires": [["4"]]
        },
        {"id": "4", "z": "100", "type": "link out", "mode": "return"},
        {
            "id": "5", "z": "100", "type": "link call",
            "linkType": "dynamic", "links": [], "wires": [["6"]]
        },
        {"id": "6", "z": "100", "type": "console-json"}
    ]
    injections = [
        {"nid": "5", "msg": {'payload': payload, 'target': 'double payload'}},
    ]
    msgs = await run_flow_with_msgs_ntimes(flows, injections, 1)
    assert msgs[0]["payload"] == payload + payload
