#!/usr/bin/env python3
"""Shift-tolerant diff: content-defined chunking with a rolling hash."""
import sys, hashlib

MASK = (1 << 11) - 1   # ~2KB average chunk
def chunks(d):
    out, start, h = [], 0, 0
    for i, b in enumerate(d):
        h = ((h << 1) + b) & 0xFFFFFFFF
        if (h & MASK) == MASK and i - start >= 256:
            out.append((start, i + 1)); start = i + 1
    if start < len(d): out.append((start, len(d)))
    return out

def sig(d, cs): return [(hashlib.sha256(d[a:b]).digest(), a, b) for a, b in cs]

a = open(sys.argv[1], 'rb').read(); b = open(sys.argv[2], 'rb').read()
sa, sb = sig(a, chunks(a)), sig(b, chunks(b))
ha = {}
for h, s, e in sa: ha.setdefault(h, []).append((s, e))
hb = set(h for h, _, _ in sb)
matched_a = sum(e - s for h, s, e in sa if h in hb)
matched_b = sum(e - s for h, s, e in sb if h in ha)
print(f"A: {len(a)} bytes in {len(sa)} chunks | B: {len(b)} bytes in {len(sb)} chunks")
print(f"A bytes present in B: {matched_a} ({100.0*matched_a/len(a):.2f}%)")
print(f"B bytes present in A: {matched_b} ({100.0*matched_b/len(b):.2f}%)")
print(f"\nB chunks NOT found in A (genuinely new/changed content):")
tot = 0
for h, s, e in sb:
    if h not in ha:
        tot += e - s
        if True:
            print(f"  offset 0x{s:07X}-0x{e:07X}  {e-s:6d} bytes")
print(f"  --> total changed/new in B: {tot} bytes ({100.0*tot/len(b):.2f}%)")
