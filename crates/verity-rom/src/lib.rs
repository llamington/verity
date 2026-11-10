//! Reads what verity needs from the user's own Gen 4/5 ROM: the NDS file system, NARC archives,
//! LZ-compressed data and the games' fonts. Nothing from a ROM is ever shipped with verity; the DS
//! build extracts it on the console from a ROM the user owns. Formats: `docs/research/rom-font.md`.
//!
//! The ROM is read through [`Source`] so a 512 MB image never has to fit in memory.
#![no_std]

extern crate alloc;

#[cfg(test)]
extern crate std;

pub mod font;
pub mod icons;
pub mod items;
pub mod lz;
pub mod moves;
pub mod narc;
pub mod nds;
pub mod text;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// The source could not be read (or a read ran past its end).
    Io,
    /// The data is not in the expected format.
    Format(&'static str),
    /// A path, member or game code that does not exist.
    NotFound,
}

pub type Result<T> = core::result::Result<T, Error>;

/// Random-access reads from a ROM image (an SD card file on the DS, a slice in tests).
pub trait Source {
    fn read_at(&mut self, offset: u32, buf: &mut [u8]) -> Result<()>;
}

impl Source for &[u8] {
    fn read_at(&mut self, offset: u32, buf: &mut [u8]) -> Result<()> {
        let start = offset as usize;
        let src = self.get(start..start + buf.len()).ok_or(Error::Io)?;
        buf.copy_from_slice(src);
        Ok(())
    }
}

pub(crate) fn u16_at(b: &[u8], o: usize) -> Result<u16> {
    b.get(o..o + 2)
        .map(|s| u16::from_le_bytes([s[0], s[1]]))
        .ok_or(Error::Format("truncated"))
}

pub(crate) fn u32_at(b: &[u8], o: usize) -> Result<u32> {
    b.get(o..o + 4)
        .map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
        .ok_or(Error::Format("truncated"))
}
