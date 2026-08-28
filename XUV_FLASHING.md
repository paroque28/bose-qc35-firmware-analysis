# How the `.xuv` files are flashed: answering bose-dfu issue 6

Question that started this: `bose-dfu` can only flash the `.dfu` application image. The
official Bose Updater additionally writes the two `.xuv` images (the ANC coefficient blob and
the 22.6 MB external-flash partition). Upstream issue 6 asks how those `.xuv` writes work.
This document answers it from static analysis of the official macOS updater, with no device
touched.

## Method

- Binary examined: `Bose Updater.app/Contents/MacOS/Bose Updater`, version 6.0.0.4388,
  extracted from `BoseUpdater_6.0.0.4388.dmg`, SHA-256
  `eaa05106bfcdbda4d8989ffa02454ace6defc5c9b2ee04a3a14dad2bd0a567d4`. It comes from the same
  `github.com/bosefirmware/ced` archive that supplied our verified firmware.
- It is a 64-bit x86_64 Qt application. Not encrypted, not packed. The C++ class names and
  source file paths survive in the `__TEXT,__cstring` section, so the whole update state
  machine is legible from strings alone.
- The binary was never executed. It was mounted read-only, copied, and its string table
  extracted with `otool`. The full dump is in the scratchpad as `cstrings.txt`.

## The short answer

The `.xuv` files are **DFU-signed images written to the external SPI flash (SQIF: Serial
Quad I/O Flash)**, not to the internal application area. They travel over the same USB HID
DFU transport as the `.dfu`, but in a **separate device mode** ("external bootmode"), through
a separate flasher class (`ExternalFlasher`) with its own erase, partition, and write
commands. `bose-dfu` implements only the internal path, which is why it cannot write them.

This is a normal CSR/Qualcomm two-stage arrangement, not a Bose-proprietary side channel.
There is no encryption in the way. A `bose-dfu` fork could implement it.

## The image-to-partition map (from `index.xml`)

Every release lists three images. The `SUBID` and `TARGET` attributes are the key:

| File | SUBID | TARGET | Where it goes |
|---|---|---|---|
| `*_stack_plus_app.dfu` | 0 | (none) | Internal firmware. HID DFU. This is what bose-dfu flashes. |
| `*_ext_signed.xuv` | 1 | 1 | External flash (SQIF) partition 1. Voice prompts. |
| `*_acorn_coeffs_signed.xuv` | 2 | 3 | External flash (SQIF) partition 3. The ANC coefficients. |

So the ANC coefficient blob, the file at the center of this whole investigation, is SQIF
partition 3. The `.dfu` carries `CRC="0XFFFFFFFF"` in the index (meaning "do not CRC-check,
the DFU suffix covers it"), while each `.xuv` carries a real CRC that the updater verifies.

## The device has three USB identities, not two

We already knew two from flashing 4.3.6:

- **Normal mode**: `05a7:40fe`. Bluetooth works, `bose-dfu list` shows "normal mode".
- **Internal bootmode / HID DFU**: `05a7:4020`. Reached with `bose-dfu enter-dfu`. The `.dfu`
  is written here. (Note `index.xml` labels the whole device `ID="0x4020"`, keyed on this
  bootmode PID.)

The updater reveals a third, now **confirmed on hardware**:

- **External bootmode**: `05a7:40fd`. Entered by sending the enter-bootmode command directly
  from normal mode. The `.xuv` images are written here. This PID is distinct from both normal
  mode (`0x40fe`) and internal DFU (`0x4020`).

The updater has a generic reset primitive with these named transitions (all present as
strings): `Enter Internal Bootmode` / `Exit Internal Bootmode` /
`Exit Internal Bootmode with Micro`, and `Enter External Bootmode` / `Exit External
Bootmode`. After each reset it polls for the device to disappear and re-enumerate under the
expected new PID (`The Device did not reset into the new bootmode (0x%1) in time`,
`Device with original PID (0x%1) detected... retrying reset commands`). This is exactly the
disappear/re-enumerate dance we watched during the 4.3.6 flash, generalised to more modes.

**Resolved (was an open point).** Static analysis could not tell whether the QC35 II gives
external bootmode its own PID or reuses `0x4020`. A non-destructive probe on the actual unit
settled it: sending `03 01 01` (enter external bootmode) in normal mode is acknowledged with
status `0x01`, and the device re-enumerates as `05a7:40fd`, a third, distinct PID. Sending
`03 01 00` (exit) returns it to `05a7:40fe`. Two enter/exit cycles left the headset healthy
on `4.5.2.144` with its serial intact, and no partition was erased, because the erase is a
separate opcode. A further finding: external bootmode is reachable **directly from normal
mode**, so the internal-then-external ordering the update sequence implies is not required for
the external path alone.

## The full update sequence, in order

Reconstructed from the state and log strings (`ExternalFlasher`, `Device`, and the
`reset-*` transition names):

1. **Enter internal bootmode.** Reset the device from normal mode into the internal DFU
   bootmode (`reset-enter-internal-bootmode`, `Waiting for internal bootmode`). This is
   `bose-dfu enter-dfu`.
2. **Write the internal firmware** (`*_stack_plus_app.dfu`, SUBID 0) via the standard HID DFU
   flasher (`HidDfuFlasher`, `Downloading firmware to device...`, `Transferring firmware to
   device...`). This is the entire scope of `bose-dfu` today.
3. **Reset, then enter external bootmode** (`Waiting for internal bootmode reset`,
   `reset-enter-external-bootmode`, `Waiting for external bootmode`).
4. **Partition/format the external flash** (`Partitioning external flash`,
   `Formatting external flash partitions`). The updater can repartition on recovery
   (`EXTERNAL_BOOTMODE_RECOVERY: Partitioning External Flash`).
5. **For each external image** (`Processing EXTERNAL_FLASH_SUBID partition: %1`): erase the
   target SQIF partition, then write it (`Erasing SQIF %1...`, `Writing SQIF image...`),
   then validate (`Validating downloaded SQIF image`). Language packs are handled here too
   (`SINGLE_LANGUAGE_PACK_PARTITION`), which is why the 22.6 MB ext image is mostly
   thirteen-language voice prompts, as our earlier analysis found.
6. **Final resets** back to normal, including a DSP reset (`reset-final-dsp-reset`,
   `Waiting for reset after final DSP`, `Resetting Device after external flashing`).

The `.xuv` file itself is validated as a DFU-signed container before writing
(`Input file is not DFU signed`, `File format is invalid, no payload found`,
`GetDfuFileLength`). That matches the `fsr_dfu1` magic and signature block our `tools/xuv.py`
already parses. So the ExternalFlasher extracts the same payload our tools see and writes it
to the SQIF target partition named by the `TARGET` attribute.

## Two device families in the updater, and which one the QC35 II is

The binary carries two parallel device abstractions:

- **`Device` + `HidDfu*` + `ExternalFlasher`**: the CSR/Qualcomm BlueCore path (SQIF, HID DFU,
  bootmode PIDs). **The QC35 II uses this one.**
- **`ChibiDevice` + `FirmwareUpdate` function block** (`DataTransferSeqNumber`, GAIA-style
  function blocks, routing ports): the newer Bose path for later products. Not relevant to
  the QC35 II.

Keeping these straight matters for a reimplementation: only the first family applies here, and
it is the simpler of the two.

## The recovered protocol (from disassembly)

The updater binary is unstripped, with full C++ symbols and the `ExternalFlasher` class
methods individually named. Disassembling them (LLVM `objdump`, x86_64) recovered the exact
wire protocol, so the "missing numeric opcodes" gap noted below is now closed for the
external-flash path. The device was not touched to obtain any of this.

**Transport.** Every command is a USB HID report whose first byte is the report ID `0x03`.
The frame is:

```
[0x03] [opcode] [args...]
```

The host writes this as an output report (`ExternalFlasher::ExecuteCommand(opcode, payload)`
builds exactly `buf[0]=0x03`, `buf[1]=opcode`, then appends the payload), then reads a
response report back. In the response, the **status byte is at offset 2**: `0x01` means
success/continue. The read uses a 50000 ms (50 s) timeout, which is the same manifest wait we
saw during the 4.3.6 flash.

**Command set** (`ExternalFlasher`, opcode = the `signed char` first argument):

| Opcode | Name | Payload | Meaning |
|---|---|---|---|
| `0x01` | bootmode control | 1 byte | `0x01` = enter external bootmode (`EnterBootmode`), `0x00` = reset / exit bootmode (`ResetDevice`) |
| `0x02` | erase SQIF partition | 1 byte | partition number to erase (`EraseSqif(int)`, payload is exactly that int) |
| `0x03` | write data chunk | `[len_hi][len_lo]` + data | 16-bit big-endian byte count (up to 1019) followed by that many payload bytes (`WriteSqif`) |

**Write loop** (`WriteSqif`). The report buffer starts `0x03 0x03` (report ID, write opcode),
bytes 2 and 3 are the chunk length big-endian, and the data follows from byte 4. It reads the
`.xuv` in an initial 140-byte (`0x8c`) priming read (the DFU signature header), then loops
reading 1019-byte (`0x3fb`) chunks and sending each as one `0x03` report (total report size
1023 = `0x3ff`), until a short read ends the file. `ReadXuv(QFile&, buf, len)` parses the
`@ADDR HHHH` word lines into raw bytes, carrying the odd byte between reads. This is the same
payload `tools/xuv.py` already extracts.

**File validation.** Before writing, `GetFileSignature` / `RunVerification` confirm the `.xuv`
is a DFU-signed image (the `fsr_dfu1` magic and signature block). An unsigned or malformed
file is rejected (`Input file is not DFU signed`, `File format is invalid, no payload found`).

**Full external-flash sequence for the QC35 II** (CSR `Device` path):

1. Enter external bootmode: `03 01 01`, wait for the device to re-enumerate under its bootmode
   PID (learned via `getBootmodePid`).
2. For each external image, in `SUBID` order, using the partition number from the `TARGET`
   attribute (`ext_signed.xuv` = 1, `acorn_coeffs_signed.xuv` = 3):
   a. Erase: `03 02 <target>`.
   b. Write: stream `03 03 <len_be> <data>` chunks of up to 1019 bytes until the file ends.
   c. Read the final response and confirm status `0x01`.
3. Reset out of bootmode: `03 01 00`, then the normal-mode / DSP resets.

That is a complete, implementable specification. Both residual unknowns are now closed on
hardware (see the probe result above): the external bootmode PID is `0x40fd`, and report ID
`0x03` output with a response carrying status `0x01` at offset 2 is confirmed, because the
enter-bootmode command was acknowledged exactly that way. The response arrives as input report
`0x04` (`[04, 01, 01]`).

## Could issue 6 be fixed? Yes. What it would take

Nothing found here blocks a clean-room reimplementation. The expensive part, recovering the
wire protocol, is already done (see "The recovered protocol" above). What remains is a bounded,
mostly mechanical piece of work with exactly one genuine subtlety.

### What is already in hand

1. **The full protocol.** Report ID `0x03`, three opcodes (`0x01` bootmode, `0x02` erase,
   `0x03` write), the 1019-byte chunking, the 140-byte priming read, and the status byte at
   offset 2. Recovered by disassembling the unstripped `ExternalFlasher` methods, so a USB
   capture of the official updater is now only a confirmation step, not the way in.
2. **A working `.xuv` parser**, `tools/xuv.py`. It already turns the `@ADDR HHHH` text into the
   word map. Reconstructing the raw byte payload from that map is a few more lines.
3. **The device's HID report descriptor**, read passively from the actual QC35 II, confirming
   that report ID `0x03` is a real Output report of 1022 data bytes. The framing is verified
   against hardware.

### The one real subtlety: Output reports, not Feature reports

This is the detail that shapes the whole implementation, and it was confirmed by reading the
`bose-dfu` 1.1.0 source (`src/protocol.rs`). The existing internal DFU path speaks entirely in
**Feature reports** (`send_feature_report` / `get_feature_report`). The external flasher does
not. Report `0x03` is declared **Output**, and its response comes back as an **Input** report.
In `hidapi` terms that means `device.write()` and `device.read()`, not the feature-report
calls. So the external flasher is a *parallel* transport alongside the one already in the
crate, not a change to it. That is good news: the work is a new module, and the working
internal DFU code is left untouched.

### The code to write

Four pieces, all small, on top of the existing crate:

1. **A `.xuv` reader in Rust** (about 40 lines). Port `tools/xuv.py`: parse the word lines,
   unswap each 16-bit word to bytes, return a byte stream. The `fsr_dfu1` header travels as-is,
   because it is part of the signed payload the device expects.
2. **An `external.rs` module** (about 120 lines) mirroring `ExternalFlasher`:
   - `enter_external_bootmode`: `write([0x03, 0x01, 0x01])`, then poll for re-enumeration.
   - `erase_sqif(target)`: `write([0x03, 0x02, target])`, read status.
   - `write_sqif(reader)`: the loop. A 140-byte priming write, then 1019-byte chunks framed as
     `[0x03][0x03][len_be_hi][len_be_lo][data...]`, each sent as one `write()`, checking the
     status byte at offset 2 of each response.
   - `exit_bootmode`: `write([0x03, 0x01, 0x00])`.
3. **Re-enumeration polling** (about 30 lines). After a bootmode reset the device drops off the
   bus and comes back. `bose-dfu` already does this dance for `enter-dfu` / `leave-dfu`, so the
   loop is copy-and-adapt: close the handle, poll the PID until it reappears.
4. **A `download-external` subcommand** in `main.rs` (about 30 lines) taking an `.xuv` file plus
   the target partition from the `TARGET` attribute in `index.xml` (`1` for the voice prompts,
   `3` for the coefficients), and driving the sequence.

That is a few hundred lines total.

### The two residual unknowns (need a device, cheaply resolved)

Neither blocks writing the code, only the first live run:

- **Does external bootmode present a distinct PID or reuse `0x4020`?** The updater learns it at
  runtime via `getBootmodePid`. On the CSR-based QC35 II it may just reuse `0x4020`. One dry
  run settles it: enter external bootmode and watch which PID appears.
- **Does the device acknowledge Output report `0x03` as the disassembly implies?** The
  descriptor says the report exists. The first `erase` response confirms the status-byte
  semantics.

### The safe way to prove it

A wrong write to SQIF corrupts voice prompts or the ANC coefficients, so the first live test
must remove almost all risk. The `ext_signed.xuv` payload (partition 1) is byte-identical
across 4.3.6, 4.5.2, and 4.8.1 (only the signature and version stamp differ, per our earlier
diff). So the very first live SQIF write can push content the device already holds. If the
command sequence is wrong, it fails at `erase` or `write` before damaging anything meaningful.
If it succeeds, nothing actually changed. Only after that dry run proves the sequence should
the coefficient partition (TARGET 3) be written, the one with real consequences, and even that
stays recoverable while Bose still serves the full official update.

### Recommendation

It is worth doing, and it is the tool this whole ANC investigation actually needs: the
coefficient blob is SQIF partition 3, and `.dfu`-only flashes can never set it deterministically.
But for the immediate 4.3.6-versus-4.5.2 listening test it is not on the critical path, because
those two versions share byte-identical coefficient payloads. The sensible order is to do the
listening comparison first on the firmware already flashed, then build the external flasher as
the next project. The Rust `.xuv` reader and the `external.rs` skeleton can both be written
with no device touched, so that when the time comes the only step left is the single dry run to
pin down the bootmode PID.

## Status: the tool exists (`tools/bose-xuv`)

The external flasher described above is now written, as a small standalone Rust crate in
`tools/bose-xuv`. It does not fork bose-dfu, because the external path is a different transport
(HID output and input reports, not feature reports), so it shares no code with it. Three of its
four parts are verified:

- **`xuv.rs`**: parses a `.xuv` into its little-endian payload and checks the `fsr_dfu1` magic.
  Verified offline against both real 4.5.2 images (coefficients 4118 bytes, external 2.83 MB),
  with unit tests for the magic, address gaps, and malformed input.
- **`external.rs`**: the three commands (bootmode, erase, write) and the
  prime-then-1019-byte-chunk write loop, exactly as recovered.
- **`main.rs`**: `parse` (offline), `probe` (non-destructive bootmode reconnaissance), `reset`
  (exit bootmode), and `flash` (the full enter, erase, write, reset sequence, guarded by
  `--yes`).

What is confirmed on the actual QC35 II: the parser, the transport (report `0x03` out,
`0x04` in, status `0x01` at offset 2), the external bootmode PID (`0x40fd`), and that
enter/exit bootmode is fully reversible with the device left healthy on `4.5.2.144`.

What is deliberately not yet exercised: the live `erase` and `write`. The safe first target is
`ext_signed.xuv` into partition 1 (voice prompts), because that payload is byte-identical to
what the device already holds, so a correct write changes nothing and a wrong one can only
corrupt voice prompts, which the official updater restores. It cannot brick the headset: the
internal firmware and the ANC coefficient partition are untouched. Only after that write is
proven should partition 3 (the coefficients) be written.

## Why this matters for the ANC investigation

This is not just tooling. The coefficient blob is SQIF partition 3, and our flash of 4.3.6
rewrote **only** the internal firmware (SUBID 0). If this unit was on 4.8.1 before, its
coefficient partition still holds the 2020 blob (39568 bytes) while the application is now
4.3.6. An external flasher, once built, is exactly the tool that would let us set the
coefficient partition deterministically and remove that ambiguity, so a 4.3.6-versus-4.5.2
listening test compares only what we intend.

## Loose end: the index CRC

`index.xml` lists a real CRC for each `.xuv` (4.5.2 ext = `0xC64078AA`). A plain CRC32 of the
reconstructed payload (either byte order) does not match, so the updater uses a different
polynomial or a different byte span (CSR firmware CRCs commonly exclude the signature block).
Recovering the exact CRC routine from the updater (the code behind
`Validating downloaded SQIF image`) would let us independently verify the archived 4.5.2
images against Bose's own published checksum, which is the provenance check the project README
noted is currently missing for 4.5.2. Worth doing, not yet done.
