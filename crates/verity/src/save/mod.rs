pub mod bag;
pub mod crc16;
pub mod detect;
pub mod dex;
pub mod gen4;
pub mod gen5;
pub mod gen5_blocks;

pub use crc16::crc16_ccitt;
pub use detect::{detect, SaveKind};

use crate::pkm::{Format, Pkm, SIZE_4PARTY, SIZE_4STORED, SIZE_5PARTY, SIZE_5STORED};

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
pub struct Trainer {
    pub name_raw: [u16; 8],
    pub tid: u16,
    pub sid: u16,
    pub gender: u8,
    pub language: u8,
    pub money: u32,
    pub badges: u8,
    pub played_hours: u16,
    pub played_minutes: u8,
    pub played_seconds: u8,
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

    /// Writes the party count byte (no other change; used to roll back a rejected write).
    pub fn set_party_count(&mut self, n: usize) {
        let ofs = match &self.layout {
            Layout::Gen4(l) => l.party_count_offset(),
            Layout::Gen5(l) => l.party_count_offset(),
        };
        self.data[ofs] = n.min(6) as u8;
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

    pub fn trainer(&self) -> Trainer {
        let d = &*self.data;
        let ids = self.trainer_ids();
        let (gender, language, money, badges, hours, minutes, seconds) = match &self.layout {
            Layout::Gen4(l) => {
                let t = l.ot_offset();
                (
                    d[t + 0x18],
                    d[t + 0x19],
                    read_u32(d, t + 0x14),
                    d[t + 0x1A],
                    read_u16(d, t + 0x22),
                    d[t + 0x24],
                    d[t + 0x25],
                )
            }
            Layout::Gen5(l) => {
                let p = gen5::PLAYER_DATA;
                let m = l.misc_offset();
                (
                    d[p + 0x21],
                    d[p + 0x1E],
                    read_u32(d, m),
                    d[m + 4],
                    read_u16(d, p + 0x24),
                    d[p + 0x26],
                    d[p + 0x27],
                )
            }
        };
        Trainer {
            name_raw: self.trainer_name_raw(),
            tid: ids.tid,
            sid: ids.sid,
            gender,
            language,
            money,
            badges,
            played_hours: hours,
            played_minutes: minutes,
            played_seconds: seconds,
        }
    }

    fn dex_offset(&self) -> usize {
        match &self.layout {
            Layout::Gen4(l) => l.dex_offset(),
            Layout::Gen5(l) => l.dex_offset(),
        }
    }

    /// Registers a Pokémon in the Pokédex as PKHeX `SetDex` does on import.
    pub fn register_dex(&mut self, pk: &Pkm) {
        let ofs = self.dex_offset();
        dex::set_dex(self.kind, &mut self.data[ofs..], pk);
    }

    pub fn dex_caught(&self, species: u16) -> bool {
        dex::caught(self.kind, &self.data[self.dex_offset()..], species)
    }

    pub fn dex_seen(&self, species: u16) -> bool {
        dex::seen(self.kind, &self.data[self.dex_offset()..], species)
    }

    /// (flags byte offset, flag count, work offset, work count). PKHeX `SAV4.EventFlag/EventWork`,
    /// `EventWork5BW/B2W2`.
    fn event_layout(&self) -> (usize, usize, usize, usize) {
        match &self.layout {
            Layout::Gen4(l) => l.event_layout(),
            Layout::Gen5(l) => l.event_layout(),
        }
    }

    pub fn event_flag_count(&self) -> usize {
        self.event_layout().1
    }

    pub fn event_work_count(&self) -> usize {
        self.event_layout().3
    }

    pub fn event_flag(&self, i: usize) -> Result<bool, SaveError> {
        let (ofs, count, _, _) = self.event_layout();
        if i >= count {
            return Err(SaveError::OutOfRange);
        }
        Ok((self.data[ofs + (i >> 3)] >> (i & 7)) & 1 != 0)
    }

    pub fn set_event_flag(&mut self, i: usize, value: bool) -> Result<(), SaveError> {
        let (ofs, count, _, _) = self.event_layout();
        if i >= count {
            return Err(SaveError::OutOfRange);
        }
        let b = &mut self.data[ofs + (i >> 3)];
        *b = (*b & !(1 << (i & 7))) | (value as u8) << (i & 7);
        Ok(())
    }

    pub fn event_work(&self, i: usize) -> Result<u16, SaveError> {
        let (_, _, ofs, count) = self.event_layout();
        if i >= count {
            return Err(SaveError::OutOfRange);
        }
        Ok(read_u16(self.data, ofs + 2 * i))
    }

    pub fn set_event_work(&mut self, i: usize, value: u16) -> Result<(), SaveError> {
        let (_, _, ofs, count) = self.event_layout();
        if i >= count {
            return Err(SaveError::OutOfRange);
        }
        write_u16(self.data, ofs + 2 * i, value);
        Ok(())
    }

    pub fn pouches(&self) -> &'static [bag::PouchDef] {
        bag::pouches(self.kind)
    }

    fn bag_entry_offset(&self, pouch: usize, slot: usize) -> Result<usize, SaveError> {
        let def = self.pouches().get(pouch).ok_or(SaveError::OutOfRange)?;
        if slot >= def.slots as usize {
            return Err(SaveError::OutOfRange);
        }
        let base = match &self.layout {
            Layout::Gen4(l) => l.general_offset(),
            Layout::Gen5(l) => l.items_offset(),
        };
        Ok(base + def.offset as usize + 4 * slot)
    }

    /// (item id, count)
    pub fn bag_item(&self, pouch: usize, slot: usize) -> Result<(u16, u16), SaveError> {
        let ofs = self.bag_entry_offset(pouch, slot)?;
        Ok((read_u16(self.data, ofs), read_u16(self.data, ofs + 2)))
    }

    pub fn set_bag_item(
        &mut self,
        pouch: usize,
        slot: usize,
        item: u16,
        count: u16,
    ) -> Result<(), SaveError> {
        let ofs = self.bag_entry_offset(pouch, slot)?;
        write_u16(self.data, ofs, item);
        write_u16(self.data, ofs + 2, count);
        Ok(())
    }

    /// Raw box name (Gen 4: 20 code units; Gen 5: 10, rest zero).
    pub fn box_name_raw(&self, b: usize) -> Result<[u16; 20], SaveError> {
        if b >= self.box_count() {
            return Err(SaveError::OutOfRange);
        }
        let (ofs, len) = match &self.layout {
            Layout::Gen4(l) => (l.box_name_offset(b), 20),
            Layout::Gen5(_) => (gen5::box_name_offset(b), 10),
        };
        Ok(core::array::from_fn(|i| {
            if i < len {
                read_u16(self.data, ofs + 2 * i)
            } else {
                0
            }
        }))
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

    pub fn pkm_format(&self) -> Format {
        match self.layout {
            Layout::Gen4(_) => Format::Pk4,
            Layout::Gen5(_) => Format::Pk5,
        }
    }

    pub fn pkm(&self, slot: Slot) -> Result<Pkm, SaveError> {
        let raw = self.slot_raw(slot)?;
        Pkm::from_encrypted(self.pkm_format(), raw).ok_or(SaveError::BadLength)
    }

    /// Writes a Pokémon with a refreshed checksum. Party slots need party-format data and
    /// update the party count like PKHeX `SetPartySlotAtIndex`.
    pub fn set_pkm(&mut self, slot: Slot, pk: &Pkm) -> Result<(), SaveError> {
        if pk.format() != self.pkm_format() {
            return Err(SaveError::BadLength);
        }
        let mut pk = match slot {
            Slot::Box(..) => pk.to_stored(),
            Slot::Party(_) if pk.is_party_format() => pk.clone(),
            Slot::Party(_) => return Err(SaveError::BadLength),
        };
        pk.refresh_checksum();
        let (enc, len) = pk.encrypted();
        self.set_slot_raw(slot, &enc[..len])?;
        if let Slot::Party(i) = slot {
            let count = self.party_count();
            let i = i as usize;
            let ofs = match &self.layout {
                Layout::Gen4(l) => l.party_count_offset(),
                Layout::Gen5(l) => l.party_count_offset(),
            };
            if pk.species() != 0 && count <= i {
                self.data[ofs] = (i + 1) as u8;
            } else if pk.species() == 0 && count > i {
                self.data[ofs] = i as u8;
            }
        }
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

    /// The Gen 4 General block and the offset of its Mystery Gift block, or `None` on Gen 5.
    pub fn mystery4(&self) -> Option<(&[u8], usize)> {
        match &self.layout {
            Layout::Gen4(l) => Some((
                &self.data[l.general_offset()..],
                l.mystery_offset() - l.general_offset(),
            )),
            Layout::Gen5(_) => None,
        }
    }

    /// The Gen 5 Mystery Gift block, still encrypted, or `None` on Gen 4.
    pub fn mystery5(&self) -> Option<&[u8]> {
        match &self.layout {
            Layout::Gen4(_) => None,
            Layout::Gen5(_) => Some(&self.data[gen5::MYSTERY_RANGE]),
        }
    }

    /// Mutable form of [`Save::mystery5`].
    pub fn mystery5_mut(&mut self) -> Option<&mut [u8]> {
        match &self.layout {
            Layout::Gen4(_) => None,
            Layout::Gen5(_) => Some(&mut self.data[gen5::MYSTERY_RANGE]),
        }
    }

    /// The game a Gen 5 save belongs to (20 W, 21 B, 22 W2, 23 B2), or 0 on Gen 4, where the save
    /// doesn't record it.
    pub fn game_version(&self) -> u8 {
        match &self.layout {
            Layout::Gen4(_) => 0,
            Layout::Gen5(_) => self.data[gen5::PLAYER_DATA + 0x1F],
        }
    }

    /// Mutable form of [`Save::mystery4`].
    pub fn mystery4_mut(&mut self) -> Option<(&mut [u8], usize)> {
        match &self.layout {
            Layout::Gen4(l) => Some((
                &mut self.data[l.general_offset()..],
                l.mystery_offset() - l.general_offset(),
            )),
            Layout::Gen5(_) => None,
        }
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
