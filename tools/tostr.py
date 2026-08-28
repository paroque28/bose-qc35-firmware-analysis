#!/usr/bin/env python3
"""Extract a word range from a .xuv file as a byte string (big-endian per word).

Usage: python3 tools/tostr.py FILE.xuv <lo_hex> <hi_hex>
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from xuv import load


def main():
    w = load(sys.argv[1])
    lo, hi = int(sys.argv[2], 16), int(sys.argv[3], 16)
    bs = bytearray()
    for x in range(lo, hi + 1):
        v = w.get(x, 0)
        bs += bytes([v >> 8, v & 0xFF])
    print(repr(bs.decode('latin1')))


if __name__ == '__main__':
    main()
