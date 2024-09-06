import json
import pytest
import time

from tests import *

@pytest.mark.asyncio
async def test_simple():
    '''should output JSON'''
    flows = [
        {"id": "100", "type": "tab"},  # flow 1
        {"id": "1", "z": "100", "type": "console-json"}
    ]
    injections = [
        {'nid': '1', 'msg': {'payload': 'hello'} },
        {'nid': '1', 'msg': {'payload': 'world'} }
    ]
    msgs = await run_flow_with_msgs_ntimes(flows, injections, 2)
    assert msgs[0]["payload"] == 'hello'
    assert msgs[1]["payload"] == 'world'

