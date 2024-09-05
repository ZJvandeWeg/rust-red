import json
import pytest
import pytest_asyncio
import asyncio
import os

from tests import *

SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))


@pytest.mark.asyncio
async def test_0001():
    '''should only send output if payload changes - with multiple topics (rbe)'''
    node = {
        "type": "rbe", "func": "rbe", "gap": "0"
    }
    injections = [
        {'payload': 'a'},
        {'payload': 'a'},
        {'payload': 'a'},
        {'payload': 2.0},
        {'payload': 2.0},
        {'payload': {'b': 1.0, 'c': 2.0}},
        {'payload': {'c': 2.0, 'b': 1.0}},
        {'payload': {'c': 2.0, 'b': 1.0}},
        {'payload': True},
        {'payload': False},
        {'payload': False},
        {'payload': True},
        {'topic': "a", 'payload': 1.0},
        {'topic': "b", 'payload': 1.0},
        {'topic': "b", 'payload': 1.0},
        {'topic': "a", 'payload': 1.0},
        {'topic': "c", 'payload': 1.0},
    ]
    msgs = await run_single_node_with_msgs_ntimes(node, injections, 9)
    print(msgs)
    assert msgs[0]['payload'] == 'a'
    assert msgs[1]['payload'] == 2.0
    assert msgs[2]['payload']['b'] == 1.0
    assert msgs[2]['payload']['c'] == 2.0
    assert msgs[3]['payload'] == True
    assert msgs[4]['payload'] == False
    assert msgs[5]['payload'] == True
    assert msgs[6]['topic'] == 'a'
    assert msgs[6]['payload'] == 1.0
    assert msgs[7]['topic'] == 'b'
    assert msgs[7]['payload'] == 1.0
    assert msgs[8]['topic'] == 'c'
    assert msgs[8]['payload'] == 1.0
