import json
import pytest
import pytest_asyncio
import asyncio
import os

from tests import *

SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))

def assert_msg_topic_payload(msg, payload):
    assert msg['topic'] == 't1'
    assert msg['payload'] == payload


@pytest.mark.asyncio
async def test_inject_basic_num():
    flows_path = os.path.join(SCRIPT_DIR, 'basic_num.json')
    msgs = await run_edgelink(flows_path, 1)
    assert_msg_topic_payload(msgs[0], 10)


@pytest.mark.asyncio
async def test_inject_basic_str():
    flows_path = os.path.join(SCRIPT_DIR, 'basic_str.json')
    msgs = await run_edgelink(flows_path, 2)
    msgs = await run_edgelink(flows_path, 1)
    assert_msg_topic_payload(msgs[0], '10')


@pytest.mark.asyncio
async def test_inject_basic_bool():
    flows_path = os.path.join(SCRIPT_DIR, 'basic_bool.json')
    msgs = await run_edgelink(flows_path, 2)
    assert_msg_topic_payload(msgs[0], True)


@pytest.mark.asyncio
async def test_inject_basic_json():
    flows_path = os.path.join(SCRIPT_DIR, 'basic_json.json')
    expected = json.loads('{ "x":"vx", "y":"vy", "z":"vz" }')
    msgs = await run_edgelink(flows_path, )
    assert_msg_topic_payload(msgs[0], expected)

@pytest.mark.asyncio
async def test_inject_basic_bin():
    flows_path = os.path.join(SCRIPT_DIR, 'basic_bin.json')
    expected = json.loads('[1,2,3,4,5]')
    msgs = await run_edgelink(flows_path, )
    assert_msg_topic_payload(msgs[0], expected)
