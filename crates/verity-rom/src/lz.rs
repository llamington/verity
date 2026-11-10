//! Nintendo LZ77 decompression: type 0x10 (LZ10, the BIOS format) and 0x11 (LZ11, longer runs).

use alloc::vec::Vec;

use crate::{Error, Result};

/// Largest output accepted (Gen 4/5 graphics decompress to tens of KB).
pub const MAX_OUT: usize = 1 << 22;

/// Decompresses an LZ10 or LZ11 stream (the type is the first byte).
pub fn decompress(src: &[u8]) -> Result<Vec<u8>> {
    let bad = Error::Format("LZ");
    if src.len() < 4 {
        return Err(bad);
    }
    let kind = src[0];
    let mut size = (src[1] as usize) | (src[2] as usize) << 8 | (src[3] as usize) << 16;
    let mut p = 4;
    if size == 0 && kind == 0x11 {
        let b = src.get(4..8).ok_or(bad)?;
        size = u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize;
        p = 8;
    }
    if !matches!(kind, 0x10 | 0x11) || size > MAX_OUT {
        return Err(bad);
    }
    let mut out = Vec::with_capacity(size);
    let next = |p: &mut usize| -> Result<usize> {
        let b = *src.get(*p).ok_or(bad)?;
        *p += 1;
        Ok(b as usize)
    };
    while out.len() < size {
        let flags = next(&mut p)?;
        for bit in (0..8).rev() {
            if out.len() >= size {
                break;
            }
            if flags & (1 << bit) == 0 {
                out.push(next(&mut p)? as u8);
                continue;
            }
            let (len, disp);
            let a = next(&mut p)?;
            if kind == 0x10 {
                let b = next(&mut p)?;
                len = (a >> 4) + 3;
                disp = ((a & 0xF) << 8 | b) + 1;
            } else {
                match a >> 4 {
                    0 => {
                        let (b, c) = (next(&mut p)?, next(&mut p)?);
                        len = ((a & 0xF) << 4 | b >> 4) + 0x11;
                        disp = ((b & 0xF) << 8 | c) + 1;
                    }
                    1 => {
                        let (b, c, d) = (next(&mut p)?, next(&mut p)?, next(&mut p)?);
                        len = ((a & 0xF) << 12 | b << 4 | c >> 4) + 0x111;
                        disp = ((c & 0xF) << 8 | d) + 1;
                    }
                    n => {
                        let b = next(&mut p)?;
                        len = n + 1;
                        disp = ((a & 0xF) << 8 | b) + 1;
                    }
                }
            }
            if disp > out.len() {
                return Err(bad);
            }
            for _ in 0..len.min(size - out.len()) {
                out.push(out[out.len() - disp]);
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lz10_literals_and_overlapping_copy() {
        // "ab" then copy len 6 disp 2 -> "abababab"
        let s = [0x10, 8, 0, 0, 0b0010_0000, b'a', b'b', 0x30, 0x01];
        assert_eq!(decompress(&s).unwrap(), b"abababab");
    }

    #[test]
    fn lz11_all_length_forms() {
        // 'x', then short copy (len 3 disp 1), then 0x11-form (len 0x11) and 0x111-form (len 0x111).
        let s = [
            0x11,
            0x26,
            0x01,
            0, // size 1 + 3 + 0x11 + 0x111 = 0x126
            0b0111_0000,
            b'x',
            0x20,
            0x00, // len 3 disp 1
            0x00,
            0x00,
            0x00, // len 0x11 disp 1
            0x10,
            0x00,
            0x00,
            0x00, // len 0x111 disp 1
        ];
        let out = decompress(&s).unwrap();
        assert_eq!(out.len(), 0x126);
        assert!(out.iter().all(|&b| b == b'x'));
    }

    #[test]
    fn rejects_bad_streams() {
        assert!(decompress(&[0x10, 4, 0, 0, 0x80, 0x00, 0x00]).is_err()); // copy before data
        assert!(decompress(&[0x10, 4, 0, 0, 0x00, b'a']).is_err()); // truncated
        assert!(decompress(&[0x40, 1, 0, 0, 0, 0]).is_err()); // unknown type
        assert!(decompress(&[0x10, 0xFF, 0xFF, 0xFF]).is_err()); // too large
    }
}
