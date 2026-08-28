# Open firmware for the Bose QC35 II: feasibility, prior art, and what it would take

A proposal, written honestly. It separates what is realistic from what is wishful, states
the hard blocker up front, and describes a route that delivers most of the practical value
without fighting that blocker at all.

---

## 1. The one-paragraph answer

A fully open, from-scratch replacement firmware for the QC35 II is **not realistically
achievable** by a small effort, and probably not by a large one. The audio path runs on a
proprietary CSR/Qualcomm BlueCore part with a closed Kalimba DSP, no public SDK for this
silicon generation, no datasheet for the ANC analogue front end, and a signed update chain.
Replacing the noise cancelling itself means re-implementing a DSP pipeline that Bose spent
years tuning, blind, on undocumented hardware. However, a **much more valuable and actually
feasible** project sits next to it: an open **control and configuration** layer plus
targeted **binary patches** to the stock firmware. That path is already partly built by
other people, needs no secure-boot break, and would give owners real control over their
headphones. Sections 5 and 6 describe it.

---

## 2. What already exists (search results)

Nobody has built an open replacement firmware for any QC35. The community work splits into
three groups, and understanding the split is the whole strategic picture.

### Group A. Firmware archival and flashing

| Project | What it does | Use to us |
|---|---|---|
| [bosefirmware/ced](https://github.com/bosefirmware/ced) | Archives every stock firmware and documents downgrade | The corpus. Source of the images analysed here. |
| [tchebb/bose-dfu](https://github.com/tchebb/bose-dfu) | Open, cross-platform tool that enters DFU mode and writes `.dfu` images over USB HID, and unlike Bose's own tool it will downgrade | **The flashing primitive.** Anything we build ships through this. |
| [tchebb/bose-dfu issue #6](https://github.com/tchebb/bose-dfu/issues/6) | Ongoing work to also flash the `.xuv` components | Relevant if we ever need to write the coefficient blob directly. |

`bose-dfu` is the single most important asset for any custom-firmware ambition. It already
puts the device into DFU mode and writes an image. What it does **not** do is bypass
signature checks, and its author is explicit that it does only basic sanity checks, not
anything cryptographic.

### Group B. Runtime control by reverse-engineering the protocol

This is where the momentum actually is, and it sidesteps the firmware entirely.

| Project | Target | Method | Relevance |
|---|---|---|---|
| [MadEyez/quietrebellion](https://github.com/MadEyez/quietrebellion) | QC Ultra 2 | Reverse-engineered **BMAP** protocol over Bluetooth RFCOMM, Android and Windows | Proves the protocol approach works. On F-Droid. |
| [aaronsb/bosectl](https://github.com/aaronsb/bosectl) | QC Ultra 2 | Same BMAP protocol, Linux | Same, for Linux. |
| [Linux Bluetooth list, QC35 ANC control](https://www.spinics.net/lists/linux-bluetooth/msg88295.html) | **QC35** | Documents QC35 control over **RFCOMM channel 8**, three-byte opcode, one-byte length, payload | Direct evidence the QC35 is controllable this way. |

The key finding from the BMAP work: Bose gates the "write a setting" operator behind
cloud-mediated authentication, but the **SETGET and START operators are unauthenticated** on
the settings and audio-mode blocks. In plain terms, you can read and drive noise cancelling
and EQ over a standard Bluetooth channel without any keys, without breaking any encryption,
and without touching the firmware. The QC35 speaks an older dialect of this on channel 8.

### Group C. Firmware reverse engineering

| Project | Status |
|---|---|
| [HarrytheOrange/boseReverse](https://github.com/HarrytheOrange/boseReverse) | States the exact goal, "recover the ANC and make the best unofficial firmware." Three commits, no published findings. Effectively abandoned. |
| [avicoder/Bose-headphones-firmware](https://github.com/avicoder/Bose-headphones-firmware) | An archive with a suggestion to use binwalk. No analysis. |
| This repository | The first published binary diff establishing what 4.5.2 actually changed. |

So: the flashing tool exists, the runtime-control approach is proven and active, and the
"open firmware" ambition specifically has been attempted once and abandoned. That tells you
where the achievable value is.

---

## 3. The hardware reality, and why a full rewrite is so hard

The QC35 II audio path is a CSR, later Qualcomm, **BlueCore** SoC. Confirmed from strings in
the shipped image: `CSR CVC CONTROL` (Clear Voice Capture) and `CSRPM parameters`. The part
pairs an **XAP** application processor with a **Kalimba** DSP.

Four independent obstacles to a from-scratch replacement, each serious on its own:

1. **No SDK for this silicon.** CSR's audio development kit for this BlueCore generation was
   never openly released and the vendor was absorbed by Qualcomm. There is no supported
   toolchain, no board support, no driver source.
2. **The DSP is undocumented and has no mature open disassembler.** Kalimba is where the
   noise cancelling actually runs. An open XAP assembler exists in the wild, but no capable
   Kalimba disassembler was found. Re-implementing the ANC means writing DSP code for a core
   with no public documentation.
3. **The ANC analogue path is undocumented.** Feed-forward and feed-back microphone
   placement, the analogue front end, filter topology, and the acoustic tuning are all
   proprietary. Even with perfect DSP tooling, matching Bose's tuning by trial and error on
   a shipping product is a multi-year acoustics project, not a software project.
4. **The update is signed.** Each shipped file carries a 132-byte signature block right after
   the `fsr_dfu1` magic (verified in this repository: identical across releases that share a
   build, changing only when the payload is rebuilt). Whether the device enforces this at
   flash time or at boot is not yet established, and that unknown is decisive. See section 4.

Blunt assessment: a group that merely wants their headphones to cancel noise well again
should **downgrade to 4.3.6** and stop there. The evidence in this repository says the old
firmware's ANC tuning is intact. Writing new ANC firmware to solve a problem the analysis
suggests does not exist in the firmware would be effort spectacularly misdirected.

---

## 4. The signing question, which decides everything

Every practical custom-firmware plan hinges on one experiment that has not yet been run:
**does the QC35 II verify the signature, and does it verify at flash time or at boot?**

Three possible worlds:

- **World 1: no meaningful enforcement.** The signature is present but the bootloader does
  not reject a bad one, or checks only a CRC. Modified images boot. Custom firmware by
  patching becomes straightforward. Least likely, but it costs one test to rule in or out.
- **World 2: verified at boot, in ROM.** A mask-ROM bootloader checks the signature before
  running the application. This is the common CSR arrangement and the most probable. Patched
  application images are rejected. Only Bose-signed images run. A full custom firmware is
  then blocked unless a bootloader flaw exists, which is its own research programme.
- **World 3: verified at flash time by the updater, not the device.** If the check lives in
  software rather than the device, `bose-dfu` writing straight to the device might bypass it
  even though the stock updater would not. Plausible, and again cheap to test.

**How to find out, safely, in order of preference:**

1. Read the CSR/Qualcomm BlueCore boot documentation and the `bose-dfu` protocol notes to
   learn where verification is specified to happen.
2. Statically locate the verifier. The bootloader is small. Its signature-check routine will
   reference the 132-byte block and a public key. Finding it in the image tells you the
   algorithm and whether it is reachable.
3. **Only with a sacrificial unit, never a daily driver:** flash a stock image with a single
   trivially flipped byte in a non-critical region and observe. Boots means weak or
   flash-time enforcement (World 1 or 3). Refuses or drops to recovery means World 2.
   The device reportedly falls back to DFU/recovery mode on a bad image, which is what makes
   this test survivable, but treat bricking as a real outcome and never do this on hardware
   you cannot afford to lose. Remember the archived warning about serials ending in `AZ`.

Until this is answered, everything about custom **firmware** is speculative. Nothing about
custom **control** (section 5) depends on it at all, which is exactly why control is the
right place to start.

---

## 5. The realistic project: open control, not open firmware

This is the recommendation. It delivers most of what an owner actually wants, it is feasible
now, and it stands entirely on proven ground.

**Build an open, cross-platform QC35 control application** that speaks the headphones' own
Bluetooth protocol on RFCOMM channel 8. No firmware modification, no signing break, no risk
to the device.

What it could realistically do, based on what the BMAP work already demonstrates on sibling
models and what the Linux Bluetooth list documents for the QC35:

- **Read and set the noise cancelling level directly**, including holding a level the app UI
  makes awkward, and reading back the actual CNC index and step count the device reports.
- **Expose settings the official app hides or has since removed**, such as the auto-off
  timer and the wake/standby behaviour.
- **Remap the action button** locally.
- **Log the ANC state over time**, which incidentally provides the empirical counterpart to
  the static analysis in this repository. You could confirm from a live device whether the
  CNC step values really are identical across firmware versions.
- **Run on Linux and as a daemon**, which no official Bose software does.

Why this is the right call:

- It is **proven**. `quietrebellion` and `bosectl` already do exactly this on the Ultra 2,
  and the QC35 protocol on channel 8 is documented.
- It needs **no exploit**. The read and action operators are unauthenticated by Bose's own
  design. Nothing is bypassed.
- It carries **no bricking risk**. You never write to flash.
- It is **useful to real people** regardless of how the ANC controversy resolves.

The one experiment worth pairing with it: use the app to read the CNC index and step count
under 4.3.6 versus 4.5.2 on a physical unit. That is the empirical test of hypothesis H1
from `REPORT_FOR_AI.md`, and the control app is the cleanest way to run it.

---

## 6. If you insist on touching the firmware: patch, do not rewrite

Between "control app" and "from-scratch firmware" lies a middle path that is far more
tractable than a rewrite: **surgical binary patches to the stock image.** The image is not
encrypted or compressed (measured 6.35 bits per byte), so it can be edited directly. The
only blocker is signing, which is why section 4 must be answered first.

Realistic patch-level goals, assuming World 1 or World 3 from section 4:

- **Unlock or rebalance the CNC steps.** If the level table is a short run of integers in
  the application, editing it changes the available noise cancelling levels. This is a
  handful of bytes, not a DSP rewrite.
- **Change default power and timeout behaviour.**
- **Re-enable or alter the action-button mapping at the source.**
- **Adjust the voice-prompt set**, which is plain WAV data in the `ext` image.

What patching **cannot** realistically deliver: new or better ANC. That lives in the Kalimba
DSP and the coefficient blob, and improving it needs the tuning and acoustics work described
in section 3. Patching lets you change *policy*, not *signal processing*.

The honest sequencing: only pursue patching after (a) section 4 shows modified images run,
and (b) the disassembly in `REPORT_FOR_AI.md` step 1 has located the tables worth editing.
Absent both, patching is guesswork against a signed target.

---

## 7. What it would take to flash a custom firmware, concretely

Assembled as a checklist, most-blocking item first. Items 1 and 2 are the gates. If either
fails, stop, and the control app in section 5 is your project instead.

1. **A sacrificial QC35 II.** Non-negotiable. Assume you will brick at least one. Do not
   use a unit whose serial ends in `AZ` for anything below 2.1.3.
2. **Resolve the signing question (section 4).** Determine whether modified images run at
   all. This gates the entire firmware path. Everything below is wasted effort until this is
   answered yes.
3. **The flashing primitive.** [`tchebb/bose-dfu`](https://github.com/tchebb/bose-dfu),
   built from source, with the udev rules on Linux. It already enters DFU mode and writes a
   `.dfu`. Confirm it round-trips a **stock** image on your sacrificial unit first, so you
   know recovery works before you ever write a modified one.
4. **A recovery path you have tested.** Confirm the device drops into DFU/recovery on a bad
   image and that you can rewrite a stock image from that state. Do this deliberately, once,
   before it happens by accident.
5. **A disassembler for the target core.** For patch work you need to read the code around
   the change. XAP has an open assembler in the wild; Kalimba likely needs tooling written.
   For pure table edits (section 6) you may get away with the string-anchor offsets already
   in `REPORT_FOR_AI.md` and careful hex editing.
6. **The change itself, and a way to re-satisfy the container.** Even in World 1, the
   `CSR-dfu2` header carries a length field (filesize minus 16) and the image carries a
   trailing checksum. A patch must update these. If any signature is enforced (World 2), you
   are blocked here, full stop, absent a signing key or a bootloader flaw. Do not plan around
   defeating cryptography.
7. **Validation on the sacrificial unit, then and only then a decision** about whether the
   result is safe for a daily-driver device.

Time and risk, stated plainly. The control app (section 5): a weekend to a couple of weeks,
near-zero risk, high certainty of success. A useful firmware **patch** (section 6): weeks to
months, real bricking risk, and **entirely contingent on the signing test in item 2**. A
from-scratch **open firmware** (section 3): effectively a research programme with a low
probability of ever matching stock ANC, and not recommended.

---

## 8. Advantages and disadvantages, summarised

### Open control application (recommended)

**Advantages.** Feasible now. No device risk. No exploit needed. Linux and headless support
that Bose never shipped. Direct read-back of ANC state, which doubles as empirical
verification of this repository's static findings. Builds on two active, proven projects.

**Disadvantages.** Cannot change signal processing or fix genuine ANC hardware issues.
Bounded by what the protocol exposes. The unauthenticated operators could be locked down by
a future Bose firmware, though the QC35 II is end-of-life and unlikely to see such a change.

### Firmware patching

**Advantages.** Can change device policy at the source: CNC steps, timeouts, button mapping,
voice prompts. Works on an unencrypted image. Small, auditable changes.

**Disadvantages.** Gated entirely on the signing test. Real bricking risk. Needs disassembly
tooling for full confidence. Cannot improve ANC. A signed, boot-verified device (the most
likely case) blocks it outright.

### From-scratch open firmware

**Advantages.** Total control in principle. Community ownership of a popular, now-unsupported
product. Genuinely interesting reverse-engineering.

**Disadvantages.** No SDK, no DSP documentation, no open Kalimba disassembler, undocumented
ANC analogue path, and a signed boot chain. Re-tuning ANC blind is a multi-year acoustics
effort. One prior attempt exists and was abandoned. The payoff for the ANC complaint
specifically is near zero, because this repository's evidence says the old firmware's ANC
was never broken. Not recommended.

---

## 9. Recommendation

1. **For an owner who just wants good ANC back:** downgrade to 4.3.6 with `bose-dfu`, check
   the ear cushions are genuine and fully seated, factory-reset. No new firmware required.
2. **For someone who wants to build something open and useful:** write the QC35 control app
   of section 5. It is the achievable, low-risk, high-value project, and it produces the
   live measurement that would let anyone finally settle the 4.5.2 question empirically.
3. **For a reverse-engineer specifically motivated by custom firmware:** answer the signing
   question in section 4 on a sacrificial unit *before* investing in anything else. That one
   result determines whether patching (section 6) is a real option or a dead end, and it is
   cheap to obtain.
4. **A from-scratch open firmware is not a sensible goal** for this hardware, and least of
   all as a fix for an ANC regression that the binary evidence indicates never happened.

---

## Sources

- [tchebb/bose-dfu](https://github.com/tchebb/bose-dfu) and [issue #6, flashing `.xuv`](https://github.com/tchebb/bose-dfu/issues/6)
- [MadEyez/quietrebellion](https://github.com/MadEyez/quietrebellion) and [on F-Droid](https://f-droid.org/packages/net.quietrebellion/)
- [aaronsb/bosectl](https://github.com/aaronsb/bosectl)
- [Linux Bluetooth list: QC35 battery and ANC control over RFCOMM channel 8](https://www.spinics.net/lists/linux-bluetooth/msg88295.html)
- [HarrytheOrange/boseReverse](https://github.com/HarrytheOrange/boseReverse)
- [bosefirmware/ced](https://github.com/bosefirmware/ced)
- Binary evidence on signing blocks, entropy, and coefficient stability: this repository, `REPORT_FOR_AI.md` and `findings/`.
