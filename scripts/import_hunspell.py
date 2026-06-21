#!/usr/bin/env python3
"""
TypeFix Hunspell Importer
-------------------------
This script converts the output of Hunspell's 'unmunch' command 
into the optimized JSON format required by TypeFix, which can then 
be compiled into an ultra-fast FST dictionary.

Usage:
1. Generate the expanded wordlist using Hunspell tools:
   unmunch es_ES.dic es_ES.aff > expanded.txt

2. Run this script to generate the JSON dictionary:
   python import_hunspell.py expanded.txt es.json
   
3. Compile to FST using TypeFix:
   typefix build-dict es.json es.fst
"""

import sys
import json
import argparse
import re

def main():
    parser = argparse.ArgumentParser(description="Convert a raw wordlist (e.g. from unmunch) to TypeFix JSON format.")
    parser.add_argument("input_txt", help="Path to the expanded wordlist text file")
    parser.add_argument("output_json", help="Path to the output JSON file (e.g. es.json)")
    parser.add_argument("--default-freq", type=int, default=10, help="Default frequency for expanded words")
    
    args = parser.parse_args()
    
    print(f"Reading wordlist from {args.input_txt}...")
    words_map = {}
    
    try:
        with open(args.input_txt, 'r', encoding='utf-8', errors='ignore') as f:
            for line in f:
                word = line.strip().lower()
                # Keep alphanumeric, apostrophes and hyphens
                word = re.sub(r'[^\w\-\']', '', word)
                # Trim edge punctuation
                word = word.strip("'-")
                
                if word and len(word) > 1:
                    words_map[word] = args.default_freq
    except Exception as e:
        print(f"Error reading input file: {e}")
        sys.exit(1)
        
    print(f"Total unique valid words extracted: {len(words_map)}")
    
    # Extract language code from filename (e.g. es.json -> es)
    import os
    lang_code = os.path.basename(args.output_json).split('.')[0][:2]
    
    output_struct = {
        "language": lang_code,
        "version": "1.0",
        "words": [
            {"word": w, "frequency": f} for w, f in words_map.items()
        ]
    }
    
    print(f"Saving to {args.output_json}...")
    try:
        with open(args.output_json, 'w', encoding='utf-8') as f:
            json.dump(output_struct, f, ensure_ascii=False, separators=(',', ':'))
    except Exception as e:
        print(f"Error writing output file: {e}")
        sys.exit(1)
        
    print("Done!")
    print(f"You can now compile it to FST with:")
    print(f"  cargo run --bin typefix -- build-dict {args.output_json} {args.output_json.replace('.json', '.fst')}")

if __name__ == '__main__':
    main()
