import json
import pytest
import pytest_asyncio
import asyncio
import os

from tests import *

SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))

@pytest.mark.asyncio
async def test_0001():
    flows_path = os.path.join(SCRIPT_DIR, '0001.json')
    msgs = await run_edgelink(flows_path, 1)
    assert msgs[0]['topic'] == 'bar'
    assert msgs[0]['payload'] == 'foo'

@pytest.mark.asyncio
async def test_0002():
    flows_path = os.path.join(SCRIPT_DIR, '0002.json')
    msgs = await run_edgelink(flows_path, 2)
    print(msgs)
    assert msgs[0]['topic'] == msgs[1]['topic']
    assert msgs[0]['payload'] != msgs[1]['payload']
    assert sorted([msgs[0]['payload'], msgs[1]['payload']]) == ['foo', 'p2']

