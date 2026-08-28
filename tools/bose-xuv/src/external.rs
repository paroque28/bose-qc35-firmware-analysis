//! The external-flash (SQIF) protocol, recovered by disassembling the official Bose Updater's
//! `ExternalFlasher` class. See `XUV_FLASHING.md` for the full derivation.
//!
//! Every command is a USB HID **output** report whose first byte is the report ID `0x03`,
//! framed as `[0x03][opcode][args...]`. The device replies with an **input** report whose
//! status byte sits at offset 2, where `0x01` means success. This is a different transport
//! from the internal DFU path (which uses HID *feature* reports), so it lives in its own
//! module and shares nothing with bose-dfu's `protocol.rs`.

use anyhow::{bail, Context, Result};
use hidapi::HidDevice;
use log::{debug, trace};

/// Report ID of the external-flash output report (declared Output(1022) in the HID descriptor).
pub const REPORT_ID: u8 = 0x03;

/// Opcodes, the second byte of every command.
pub const OP_BOOTMODE: u8 = 0x01;
pub const OP_ERASE: u8 = 0x02;
pub const OP_WRITE: u8 = 0x03;

/// Bootmode control arguments.
pub const BOOTMODE_ENTER: u8 = 0x01;
pub const BOOTMODE_EXIT: u8 = 0x00;

/// The response status byte lives at offset 2; this value means success/continue.
pub const STATUS_OK: u8 = 0x01;
const STATUS_OFFSET: usize = 2;

/// Total on-the-wire size of a report-0x03 output report (report ID + 1022 data bytes),
/// matching the updater's `MaxOutputReportSize`. Every command is padded to this length.
const REPORT_SIZE: usize = 1023;

/// The first write is a 140-byte priming chunk covering the DFU signature header, then the
/// rest of the payload streams in 1019-byte chunks. Both figures are from the disassembly.
const PRIME_LEN: usize = 140;
const CHUNK_LEN: usize = 1019;

/// Read timeout in milliseconds. The updater uses 50 s, matching the manifest wait we saw on
/// the internal DFU path, so a slow erase or commit does not look like a hang.
const READ_TIMEOUT_MS: i32 = 50_000;

/// Send one report-0x03 command and return the raw response report.
///
/// The output report is padded to [`REPORT_SIZE`]. The response is read back with the long
/// timeout; the caller decides what to do with it (a bootmode switch may not answer at all
/// because the device re-enumerates).
fn execute(dev: &HidDevice, opcode: u8, args: &[u8]) -> Result<Vec<u8>> {
    let mut report = vec![0u8; REPORT_SIZE];
    report[0] = REPORT_ID;
    report[1] = opcode;
    report[2..2 + args.len()].copy_from_slice(args);

    trace!("external: write opcode={opcode:#04x} args={args:02x?}");
    dev.write(&report)
        .with_context(|| format!("writing external-flash command {opcode:#04x}"))?;

    let mut resp = [0u8; 256];
    let n = dev
        .read_timeout(&mut resp, READ_TIMEOUT_MS)
        .with_context(|| format!("reading response to command {opcode:#04x}"))?;
    trace!("external: response {} bytes: {:02x?}", n, &resp[..n.min(16)]);
    Ok(resp[..n].to_vec())
}

/// Fail unless the response carries the success status byte at offset 2.
fn check_status(resp: &[u8], what: &str) -> Result<()> {
    let status = resp
        .get(STATUS_OFFSET)
        .with_context(|| format!("{what}: response too short ({} bytes)", resp.len()))?;
    if *status != STATUS_OK {
        bail!("{what}: device returned status {status:#04x}, expected {STATUS_OK:#04x}");
    }
    Ok(())
}

/// Enter external bootmode (`03 01 01`). The device then re-enumerates under its bootmode PID.
/// Returns the raw response, which may be empty if the device resets before answering.
pub fn enter_bootmode(dev: &HidDevice) -> Result<Vec<u8>> {
    execute(dev, OP_BOOTMODE, &[BOOTMODE_ENTER])
}

/// Leave bootmode / reset the device (`03 01 00`).
pub fn exit_bootmode(dev: &HidDevice) -> Result<Vec<u8>> {
    execute(dev, OP_BOOTMODE, &[BOOTMODE_EXIT])
}

/// Erase one SQIF partition (`03 02 <partition>`). Destructive: this is the point of no return
/// for the partition's current contents.
pub fn erase_sqif(dev: &HidDevice, partition: u8) -> Result<()> {
    debug!("external: erasing SQIF partition {partition}");
    let resp = execute(dev, OP_ERASE, &[partition])?;
    check_status(&resp, "erase SQIF partition")
}

/// Write a full payload to the currently selected SQIF partition.
///
/// Streams a 140-byte priming chunk, then 1019-byte chunks, each as one `03 03 <len_be> <data>`
/// output report. Every chunk's response is checked for the success status.
pub fn write_sqif(dev: &HidDevice, payload: &[u8]) -> Result<()> {
    let mut offset = 0usize;
    let first = PRIME_LEN.min(payload.len());
    send_chunk(dev, &payload[..first])?;
    offset = offset.max(first);

    while offset < payload.len() {
        let end = (offset + CHUNK_LEN).min(payload.len());
        send_chunk(dev, &payload[offset..end])?;
        offset = end;
    }
    Ok(())
}

/// Send one write chunk: `03 03 <len_hi> <len_lo> <data...>`, padded to the full report size.
fn send_chunk(dev: &HidDevice, data: &[u8]) -> Result<()> {
    let len = data.len();
    assert!(len <= CHUNK_LEN, "chunk larger than the protocol allows");

    let mut report = vec![0u8; REPORT_SIZE];
    report[0] = REPORT_ID;
    report[1] = OP_WRITE;
    report[2] = (len >> 8) as u8; // 16-bit length, big-endian
    report[3] = (len & 0xff) as u8;
    report[4..4 + len].copy_from_slice(data);

    trace!("external: write chunk {len} bytes");
    dev.write(&report).context("writing SQIF data chunk")?;

    let mut resp = [0u8; 256];
    let n = dev
        .read_timeout(&mut resp, READ_TIMEOUT_MS)
        .context("reading SQIF chunk response")?;
    check_status(&resp[..n], "write SQIF chunk")
}
