import json
import pytest

from . import *


@pytest.mark.asyncio
async def test_0001():
    '''should define subflow'''
    flows = [
        {"id": "100", "type": "tab"},
        {"id": "1", "z": "100", "type": "subflow:200", "wires": [["2"]]},
        {"id": "2", "z": "100", "type": "console-json", "wires": []},
        # Subflow
        {"id": "200", "type": "subflow", "name": "Subflow", "info": "", "in": [
            {"wires": [{"id": "3"}]}], "out": [{"wires": [{"id": "3", "port": 0}]}]},
        {"id": "3", "z": "200", "type": "function",
            "func": "return msg;", "wires": []}
    ]
    injections = [
        {"nid": "1", "msg": {"payload": "hello"}},
    ]
    msgs = await run_flow_with_msgs_ntimes(flows, injections, 1)
    assert msgs[0]["payload"] == "hello"


@pytest.mark.asyncio
async def test_0002():
    '''should pass data to/from subflow'''
    flows = [
        {"id": "100", "type": "tab"},
        {"id": "1", "z": "100", "type": "subflow:200", "wires": [["2"]]},
        {"id": "2", "z": "100", "type": "console-json", "wires": []},
        # Subflow
        {"id": "200", "type": "subflow", "name": "Subflow", "info": "", "in": [
            {"wires": [{"id": "3"}]}], "out": [{"wires": [{"id": "3", "port": 0}]}]},
        {"id": "3", "z": "200", "type": "function",
            "func": "msg.payload = msg.payload+'bar'; return msg;", "wires": []}
    ]
    injections = [
        {"nid": "1", "msg": {"payload": "foo"}},
    ]
    msgs = await run_flow_with_msgs_ntimes(flows, injections, 1)
    assert msgs[0]["payload"] == "foobar"


@pytest.mark.asyncio
async def test_0003():
    '''should pass data to/from nested subflow'''
    flows = [
        {"id": "100", "type": "tab", "info": ""},
        {"id": "1", "z": "100", "type": "subflow:200", "wires": [["2"]]},
        {"id": "2", "z": "100", "type": "console-json", "wires": []},
        # Subflow1
        {"id": "200", "type": "subflow", "name": "Subflow1", "info": "",
         "in": [{"wires": [{"id": "3"}]}],
         "out": [{"wires": [{"id": "4", "port": 0}]}]
         },
        {"id": "3", "z": "200", "type": "subflow:300",
         "wires": [["4"]]},
        {"id": "4", "z": "200", "type": "function",
         "func": "msg.payload = msg.payload+'baz'; return msg;", "wires": []},
        # Subflow2
        {"id": "300", "type": "subflow", "name": "Subflow2", "info": "",
         "in": [{"wires": [{"id": "5"}]}],
         "out": [{"wires": [{"id": "5", "port": 0}]}]},
        {"id": "5", "z": "300", "type": "function",
         "func": "msg.payload=msg.payload+'bar'; return msg;", "wires": []}
    ]
    injections = [
        {"nid": "1", "msg": {"payload": "foo"}},
    ]
    msgs = await run_flow_with_msgs_ntimes(flows, injections, 1)
    assert msgs[0]["payload"] == "foobarbaz"
