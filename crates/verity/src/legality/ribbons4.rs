//! Ribbon checks for a Gen 4-origin PK4 in the Gen 4 format. Ported from PKHeX
//! `Verifiers/Ribbons/RibbonVerifier.cs` (the PK4 ribbon sets: `RibbonVerifierEvent3`,
//! `RibbonVerifierEvent4`, `RibbonVerifierUnique3`, `RibbonVerifierCommon3`,
//! `RibbonVerifierUnique4`, `RibbonVerifierCommon4`) and the `RibbonRules` they use. A Gen 4
//! origin has never visited Gen 3, Gen 6 or BD/SP, which fixes most of the rules.
//!
//! PK4 ribbon bytes (PKHeX `PK4.cs`, bit order as listed):
//! - 0x24: Champion Sinnoh, Ability, Ability Great/Double/Multi/Pair/World, Alert
//! - 0x25: Shock, Downcast, Careless, Relax, Snooze, Smile, Gorgeous, Royal
//! - 0x26: Gorgeous Royal, Footprint, Record, Event, Legend, Champion World, Birthday, Special
//! - 0x27: Souvenir, Wishing, Classic, Premier (bits 4-7 unused, unchecked)
//! - 0x3C-0x3E: Hoenn contest (Cool, Beauty, Cute, Smart, Tough x normal/Super/Hyper/Master),
//!   then Champion G3, Winning, Victory, Artist
//! - 0x3F: Effort, Champion Battle/Regional/National, Country, National, Earth, World
//! - 0x60-0x62: Sinnoh contest (Cool, Beauty, Cute, Smart, Tough x normal/Great/Ultra/Master)

const DITTO: u16 = 132;
const UNOWN: u16 = 201;
const PICHU: u16 = 172;

/// PKHeX `RibbonRules.BattleFrontierBanlist` (Gen 4 species).
const FRONTIER_BANNED: [u16; 18] = [
    150, 151, 249, 250, 251, 382, 383, 384, 385, 386, 483, 484, 487, 489, 490, 491, 492, 493,
];

/// Inputs to the ribbon check.
pub struct RibbonArgs {
    /// The PK4's ribbon bytes 0x24..0x28, 0x3C..0x40 and 0x60..0x63.
    pub sinnoh: [u8; 4],
    pub hoenn: [u8; 4],
    pub contest4: [u8; 3],
    pub is_egg: bool,
    /// The encounter's event ribbons (a gift's template, else none), in `event_ribbons4` form.
    pub event: [u8; 3],
    /// `EvolutionHistory.HasVisitedGen4`.
    pub gen4: bool,
    /// `evos.Gen4[0]`: the current species and form.
    pub species: u16,
    pub form: u8,
    /// The encounter is the spiky-eared Pichu static (`EncounterStatic4 { Pichu, Form 1 }`).
    pub spiky_static: bool,
    /// A Gen 3 origin (`EvolutionHistory.HasVisitedGen3`).
    pub gen3: Option<Gen3Ribbons>,
    /// The encounter is from Gen 3 (`enc.Generation`), matched or not.
    pub gen3_origin: bool,
}

/// What the Gen 3 ribbons depend on.
#[derive(Clone, Copy)]
pub struct Gen3Ribbons {
    /// `evos.Gen3[0].Species`.
    pub species: u16,
    /// `enc.LevelMin`.
    pub encounter_level: u8,
}

/// `AddMissingContest3`: a rank needs every rank below it.
fn contest3_ranks_valid(hoenn: [u8; 3]) -> bool {
    let bits = hoenn[0] as u32 | (hoenn[1] as u32) << 8 | (hoenn[2] as u32) << 16;
    (0..5).all(|i| {
        matches!(
            (bits >> (i * 4)) & 0xF,
            0b0000 | 0b0001 | 0b0011 | 0b0111 | 0b1111
        )
    })
}

/// True when any ribbon is invalid or missing (PKHeX `RibbonVerifier` adds an Invalid line).
pub fn ribbons_invalid(a: &RibbonArgs) -> bool {
    let event = [a.sinnoh[2] & 0xE8, a.sinnoh[3] & 0x0F, a.hoenn[3] & 0xFE];
    if a.is_egg {
        // ParseEgg: only Event4 ribbons (as the encounter had them); nothing else.
        let want = [a.event[0], a.event[1], a.event[2] & 0x80];
        return event != want
            || a.sinnoh[0] != 0
            || a.sinnoh[1] != 0
            || a.sinnoh[2] & !0xE8 != 0
            || a.hoenn[..3] != [0, 0, 0]
            || a.hoenn[3] & 0x01 != 0
            || a.contest4[0] != 0
            || a.contest4[1] != 0
            || a.contest4[2] & 0x0F != 0;
    }
    // Event3 + Event4: as the encounter had them (Earth and National included: never for Gen 4).
    // Earth can be earned on Colosseum/XD Pokémon, so a Gen 3 origin may carry it.
    let earth = if a.gen3_origin { !0x40 } else { 0xFF };
    if event[0] != a.event[0] || event[1] != a.event[1] || event[2] & earth != a.event[2] & earth {
        return true;
    }
    match a.gen3 {
        Some(g) => {
            // Unique3: Winning needs a Frontier-allowed species below level 50 at some point,
            // Victory a Frontier-allowed species. Contest ribbons: each rank needs those below.
            let frontier = !FRONTIER_BANNED.contains(&g.species);
            if a.hoenn[2] & 0x20 != 0 && !(frontier && g.encounter_level <= 50) {
                return true;
            }
            if a.hoenn[2] & 0x40 != 0 && !frontier {
                return true;
            }
            if !contest3_ranks_valid([a.hoenn[0], a.hoenn[1], a.hoenn[2] & 0x0F]) {
                return true;
            }
        }
        None => {
            // Unique3 (not visited Gen 3): no Winning/Victory. Common3: no Champion G3/Artist;
            // Effort needs Gen 4. Unique4: no Hoenn contest ribbons.
            if a.hoenn[0] != 0 || a.hoenn[1] != 0 || a.hoenn[2] != 0 {
                return true;
            }
            if a.hoenn[3] & 0x01 != 0 && !a.gen4 {
                return true;
            }
        }
    }
    // Unique4: Battle Frontier ability ribbons.
    let frontier =
        a.gen4 && !(a.species == PICHU && a.form == 1) && !FRONTIER_BANNED.contains(&a.species);
    if !frontier && a.sinnoh[0] & 0x7E != 0 {
        return true;
    }
    // Unique4: Sinnoh contest ribbons, each rank needing the ones below.
    let contest =
        a.gen4 && !matches!(a.species, DITTO | UNOWN) && !(a.species == PICHU && a.form == 1);
    let ranks: [u8; 5] = [
        a.contest4[0] & 0x0F,
        a.contest4[0] >> 4,
        a.contest4[1] & 0x0F,
        a.contest4[1] >> 4,
        a.contest4[2] & 0x0F,
    ];
    for r in ranks {
        if !contest {
            if r != 0 {
                return true;
            }
        } else if !matches!(r, 0b0000 | 0b0001 | 0b0011 | 0b0111 | 0b1111) {
            return true;
        }
    }
    // Common4.
    let bit = |b: u8, n: u32| b & (1 << n) != 0;
    if bit(a.sinnoh[2], 1) && !a.gen4 {
        return true; // Footprint (format < 6 and no Gen 3/4)
    }
    if bit(a.sinnoh[2], 2) {
        return true; // Record: unobtainable
    }
    if bit(a.sinnoh[2], 4) && !a.gen4 {
        return true; // Legend
    }
    let sinnoh4 = a.gen4 && !a.spiky_static;
    if bit(a.sinnoh[0], 0) && !sinnoh4 {
        return true; // Champion Sinnoh
    }
    // Daily ribbons (Alert..Smile) need Gen 4.
    if !a.gen4 && (a.sinnoh[0] & 0x80 != 0 || a.sinnoh[1] & 0x3F != 0) {
        return true;
    }
    let (gorgeous, royal, gorgeous_royal) = (
        bit(a.sinnoh[1], 6),
        bit(a.sinnoh[1], 7),
        bit(a.sinnoh[2], 0),
    );
    if !sinnoh4 {
        if gorgeous || royal || gorgeous_royal {
            return true;
        }
    } else {
        // FlagCosmeticRequsite: Gorgeous -> Royal -> Gorgeous Royal.
        if royal && !gorgeous {
            return true;
        }
        if gorgeous_royal && !(royal && gorgeous) {
            return true;
        }
    }
    false
}
