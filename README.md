# Bose QC35 / QC35 II: did firmware 4.5.2 damage noise cancelling?

This directory holds the firmware images, the tools, and the first-pass analysis for the
2019 claim that a Bose firmware update degraded active noise cancelling on the
QuietComfort 35 and QuietComfort 35 II.

Prepared for a deeper follow-up analysis. Everything needed to reproduce or extend the
work is here. Nothing was flashed to any device, and no code from these images was executed.

## The claim, in short

In June 2019 Bose shipped firmware 4.5.2 for the QC35 II and 3.0.3 for the QC35 (first
generation). Large numbers of owners reported that noise cancelling became noticeably
weaker afterwards, in particular that the difference between the low and high noise
cancelling levels had collapsed. Bose investigated, published a report in April 2020
concluding that the firmware did not affect noise cancelling, and blamed worn ear
cushions, aftermarket parts, and mechanical damage. Bose nevertheless re-opened the
ability to downgrade, to 4.3.6 on the QC35 II and 2.5.5 on the QC35, for a limited period.

The open question this directory addresses: can the claim be checked directly against the
shipped binaries, rather than by listening tests?

## What the first pass found

The answer so far leans strongly toward Bose being correct on the narrow technical point,
with one important region still unexamined.

### 1. The ANC coefficient blob was never touched

Each release ships three files. One of them, `*_acorn_coeffs_signed.xuv`, is the
coefficient blob for the noise cancelling DSP. On the QC35 II it is **byte-identical
across every release from 2.0.1 (2017) through 4.5.2 (June 2019)**, all with SHA-256
`742d676712cd1c254acd5887affbacedfb2ffab59257e8c4d4d8a0b77dbd7da7`. It changes for the
first time in 4.8.1 (October 2020), where it also grows from 32944 to 39568 bytes, which
is consistent with the "Self-Voice" feature that release added.

The first generation QC35 shows the same pattern. The blob is identical from 1.3.2 all the
way through 3.0.3, the release that was also blamed.

Stronger still, the blob embeds a plaintext build timestamp. On the QC35 II it reads
`md9809 - May 24, 2017 10:35:50.563 AM` on every release from 2.0.1 through 4.5.2, and only
advances to `cb14476 - May 19, 2020 2:44:52 PM` in 4.8.1. The noise cancelling tuning in the
blamed release was **compiled in May 2017**, before the product shipped, and was never
rebuilt until 2020. A frozen human-readable date is harder to argue against than a hash.

The application image (`.dfu`) also carries its own embedded copy of this coefficient
payload, and that copy is byte-identical between 4.3.6 and 4.5.2 as well. So the tuning was
not quietly changed inside the app while the standalone file was left as a decoy.

Full table: `findings/anc_coefficient_timeline.txt`.

This means the tuning parameters of the noise cancelling filter were not modified by the
releases people complained about. That is a hard, verifiable fact, not an opinion.

### 2. The large image is voice prompts, and its content never changes

The 22.6 MB `*_ext_signed.xuv` looked like the obvious place for DSP code. It is not. It is
the external flash partition, and it is almost entirely RIFF/WAVE voice prompts in thirteen
languages.

Comparing 4.3.6 against 4.5.2 word by word: out of 1,413,664 sixteen-bit words, exactly
**71 differ**, in three regions.

| Region (word address) | Words | What it is |
|---|---|---|
| `0x000004`-`0x000045` | 66 | signature block, right after the `fsr_dfu1` magic |
| `0x1363F3`-`0x1363F6` | 4 | the ASCII version string |
| `0x15921A` | 1 | trailing checksum |

The version string region decodes to `4.3.6-105`, `4.5.2-144`, and `4.8.1-321` respectively,
matching the `REVISION` attributes in Bose's own `index.xml`. In other words the payload is
bit-for-bit identical and only the stamp and the signature move.

The same 71-word, three-region pattern holds for 4.5.2 against 4.8.1. See
`findings/diff_ext_4.3.6_vs_4.5.2.txt` and `findings/diff_ext_4.5.2_vs_4.8.1.txt`.

### 3. The only real change is in the application image, and it is Bose AR

`*_stack_plus_app.dfu` is a `CSR-dfu2` container holding the Bluetooth stack and the
application. It grew from 1,939,792 to 1,942,460 bytes between 4.3.6 and 4.5.2. Entropy is
6.35 bits per byte, so it is **neither encrypted nor compressed** and can be analysed
directly.

Strings are stored with each 16-bit word byte-swapped. After unswapping, the complete list
of strings that changed between 4.3.6 and 4.5.2 concerns the sensor hub: accelerometer,
gyroscope, magnetometer, rotation vector, game rotation vector, orientation, calibration
status, and the head gestures (single tap, double tap, head nod, head shake). Several new
sensor-hub diagnostic and error strings appear. This is the Bose AR subsystem, which the
4.3.6 release notes describe as groundwork Bose was laying at the time.

Not one noise-cancelling string changed. The ANC-related strings, including
`Current CNC Index: %d` and `Number of CNC Steps(s):  %d` (CNC being Bose's Controllable
Noise Cancellation), are present and identical in both releases.

Full list: `findings/strings_diff_4.3.6_vs_4.5.2.txt`.

### 4. The caveat that keeps the question open

A shift-tolerant content-defined-chunking diff of the two `.dfu` images reports about
36 percent of bytes as changed (`findings/diff_dfu_4.3.6_vs_4.5.2.txt`). That number on
its own means very little. The image grew by 2,668 bytes, so all later code shifted and
every absolute address moved with it. A plain recompile produces exactly this signature.

However, it does mean the byte-level diff cannot by itself rule out a behavioural change
in the application. Specifically, **the coefficients are provably unchanged, but the code
that selects, scales, or applies them has not yet been examined.** A change to how the CNC
index maps onto the DSP, or to a gain or step table in the application, would be invisible
to everything established above. That is the main open thread.

## Conclusions

1. **The noise cancelling tuning was not changed in the firmware people complained about.**
   The coefficient blob is byte-identical from 2.0.1 through 4.5.2, carries a build date
   frozen at May 2017, and its embedded copy inside the application matches too. On the
   first generation QC35 the same holds through 3.0.3. This is proven, not inferred.

2. **What 4.5.2 actually changed was Bose AR, not noise cancelling.** Every differing string
   in the application belongs to the head-tracking sensor hub. Not one noise cancelling
   string changed. The release added a feature. It did not retune the ANC.

3. **On the evidence available, Bose's public claim holds up.** Their statement that 4.5.2
   made no change to the noise cancelling feature is consistent with everything measurable
   in the binaries. The popular theory of a deliberate, timed downgrade is not supported by
   what actually shipped.

4. **The question is not fully closed, and honesty requires saying so.** The data is
   provably unchanged. The code that reads that data has not been disassembled. A change to
   the mapping from noise cancelling level to DSP gain, living in the application rather than
   the coefficient file, would not show up in anything done so far. That single hypothesis,
   which happens to match the most common form of the complaint (low and high stopped
   differing), is the one thing still worth testing.

5. **The most likely real-world explanation remains mundane and is outside a binary
   analysis.** Bose's own account (worn or misseated ear cushions, aftermarket parts) plus
   the separately documented QC35 power-switch defect can degrade perceived performance with
   no firmware involvement at all. A factory reset and genuine, fully seated cushions cost
   nothing and resolved the issue for a number of owners.

Net: this looks far more like a feature addition that coincided with unrelated hardware
wear and a wave of expectation-driven perception than like a deliberate sabotage. One
concrete test (Next steps 2 and 3 below) could move conclusion 3 from "well supported" to
"settled". Until someone runs it, do not state it as settled.

## Next steps

Ordered by expected value. Steps 2 and 3 are the ones that would actually close the
question, and step 3 needs no disassembler.

1. **Disassemble the application.** The QC35 II is built on a CSR/Qualcomm BlueCore part,
   which pairs an XAP application processor with a Kalimba DSP. Identify which is present
   in the `.dfu` payload, locate the load addresses from the container header, and
   disassemble. There is no encryption in the way.
2. **Find the CNC code path.** Use the surviving format strings (`Current CNC Index: %d`,
   `Number of CNC Steps(s):  %d`, `dacgain: %u`, `BDSP volume: %u`, `BDSP data: 0x%06x`)
   as anchors. Their exact offsets in all three builds are tabulated in `REPORT_FOR_AI.md`.
   Cross-reference them in both builds and compare the functions that reference them. This
   is the single most direct test of the user complaint, since the reported symptom was that
   the levels stopped differing from each other.
3. **Compare the CNC step tables (no disassembler required).** If a small integer table
   backs the noise cancelling levels, extract it from both builds and compare values. The
   complaint that low and high became indistinguishable would show up as compressed or
   equalised entries. This is the cheapest experiment with the highest payoff.
4. **Establish function-level equivalence properly.** Rather than a byte diff, recover
   function boundaries and compare normalised bodies with addresses masked out. That
   separates a pure recompile from a genuine logic change, which the current 36 percent
   figure cannot do.
5. **Check whether the ANC DSP firmware itself ships in the `.dfu`.** The coefficient blob
   is separate, but if the DSP's own program image is embedded in the application container
   it needs the same treatment as the coefficients.
6. **Test Bose's own methodology objection.** Bose compared 4.1.3 against 4.5.2, but most
   complaints came from users on 2.x or 3.x. All those versions are in `firmware/baywolf/`.
   Run steps 3 and 4 across the whole chain, not just the adjacent pair.

Full detail, with the anchor offset tables and falsifiable hypotheses, is in
`REPORT_FOR_AI.md`. A separate proposal for building an open replacement firmware, with what
it would take and whether it is even feasible, is in `OPEN_FIRMWARE.md`.

### Deeper-analysis notes

1. **Disassemble the application.** The QC35 II is built on a CSR/Qualcomm BlueCore part,
   which pairs an XAP application processor with a Kalimba DSP. Identify which is present
   in the `.dfu` payload, locate the load addresses from the container header, and
   disassemble. There is no encryption in the way.
2. **Find the CNC code path.** Use the surviving format strings (`Current CNC Index: %d`,
   `Number of CNC Steps(s):  %d`, `dacgain: %u`, `BDSP volume: %u`, `BDSP data: 0x%06x`)
   as anchors. Cross-reference their addresses in both builds and compare the functions
   that reference them. This is the single most direct test of the user complaint, since
   the reported symptom was that the levels stopped differing from each other.
3. **Compare the CNC step tables.** If a small integer table backs the noise cancelling
   levels, extract it from both builds and compare values. The complaint that low and high
   became indistinguishable would show up as compressed or equalised entries.
4. **Establish function-level equivalence properly.** Rather than a byte diff, recover
   function boundaries and compare normalised bodies with addresses masked out. That
   separates a pure recompile from a genuine logic change, which the current 36 percent
   figure cannot do.
5. **Check whether the ANC DSP firmware itself ships in the `.dfu`.** The coefficient blob
   is separate, but if the DSP's own program image is embedded in the application container
   it needs the same treatment as the coefficients.
6. **Parse the `CSR-dfu2` container properly.** Header layout confirmed so far: magic
   `CSR-dfu2` at offset 0, a 16-bit field `0x0003`, then a 32-bit little-endian length at
   offset 10 equal to filesize minus 16. Verified on both images.

### The one gap in the evidence

4.1.3 and 4.3.6 are the releases immediately before 4.5.2, and both are present here, so
the bracket is complete for the QC35 II. Note that Bose's own published investigation
compared 4.1.3 against 4.5.2. A recurring community objection was that almost all
complaints came from people upgrading from 2.x or 3.x, not from 4.1.3, so Bose may have
tested a path that few users actually took. Every one of those versions is in
`firmware/baywolf/`, so that objection can now be tested directly.

## Contents

```
firmware/baywolf/      QC35 II. All 11 releases: 2.0.1, 2.1.3, 2.2.0, 2.2.1, 2.5.1,
                       3.1.7, 3.1.8, 4.1.3, 4.3.6, 4.5.2, 4.8.1.
                       All .dfu and all acorn_coeffs. ext images for 4.3.6, 4.5.2, 4.8.1
                       only, since the others are byte-identical in payload.
firmware/wolfcastle/   QC35 gen 1. All 12 releases, 1.0.0 through 3.0.3.
                       All .dfu and all acorn_coeffs. ext images for 2.5.5 and 3.0.3.
tools/                 Analysis scripts, Python 3 standard library only.
findings/              Output of the first pass. See below.
research/              Live copies of Bose's own firmware index files.
RESEARCH.md            The public record: reporting, Bose's statements, community work.
```

### Tools

| Script | Purpose |
|---|---|
| `tools/xuv.py A B` | Parse two `.xuv` word dumps and report every differing word, collapsed into contiguous regions |
| `tools/show.py A B lo hi` | Print a word range from both files side by side with ASCII |
| `tools/tostr.py F lo hi` | Extract a word range as a byte string |
| `tools/dfu.py A B` | `CSR-dfu2` header fields, entropy profile, aligned block overlap |
| `tools/cdc.py A B` | Shift-tolerant diff using content-defined chunking |

### Findings files

| File | Contents |
|---|---|
| `findings/anc_coefficient_timeline.txt` | Size and hash of every ANC coefficient blob, both products |
| `findings/diff_ext_4.3.6_vs_4.5.2.txt` | The 71-word result |
| `findings/diff_ext_4.5.2_vs_4.8.1.txt` | Same comparison one release later, as a control |
| `findings/diff_dfu_4.3.6_vs_4.5.2.txt` | Changed-region map of the application image |
| `findings/strings_diff_4.3.6_vs_4.5.2.txt` | Every string added and removed |
| `findings/strings_4.*.txt` | Full unswapped string dumps per version |
| `findings/SHA256SUMS.txt` | Hash manifest for everything |

## File formats, for reference

**`.xuv`** is the CSR flash format. Plain ASCII, one 16-bit word per line, written as
`@ADDRESS   HHHH` with CRLF endings. Addresses are word addresses, so byte offset is twice
the address. Both `ext_signed` and `acorn_coeffs_signed` use it. The first four words spell
`fsr_dfu1` once each word is byte-swapped, then a signature block follows.

**`.dfu`** is the `CSR-dfu2` container carrying the Bluetooth stack and the application.
Plain binary, not encrypted, not compressed.

Text inside both formats is byte-swapped per 16-bit word. Swap before running `strings`,
otherwise the output is scrambled. `tools/` handles this.

## Provenance

The QC35 II 4.5.2 binaries are **no longer available from Bose**. Every 4.5.2 URL on
`downloads.bose.com` now returns HTTP 403, and the Wayback Machine holds no snapshot of
them. Bose's live index currently offers only 4.3.6 and 4.8.1.

The complete historical set here comes from the community archive
`github.com/bosefirmware/ced`. The 4.3.6 and 4.8.1 images in that archive were verified
byte-for-byte against fresh downloads from `downloads.bose.com`, and both matched exactly.
That is good evidence the archive is faithful, but it is not proof for 4.5.2 specifically,
since no Bose-hosted copy of 4.5.2 survives to compare against. Bose's `index.xml` does
publish a CRC for each image, so any 4.5.2 index recovered later would allow an independent
check.

Treat all of these as untrusted binary data. Analyse statically. Do not flash them without
understanding the risk, and note the community warning that a QC35 II whose serial number
ends in `AZ` can be bricked by downgrading below 2.1.3.
