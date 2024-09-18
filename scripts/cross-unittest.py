#!/usr/bin/env python3

import subprocess
import sys
import json
import os
import argparse

# 使用 argparse 处理命令行参数
parser = argparse.ArgumentParser(description="Run test binaries using qemu-arm.")
parser.add_argument("toolchain_prefix", help="The toolchain prefix (e.g., arm-linux-gnueabihf)")
parser.add_argument("cargo_output", help="The path to the cargo-output.json file")
args = parser.parse_args()

toolchain_prefix = args.toolchain_prefix
cargo_output_path = args.cargo_output

# 读取 cargo 输出的 JSON 文件
try:
    with open(cargo_output_path, 'r') as f:
        cargo_output = [json.loads(line) for line in f]
except FileNotFoundError:
    print(f"Error: {cargo_output_path} not found. Please run cargo test first.")
    sys.exit(1)

# 过滤出所有测试二进制文件
test_binaries = [
    entry['executable'] for entry in cargo_output
    if entry.get('profile', {}).get('test') == True
]

if not test_binaries:
    print("No test binaries found.")
    sys.exit(0)

# 初始化退出状态
exit_code = 0

# 运行每个测试二进制文件
for test_binary in test_binaries:
    print(f"Running test binary: {test_binary}")
    
    # 使用 subprocess 运行 qemu-arm
    result = subprocess.run([f"qemu-arm", "-L", f"/usr/{toolchain_prefix}", test_binary])
    
    # 如果测试失败，更新退出码
    if result.returncode != 0:
        print(f"Test failed: {test_binary}")
        exit_code = 1

# 返回最终的退出码
sys.exit(exit_code)

