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

    /// Erase a SQIF partition and write a .xuv into it. Destructive.
    Flash {
        #[clap(parse(from_os_str))]
        file: PathBuf,

        /// SQIF partition number (index.xml TARGET: 1 = voice prompts, 3 = ANC coefficients).
        #[clap(short = 't', long)]
        partition: u8,

        /// Match this product ID (hex, unprefixed). Required, so the mode is chosen deliberately.
        #[clap(short, long, parse(try_from_str = parse_hex16))]
        pid: u16,

        /// Actually perform the erase and write. Without this, the command only prints a plan.
        #[clap(long)]
        yes: bool,
    },
}

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
        Cmd::Flash {
            file,
            partition,
            pid,
            yes,
        } => flash_cmd(&file, partition, pid, yes),
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

    // Watch for re-enumeration for up to 12 seconds.
    let deadline = Instant::now() + Duration::from_secs(12);
    let mut observed = before.clone();
    while Instant::now() < deadline {
        sleep(Duration::from_millis(500));
        api.refresh_devices()?;
        let now = bose_pids(&api);
        if now != before {
            observed = now;
            break;
        }
        observed = now;
    }
    println!("Bose PIDs after enter-bootmode: {observed:04x?}");

    let new_pids: Vec<u16> = observed.iter().copied().filter(|p| !before.contains(p)).collect();
    if new_pids.is_empty() && observed == before {
        println!(
            "No PID change. The device ignored the command in this mode, or the external \
             bootmode reuses the same PID. Either way, nothing was erased."
        );
    } else {
        println!("External bootmode PID(s): {new_pids:04x?}");
    }

    // Best-effort return to normal: send exit to whatever we can now open.
    api.refresh_devices()?;
    if let Ok(info) = find_device(&api, None) {
        if let Ok(dev) = info.open_device(&api) {
            println!("Sending exit-bootmode (03 01 00) to 05a7:{:04x}", info.product_id());
            let _ = external::exit_bootmode(&dev);
        }
    }

    // Report where the device ended up.
    sleep(Duration::from_secs(2));
    api.refresh_devices()?;
    println!("Bose PIDs after exit: {:04x?}", bose_pids(&api));
    Ok(())
}

fn flash_cmd(file: &std::path::Path, partition: u8, pid: u16, yes: bool) -> Result<()> {
    let f = std::fs::File::open(file).with_context(|| format!("opening {}", file.display()))?;
    let xuv = xuv::parse(BufReader::new(f))?;

    println!("Plan:");
    println!("  file          {}", file.display());
    println!("  payload       {} bytes", xuv.payload.len());
    println!("  device        05a7:{pid:04x}");
    println!("  SQIF partition {partition}  (ERASE, then WRITE)");

    if !yes {
        println!("\nDry run only. Re-run with --yes to erase and write. This is destructive.");
        return Ok(());
    }

    let api = HidApi::new()?;
    let info = find_device(&api, Some(pid))?;
    let dev = info.open_device(&api).context("opening device")?;

    warn!("Erasing SQIF partition {partition}; contents are lost now");
    external::erase_sqif(&dev, partition)?;
    info!("Erase acknowledged; writing {} bytes", xuv.payload.len());
    external::write_sqif(&dev, &xuv.payload)?;
    info!("Write complete; resetting device");
    let _ = external::exit_bootmode(&dev);
    println!("Done.");
    Ok(())
}
