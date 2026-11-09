//! Encounter templates exported from PKHeX `Encounters4DPPt`, `Encounters4HGSS`,
//! `Encounters5BW`, `Encounters5B2W2` and `Encounters5DR`. Values keep PKHeX's conventions
//! (gender 255 = random, nature 25 = random, `Shiny`/`AbilityPermission`/`Ball` enum values).

use super::blob::{Blob, BlobError, BlobKind, Indexed, U16s};

pub mod table {
    pub const TEMPLATES: u16 = 1;
    pub const AREAS: u16 = 2;
    pub const GROUP_NAMES: u16 = 3;
    pub const GROUP_RANGES: u16 = 4;
    pub const SLOTS: u16 = 5;
    /// In-game trade nicknames and OT names by language, one record per template.
    pub const TRADE_NAMES: u16 = 6;
    /// Fixed moves of Gen 3 swarm slots, 4 u16s each, indexed by the slot record's byte 19.
    pub const SWARM_MOVES: u16 = 7;
}

pub const TEMPLATE_LEN: usize = 68;
pub const AREA_LEN: usize = 12;
/// Wild slot record: kind, form, species u16, level min/max, slot number, magnet pull
/// index/count, static index/count, flags, area u16, slot type, ball, ability, shiny, 2 spare.
pub const SLOT_LEN: usize = 20;
/// Set in a group range's count when the group indexes the slot table.
pub const RANGE_SLOTS: u32 = 1 << 31;
/// Slot record flag bits (byte 11).
pub const SLOT_RANDOM_FORM: u8 = 1 << 0;
pub const SLOT_HIDDEN_GROTTO: u8 = 1 << 1;
pub const SLOT_CAN_RADAR: u8 = 1 << 2;
pub const NO_AREA: u16 = 0xFFFF;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum EncounterKind {
    Slot4 = 1,
    Static4 = 2,
    Pokewalker4 = 3,
    Trade4Pid = 4,
    Ranch4 = 5,
    Slot3 = 6,
    Slot3Swarm = 7,
    Static3 = 8,
    Trade3 = 9,
    Slot5 = 10,
    Static5 = 11,
    Entree5 = 12,
    Radar5 = 13,
    Static5N = 14,
    Trade5BW = 15,
    Trade5B2W2 = 16,
}

impl EncounterKind {
    pub fn from_u8(v: u8) -> Option<Self> {
        use EncounterKind::*;
        Some(match v {
            1 => Slot4,
            2 => Static4,
            3 => Pokewalker4,
            4 => Trade4Pid,
            5 => Ranch4,
            6 => Slot3,
            7 => Slot3Swarm,
            8 => Static3,
            9 => Trade3,
            10 => Slot5,
            11 => Static5,
            12 => Entree5,
            13 => Radar5,
            14 => Static5N,
            15 => Trade5BW,
            16 => Trade5B2W2,
            _ => return None,
        })
    }

    pub fn from_pkhex_name(name: &str) -> Option<Self> {
        use EncounterKind::*;
        Some(match name {
            "EncounterSlot4" => Slot4,
            "EncounterStatic4" => Static4,
            "EncounterStatic4Pokewalker" => Pokewalker4,
            "EncounterTrade4PID" => Trade4Pid,
            "EncounterTrade4RanchGift" => Ranch4,
            "EncounterSlot3" => Slot3,
            "EncounterSlot3Swarm" => Slot3Swarm,
            "EncounterStatic3" => Static3,
            "EncounterTrade3" => Trade3,
            "EncounterSlot5" => Slot5,
            "EncounterStatic5" => Static5,
            "EncounterStatic5Entree" => Entree5,
            "EncounterStatic5Radar" => Radar5,
            "EncounterStatic5N" => Static5N,
            "EncounterTrade5BW" => Trade5BW,
            "EncounterTrade5B2W2" => Trade5B2W2,
            _ => return None,
        })
    }
}

pub mod flag {
    pub const FATEFUL: u16 = 1 << 0;
    pub const EGG: u16 = 1 << 1;
    pub const ROAMING: u16 = 1 << 2;
    pub const WILD_CORRELATION_PID: u16 = 1 << 3;
    pub const HIDDEN_GROTTO: u16 = 1 << 4;
    pub const FIXED_TRAINER: u16 = 1 << 5;
    pub const FIXED_NICKNAME: u16 = 1 << 6;
    pub const LANGUAGE_SWAP: u16 = 1 << 7;
    pub const RANDOM_UNSPECIFIC_FORM: u16 = 1 << 8;
    pub const HAS_MOVES: u16 = 1 << 9;
    pub const HAS_IVS: u16 = 1 << 10;
    pub const HAS_PID: u16 = 1 << 11;
    pub const CORONET_FEEBAS: u16 = 1 << 12;
}

#[derive(Clone, Copy)]
pub struct Template<'a>(&'a [u8]);

impl Template<'_> {
    fn u16(&self, o: usize) -> u16 {
        u16::from_le_bytes([self.0[o], self.0[o + 1]])
    }
    fn u32(&self, o: usize) -> u32 {
        u32::from_le_bytes([self.0[o], self.0[o + 1], self.0[o + 2], self.0[o + 3]])
    }
    pub fn kind(&self) -> Option<EncounterKind> {
        EncounterKind::from_u8(self.0[0])
    }
    pub fn version(&self) -> u8 {
        self.0[1]
    }
    pub fn species(&self) -> u16 {
        self.u16(2)
    }
    pub fn form(&self) -> u8 {
        self.0[4]
    }
    pub fn level_min(&self) -> u8 {
        self.0[5]
    }
    pub fn level_max(&self) -> u8 {
        self.0[6]
    }
    pub fn ability(&self) -> u8 {
        self.0[7]
    }
    pub fn shiny(&self) -> u8 {
        self.0[8]
    }
    pub fn gender(&self) -> u8 {
        self.0[9]
    }
    pub fn nature(&self) -> u8 {
        self.0[10]
    }
    pub fn ball(&self) -> u8 {
        self.0[11]
    }
    pub fn location(&self) -> u16 {
        self.u16(12)
    }
    pub fn egg_location(&self) -> u16 {
        self.u16(14)
    }
    pub fn held_item(&self) -> u16 {
        self.u16(16)
    }
    pub fn flags(&self) -> u16 {
        self.u16(18)
    }
    pub fn has(&self, f: u16) -> bool {
        self.flags() & f != 0
    }
    pub fn moves(&self) -> [u16; 4] {
        core::array::from_fn(|i| self.u16(20 + 2 * i))
    }
    pub fn ground_tile(&self) -> u32 {
        self.u32(28)
    }
    pub fn pid(&self) -> u32 {
        self.u32(32)
    }
    pub fn tid(&self) -> u16 {
        self.u16(36)
    }
    pub fn sid(&self) -> u16 {
        self.u16(38)
    }
    pub fn ot_gender(&self) -> u8 {
        self.0[40]
    }
    pub fn met_level(&self) -> u8 {
        self.0[41]
    }
    pub fn met_location(&self) -> u16 {
        self.u16(42)
    }
    /// HP, Atk, Def, Spe, SpA, SpD.
    pub fn ivs(&self) -> [u8; 6] {
        core::array::from_fn(|i| self.0[44 + i])
    }
    /// Cool, Beauty, Cute, Smart, Tough, Sheen.
    pub fn contest(&self) -> [u8; 6] {
        core::array::from_fn(|i| self.0[50 + i])
    }
    pub fn course(&self) -> u8 {
        self.0[56]
    }
    pub fn slot_number(&self) -> u8 {
        self.0[57]
    }
    pub fn magnet_pull(&self) -> (u8, u8) {
        (self.0[58], self.0[59])
    }
    pub fn static_slot(&self) -> (u8, u8) {
        (self.0[60], self.0[61])
    }
    pub fn slot_type(&self) -> u8 {
        self.0[62]
    }
    pub fn area_rate(&self) -> u8 {
        self.0[63]
    }
    pub fn area(&self) -> Option<u16> {
        Some(self.u16(64)).filter(|&a| a != NO_AREA)
    }
}

#[derive(Clone, Copy)]
pub struct Area<'a>(&'a [u8]);

impl Area<'_> {
    pub fn location(&self) -> u16 {
        u16::from_le_bytes([self.0[0], self.0[1]])
    }
    pub fn slot_type(&self) -> u8 {
        self.0[2]
    }
    pub fn rate(&self) -> u8 {
        self.0[3]
    }
    pub fn ground_tile(&self) -> u32 {
        u32::from_le_bytes([self.0[4], self.0[5], self.0[6], self.0[7]])
    }
    pub fn version(&self) -> u8 {
        self.0[8]
    }
    pub fn flags(&self) -> u16 {
        u16::from_le_bytes([self.0[9], self.0[10]])
    }
}

/// A wild slot or a template, with one accessor set.
#[derive(Clone, Copy)]
pub enum Encounter<'a> {
    Slot { slot: &'a [u8], area: Area<'a> },
    Template(Template<'a>),
}

macro_rules! pick {
    ($self:ident, $slot:expr, $area:expr, $tmpl:expr) => {
        match $self {
            Encounter::Slot { slot, area } => {
                let (_s, _a) = (slot, area);
                #[allow(clippy::redundant_closure_call)]
                ($slot)(_s, _a)
            }
            Encounter::Template(t) => ($tmpl)(t),
        }
    };
}

impl Encounter<'_> {
    pub fn kind(&self) -> Option<EncounterKind> {
        pick!(
            self,
            |s: &[u8], _: &Area| EncounterKind::from_u8(s[0]),
            (),
            |t: &Template| t.kind()
        )
    }
    pub fn is_slot(&self) -> bool {
        matches!(self, Encounter::Slot { .. })
    }
    pub fn species(&self) -> u16 {
        pick!(
            self,
            |s: &[u8], _: &Area| u16::from_le_bytes([s[2], s[3]]),
            (),
            |t: &Template| t.species()
        )
    }
    pub fn form(&self) -> u8 {
        pick!(self, |s: &[u8], _: &Area| s[1], (), |t: &Template| t.form())
    }
    pub fn level_min(&self) -> u8 {
        pick!(self, |s: &[u8], _: &Area| s[4], (), |t: &Template| t
            .level_min())
    }
    pub fn level_max(&self) -> u8 {
        pick!(self, |s: &[u8], _: &Area| s[5], (), |t: &Template| t
            .level_max())
    }
    pub fn version(&self) -> u8 {
        pick!(
            self,
            |_: &[u8], a: &Area| a.version(),
            (),
            |t: &Template| t.version()
        )
    }
    pub fn location(&self) -> u16 {
        pick!(
            self,
            |_: &[u8], a: &Area| a.location(),
            (),
            |t: &Template| t.location()
        )
    }
    pub fn ground_tile(&self) -> u32 {
        pick!(
            self,
            |_: &[u8], a: &Area| a.ground_tile(),
            (),
            |t: &Template| t.ground_tile()
        )
    }
    pub fn ball(&self) -> u8 {
        pick!(self, |s: &[u8], _: &Area| s[15], (), |t: &Template| t
            .ball())
    }
    pub fn ability(&self) -> u8 {
        pick!(self, |s: &[u8], _: &Area| s[16], (), |t: &Template| t
            .ability())
    }
    pub fn shiny(&self) -> u8 {
        pick!(self, |s: &[u8], _: &Area| s[17], (), |t: &Template| t
            .shiny())
    }
    pub fn flags(&self) -> u16 {
        match self {
            Encounter::Slot { slot, area } => {
                let mut f = area.flags() & flag::CORONET_FEEBAS;
                if slot[11] & SLOT_RANDOM_FORM != 0 {
                    f |= flag::RANDOM_UNSPECIFIC_FORM;
                }
                if slot[11] & SLOT_HIDDEN_GROTTO != 0 {
                    f |= flag::HIDDEN_GROTTO;
                }
                f
            }
            Encounter::Template(t) => t.flags(),
        }
    }
    pub fn has(&self, f: u16) -> bool {
        self.flags() & f != 0
    }
    pub fn slot_type(&self) -> u8 {
        pick!(self, |s: &[u8], _: &Area| s[14], (), |t: &Template| t
            .slot_type())
    }
    pub fn slot_number(&self) -> u8 {
        pick!(self, |s: &[u8], _: &Area| s[6], (), |t: &Template| t
            .slot_number())
    }
    pub fn magnet_pull(&self) -> (u8, u8) {
        pick!(
            self,
            |s: &[u8], _: &Area| (s[7], s[8]),
            (),
            |t: &Template| t.magnet_pull()
        )
    }
    pub fn static_slot(&self) -> (u8, u8) {
        pick!(
            self,
            |s: &[u8], _: &Area| (s[9], s[10]),
            (),
            |t: &Template| t.static_slot()
        )
    }
    /// Gen 4 slot level after Pressure/Hustle/Vital Spirit (PKHeX `EncounterSlot4.PressureLevel`).
    pub fn pressure_level(&self) -> u8 {
        match self {
            Encounter::Slot { slot, .. } => slot[18],
            Encounter::Template(t) => t.level_max(),
        }
    }

    /// PKHeX `EncounterSlot4.CanUseRadar`.
    pub fn can_radar(&self) -> bool {
        matches!(self, Encounter::Slot { slot, .. } if slot[11] & SLOT_CAN_RADAR != 0)
    }

    pub fn area_rate(&self) -> u8 {
        pick!(self, |_: &[u8], a: &Area| a.rate(), (), |t: &Template| t
            .area_rate())
    }
    pub fn area(&self) -> Option<Area<'_>> {
        match self {
            Encounter::Slot { area, .. } => Some(*area),
            Encounter::Template(_) => None,
        }
    }
    /// Full template fields (statics, gifts, trades); `None` for wild slots.
    pub fn template(&self) -> Option<Template<'_>> {
        match self {
            Encounter::Template(t) => Some(*t),
            Encounter::Slot { .. } => None,
        }
    }
}

impl<'a> Encounter<'a> {
    /// The template's bytes inside the blob (for blob-relative lookups).
    pub fn template_bytes(&self) -> Option<&'a [u8]> {
        match self {
            Encounter::Template(t) => Some(t.0),
            Encounter::Slot { .. } => None,
        }
    }
}

#[derive(Clone, Copy)]
pub struct EncounterData<'a> {
    blob: Blob<'a>,
}

impl<'a> EncounterData<'a> {
    pub fn load(data: &'a [u8]) -> Result<Self, BlobError> {
        Ok(Self {
            blob: Blob::expect(data, BlobKind::Encounters)?,
        })
    }

    pub fn template(&self, i: usize) -> Option<Template<'a>> {
        self.blob
            .bytes(table::TEMPLATES)
            .get(i * TEMPLATE_LEN..(i + 1) * TEMPLATE_LEN)
            .map(Template)
    }

    /// Fixed moves of a Gen 3 swarm slot.
    pub fn swarm_moves(&self, e: &Encounter<'a>) -> Option<[u16; 4]> {
        let Encounter::Slot { slot, .. } = e else {
            return None;
        };
        if slot[0] != EncounterKind::Slot3Swarm as u8 {
            return None;
        }
        let b = self.blob.bytes(table::SWARM_MOVES);
        let r = b.get(slot[19] as usize * 8..slot[19] as usize * 8 + 8)?;
        Some(core::array::from_fn(|i| {
            u16::from_le_bytes([r[2 * i], r[2 * i + 1]])
        }))
    }

    /// Fixed nickname and OT name (UTF-16 units) of a trade template for a language ID, if the
    /// template has them (PKHeX `EncounterTrade4PID.Nicknames` / `TrainerNames`).
    pub fn trade_names(&self, e: &Encounter<'a>, language: u8) -> Option<(U16s<'a>, U16s<'a>)> {
        let t = e.template_bytes()?;
        let base = self.blob.bytes(table::TEMPLATES).as_ptr() as usize;
        let i = (t.as_ptr() as usize).checked_sub(base)? / TEMPLATE_LEN;
        let rec = Indexed(self.blob.bytes(table::TRADE_NAMES)).get(i);
        let mut o = 0usize;
        let mut read = |rec: &'a [u8]| -> Option<U16s<'a>> {
            let n = *rec.get(o)? as usize * 2;
            let s = rec.get(o + 1..o + 1 + n)?;
            o += 1 + n;
            Some(U16s(s))
        };
        let mut found = None;
        for lang in 0..=language {
            let nick = read(rec)?;
            let ot = read(rec)?;
            if lang == language {
                found = Some((nick, ot));
            }
        }
        found
    }

    pub fn slot(&self, i: usize) -> Option<Encounter<'a>> {
        let slot = self
            .blob
            .bytes(table::SLOTS)
            .get(i * SLOT_LEN..(i + 1) * SLOT_LEN)?;
        let area = self.area(u16::from_le_bytes([slot[12], slot[13]]))?;
        Some(Encounter::Slot { slot, area })
    }

    pub fn area(&self, i: u16) -> Option<Area<'a>> {
        let i = i as usize;
        self.blob
            .bytes(table::AREAS)
            .get(i * AREA_LEN..(i + 1) * AREA_LEN)
            .map(Area)
    }

    pub fn group_count(&self) -> usize {
        Indexed(self.blob.bytes(table::GROUP_NAMES)).count()
    }

    /// Group name (PKHeX field name, e.g. `SlotsPt`) as ASCII bytes.
    pub fn group_name(&self, g: usize) -> &'a [u8] {
        Indexed(self.blob.bytes(table::GROUP_NAMES)).get(g)
    }

    /// (index range, is slot table)
    pub fn group_range(&self, g: usize) -> (core::ops::Range<usize>, bool) {
        let r = self.blob.bytes(table::GROUP_RANGES);
        let Some(b) = r.get(8 * g..8 * g + 8) else {
            return (0..0, false);
        };
        let start = u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize;
        let len = u32::from_le_bytes([b[4], b[5], b[6], b[7]]);
        let slots = len & RANGE_SLOTS != 0;
        (start..start + (len & !RANGE_SLOTS) as usize, slots)
    }

    pub fn group_encounters(&self, g: usize) -> impl Iterator<Item = Encounter<'a>> + 'a {
        let this = *self;
        let (range, slots) = self.group_range(g);
        range.filter_map(move |i| {
            if slots {
                this.slot(i)
            } else {
                this.template(i).map(Encounter::Template)
            }
        })
    }

    /// The slots of a slot group met at `location`, in table order. Scans only each record's
    /// area index, since a group has thousands of slots and the legality check needs a few.
    pub fn slots_at(&self, name: &[u8], location: u16) -> impl Iterator<Item = Encounter<'a>> + 'a {
        let this = *self;
        let g = (0..self.group_count()).find(|&g| self.group_name(g) == name);
        let range = g.map_or(0..0, |g| self.group_range(g).0);
        let bytes = self.blob.bytes(table::SLOTS);
        let mut last = None;
        let mut here = false;
        range.filter_map(move |i| {
            let o = i * SLOT_LEN + 12;
            let area = u16::from_le_bytes([*bytes.get(o)?, *bytes.get(o + 1)?]);
            if last != Some(area) {
                last = Some(area);
                here = this.area(area).is_some_and(|a| a.location() == location);
            }
            if here {
                this.slot(i)
            } else {
                None
            }
        })
    }

    pub fn group(&self, name: &[u8]) -> impl Iterator<Item = Encounter<'a>> + 'a {
        let this = *self;
        let g = (0..self.group_count()).find(|&g| self.group_name(g) == name);
        g.into_iter().flat_map(move |g| this.group_encounters(g))
    }
}
