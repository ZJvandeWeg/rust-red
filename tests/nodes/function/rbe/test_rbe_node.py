import json
import pytest
import pytest_asyncio
import asyncio
import os

from tests import *

SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))

# should only send output if payload changes - with multiple topics (rbe)


@pytest.mark.asyncio
async def test_0001():
    node = {'type': 'rbe', 'func': 'rbe', 'gap': '0'}
    msgs = await run_with_single_node_ntimes('num', '0.1', node, 1, once=False, topic='t1')
    assert msgs[0]['topic'] == 't1'
