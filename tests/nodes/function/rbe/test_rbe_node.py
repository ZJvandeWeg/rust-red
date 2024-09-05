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


@pytest.mark.asyncio
async def test_0002():
    '''should ignore multiple topics if told to (rbe)'''
    node = {
        "type": "rbe", "func": "rbe", "gap": "0", 'septopics': False,
    }
    injections = [
        {'topic': "a", 'payload': 'a'},
        {'topic': "b", 'payload': 'a'},
        {'topic': "c", 'payload': 'a'},
        {'topic': "a", 'payload': 2.0},
        {'topic': "b", 'payload': 2.0},
        {'payload': {'b': 1.0, 'c': 2.0}},
        {'payload': {'c': 2.0, 'b': 1.0}},
        {'payload': {'c': 2.0, 'b': 1.0}},
        {'topic': "a", 'payload': True},
        {'topic': "b", 'payload': False},
        {'topic': "c", 'payload': False},
        {'topic': "d", 'payload': True},
        {'topic': "a", 'payload': 1.0},
        {'topic': "b", 'payload': 1.0},
        {'topic': "c", 'payload': 1.0},
        {'topic': "d", 'payload': 1.0},
        {'topic': "a", 'payload': 2.0},
    ]
    msgs = await run_single_node_with_msgs_ntimes(node, injections, 8)
    assert msgs[0]['payload'] == 'a'
    assert msgs[1]['payload'] == 2.0
    assert msgs[2]['payload']['b'] == 1.0
    assert msgs[2]['payload']['c'] == 2.0
    assert msgs[3]['payload'] == True
    assert msgs[4]['payload'] == False
    assert msgs[5]['payload'] == True
    assert msgs[6]['topic'] == 'a'
    assert msgs[6]['payload'] == 1.0
    assert msgs[7]['topic'] == 'a'
    assert msgs[7]['payload'] == 2.0


@pytest.mark.asyncio
async def test_0003():
    '''should only send output if another chosen property changes - foo (rbe)'''
    node = {
        "type": "rbe", "func": "rbe", "gap": "0", 'property': 'foo',
    }
    injections = [
        {'foo': "a"},
        {'payload': "a"},
        {'foo': "a"},
        {'payload': "a"},
        {'foo': "a"},
        {'foo': "b"},
        {'foo': {"b": 1.0, "c": 2.0}},
        {'foo': {"c": 2.0, "b": 1.0}},
        {'payload': {"c": 2.0, "b": 1.0}},
    ]
    msgs = await run_single_node_with_msgs_ntimes(node, injections, 3)
    assert msgs[0]['foo'] == 'a'
    assert msgs[1]['foo'] == 'b'
    assert msgs[2]['foo']['b'] == 1.0
    assert msgs[2]['foo']['c'] == 2.0


@pytest.mark.asyncio
async def test_0004():
    '''should only send output if payload changes - ignoring first value (rbei)'''
    node = {
        "type": "rbe", "func": "rbei", "gap": "0"
    }
    injections = [
        {"payload": "a", "topic": "a"},
        {"payload": "a", "topic": "b"},
        {"payload": "a", "topic": "a"},
        {"payload": "b", "topic": "a"},
        {"payload": "b", "topic": "b"},
        {"payload": "c", "topic": "a"},
        {"payload": "c", "topic": "b"},
    ]
    msgs = await run_single_node_with_msgs_ntimes(node, injections, 4)
    assert msgs[0]['payload'] == 'b'
    assert msgs[0]['topic'] == 'a'
    assert msgs[1]['payload'] == 'b'
    assert msgs[1]['topic'] == 'b'
    assert msgs[2]['payload'] == 'c'
    assert msgs[2]['topic'] == 'a'
    assert msgs[3]['payload'] == 'c'
    assert msgs[3]['topic'] == 'b'


@pytest.mark.asyncio
async def test_0005():
    '''should send output if queue is reset (rbe)'''
    node = {
        "type": "rbe", "func": "rbe", "gap": "0"
    }
    injections = [
        {"topic": "a", "payload": "a"},
        {"topic": "a", "payload": "a"},
        {"topic": "b", "payload": "b"},
        {"reset": True},  # reset all
        {"topic": "a", "payload": "a"},
        {"topic": "b", "payload": "b"},
        {"topic": "b", "payload": "b"},
        {"topic": "b", "reset": ""},  # reset b
        {"topic": "b", "payload": "b"},
        {"topic": "a", "payload": "a"},
        {"reset": ""},  # reset all
        {"topic": "b", "payload": "b"},
        {"topic": "a", "payload": "a"},
        {"topic": "c"},  # don't reset a non topic
        {"topic": "b", "payload": "b"},
        {"topic": "a", "payload": "a"},
        {"topic": "c", "payload": "c"},
    ]
    msgs = await run_single_node_with_msgs_ntimes(node, injections, 8)
    assert msgs[0]['payload'] == 'a'
    assert msgs[1]['payload'] == 'b'
    assert msgs[2]['payload'] == 'a'
    assert msgs[3]['payload'] == 'b'
    assert msgs[4]['payload'] == 'b'
    assert msgs[5]['payload'] == 'b'
    assert msgs[6]['payload'] == 'a'
    assert msgs[7]['payload'] == 'c'


@pytest.mark.asyncio
async def test_0006():
    '''should only send output if x away from original value (deadbandEq)'''
    node = {
        "type": "rbe", "func": "deadbandEq", "gap": "10", "inout": "out"
    }
    injections = [
        {"payload": 0.0},
        {"payload": 2.0},
        {"payload": 4.0},
        {"payload": 6.0},
        {"payload": 8.0},
        {"payload": 10.0},
        {"payload": 15.0},
        {"payload": 20.0},
    ]
    msgs = await run_single_node_with_msgs_ntimes(node, injections, 3)
    assert msgs[0]['payload'] == 0.0
    assert msgs[1]['payload'] == 10.0
    assert msgs[2]['payload'] == 20.0


@pytest.mark.asyncio
async def test_0007():
    '''should only send output if more than x away from original value (deadband)'''
    node = {
        "type": "rbe", "func": "deadband", "gap": "10"
    }
    injections = [
        {"payload": 0.0},
        {"payload": 2.0},
        {"payload": 4.0},
        {"payload": "6 deg"},
        {"payload": 8.0},
        {"payload": 20.0},
        {"payload": 15.0},
        {"payload": "5 deg"},
    ]
    msgs = await run_single_node_with_msgs_ntimes(node, injections, 3)
    assert msgs[0]['payload'] == 0.0
    assert msgs[1]['payload'] == 20.0
    assert msgs[2]['payload'] == "5 deg"


@pytest.mark.asyncio
async def test_0008():
    '''should only send output if more than x% away from original value (deadband)'''
    node = {
        "type": "rbe", "func": "deadband", "gap": "10%"
    }
    injections = [
        {"payload": 100.0},
        {"payload": 95.0},
        {"payload": 105.0},
        {"payload": 111.0},
        {"payload": 120.0},
        {"payload": 135.0},
    ]
    msgs = await run_single_node_with_msgs_ntimes(node, injections, 3)
    assert msgs[0]['payload'] == 100.0
    assert msgs[1]['payload'] == 111.0
    assert msgs[2]['payload'] == 135.0

# TODO 'should warn if no number found in deadband mode'


@pytest.mark.asyncio
async def test_0010():
    '''should not send output if x away or greater from original value (narrowbandEq)'''
    node = {
        "type": "rbe", "func": "narrowbandEq", "gap": "10", "inout": "out", "start": "1"
    }
    injections = [
        {"payload": 100.0},
        {"payload": 0.0},
        {"payload": 10.0},
        {"payload": 5.0},
        {"payload": 15.0},
        {"payload": 10.0},
        {"payload": 20.0},
        {"payload": 25.0},
    ]
    msgs = await run_single_node_with_msgs_ntimes(node, injections, 3)
    assert msgs[0]['payload'] == 0.0
    assert msgs[1]['payload'] == 5.0
    assert msgs[2]['payload'] == 10.0


@pytest.mark.asyncio
async def test_0011():
    '''should not send output if more than x away from original value (narrowband)'''
    node = {
        "type": "rbe", "func": "narrowband", "gap": "10"
    }
    injections = [
        {"payload": 0.0},
        {"payload": 20.0},
        {"payload": 40.0},
        {"payload": "6 deg"},
        {"payload": 18.0},
        {"payload": 20.0},
        {"payload": 50.0},
        {"payload": "5 deg"},
    ]
    msgs = await run_single_node_with_msgs_ntimes(node, injections, 3)
    assert msgs[0]['payload'] == 0.0
    assert msgs[1]['payload'] == "6 deg"
    assert msgs[2]['payload'] == "5 deg"


@pytest.mark.asyncio
async def test_0012():
    '''should send output if gap is 0 and input doesnt change (narrowband)'''
    node = {
        "type": "rbe", "func": "narrowband", "gap": "0"
    }
    injections = [
        {"payload": 1.0},
        {"payload": 1.0},
        {"payload": 1.0},
        {"payload": 1.0},
        {"payload": 0.0},
        {"payload": 1.0},
    ]
    msgs = await run_single_node_with_msgs_ntimes(node, injections, 2)
    assert msgs[0]['payload'] == 1.0
    assert msgs[1]['payload'] == 1.0


@pytest.mark.asyncio
async def test_0013():
    '''should not send output if more than x away from original value (narrowband in step mode)'''
    node = {
        "type": "rbe", "func": "narrowband", "gap": "10", "inout": "in", "start": "500"
    }
    injections = [
        {"payload": 50.0},
        {"payload": 55.0},
        {"payload": 200.0},
        {"payload": 205.0},
    ]
    msgs = await run_single_node_with_msgs_ntimes(node, injections, 2)
    assert msgs[0]['payload'] == 55.0
    assert msgs[1]['payload'] == 205.0
