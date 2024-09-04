import pytest
import os

from tests import *

SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))


@pytest.mark.asyncio
async def test_0001():
    node = {
        "type": "function",
        "func": "return msg;",
    }
    msgs = await run_with_single_node_ntimes('str', 'foo', node, 1, once=True, topic='bar')
    assert msgs[0]['topic'] == 'bar'
    assert msgs[0]['payload'] == 'foo'


@pytest.mark.asyncio
async def test_0002():
    node = {
        "type": "function",
        "func": "var msg2 = RED.util.cloneMessage(msg); msg2.payload='p2'; return [msg, msg2];",
        "wires": [["3"], ["3"]]
    }
    msgs = await run_with_single_node_ntimes('str', 'foo', node, 2, once=True, topic='bar')
    assert msgs[0]['topic'] == 'bar'
    assert msgs[0]['topic'] == msgs[1]['topic']
    assert msgs[0]['payload'] != msgs[1]['payload']
    assert sorted([msgs[0]['payload'], msgs[1]['payload']]) == ['foo', 'p2']
