//! NDS cartridge header and file system (FNT name table, FAT allocation table).

use alloc::vec;
use alloc::vec::Vec;

use crate::{Error, Result, Source, u16_at, u32_at};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Game {
    Diamond,
    Pearl,
    Platinum,
    HeartGold,
    SoulSilver,
    Black,
    White,
    Black2,
    White2,
}

impl Game {
    /// The game from the first three letters of the header's game code (the fourth is the region).
    pub fn from_code(code: [u8; 4]) -> Option<Game> {
        Some(match &code[..3] {
            b"ADA" => Game::Diamond,
            b"APA" => Game::Pearl,
            b"CPU" => Game::Platinum,
            b"IPK" => Game::HeartGold,
            b"IPG" => Game::SoulSilver,
            b"IRB" => Game::Black,
            b"IRA" => Game::White,
            b"IRE" => Game::Black2,
            b"IRD" => Game::White2,
            _ => return None,
        })
    }

    pub fn generation(self) -> u8 {
        match self {
            Game::Diamond | Game::Pearl | Game::Platinum | Game::HeartGold | Game::SoulSilver => 4,
            _ => 5,
        }
    }
}

/// A file's byte range in the ROM image.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileRange {
    pub start: u32,
    pub end: u32,
}

impl FileRange {
    pub fn len(self) -> u32 {
        self.end - self.start
    }

    pub fn is_empty(self) -> bool {
        self.end == self.start
    }
}

/// Largest name table accepted; retail Gen 4/5 tables are tens of KB.
const MAX_FNT: u32 = 1 << 20;

/// Reads the 4-letter game code without touching the file system (for scanning an SD card).
pub fn game_code<S: Source>(src: &mut S) -> Result<[u8; 4]> {
    let mut code = [0; 4];
    src.read_at(0x0C, &mut code)?;
    Ok(code)
}

pub struct Rom<S> {
    src: S,
    code: [u8; 4],
    fnt: Vec<u8>,
    fat_offset: u32,
    file_count: u32,
}

impl<S: Source> Rom<S> {
    pub fn open(mut src: S) -> Result<Self> {
        let mut h = [0u8; 0x50];
        src.read_at(0, &mut h)?;
        let code = [h[0x0C], h[0x0D], h[0x0E], h[0x0F]];
        let fnt_offset = u32_at(&h, 0x40)?;
        let fnt_len = u32_at(&h, 0x44)?;
        let fat_offset = u32_at(&h, 0x48)?;
        let fat_len = u32_at(&h, 0x4C)?;
        if !(8..=MAX_FNT).contains(&fnt_len) || fat_len % 8 != 0 {
            return Err(Error::Format("NDS header"));
        }
        let mut fnt = vec![0; fnt_len as usize];
        src.read_at(fnt_offset, &mut fnt)?;
        Ok(Rom {
            src,
            code,
            fnt,
            fat_offset,
            file_count: fat_len / 8,
        })
    }

    pub fn game_code(&self) -> [u8; 4] {
        self.code
    }

    pub fn game(&self) -> Option<Game> {
        Game::from_code(self.code)
    }

    pub fn source(&mut self) -> &mut S {
        &mut self.src
    }

    pub fn file_by_id(&mut self, id: u16) -> Result<FileRange> {
        if u32::from(id) >= self.file_count {
            return Err(Error::NotFound);
        }
        let mut e = [0u8; 8];
        self.src
            .read_at(self.fat_offset + u32::from(id) * 8, &mut e)?;
        let r = FileRange {
            start: u32_at(&e, 0)?,
            end: u32_at(&e, 4)?,
        };
        if r.end < r.start {
            return Err(Error::Format("FAT entry"));
        }
        Ok(r)
    }

    /// The file at a slash-separated path such as `a/0/2/3` or `graphic/pl_font.narc`.
    pub fn find(&mut self, path: &str) -> Result<FileRange> {
        let id = self.file_id(path)?;
        self.file_by_id(id)
    }

    fn file_id(&self, path: &str) -> Result<u16> {
        let mut dir = 0u16; // root (0xF000 & 0xFFF)
        let mut parts = path.split('/').peekable();
        while let Some(name) = parts.next() {
            let last = parts.peek().is_none();
            let entry = 8 * usize::from(dir);
            let mut p = u32_at(&self.fnt, entry)? as usize;
            let mut file = u16_at(&self.fnt, entry + 4)?;
            let mut found = None;
            loop {
                let t = *self.fnt.get(p).ok_or(Error::Format("FNT"))?;
                p += 1;
                if t == 0 {
                    break;
                }
                let len = usize::from(t & 0x7F);
                let n = self.fnt.get(p..p + len).ok_or(Error::Format("FNT"))?;
                p += len;
                if t & 0x80 != 0 {
                    let sub = u16_at(&self.fnt, p)?;
                    p += 2;
                    if !last && n == name.as_bytes() {
                        found = Some(sub & 0x0FFF);
                        break;
                    }
                } else {
                    if last && n == name.as_bytes() {
                        return Ok(file);
                    }
                    file += 1;
                }
            }
            dir = found.ok_or(Error::NotFound)?;
            if 8 * usize::from(dir) >= self.fnt.len() {
                return Err(Error::Format("FNT"));
            }
        }
        Err(Error::NotFound)
    }

    /// The ARM9 binary, BLZ-decompressed when its footer says it is compressed (HeartGold and the
    /// Gen 5 games; Platinum's is plain).
    pub fn arm9(&mut self) -> Result<Vec<u8>> {
        let mut h = [0u8; 0x30];
        self.src.read_at(0, &mut h)?;
        let (offset, size) = (u32_at(&h, 0x20)?, u32_at(&h, 0x2C)?);
        if size > 4 << 20 {
            return Err(Error::Format("ARM9 size"));
        }
        let raw = self.read(FileRange {
            start: offset,
            end: offset + size,
        })?;
        Ok(crate::lz::blz_decompress(&raw).unwrap_or(raw))
    }

    /// ARM9 overlay `id`, BLZ-decompressed when its table entry says so. The overlay table (header
    /// 0x50, 32 bytes per entry) gives each overlay's file ID at 0x18 and, at 0x1C, its compressed
    /// size in the low 24 bits and the compressed flag in bit 24.
    pub fn overlay(&mut self, id: u32) -> Result<Vec<u8>> {
        let mut h = [0u8; 8];
        self.src.read_at(0x50, &mut h)?;
        let (table, size) = (u32_at(&h, 0)?, u32_at(&h, 4)?);
        if id >= size / 32 {
            return Err(Error::NotFound);
        }
        let mut e = [0u8; 32];
        self.src.read_at(table + 32 * id, &mut e)?;
        let file = u16::try_from(u32_at(&e, 0x18)?).map_err(|_| Error::Format("overlay"))?;
        let r = self.file_by_id(file)?;
        if r.len() > 4 << 20 {
            return Err(Error::Format("overlay size"));
        }
        let mut raw = self.read(r)?;
        let flags = u32_at(&e, 0x1C)?;
        if flags & (1 << 24) == 0 {
            return Ok(raw);
        }
        // The file may be padded past the compressed size.
        let packed = (flags & 0xFF_FFFF) as usize;
        if packed > 0 && packed <= raw.len() {
            raw.truncate(packed);
        }
        crate::lz::blz_decompress(&raw)
    }

    pub fn read(&mut self, r: FileRange) -> Result<Vec<u8>> {
        let mut out = vec![0; r.len() as usize];
        self.src.read_at(r.start, &mut out)?;
        Ok(out)
    }
}
