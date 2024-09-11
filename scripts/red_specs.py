#!/bin/python

import re
import argparse
import ast

def extract_it_strings(file_path):
    pattern = re.compile(r"""\s*it\(['"](.*?)['"],\s*""")
    counter = 1

    with open(file_path, 'r', encoding='utf-8') as file:
        for line in file:
            matches = pattern.findall(line)
            for match in matches:
                try:
                    escaped_string = ast.literal_eval(f'"{match}"')
                    print(f"{counter:04d}|\"{escaped_string}\"")
                    counter += 1
                except SyntaxError:
                    print(f"Unable to parse string: {match}")

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Scan a .js file, extract lines containing it('arbitrary text') or it(\"arbitrary text\"), and print the text with a four-digit number prefix.")
    parser.add_argument('file_path', type=str, help="Path to the .js file to scan")

    args = parser.parse_args()
    extract_it_strings(args.file_path)