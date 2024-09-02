import asyncio
import json
import os
import platform
import subprocess
import signal


async def read_json_from_process(num_json):
    # Get the path of the current script
    script_dir = os.path.dirname(os.path.abspath(__file__))

    # Determine the operating system and choose the appropriate executable name
    if platform.system() == 'Windows':
        createion_flags = subprocess.CREATE_NEW_PROCESS_GROUP
        myprog_name = 'edgelinkd.exe'
    elif platform.system() == 'Linux':
        myprog_name = 'edgelinkd'
        createion_flags = 0
    else:
        raise OSError("Unsupported operating system")

    # Construct the full path to the myprog executable
    myprog_path = os.path.join(script_dir, '../target/release', myprog_name)

    # Start the process
    process = await asyncio.create_subprocess_exec(
        myprog_path, '-v', '0',
        stdout=asyncio.subprocess.PIPE,
        stderr=asyncio.subprocess.PIPE,
        creationflags=createion_flags
    )

    # Read from the process's stdout
    buffer = ''
    json_count = 0
    try:
        while True:
            line = await asyncio.wait_for(process.stdout.readline(), timeout=8)
            if not line:
                break
            buffer += line.decode('utf-8')

            # Look for delimiters \x1E and \n
            while '\x1E' in buffer:
                start, rest = buffer.split('\x1E', 1)
                if '\n' in rest:
                    json_str, buffer = rest.split('\n', 1)
                    try:
                        json_obj = json.loads(json_str)
                        yield json_obj  # Yield parsed JSON object
                        json_count += 1
                        if json_count >= num_json:
                            if platform.system() == 'Windows':
                                # Send CTRL+C signal
                                process.send_signal(signal.CTRL_BREAK_EVENT)
                            else:
                                process.send_signal(signal.SIGINT)
                            # Wait for the process to respond and exit
                            await process.wait()  # Wait for the process to finish
                            return
                    except json.JSONDecodeError as e:
                        print(f"JSON decode error: {e}")
                else:
                    break
    except asyncio.TimeoutError as e:
        print("Timeout: No more output in 8 seconds")
        process.kill()
        raise e
        #await asyncio.sleep(2)  # Wait for the process to respond and exit

    # Ensure the process exits completely
    await process.wait()

async def run_edgelink_and_collect_msgs(num_msgs=2) -> list[dict]:
    msgs = []
    async for msg in read_json_from_process(num_json=2):
        msgs.append(msg)
    return msgs


if __name__ == '__main__':
    async def async_main():
        try:
            msgs = await run_edgelink_and_collect_msgs(num_msgs=2)
            print("Received msg JSON:")
            for msg in msgs:
                print(msg)
        except Exception as e:
            print(f"Exception in async_main: {e}")


    # Run the main function
    asyncio.run(async_main())
