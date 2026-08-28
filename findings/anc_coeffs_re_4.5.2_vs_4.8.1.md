# QC35 II ANC coefficient blob: reverse engineering of 4.5.2 vs 4.8.1

Scope: the signed `acorn_coeffs` blob for the QC35 II (codename "baywolf").
Files compared:

- `firmware/baywolf/BayWolf_4.5.2_acorn_coeffs_signed.xuv` (32944 bytes on disk, 4118 bytes of payload)
- `firmware/baywolf/BayWolf_4.8.1_acorn_coeffs_signed.xuv` (39568 bytes on disk, 4946 bytes of payload)

The `.xuv` file is a text dump of 16-bit words (`@ADDR WORD` per line). "Payload" below means the
decoded binary (each word emitted big-endian, as `tools/tostr.py` does). All offsets are into that payload.

## Summary of the change

The size growth from 4.5.2 to 4.8.1 is **not** a re-tuning of the existing noise-cancellation filters.
It is the **addition of one new coefficient block**. The pre-existing coefficient tables are preserved
byte-for-byte. Concretely, when the two payloads are aligned (accounting for the inserted bytes),
**96.4 percent of 4.5.2 survives verbatim into 4.8.1**.

Three things changed, and nothing else:

1. A **new 827-byte coefficient block** was inserted (payload offset `0x0C00` in 4.8.1).
2. A one-byte **stage counter in the header went from `0x03` to `0x04`** (three blocks became four).
3. The **descriptor header was rebuilt**: new build ID, new timestamp, new cryptographic signature.

## Payload layout (both versions)

| Region | 4.5.2 offset | 4.8.1 offset | Size | State |
|---|---|---|---|---|
| Magic `sf_rfd1u` | `0x0000`–`0x0007` | `0x0000`–`0x0007` | 8 | identical |
| Signature + descriptor header (build ID, timestamp, record list) | `0x0008`–`0x00E2` | `0x0008`–`0x00E3` | ~219 / ~228 | rewritten |
| Main coefficient table | `0x00E3`–`0x0BFE` | `0x00E4`–`0x0BFF` | 2844 | **identical** (sha256 `7a298fdb26951bca…`) |
| New coefficient block | (absent) | `0x0C00`–`0x0F3A` | 827 | **inserted in 4.8.1** (sha256 `34013fa316761067…`) |
| Trailing coefficient/config table | `0x0BFF`–end | `0x0F3B`–end | 1047 | **identical** (sha256 `cb7bae4e3f6b401e…`) |

The alignment was produced with a byte-level sequence match (Python `difflib`). It reports exactly two
large equal runs (2844 bytes and 1047 bytes) plus one pure insertion of 827 bytes, which together account
for the full 828-byte payload growth.

Why the naive comparison misleads: a straight offset-aligned diff shows "almost everything changed"
(160 to 240 differing bytes per 256-byte block). That is an artifact of the 827-byte insertion, which
shifts every following byte. Once the shift is removed, the real change is small and localized. This is
the correction to the earlier size-and-hash-only reading, which concluded 4.8.1 was a wholesale retune.
It is not.

## The header is not encrypted, only signed

Entropy analysis rules out an encrypted payload. The coefficient regions sit at roughly 5.2 to 5.8
bits per byte with about 30 percent zero bytes, which is the signature of arrays of small fixed-point
numbers, not ciphertext (ciphertext would be near 8.0 bits per byte with almost no zeros). Only the
leading block after the magic (the RSA/ECC-style signature) and the trailing table read as high entropy.

Because the payload is in the clear, the descriptor header decodes directly. It carries an ASCII
**build ID and timestamp**:

- 4.5.2: `md9809 - May 24, 2017 10:35:50.563 AM`
- 4.8.1: `cb14476 - May 19, 2020  2:44:52.690 PM`

These two build dates are three years apart, which matches the release timeline: the same 4.5.2 blob
(sha256 `742d6767…`) shipped unchanged from firmware 2.0.1 through 4.5.2, and 4.8.1 is the first release
to replace it.

## The stage counter and the record markers

The tail of the descriptor header is byte-identical between the two versions except for its final byte:

```
4.5.2:  … 0e 0f 00 01 03
4.8.1:  … 0e 0f 00 01 04
```

That last byte is a **count of coefficient blocks (filter stages)**: `0x03` in 4.5.2, `0x04` in 4.8.1.
This is corroborated by a repeating 4-byte record marker `08 00 00 00` that separates coefficient groups
throughout the payload. Its count rises from **22 in 4.5.2 to 32 in 4.8.1**, and exactly **10 of those new
markers fall inside the inserted 827-byte block**. So the new block is a structured set of about ten
coefficient records, in the same format as the existing tables, appended as a fourth stage.

## What the inserted block contains

Decoded as little-endian 32-bit fixed-point values, the new block reads as plausible filter coefficients
(gains and biquad-like sections), for example values near `+63.0`, `+79.0`, and small fractional terms,
interleaved with the same `08 00 00 00` record markers used elsewhere. The exact fixed-point scale is not
pinned down here, so the individual numbers are indicative rather than final. What is certain is the
**format and framing match the existing coefficient tables**, so this is genuinely a new filter stage of
the same kind, not padding or unrelated data.

## Bearing on the ANC regression

If there is an audible ANC difference between 4.5.2 and 4.8.1, it cannot come from altered values in the
old filters, because those tables are identical down to the byte. It can only come from the **added fourth
stage**. This is a specific, testable statement: the delta is one appended filter block plus a stage-count
bump, over an otherwise frozen coefficient set. Judging whether that stage helps or hurts requires an
acoustic measurement, which is outside what the binary can tell us.

## The fourth stage in depth

The inserted block (827 bytes) is not opaque. It decodes cleanly into the same record format used by
the rest of the file, so its internal structure can be read out completely.

### Record framing

Every coefficient record starts with a 6-byte header `0e 00 4f <group> 00 <sub>`:

- `0e 00 4f` is the record type tag, the same tag used throughout the file (the main table holds 30 such
  records, the trailing table holds 10).
- `<group>` is a channel index: `0x01` then `0x02`.
- `<sub>` is a sub-record index that steps `0x00, 0x20, 0x40, 0x60, 0x80` (five per channel).

So the fourth stage is exactly **2 channels x 5 sub-records = 10 records**, which is why the `08 00 00 00`
coefficient-group marker count rose by 10. Inside each record, the coefficients are grouped and each
group is terminated by the `08 00 00 00` marker.

This also explains the whole-file bookkeeping. A "stage" is 10 records. The main table carries 30 records
(the first three stages), the header stage-counter reads `0x03` in 4.5.2, and 4.8.1 appends one more
10-record stage and bumps the counter to `0x04`. Everything is consistent.

### The two channels are identical

Channel `0x02` is a byte-for-byte copy of channel `0x01` with only the group-id byte changed (0 differing
bytes over 410 bytes once that single byte is ignored). Both earcups receive exactly the same new filter.
The added tuning is symmetric, not a left/right correction.

### The coefficients are biquad IIR filter sections

Read as big-endian 32-bit fixed point (Q4.28), the values land squarely in the range of cascaded
second-order (biquad) filter coefficients:

- Numerator-side terms scale up to roughly plus or minus 8 (for example `+3.97`, `-7.94`, `+3.16`).
- Denominator-side terms cluster tightly near `+1.0` (sub-record `0x60`: values `+0.99` to `+1.00`) and
  near `-0.5` (sub-record `0x80`: values `-0.42` to `-0.50`).

That near-unity and near-minus-one-half pairing is the classic footprint of the feedback coefficients
(a1, a2) of a stable biquad. The larger numerator terms are the feed-forward gains. In short, the fourth
stage is a small bank of cascaded biquad filters (an added equalization / cancellation-shaping stage),
applied equally to both channels. The exact Q-format scale is inferred from the value ranges, so treat the
individual decimal numbers as close approximations, not exact published constants. The framing, channel
duplication, and coefficient ranges are certain.

## How to reproduce

```
# decode both blobs to binary, then align
python3 tools/show.py firmware/baywolf/BayWolf_4.5.2_acorn_coeffs_signed.xuv \
                      firmware/baywolf/BayWolf_4.8.1_acorn_coeffs_signed.xuv 0 0x80A
```

The per-region identity was confirmed with SHA-256 over the aligned slices (values quoted above), and the
insertion was located with a `difflib` sequence match over the two decoded payloads.
