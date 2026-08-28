#!/usr/bin/env python3
"""Print a word range from two .xuv files side by side, marking differences.

Usage: python3 tools/show.py A.xuv B.xuv <lo_hex> <hi_hex>
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from xuv import load


def ascii_pair(v):
    if v is None:
        return '..'
    return ''.join(chr(c) if 32 <= c < 127 else '.' for c in (v & 0xFF, v >> 8))


def main():
    a, b = load(sys.argv[1]), load(sys.argv[2])
    lo, hi = int(sys.argv[3], 16), int(sys.argv[4], 16)
    for x in range(lo, hi + 1):
        va, vb = a.get(x), b.get(x)
        mark = '  <-- DIFF' if va != vb else ''
        print(f"@{x:06X}  A={va:04X} '{ascii_pair(va)}'   "
              f"B={vb:04X} '{ascii_pair(vb)}'{mark}")


if __name__ == '__main__':
    main()
