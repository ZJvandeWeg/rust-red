import time
import json
import os
import pytest

from tests import *


async def basic_test(type: str, val, rval=None):
    flows = [
        {"id": "100", "type": "tab"},  # flow 1
        {"id": "1", "z": "100", "type": "inject", "once": True, "onceDelay": 0.0, "repeat": "",
            "topic": "t1", "payload": val, "payloadType": type, "wires": [["2"]]},
        {"id": "2", "z": "100", "type": "console-json"}
    ]
    injections = []
    msgs = await run_flow_with_msgs_ntimes(flows, injections, 1)
    assert msgs[0]["topic"] == 't1'
    if rval != None:
        assert msgs[0]["payload"] == rval
    else:
        assert msgs[0]["payload"] == val


@pytest.mark.asyncio
async def test_0001():
    '''should works with simple payload'''
    await basic_test("num", 10)
    await basic_test("str", "10")
    await basic_test("bool", True)
    val_json = '{ "x":"vx", "y":"vy", "z":"vz" }'
    await basic_test("json", val_json, json.loads(val_json))
    val_buf = '[1,2,3,4,5]'
    await basic_test("bin", val_json, bytes(json.loads(val_buf)))


@pytest.mark.asyncio
async def test_0002():
    '''inject value of environment variable'''
    flows = [
        {"id": "100", "type": "tab"},  # flow 1
        {"id": "1", "z": "100", "type": "inject", "once": True, "onceDelay": 0.0, "repeat": "",
            "topic": "t1", "payload": "NR_TEST", "payloadType": "env", "wires": [["2"]]},
        {"id": "2", "z": "100", "type": "console-json"}
    ]
    injections = []
    os.environ["NR_TEST"] = "foo"
    msgs = await run_flow_with_msgs_ntimes(flows, injections, 1)
    del os.environ["NR_TEST"]
    assert msgs[0]["topic"] == 't1'
    assert msgs[0]["payload"] == "foo"


@pytest.mark.asyncio
async def test_0003():
    '''inject name of node as environment variable'''
    flows = [
        {"id": "100", "type": "tab"},  # flow 1
        {"id": "1", "z": "100", "type": "inject", "name": "NAME", "once": True, "onceDelay": 0.0, "repeat": "",
            "topic": "t1", "payload": "NR_NODE_NAME", "payloadType": "env", "wires": [["2"]]},
        {"id": "2", "z": "100", "type": "console-json"}
    ]
    injections = []
    msgs = await run_flow_with_msgs_ntimes(flows, injections, 1)
    assert msgs[0]["topic"] == 't1'
    assert msgs[0]["payload"] == "NAME"


@pytest.mark.asyncio
async def test_0004():
    '''inject id of node as environment variable'''
    flows = [
        {"id": "100", "type": "tab"},  # flow 1
        {"id": "1", "z": "100", "type": "inject", "name": "NAME", "once": True, "onceDelay": 0.0, "repeat": "",
            "topic": "t1", "payload": "NR_NODE_ID", "payloadType": "env", "wires": [["2"]]},
        {"id": "2", "z": "100", "type": "console-json"}
    ]
    injections = []
    msgs = await run_flow_with_msgs_ntimes(flows, injections, 1)
    assert msgs[0]["topic"] == 't1'
    assert msgs[0]["payload"] == "1"


@pytest.mark.asyncio
async def test_0005():
    '''inject path of node as environment variable'''
    flows = [
        {"id": "100", "type": "tab"},  # flow 1
        {"id": "1", "z": "100", "type": "inject", "name": "NAME", "once": True, "onceDelay": 0.0, "repeat": "",
            "topic": "t1", "payload": "NR_NODE_PATH", "payloadType": "env", "wires": [["2"]]},
        {"id": "2", "z": "100", "type": "console-json"}
    ]
    injections = []
    msgs = await run_flow_with_msgs_ntimes(flows, injections, 1)
    assert msgs[0]["topic"] == 't1'
    assert msgs[0]["payload"] == "100/1"  # TODO FIXME


@pytest.mark.asyncio
async def test_0006():
    '''inject name of flow as environment variable'''
    flows = [
        {"id": "100", "type": "tab", "label": "FLOW"},  # flow 1
        {"id": "1", "z": "100", "type": "inject", "name": "NAME", "once": True, "onceDelay": 0.0, "repeat": "",
            "topic": "t1", "payload": "NR_FLOW_NAME", "payloadType": "env", "wires": [["2"]]},
        {"id": "2", "z": "100", "type": "console-json"}
    ]
    injections = []
    msgs = await run_flow_with_msgs_ntimes(flows, injections, 1)
    assert msgs[0]["topic"] == 't1'
    assert msgs[0]["payload"] == "FLOW"


@pytest.mark.asyncio
async def test_0007():
    '''inject id of flow as environment variable'''
    flows = [
        {"id": "100", "type": "tab"},  # flow 1
        {"id": "1", "z": "100", "type": "inject", "name": "NAME", "once": True, "onceDelay": 0.0, "repeat": "",
            "topic": "t1", "payload": "NR_FLOW_ID", "payloadType": "env", "wires": [["2"]]},
        {"id": "2", "z": "100", "type": "console-json"}
    ]
    injections = []
    msgs = await run_flow_with_msgs_ntimes(flows, injections, 1)
    assert msgs[0]["topic"] == 't1'
    assert msgs[0]["payload"] == "100"  # TODO FIXME


@pytest.mark.asyncio
async def test_0008():
    '''inject name of group as environment variable'''
    flows = [
        {"id": "100", "type": "tab"},  # flow 1
        {"id": "1", "z": "100", "type": "inject", "name": "NAME", "once": True, "onceDelay": 0.0, "repeat": "",
         "g": "FF", "topic": "t1", "payload": "NR_GROUP_NAME", "payloadType": "env", "wires": [["2"]]},
        {"id": "2", "z": "100", "type": "console-json"},
        {"id": "FF", "z": "100", "type": "group", "name": "GROUP"}
    ]
    injections = []
    msgs = await run_flow_with_msgs_ntimes(flows, injections, 1)
    assert msgs[0]["topic"] == 't1'
    assert msgs[0]["payload"] == "GROUP"


@pytest.mark.asyncio
async def test_0009():
    '''inject id of group as environment variable'''
    flows = [
        {"id": "100", "type": "tab"},  # flow 1
        {"id": "1", "z": "100", "type": "inject", "name": "NAME", "once": True, "onceDelay": 0.0, "repeat": "",
         "g": "FF", "topic": "t1", "payload": "NR_GROUP_ID", "payloadType": "env", "wires": [["2"]]},
        {"id": "2", "z": "100", "type": "console-json"},
        {"id": "FF", "z": "100", "type": "group", "name": "GROUP"}
    ]
    injections = []
    msgs = await run_flow_with_msgs_ntimes(flows, injections, 1)
    assert msgs[0]["topic"] == 't1'
    assert msgs[0]["payload"] == "FF"  # TODO FIXME


@pytest.mark.asyncio
async def test_0010():
    '''inject name of node as environment variable by substitution'''
    flows = [
        {"id": "100", "type": "tab"},  # flow 1
        {"id": "1", "z": "100", "type": "inject", "name": "NAME", "once": True, "onceDelay": 0.0, "repeat": "",
         "topic": "t1", "payload": r"${NR_NODE_NAME}", "payloadType": "str", "wires": [["2"]]},
        {"id": "2", "z": "100", "type": "console-json"},
    ]
    injections = []
    msgs = await run_flow_with_msgs_ntimes(flows, injections, 1)
    assert msgs[0]["topic"] == 't1'
    assert msgs[0]["payload"] == "NAME"


@pytest.mark.asyncio
async def test_0011():
    '''inject id of node as environment variable by substitution'''
    flows = [
        {"id": "100", "type": "tab"},  # flow 1
        {"id": "1", "z": "100", "type": "inject", "name": "NAME", "once": True, "onceDelay": 0.0, "repeat": "",
         "topic": "t1", "payload": r"${NR_NODE_ID}", "payloadType": "str", "wires": [["2"]]},
        {"id": "2", "z": "100", "type": "console-json"},
    ]
    injections = []
    msgs = await run_flow_with_msgs_ntimes(flows, injections, 1)
    assert msgs[0]["topic"] == 't1'
    assert msgs[0]["payload"] == "FF"  # FIXME


"""
@pytest.mark.asyncio
async def test_0101():
    '''sets the value of flow context property'''
    flows = [
        {"id": "100", "type": "tab"},  # flow 1
        {"id": "1", "z": "100", "type": "inject", "name": "NAME", "once": True, "onceDelay": 0.0, "repeat": "",
         "topic": "t1", "payload": "flowValue", "payloadType": "flow", "wires": [["2"]]},
        {"id": "2", "z": "100", "type": "console-json"},
    ]
    injections = []
    msgs = await run_flow_with_msgs_ntimes(flows, injections, 1)
    assert msgs[0]["topic"] == 't1'
    assert msgs[0]["payload"] == "changeMe"
"""


@pytest.mark.asyncio
async def test_0201():
    '''should inject once with default delay property'''
    flows = [
        {"id": "100", "type": "tab"},  # flow 1
        {"id": "1", "z": "100", "type": "inject", "once": True,
         "topic": "t1", "payload": "", "payloadType": "date", "wires": [["2"]]},
        {"id": "2", "z": "100", "type": "console-json"},
    ]
    injections = []
    msgs = await run_flow_with_msgs_ntimes(flows, injections, 1)
    assert msgs[0]["topic"] == 't1'
    assert msgs[0]["payload"] > 1  # TODO CHECK TYPE


@pytest.mark.asyncio
async def test_0202():
    '''should inject once with default delay'''
    flows = [
        {"id": "100", "type": "tab"},  # flow 1
        {"id": "1", "z": "100", "type": "inject", "once": True,
         "topic": "t1", "payload": "", "payloadType": "date", "wires": [["2"]]},
        {"id": "2", "z": "100", "type": "console-json"},
    ]
    injections = []
    msgs = await run_flow_with_msgs_ntimes(flows, injections, 1)
    assert msgs[0]["topic"] == 't1'
    assert msgs[0]["payload"] < time.time()


@pytest.mark.asyncio
async def test_0203():
    '''should inject once with 500 msec. delay'''
    flows = [
        {"id": "100", "type": "tab"},  # flow 1
        {"id": "1", "z": "100", "type": "inject", "once": True, "onceDelay": 0.5,
         "topic": "t1", "payload": "", "payloadType": "date", "wires": [["2"]]},
        {"id": "2", "z": "100", "type": "console-json"},
    ]
    injections = []
    msgs = await run_flow_with_msgs_ntimes(flows, injections, 1)
    assert msgs[0]["topic"] == 't1'
    assert msgs[0]["payload"] < time.time()


"""
@pytest.mark.asyncio
async def test_0204():
    '''should inject once with delay of two seconds'''
    flows = [
        {"id": "100", "type": "tab"},  # flow 1
        {"id": "1", "z": "100", "type": "inject", "once": True, "onceDelay": 0.5,
         "topic": "t1", "payload": "", "payloadType": "date", "wires": [["2"]]},
        {"id": "2", "z": "100", "type": "console-json"},
    ]
    injections = []
    msgs = await run_flow_with_msgs_ntimes(flows, injections, 1)
    assert msgs[0]["topic"] == 't1'
    assert msgs[0]["payload"] < time.time()
"""


@pytest.mark.asyncio
async def test_0205():
    '''should inject repeatedly'''
    flows = [
        {"id": "100", "type": "tab"},  # flow 1
        {"id": "1", "z": "100", "type": "inject", "repeat": 0.2,
         "topic": "t2", "payload": "payload", "wires": [["2"]]},
        {"id": "2", "z": "100", "type": "console-json"},
    ]
    injections = []
    msgs = await run_flow_with_msgs_ntimes(flows, injections, 2)
    assert msgs[0]["topic"] == 't1'
    assert msgs[0]["payload"] == "payload"
    assert msgs[1]["topic"] == 't1'
    assert msgs[1]["payload"] == "payload"

# 0206 should inject once with delay of two seconds and repeatedly

@pytest.mark.asyncio
async def test_0207():
    '''should inject with cron'''
    flows = [
        {"id": "100", "type": "tab"},  # flow 1
        {"id": "1", "z": "100", "type": "inject", "crontab": "* * * * * *",
         "topic": "t3", "payloadType": "date", "wires": [["2"]]},
        {"id": "2", "z": "100", "type": "console-json"},
    ]
    injections = []
    msgs = await run_flow_with_msgs_ntimes(flows, injections, 2)
    assert msgs[0]["topic"] > 1
    assert msgs[0]["payload"] == "payload"
    assert msgs[1]["topic"] == 't3'
    assert msgs[1]["payload"] > 1

