//! Decrypted Gen 4/5 Pokémon. Ported from PKHeX `PKHeX.Core/PKM/PK4.cs`, `PK5.cs` and
//! `Shared/G4PKM.cs`.

use super::crypto::{calc_checksum, decrypt_45, encrypt_45};
use super::{SIZE_4PARTY, SIZE_4STORED, SIZE_5PARTY};

const MAX_LEN: usize = SIZE_4PARTY;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Pk4,
    Pk5,
}

/// Stat order used throughout: HP, Atk, Def, Spe, SpA, SpD (the stored order).
pub type Stats = [u16; 6];

#[derive(Clone, PartialEq, Eq)]
pub struct Pkm {
    data: [u8; MAX_LEN],
    len: usize,
    format: Format,
}

impl core::fmt::Debug for Pkm {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Pkm")
            .field("format", &self.format)
            .field("pid", &self.pid())
            .field("species", &self.species())
            .finish()
    }
}

impl Pkm {
    /// Builds from decrypted bytes (stored or party length).
    pub fn from_decrypted(format: Format, bytes: &[u8]) -> Option<Self> {
        let party = match format {
            Format::Pk4 => SIZE_4PARTY,
            Format::Pk5 => SIZE_5PARTY,
        };
        if bytes.len() != SIZE_4STORED && bytes.len() != party {
            return None;
        }
        let mut data = [0u8; MAX_LEN];
        data[..bytes.len()].copy_from_slice(bytes);
        Some(Self {
            data,
            len: bytes.len(),
            format,
        })
    }

    pub fn from_encrypted(format: Format, bytes: &[u8]) -> Option<Self> {
        let mut pk = Self::from_decrypted(format, bytes)?;
        decrypt_45(&mut pk.data[..pk.len]);
        Some(pk)
    }

    pub fn format(&self) -> Format {
        self.format
    }

    pub fn is_party_format(&self) -> bool {
        self.len > SIZE_4STORED
    }

    pub fn decrypted(&self) -> &[u8] {
        &self.data[..self.len]
    }

    pub fn encrypted(&self) -> ([u8; MAX_LEN], usize) {
        let mut out = self.data;
        encrypt_45(&mut out[..self.len]);
        (out, self.len)
    }

    fn u8(&self, ofs: usize) -> u8 {
        self.data[ofs]
    }

    fn u16(&self, ofs: usize) -> u16 {
        u16::from_le_bytes([self.data[ofs], self.data[ofs + 1]])
    }

    fn u32(&self, ofs: usize) -> u32 {
        u32::from_le_bytes([
            self.data[ofs],
            self.data[ofs + 1],
            self.data[ofs + 2],
            self.data[ofs + 3],
        ])
    }

    fn set_u8(&mut self, ofs: usize, v: u8) {
        self.data[ofs] = v;
    }

    fn set_u16(&mut self, ofs: usize, v: u16) {
        self.data[ofs..ofs + 2].copy_from_slice(&v.to_le_bytes());
    }

    fn set_u32(&mut self, ofs: usize, v: u32) {
        self.data[ofs..ofs + 4].copy_from_slice(&v.to_le_bytes());
    }

    pub fn pid(&self) -> u32 {
        self.u32(0x00)
    }
    pub fn set_pid(&mut self, v: u32) {
        self.set_u32(0x00, v)
    }

    pub fn checksum(&self) -> u16 {
        self.u16(0x06)
    }
    pub fn checksum_valid(&self) -> bool {
        self.checksum() == calc_checksum(&self.data)
    }
    pub fn refresh_checksum(&mut self) {
        let chk = calc_checksum(&self.data);
        self.set_u16(0x06, chk)
    }

    pub fn species(&self) -> u16 {
        self.u16(0x08)
    }
    pub fn set_species(&mut self, v: u16) {
        self.set_u16(0x08, v)
    }

    pub fn held_item(&self) -> u16 {
        self.u16(0x0A)
    }
    pub fn set_held_item(&mut self, v: u16) {
        self.set_u16(0x0A, v)
    }

    pub fn tid(&self) -> u16 {
        self.u16(0x0C)
    }
    pub fn sid(&self) -> u16 {
        self.u16(0x0E)
    }
    pub fn set_ids(&mut self, tid: u16, sid: u16) {
        self.set_u16(0x0C, tid);
        self.set_u16(0x0E, sid);
    }

    pub fn exp(&self) -> u32 {
        self.u32(0x10)
    }
    pub fn set_exp(&mut self, v: u32) {
        self.set_u32(0x10, v)
    }

    pub fn friendship(&self) -> u8 {
        self.u8(0x14)
    }
    pub fn set_friendship(&mut self, v: u8) {
        self.set_u8(0x14, v)
    }

    pub fn ability(&self) -> u8 {
        self.u8(0x15)
    }
    pub fn set_ability(&mut self, v: u8) {
        self.set_u8(0x15, v)
    }

    pub fn markings(&self) -> u8 {
        self.u8(0x16)
    }

    pub fn language(&self) -> u8 {
        self.u8(0x17)
    }
    pub fn set_language(&mut self, v: u8) {
        self.set_u8(0x17, v)
    }

    pub fn evs(&self) -> Stats {
        core::array::from_fn(|i| self.u8(0x18 + i) as u16)
    }
    pub fn set_evs(&mut self, evs: [u8; 6]) {
        self.data[0x18..0x1E].copy_from_slice(&evs)
    }

    /// Cool, Beauty, Cute, Smart, Tough, Sheen.
    pub fn contest(&self) -> [u8; 6] {
        core::array::from_fn(|i| self.u8(0x1E + i))
    }

    pub fn moves(&self) -> [u16; 4] {
        core::array::from_fn(|i| self.u16(0x28 + 2 * i))
    }
    pub fn set_moves(&mut self, moves: [u16; 4]) {
        for (i, m) in moves.iter().enumerate() {
            self.set_u16(0x28 + 2 * i, *m);
        }
    }

    pub fn pp(&self) -> [u8; 4] {
        core::array::from_fn(|i| self.u8(0x30 + i))
    }
    pub fn set_pp(&mut self, pp: [u8; 4]) {
        self.data[0x30..0x34].copy_from_slice(&pp)
    }

    pub fn pp_ups(&self) -> [u8; 4] {
        core::array::from_fn(|i| self.u8(0x34 + i))
    }
    pub fn set_pp_ups(&mut self, ups: [u8; 4]) {
        self.data[0x34..0x38].copy_from_slice(&ups)
    }

    pub fn iv32(&self) -> u32 {
        self.u32(0x38)
    }
    pub fn set_iv32(&mut self, v: u32) {
        self.set_u32(0x38, v)
    }

    pub fn ivs(&self) -> Stats {
        let iv = self.iv32();
        core::array::from_fn(|i| ((iv >> (5 * i)) & 0x1F) as u16)
    }
    pub fn set_ivs(&mut self, ivs: [u8; 6]) {
        let mut v = self.iv32() & 0xC000_0000;
        for (i, iv) in ivs.iter().enumerate() {
            v |= ((*iv).min(31) as u32) << (5 * i);
        }
        self.set_iv32(v)
    }

    pub fn is_egg(&self) -> bool {
        self.iv32() & 0x4000_0000 != 0
    }
    pub fn is_nicknamed(&self) -> bool {
        self.iv32() & 0x8000_0000 != 0
    }

    pub fn fateful(&self) -> bool {
        self.u8(0x40) & 1 != 0
    }
    pub fn gender(&self) -> u8 {
        (self.u8(0x40) >> 1) & 3
    }
    pub fn form(&self) -> u8 {
        self.u8(0x40) >> 3
    }
    pub fn set_fateful_gender_form(&mut self, fateful: bool, gender: u8, form: u8) {
        self.set_u8(0x40, fateful as u8 | (gender & 3) << 1 | form << 3)
    }

    pub fn nature(&self) -> u8 {
        match self.format {
            Format::Pk4 => (self.pid() % 25) as u8,
            Format::Pk5 => self.u8(0x41),
        }
    }
    pub fn set_nature_pk5(&mut self, v: u8) {
        if self.format == Format::Pk5 {
            self.set_u8(0x41, v)
        }
    }

    pub fn shiny_leaf(&self) -> u8 {
        match self.format {
            Format::Pk4 => self.u8(0x41),
            Format::Pk5 => 0,
        }
    }

    pub fn hidden_ability(&self) -> bool {
        self.format == Format::Pk5 && self.u8(0x42) & 1 != 0
    }
    pub fn n_sparkle(&self) -> bool {
        self.format == Format::Pk5 && self.u8(0x42) & 2 != 0
    }

    pub fn nickname_raw(&self) -> [u16; 11] {
        core::array::from_fn(|i| self.u16(0x48 + 2 * i))
    }
    pub fn set_nickname_raw(&mut self, raw: &[u16; 11]) {
        for (i, c) in raw.iter().enumerate() {
            self.set_u16(0x48 + 2 * i, *c);
        }
    }

    /// PKHeX `Nickname` setter (max 10, trailing trash kept). Does not change the
    /// nicknamed flag.
    pub fn set_nickname(&mut self, text: &[u16]) {
        let mut language = self.language();
        if language == crate::text::LANGUAGE_KOREAN
            && !self.is_nicknamed()
            && matches!(self.version(), 10..=12)
        {
            // Not-nicknamed D/P/Pt Korean Nidoran use full-width gender symbols.
            let name: &[u16] = match self.species() {
                32 => &[0xB2C8, 0xB4DC, 0xB7F0, 0x2642],
                29 => &[0xB2C8, 0xB4DC, 0xB7F0, 0x2640],
                _ => &[],
            };
            if !name.is_empty() && text == name {
                language = 1;
            }
        }
        let mut raw = self.nickname_raw();
        match self.format {
            Format::Pk4 => {
                crate::text::encode4(text, 10, language, &mut raw, crate::text::Clear::None)
            }
            Format::Pk5 => {
                crate::text::encode5(text, 10, language, &mut raw, crate::text::Clear::None)
            }
        };
        self.set_nickname_raw(&raw);
    }

    pub fn set_is_nicknamed(&mut self, v: bool) {
        let iv = self.iv32() & 0x7FFF_FFFF;
        self.set_iv32(iv | (v as u32) << 31)
    }

    pub fn version(&self) -> u8 {
        self.u8(0x5F)
    }

    pub fn ot_name_raw(&self) -> [u16; 8] {
        core::array::from_fn(|i| self.u16(0x68 + 2 * i))
    }
    pub fn set_ot_name_raw(&mut self, raw: &[u16; 8]) {
        for (i, c) in raw.iter().enumerate() {
            self.set_u16(0x68 + 2 * i, *c);
        }
    }

    /// (year since 2000, month, day)
    pub fn egg_date(&self) -> [u8; 3] {
        [self.u8(0x78), self.u8(0x79), self.u8(0x7A)]
    }
    pub fn met_date(&self) -> [u8; 3] {
        [self.u8(0x7B), self.u8(0x7C), self.u8(0x7D)]
    }

    /// Gen 4 prefers the Pt/HGSS location (0x44/0x46) when set (`G4PKM.EggLocation`).
    pub fn egg_location(&self) -> u16 {
        match self.format {
            Format::Pk4 => match self.u16(0x44) {
                0 => self.u16(0x7E),
                v => v,
            },
            Format::Pk5 => self.u16(0x7E),
        }
    }

    pub fn met_location(&self) -> u16 {
        match self.format {
            Format::Pk4 => match self.u16(0x46) {
                0 => self.u16(0x80),
                v => v,
            },
            Format::Pk5 => self.u16(0x80),
        }
    }

    pub fn pokerus(&self) -> u8 {
        self.u8(0x82)
    }

    /// Gen 4 displays the larger of the DPPt (0x83) and HGSS (0x86) ball (`G4PKM.Ball`).
    pub fn ball(&self) -> u8 {
        match self.format {
            Format::Pk4 => self.u8(0x83).max(self.u8(0x86)),
            Format::Pk5 => self.u8(0x83),
        }
    }

    pub fn met_level(&self) -> u8 {
        self.u8(0x84) & 0x7F
    }
    pub fn ot_gender(&self) -> u8 {
        self.u8(0x84) >> 7
    }

    pub fn ground_tile(&self) -> u8 {
        self.u8(0x85)
    }

    pub fn status_condition(&self) -> Option<u32> {
        self.is_party_format().then(|| self.u32(0x88))
    }

    pub fn stat_level(&self) -> Option<u8> {
        self.is_party_format().then(|| self.u8(0x8C))
    }

    /// Stored party stats (max HP, Atk, Def, Spe, SpA, SpD).
    pub fn stored_stats(&self) -> Option<Stats> {
        self.is_party_format()
            .then(|| core::array::from_fn(|i| self.u16(0x90 + 2 * i)))
    }

    pub fn hp_current(&self) -> Option<u16> {
        self.is_party_format().then(|| self.u16(0x8E))
    }

    /// Converts to party format, writing level and stats; HP is set to full.
    pub fn to_party(&self, level: u8, stats: &Stats) -> Self {
        let party = match self.format {
            Format::Pk4 => SIZE_4PARTY,
            Format::Pk5 => SIZE_5PARTY,
        };
        let mut pk = self.clone();
        pk.len = party;
        pk.set_u8(0x8C, level);
        pk.set_u16(0x8E, stats[0]);
        for (i, s) in stats.iter().enumerate() {
            pk.set_u16(0x90 + 2 * i, *s);
        }
        pk
    }

    /// PKHeX `ResetPartyStats`: recompute level and stats, full HP, clear status.
    pub fn reset_party_stats(&mut self, info: &super::BaseInfo) {
        let level = self.level(info);
        let stats = self.calc_stats(info);
        *self = self.to_party(level, &stats);
        self.set_u32(0x88, 0);
    }

    pub fn to_stored(&self) -> Self {
        let mut pk = self.clone();
        pk.data[SIZE_4STORED..].fill(0);
        pk.len = SIZE_4STORED;
        pk
    }
}
