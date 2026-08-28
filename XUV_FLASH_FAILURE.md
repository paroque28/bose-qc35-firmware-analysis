# The first live `.xuv` write attempt: what failed

Date: 2026-08-28.
Host: macOS (Darwin 24.6.0), Intel Mac, built-in USB.
Device: Bose QuietComfort 35 II, hardware serial `077061Z83182423AZ`, running `Main 4.5.2.144`.
Tool: `tools/bose-xuv` at commit `79fed7c`.
Outcome: the erase command was rejected by the device, no partition was written, and the
headset was left stuck in external bootmode (`05a7:40fd`) until a manual power cycle. The
device was never in danger of bricking and nothing was erased.

This is the companion to `XUV_FLASHING.md` (the protocol) and `FLASHING.md` (the successful
`.dfu` flashes). It records the first attempt to actually write an external-flash partition,
which failed, and exactly why.

## What was attempted

A deliberately safe first write: `BayWolf_4.5.2_ext_signed.xuv` (the voice-prompt partition,
SUBID 1, TARGET 1) into SQIF partition 1. This target was chosen because its payload is
byte-identical to what the device already holds, so a correct write changes nothing observable
and an incorrect one can at worst corrupt voice prompts, which the official Bose Updater
restores. The internal application firmware and the ANC coefficient partition (partition 3) are
not touched by a partition-1 write, so this attempt could not brick the headset.

The command driving it:

```
bose-xuv flash BayWolf_4.5.2_ext_signed.xuv --partition 1 --yes
```

which runs the full sequence recovered in `XUV_FLASHING.md`: enter external bootmode from
normal mode, erase the partition, stream the payload, reset back to normal.

## The exact sequence and device responses

1. **Parse.** The `.xuv` parsed cleanly to a 2,827,328-byte payload beginning with the
   `fsr_dfu1` magic. This part is verified and was not the problem.
2. **Enter external bootmode.** The tool sent the enter-bootmode command (report `03 01 01`)
   to the normal-mode device (`05a7:40fe`). The device acknowledged and re-enumerated as
   `05a7:40fd`, exactly as the non-destructive probe had already shown. This step worked.
3. **Erase partition 1.** The tool sent the erase command (`03 02 01`, opcode `0x02`, argument
   = partition number `1`) and read the response. The status byte at offset 2 came back as
   **`0x00`**, not the `0x01` that means success. The tool stopped there and wrote nothing:

   ```
   Error: erase SQIF partition: device returned status 0x00, expected 0x01
   ```

So the failure is precise and narrow. The erase opcode was **rejected by the device**. The
write loop never ran. No flash contents were changed.

## The second failure: the device would not leave bootmode

After the rejected erase, the device would not return to normal mode by software:

- Sending exit-bootmode (`03 01 00`) returned a success response (`[04, 01, 01]`, status
  `0x01` at offset 2), but the device stayed enumerated as `05a7:40fd`. IOKit
  (`system_profiler`) confirmed the PID did not change.
- Re-sending enter-bootmode (`03 01 01`) from `40fd` returned `[04, 01, 00]`, status `0x00`,
  a rejection, consistent with the device already being in bootmode.
- Repeated exit attempts, with longer settle delays and fresh device handles, all behaved the
  same: success status, no re-enumeration.

This is the important behavioral finding. Before the erase, the enter/exit bootmode cycle was
fully reversible by software and was proven so twice during the probe (`40fe -> 40fd -> 40fe`).
After the erase opcode was sent, the software exit no longer re-enumerated the device. The
erase command is therefore the trigger that latched the device into a bootmode state that only
a hardware power cycle clears.

## The device was never at risk

Every signal says the headset is healthy, just parked in the wrong mode:

- It enumerates normally on USB and responds to every command with a status byte.
- The USB serial (`077061Z83182423AZ`) is intact and unchanged.
- `bose-dfu info -f` still reads `Device model: BayWolf` and `Current firmware: Main 4.5.2.144`
  even while the device is in external bootmode. This is itself a new finding: external
  bootmode keeps servicing the normal info feature reports (report `0x02`).
- The erase was rejected, so no partition was modified. The internal application firmware and
  the coefficient partition were never addressed by any command.

Recovery is a power cycle: unplug USB, slide the power switch fully off, wait about ten
seconds, slide it on, replug. The device then boots the normal 4.5.2 firmware.

## Why the erase was most likely rejected

The three opcodes recovered by disassembly are not all wrong. Bootmode entry and exit are
acknowledged with status `0x01`, so the report ID (`0x03`), the framing (`[report][opcode][args]`),
and the status-byte offset (2) are all correct. The failure is specific to the erase step, and
the most likely causes, in rough order of probability:

1. **A missing setup step before the erase.** The updater's log strings include
   `Partitioning external flash`, `Formatting external flash partitions`, and
   `Processing EXTERNAL_FLASH_SUBID partition: %1`. These strongly suggest the erase is not the
   first thing sent after entering bootmode. There is probably a partition-select, a
   size/format, or a "begin external flash" command that must precede the erase, and without it
   the device refuses to erase. The disassembly recovered the individual opcodes but did not
   prove the exact ordering and arguments of the calls in `ExternalFlasher`, and this is the
   gap that ordering assumption fell into.
2. **A wrong erase argument.** The argument may not be the raw `TARGET` number. It could be a
   different partition index, a multi-byte value, or an address/length pair rather than a
   single partition byte.
3. **A protected partition.** Partition 1 might require an unlock or be write-protected until a
   preceding command authorizes the operation.

All three point to the same conclusion: the erase needs a command or an argument that the
current implementation does not send. This is knowable, but not from the opcode list alone.

## What this does and does not change

Still valid from before this attempt:

- The `.xuv` parser and payload reconstruction (verified offline and unchanged).
- The transport: report ID `0x03` output, response with status at offset 2, response input
  report `0x04`.
- The external bootmode PID `0x40fd`, and that it is reachable directly from normal mode.
- Bootmode entry and exit as commands (they are acknowledged, and before an erase they are
  reversible in software).

Newly learned from the failure:

- The erase opcode alone, with a bare partition-number argument, is **not** accepted. The erase
  sub-protocol needs more than the three-opcode model captured so far.
- Sending the erase opcode moves the device into a bootmode state that a software exit does not
  clear. A power cycle is required after any erase attempt, successful or not.
- External bootmode still answers the info feature reports, so the running firmware version is
  readable even in that mode.

## Consequence for the upstream plan

The precondition for forking `bose-dfu` and opening a pull request was a proven write path.
That precondition is **not met**. Upstreaming a flasher whose erase step is refused by the
device would be wrong, so the fork and the PR are on hold until the erase sequence is correct.

## Next steps, in order

1. **Recover the device** with a power cycle and confirm it returns to normal mode on 4.5.2.
2. **Recover the exact erase sequence.** Two independent ways:
   - Deeper disassembly of `ExternalFlasher` around `EraseSqif`, `Partitioning external flash`,
     and `Processing EXTERNAL_FLASH_SUBID partition`, to find the command(s) and argument(s)
     that precede the erase.
   - A USB capture (Wireshark plus USBPcap on Windows) of the official Bose Updater performing a
     real external-flash write, which shows the true command order on the wire. This is the
     definitive source and was flagged in `XUV_FLASHING.md` as the one thing worth capturing.
3. **Only then retry the safe partition-1 write**, and require a power cycle in the procedure
   after any erase attempt.
4. Fork and prepare the PR **after** the write path is proven end to end, not before.

## Lesson

The disassembly gave the vocabulary (the opcodes) but not the grammar (the exact order and
arguments). Individual commands being acknowledged is not proof that a sequence built from them
is correct. The safe-target choice worked exactly as intended: because partition 1 holds
byte-identical data and the erase was refused before any write, a genuine protocol gap surfaced
as a rejected command and a recoverable stuck mode, not as a damaged headset.
