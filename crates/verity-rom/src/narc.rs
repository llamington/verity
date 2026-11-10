//! NARC archives (BTAF/BTNF/GMIF). Only the allocation table is read up front, so members of a
//! large archive (sprites) can be read one at a time.

use alloc::vec::Vec;

use crate::nds::FileRange;
use crate::{u16_at, u32_at, Error, Result, Source};

pub struct Narc {
    members: Vec<FileRange>,
}

/// Retail archives hold at most a few thousand members.
const MAX_MEMBERS: usize = 1 << 16;

impl Narc {
    /// Reads the archive header of the NARC stored at `file` in `src`.
    pub fn open<S: Source>(src: &mut S, file: FileRange) -> Result<Narc> {
        let mut h = [0u8; 0x1C];
        if file.len() < h.len() as u32 {
            return Err(Error::Format("NARC"));
        }
        src.read_at(file.start, &mut h)?;
        if &h[..4] != b"NARC" {
            return Err(Error::Format("NARC"));
        }
        let fatb = u32::from(u16_at(&h, 0x0C)?);
        if h.get(fatb as usize..fatb as usize + 12)
            .is_none_or(|t| &t[..4] != b"BTAF")
        {
            return Err(Error::Format("NARC BTAF"));
        }
        let fatb_len = u32_at(&h, fatb as usize + 4)?;
        let count = usize::from(u16_at(&h, fatb as usize + 8)?);
        if count > MAX_MEMBERS || fatb_len < 12 + 8 * count as u32 {
            return Err(Error::Format("NARC BTAF"));
        }
        let mut table = alloc::vec![0u8; 8 * count];
        src.read_at(file.start + fatb + 12, &mut table)?;

        let fntb = fatb + fatb_len;
        let mut s = [0u8; 8];
        src.read_at(file.start + fntb, &mut s)?;
        if &s[..4] != b"BTNF" {
            return Err(Error::Format("NARC BTNF"));
        }
        let fimg = fntb + u32_at(&s, 4)?;
        src.read_at(file.start + fimg, &mut s)?;
        if &s[..4] != b"GMIF" {
            return Err(Error::Format("NARC GMIF"));
        }
        let base = file.start + fimg + 8;
        let image_end = file.start + fimg + u32_at(&s, 4)?;
        if image_end > file.end {
            return Err(Error::Format("NARC GMIF"));
        }

        let mut members = Vec::with_capacity(count);
        for i in 0..count {
            let (a, b) = (u32_at(&table, 8 * i)?, u32_at(&table, 8 * i + 4)?);
            let r = FileRange {
                start: base.checked_add(a).ok_or(Error::Format("NARC"))?,
                end: base.checked_add(b).ok_or(Error::Format("NARC"))?,
            };
            if r.end < r.start || r.end > image_end {
                return Err(Error::Format("NARC member"));
            }
            members.push(r);
        }
        Ok(Narc { members })
    }

    pub fn len(&self) -> usize {
        self.members.len()
    }

    pub fn is_empty(&self) -> bool {
        self.members.is_empty()
    }

    pub fn member(&self, i: usize) -> Result<FileRange> {
        self.members.get(i).copied().ok_or(Error::NotFound)
    }

    pub fn read_member<S: Source>(&self, src: &mut S, i: usize) -> Result<Vec<u8>> {
        let r = self.member(i)?;
        let mut out = alloc::vec![0; r.len() as usize];
        src.read_at(r.start, &mut out)?;
        Ok(out)
    }
}
