# Technical handoff: did Bose firmware 4.5.2 degrade noise cancelling on the QC35 II?

**Audience.** An AI agent or engineer picking this up cold, with the intent of taking the
analysis further. This document is self-contained. It assumes no prior context from the
session that produced it.

**Status.** First pass complete. Five independent lines of binary evidence established. One
significant region remains unexamined, and that gap is stated precisely in section 6.

**Position of this report.** The evidence gathered so far supports Bose's public claim that
the noise cancelling configuration was not modified in 4.5.2. It does **not** yet fully
close the question, because the application code that consumes that configuration has not
been disassembled. Do not read section 4 as a verdict without also reading section 6.

---

## 1. The claim under investigation

In June 2019 Bose shipped firmware **4.5.2** for the QuietComfort 35 II and **3.0.3** for
the first generation QuietComfort 35. A large number of owners reported that active noise
cancelling became audibly weaker afterwards. The most specific and most frequently repeated
form of the complaint was that the **low and high noise cancelling levels stopped sounding
different from each other**. That specificity matters, and section 7 explains why it is the
most useful thing in the entire complaint record.

Bose investigated and published a report on 2 April 2020 concluding that firmware 4.5.2 did
not affect noise cancelling, attributing the reports to worn ear cushions, aftermarket
parts, and mechanical damage. Bose nevertheless re-enabled downgrades to 4.3.6 (QC35 II) and
2.5.5 (QC35) for a limited period.

A widely held user theory was that the degradation was deliberate, timed to push the
then-new Noise Cancelling Headphones 700.

The question this repository exists to answer: **can the claim be settled from the shipped
binaries rather than from listening tests?**

Background reporting, quotes, the timeline, and the community's two methodological
objections to Bose's report are in `RESEARCH.md`. Read that too. Objection number one in
particular has a direct bearing on what to test.

---

## 2. Hardware and firmware model

The QC35 II is built on a CSR, later Qualcomm, **BlueCore** part. That family pairs an
**XAP** application processor with a **Kalimba** DSP for audio. Confirmed in-image strings
include `CSR CVC CONTROL` (Clear Voice Capture, the microphone-side noise reduction) and
`CSRPM parameters`.

Noise cancelling on this product is referred to internally as **CNC**, Controllable Noise
Cancellation. The corresponding tuning data is called **acorn**. Both names appear in
shipped strings and filenames and are the highest-value search terms in the image.

Each firmware release consists of exactly three files.

| File | Size (QC35 II) | Contents | Verdict from this analysis |
|---|---|---|---|
| `*_acorn_coeffs_signed.xuv` | 32944 bytes (39568 in 4.8.1) | Noise cancelling DSP coefficients and configuration | **Never changed between 2.0.1 and 4.5.2** |
| `*_ext_signed.xuv` | 22618624 bytes | External flash. Almost entirely RIFF/WAVE voice prompts in 13 languages | **Payload never changes at all** |
| `*_stack_plus_app.dfu` | 1939792 (4.3.6) / 1942460 (4.5.2) | Bluetooth stack plus application code | **The only file with real changes** |

Device codenames, needed to navigate Bose's own servers and the community archives:

- **BayWolf** = QuietComfort 35 II, USB device ID `0x4020`
- **Wolfcastle** = QuietComfort 35 first generation, USB device ID `0x400C`

---

## 3. File formats

Get these right before writing any analysis code. Two of the three are easy to misread.

### 3.1 `.xuv`

Plain ASCII text. One 16-bit word per line, formatted `@ADDRESS   HHHH` with **CRLF** line
endings. Addresses are **word** addresses, so the byte offset is twice the address.

```
@000000   7366
@000001   5F72
@000002   6664
@000003   3175
```

The first four words are the magic. Read as big-endian byte pairs they spell **`fsr_dfu1`**.
Words `0x04` through `0x45`, that is 66 words or 132 bytes, are a signature block. The
payload begins at word `0x46`.

`tools/xuv.py` parses this. Do not write your own parser without handling the CRLF and the
word-versus-byte addressing.

### 3.2 `.dfu`

Binary container, magic `CSR-dfu2` at offset 0. Header layout confirmed identical across
all three examined builds:

| Offset | Size | Value | Meaning |
|---|---|---|---|
| `0x00` | 8 | `CSR-dfu2` | magic |
| `0x08` | 2 | `0x0003` LE | format or type version |
| `0x0A` | 4 | varies LE | payload length, **always equals filesize minus 16** |
| `0x0E` | 2 | `0x04C8` LE | constant across builds, meaning unknown |
| `0x10` | 64 | ASCII spaces | padding |
| `0x50` | 32 | mostly zero, `0x0010` at `+0x18` | unknown |
| `0x70` | 8 | `FFFFFFFFFFFFFFFF` | unknown |
| `0x78` | 4 | `0x000CE82E` LE | constant across builds |

The tail of every image ends with the byte-swapped markers `File` and `DFU`.

**The payload is neither encrypted nor compressed.** Measured Shannon entropy is 6.347 bits
per byte overall, with 29 of 30 64KB blocks below 7.0. This is the single most important
practical fact in this document. Nothing has to be defeated. The work is disassembly, not
decryption.

### 3.3 The byte-swap, which will otherwise waste your time

**Text inside both formats is stored byte-swapped within each 16-bit word.** Running
`strings` directly on a `.dfu` produces scrambled output such as `PeGtsru eadat`, which is
easy to mistake for obfuscation. It is not. It is word-endianness.

Swap every adjacent byte pair across the whole file first, then run `strings`. The same
string becomes `Gesture data`.

```python
d = bytearray(open(path, 'rb').read())
n = len(d) & ~1
sw = bytes(b''.join(bytes([d[i+1], d[i]]) for i in range(0, n, 2)))
```

Every offset quoted in this document for `.dfu` content is an offset **into the swapped
buffer**. Since the transform is a pure pairwise swap, offsets map to the original file
under `orig = off ^ 1` per byte, and any aligned 16-bit word keeps its address.

---

## 4. The evidence

Five independent findings. Listed strongest first. Each is mechanically reproducible from
this repository using the commands in section 9.

### Finding 1. The noise cancelling coefficient blob was never modified

`acorn_coeffs_signed.xuv` is byte-identical across **every QC35 II release from 2.0.1
through 4.5.2**, all ten sharing SHA-256
`742d676712cd1c254acd5887affbacedfb2ffab59257e8c4d4d8a0b77dbd7da7`.

It changes for the first time in 4.8.1, where it also grows from 32944 to 39568 bytes,
consistent with the Self-Voice feature that release introduced.

The first generation QC35 shows the same pattern independently. Its blob is identical from
1.3.2 through 3.0.3, and 3.0.3 is the release that was blamed on that product.

Full table in `findings/anc_coefficient_timeline.txt`.

### Finding 2. The coefficient blob carries a frozen build timestamp

The blob embeds an ASCII build stamp. Extracted across every release:

| Product | Releases | Embedded build stamp |
|---|---|---|
| QC35 II | 2.0.1 through **4.5.2** | `md9809 - May 24, 2017 10:35:50.563 AM` |
| QC35 II | 4.8.1 | `cb14476 - May 19, 2020  2:44:52.690 PM` |
| QC35 gen 1 | 1.3.2 through **3.0.3** | `md9809 - May 18, 2017  2:56:36.698 PM` |

The noise cancelling tuning shipped in the two blamed releases was **compiled in May 2017**,
before the QC35 II went on sale, and was never recompiled until 4.8.1 in 2020. This is
strictly stronger than Finding 1, because a hash collision argument is unavailable against
a human-readable timestamp that simply never advances.

### Finding 3. The 22.6 MB image differs by 71 words, and they are a version string

Comparing `ext_signed.xuv` for 4.3.6 against 4.5.2, word by word, over 1,413,664 words:
**exactly 71 words differ**, in three contiguous regions.

| Word range | Words | Byte range | What it is |
|---|---|---|---|
| `0x000004`-`0x000045` | 66 | `0x0000008`-`0x000008B` | signature block |
| `0x1363F3`-`0x1363F6` | 4 | `0x026C7E6`-`0x026C7ED` | ASCII version string |
| `0x15921A` | 1 | `0x02B2434`-`0x02B2435` | trailing checksum |

The version string region decodes to `4.3.6-105`, `4.5.2-144`, and `4.8.1-321`, which match
the `REVISION` attributes in Bose's own `index.xml` exactly. It sits immediately before a
`RIFF....WAVEfmt ` header, confirming this file is the voice prompt partition.

The identical 71-word, three-region pattern holds for 4.5.2 against 4.8.1, which serves as a
control. The payload of this file is constant across all three releases.

### Finding 4. Every changed string in the application is Bose AR, none is noise cancelling

After byte-swapping, the complete set of strings that differ between 4.3.6 and 4.5.2
concerns the sensor hub and gesture subsystem. Representative additions in 4.5.2:

```
Sensorhub initialized
Sensor configuration / Sensor data / Sensor information
Gesture configuration / Gesture information / Gesture data
ERROR: handleTapSensorHubDiagMsg received a NULL Message
Error status 0x%x returned from SensorHub Diagnostic Service
handleTapSensorHubDiagMsg received unexpected MessageId %d
tapCachePrint overwrote buffer
```

Representative removals:

```
Accelerometer / Gyroscope / Magnetometer / UncalMagnetometer
RotationVector / GameRotationVector / Orientation
----> GESTURE: SingleTap / DoubleTap / HeadNod / HeadShake
Cal Status: CAL FAILED / UNKNOWN STATUS
```

This is the Bose AR head-tracking subsystem, which the 4.3.6 release notes describe as
groundwork Bose was laying at the time. It is a plausible and coherent explanation for what
4.5.2 actually was.

Meanwhile the noise cancelling strings are present and **identical** in both builds:

```
Current CNC Index: %d
Current CNC Index: N/A
Number of CNC Steps(s):  %d
dacgain: %u
BDSP volume: %u / BDSP data: 0x%06x / BDSP version: %u / BDSP batt logic: %u
DSP is OK / DSP logging / DSP read/write
```

Full diff in `findings/strings_diff_4.3.6_vs_4.5.2.txt`.

### Finding 5. The coefficient payload embedded inside the application is also identical

The application image carries its own copy of the coefficient payload, not merely a
reference to the separate file. Located by the `0x81 0x00 0x54 0x07 0x00 0x01 0x01 0x08`
header immediately preceding the build stamp.

| Version | Offset in swapped `.dfu` | Length | SHA-256 (first 32) | Matches standalone file |
|---|---|---|---|---|
| 4.3.6 | `0x01D24E8` | 3978 | `445f4642e855e97e5451d7c6f236ea67` | yes |
| 4.5.2 | `0x01D2F44` | 3978 | `445f4642e855e97e5451d7c6f236ea67` | yes |
| 4.8.1 | `0x01D7308` | 4806 | `fe3c2a417ef9d69e994486991ded8943` | yes |

The embedded copies for 4.3.6 and 4.5.2 are byte-identical to each other and to the
standalone blob. This closes a loophole worth closing: the tuning was not quietly changed
inside the application while the separately distributed file was left untouched as a decoy.

---

## 5. What did change in the application, and why the number is misleading

The application grew by 2,668 bytes between 4.3.6 and 4.5.2. A shift-tolerant
content-defined-chunking diff (`tools/cdc.py`, Rabin-style rolling hash, 2KB average chunk)
reports about **36.6 percent of bytes as changed**, spread over roughly a hundred regions.
Full map in `findings/diff_dfu_4.3.6_vs_4.5.2.txt`.

**Do not treat that percentage as evidence of anything.** The image grew, so all subsequent
code shifted, and every absolute address moved with it. A plain recompile with one feature
change produces exactly this signature. The string-table relocation is directly measurable
and confirms it: the `Current CNC Index: %d` anchor moves from `0x01D249C` to `0x01D03CC`,
a shift of `-0x20D0`, while its content is unchanged.

The correct way to interpret the 36.6 percent figure is: **the byte diff is uninformative at
this granularity, and a function-level comparison is required.** That is step 4 in section 8.

---

## 6. What has NOT been established

State this plainly in any summary you write. Overclaiming here would be the main failure
mode for this investigation.

1. **The application code that consumes the coefficients has not been disassembled.** The
   tuning data is provably unchanged. The code that selects, scales, indexes, or applies it
   has not been examined at all. A change to the mapping from CNC index to DSP gain, or to a
   step table living in the application rather than the coefficient blob, would be invisible
   to all five findings above.
2. **No Kalimba DSP program image has been identified.** The coefficient blob is
   configuration data. If the DSP's own executable code ships inside the application
   container, it has not been located and therefore has not been compared.
3. **No behavioural or acoustic verification was performed.** Nothing was flashed to a
   device. No measurement was taken. This is a static analysis only.
4. **4.5.2's authenticity is corroborated but not cryptographically proven.** See section 10.
5. **A non-firmware cause has not been ruled out and is not in scope.** Bose's own
   explanation, plus the separately documented QC35 power switch defect, remain live
   alternative hypotheses that a binary analysis cannot address either way.

---

## 7. Falsifiable hypotheses

Framed so that each can be confirmed or killed by inspecting the binaries. H1 is the one
that matters.

**H1. The CNC index to gain mapping changed in the application.**
The most-repeated complaint was that the low and high levels stopped differing. If true, a
table or arithmetic mapping the CNC index to a DSP gain was compressed, clamped, or
flattened. Test: locate the code referencing `Current CNC Index: %d` and
`Number of CNC Steps(s):  %d` in both builds, recover the backing table, compare values.
**This is the highest-value single experiment in the whole project.** If the tables are
identical, H1 dies and the firmware theory is close to dead with it.

**H2. The number of CNC steps changed.**
A narrower and even easier form of H1. `Number of CNC Steps(s):  %d` implies a step count
constant or variable. Recover it from both builds and compare.

**H3. Bose AR sensor code introduced a resource regression.**
4.5.2 demonstrably added sensor hub work. If that code shares a processor, an interrupt
priority, or a DSP message queue with the noise cancelling path, it could degrade ANC
without any ANC code being edited. This would make both parties right at once: Bose changed
nothing in the noise cancelling feature, and noise cancelling still got worse. Test: look
for shared scheduling, message queues, or priority changes between the sensor hub code and
the CNC path.

**H4. A DSP program image, distinct from the coefficients, changed.**
Test: identify Kalimba code regions in the container and compare them across builds.

**H5. Nothing relevant changed and the cause is not the firmware.**
The current default hypothesis. Every result so far is consistent with it. It cannot be
promoted to a conclusion until H1 through H4 are tested.

---

## 8. Recommended next steps, in priority order

### Step 1. Disassemble the application image

Determine whether the payload is XAP code, Kalimba code, or both, and recover load
addresses. Notes on tooling, gathered but not verified:

- An open-source XAP assembler exists, originating from darkircop.org and revived since.
- Some CSR toolchain source has been published on GitHub, including GPL code CSR had
  omitted to release.
- An IDA plugin for XAP2 has been discussed on mailing lists over many years. Availability
  and quality are unconfirmed.
- No mature public Kalimba disassembler was found. This may need to be written.

If no ready disassembler exists, the fallback in Step 3 does not require one.

### Step 2. Anchor on the surviving format strings

The noise cancelling strings did not change, which makes them ideal fixed points. Their
offsets in the swapped buffer, already extracted:

| String | 4.3.6 | 4.5.2 | 4.8.1 |
|---|---|---|---|
| `Current CNC Index: %d` | `0x01D249C` | `0x01D03CC` | `0x01D4790` |
| `Current CNC Index: N/A` | `0x01D24B2` | `0x01D03E2` | `0x01D47A6` |
| `Number of CNC Steps(s):  %d` | `0x01D24CA` | `0x01D03FA` | `0x01D47BE` |
| `dacgain: %u` | `0x01D1FA2` | `0x01D2EC2` | `0x01D7286` |
| `BDSP volume: %u` | `0x01D1F82` | `0x01D2EA2` | `0x01D7266` |
| `BDSP data: 0x%06x` | `0x01D1F70` | `0x01D2E90` | `0x01D7254` |
| `BDSP version: %u` | `0x01D1EEC` | `0x01D2E0C` | `0x01D71D0` |
| `BDSP batt logic: %u` | `0x01D1EFE` | `0x01D2E1E` | `0x01D71E2` |
| `DSP is OK` | `0x01D0CF2` | `0x01D0420` | `0x01D47E4` |
| `DSP read/write` | `0x01D2378` | `0x01D02A4` | `0x01D4668` |
| `DSP logging` | `0x01D230E` | `0x01D023A` | `0x01D45FE` |

Find the code that references each. Compare the referencing functions between builds. The
whole question likely lives within a few hundred instructions of these anchors.

Note that the string block itself was reordered between builds, not merely shifted. The CNC
group moves by `-0x20D0` while the BDSP group moves by `+0xF20`. Do not assume a single
global delta.

### Step 3. Recover and compare the CNC step table

This is the direct test of H1 and H2 and it does **not** require a working disassembler.
A CNC level table is likely a short run of small integers, plausibly 2 to 10 entries,
somewhere near the CNC code or the coefficient region. Approach: extract every short
monotonic integer run from both images, compare the sets, and inspect any that differ. A
table that is identical in both is strong evidence against H1. A table that changed is the
answer to the entire question.

### Step 4. Establish function-level equivalence properly

Replace the meaningless 36.6 percent byte figure with something interpretable. Recover
function boundaries, normalise by masking absolute addresses and relocation targets, hash
each normalised body, and compare the sets across builds. The expected result if Bose is
telling the truth: the sensor hub functions differ, everything on the audio path matches.
Any audio-path function that differs is a finding, and should be examined instruction by
instruction.

### Step 5. Locate any Kalimba DSP image

Scan the container for a second code region with different instruction statistics from the
XAP code. Compare across builds. Tests H4.

### Step 6. Test the community's objection to Bose's methodology

Bose compared 4.1.3 against 4.5.2. Nearly every complaint came from owners upgrading from
2.x or 3.x. All of those versions are present in `firmware/baywolf/`. Run steps 3 and 4
across the full chain 2.5.1, 3.1.7, 3.1.8, 4.1.3, 4.3.6, 4.5.2 rather than only the
adjacent pair. If something changed at 4.1.3, Bose's own comparison would have been blind to
it by construction, and that possibility is currently untested.

### Step 7. Optional, behavioural verification

The QC35 exposes noise cancelling control over RFCOMM channel 8, using a three-byte opcode,
a one-byte length, and a payload. Documented on the Linux Bluetooth mailing list, cited in
`RESEARCH.md`. With a physical device this permits reading back the CNC index and step count
at runtime under each firmware, which would confirm or refute Step 3 empirically.

---

## 9. Reproducing every result

All scripts are Python 3 standard library only. No dependencies. Run from the repository
root.

```bash
# Finding 1: coefficient blob identical 2.0.1 through 4.5.2
shasum -a 256 firmware/baywolf/*_acorn_coeffs_signed.xuv

# Finding 3: the 71-word result, and the control
python3 tools/xuv.py firmware/baywolf/BayWolf_4.3.6_ext_signed.xuv \
                     firmware/baywolf/BayWolf_4.5.2_ext_signed.xuv
python3 tools/xuv.py firmware/baywolf/BayWolf_4.5.2_ext_signed.xuv \
                     firmware/baywolf/BayWolf_4.8.1_ext_signed.xuv

# Finding 3: decode the version string region
python3 tools/show.py firmware/baywolf/BayWolf_4.3.6_ext_signed.xuv \
                      firmware/baywolf/BayWolf_4.5.2_ext_signed.xuv 1363F2 1363F6

# Section 3.2 and section 5: container header, entropy, block overlap
python3 tools/dfu.py firmware/baywolf/BayWolf_4.3.6_stack_plus_app.dfu \
                     firmware/baywolf/BayWolf_4.5.2_stack_plus_app.dfu

# Section 5: shift-tolerant region map
python3 tools/cdc.py firmware/baywolf/BayWolf_4.3.6_stack_plus_app.dfu \
                     firmware/baywolf/BayWolf_4.5.2_stack_plus_app.dfu
```

Regenerating the string diff of Finding 4 requires the byte-swap from section 3.3, then
`strings -n 6`, `sort -u`, and `comm`. Pre-generated output is in `findings/`.

### Tools reference

| Script | Purpose |
|---|---|
| `tools/xuv.py A B` | Parse two `.xuv` dumps, report differing words collapsed into contiguous regions |
| `tools/show.py A B lo hi` | Print a word range from both files side by side with ASCII |
| `tools/tostr.py F lo hi` | Extract a word range as a byte string |
| `tools/dfu.py A B` | `CSR-dfu2` header fields, entropy profile, aligned block overlap |
| `tools/cdc.py A B` | Shift-tolerant diff via content-defined chunking |

---

## 10. Provenance and how much to trust these files

**Bose has deleted 4.5.2.** Every 4.5.2 URL under `downloads.bose.com/ced/baywolf/` returns
HTTP 403. The Wayback Machine holds no snapshot of any of the three files. Bose's live index
currently offers only 4.3.6 and 4.8.1, and a `PRE_RRA` tree offering 3.1.8. Live copies of
all three index files are preserved in `research/`.

The complete historical set here comes from the community archive
[`bosefirmware/ced`](https://github.com/bosefirmware/ced).

**Integrity check performed.** 4.3.6 and 4.8.1 were downloaded fresh from
`downloads.bose.com` and compared against the archive copies. All six files matched
byte-for-byte.

```
4.3.6  stack_plus_app.dfu         MATCH
4.3.6  ext_signed.xuv             MATCH
4.3.6  acorn_coeffs_signed.xuv    MATCH
4.8.1  stack_plus_app.dfu         MATCH
4.8.1  ext_signed.xuv             MATCH
4.8.1  acorn_coeffs_signed.xuv    MATCH
```

That is good evidence the archive is faithful in general. It is **not** proof for 4.5.2
specifically, because no Bose-hosted copy survives to compare against.

Independent corroboration for 4.5.2, weaker but real:

- The repository [`sunzj/Way_of_Downgrade_BOSE_QC35ii`](https://github.com/sunzj/Way_of_Downgrade_BOSE_QC35ii)
  documents the 4.5.2 file sizes observed in the updater's own temporary directory in 2019:
  33 KB, 22089 KB, and 1897 KB. These match the archived files.
- The version string recovered from inside the archived `ext` image is `4.5.2-144`,
  structurally consistent with `4.3.6-105` and `4.8.1-321`, which match the `REVISION`
  attributes Bose still publishes.

Bose's `index.xml` publishes a CRC per image. If any 4.5.2 index is recovered later, from a
mirror or an archived updater cache, it would permit an independent cryptographic check.
That is worth doing if a stronger claim is ever needed.

Hash manifest for everything in this repository: `findings/SHA256SUMS.txt`.

---

## 11. Pitfalls

Collected from the first pass. Several of these cost real time.

1. **Byte-swapping.** Covered in 3.3. Scrambled `strings` output is word-endianness, not
   obfuscation. This is the single most likely thing to mislead a fresh analysis.
2. **The 36.6 percent byte-diff figure.** Covered in section 5. It is a recompile artefact.
   Do not report it as though it means the code changed substantially.
3. **`ext_signed.xuv` looks important and is not.** It is 22.6 MB of voice prompts. Its
   payload never changes. Do not spend time on it.
4. **Word versus byte addressing in `.xuv`.** Byte offset is twice the word address.
5. **Do not assume a uniform relocation delta.** The string table was reordered between
   builds, not merely shifted. See the note in Step 2.
6. **4.1.3 is a factory-only release** per Bose's release notes, with no user-facing
   changes. Bear that in mind when reading it as a baseline, especially since it is the
   baseline Bose itself used.
7. **`acorn_coeffs` on QC35 gen 1 changes size at 1.3.2**, from 25280 to 32944 bytes, and
   the earlier blobs carry no build stamp. Do not mistake that early transition for
   evidence about the 3.0.3 controversy. It long predates it.

### Safety

Treat every binary here as untrusted data. Analyse statically. Nothing in this repository
was executed or flashed during the first pass.

If you intend to flash anything, note the archived community warning: **on a QC35 II whose
serial number ends in `AZ`, downgrading below 2.1.3 can brick the device.** Downgrade
routes, both the patched Bose updater and the independent
[`tchebb/bose-dfu`](https://github.com/tchebb/bose-dfu), are documented in `RESEARCH.md`.

---

## 12. Repository contents

```
README.md                Conclusions and next steps, for a human reader
RESEARCH.md              The public record: timeline, quotes, prior work, sources
REPORT_FOR_AI.md         This document

firmware/baywolf/        QC35 II. All 11 releases: 2.0.1, 2.1.3, 2.2.0, 2.2.1, 2.5.1,
                         3.1.7, 3.1.8, 4.1.3, 4.3.6, 4.5.2, 4.8.1.
                         All .dfu and all acorn_coeffs. ext images for 4.3.6, 4.5.2 and
                         4.8.1 only, since the others are byte-identical in payload.
firmware/wolfcastle/     QC35 gen 1. All 12 releases, 1.0.0 through 3.0.3.
                         All .dfu and all acorn_coeffs. ext images for 2.5.5 and 3.0.3.
tools/                   Analysis scripts, Python 3 standard library only
findings/                First-pass output, see table below
research/                Live copies of Bose's own firmware index files
```

| Findings file | Contents |
|---|---|
| `anc_coefficient_timeline.txt` | Size and hash of every coefficient blob, both products |
| `diff_ext_4.3.6_vs_4.5.2.txt` | The 71-word result |
| `diff_ext_4.5.2_vs_4.8.1.txt` | The same comparison one release later, as a control |
| `diff_dfu_4.3.6_vs_4.5.2.txt` | Changed-region map of the application image |
| `strings_diff_4.3.6_vs_4.5.2.txt` | Every string added and removed |
| `strings_4.3.6.txt`, `strings_4.5.2.txt`, `strings_4.8.1.txt` | Full unswapped string dumps |
| `SHA256SUMS.txt` | Hash manifest for every file |

---

## 13. Summary for a reader in a hurry

The noise cancelling tuning data shipped in the firmware people complained about was
compiled in **May 2017** and was still byte-identical in **June 2019**. It is unchanged in
the standalone file and unchanged in the copy embedded inside the application. The 22.6 MB
companion image differs across the two releases by 71 words out of 1.4 million, and those
words are a signature, a checksum, and the ASCII version number. Every string that changed
in the application belongs to the Bose AR head-tracking subsystem. Not one noise cancelling
string changed.

On the evidence available, Bose's claim that 4.5.2 made no change to the noise cancelling
feature is **well supported**.

The remaining gap is real and should not be glossed over. The data is provably unchanged,
but the code that reads it has not been disassembled. Hypothesis H1, that the mapping from
CNC index to gain was altered in the application, is untested, and it is precisely the
hypothesis that would explain the most common form of the complaint. Section 8 Step 3
describes how to test it without needing a disassembler at all. Do that first.
