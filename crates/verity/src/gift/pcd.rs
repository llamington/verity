//! Gen 4 Wonder Cards (PCD) and their inner gifts (PGT). Ported from PKHeX
//! `MysteryGifts/PCD.cs` and `MysteryGifts/PGT.cs`; the database is PKHeX's
//! `EncounterEvent.MGDB_G4` (`wc4.pkl`) exported by the oracle (`gifts4.bin`).

use crate::data::blob::{Blob, BlobError, BlobKind};
use crate::pkm::{Format, Pkm, SIZE_4PARTY};

pub const PCD_SIZE: usize = 0x358;
pub const PGT_SIZE: usize = 0x104;
/// Blob record: u16 MGDB index, u8 language, u8 region, u8 games, u8 flags, u16 reserved, then
/// the PCD (inner PK4 decrypted).
const HEADER: usize = 8;
const REC: usize = HEADER + PCD_SIZE;
/// `Gift4::mgdb_index` of the Ranger Manaphy record (no Wonder Card, not in PKHeX's database).
pub const RANGER_INDEX: u16 = 0xFFFF;
const PK_OFS: usize = 8;

/// PKHeX `GiftType4`.
pub mod gift_type {
    pub const POKEMON: u16 = 1;
    pub const POKEMON_EGG: u16 = 2;
    pub const ITEM: u16 = 3;
    pub const MANAPHY_EGG: u16 = 7;
    pub const POKEMON_MOVIE: u16 = 13;
}

/// The Gen 4 event database (Pokémon gifts only, in PKHeX's MGDB order).
#[derive(Clone, Copy)]
pub struct Gifts4<'a> {
    recs: &'a [u8],
}

impl<'a> Gifts4<'a> {
    pub fn load(data: &'a [u8]) -> Result<Self, BlobError> {
        let b = Blob::expect(data, BlobKind::Gifts)?;
        let (t, elem) = b.table(1).ok_or(BlobError::BadDirectory)?;
        if elem != REC || !t.len().is_multiple_of(REC) {
            return Err(BlobError::BadDirectory);
        }
        Ok(Self { recs: t })
    }

    /// No gifts (event matching disabled).
    pub const fn empty() -> Self {
        Self { recs: &[] }
    }

    pub fn len(&self) -> usize {
        self.recs.len() / REC
    }

    pub fn is_empty(&self) -> bool {
        self.recs.is_empty()
    }

    pub fn get(&self, i: usize) -> Option<Gift4<'a>> {
        let rec = self.recs.get(i * REC..(i + 1) * REC)?;
        Some(Gift4 { rec })
    }

    pub fn iter(&self) -> impl Iterator<Item = Gift4<'a>> + 'a + use<'a> {
        self.recs
            .as_chunks::<REC>()
            .0
            .iter()
            .map(|rec| Gift4 { rec })
    }
}

/// One Wonder Card from the database.
#[derive(Clone, Copy)]
pub struct Gift4<'a> {
    rec: &'a [u8],
}

impl<'a> Gift4<'a> {
    /// Index into PKHeX's `EncounterEvent.MGDB_G4`.
    pub fn mgdb_index(&self) -> u16 {
        u16::from_le_bytes([self.rec[0], self.rec[1]])
    }

    /// The language the card was distributed in (a save language ID); 0 means any.
    pub fn language(&self) -> u8 {
        self.rec[2]
    }

    /// The distribution region within that language: 0 none, 1 NA, 2 UK, 3 AU, 4 EU.
    pub fn region(&self) -> u8 {
        self.rec[3]
    }

    /// The games the event was for: bit 0 D, 1 P, 2 Pt, 3 HG, 4 SS.
    pub fn games(&self) -> u8 {
        self.rec[4]
    }

    /// Whether this is a genuine card that can be delivered (see `docs/research/events-region.md`).
    pub fn injectable(&self) -> bool {
        self.rec[5] != 0
    }

    /// The whole Wonder Card (inner PK4 decrypted).
    pub fn pcd(&self) -> &'a [u8] {
        &self.rec[HEADER..]
    }

    /// The gift's title (raw Gen 4 characters).
    pub fn title_raw(&self) -> [u16; 36] {
        let mut out = [0u16; 36];
        for (o, c) in out
            .iter_mut()
            .zip(self.pcd()[0x104..0x14C].as_chunks::<2>().0)
        {
            *o = u16::from_le_bytes(*c);
        }
        out
    }

    /// The inner gift (PGT).
    pub fn pgt(&self) -> &'a [u8] {
        &self.pcd()[..PGT_SIZE]
    }

    /// PKHeX `PGT.CardType` (`GiftType4`).
    pub fn card_type(&self) -> u16 {
        u16::from_le_bytes([self.pgt()[0], self.pgt()[1]])
    }

    /// PKHeX `PCD.CardID`.
    pub fn card_id(&self) -> u16 {
        u16::from_le_bytes([self.pcd()[0x150], self.pcd()[0x151]])
    }

    /// PKHeX `PCD.CardCompatibility`.
    pub fn compatibility(&self) -> u16 {
        u16::from_le_bytes([self.pcd()[0x14C], self.pcd()[0x14D]])
    }

    fn pk_bytes(&self) -> &'a [u8] {
        &self.pgt()[PK_OFS..PK_OFS + SIZE_4PARTY]
    }

    /// The template PK4 (PKHeX `PGT.PK`).
    pub fn pk(&self) -> Pkm {
        Pkm::from_decrypted(Format::Pk4, self.pk_bytes()).expect("PK4 party length")
    }

    /// PKHeX `PCD.Species` (Manaphy for the Manaphy egg card).
    pub fn species(&self) -> u16 {
        if self.card_type() == gift_type::MANAPHY_EGG {
            return 490;
        }
        let b = self.pk_bytes();
        u16::from_le_bytes([b[8], b[9]])
    }

    /// PKHeX `PGT.IsEgg`.
    pub fn is_egg(&self) -> bool {
        matches!(
            self.card_type(),
            gift_type::POKEMON_EGG | gift_type::MANAPHY_EGG
        )
    }

    /// PKHeX `PGT.IsEntity`.
    pub fn is_entity(&self) -> bool {
        matches!(
            self.card_type(),
            gift_type::POKEMON
                | gift_type::POKEMON_EGG
                | gift_type::MANAPHY_EGG
                | gift_type::POKEMON_MOVIE
        )
    }
}
