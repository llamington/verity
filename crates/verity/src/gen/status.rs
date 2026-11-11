//! Outcome codes of the Pokémon generators (wild, eggs, closest IVs), shared by the front ends.

use super::api4::Error;
use super::wild4::GenError;

/// Generator outcomes. The values are the DS's `verity_gen_status` codes.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GenStatus {
    Ok = 0,
    Unsupported = 1,
    NoEncounter = 2,
    NotInEncounter = 3,
    LevelOutsideSlot = 4,
    NatureConflictsWithSynchronize = 5,
    ShinyImpossibleForCuteCharm = 6,
    GenderImpossible = 7,
    CuteCharmNeedsGenderedSpecies = 8,
    FormUnavailable = 9,
    RadarNotAllowedHere = 10,
    MunchlaxTreeUnavailable = 11,
    NoFrameFound = 12,
    BallNotAllowed = 13,
    AbilityUnavailable = 14,
    MovesIllegal = 15,
    SlotUnavailable = 16,
    Unsafe = 17,
    Internal = 18,
    NotWild = 19,
    ShinyInGrotto = 20,
}

/// The texts, NUL-terminated for the C ABI, by code.
const TEXTS: [&str; 21] = [
    "OK\0",
    "Only Gen 4 wild encounters for now\0",
    "No wild encounter in this game\0",
    "Not in that encounter\0",
    "Level outside the slot\0",
    "Nature conflicts with Synchronize\0",
    "Shiny impossible with Cute Charm\0",
    "Gender impossible here\0",
    "Cute Charm needs a gendered species\0",
    "Form not available here\0",
    "Poke Radar not usable here\0",
    "Munchlax not in this tree for this ID\0",
    "No legal frame found (try again)\0",
    "Ball not allowed\0",
    "Ability not available\0",
    "Moves not legal\0",
    "Slot not available\0",
    "Rejected by the game-safe check\0",
    "Internal error\0",
    "Only for Gen 4 wild catches\0",
    "Hidden Grotto Pokemon are never shiny\0",
];

impl GenStatus {
    /// What to tell the user.
    pub fn text(self) -> &'static str {
        let t = TEXTS[self as usize];
        &t[..t.len() - 1]
    }

    /// [`Self::text`] for a raw code, NUL-terminated (unknown codes read as internal errors).
    pub fn c_text(code: u8) -> &'static str {
        TEXTS
            .get(code as usize)
            .copied()
            .unwrap_or(TEXTS[GenStatus::Internal as usize])
    }
}

impl From<Error> for GenStatus {
    fn from(e: Error) -> Self {
        use GenStatus as S;
        match e {
            Error::Gen(g) => match g {
                GenError::LevelOutsideSlot => S::LevelOutsideSlot,
                GenError::NatureConflictsWithSynchronize => S::NatureConflictsWithSynchronize,
                GenError::ShinyImpossibleForCuteCharm => S::ShinyImpossibleForCuteCharm,
                GenError::GenderImpossible => S::GenderImpossible,
                GenError::CuteCharmNeedsGenderedSpecies => S::CuteCharmNeedsGenderedSpecies,
                GenError::FormUnavailable => S::FormUnavailable,
                GenError::RadarNotAllowedHere => S::RadarNotAllowedHere,
                GenError::MunchlaxTreeUnavailable => S::MunchlaxTreeUnavailable,
                GenError::BudgetExhausted => S::NoFrameFound,
            },
            Error::VersionUnsupported => S::Unsupported,
            Error::NoEncounter => S::NoEncounter,
            Error::NotInEncounter => S::NotInEncounter,
            Error::BallNotAllowed => S::BallNotAllowed,
            Error::AbilityUnavailable => S::AbilityUnavailable,
            Error::MovesIllegal => S::MovesIllegal,
            Error::PartyGap | Error::Save(_) | Error::WrongFormat => S::SlotUnavailable,
            Error::Unsafe(_) => S::Unsafe,
            Error::Illegal(_) => S::Internal,
            Error::NotWild => S::NotWild,
        }
    }
}
