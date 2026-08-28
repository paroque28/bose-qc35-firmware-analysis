# Flashing 4.3.6 onto a live QC35 II: full report

Date: 2026-08-28.
Host: macOS (Darwin 24.6.0), Intel Mac with a T2 bridge, USB via the built-in ports.
Device: Bose QuietComfort 35 II, hardware serial `077061Z83182423AZ`.
Outcome: success. The device reports `Main 4.3.6.105` after the flash and reboots normally.

This documents the first live-device experiment in this project.
Until now everything in this repository was static analysis. This run proves the archived
binaries are actually flashable and gives us a headset running 4.3.6 for listening
comparisons against the blamed 4.5.2 lineage.

## What was flashed

`firmware/baywolf/BayWolf_4.3.6_stack_plus_app.dfu`, SHA-256
`df6d8d667358e16509f31da595c9148236d2a0f6490e7da08699cca808fab97d`, verified against
`findings/SHA256SUMS.txt` before flashing.

4.3.6 was chosen deliberately as the safest image in the archive.
It is one of the two versions (with 4.8.1) that the archive verified byte-for-byte against a
fresh download from `downloads.bose.com`, so its provenance is stronger than that of 4.5.2.

Only the `.dfu` application image was flashed.
The two `.xuv` companions (`acorn_coeffs_signed`, `ext_signed`) were not, because the tool
used cannot flash them (see "The .xuv gap" below). For this particular downgrade that is
acceptable: the repository's own analysis proved both payloads byte-identical from 2.0.1
through 4.5.2, and the community has downgraded from 4.8.1 this same way at scale.

## Tooling

- `bose-dfu` 1.1.0 (github.com/tchebb/bose-dfu, installed with `cargo install bose-dfu`).
  Rust was installed with `brew install rust` first.
- `bose-dfu` speaks the CSR/Qualcomm DFU protocol over USB HID. No driver is needed on macOS.

## The procedure, exactly as run

1. Verify the image hash against `findings/SHA256SUMS.txt`. It matched.
2. Connect the headset over USB, powered on, with a data-capable micro-USB cable.
   It enumerates as `05a7:40fe` ("Bose QC35 II", 12 Mb/s full speed).
3. `bose-dfu list` reports `compatible device in normal mode`.
4. `bose-dfu enter-dfu`. The device drops off the bus and re-enumerates as `05a7:4020`
   after a few seconds.
5. `bose-dfu download -f firmware/baywolf/BayWolf_4.3.6_stack_plus_app.dfu`.
   The tool validated the file's DFU suffix CRC (`0x41cf4ee7`), confirmed the update targets
   the connected device, transferred the 1,939,792-byte image, and then honored a
   device-requested manifest wait of 50.5 seconds while the headset committed the firmware
   internally. Total time was a few minutes.
6. `bose-dfu leave-dfu -f`. The device rebooted and re-enumerated as `05a7:40fe`.
7. `bose-dfu info -f` reports: model `BayWolf`, hardware serial `077061Z83182423AZ`,
   `Current firmware: Main 4.3.6.105`.

The `.105` build suffix matches the `REVISION` attribute Bose's own `index.xml` gives for
4.3.6, which is a second, independent confirmation that the archived image is the genuine
release build.

## Why `-f` was needed, and why it was safe

`bose-dfu` 1.1.0 ships a compiled-in device table with a single entry, the SoundLink
Color II (`40fe` normal, `400d` DFU). Our QC35 II shares the normal-mode PID `40fe`, so in
normal mode it is reported as "compatible". In DFU mode it presents `4020`, which is not in
the table, so the tool downgrades it to "UNTESTED device in unknown mode" and refuses to act
without `-f`.

Before forcing, `4020` was confirmed to be the genuine QC35 II update-mode PID from three
independent directions:

1. The upstream `bose-dfu` README lists the QC35 II as a tested device (partial support,
   meaning `.dfu` only).
2. An independent downgrade guide (github.com/sunzj/Way_of_Downgrade_BOSE_QC35ii) shows
   `05a7:4020` as the ID the headset presents while the official Bose Updater flashes it.
3. Strongest of all: `bose-dfu file-info` on the firmware file itself prints
   `For USB ID: 05a7:4020`. Bose's own signed image names that ID as its target in the DFU
   suffix. The "untested" label was purely the tool's conservative table, not a real anomaly.

## The AZ serial caveat

This unit's serial ends in `AZ`, which is exactly the hardware revision the community warns
about: flashing firmware older than 2.1.3 can brick it permanently. That warning did not
apply here (4.3.6 is far above 2.1.3), but it is now confirmed that this specific unit is an
`AZ` revision, so 2.0.1 must never be flashed onto it.

## Open caveat: the coefficient partition may not match the app

The firmware version running before the flash was not recorded (see lessons learned).
If the headset was on 4.8.1 before, its ANC coefficient partition holds the 2020 blob
(39,568 bytes, `cb14476 - May 19, 2020` build stamp), because only the application image was
rewritten. The 4.3.6 application would then be running against 4.8.1-era coefficients.

Two mitigating facts. First, the `.dfu` image carries its own embedded copy of the 2017
coefficient payload, and it is not yet known whether the application rewrites the
coefficient partition from that copy on first boot. Second, thousands of community
downgrades from 4.8.1 took this exact path and reported restored ANC behavior.
Still, for a rigorous listening comparison this should be resolved, either by dumping the
coefficient partition or by fixing the `.xuv` gap below and flashing
`BayWolf_4.3.6_acorn_coeffs_signed.xuv` explicitly.

## The .xuv gap (bose-dfu issue 6) and what fixing it would take

`bose-dfu` can only flash `.dfu` containers. The official Bose Updater additionally writes
the two `.xuv` images (the ANC coefficients and the external flash partition holding the
voice prompts). Issue 6 on the upstream tracker asks how. A fix would need:

1. **A USB capture of the official updater doing a full update.** Wireshark with USBPcap on
   Windows, against the legacy Bose Updater, while it installs a version whose `.xuv`
   content actually differs (4.8.1 is the only such QC35 II release). The capture answers
   the one real unknown: whether the `.xuv` payload travels over the same HID DFU download
   channel as a second image, or needs a partition-select or different report sequence
   first.
2. **Reconstructing the wire payload from the `.xuv` text.** This part is essentially done
   in this repository already. The format is one 16-bit word per line, the payload starts
   with an `fsr_dfu1` magic and a signature block, and `tools/xuv.py` parses it. Converting
   the text dump back to the binary image is trivial.
3. **Implementing it in bose-dfu.** The crate already contains the full HID DFU state
   machine in its protocol module. The work is a small `.xuv` parser, the container
   conversion, and wiring a second download into the existing flow. The `fsr_dfu1` magic
   (plausibly "flash serial ROM DFU, format 1") suggests the device side already treats it
   as a DFU-class image, which would make the change small.
4. **A sacrificial test device.** A wrong write to the external flash corrupts voice
   prompts or, worse, the ANC coefficients. While Bose still serves full official updates
   this is recoverable, but the test should not be run first on a headset one cares about.

## Lessons learned

1. **Record the starting state before touching the device.** `bose-dfu info -f` reads the
   model, serial, and current firmware version in normal mode. Running it before the flash
   would have recorded which version the headset was actually on. This run learned about
   the command only afterwards, so the prior version of this unit is now unknowable.
   Always: `list`, then `info`, then hash-verify the file, then flash.
2. **Charge-only micro-USB cables are the first failure mode.** The headset was invisible
   to the USB tree through the first cable tried. Nothing in software distinguishes a
   charge-only cable from an unplugged device, so check `system_profiler SPUSBDataType`
   for vendor `05a7` before debugging anything else.
3. **A tool's warning label is data, not a verdict.** The "UNTESTED device in unknown mode"
   warning came from a one-entry PID table, while the tool's own README, a community guide,
   and the firmware file's own DFU suffix all confirmed the device was fine. Reading the
   crate source (cached locally under `~/.cargo/registry`, no network needed) took minutes
   and turned a scary warning into an understood, bounded risk.
4. **`file-info` before `download`.** It validates the suffix CRC and prints the USB ID the
   image targets, offline. It is the cheapest possible pre-flight check and it doubles as
   confirmation of which PID the device should present in DFU mode.
5. **The manifest wait is normal.** After the transfer the device requested 50.5 seconds to
   commit the image. Do not unplug during this window. The tool prints it clearly.
6. **The device stays recoverable in DFU mode.** Between `enter-dfu` and `leave-dfu` the
   headset sat happily at `05a7:4020` through listing, inspection, and the flash itself.
   Nothing suggested a narrow timing window, unlike the file-swap race the Windows guide
   relies on.
7. **Version suffixes cross-check provenance.** The device reporting `4.3.6.105` matches
   `REVISION="105"` in Bose's `index.xml` and the `4.3.6-105` string found in the ext image
   analysis. Three sources agreeing on an obscure build number is strong evidence nothing
   was tampered with.

## Follow-ups this enables

1. Record the headset's ANC behavior on 4.3.6 (low versus high difference) as a baseline.
2. Flash 4.5.2 (the blamed release, `.dfu` also archived here) on the same unit and repeat
   the same listening test. Same hardware, same cushions, same ears, only the firmware
   differs. That is the experiment the whole controversy never had.
3. Resolve the coefficient-partition question above before treating any listening result as
   final.

## Second flash: 4.5.2 (same day)

Follow-up 2 was executed the same day, on the same unit, with the same procedure.

- Image: `firmware/baywolf/BayWolf_4.5.2_stack_plus_app.dfu`, hash verified against
  `findings/SHA256SUMS.txt` before flashing.
- Starting state recorded this time (lesson 1 applied): `Main 4.3.6.105`.
- `enter-dfu`, then `download`. The transfer completed and the device requested a
  50.6-second manifest wait, essentially identical to the 4.3.6 run.
- `leave-dfu -f`, and after re-enumeration at `05a7:40fe` the device reports
  `Current firmware: Main 4.5.2.144`.
- The `.144` suffix matches `REVISION="4.5.2.144"` in Bose's `index.xml`, the same
  three-way provenance check that validated 4.3.6.

One operational note: the download ran as a background task and the terminal session was
interrupted during the manifest wait. The device simply stayed parked in DFU mode
(`05a7:4020`) until `leave-dfu` was sent later. This confirms lesson 6 again: DFU mode is a
stable parking state, not a timing-critical window.

The coefficient caveat does not apply to this transition. The 4.3.6 and 4.5.2
`acorn_coeffs_signed.xuv` payloads are byte-identical (proved in this repository's static
analysis), so whatever the coefficient partition held under 4.3.6 is equally correct for
4.5.2. The unit is now running the blamed release, ready for the listening comparison
against the 4.3.6 baseline.
