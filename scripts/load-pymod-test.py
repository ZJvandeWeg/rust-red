import os
import sys
import asyncio
import importlib.util


current_script_path = os.path.abspath(__file__)
current_directory = os.path.dirname(current_script_path)
target_directory = os.path.normpath(os.path.join(current_directory, '..', 'target', 'debug'))
module_path = os.path.join(target_directory, "libedgelink_pymod.so")
spec = importlib.util.spec_from_file_location("edgelink_pymod", module_path)
edgelink = importlib.util.module_from_spec(spec)
spec.loader.exec_module(edgelink)


async def main():
    await edgelink.rust_sleep()

# should sleep for 1s
asyncio.run(main())
