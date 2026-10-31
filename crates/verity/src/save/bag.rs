//! Bag pouches. Offsets from PKHeX `SAV4DP/SAV4Pt/SAV4HGSS.Inventory` (relative to the general
//! block) and `MyItem5BW`/`MyItem5B2W2` (relative to block 25). Slot counts are PKHeX's
//! `PouchDataSize` (the number of items valid in each pouch), exported by the oracle.

use super::SaveKind;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PouchKind {
    Items,
    KeyItems,
    TmHms,
    Mail,
    Medicine,
    Berries,
    Balls,
    BattleItems,
}

#[derive(Clone, Copy, Debug)]
pub struct PouchDef {
    pub kind: PouchKind,
    pub offset: u16,
    pub slots: u16,
    pub max_count: u16,
}

const fn p(kind: PouchKind, offset: u16, slots: u16, max_count: u16) -> PouchDef {
    PouchDef {
        kind,
        offset,
        slots,
        max_count,
    }
}

use PouchKind::*;

static DP: [PouchDef; 8] = [
    p(Items, 0x624, 161, 999),
    p(KeyItems, 0x8B8, 37, 1),
    p(TmHms, 0x980, 100, 99),
    p(Mail, 0xB10, 12, 999),
    p(Medicine, 0xB40, 38, 999),
    p(Berries, 0xBE0, 64, 999),
    p(Balls, 0xCE0, 15, 999),
    p(BattleItems, 0xD1C, 13, 999),
];

static PT: [PouchDef; 8] = [
    p(Items, 0x630, 162, 999),
    p(KeyItems, 0x8C4, 40, 1),
    p(TmHms, 0x98C, 100, 99),
    p(Mail, 0xB1C, 12, 999),
    p(Medicine, 0xB4C, 38, 999),
    p(Berries, 0xBEC, 64, 999),
    p(Balls, 0xCEC, 15, 999),
    p(BattleItems, 0xD28, 13, 999),
];

static HGSS: [PouchDef; 8] = [
    p(Items, 0x644, 162, 999),
    p(KeyItems, 0x8D8, 38, 1),
    p(TmHms, 0x9A0, 100, 99),
    p(Mail, 0xB34, 12, 999),
    p(Medicine, 0xB64, 38, 999),
    p(Berries, 0xC04, 64, 999),
    p(Balls, 0xD04, 24, 999),
    p(BattleItems, 0xD64, 13, 999),
];

static BW: [PouchDef; 5] = [
    p(Items, 0x000, 261, 999),
    p(KeyItems, 0x4D8, 19, 1),
    p(TmHms, 0x624, 101, 1),
    p(Medicine, 0x7D8, 47, 999),
    p(Berries, 0x898, 64, 999),
];

static B2W2: [PouchDef; 5] = [
    p(Items, 0x000, 261, 999),
    p(KeyItems, 0x4D8, 27, 1),
    p(TmHms, 0x624, 101, 1),
    p(Medicine, 0x7D8, 47, 999),
    p(Berries, 0x898, 64, 999),
];

pub fn pouches(kind: SaveKind) -> &'static [PouchDef] {
    match kind {
        SaveKind::DP => &DP,
        SaveKind::Pt => &PT,
        SaveKind::HGSS => &HGSS,
        SaveKind::BW => &BW,
        SaveKind::B2W2 => &B2W2,
    }
}
