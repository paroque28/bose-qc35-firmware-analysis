//! bose-xuv: flash the `.xuv` external-flash (SQIF) images the official Bose Updater writes
//! but bose-dfu does not. See `XUV_FLASHING.md` in this repository for the protocol derivation.
//!
//! Subcommands, in increasing order of consequence:
//!   parse   read a .xuv and report its payload. Never touches the device.
//!   probe   enter external bootmode, report the USB PID it presents, then exit. No erase.
//!   flash   erase a SQIF partition and write a .xuv into it. Destructive. Guarded by --yes.

mod external;
mod xuv;

use anyhow::{bail, Context, Result};
use clap::Parser;
use hidapi::{DeviceInfo, HidApi};
use log::{info, warn};
use std::io::BufReader;
use std::path::PathBuf;
use std::thread::sleep;
use std::time::{Duration, Instant};

/// Bose's USB vendor ID.
const BOSE_VID: u16 = 0x05a7;
/// The vendor-specific HID usage page that carries the DFU and external-flash reports.
/// On Linux/libusb the interfaces collapse into one device reported as usage page 0.
const BOSE_USAGE_PAGE: u16 = 0xff00;

/// QC35 II USB product IDs. Confirmed on hardware:
///   0x40fe  normal mode
///   0x4020  internal DFU bootmode (bose-dfu enter-dfu, writes the .dfu)
///   0x40fd  external bootmode (writes the .xuv, reached with enter-bootmode from normal mode)
const PID_NORMAL: u16 = 0x40fe;
const PID_EXTERNAL_BOOTMODE: u16 = 0x40fd;

#[derive(Parser, Debug)]
#[clap(version, about)]
enum Cmd {
    /// Parse a .xuv file and report its payload. Never touches a device.
    Parse {
        #[clap(parse(from_os_str))]
        file: PathBuf,
    },

    /// Enter external bootmode, report the PID the device presents, then exit. No erase.
    Probe {
        /// Only match this product ID (hex, unprefixed). Defaults to the single Bose device.
        #[clap(short, long, parse(try_from_str = parse_hex16))]
        pid: Option<u16>,
    },

    /// Send exit-bootmode (03 01 00) to return a device to normal mode. No erase.
    Reset {
        /// Match this product ID (hex, unprefixed), e.g. 40fd for external bootmode.
        #[clap(short, long, parse(try_from_str = parse_hex16))]
        pid: u16,
    },

    /// Flash the external-flash (SQIF) images. Destructive. Drives the full official sequence:
    /// enter external bootmode, format partition 0, then erase and write each target partition,
    /// then reset to normal.
    ///
    /// Formatting partition 0 wipes the WHOLE external flash, so both external images should be
    /// written together. Pass ext_signed.xuv (voice prompts, partition 1) and
    /// acorn_coeffs_signed.xuv (ANC coefficients, partition 3) from the same firmware version.
    Flash {
        /// ext_signed.xuv, written to SQIF partition 1 (voice prompts).
        #[clap(long, parse(from_os_str))]
        ext: Option<PathBuf>,

        /// acorn_coeffs_signed.xuv, written to SQIF partition 3 (ANC coefficients).
        #[clap(long, parse(from_os_str))]
        coeffs: Option<PathBuf>,

        /// Actually perform the format, erase, and writes. Without this, only a plan is printed.
        #[clap(long)]
        yes: bool,
    },
}

/// SQIF target partition numbers, from the index.xml TARGET attributes.
const TARGET_EXT: u8 = 1;
const TARGET_COEFFS: u8 = 3;

fn parse_hex16(src: &str) -> Result<u16, std::num::ParseIntError> {
    u16::from_str_radix(src.trim_start_matches("0x"), 16)
}

fn main() -> Result<()> {
    env_logger::Builder::from_env(
        env_logger::Env::new()
            .filter_or("BOSE_XUV_LOG", "info")
            .write_style("BOSE_XUV_LOG_STYLE"),
    )
    .format_timestamp(None)
    .init();

    match Cmd::parse() {
        Cmd::Parse { file } => parse_cmd(&file),
        Cmd::Probe { pid } => probe_cmd(pid),
        Cmd::Reset { pid } => reset_cmd(pid),
        Cmd::Flash { ext, coeffs, yes } => flash_cmd(ext, coeffs, yes),
    }
}

/// A snapshot of the Bose HID interfaces currently on the bus, by product ID.
fn bose_pids(api: &HidApi) -> Vec<u16> {
    let mut pids: Vec<u16> = api
        .device_list()
        .filter(|d| d.vendor_id() == BOSE_VID)
        .filter(|d| [0, BOSE_USAGE_PAGE].contains(&d.usage_page()))
        .map(|d| d.product_id())
        .collect();
    pids.sort_unstable();
    pids.dedup();
    pids
}

/// Find the one Bose vendor-HID interface matching an optional PID filter.
fn find_device<'a>(api: &'a HidApi, pid: Option<u16>) -> Result<&'a DeviceInfo> {
    let mut matches = api
        .device_list()
        .filter(|d| d.vendor_id() == BOSE_VID)
        .filter(|d| [0, BOSE_USAGE_PAGE].contains(&d.usage_page()))
        .filter(|d| pid.map_or(true, |p| d.product_id() == p));

    let first = matches.next().context("no matching Bose device found")?;
    if matches.next().is_some() {
        bail!("multiple Bose devices match; pass --pid to disambiguate");
    }
    Ok(first)
}

/// Poll the USB bus until a device with `want` product ID appears, or time out.
fn wait_for_pid(api: &mut HidApi, want: u16, timeout: Duration) -> Result<()> {
    let deadline = Instant::now() + timeout;
    loop {
        api.refresh_devices()?;
        if bose_pids(api).contains(&want) {
            return Ok(());
        }
        if Instant::now() > deadline {
            bail!("device did not present PID {want:04x} within {timeout:?}");
        }
        sleep(Duration::from_millis(300));
    }
}

fn parse_cmd(file: &std::path::Path) -> Result<()> {
    let f = std::fs::File::open(file).with_context(|| format!("opening {}", file.display()))?;
    let xuv = xuv::parse(BufReader::new(f))?;
    println!("File: {}", file.display());
    println!("Payload: {} bytes", xuv.payload.len());
    println!("Base word address: 0x{:06X}", xuv.base_word_addr);
    println!(
        "Magic: {} (verified)",
        std::str::from_utf8(xuv::MAGIC).unwrap()
    );
    println!("First 16 bytes: {:02x?}", &xuv.payload[..xuv.payload.len().min(16)]);
    Ok(())
}

/// Non-destructive reconnaissance: enter external bootmode, watch which PID appears, then exit.
/// This answers the one open question in XUV_FLASHING.md without erasing anything.
fn probe_cmd(pid: Option<u16>) -> Result<()> {
    let mut api = HidApi::new()?;
    let before = bose_pids(&api);
    println!("Bose PIDs before: {before:04x?}");

    let info = find_device(&api, pid)?;
    let start_pid = info.product_id();
    let dev = info.open_device(&api).context("opening device")?;
    println!("Opened 05a7:{start_pid:04x}; sending enter-external-bootmode (03 01 01)");

    match external::enter_bootmode(&dev) {
        Ok(resp) if resp.is_empty() => info!("no response (device likely reset immediately)"),
        Ok(resp) => info!("response: {:02x?}", &resp[..resp.len().min(16)]),
        Err(e) => warn!("enter-bootmode returned an error (may be normal on reset): {e:#}"),
    }
    drop(dev);

    // Watch for re-enumeration, skipping the transient empty state while the device resets.
    let deadline = Instant::now() + Duration::from_secs(12);
    let mut observed = before.clone();
    while Instant::now() < deadline {
        sleep(Duration::from_millis(300));
        api.refresh_devices()?;
        let now = bose_pids(&api);
        if !now.is_empty() && now != before {
            observed = now;
            break;
        }
    }
    println!("Bose PIDs after enter-bootmode: {observed:04x?}");

    let new_pids: Vec<u16> = observed.iter().copied().filter(|p| !before.contains(p)).collect();
    if new_pids.is_empty() {
        println!(
            "No PID change. The device ignored the command in this mode, or external bootmode \
             reuses the same PID. Either way, nothing was erased."
        );
    } else {
        println!("External bootmode PID(s): {new_pids:04x?}");
    }

    // Return to normal: send exit-bootmode to the new PID, then confirm.
    if let Some(&bm) = new_pids.first() {
        if let Ok(info) = find_device(&api, Some(bm)) {
            if let Ok(dev) = info.open_device(&api) {
                println!("Sending exit-bootmode (03 01 00) to 05a7:{bm:04x}");
                let _ = external::exit_bootmode(&dev);
            }
        }
    }
    match wait_for_pid(&mut api, PID_NORMAL, Duration::from_secs(12)) {
        Ok(()) => println!("Device returned to normal mode (05a7:{PID_NORMAL:04x})"),
        Err(e) => warn!("device did not return to normal mode: {e:#}"),
    }
    Ok(())
}

/// Send exit-bootmode to a device and report where it lands. No erase.
fn reset_cmd(pid: u16) -> Result<()> {
    let mut api = HidApi::new()?;
    println!("Bose PIDs before: {:04x?}", bose_pids(&api));
    let info = find_device(&api, Some(pid))?;
    let dev = info.open_device(&api).context("opening device")?;
    println!("Sending exit-bootmode (03 01 00) to 05a7:{pid:04x}");
    match external::exit_bootmode(&dev) {
        Ok(resp) if resp.is_empty() => info!("no response (device likely reset immediately)"),
        Ok(resp) => info!("response: {:02x?}", &resp[..resp.len().min(16)]),
        Err(e) => warn!("exit-bootmode returned an error (may be normal on reset): {e:#}"),
    }
    drop(dev);
    sleep(Duration::from_secs(3));
    api.refresh_devices()?;
    println!("Bose PIDs after: {:04x?}", bose_pids(&api));
    Ok(())
}

/// One external image to write: its target partition and reconstructed payload.
struct Image {
    partition: u8,
    label: &'static str,
    path: PathBuf,
    xuv: xuv::Xuv,
}

fn load_image(partition: u8, label: &'static str, path: PathBuf) -> Result<Image> {
    let f = std::fs::File::open(&path).with_context(|| format!("opening {}", path.display()))?;
    let xuv = xuv::parse(BufReader::new(f))?;
    Ok(Image { partition, label, path, xuv })
}

fn flash_cmd(ext: Option<PathBuf>, coeffs: Option<PathBuf>, yes: bool) -> Result<()> {
    // Parse whichever images were given, in target-partition order (ext = 1, then coeffs = 3),
    // matching the SUBID order in index.xml.
    let mut images = Vec::new();
    if let Some(p) = ext {
        images.push(load_image(TARGET_EXT, "voice prompts (ext_signed)", p)?);
    }
    if let Some(p) = coeffs {
        images.push(load_image(TARGET_COEFFS, "ANC coefficients (acorn_coeffs)", p)?);
    }
    if images.is_empty() {
        bail!("nothing to flash: pass --ext and/or --coeffs");
    }

    println!("Plan:");
    println!(
        "  sequence        normal (05a7:{PID_NORMAL:04x}) -> external bootmode \
         (05a7:{PID_EXTERNAL_BOOTMODE:04x})"
    );
    println!("  format          erase SQIF partition 0 (wipes the WHOLE external flash)");
    for img in &images {
        println!(
            "  write partition {}  {} bytes  {}  ({})",
            img.partition,
            img.xuv.payload.len(),
            img.path.display(),
            img.label
        );
    }
    println!("  finish          reset to normal (05a7:{PID_NORMAL:04x})");
    if images.len() < 2 {
        println!(
            "\nNOTE: formatting partition 0 wipes every external partition. You are writing only \
             {} of the two, so the other stays blank until it is written too.",
            images.len()
        );
    }

    if !yes {
        println!("\nDry run only. Re-run with --yes to format and write. This is destructive.");
        return Ok(());
    }

    let mut api = HidApi::new()?;

    // 1. Enter external bootmode from normal mode.
    {
        let info = find_device(&api, Some(PID_NORMAL))
            .context("device must be in normal mode (05a7:40fe) to start")?;
        let dev = info.open_device(&api).context("opening normal-mode device")?;
        info!("Entering external bootmode");
        let _ = external::enter_bootmode(&dev);
    }

    // 2. Wait for the external-bootmode PID to appear, then open it.
    wait_for_pid(&mut api, PID_EXTERNAL_BOOTMODE, Duration::from_secs(15))
        .context("device did not enter external bootmode")?;
    let dev = {
        let info = find_device(&api, Some(PID_EXTERNAL_BOOTMODE))?;
        info.open_device(&api).context("opening external-bootmode device")?
    };

    // 3. Format partition 0, then erase and write each target partition.
    //
    // This mirrors the official helper's doUpdate state machine (see XUV_FLASH_FAILURE.md):
    // doUpdateFormatPartitions() calls EraseSqif(0) ("Formatting ext"), then doUpdateFlashFile()
    // erases and writes each target. A target erase is refused (status 0x00) until partition 0
    // is formatted, which is exactly what the first live attempt hit. Formatting partition 0
    // wipes the whole external flash, so every image passed is rewritten after it.
    warn!("Formatting external flash (erase partition 0); ALL external partitions are wiped now");
    external::erase_sqif(&dev, 0)?;
    for img in &images {
        warn!("Erasing SQIF partition {} ({})", img.partition, img.label);
        external::erase_sqif(&dev, img.partition)?;
        info!("Writing {} bytes to partition {}", img.xuv.payload.len(), img.partition);
        external::write_sqif(&dev, &img.xuv.payload)?;
    }
    info!("All writes complete");

    // 4. Reset back to normal.
    let _ = external::exit_bootmode(&dev);
    drop(dev);
    match wait_for_pid(&mut api, PID_NORMAL, Duration::from_secs(15)) {
        Ok(()) => println!("Done. Device is back in normal mode (05a7:{PID_NORMAL:04x})."),
        Err(e) => warn!("write finished but device did not return to normal mode: {e:#}"),
    }
    Ok(())
}
