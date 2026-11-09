//! Gen 5 Wonder Cards: PKHeX `MysteryGifts/PGF.cs`, read from the raw 0xCC-byte card.

use crate::data::blob::{Blob, BlobError, BlobKind};

pub const PGF_SIZE: usize = 0xCC;

/// One card.
#[derive(Clone, Copy)]
pub struct Pgf<'a>(pub &'a [u8]);

impl<'a> Pgf<'a> {
    fn u16(&self, o: usize) -> u16 {
        u16::from_le_bytes([self.0[o], self.0[o + 1]])
    }
    pub fn tid(&self) -> u16 {
        self.u16(0)
    }
    pub fn sid(&self) -> u16 {
        self.u16(2)
    }
    pub fn origin_game(&self) -> u8 {
        self.0[4]
    }
    pub fn pid(&self) -> u32 {
        u32::from_le_bytes([self.0[8], self.0[9], self.0[10], self.0[11]])
    }
    /// Country, National, Earth, World, Classic, Premier, Event, Birthday.
    pub fn rib0(&self) -> u8 {
        self.0[0x0C]
    }
    /// Special, Souvenir, Wishing, Champion Battle/Regional/National/World.
    pub fn rib1(&self) -> u8 {
        self.0[0x0D]
    }
    pub fn ball(&self) -> u8 {
        self.0[0x0E]
    }
    pub fn moves(&self) -> [u16; 4] {
        core::array::from_fn(|i| self.u16(0x12 + 2 * i))
    }
    pub fn species(&self) -> u16 {
        self.u16(0x1A)
    }
    pub fn form(&self) -> u8 {
        self.0[0x1C]
    }
    pub fn language(&self) -> u8 {
        self.0[0x1D]
    }
    pub fn nickname_raw(&self) -> [u16; 11] {
        core::array::from_fn(|i| self.u16(0x1E + 2 * i))
    }
    pub fn nature(&self) -> u8 {
        self.0[0x34]
    }
    pub fn gender(&self) -> u8 {
        self.0[0x35]
    }
    pub fn ability_type(&self) -> u8 {
        self.0[0x36]
    }
    /// 0 never shiny, 1 random, 2 always shiny.
    pub fn pid_type(&self) -> u8 {
        self.0[0x37]
    }
    pub fn egg_location(&self) -> u16 {
        self.u16(0x38)
    }
    pub fn location(&self) -> u16 {
        self.u16(0x3A)
    }
    /// Cool, Beauty, Cute, Smart, Tough, Sheen.
    pub fn contest(&self) -> [u8; 6] {
        core::array::from_fn(|i| self.0[0x3D + i])
    }
    /// HP, Atk, Def, Spe, SpA, SpD; 0xFF is random.
    pub fn ivs(&self) -> [u8; 6] {
        core::array::from_fn(|i| self.0[0x43 + i])
    }
    pub fn ot_raw(&self) -> [u16; 8] {
        core::array::from_fn(|i| self.u16(0x4A + 2 * i))
    }
    pub fn ot_gender(&self) -> u8 {
        self.0[0x5A]
    }
    pub fn level(&self) -> u8 {
        self.0[0x5B]
    }
    pub fn is_egg(&self) -> bool {
        self.0[0x5C] == 1
    }
    /// 1 Pokémon, 2 item, 3 Pass Power.
    pub fn card_type(&self) -> u8 {
        self.0[0xB3]
    }
    pub fn is_nicknamed(&self) -> bool {
        // An empty name starts with the terminator.
        !matches!(self.u16(0x1E), 0 | 0xFFFF)
    }
    /// `(sinnoh[2] & 0xE8, sinnoh[3] & 0x0F, hoenn[3] & 0xFE)` as a PK5 holds these ribbons.
    pub fn event_ribbons(&self) -> [u8; 3] {
        let bit = |b: u8, n: u32| (b >> n) & 1;
        let (r0, r1) = (self.rib0(), self.rib1());
        [
            bit(r0, 6) << 3 | bit(r1, 6) << 5 | bit(r0, 7) << 6 | bit(r1, 0) << 7,
            bit(r1, 1) | bit(r1, 2) << 1 | bit(r0, 4) << 2 | bit(r0, 5) << 3,
            bit(r1, 3) << 1
                | bit(r1, 4) << 2
                | bit(r1, 5) << 3
                | bit(r0, 0) << 4
                | bit(r0, 1) << 5
                | bit(r0, 2) << 6
                | bit(r0, 3) << 7,
        ]
    }
}

/// PKHeX's Gen 5 event database (`gifts5.bin`).
#[derive(Clone, Copy)]
pub struct Gifts5<'a> {
    recs: &'a [u8],
}

impl<'a> Gifts5<'a> {
    pub fn load(data: &'a [u8]) -> Result<Self, BlobError> {
        let b = Blob::expect(data, BlobKind::Gifts)?;
        let (t, elem) = b.table(1).ok_or(BlobError::BadDirectory)?;
        if elem != PGF_SIZE || !t.len().is_multiple_of(PGF_SIZE) {
            return Err(BlobError::BadDirectory);
        }
        Ok(Self { recs: t })
    }

    pub const fn empty() -> Self {
        Self { recs: &[] }
    }

    pub fn iter(&self) -> impl Iterator<Item = Pgf<'a>> + 'a {
        self.recs
            .as_chunks::<PGF_SIZE>()
            .0
            .iter()
            .map(|c| Pgf(c.as_slice()))
    }
}
