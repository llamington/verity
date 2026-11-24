//! The encounters the add-Pokémon wizards offer per game: wild slots, then the static
//! encounters, gifts and in-game trades the generators can build.

use crate::data::encounters::{Encounter, EncounterData, EncounterKind, flag};

/// The encounter group holding `version`'s wild slots (10 D, 11 P, 12 Pt, 7 HG, 8 SS, 20 W,
/// 21 B, 22 W2, 23 B2).
pub fn slots_group(version: u8) -> Option<&'static [u8]> {
    match version {
        10 => Some(b"SlotsD"),
        11 => Some(b"SlotsP"),
        12 => Some(b"SlotsPt"),
        7 => Some(b"SlotsHG"),
        8 => Some(b"SlotsSS"),
        20 => Some(b"SlotsW"),
        21 => Some(b"SlotsB"),
        22 => Some(b"SlotsW2"),
        23 => Some(b"SlotsB2"),
        _ => None,
    }
}

/// `version`'s static encounter and in-game trade groups (PKHeX `Encounters4DPPt`,
/// `Encounters4HGSS`, `Encounters5BW`, `Encounters5B2W2`). `TradeGift_B2W2` repeats
/// `TradeGift_B2`/`TradeGift_W2`, so only it is used.
fn template_groups(version: u8) -> &'static [&'static [u8]] {
    match version {
        10 => &[b"StaticD", b"StaticDP", b"StaticDPPt"],
        11 => &[b"StaticP", b"StaticDP", b"StaticDPPt"],
        12 => &[b"StaticPt", b"StaticDPPt"],
        7 => &[b"StaticHG", b"Encounter_HGSS"],
        8 => &[b"StaticSS", b"Encounter_HGSS"],
        20 => &[b"StaticW", b"Encounter_BW", b"TradeGift_W", b"TradeGift_BW"],
        21 => &[b"StaticB", b"Encounter_BW", b"TradeGift_B", b"TradeGift_BW"],
        22 => &[b"StaticW2", b"Encounter_B2W2_Regular", b"TradeGift_B2W2"],
        23 => &[b"StaticB2", b"Encounter_B2W2_Regular", b"TradeGift_B2W2"],
        _ => &[],
    }
}

/// Whether a template's PKHeX `GameVersion` includes `version` (63 DP, 64 DPPt, 65 HGSS,
/// 66 BW, 67 B2W2).
pub fn template_in_game(template_version: u8, version: u8) -> bool {
    template_version == version
        || match template_version {
            63 => matches!(version, 10 | 11),
            64 => matches!(version, 10..=12),
            65 => matches!(version, 7 | 8),
            66 => matches!(version, 20 | 21),
            67 => matches!(version, 22 | 23),
            _ => false,
        }
}

/// Templates the generators can build: no gift eggs, and no HG/SS Spiky-eared Pichu (its PID
/// comes from the Pokéwalker method).
fn offered_template(e: &Encounter, version: u8) -> bool {
    const PICHU: u16 = 172;
    let buildable = match e.kind() {
        Some(EncounterKind::Static4) => e.species() != PICHU,
        Some(EncounterKind::Static5 | EncounterKind::Trade5BW | EncounterKind::Trade5B2W2) => true,
        _ => false,
    };
    buildable
        && !e.has(flag::EGG)
        && e.template()
            .is_some_and(|t| t.egg_location() == 0 && template_in_game(t.version(), version))
}

/// What the wizard offers in `version`: its wild slots, then its static encounters, gifts and
/// in-game trades. Indices into this are what [`super::wizard::WildRequest::encounter`] takes.
pub fn encounters<'a>(
    data: &EncounterData<'a>,
    version: u8,
) -> impl Iterator<Item = Encounter<'a>> + 'a + use<'a> {
    let d = *data;
    let slots = slots_group(version)
        .into_iter()
        .flat_map(move |g| d.group(g));
    let templates = template_groups(version)
        .iter()
        .flat_map(move |g| d.group(g))
        .filter(move |e| offered_template(e, version));
    slots.chain(templates)
}

/// How an offered encounter is met. The values are the DS's `verity_encounter.encounter_type`.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Offer {
    Slot = 0,
    Static = 1,
    /// PKHeX `EncounterStatic4.Gift`/`EncounterStatic5.Gift`: given rather than battled, so its
    /// ball is a Poké Ball.
    Gift = 2,
    /// Met on any of its routes; generated where the template places it.
    Roaming = 3,
    Trade = 4,
}

pub fn offer(e: &Encounter) -> Offer {
    const BALL_POKE: u8 = 4;
    match e.kind() {
        _ if e.is_slot() => Offer::Slot,
        Some(EncounterKind::Trade5BW | EncounterKind::Trade5B2W2) => Offer::Trade,
        _ if e.has(flag::ROAMING) => Offer::Roaming,
        _ if e.ball() == BALL_POKE => Offer::Gift,
        _ => Offer::Static,
    }
}
