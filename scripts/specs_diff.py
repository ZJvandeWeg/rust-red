#!/bin/python

import re
import argparse
import ast
import difflib
import os
import json

from colorama import init as colorama_init
from colorama import Fore
from colorama import Style



_SCRIPT_PATH = os.path.abspath(__file__)
_SCRIPT_DIR = os.path.dirname(_SCRIPT_PATH)
TESTS_DIR = os.path.join(_SCRIPT_DIR, '..', "tests")

JS_IT_PATTERN = re.compile(r"""^\s*it\s*\(\s*(['"].*?['"]+)\s*,\s*""")
PY_IT_PATTERN = re.compile(r"""\@.*it\s*\(\s*(['"].*?['"]+)\s*\)\s*""")


def extract_it_strings_js(file_path):
    counter = 1
    specs = []
    with open(file_path, 'r', encoding='utf-8') as file:
        for line in file:
            matches = JS_IT_PATTERN.findall(line)
            for match in matches:
                try:
                    escaped_string = ast.literal_eval(match)
                    specs.append(escaped_string)
                    # print(f"{counter:04d}|\"{escaped_string}\"")
                    counter += 1
                except SyntaxError as e:
                    print(f"Unable to parse JS string: {match}")
                    raise e
    return specs


def extract_it_strings_py(file_path):
    counter = 1
    specs = []
    with open(file_path, 'r', encoding='utf-8') as file:
        for line in file:
            matches = PY_IT_PATTERN.findall(line)
            for match in matches:
                try:
                    escaped_string = ast.literal_eval(match)
                    specs.append(escaped_string)
                    # print(f"{counter:04d}|\"{escaped_string}\"")
                    counter += 1
                except SyntaxError as e:
                    print(f"Unable to parse Python string: {match}")
                    raise e
    return specs


def read_pairs() -> list[list]:
    json_path = os.path.join(_SCRIPT_DIR, 'specs_diff.json')
    with open(json_path, 'r', encoding='utf-8') as file:
        json_text = file.read()
        return json.loads(json_text)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(
        description="Scan a .js file, extract lines containing it('arbitrary text') or it(\"arbitrary text\"), and print the text with a four-digit number prefix.")
    parser.add_argument('NR_PATH', type=str,
                        help="Path to the directory of Node-RED")
    args = parser.parse_args()

    colorama_init()

    pairs = read_pairs()

    total_diff_count = 0
    for p in pairs:
        js_path = os.path.join(args.NR_PATH, p[1])
        py_path = os.path.join(os.path.normpath(os.path.join(TESTS_DIR, p[0])))
        js_specs = extract_it_strings_js(js_path)
        py_specs = extract_it_strings_py(py_path)

        diff = difflib.Differ().compare(js_specs, py_specs)
        differences = [line for line in diff if line.startswith('-') or line.startswith('+')]
        # differences = [line for line in diff]
        total_diff_count += len(differences)
        if len(differences) > 0:
            print(f'''{Fore.WHITE}{p[0]}({len(py_specs)}/{len(js_specs)}):{Style.RESET_ALL}''')
            for s in differences:
                if s[0] == '-':
                    print(f'''\t{Fore.RED}{s[0]}{Style.RESET_ALL}{s[1:]}''')
                elif s[0] == '+':
                    print(f'''\t{Fore.GREEN}{s[0]}{Style.RESET_ALL}{s[1:]}''')
                else:
                    print(f'''\t{Style.DIM}{s}{Style.RESET_ALL}''')


    if total_diff_count > 0:
        exit(-1)
    else:
        exit(0)
