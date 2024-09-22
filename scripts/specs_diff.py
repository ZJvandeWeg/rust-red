#!/bin/python3

import re
import argparse
import ast
import difflib
import os
import json
import shutil
import subprocess
import tempfile
import pytest
import io
import contextlib

from colorama import init as colorama_init
from colorama import Fore
from colorama import Style


_SCRIPT_PATH = os.path.abspath(__file__)
_SCRIPT_DIR = os.path.dirname(_SCRIPT_PATH)
TESTS_DIR = os.path.join(_SCRIPT_DIR, '..', "tests")

JS_IT_PATTERN = re.compile(r"""^\s*it\s*\(\s*(['"].*?['"]+)\s*,\s*""")
PY_IT_PATTERN = re.compile(r"""\@.*it\s*\(\s*(['"].*?['"]+)\s*\)\s*""")

def load_json(json_path): 
    with open(json_path, 'r') as fp:
        return json.load(fp)


def extract_it_strings_js(red_dir, file_path) -> list[str]:
    specs = []
    with tempfile.NamedTemporaryFile(delete=True) as report_file:
        original_cwd = os.getcwd()
        os.chdir(red_dir)
        try:
            result = subprocess.run([
                'mocha', 
                file_path, "--dry-run", "--reporter=json", "--exit", 
                "--reporter-options", f"output={report_file.name}"
            ])
            report = load_json(report_file.name)
            for test in report['tests']:
                specs.append(test['fullTitle'].rstrip())
        finally:
            os.chdir(original_cwd)

    return specs


def extract_it_strings_py(file_path) -> list[str]:
    specs = []
    with tempfile.NamedTemporaryFile(delete=True) as report_file:
        output_capture = io.StringIO()
        with contextlib.redirect_stdout(output_capture), contextlib.redirect_stderr(output_capture):
            pytest.main(["-q", "--co", "--disable-warnings", "-p", "no:skip", "--json-report", f"--json-report-file={report_file.name}", file_path])
        report = load_json(report_file.name)
        for coll in report['collectors']:
            for result in coll['result']:
                if "title" in result:
                    specs.append(result['fullTitle'].rstrip())
    return specs


def read_pairs() -> list[list]:
    json_path = os.path.join(_SCRIPT_DIR, 'specs_diff.json')
    with open(json_path, 'r', encoding='utf-8') as file:
        json_text = file.read()
        return json.loads(json_text)


def print_sep(text=''):
    terminal_size = shutil.get_terminal_size()
    filled_text = text.ljust(terminal_size.columns, '-')
    print(filled_text)

def print_subtitle(text=''):
    terminal_size = shutil.get_terminal_size()
    filled_text = text.ljust(terminal_size.columns, '.')
    print(filled_text)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(
        description="Scan a .js file, extract lines containing it('arbitrary text') or it(\"arbitrary text\"), and print the text with a four-digit number prefix.")
    parser.add_argument('NR_PATH', type=str,
                        help="Path to the directory of Node-RED")
    args = parser.parse_args()

    colorama_init()

    pairs = read_pairs()

    total_js_count = 0
    total_py_count = 0
    for p in pairs:
        js_path = os.path.join(args.NR_PATH, p[1])
        py_path = os.path.join(os.path.normpath(os.path.join(TESTS_DIR, p[0])))
        js_specs = extract_it_strings_js(args.NR_PATH, js_path)
        py_specs = extract_it_strings_py(py_path)

        diff = difflib.Differ().compare(js_specs, py_specs)
        #differences = [line for line in diff if line.startswith(
        #    '-') or line.startswith('+')]
        differences = [line for line in diff]
        total_js_count += len(js_specs)
        total_py_count += len(py_specs)
        if len(py_specs) >= len(js_specs):
            print_subtitle(
                f'''{Fore.GREEN}* [✓]{Style.RESET_ALL} "{p[0]}" ({len(py_specs)}/{len(js_specs)}) ''')
        else:
            print_subtitle(
                f'''{Fore.RED}* [×]{Style.RESET_ALL} "{p[0]}" {Fore.RED}({len(py_specs)}/{len(js_specs)}){Style.RESET_ALL} ''')
        for s in differences:
            if s[0] == '-':
                print(f'\t{Fore.RED}{s[0]} It: {Style.RESET_ALL}{s[2:]}')
            elif s[0] == '+':
                print(f'\t{Fore.GREEN}{s[0]} It: {Style.RESET_ALL}{s[2:]}')
            elif s[0] == '?':
                print(f'\t{Fore.YELLOW}{s[0]} It: {Style.RESET_ALL}{s[2:]}')
            else:
                print(f'\t{Style.DIM}{s[0]} It: {s[2:]}{Style.RESET_ALL}')

    print_sep("")
    print("Total:")
    print(f"JS specs:\t{str(total_js_count).rjust(8)}")
    print(f"Python specs:\t{str(total_py_count).rjust(8)}")
    pc = "{:>{}.1%}".format(total_py_count * 1.0 / total_js_count, 8)
    print(f"Percent:\t{pc}")

    if total_py_count < total_js_count:
        exit(-1)
    else:
        exit(0)
