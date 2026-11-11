//! Field edits from the editors (DS and web): applied to a copy of a stored Pokémon, which the
//! caller previews (game-safe check, legality) or writes through `safety::checked_set_pkm`.

use crate::data::Strings;
use crate::pkm::derived::exp_for_level;
use crate::pkm::Pkm;
use crate::safety::Rules;
use crate::save::{Save, Slot};

/// The fields to change; `None` leaves a field as it is.
#[derive(Clone, Debug, Default)]
pub struct Edit {
    pub held_item: Option<u16>,
    /// PP are reset to the moves' maximum and PP Ups cleared.
    pub moves: Option<[u16; 4]>,
    /// The PID that goes with a closest-legal IV spread (set before the IVs).
    pub pid: Option<u32>,
    /// HP, Atk, Def, Spe, SpA, SpD.
    pub ivs: Option<[u8; 6]>,
    pub evs: Option<[u8; 6]>,
    /// EXP is set to the level's minimum.
    pub level: Option<u8>,
    pub friendship: Option<u8>,
    pub ball: Option<u8>,
    pub met: Option<Met>,
    /// Cool, Beauty, Cute, Smart, Tough, Sheen.
    pub contest: Option<[u8; 6]>,
    /// Bytes 0x24..0x28, 0x3C..0x40, 0x60..0x64.
    pub ribbons: Option<[u8; 12]>,
    pub nickname: Option<Nickname>,
}

#[derive(Clone, Copy, Debug)]
pub struct Met {
    pub location: u16,
    pub level: u8,
    /// Years since 2000, month, day.
    pub date: [u8; 3],
}

/// Longest nickname the games store (Japanese and Korean names are 5 in game, which the
/// editors enforce).
pub const NICKNAME_MAX: usize = 10;

#[derive(Clone, Copy, Debug)]
pub enum Nickname {
    /// UTF-16, `len` units (at most [`NICKNAME_MAX`]).
    Set { text: [u16; NICKNAME_MAX], len: u8 },
    /// Back to the species name in the Pokémon's language.
    Reset,
}

impl Nickname {
    /// A nickname from UTF-16, cut to [`NICKNAME_MAX`] units.
    pub fn new(units: &[u16]) -> Self {
        let mut text = [0; NICKNAME_MAX];
        let len = units.len().min(NICKNAME_MAX);
        text[..len].copy_from_slice(&units[..len]);
        Nickname::Set {
            text,
            len: len as u8,
        }
    }
}

/// The Pokémon at `slot` with `e` applied (party stats recomputed, checksum refreshed). `None`
/// for an empty or out-of-range slot, or a nickname reset without `names` (the per-language
/// species names, `names4.bin`).
pub fn edited(
    save: &Save,
    rules: &Rules,
    slot: Slot,
    e: &Edit,
    names: Option<&Strings>,
) -> Option<Pkm> {
    if let Slot::Party(i) = slot {
        if i as usize >= save.party_count() {
            return None;
        }
    }
    let mut pk = save.pkm(slot).ok()?;
    if pk.species() == 0 {
        return None;
    }
    let pi = rules.game.species_form(pk.species(), pk.form());
    if let Some(v) = e.held_item {
        pk.set_held_item(v);
    }
    if let Some(v) = e.moves {
        // Clear PP Ups first: PP are filled to the maximum for the PP Ups in place.
        pk.set_pp_ups([0; 4]);
        pk.set_moves_with_pp(v, &rules.moves);
    }
    if let Some(v) = e.pid {
        pk.set_pid(v);
    }
    if let Some(v) = e.ivs {
        pk.set_ivs(v);
    }
    if let Some(v) = e.evs {
        pk.set_evs(v);
    }
    if let Some(v) = e.level {
        pk.set_exp(exp_for_level(v.clamp(1, 100), pi.growth()));
    }
    if let Some(v) = e.friendship {
        pk.set_friendship(v);
    }
    if let Some(v) = e.ball {
        pk.set_ball(v);
    }
    if let Some(m) = e.met {
        pk.set_met_location(m.location);
        pk.set_met_level(m.level);
        pk.set_met_date(m.date);
    }
    if let Some(v) = e.contest {
        pk.set_contest(v);
    }
    if let Some(v) = e.ribbons {
        pk.set_ribbons(v);
    }
    match e.nickname {
        Some(Nickname::Set { text, len }) => {
            pk.set_nickname(&text[..len as usize]);
            pk.set_is_nicknamed(true);
        }
        Some(Nickname::Reset) => {
            let name = names?.get(pk.language() as u16, pk.species() as usize);
            let mut buf = [0u16; NICKNAME_MAX];
            let mut n = 0;
            for c in name.iter().take(NICKNAME_MAX) {
                buf[n] = c;
                n += 1;
            }
            pk.set_nickname(&buf[..n]);
            pk.set_is_nicknamed(false);
        }
        None => {}
    }
    if pk.is_party_format() {
        pk.reset_party_stats(&pi.base_info());
    }
    pk.refresh_checksum();
    Some(pk)
}

/// The ribbons the editors list, in order, with their place in [`Edit::ribbons`]: the Sinnoh set
/// (28 bits from byte 0), the Hoenn set (32 bits from byte 4) and the Sinnoh contest set (20 bits
/// from byte 8).
#[rustfmt::skip]
pub const RIBBONS: [&str; 80] = [
    "Champion Sinnoh", "Ability", "Great Ability", "Double Ability", "Multi Ability",
    "Pair Ability", "World Ability", "Alert", "Shock", "Downcast", "Careless", "Relax", "Snooze",
    "Smile", "Gorgeous", "Royal", "Gorgeous Royal", "Footprint", "Record", "Event", "Legend",
    "Champion World", "Birthday", "Special", "Souvenir", "Wishing", "Classic", "Premier",
    "Cool (G3)", "Cool Super", "Cool Hyper", "Cool Master", "Beauty (G3)", "Beauty Super",
    "Beauty Hyper", "Beauty Master", "Cute (G3)", "Cute Super", "Cute Hyper", "Cute Master",
    "Smart (G3)", "Smart Super", "Smart Hyper", "Smart Master", "Tough (G3)", "Tough Super",
    "Tough Hyper", "Tough Master", "Champion G3", "Winning", "Victory", "Artist", "Effort",
    "Battle Champion", "Regional Champion", "National Champion", "Country", "National", "Earth",
    "World",
    "Cool (G4)", "Cool Great", "Cool Ultra", "Cool Master", "Beauty (G4)", "Beauty Great",
    "Beauty Ultra", "Beauty Master", "Cute (G4)", "Cute Great", "Cute Ultra", "Cute Master",
    "Smart (G4)", "Smart Great", "Smart Ultra", "Smart Master", "Tough (G4)", "Tough Great",
    "Tough Ultra", "Tough Master",
];

/// Byte and bit in [`Edit::ribbons`] of the ribbon at `i` in [`RIBBONS`].
pub const fn ribbon_pos(i: usize) -> (usize, u8) {
    let off = if i < 28 {
        i
    } else if i < 60 {
        32 + i - 28
    } else {
        64 + i - 60
    };
    (off / 8, (off % 8) as u8)
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use super::*;
    use crate::data::{GameData, MoveData};
    use crate::safety::check_pkm;
    use crate::save::SaveKind;

    fn file(rel: &str) -> Option<std::vec::Vec<u8>> {
        std::fs::read(std::format!("{}/../../{rel}", env!("CARGO_MANIFEST_DIR"))).ok()
    }

    /// A move change on a Pokémon with PP Ups used to leave PP above the new maximum.
    #[test]
    fn move_change_clears_pp_ups_before_filling_pp() {
        let (Some(mut buf), Some(game), Some(moves)) = (
            file("corpus/real/pkhex-avalonia/gen4_platinum.sav"),
            file("data/blobs/pt.bin"),
            file("data/blobs/moves4.bin"),
        ) else {
            return;
        };
        let save = Save::open(&mut buf).unwrap();
        let rules = Rules {
            kind: SaveKind::Pt,
            game: GameData::load(&game).unwrap(),
            moves: MoveData::load(&moves).unwrap(),
        };
        let before = save.pkm(Slot::Party(0)).unwrap();
        assert!(
            before.pp_ups().iter().any(|&u| u > 0),
            "fixture needs PP Ups"
        );
        let mut m = before.moves();
        m[0] = 147; // Spore
        let e = Edit {
            moves: Some(m),
            ..Edit::default()
        };
        let pk = edited(&save, &rules, Slot::Party(0), &e, None).unwrap();
        assert_eq!(pk.pp_ups(), [0; 4]);
        assert_eq!(pk.pp()[0], rules.moves.max_pp(147, 0));
        assert_eq!(check_pkm(&rules, &pk, true), Ok(()));
    }
}
