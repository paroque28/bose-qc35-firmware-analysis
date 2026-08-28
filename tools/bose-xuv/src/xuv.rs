//! Parser for the CSR/Qualcomm `.xuv` flash-image text format.
//!
//! A `.xuv` file is plain ASCII, one 16-bit word per line, written as `@ADDRESS  HHHH`
//! with CRLF line endings. ADDRESS is a word address, so the byte offset is twice its value.
//! Each 16-bit word is stored byte-swapped relative to the payload, so the payload byte order
//! is little-endian: the word text `7366` becomes the bytes 0x66 0x73, which is `fs`, the
//! start of the `fsr_dfu1` magic every signed image begins with.
//!
//! This mirrors `tools/xuv.py` and adds the payload reconstruction that the flasher needs.

use anyhow::{bail, Context, Result};
use std::io::BufRead;

/// The magic every DFU-signed `.xuv` payload begins with, once reconstructed.
pub const MAGIC: &[u8; 8] = b"fsr_dfu1";

/// A parsed `.xuv` file: the reconstructed raw payload ready to stream to the device.
pub struct Xuv {
    /// The payload bytes, in address order, little-endian per word. Starts with [`MAGIC`].
    pub payload: Vec<u8>,
    /// The word address the payload starts at (0 for every real Bose image seen so far).
    pub base_word_addr: u32,
}

/// Parse a `.xuv` file into its raw payload.
///
/// Fails if the address range has a gap, because the flasher streams the payload as one
/// contiguous run and a gap would silently misalign every following word.
pub fn parse(reader: impl BufRead) -> Result<Xuv> {
    let mut words: Vec<(u32, u16)> = Vec::new();

    for (idx, line) in reader.lines().enumerate() {
        let lineno = idx + 1;
        let line = line.with_context(|| format!("reading line {lineno}"))?;
        let text = line.trim();
        if text.is_empty() {
            continue;
        }
        if !text.starts_with('@') {
            bail!("line {lineno}: expected a line starting with '@', got {text:?}");
        }
        let mut fields = text[1..].split_whitespace();
        let addr_str = fields
            .next()
            .with_context(|| format!("line {lineno}: missing address"))?;
        let val_str = fields
            .next()
            .with_context(|| format!("line {lineno}: missing value"))?;
        let addr = u32::from_str_radix(addr_str, 16)
            .with_context(|| format!("line {lineno}: bad address {addr_str:?}"))?;
        let val = u16::from_str_radix(val_str, 16)
            .with_context(|| format!("line {lineno}: bad value {val_str:?}"))?;
        words.push((addr, val));
    }

    if words.is_empty() {
        bail!("no `@ADDR HHHH` lines found; is this a .xuv file?");
    }

    words.sort_by_key(|(addr, _)| *addr);

    let base = words[0].0;
    for (i, (addr, _)) in words.iter().enumerate() {
        let expected = base + i as u32;
        if *addr != expected {
            bail!(
                "address gap: expected word 0x{expected:06X} but found 0x{addr:06X}; \
                 the flasher cannot stream a non-contiguous image"
            );
        }
    }

    let mut payload = Vec::with_capacity(words.len() * 2);
    for (_, val) in &words {
        payload.extend_from_slice(&val.to_le_bytes());
    }

    if !payload.starts_with(MAGIC) {
        bail!(
            "reconstructed payload does not start with the `fsr_dfu1` magic; \
             this is not a signed .xuv image (first bytes: {:02x?})",
            &payload[..payload.len().min(8)]
        );
    }

    Ok(Xuv {
        payload,
        base_word_addr: base,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn reconstructs_magic() {
        // First four words of a real acorn_coeffs image.
        let text = "@000000   7366\r\n@000001   5F72\r\n@000002   6664\r\n@000003   3175\r\n";
        let xuv = parse(Cursor::new(text)).unwrap();
        assert_eq!(&xuv.payload[..8], MAGIC);
        assert_eq!(xuv.base_word_addr, 0);
    }

    #[test]
    fn rejects_gap() {
        let text = "@000000   7366\r\n@000002   6664\r\n";
        assert!(parse(Cursor::new(text)).is_err());
    }

    #[test]
    fn rejects_non_magic() {
        let text = "@000000   0000\r\n@000001   0000\r\n@000002   0000\r\n@000003   0000\r\n";
        assert!(parse(Cursor::new(text)).is_err());
    }
}
