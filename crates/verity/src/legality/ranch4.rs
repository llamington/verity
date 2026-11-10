//! My Pokémon Ranch gifts, the trades Hayley offers on Diamond. Table generated from PKHeX 25.12.21
//! `Legality/Encounters/Data/Gen4/Encounters4DPPt.cs` (`RanchGifts`); matching from
//! `Templates/Gen4/EncounterTrade4RanchGift.cs`.

use super::enc4::MatchPkm;
use super::evo::EvoCriteria;

/// One gift. `fateful` gifts (Mew, Phione) have a random, non-shiny PID.
#[derive(Clone, Copy, Debug)]
pub struct RanchGift {
    pub pid: u32,
    pub species: u16,
    pub met_level: u8,
    pub level: u8,
    pub moves: [u16; 4],
    pub sid: u16,
    pub ot_gender: u8,
    pub location: u16,
    pub gender: u8,
    /// 0 either, 1 first, 2 second.
    pub ability: u8,
    pub ball: u8,
    pub form: u8,
    pub egg_location: u16,
    pub fateful: bool,
}

/// Every gift has this trainer ID.
const TID: u16 = 1000;

#[allow(clippy::too_many_arguments)]
const fn r(
    pid: u32,
    species: u16,
    met_level: u8,
    level: u8,
    moves: [u16; 4],
    sid: u16,
    ot_gender: u8,
    location: u16,
    gender: u8,
    ability: u8,
    ball: u8,
    form: u8,
    egg_location: u16,
    fateful: bool,
) -> RanchGift {
    RanchGift {
        pid,
        species,
        met_level,
        level,
        moves,
        sid,
        ot_gender,
        location,
        gender,
        ability,
        ball,
        form,
        egg_location,
        fateful,
    }
}

#[rustfmt::skip]
pub static RANCH_GIFTS: [RanchGift; 22] = [
    r(323975838, 25, 18, 20, [447, 85, 148, 104], 19840, 1, 68, 0, 1, 4, 0, 0, false), // Pikachu
    r(323977664, 37, 16, 30, [412, 109, 53, 219], 21150, 1, 3000, 0, 1, 4, 0, 0, false), // Vulpix
    r(323975579, 77, 13, 16, [36, 33, 39, 52], 1123, 1, 3000, 0, 2, 4, 0, 0, false), // Ponyta
    r(323975564, 108, 34, 40, [76, 111, 14, 205], 3050, 1, 77, 0, 1, 4, 0, 0, false), // Lickitung
    r(323977579, 114, 1, 1, [437, 438, 79, 246], 49497, 1, 3000, 1, 2, 4, 0, 0, false), // Tangela
    r(323977675, 133, 16, 30, [363, 270, 98, 247], 47710, 1, 68, 0, 2, 4, 0, 0, false), // Eevee
    r(323977588, 142, 20, 50, [363, 89, 444, 332], 43066, 1, 94, 0, 1, 4, 0, 0, false), // Aerodactyl
    r(232975554, 193, 22, 45, [318, 95, 246, 138], 42301, 1, 52, 0, 1, 5, 0, 0, false), // Yanma
    r(323975570, 241, 16, 48, [208, 215, 360, 359], 2707, 1, 3000, 1, 1, 4, 0, 0, false), // Miltank
    r(323975563, 285, 22, 45, [402, 147, 206, 78], 2788, 1, 3000, 0, 2, 5, 0, 0, false), // Shroomish
    r(323975559, 320, 30, 45, [156, 323, 133, 58], 27046, 1, 38, 0, 2, 4, 0, 0, false), // Wailmer
    r(323977657, 360, 1, 1, [204, 150, 227, 0], 1788, 1, 4, 0, 2, 4, 0, 2000, false), // Wynaut
    r(323975563, 397, 2, 23, [355, 17, 283, 18], 59298, 1, 16, 0, 2, 4, 0, 0, false), // Staravia
    r(323970584, 415, 5, 20, [230, 16, 0, 0], 54140, 1, 20, 1, 1, 4, 0, 0, false), // Combee
    r(323977539, 417, 9, 10, [447, 45, 351, 98], 18830, 1, 20, 1, 2, 4, 0, 0, false), // Pachirisu
    r(323974107, 422, 20, 25, [363, 352, 426, 104], 39272, 1, 28, 0, 2, 4, 1, 0, false), // Shellos
    r(323977566, 427, 10, 16, [204, 193, 409, 98], 31045, 1, 3000, 1, 1, 4, 0, 0, false), // Buneary
    r(323975579, 453, 22, 31, [310, 207, 426, 389], 41342, 1, 52, 0, 2, 5, 0, 0, false), // Croagunk
    r(323977566, 456, 15, 35, [213, 352, 219, 392], 48348, 1, 20, 1, 1, 4, 0, 0, false), // Finneon
    r(323975582, 459, 32, 41, [452, 420, 275, 59], 23360, 1, 31, 0, 1, 4, 0, 0, false), // Snover
    r(0, 151, 50, 50, [235, 216, 95, 100], 59228, 1, 3000, 2, 0, 16, 0, 0, true), // Mew
    r(0, 489, 1, 50, [447, 240, 156, 57], 9248, 1, 3000, 2, 0, 16, 0, 3000, true), // Phione
];

/// `IsTrainerMatch`: Hayley's name in the Pokémon's language (none exists for Korean).
pub fn trainer_matches(language: u8, ot: &[u16]) -> bool {
    let name: &str = match language {
        1 => "ユカリ",
        2 => "Hayley",
        3 => "EULALIE",
        4 => "GIULIA",
        5 => "EUKALIA",
        7 => "Eulalia",
        _ => return false,
    };
    name.encode_utf16().eq(ot.iter().copied())
}

/// `EncounterTrade4RanchGift.IsMatchExact` (format 4).
pub fn is_match_exact(g: &RanchGift, pk: &MatchPkm, evo: &EvoCriteria) -> bool {
    let a = &pk.analyze;
    let shiny_ok = if g.fateful {
        !pk.is_shiny
    } else {
        a.pid == g.pid
    };
    let (level_ok, location_ok) = if pk.format5 {
        (evo.level_max >= g.level, true)
    } else {
        (a.met_level == g.met_level, pk.met_location == g.location)
    };
    level_ok
        && a.tid == TID
        && a.sid == g.sid
        && a.gender == g.gender
        && shiny_ok
        && location_ok
        && !(evo.form != g.form && !super::enc4::form_changeable(g.species))
        && a.ot_gender == g.ot_gender
        && pk.egg_location == g.egg_location
        && !pk.is_egg
}
