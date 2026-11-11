//! Generates legal Gen 5 wild Pokémon. Mirrors PKHeX `EncounterSlot5.ConvertToPKM` and
//! `MonochromeRNG.Generate`. Gen 5's PID isn't linked to the IVs or nature, so a PID meeting the
//! rules can be built directly; the two links left are gender (the low byte) and a parity check
//! that ties the PID's top and bottom bits to the trainer IDs.

use super::build4::{Trainer4, encounter_moves, safe_language};
use super::wild4::Rand;
use crate::data::encounters::Encounter;
use crate::legality::analysis4::Matched;
use crate::legality::analysis5::{Context5, analyze5};
use crate::pkm::derived::{exp_for_level, gender_from_pid};
use crate::pkm::{Format, Pkm};

/// `EncounterUtil.FormRandom`: the slot's form is rolled.
const FORM_RANDOM: u8 = 31;
const BALL_POKE: u8 = 4;
const SIZE_5STORED: usize = 136;

#[derive(Clone, Copy)]
pub struct Request5<'a> {
    pub encounter: Encounter<'a>,
    /// Met level; `None` rolls within the slot's range.
    pub level: Option<u8>,
    pub nature: Option<u8>,
    pub gender: Option<u8>,
    pub shiny: bool,
    pub ball: Option<u8>,
    /// Met date: years since 2000, month, day.
    pub date: [u8; 3],
    pub seed: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error5 {
    LevelOutsideSlot,
    GenderImpossible,
    /// Hidden Grotto Pokémon are never shiny.
    ShinyImpossible,
    BallNotAllowed,
    /// The built Pokémon failed the legality gate: a generator bug, and it is never returned.
    Illegal,
}

fn is_shiny(pid: u32, id: u32) -> bool {
    let x = pid ^ id;
    ((x >> 16) ^ (x & 0xFFFF)) < 8
}

/// A PID that satisfies the wild parity check, the gender, and shininess.
fn roll_pid(r: &mut Rand, tr: &Trainer4, ratio: u8, gender: Option<u8>, shiny: bool) -> u32 {
    let id = (tr.sid as u32) << 16 | tr.tid as u32;
    let parity = (tr.tid ^ tr.sid) as u32 & 1;
    loop {
        let mut pid = r.next_u32();
        if shiny {
            let lo = pid & 0xFFFF;
            let xor = (tr.tid ^ tr.sid) as u32;
            let mut low = lo;
            // The high half is the low half xor the IDs, so bit 31 follows bit 15 of that.
            if ((low ^ xor) >> 15 & 1) ^ (low & 1) ^ parity != 0 {
                low ^= 0x8000;
            }
            let extra = r.next_u32() & 7;
            pid = (low ^ xor ^ extra) << 16 | low;
        } else {
            if is_shiny(pid, id) {
                pid ^= 0x1000_0000;
            }
            if (pid >> 31) ^ (pid & 1) ^ parity != 0 {
                pid ^= 0x8000_0000;
            }
            if is_shiny(pid, id) {
                continue;
            }
        }
        if gender.is_none_or(|g| gender_from_pid(pid, ratio) == g) {
            return pid;
        }
    }
}

/// Builds a legal PK5 from a wild slot for this trainer. Nothing illegal is returned: the result
/// has passed [`analyze5`].
pub fn generate(ctx: &Context5, tr: &Trainer4, q: &Request5) -> Result<Pkm, Error5> {
    let e = &q.encounter;
    let mut r = Rand::new(q.seed | 1);
    let level = match q.level {
        Some(l) if (e.level_min()..=e.level_max()).contains(&l) => l,
        Some(_) => return Err(Error5::LevelOutsideSlot),
        None => {
            let span = (e.level_max() - e.level_min()) as u32 + 1;
            e.level_min() + (r.next_u32() % span) as u8
        }
    };
    let species = e.species();
    let form = if e.form() == FORM_RANDOM {
        let count = ctx.b2w2.personal(species as usize).form_count().max(1) as u32;
        (r.next_u32() % count) as u8
    } else {
        e.form()
    };
    let pi = ctx.b2w2.species_form(species, form);
    let ratio = pi.gender_ratio();
    if let Some(g) = q.gender {
        let possible = match ratio {
            255 => g == 2,
            254 => g == 1,
            0 => g == 0,
            _ => g < 2,
        };
        if !possible {
            return Err(Error5::GenderImpossible);
        }
    }
    let grotto = e.is_hidden_grotto();
    if q.shiny && grotto {
        return Err(Error5::ShinyImpossible);
    }
    let ball = q.ball.unwrap_or(BALL_POKE);
    if !matches!(ball, 1..=4 | 6..=15) {
        return Err(Error5::BallNotAllowed);
    }
    let pid = roll_pid(&mut r, tr, ratio, q.gender, q.shiny);
    let [a1, a2, ah] = pi.abilities();
    let ability = if grotto {
        ah
    } else if pid >> 16 & 1 == 0 {
        a1
    } else {
        a2
    };
    let language = safe_language(tr.language);
    let version = e.version();

    let mut raw = [0u8; SIZE_5STORED];
    raw[0x5F] = version;
    raw[0x17] = language;
    if grotto {
        raw[0x42] = 1;
    }
    raw[0x84] = (tr.gender & 1) << 7;
    let mut pk = Pkm::from_decrypted(Format::Pk5, &raw).ok_or(Error5::Illegal)?;
    pk.set_pid(pid);
    pk.set_species(species);
    pk.set_ids(tr.tid, tr.sid);
    pk.set_exp(exp_for_level(level, pi.growth()));
    pk.set_friendship(pi.base_friendship());
    pk.set_ability(ability);
    pk.set_nature_pk5(q.nature.unwrap_or((r.next_u32() % 25) as u8));
    pk.set_ivs(core::array::from_fn(|_| (r.next_u32() % 32) as u8));
    pk.set_fateful_gender_form(false, gender_from_pid(pid, ratio), form);
    pk.set_ot_name_raw(&tr.name_raw);
    pk.set_met_location(e.location());
    pk.set_met_level(level);
    pk.set_met_date(q.date);
    pk.set_ball(ball);
    let learn = if matches!(version, 20 | 21) {
        &ctx.bw
    } else {
        &ctx.b2w2
    };
    pk.set_moves_with_pp(encounter_moves(learn, species, form, level), &ctx.moves);
    let mut name = [0u16; 11];
    let mut n = 0;
    for c in ctx
        .names
        .get(language as u16, species as usize)
        .iter()
        .take(10)
    {
        name[n] = c;
        n += 1;
    }
    pk.set_nickname(&name[..n]);
    pk.refresh_checksum();
    let report = analyze5(ctx, &pk);
    if !report.valid() || matches!(report.matched, Matched::Invalid) {
        return Err(Error5::Illegal);
    }
    Ok(pk)
}

/// Writes a generated Pokémon into `slot` through the game-safe validator, with Pokédex flags.
pub fn insert(
    ctx: &Context5,
    save: &mut crate::save::Save,
    slot: crate::save::Slot,
    pk: &Pkm,
) -> Result<(), crate::safety::WriteError> {
    let game = if matches!(save.kind(), crate::save::SaveKind::BW) {
        ctx.bw
    } else {
        ctx.b2w2
    };
    let rules = crate::safety::Rules {
        kind: save.kind(),
        game,
        moves: ctx.moves,
    };
    let mut p = match slot {
        crate::save::Slot::Party(_) => {
            let info = game.species_form(pk.species(), pk.form()).base_info();
            let mut p = pk.clone();
            p.reset_party_stats(&info);
            p
        }
        crate::save::Slot::Box(..) => pk.to_stored(),
    };
    p.refresh_checksum();
    crate::safety::checked_set_pkm(&rules, save, slot, &p)?;
    save.register_dex(&p);
    Ok(())
}
