import pytest
import os

from tests import *

# 0001 should do something with the catch node

@pytest.mark.describe('function Node')
class TestFunctionNode:

    @pytest.mark.asyncio
    @pytest.mark.it('''should send returned message''')
    async def test_0002(self):
        node = {
            "type": "function",
            "func": "return msg;",
        }
        msgs = await run_with_single_node_ntimes('str', 'foo', node, 1, once=True, topic='bar')
        assert msgs[0]['topic'] == 'bar'
        assert msgs[0]['payload'] == 'foo'

    # 0004 should send returned message using send()
    # 0005 should allow accessing node.id and node.name and node.outputCount

    # 0006 should clone single message sent using send()
    # 0007 should not clone single message sent using send(,false)

    # 0008 should clone first message sent using send() - array 1
    # 0009 should clone first message sent using send() - array 2
    # 0010 should clone first message sent using send() - array 3
    # 0011 should clone first message sent using send() - array 3
    # 0012 should pass through _topic

    # TODO FIXME

    @pytest.mark.asyncio
    @pytest.mark.it('''should send to multiple outputs''')
    async def test_it_should_send_to_multiple_outputs(self):
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

    # 0014 should send to multiple messages

    class TestEnvVar:
        def setup_method(self, method):
            os.environ["_TEST_FOO_"] = "hello"

        def teardown_method(self, method):
            del os.environ["_TEST_FOO_"]

        @pytest.mark.asyncio
        @pytest.mark.it('should allow accessing env vars')
        async def test_it_should_send_to_multiple_outputs(self):
            node = {
                "type": "function",
                "func": "msg.payload = env.get('_TEST_FOO_'); return msg;",
                "wires": [["3"]]
            }
            msgs = await run_with_single_node_ntimes(payload_type='str', payload='foo', node_json=node, nexpected=1, once=True, topic='bar')
            assert msgs[0]['topic'] == 'bar'
            assert msgs[0]['payload'] == 'hello'


    @pytest.mark.asyncio
    @pytest.mark.it('should allow accessing node.id and node.name and node.outputCount')
    async def test_it_should_allow_accessing_node_id_and_node_name_and_node_output_count(self):
            flows = [
                {"id": "100", "type": "tab"},  # flow 1
                {"id": "1", "type": "function", "z": "100", "name":"test-function", "wires": [["2"]], "outputs": 2,
                    "func": "return [{ topic: node.name, payload:node.id, outputCount: node.outputCount }];",
                    },
                {"id": "2", "z": "100", "type": "console-json"}
            ]
            injections = [
                {"nid": "1", "msg": {'payload': ''}},
            ]
            msgs = await run_flow_with_msgs_ntimes(flows, injections, 1)
            assert msgs[0]["payload"] == "0000000000000001"
            assert msgs[0]["topic"] == "test-function"
            assert msgs[0]["outputCount"] == 2