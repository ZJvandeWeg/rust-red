import json
import pytest
import pytest_asyncio
import asyncio
import os

from tests import *

SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))

def assert_msg_topic_payload(msg, topic, payload):
    assert msg['topic'] == topic
    assert msg['payload'] == payload


@pytest.mark.asyncio
async def test_0001():
    flows_path = os.path.join(SCRIPT_DIR, '0001.json')
    msgs = await run_edgelink(flows_path, 1)
    assert_msg_topic_payload(msgs[0], 'bar', 'foo')

