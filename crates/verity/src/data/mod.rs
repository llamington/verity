//! Static data loaded from blobs built by `datagen` from PKHeX exports.

pub mod blob;
pub mod encounters;

use blob::{Blob, BlobError, BlobKind, Indexed, U16s};

use crate::pkm::{BaseInfo, Stats};

/// Table ids in a [`BlobKind::Game`] blob.
pub mod game_table {
    pub const PERSONAL: u16 = 1;
    pub const TYPE_TUTORS: u16 = 2;
    pub const LEVELUP: u16 = 3;
    pub const EGG_MOVES: u16 = 4;
    pub const HELD_ITEMS: u16 = 5;
    pub const MAX_SPECIES: u16 = 6;
    pub const EVOLUTIONS: u16 = 7;
    /// Gen 3 origin blob only: level-up moves of the other games, and the encounter blob.
    pub const LEVELUP_RS: u16 = 8;
    pub const LEVELUP_FR: u16 = 9;
    pub const LEVELUP_LG: u16 = 10;
    pub const ENCOUNTERS: u16 = 11;
}

/// Table ids in a [`BlobKind::Moves`] blob.
pub mod move_table {
    pub const PP: u16 = 1;
    pub const TYPE: u16 = 2;
    pub const G4_TUTOR: u16 = 10;
    pub const G4_TM: u16 = 11;
    pub const G4_HM_DPPT: u16 = 12;
    pub const G4_HM_HGSS: u16 = 13;
    pub const G4_SPECIAL_BLAST_BURN: u16 = 14;
    pub const G4_SPECIAL_HYDRO_CANNON: u16 = 15;
    pub const G4_SPECIAL_FRENZY_PLANT: u16 = 16;
    pub const G4_SPECIAL_DRACO_METEOR: u16 = 17;
    pub const G5_TMHM: u16 = 20;
    pub const G5_TYPE_TUTOR: u16 = 21;
    pub const B2W2_TUTOR_1: u16 = 22;
    pub const B2W2_TUTOR_2: u16 = 23;
    pub const B2W2_TUTOR_3: u16 = 24;
    pub const B2W2_TUTOR_4: u16 = 25;
    /// PKHeX `ItemRestrictions.IsHeldItemAllowed` for the generation (sorted item IDs).
    pub const RELEASED_HELD: u16 = 30;
}

/// String categories in a [`BlobKind::Strings`] blob.
pub mod string_table {
    pub const SPECIES: u16 = 1;
    pub const MOVES: u16 = 2;
    pub const ABILITIES: u16 = 3;
    pub const NATURES: u16 = 4;
    pub const TYPES: u16 = 5;
    pub const ITEMS4: u16 = 6;
    pub const ITEMS5: u16 = 7;
    /// Met location names (PKHeX `GameStrings.Gen4.Met0/Met2/Met3`, `Gen5.Met0/3/4/6`), one
    /// category per bank; see [`super::Strings::location`].
    pub const LOC4_0: u16 = 8;
    pub const LOC4_2000: u16 = 9;
    pub const LOC4_3000: u16 = 10;
    pub const LOC5_0: u16 = 11;
    pub const LOC5_30000: u16 = 12;
    pub const LOC5_40000: u16 = 13;
    pub const LOC5_60000: u16 = 14;
}

/// A species/form entry (PKHeX `PersonalInfo4` / `PersonalInfo5BW` / `PersonalInfo5B2W2`).
#[derive(Clone, Copy)]
pub struct Personal<'a> {
    raw: &'a [u8],
    gen5: bool,
}

impl Personal<'_> {
    pub fn raw(&self) -> &[u8] {
        self.raw
    }

    pub fn base_stats(&self) -> Stats {
        core::array::from_fn(|i| self.raw[i] as u16)
    }

    pub fn types(&self) -> [u8; 2] {
        [self.raw[6], self.raw[7]]
    }

    pub fn catch_rate(&self) -> u8 {
        self.raw[8]
    }

    fn at(&self, g4: usize, g5: usize) -> u8 {
        self.raw[if self.gen5 { g5 } else { g4 }]
    }

    fn u16_at(&self, g4: usize, g5: usize) -> u16 {
        let o = if self.gen5 { g5 } else { g4 };
        u16::from_le_bytes([self.raw[o], self.raw[o + 1]])
    }

    pub fn held_items(&self) -> [u16; 3] {
        if self.gen5 {
            [
                self.u16_at(0, 0xC),
                self.u16_at(0, 0xE),
                self.u16_at(0, 0x10),
            ]
        } else {
            [self.u16_at(0xC, 0), self.u16_at(0xE, 0), 0]
        }
    }

    pub fn gender_ratio(&self) -> u8 {
        self.at(0x10, 0x12)
    }

    pub fn hatch_cycles(&self) -> u8 {
        self.at(0x11, 0x13)
    }

    pub fn base_friendship(&self) -> u8 {
        self.at(0x12, 0x14)
    }

    pub fn growth(&self) -> u8 {
        self.at(0x13, 0x15)
    }

    pub fn egg_groups(&self) -> [u8; 2] {
        [self.at(0x14, 0x16), self.at(0x15, 0x17)]
    }

    /// Ability 1, 2 and hidden (0 in Gen 4).
    pub fn abilities(&self) -> [u8; 3] {
        if self.gen5 {
            [self.raw[0x18], self.raw[0x19], self.raw[0x1A]]
        } else {
            [self.raw[0x16], self.raw[0x17], 0]
        }
    }

    pub fn form_count(&self) -> u8 {
        self.at(0x29, 0x20)
    }

    pub fn form_stats_index(&self) -> u16 {
        self.u16_at(0x2A, 0x1C)
    }

    /// TM/HM learn bit (Gen 4: 92 TMs then 8 HMs; Gen 5: 101 TM/HM entries).
    pub fn tmhm(&self, index: usize) -> bool {
        let base = if self.gen5 { 0x28 } else { 0x1C };
        index < 104 && (self.raw[base + (index >> 3)] >> (index & 7)) & 1 != 0
    }

    pub fn base_info(&self) -> BaseInfo {
        BaseInfo {
            base: self.base_stats(),
            gender_ratio: self.gender_ratio(),
            growth: self.growth(),
        }
    }
}

/// Per-game species and learnset data.
#[derive(Clone, Copy)]
pub struct GameData<'a> {
    blob: Blob<'a>,
    personal: &'a [u8],
    personal_size: usize,
}

impl<'a> GameData<'a> {
    pub fn load(data: &'a [u8]) -> Result<Self, BlobError> {
        let blob = Blob::expect(data, BlobKind::Game)?;
        let (personal, personal_size) = blob
            .table(game_table::PERSONAL)
            .ok_or(BlobError::BadDirectory)?;
        if personal_size == 0 {
            return Err(BlobError::BadDirectory);
        }
        Ok(Self {
            blob,
            personal,
            personal_size,
        })
    }

    pub fn generation(&self) -> u8 {
        self.blob.generation()
    }

    pub fn personal_count(&self) -> usize {
        self.personal.len() / self.personal_size
    }

    pub fn max_species(&self) -> u16 {
        U16s(self.blob.bytes(game_table::MAX_SPECIES))
            .get(0)
            .unwrap_or(0)
    }

    /// Entry by table index; out-of-range indexes return entry 0 like PKHeX.
    pub fn personal(&self, index: usize) -> Personal<'a> {
        let index = if index < self.personal_count() {
            index
        } else {
            0
        };
        let s = self.personal_size;
        Personal {
            raw: &self.personal[index * s..(index + 1) * s],
            gen5: self.generation() == 5,
        }
    }

    /// PKHeX `PersonalTable.GetFormIndex` / `PersonalInfo.FormIndex`.
    pub fn form_index(&self, species: u16, form: u8) -> usize {
        if species > self.max_species() {
            return 0;
        }
        let p = self.personal(species as usize);
        let fsi = p.form_stats_index();
        if form == 0 || fsi == 0 || form >= p.form_count() {
            return species as usize;
        }
        fsi as usize + form as usize - 1
    }

    pub fn species_form(&self, species: u16, form: u8) -> Personal<'a> {
        self.personal(self.form_index(species, form))
    }

    /// Level-up moves for a table index as (level, move).
    pub fn levelup(&self, index: usize) -> impl Iterator<Item = (u8, u16)> + 'a {
        Indexed(self.blob.bytes(game_table::LEVELUP))
            .get(index)
            .as_chunks::<4>()
            .0
            .iter()
            .map(|c| (c[2], u16::from_le_bytes([c[0], c[1]])))
    }

    /// Level-up moves from one of the Gen 3 per-game tables ([`game_table::LEVELUP_RS`] etc.).
    pub fn levelup_from(&self, id: u16, index: usize) -> impl Iterator<Item = (u8, u16)> + 'a {
        Indexed(self.blob.bytes(id))
            .get(index)
            .as_chunks::<4>()
            .0
            .iter()
            .map(|c| (c[2], u16::from_le_bytes([c[0], c[1]])))
    }

    /// Bytes of a table in this blob.
    pub fn raw_table(&self, id: u16) -> &'a [u8] {
        self.blob.bytes(id)
    }

    pub fn egg_moves(&self, index: usize) -> U16s<'a> {
        U16s(Indexed(self.blob.bytes(game_table::EGG_MOVES)).get(index))
    }

    /// HGSS type-tutor bits (64 per entry, index into the Gen 4 tutor move table).
    pub fn type_tutor(&self, index: usize, bit: usize) -> bool {
        let t = self.blob.bytes(game_table::TYPE_TUTORS);
        let o = index * 8 + (bit >> 3);
        bit < 64 && o < t.len() && (t[o] >> (bit & 7)) & 1 != 0
    }

    pub fn held_items(&self) -> U16s<'a> {
        U16s(self.blob.bytes(game_table::HELD_ITEMS))
    }

    /// Forward evolutions for a table index (PKHeX `EvolutionTree.Forward.GetForward`).
    pub fn evolutions(&self, index: usize) -> impl Iterator<Item = Evolution> + 'a {
        Indexed(self.blob.bytes(game_table::EVOLUTIONS))
            .get(index)
            .as_chunks::<8>()
            .0
            .iter()
            .map(|c| Evolution {
                species: u16::from_le_bytes([c[0], c[1]]),
                argument: u16::from_le_bytes([c[2], c[3]]),
                form: c[4],
                method: c[5],
                level: c[6],
                level_up: c[7],
            })
    }
}

/// PKHeX `EvolutionMethod`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Evolution {
    pub species: u16,
    pub argument: u16,
    pub form: u8,
    pub method: u8,
    pub level: u8,
    pub level_up: u8,
}

/// Per-generation move data and move lists.
#[derive(Clone, Copy)]
pub struct MoveData<'a> {
    blob: Blob<'a>,
}

impl<'a> MoveData<'a> {
    pub fn load(data: &'a [u8]) -> Result<Self, BlobError> {
        Ok(Self {
            blob: Blob::expect(data, BlobKind::Moves)?,
        })
    }

    pub fn max_move(&self) -> u16 {
        (self.blob.bytes(move_table::PP).len() as u16).saturating_sub(1)
    }

    pub fn base_pp(&self, mv: u16) -> u8 {
        self.blob
            .bytes(move_table::PP)
            .get(mv as usize)
            .copied()
            .unwrap_or(0)
    }

    pub fn move_type(&self, mv: u16) -> u8 {
        self.blob
            .bytes(move_table::TYPE)
            .get(mv as usize)
            .copied()
            .unwrap_or(0)
    }

    /// PKHeX `PKM.GetMovePP`.
    pub fn max_pp(&self, mv: u16, pp_ups: u8) -> u8 {
        (self.base_pp(mv) as u32 * (5 + pp_ups as u32) / 5) as u8
    }

    pub fn list(&self, id: u16) -> U16s<'a> {
        U16s(self.blob.bytes(id))
    }
}

/// Localized names (UTF-16).
#[derive(Clone, Copy)]
pub struct Strings<'a> {
    blob: Blob<'a>,
}

impl<'a> Strings<'a> {
    pub fn load(data: &'a [u8]) -> Result<Self, BlobError> {
        Ok(Self {
            blob: Blob::expect(data, BlobKind::Strings)?,
        })
    }

    pub fn count(&self, category: u16) -> usize {
        Indexed(self.blob.bytes(category)).count()
    }

    pub fn get(&self, category: u16, id: usize) -> U16s<'a> {
        U16s(Indexed(self.blob.bytes(category)).get(id))
    }

    /// Met/egg location name for a Gen 4 or 5 location ID (empty if unknown). Mirrors PKHeX
    /// `GameStrings.GetLocationName` bank selection.
    pub fn location(&self, generation: u8, id: u16) -> U16s<'a> {
        use string_table as st;
        let (cat, base) = match (generation, id) {
            (4, 0..=1999) => (st::LOC4_0, 0),
            (4, 2000..=2999) => (st::LOC4_2000, 2000),
            (4, 3000..=3999) => (st::LOC4_3000, 3000),
            (5, 0..=29999) => (st::LOC5_0, 0),
            (5, 30000..=39999) => (st::LOC5_30000, 30000),
            (5, 40000..=59999) => (st::LOC5_40000, 40000),
            (5, 60000..) => (st::LOC5_60000, 60000),
            _ => return U16s(&[]),
        };
        let i = (id - base) as usize;
        if i >= self.count(cat) {
            return U16s(&[]);
        }
        self.get(cat, i)
    }
}
