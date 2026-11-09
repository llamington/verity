//! Legal Gen 4 bred eggs, unhatched or hatched. Mirrors PKHeX `EncounterEgg4.ConvertToPKM` and
//! `MoveBreed.GetExpectedMoves`; nothing leaves [`generate`] without passing [`analyze4`].

use super::api4::{Error, IvRequest, VERSION_D, VERSION_HG, VERSION_P, VERSION_PT, VERSION_SS};
use super::build4::{safe_language, Trainer4};
use super::wild4::{GenError, Rand};
use crate::legality::analysis4::{analyze4, can_be_egg, Context4};
use crate::legality::breed4;
use crate::pkm::derived::{exp_for_level, gender_from_pid};
use crate::pkm::{Format, Pkm, SIZE_4STORED};
use crate::text::{encode4, Clear, TERMINATOR};

/// Day Care egg location (PKHeX `Locations.Daycare4`).
const DAYCARE4: u16 = 2000;
const FARAWAY4: u16 = 3002;
const BALL_POKE: u8 = 4;
/// Friendship of a freshly hatched Pokémon.
const HATCHED_FRIENDSHIP: u8 = 120;
/// PKHeX `Locations.HatchLocationDPPt` / `HatchLocationHGSS`.
const HATCH_DPPT: u16 = 4;
const HATCH_HGSS: u16 = 182;

#[derive(Clone, Copy)]
pub struct EggRequest {
    /// Game the egg is bred in.
    pub version: u8,
    /// The egg's species (a baby, or a split-breed species such as Marill).
    pub species: u16,
    pub nature: Option<u8>,
    pub gender: Option<u8>,
    /// Ability ID; it must be one of the species' two abilities.
    pub ability: Option<u8>,
    pub shiny: bool,
    /// [`IvRequest::Nearest`] is treated as exact: an egg's IVs and PID are unrelated.
    pub ivs: IvRequest,
    /// Moves in any order; `None` gives the base moves. They are put in the order the game
    /// would use.
    pub moves: Option<[u16; 4]>,
    /// Hatch it now, rather than giving an egg.
    pub hatched: bool,
    /// Where it hatched; `None` uses the game's default. Ignored for an unhatched egg.
    pub hatch_location: Option<u16>,
    /// Date the egg was received: years since 2000, month, day.
    pub egg_date: [u8; 3],
    /// Date it hatched. Ignored for an unhatched egg.
    pub met_date: [u8; 3],
    pub seed: u64,
    /// PID candidates to try.
    pub budget: u32,
}

/// PKHeX `Breeding.IsGenderSpeciesDetermination`.
fn gender_bit_species(species: u16) -> bool {
    matches!(species, 29 | 32 | 313 | 314)
}

fn is_shiny(pid: u32, tr: &Trainer4) -> bool {
    ((pid >> 16) ^ (pid & 0xFFFF) ^ tr.tid as u32 ^ tr.sid as u32) < 8
}

fn candidate_pid(rand: &mut Rand, shiny: bool, tr: &Trainer4) -> u32 {
    let pid = rand.next_u32();
    if !shiny {
        return pid;
    }
    let high = pid >> 16;
    let low = high ^ tr.tid as u32 ^ tr.sid as u32 ^ (rand.next_u32() & 7);
    (high << 16) | (low & 0xFFFF)
}

/// The egg's moves in the order the game gives them, or an error if no order is legal.
fn egg_moves(ctx: &Context4, req: &EggRequest) -> Result<[u16; 4], Error> {
    let Some(wanted) = req.moves else {
        let table = breed4::game(&ctx.learn, req.version);
        let (mut base, n) = breed4::base_egg_moves(table, req.species as usize, 1);
        base[n..].fill(0);
        return Ok(base);
    };
    // Empty slots go last, so a short list is still contiguous.
    let mut packed = [0u16; 4];
    for (slot, m) in packed
        .iter_mut()
        .zip(wanted.iter().copied().filter(|&m| m != 0))
    {
        *slot = m;
    }
    let expected = breed4::expected_moves(&ctx.learn, req.species, req.version, packed);
    let same_moves =
        packed.iter().all(|m| expected.contains(m)) && expected.iter().all(|m| packed.contains(m));
    if same_moves && breed4::validate(&ctx.learn, req.species, req.version, expected) {
        Ok(expected)
    } else {
        Err(Error::MovesIllegal)
    }
}

fn hatch_location(req: &EggRequest) -> u16 {
    req.hatch_location
        .unwrap_or(if matches!(req.version, VERSION_HG | VERSION_SS) {
            HATCH_HGSS
        } else {
            HATCH_DPPT
        })
}

/// Writes a location the way `G4PKM` stores it: D/P's field holds Faraway Place for locations
/// D/P don't have, and the extended field is only for Pt and HG/SS.
fn put_location(raw: &mut [u8; SIZE_4STORED], version: u8, dp_at: usize, ext_at: usize, loc: u16) {
    let extended = matches!(version, VERSION_HG | VERSION_SS | VERSION_PT);
    let (dp, ext) = if (112..2000).contains(&loc) || (2011..3000).contains(&loc) {
        (FARAWAY4, loc)
    } else {
        (loc, if extended { loc } else { 0 })
    };
    raw[dp_at..dp_at + 2].copy_from_slice(&dp.to_le_bytes());
    raw[ext_at..ext_at + 2].copy_from_slice(&ext.to_le_bytes());
}

fn build(
    ctx: &Context4,
    tr: &Trainer4,
    req: &EggRequest,
    moves: [u16; 4],
    pid: u32,
    gender: u8,
    iv32: u32,
) -> Pkm {
    let mut raw = [0u8; SIZE_4STORED];
    let pi = ctx.hgss.species_form(req.species, 0);
    let language = safe_language(tr.language);

    raw[0..4].copy_from_slice(&pid.to_le_bytes());
    raw[0x08..0x0A].copy_from_slice(&req.species.to_le_bytes());
    raw[0x0C..0x0E].copy_from_slice(&tr.tid.to_le_bytes());
    raw[0x0E..0x10].copy_from_slice(&tr.sid.to_le_bytes());
    raw[0x10..0x14].copy_from_slice(&exp_for_level(1, pi.growth()).to_le_bytes());
    raw[0x14] = if req.hatched {
        HATCHED_FRIENDSHIP
    } else {
        pi.hatch_cycles()
    };
    let [a1, a2, _] = pi.abilities();
    raw[0x15] = if pid & 1 == 0 { a1 } else { a2 };
    raw[0x17] = language;
    let flags = iv32 | if req.hatched { 0 } else { 1 << 30 };
    raw[0x38..0x3C].copy_from_slice(&flags.to_le_bytes());
    raw[0x40] = (gender & 3) << 1;
    raw[0x5F] = req.version;
    raw[0x78..0x7B].copy_from_slice(&req.egg_date);
    put_location(&mut raw, req.version, 0x7E, 0x44, DAYCARE4);
    if req.hatched {
        raw[0x7B..0x7E].copy_from_slice(&req.met_date);
        put_location(&mut raw, req.version, 0x80, 0x46, hatch_location(req));
    }
    raw[0x84] = (tr.gender & 1) << 7; // met level 0

    let mut pk = Pkm::from_decrypted(Format::Pk4, &raw).expect("stored length");
    pk.set_ball(BALL_POKE);

    // Eggs are named "Egg" in the player's language (French is spelled by hand in PKHeX).
    let name: alloc::vec::Vec<u16> = if !req.hatched && language == 3 {
        alloc::vec![0x4F, 0x65, 0x75, 0x66]
    } else {
        let id = if req.hatched { req.species as usize } else { 0 };
        ctx.names4.get(language as u16, id).iter().collect()
    };
    let mut nick = [0u16; 11];
    encode4(&name, 10, language, &mut nick, Clear::Zero);
    pk.set_nickname_raw(&nick);
    pk.set_is_nicknamed(false);

    let mut ot = [0u16; 8];
    for (i, &c) in tr.name_raw.iter().take(7).enumerate() {
        ot[i] = c;
        if c == TERMINATOR {
            break;
        }
        if i == 6 {
            ot[7] = TERMINATOR;
        }
    }
    pk.set_ot_name_raw(&ot);
    pk.set_moves_with_pp(moves, &ctx.learn.moves);
    pk.refresh_checksum();
    pk
}

/// Builds a legal egg for `tr`, or says why it can't be done.
pub fn generate(ctx: &Context4, tr: &Trainer4, req: &EggRequest) -> Result<Pkm, Error> {
    if !matches!(
        req.version,
        VERSION_D | VERSION_P | VERSION_PT | VERSION_HG | VERSION_SS
    ) {
        return Err(Error::VersionUnsupported);
    }
    if !can_be_egg(ctx, req.species) {
        return Err(Error::NoEncounter);
    }
    let pi = ctx.hgss.species_form(req.species, 0);
    let ratio = pi.gender_ratio();
    let [a1, a2, _] = pi.abilities();
    let ability_bit = match req.ability {
        None => None,
        Some(a) if a == a1 => Some(0),
        Some(a) if a == a2 => Some(1),
        Some(_) => return Err(Error::AbilityUnavailable),
    };
    if let Some(g) = req.gender {
        let possible = match ratio {
            255 => g == 2,
            254 => g == 1,
            0 => g == 0,
            _ => g < 2,
        };
        if !possible {
            return Err(GenError::GenderImpossible.into());
        }
    }
    let moves = egg_moves(ctx, req)?;

    let mut rand = Rand::new(req.seed);
    let mut last = None;
    for _ in 0..req.budget {
        let pid = candidate_pid(&mut rand, req.shiny, tr);
        if is_shiny(pid, tr) != req.shiny {
            continue;
        }
        let gender = gender_from_pid(pid, ratio);
        if req.gender.is_some_and(|g| g != gender)
            || req.nature.is_some_and(|n| n != (pid % 25) as u8)
            || ability_bit.is_some_and(|b| b != pid & 1)
            || (gender_bit_species(req.species) && gender as u32 == (pid >> 15) & 1)
        {
            continue;
        }
        let iv32 = match req.ivs {
            IvRequest::Random => rand.next_u32() & 0x3FFF_FFFF,
            IvRequest::Exact(v) | IvRequest::Nearest(v) => v
                .iter()
                .enumerate()
                .fold(0, |acc, (i, &iv)| acc | (iv as u32 & 31) << (5 * i)),
        };
        let pk = build(ctx, tr, req, moves, pid, gender, iv32);
        let report = analyze4(ctx, &pk);
        if report.valid() {
            return Ok(pk);
        }
        // A random PID/IV pair occasionally looks like an RNG spread, which eggs can't have.
        last = report.invalid.first().copied();
    }
    Err(match last {
        Some(check) => Error::Illegal(check),
        None => GenError::BudgetExhausted.into(),
    })
}
