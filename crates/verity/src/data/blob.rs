//! Data blob container. Little-endian, read zero-copy from a byte slice.
//!
//! ```text
//! header (16): magic "VRTY", u16 version, u8 generation, u8 kind, u16 table_count,
//!              u16 reserved, u32 total_len
//! directory:   table_count x { u16 id, u16 elem_size (0 = variable), u32 offset, u32 len }
//! tables:      raw bytes at the given offsets
//! ```

pub const MAGIC: [u8; 4] = *b"VRTY";
pub const VERSION: u16 = 1;
pub const HEADER_LEN: usize = 16;
pub const DIR_ENTRY_LEN: usize = 12;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum BlobKind {
    Game = 1,
    Moves = 2,
    Strings = 3,
    Encounters = 4,
    Gifts = 5,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlobError {
    TooShort,
    BadMagic,
    BadVersion,
    WrongKind,
    BadDirectory,
}

#[derive(Clone, Copy)]
pub struct Blob<'a> {
    data: &'a [u8],
    generation: u8,
    kind: u8,
    tables: usize,
}

impl<'a> Blob<'a> {
    pub fn parse(data: &'a [u8]) -> Result<Self, BlobError> {
        if data.len() < HEADER_LEN {
            return Err(BlobError::TooShort);
        }
        if data[..4] != MAGIC {
            return Err(BlobError::BadMagic);
        }
        if u16::from_le_bytes([data[4], data[5]]) != VERSION {
            return Err(BlobError::BadVersion);
        }
        let tables = u16::from_le_bytes([data[8], data[9]]) as usize;
        let total = u32::from_le_bytes([data[12], data[13], data[14], data[15]]) as usize;
        if total != data.len() || HEADER_LEN + tables * DIR_ENTRY_LEN > data.len() {
            return Err(BlobError::BadDirectory);
        }
        let blob = Self {
            data,
            generation: data[6],
            kind: data[7],
            tables,
        };
        for i in 0..tables {
            let (_, _, ofs, len) = blob.entry(i);
            if ofs.checked_add(len).is_none_or(|end| end > data.len()) {
                return Err(BlobError::BadDirectory);
            }
        }
        Ok(blob)
    }

    pub fn expect(data: &'a [u8], kind: BlobKind) -> Result<Self, BlobError> {
        let b = Self::parse(data)?;
        if b.kind != kind as u8 {
            return Err(BlobError::WrongKind);
        }
        Ok(b)
    }

    pub fn generation(&self) -> u8 {
        self.generation
    }

    fn entry(&self, i: usize) -> (u16, u16, usize, usize) {
        let e = &self.data[HEADER_LEN + i * DIR_ENTRY_LEN..];
        (
            u16::from_le_bytes([e[0], e[1]]),
            u16::from_le_bytes([e[2], e[3]]),
            u32::from_le_bytes([e[4], e[5], e[6], e[7]]) as usize,
            u32::from_le_bytes([e[8], e[9], e[10], e[11]]) as usize,
        )
    }

    /// Table bytes and element size.
    pub fn table(&self, id: u16) -> Option<(&'a [u8], usize)> {
        (0..self.tables).find_map(|i| {
            let (tid, elem, ofs, len) = self.entry(i);
            (tid == id).then(|| (&self.data[ofs..ofs + len], elem as usize))
        })
    }

    pub fn bytes(&self, id: u16) -> &'a [u8] {
        self.table(id).map_or(&[], |(t, _)| t)
    }
}

/// A table of little-endian u16 values.
#[derive(Clone, Copy)]
pub struct U16s<'a>(pub &'a [u8]);

impl<'a> U16s<'a> {
    pub fn len(&self) -> usize {
        self.0.len() / 2
    }

    pub fn is_empty(&self) -> bool {
        self.0.len() < 2
    }

    pub fn get(&self, i: usize) -> Option<u16> {
        let b = self.0.get(2 * i..2 * i + 2)?;
        Some(u16::from_le_bytes([b[0], b[1]]))
    }

    pub fn iter(&self) -> impl Iterator<Item = u16> + 'a {
        let d = self.0;
        d.as_chunks::<2>().0.iter().map(|c| u16::from_le_bytes(*c))
    }

    pub fn contains(&self, v: u16) -> bool {
        self.iter().any(|x| x == v)
    }
}

/// Variable-length records: `count + 1` u32 offsets followed by the record data.
#[derive(Clone, Copy)]
pub struct Indexed<'a>(pub &'a [u8]);

impl<'a> Indexed<'a> {
    pub fn count(&self) -> usize {
        let n = self.0.len() / 4;
        if n == 0 {
            return 0;
        }
        (u32::from_le_bytes([self.0[0], self.0[1], self.0[2], self.0[3]]) as usize / 4)
            .saturating_sub(1)
    }

    pub fn get(&self, i: usize) -> &'a [u8] {
        if i >= self.count() {
            return &[];
        }
        let r = |k: usize| {
            let b = &self.0[4 * k..4 * k + 4];
            u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize
        };
        let (start, end) = (r(i), r(i + 1));
        if start > end || end > self.0.len() {
            return &[];
        }
        &self.0[start..end]
    }
}
