//! Gen 4 Mystery Gift encounter matching. Ported from PKHeX `MysteryGifts/PCD.cs`
//! (`IsMatchExact`, `IsMatchPartial`), `MysteryGifts/PGT.cs` (`IsRangerManaphy`,
//! `IsCompatible`, `IsG4ManaphyPIDValid`, `Shiny`) and the Event states of
//! `EncounterEnumerator4`.

use alloc::vec::Vec;

use super::evo::EvoCriteria;
use crate::gift::pcd::{Gift4, Gifts4};
use crate::pkm::Pkm;
use crate::rng::finder::PidType;
use crate::rng::xdrng::arng;

const MANAPHY: u16 = 490;
const MILOTIC: u16 = 350;
const LINK_TRADE4: u16 = 2002;
const RANGER4: u16 = 3001;

/// PKHeX `PGT.IsRangerManaphy` (format 4).
pub fn is_ranger_manaphy(pk: &Pkm) -> bool {
    if pk.species() != MANAPHY {
        return false;
    }
    if pk.language() >= 8 {
        return false;
    }
    let egg = pk.egg_location();
    if !pk.is_egg() {
        return matches!(egg, LINK_TRADE4 | RANGER4);
    }
    egg == RANGER4 && matches!(pk.met_location(), LINK_TRADE4 | 0)
}

fn decoded_ot(pk: &Pkm, buf: &mut [u16; 8]) -> usize {
    crate::text::decode4(&pk.ot_name_raw(), buf)
}

/// PKHeX `PCD.IsMatchExact` for a PK4 in the Gen 4 format. `t` is the gift's template PK4 and
/// `t_level` its `CurrentLevel` (from EXP and the species' growth rate).
pub fn match_exact(t: &Pkm, t_level: u8, pk: &Pkm, evo: &EvoCriteria) -> bool {
    if !t.is_egg() {
        if t.tid() != pk.tid() || t.sid() != pk.sid() {
            return false;
        }
        let (mut a, mut b) = ([0u16; 8], [0u16; 8]);
        let (na, nb) = (decoded_ot(t, &mut a), decoded_ot(pk, &mut b));
        if a[..na] != b[..nb] {
            return false;
        }
        if t.ot_gender() != pk.ot_gender() {
            return false;
        }
        if t.language() != 0 && t.language() != pk.language() {
            return false;
        }
        if t.egg_location() as u32 + 3000 != pk.met_location() as u32 {
            return false;
        }
        if t_level != pk.met_level() {
            return false;
        }
    } else {
        if t.egg_location() as u32 + 3000 != pk.egg_location() as u32
            && pk.egg_location() != LINK_TRADE4
        {
            return false;
        }
        if t_level != pk.met_level() {
            return false;
        }
    }
    if t.form() != evo.form && !super::enc4::form_changeable(t.species()) {
        return false;
    }
    if t.ball() != pk.ball() {
        return false;
    }
    if t.species() == MILOTIC && pk.contest().iter().zip(t.contest()).any(|(&c, i)| c < i) {
        return false;
    }
    if t.pid() == 1 {
        // Random PID, never shiny; gender is not checked (the KOR Arcanine can be either).
        !pk.is_shiny()
    } else {
        t.gender() == pk.gender()
    }
}

/// PKHeX `PCD.IsMatchPartial`: the card's stored version must be the origin game.
pub fn is_partial(t: &Pkm, pk: &Pkm) -> bool {
    t.version() != pk.version()
}

/// PKHeX `PGT.IsCompatible` for a regular gift.
pub fn pcd_compatible(t: &Pkm, kind: PidType) -> bool {
    (t.pid() != 1 && kind == PidType::G5MgShiny) || kind == PidType::None
}

/// PKHeX `PGT.IsG4ManaphyPIDValid` (Ranger Manaphy `IsCompatible`).
pub fn manaphy_compatible(kind: PidType, pk: &Pkm) -> bool {
    let traded_egg = pk.egg_location() == LINK_TRADE4;
    if kind == PidType::Method1 {
        if pk.is_egg() {
            return !pk.is_shiny();
        }
        return traded_egg || !pk.is_shiny();
    }
    if kind != PidType::G4MgAntiShiny {
        return false;
    }
    if !pk.is_egg() && traded_egg {
        return true;
    }
    let pid = arng::prev(pk.pid());
    let tsv = (pk.tid() ^ pk.sid()) >> 3;
    let psv = ((pid & 0xFFFF) as u16 ^ (pid >> 16) as u16) >> 3;
    tsv == psv
}

/// PKHeX `Shiny` of a gift: 0 Random, 1 Never, 2 Always.
pub fn shiny_rule(g: &Gift4, t: &Pkm) -> u8 {
    if g.is_egg() {
        0
    } else if t.pid() == 1 || !t.is_shiny() {
        1
    } else {
        2
    }
}

/// The Event states of `EncounterEnumerator4` for a fateful PK4: gifts that match exactly
/// (rating Match), in database order, and the first partial match.
pub struct EventSearch<'a> {
    pub matches: Vec<Gift4<'a>>,
    pub partial: Option<Gift4<'a>>,
}

pub fn search<'a>(
    gifts: &Gifts4<'a>,
    pk: &Pkm,
    chain: &[EvoCriteria],
    level_of: impl Fn(&Pkm) -> u8,
) -> EventSearch<'a> {
    let mut out = EventSearch {
        matches: Vec::new(),
        partial: None,
    };
    for g in gifts.iter() {
        let species = g.species();
        let Some(evo) = chain.iter().find(|e| e.species == species) else {
            continue;
        };
        let t = g.pk();
        if !match_exact(&t, level_of(&t), pk, evo) {
            continue;
        }
        if is_partial(&t, pk) {
            if out.partial.is_none() {
                out.partial = Some(g);
            }
        } else {
            out.matches.push(g);
        }
    }
    out
}
