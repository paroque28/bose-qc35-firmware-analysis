#!/usr/bin/env python3
"""Parse Bose .xuv files (CSR/Qualcomm 16-bit word dumps) and diff two of them."""
import sys, re

LINE = re.compile(rb'^@([0-9A-Fa-f]+)\s+([0-9A-Fa-f]{4})\s*$')

def load(path):
    words = {}
    with open(path, 'rb') as fh:
        for raw in fh:
            m = LINE.match(raw.strip())
            if m:
                words[int(m.group(1), 16)] = int(m.group(2), 16)
    return words

def ranges(addrs):
    """Collapse a sorted address list into contiguous runs."""
    out = []
    for a in addrs:
        if out and a == out[-1][1] + 1:
            out[-1][1] = a
        else:
            out.append([a, a])
    return out

if __name__ == '__main__':
    a, b = load(sys.argv[1]), load(sys.argv[2])
    ka, kb = set(a), set(b)
    print(f"A words: {len(a)}  range 0x{min(ka):X}-0x{max(ka):X}")
    print(f"B words: {len(b)}  range 0x{min(kb):X}-0x{max(kb):X}")
    print(f"only in A: {len(ka-kb)}   only in B: {len(kb-ka)}")
    common = sorted(ka & kb)
    diff = [x for x in common if a[x] != b[x]]
    print(f"common words: {len(common)}  differing: {len(diff)} "
          f"({100.0*len(diff)/max(1,len(common)):.4f}%)")
    rs = ranges(diff)
    print(f"contiguous changed regions: {len(rs)}")
    print("\ntop 40 regions by size (word addresses, x2 = byte offset):")
    for lo, hi in sorted(rs, key=lambda r: r[1]-r[0], reverse=True)[:40]:
        print(f"  0x{lo:06X}-0x{hi:06X}  {hi-lo+1:7d} words  "
              f"(bytes 0x{lo*2:07X}-0x{(hi+1)*2-1:07X})")
    print("\nfirst 20 regions in address order:")
    for lo, hi in rs[:20]:
        print(f"  0x{lo:06X}-0x{hi:06X}  {hi-lo+1:6d} words")
