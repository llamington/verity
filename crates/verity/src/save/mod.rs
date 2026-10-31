pub mod crc16;
pub mod detect;
pub mod gen4;
pub mod gen5;
pub mod gen5_blocks;

pub use crc16::crc16_ccitt;
pub use detect::{detect, SaveKind};

use crate::pkm::{SIZE_4PARTY, SIZE_4STORED, SIZE_5PARTY, SIZE_5STORED};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaveError {
    Unrecognized,
    OutOfRange,
    BadLength,
}

pub const BOX_SLOTS: usize = 30;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slot {
    Party(u8),
    Box(u8, u8),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TrainerIds {
    pub tid: u16,
    pub sid: u16,
}

/// A save file over a caller-owned buffer. Edits are made in place; call
/// [`Save::finalize`] before writing the buffer back.
pub struct Save<'a> {
    data: &'a mut [u8],
    kind: SaveKind,
    layout: Layout,
}

enum Layout {
    Gen4(gen4::Layout4),
    Gen5(gen5::Layout5),
}

impl<'a> Save<'a> {
    /// Accepts a raw 512 KB save, or a DeSmuME `.dsv` (raw save plus a 0x7A-byte footer, kept
    /// unchanged; PKHeX `SaveHandlerDeSmuME`).
    pub fn open(data: &'a mut [u8]) -> Result<Self, SaveError> {
        let raw_len = raw_save_len(data);
        let kind = detect(&data[..raw_len]).ok_or(SaveError::Unrecognized)?;
        let layout = match kind {
            SaveKind::DP | SaveKind::Pt | SaveKind::HGSS => {
                Layout::Gen4(gen4::Layout4::new(kind, data))
            }
            SaveKind::BW | SaveKind::B2W2 => Layout::Gen5(gen5::Layout5::new(kind)),
        };
        Ok(Self { data, kind, layout })
    }

    pub fn kind(&self) -> SaveKind {
        self.kind
    }

    pub fn generation(&self) -> u8 {
        self.kind.generation()
    }

    pub fn box_count(&self) -> usize {
        match self.layout {
            Layout::Gen4(_) => gen4::BOX_COUNT,
            Layout::Gen5(_) => gen5::BOX_COUNT,
        }
    }

    pub fn stored_size(&self) -> usize {
        match self.layout {
            Layout::Gen4(_) => SIZE_4STORED,
            Layout::Gen5(_) => SIZE_5STORED,
        }
    }

    pub fn party_size(&self) -> usize {
        match self.layout {
            Layout::Gen4(_) => SIZE_4PARTY,
            Layout::Gen5(_) => SIZE_5PARTY,
        }
    }

    pub fn party_count(&self) -> usize {
        let ofs = match &self.layout {
            Layout::Gen4(l) => l.party_count_offset(),
            Layout::Gen5(l) => l.party_count_offset(),
        };
        self.data[ofs] as usize
    }

    pub fn trainer_ids(&self) -> TrainerIds {
        let ofs = match &self.layout {
            Layout::Gen4(l) => l.tid_offset(),
            Layout::Gen5(l) => l.tid_offset(),
        };
        TrainerIds {
            tid: read_u16(self.data, ofs),
            sid: read_u16(self.data, ofs + 2),
        }
    }

    /// Raw OT name as stored (Gen 4 charset or UTF-16), 8 code units.
    pub fn trainer_name_raw(&self) -> [u16; 8] {
        let ofs = match &self.layout {
            Layout::Gen4(l) => l.ot_offset(),
            Layout::Gen5(l) => l.ot_offset(),
        };
        core::array::from_fn(|i| read_u16(self.data, ofs + 2 * i))
    }

    fn slot_range(&self, slot: Slot) -> Result<(usize, usize), SaveError> {
        match slot {
            Slot::Party(i) if (i as usize) < 6 => {
                let ofs = match &self.layout {
                    Layout::Gen4(l) => l.party_offset(i as usize),
                    Layout::Gen5(l) => l.party_offset(i as usize),
                };
                Ok((ofs, self.party_size()))
            }
            Slot::Box(b, s) if (b as usize) < self.box_count() && (s as usize) < BOX_SLOTS => {
                let ofs = match &self.layout {
                    Layout::Gen4(l) => l.box_slot_offset(b as usize, s as usize),
                    Layout::Gen5(l) => l.box_slot_offset(b as usize, s as usize),
                };
                Ok((ofs, self.stored_size()))
            }
            _ => Err(SaveError::OutOfRange),
        }
    }

    /// Encrypted slot bytes as stored in the save.
    pub fn slot_raw(&self, slot: Slot) -> Result<&[u8], SaveError> {
        let (ofs, len) = self.slot_range(slot)?;
        Ok(&self.data[ofs..ofs + len])
    }

    pub fn set_slot_raw(&mut self, slot: Slot, encrypted: &[u8]) -> Result<(), SaveError> {
        let (ofs, len) = self.slot_range(slot)?;
        if encrypted.len() != len {
            return Err(SaveError::BadLength);
        }
        self.data[ofs..ofs + len].copy_from_slice(encrypted);
        Ok(())
    }

    pub fn checksums_valid(&self) -> bool {
        match &self.layout {
            Layout::Gen4(l) => l.checksums_valid(self.data),
            Layout::Gen5(l) => l.checksums_valid(self.data),
        }
    }

    /// Applies PKHeX's export fix-ups (`GetFinalData`) and recomputes every checksum.
    pub fn finalize(&mut self) {
        match &self.layout {
            Layout::Gen4(l) => {
                l.prepare_export(self.data);
                l.set_checksums(self.data);
            }
            Layout::Gen5(l) => l.set_checksums(self.data),
        }
    }

    pub fn data(&self) -> &[u8] {
        self.data
    }
}

const DESMUME_FOOTER: usize = 0x7A;
const DESMUME_MAGIC: &[u8] = b"|-DESMUME SAVE-|";

fn raw_save_len(data: &[u8]) -> usize {
    if data.len() == detect::SIZE_G4RAW + DESMUME_FOOTER && data.ends_with(DESMUME_MAGIC) {
        detect::SIZE_G4RAW
    } else {
        data.len()
    }
}

pub(crate) fn read_u16(data: &[u8], ofs: usize) -> u16 {
    u16::from_le_bytes([data[ofs], data[ofs + 1]])
}

pub(crate) fn read_u32(data: &[u8], ofs: usize) -> u32 {
    u32::from_le_bytes([data[ofs], data[ofs + 1], data[ofs + 2], data[ofs + 3]])
}

pub(crate) fn write_u16(data: &mut [u8], ofs: usize, value: u16) {
    data[ofs..ofs + 2].copy_from_slice(&value.to_le_bytes());
}
