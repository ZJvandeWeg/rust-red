import json
import pytest
import pytest_asyncio
import asyncio
import os

from tests import *

SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))


async def _generic_range_test(action, minin, maxin, minout, maxout, round, a_payload, expected_result):
    node = {"type": "range", "minin": minin, "maxin": maxin, "minout": minout,
            "maxout": maxout, "action": action, "round": round}
    msgs = await run_with_single_node_ntimes('num', a_payload, node, 1, once=True, topic='t1')
    assert msgs[0]['payload'] == expected_result


@ pytest.mark.asyncio
async def test_0001():
    '''ranges numbers up tenfold'''
    await _generic_range_test("scale", 0, 100, 0, 1000, False, 50, 500)


@ pytest.mark.asyncio
async def test_0002():
    '''ranges numbers down such as centimetres to metres'''
    await _generic_range_test("scale", 0, 100, 0, 1, False, 55, 0.55)


@ pytest.mark.asyncio
async def test_0003():
    '''wraps numbers down say for degree/rotation reading 1/2'''
    # 1/2 around wrap => "one and a half turns"
    await _generic_range_test("roll", 0, 10, 0, 360, True, 15, 180)


@ pytest.mark.asyncio
async def test_0004():
    '''wraps numbers around say for degree/rotation reading 1/3'''
    # 1/3 around wrap => "one and a third turns"
    await _generic_range_test("roll", 0, 10, 0, 360, True, 13.3333, 120)


@ pytest.mark.asyncio
async def test_0005():
    '''wraps numbers around say for degree/rotation reading 1/4'''
    # 1/4 around wrap => "one and a quarter turns"
    await _generic_range_test("roll", 0, 10, 0, 360, True, 12.5, 90)


@ pytest.mark.asyncio
async def test_0006():
    '''wraps numbers down say for degree/rotation reading 1/4'''
    # 1/4 backwards wrap => "one and a quarter turns backwards"
    await _generic_range_test("roll", 0, 10, 0, 360, True, -12.5, 270)


@ pytest.mark.asyncio
async def test_0007():
    '''wraps numbers around say for degree/rotation reading 0'''
    await _generic_range_test("roll", 0, 10, 0, 360, True, -10, 0)


@ pytest.mark.asyncio
async def test_0008():
    '''clamps numbers within a range - over max'''
    await _generic_range_test("clamp", 0, 10, 0, 1000, False, 111, 1000)


@ pytest.mark.asyncio
async def test_0009():
    '''clamps numbers within a range - below min'''
    await _generic_range_test("clamp", 0, 10, 0, 1000, False, -1, 0)


@ pytest.mark.asyncio
async def test_0010():
    '''drops msg if in drop mode and input outside range'''
    node = {
        "id": "rangeNode1", "type": "range", "minin": 2, "maxin": 8, "minout": 20, "maxout": 80,
        "action": "drop", "round": True, "name": "rangeNode"
    }
    injections = [
        {'payload': 1.0},
        {'payload': 9.0},
        {'payload': 5.0},
    ]
    msgs = await run_single_node_with_msgs_ntimes(node, injections, 1)
    assert msgs[0]['payload'] == 50


@ pytest.mark.asyncio
async def test_0011():
    '''just passes on msg if payload not present'''
    node = {
        "id": "rangeNode1", "type": "range", "minin": 0, "maxin": 100, "minout": 0, "maxout": 100,
        "action": "scale", "round": True, "name": "rangeNode"
    }
    injections = [ {} ]
    msgs = await run_single_node_with_msgs_ntimes(node, injections, 1)
    assert 'payload' not in msgs[0]

# TODO: 'reports if input is not a number'
