//! Game-safe validator (PLAN P7): "this cannot crash or corrupt the game". Separate from
//! legality: an illegal-but-valid Pokémon (IVs not matching the PID, say) passes; values the
//! game cannot hold (unknown IDs, broken strings, bad checksums, wrong party stats) do not.
//!
//! Every write goes through [`checked_set_pkm`], in both legal and game-safe modes.
//!
//! ID limits are PKHeX `Legal.MaxSpeciesID_4` etc.; held items are the save type's
//! `HeldItems` list (exported into the game blob); location ranges are the Gen 4/5 location
//! tables PKHeX ships (`Resources/text/locations`). Ranges the games tolerate beyond these are
//! unknown until the emulator harness (Phase H) can probe them, so the validator stays at what a
//! legitimately obtained Pokémon can carry.

use crate::data::{GameData, MoveData};
use crate::pkm::derived::exp_for_level;
use crate::pkm::{Format, Pkm};
use crate::save::{Save, SaveError, SaveKind, Slot};
use crate::text::{TERMINATOR, value_to_char_g4};

/// Why a Pokémon is unsafe for the game.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PkmIssue {
    WrongFormat,
    ChecksumMismatch,
    /// The sanity word (0x04) is non-zero: the game shows a Bad Egg.
    SanityFlags,
    /// Species 0 in a slot that must hold a Pokémon, or beyond the game's last species.
    Species,
    Form,
    Ability,
    HeldItem,
    /// A move ID beyond the game's last move.
    Move,
    NoMoves,
    DuplicateMove,
    /// PP above the maximum for the move and PP Ups, PP Ups above 3, or PP on an empty slot.
    Pp,
    Nature,
    Level,
    Exp,
    /// Nickname not terminated within 10 characters, or a character the game cannot draw.
    Nickname,
    /// OT name not terminated within 7 characters, or a character the game cannot draw.
    OtName,
    Language,
    OriginGame,
    Ball,
    MetLocation,
    EggLocation,
    /// Party level byte disagrees with the EXP, a stat is 0, or current HP exceeds max HP (and,
    /// for verity's own writes, stats differ from the computed ones).
    PartyStats,
}

impl PkmIssue {
    /// What to tell the user.
    pub fn text(self) -> &'static str {
        let t = ISSUE_TEXTS[self as usize];
        &t[..t.len() - 1]
    }

    /// [`Self::text`] NUL-terminated, for the C ABI.
    pub fn c_text(self) -> &'static str {
        ISSUE_TEXTS[self as usize]
    }
}

/// By `PkmIssue` order, NUL-terminated for the C ABI.
const ISSUE_TEXTS: [&str; 22] = [
    "wrong format\0",
    "checksum mismatch\0",
    "bad egg flags\0",
    "species\0",
    "form\0",
    "ability\0",
    "held item not in this game\0",
    "move not in this game\0",
    "no moves\0",
    "duplicate move\0",
    "PP out of range\0",
    "nature\0",
    "level\0",
    "EXP out of range\0",
    "nickname\0",
    "OT name\0",
    "language\0",
    "origin game\0",
    "ball\0",
    "met location\0",
    "egg location\0",
    "party stats\0",
];

/// Why a save is unsafe for the game.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaveIssue {
    /// The party must hold 1–6 Pokémon.
    PartyCount,
    /// At least one party Pokémon must not be an egg.
    NoUsablePartyMember,
    Pkm(Slot, PkmIssue),
    Checksums,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WriteError {
    Pkm(PkmIssue),
    Save(SaveIssue),
    Io(SaveError),
}

/// Per-game data the validator reads.
#[derive(Clone, Copy)]
pub struct Rules<'a> {
    pub kind: SaveKind,
    /// The save's own game table (`dp.bin`, `pt.bin`, `hgss.bin`, `bw.bin` or `b2w2.bin`).
    pub game: GameData<'a>,
    /// `moves4.bin` or `moves5.bin`.
    pub moves: MoveData<'a>,
}

struct Limits {
    species: u16,
    moves: u16,
    abilities: u8,
    max_ball: u8,
}

fn limits(kind: SaveKind) -> Limits {
    match kind {
        SaveKind::DP | SaveKind::Pt | SaveKind::HGSS => Limits {
            species: 493,
            moves: 467,
            abilities: 123,
            max_ball: 16,
        },
        SaveKind::BW | SaveKind::B2W2 => Limits {
            species: 649,
            moves: 559,
            abilities: 164,
            max_ball: 25,
        },
    }
}

fn format_of(kind: SaveKind) -> Format {
    match kind {
        SaveKind::DP | SaveKind::Pt | SaveKind::HGSS => Format::Pk4,
        SaveKind::BW | SaveKind::B2W2 => Format::Pk5,
    }
}

fn u16_at(pk: &Pkm, ofs: usize) -> u16 {
    let d = pk.decrypted();
    u16::from_le_bytes([d[ofs], d[ofs + 1]])
}

/// Terminated within `max` characters, every character drawable.
fn string_ok(raw: &[u16], max: usize, gen4: bool) -> bool {
    let Some(end) = raw
        .iter()
        .position(|&c| c == TERMINATOR || (!gen4 && c == 0))
    else {
        return false;
    };
    end <= max
        && raw[..end].iter().all(|&v| {
            if gen4 {
                let c = value_to_char_g4(v);
                c != TERMINATOR && c != 0
            } else {
                v != 0
            }
        })
}

fn location4_ok(v: u16, dppt_field: bool) -> bool {
    let low_max = if dppt_field { 111 } else { 234 };
    v <= low_max || (2000..=2014).contains(&v) || (3000..=3076).contains(&v)
}

fn location5_ok(v: u16) -> bool {
    v <= 153
        || (30001..=30015).contains(&v)
        || (40001..=40109).contains(&v)
        || (60001..=60003).contains(&v)
}

fn origin_ok(kind: SaveKind, v: u8) -> bool {
    let gen4 = matches!(v, 1..=5 | 7 | 8 | 10 | 11 | 12 | 15);
    match kind {
        SaveKind::BW | SaveKind::B2W2 => gen4 || matches!(v, 20..=23),
        _ => gen4,
    }
}

/// Checks one Pokémon for the save's game. `party` selects the party-format checks.
pub fn check_pkm(r: &Rules, pk: &Pkm, party: bool) -> Result<(), PkmIssue> {
    use PkmIssue as I;
    let lim = limits(r.kind);
    let gen4 = pk.format() == Format::Pk4;
    if pk.format() != format_of(r.kind) || pk.is_party_format() != party {
        return Err(I::WrongFormat);
    }
    if !pk.checksum_valid() {
        return Err(I::ChecksumMismatch);
    }
    if u16_at(pk, 0x04) != 0 {
        return Err(I::SanityFlags);
    }
    let species = pk.species();
    if species == 0 || species > lim.species || species > r.game.max_species() {
        return Err(I::Species);
    }
    let pi = r.game.species_form(species, 0);
    // PKHeX `FormInfo.IsValidOutOfBoundsForm`: forms the personal table does not list.
    let form_ok = pk.form() < pi.form_count().max(1)
        || match species {
            201 => pk.form() < 28,
            414 => pk.form() < 3,
            _ => false,
        };
    if !form_ok {
        return Err(I::Form);
    }
    let pi = r.game.species_form(species, pk.form());
    let a = pk.ability();
    if a == 0 || a > lim.abilities {
        return Err(I::Ability);
    }
    let item = pk.held_item();
    if item != 0 && !r.game.held_items().iter().any(|i| i == item) {
        return Err(I::HeldItem);
    }

    let moves = pk.moves();
    let pp = pk.pp();
    let ups = pk.pp_ups();
    if moves[0] == 0 && moves.iter().all(|&m| m == 0) {
        return Err(I::NoMoves);
    }
    for i in 0..4 {
        let m = moves[i];
        if m > lim.moves || m > r.moves.max_move() {
            return Err(I::Move);
        }
        if m != 0 && moves[..i].contains(&m) {
            return Err(I::DuplicateMove);
        }
        if ups[i] > 3 || (m == 0 && (pp[i] != 0 || ups[i] != 0)) {
            return Err(I::Pp);
        }
        if m != 0 && pp[i] > r.moves.max_pp(m, ups[i]) {
            return Err(I::Pp);
        }
    }
    if !gen4 && pk.nature() >= 25 {
        return Err(I::Nature);
    }

    let growth = pi.growth();
    if pk.exp() > exp_for_level(100, growth) {
        return Err(I::Exp);
    }
    let level = pk.level(&pi.base_info());
    // Met level 0 is a hatched Pokémon.
    if !(1..=100).contains(&level) || pk.met_level() > 100 {
        return Err(I::Level);
    }

    if !string_ok(&pk.nickname_raw(), 10, gen4) {
        return Err(I::Nickname);
    }
    if !string_ok(&pk.ot_name_raw(), 7, gen4) {
        return Err(I::OtName);
    }
    if !matches!(pk.language(), 1..=5 | 7 | 8) {
        return Err(I::Language);
    }
    if !origin_ok(r.kind, pk.version()) {
        return Err(I::OriginGame);
    }
    let d = pk.decrypted();
    let ball_ok = if gen4 {
        (1..=lim.max_ball).contains(&d[0x83]) && d[0x86] <= 24
    } else {
        (1..=lim.max_ball).contains(&d[0x83])
    };
    if !ball_ok {
        return Err(I::Ball);
    }
    if gen4 {
        if !location4_ok(u16_at(pk, 0x80), true) || !location4_ok(u16_at(pk, 0x46), false) {
            return Err(I::MetLocation);
        }
        if !location4_ok(u16_at(pk, 0x7E), true) || !location4_ok(u16_at(pk, 0x44), false) {
            return Err(I::EggLocation);
        }
    } else {
        if !location5_ok(u16_at(pk, 0x80)) {
            return Err(I::MetLocation);
        }
        // Poké Transfer keeps the Gen 4 egg location (PKHeX `PK4.ConvertToPK5`).
        let egg = u16_at(pk, 0x7E);
        let from_gen4 = matches!(pk.version(), 1..=15);
        if !location5_ok(egg) && !(from_gen4 && location4_ok(egg, false)) {
            return Err(I::EggLocation);
        }
    }

    if party {
        // The games recompute stats only on level-up or a box round trip, so stale stats are
        // normal in real saves; the level byte and HP bound are not.
        let stored = pk.stored_stats().ok_or(I::PartyStats)?;
        if pk.stat_level() != Some(level)
            || stored.contains(&0)
            || pk.hp_current().is_none_or(|hp| hp > stored[0])
        {
            return Err(I::PartyStats);
        }
    }
    Ok(())
}

/// Party stats exactly as the game computes them (PKHeX `ResetPartyStats`); required of every
/// party Pokémon verity writes.
pub fn party_stats_exact(r: &Rules, pk: &Pkm) -> bool {
    let info = r.game.species_form(pk.species(), pk.form()).base_info();
    pk.stored_stats() == Some(pk.calc_stats(&info))
        && pk.hp_current() == Some(pk.calc_stats(&info)[0])
        && pk.status_condition() == Some(0)
}

/// Checks a non-empty slot, or accepts an empty one (species 0 after decryption).
fn check_slot(r: &Rules, save: &Save, slot: Slot) -> Result<(), SaveIssue> {
    let pk = save
        .pkm(slot)
        .map_err(|_| SaveIssue::Pkm(slot, PkmIssue::WrongFormat))?;
    if pk.species() == 0 && matches!(slot, Slot::Box(..)) {
        return Ok(());
    }
    check_pkm(r, &pk, matches!(slot, Slot::Party(_))).map_err(|e| SaveIssue::Pkm(slot, e))
}

/// Save-level party rules: count 1–6, every party slot valid, at least one non-egg.
pub fn check_party(r: &Rules, save: &Save) -> Result<(), SaveIssue> {
    let n = save.party_count();
    if !(1..=6).contains(&n) {
        return Err(SaveIssue::PartyCount);
    }
    let mut usable = false;
    for i in 0..n as u8 {
        check_slot(r, save, Slot::Party(i))?;
        usable |= !save.pkm(Slot::Party(i)).is_ok_and(|p| p.is_egg());
    }
    if !usable {
        return Err(SaveIssue::NoUsablePartyMember);
    }
    Ok(())
}

/// A game that hasn't reached its first Pokémon: an empty party and empty boxes. A slot that can't
/// be read counts as occupied.
fn before_first_pokemon(save: &Save) -> bool {
    save.party_count() == 0
        && (0..save.box_count() as u8).all(|b| {
            (0..crate::save::BOX_SLOTS as u8)
                .all(|s| save.pkm(Slot::Box(b, s)).is_ok_and(|p| p.species() == 0))
        })
}

/// Checks the whole save: party, every box slot and the block checksums. Run after
/// [`Save::finalize`].
pub fn check_save(r: &Rules, save: &Save) -> Result<(), SaveIssue> {
    if !before_first_pokemon(save) {
        check_party(r, save)?;
    }
    for b in 0..save.box_count() as u8 {
        for s in 0..crate::save::BOX_SLOTS as u8 {
            check_slot(r, save, Slot::Box(b, s))?;
        }
    }
    if !save.checksums_valid() {
        return Err(SaveIssue::Checksums);
    }
    Ok(())
}

/// The only write path: validates `pk` for `slot`, writes it, and re-checks the party; a
/// failing write is rolled back and rejected with the reason.
pub fn checked_set_pkm(r: &Rules, save: &mut Save, slot: Slot, pk: &Pkm) -> Result<(), WriteError> {
    if r.kind != save.kind() {
        return Err(WriteError::Pkm(PkmIssue::WrongFormat));
    }
    check_pkm(r, pk, matches!(slot, Slot::Party(_))).map_err(WriteError::Pkm)?;
    if matches!(slot, Slot::Party(_)) && !party_stats_exact(r, pk) {
        return Err(WriteError::Pkm(PkmIssue::PartyStats));
    }
    if let Slot::Party(i) = slot {
        // Party slots stay contiguous.
        if i as usize > save.party_count() || i >= 6 {
            return Err(WriteError::Save(SaveIssue::PartyCount));
        }
    }
    let before = save.slot_raw(slot).map_err(WriteError::Io)?;
    let mut backup = [0u8; crate::pkm::SIZE_4PARTY];
    let len = before.len();
    backup[..len].copy_from_slice(before);
    let count = save.party_count();
    save.set_pkm(slot, pk).map_err(WriteError::Io)?;
    if matches!(slot, Slot::Party(_))
        && let Err(e) = check_party(r, save)
    {
        save.set_slot_raw(slot, &backup[..len])
            .map_err(WriteError::Io)?;
        save.set_party_count(count);
        return Err(WriteError::Save(e));
    }
    Ok(())
}
