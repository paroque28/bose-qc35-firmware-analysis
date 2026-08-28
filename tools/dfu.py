#!/usr/bin/env python3
"""First-pass structural analysis of CSR-dfu2 containers."""
import sys, math, collections

def ent(b):
    if not b: return 0.0
    c = collections.Counter(b); n = len(b)
    return -sum((v/n)*math.log2(v/n) for v in c.values())

def report(path):
    d = open(path,'rb').read()
    print(f"\n=== {path.split('/')[-1]}  ({len(d)} bytes) ===")
    print(f"magic            : {d[:8]!r}")
    print(f"hdr[8:16]        : {d[8:16].hex()}")
    print(f"overall entropy  : {ent(d):.4f} bits/byte")
    # entropy profile in 64KB blocks
    prof = [ent(d[i:i+65536]) for i in range(0, len(d), 65536)]
    lo = min(prof); hi = max(prof)
    print(f"64KB-block entropy: min {lo:.3f}  max {hi:.3f}  blocks {len(prof)}")
    lowblocks = [i for i,e in enumerate(prof) if e < 7.0]
    print(f"blocks with entropy <7.0 (likely code/plaintext): {lowblocks[:20]}"
          f"{' ...' if len(lowblocks)>20 else ''}  total {len(lowblocks)}")
    return d

def blockdiff(a, b, bs=256):
    """Align by content hash of fixed blocks to see how much is shared."""
    ha = collections.Counter(a[i:i+bs] for i in range(0, len(a)-bs, bs))
    hb = collections.Counter(b[i:i+bs] for i in range(0, len(b)-bs, bs))
    shared = sum((ha & hb).values())
    print(f"\n-- {bs}-byte aligned block overlap --")
    print(f"A blocks {sum(ha.values())}  B blocks {sum(hb.values())}  shared {shared}"
          f"  ({100.0*shared/max(1,sum(ha.values())):.2f}% of A)")
    # common prefix / suffix
    n = min(len(a), len(b))
    p = 0
    while p < n and a[p] == b[p]: p += 1
    s = 0
    while s < n-p and a[len(a)-1-s] == b[len(b)-1-s]: s += 1
    print(f"common prefix {p} bytes (0x{p:X}) | common suffix {s} bytes (0x{s:X})")
    print(f"first differing byte at 0x{p:X}: A={a[p]:02X} B={b[p]:02X}")

if __name__ == '__main__':
    a = report(sys.argv[1]); b = report(sys.argv[2])
    blockdiff(a, b)
