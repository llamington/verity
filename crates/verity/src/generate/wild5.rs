//! Generates legal Gen 5 wild Pokémon, static encounters and in-game trades. Mirrors PKHeX
//! `EncounterSlot5`, `EncounterStatic5`, `EncounterTrade5BW` and `EncounterTrade5B2W2`
//! `.ConvertToPKM`, and `MonochromeRNG.Generate`. Gen 5's PID isn't linked to the IVs or nature, so a PID meeting the
//! rules can be built directly; the two links left are gender (the low byte) and a parity check
//! that ties the PID's top and bottom bits to the trainer IDs.

use super::build4::{Trainer4, encounter_moves, safe_language};
use super::wild4::Rand;
use crate::data::encounters::{Encounter, EncounterKind, flag};
use crate::legality::analysis4::Matched;
use crate::legality::analysis5::{Context5, analyze5};
use crate::pkm::derived::{exp_for_level, gender_from_pid};
use crate::pkm::{Format, Pkm};

/// `EncounterUtil.FormRandom`: the slot's form is rolled.
const FORM_RANDOM: u8 = 31;
const BALL_POKE: u8 = 4;
const SIZE_5STORED: usize = 136;
/// PKHeX `AbilityPermission` and `Shiny` values.
const ABILITY_ONLY_FIRST: u8 = 1;
const ABILITY_ONLY_SECOND: u8 = 2;
const ABILITY_ONLY_HIDDEN: u8 = 4;
const SHINY_NEVER: u8 = 1;
const SHINY_ALWAYS: u8 = 2;
const NATURE_RANDOM: u8 = 25;
const LANGUAGE_JAPANESE: u8 = 1;

#[derive(Clone, Copy)]
pub struct Request5<'a> {
    pub encounter: Encounter<'a>,
    /// The game it was met in: a shared static template names a pair of games.
    pub version: u8,
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
    /// The static encounter or trade is shiny-locked.
    ShinyLocked,
    /// The trade's nature is fixed to another one.
    NatureFixed,
    BallNotAllowed,
    /// The built Pokémon failed the legality gate: a generator bug, and it is never returned.
    Illegal,
}

fn is_shiny(pid: u32, id: u32) -> bool {
    let x = pid ^ id;
    ((x >> 16) ^ (x & 0xFFFF)) < 8
}

struct PidRules {
    ratio: u8,
    /// Requested gender, when not forced.
    gender: Option<u8>,
    /// A static encounter's fixed gender, for a species that can be either.
    forced_gender: Option<u8>,
    shiny: bool,
    /// The wild parity check that ties the PID's top and bottom bits to the trainer IDs.
    wild_xor: bool,
    /// A single-ability encounter's ability bit (PID bit 16).
    ability_bit: Option<u32>,
}

/// A PID meeting `p` for the trainer IDs, in `MonochromeRNG.Generate`'s order: gender, shiny,
/// ability, parity.
fn roll_pid(r: &mut Rand, tid: u16, sid: u16, p: &PidRules) -> u32 {
    let id = (sid as u32) << 16 | tid as u32;
    let xor = (tid ^ sid) as u32;
    let parity = xor & 1;
    loop {
        let mut pid = r.next_u32();
        if let Some(g) = p.forced_gender {
            let ratio = p.ratio as u32;
            let low = if g == 0 {
                ratio + r.next_u32() % (254 - ratio)
            } else {
                1 + r.next_u32() % (ratio - 1).max(1)
            };
            pid = pid & !0xFF | low;
        }
        if p.shiny {
            let mut low = pid & 0xFFFF;
            // The high half is the low half xor the IDs, so bit 31 follows bit 15 of that.
            if p.wild_xor && ((low ^ xor) >> 15 & 1) ^ (low & 1) ^ parity != 0 {
                low ^= 0x8000;
            }
            let extra = r.next_u32() & 7;
            pid = (low ^ xor ^ extra) << 16 | low;
        }
        if let Some(bit) = p.ability_bit {
            pid = pid & !0x1_0000 | bit << 16;
        }
        if !p.shiny {
            if is_shiny(pid, id) {
                pid ^= 0x1000_0000;
            }
            if p.wild_xor && (pid >> 31) ^ (pid & 1) ^ parity != 0 {
                pid ^= 0x8000_0000;
            }
            if is_shiny(pid, id) {
                continue;
            }
        }
        if p.forced_gender.is_some() || p.gender.is_none_or(|g| gender_from_pid(pid, p.ratio) == g)
        {
            return pid;
        }
    }
}

/// Builds a legal PK5 from a wild slot, static encounter or in-game trade for this trainer. Nothing illegal is returned: the result
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
    let template = e.template();
    let trade = matches!(
        e.kind(),
        Some(EncounterKind::Trade5BW | EncounterKind::Trade5B2W2)
    );
    let fixed_pid = template.filter(|t| t.has(flag::HAS_PID)).map(|t| t.pid());
    let fixed_nature = template.map(|t| t.nature()).filter(|&n| n != NATURE_RANDOM);
    if let (Some(f), Some(n)) = (fixed_nature, q.nature)
        && f != n
    {
        return Err(Error5::NatureFixed);
    }
    // Unlike `MonochromeRNG.Generate`, single-gender species too: PKHeX's PIDVerifier checks
    // the forced low byte for them (see docs/discrepancies.md).
    let forced_gender = template
        .map(|t| t.gender())
        .filter(|&g| g < 2 && fixed_pid.is_none());
    if let (Some(f), Some(g)) = (forced_gender, q.gender)
        && f != g
    {
        return Err(Error5::GenderImpossible);
    }
    let grotto = e.is_hidden_grotto();
    if q.shiny && grotto {
        return Err(Error5::ShinyImpossible);
    }
    if q.shiny && e.shiny() == SHINY_NEVER {
        return Err(Error5::ShinyLocked);
    }
    let ball = match (e.ball(), q.ball) {
        (0, b) => b.unwrap_or(BALL_POKE),
        (fixed, None) => fixed,
        (fixed, Some(b)) if b == fixed => b,
        _ => return Err(Error5::BallNotAllowed),
    };
    if !matches!(ball, 1..=4 | 6..=15) {
        return Err(Error5::BallNotAllowed);
    }
    let permission = template.map_or(0, |t| t.ability());
    let rules = PidRules {
        ratio,
        gender: q.gender,
        forced_gender,
        shiny: q.shiny || e.shiny() == SHINY_ALWAYS,
        wild_xor: e.is_slot() || e.has(flag::WILD_CORRELATION_PID),
        ability_bit: match permission {
            ABILITY_ONLY_FIRST => Some(0),
            ABILITY_ONLY_SECOND => Some(1),
            _ => None,
        },
    };
    // A trade belongs to the NPC who traded it.
    let (tid, sid, ot_gender) = match template.filter(|_| trade) {
        Some(t) => (t.tid(), t.sid(), t.ot_gender()),
        None => (tr.tid, tr.sid, tr.gender & 1),
    };
    let pid = match fixed_pid {
        Some(pid) => {
            if q.shiny {
                return Err(Error5::ShinyLocked);
            }
            if q.gender.is_some_and(|g| g != gender_from_pid(pid, ratio)) {
                return Err(Error5::GenderImpossible);
            }
            pid
        }
        None => roll_pid(&mut r, tid, sid, &rules),
    };
    let [a1, a2, ah] = pi.abilities();
    let hidden = grotto || permission == ABILITY_ONLY_HIDDEN;
    let ability = if hidden {
        ah
    } else if permission == ABILITY_ONLY_FIRST {
        a1
    } else if permission == ABILITY_ONLY_SECOND {
        a2
    } else if pid >> 16 & 1 == 0 {
        a1
    } else {
        a2
    };
    let mut language = safe_language(tr.language);
    let bw_trade = e.kind() == Some(EncounterKind::Trade5BW);
    // Trades for Japanese BW have language 0; their names are still the Japanese ones.
    let names_language = language;
    if bw_trade && language == LANGUAGE_JAPANESE {
        language = 0;
    }
    let version = q.version;

    let mut raw = [0u8; SIZE_5STORED];
    raw[0x5F] = version;
    raw[0x17] = language;
    if hidden {
        raw[0x42] = 1;
    }
    raw[0x84] = ot_gender << 7;
    let mut pk = Pkm::from_decrypted(Format::Pk5, &raw).ok_or(Error5::Illegal)?;
    pk.set_pid(pid);
    pk.set_species(species);
    pk.set_ids(tid, sid);
    pk.set_exp(exp_for_level(level, pi.growth()));
    pk.set_friendship(pi.base_friendship());
    pk.set_ability(ability);
    pk.set_nature_pk5(
        fixed_nature
            .or(q.nature)
            .unwrap_or((r.next_u32() % 25) as u8),
    );
    match template.filter(|t| t.has(flag::HAS_IVS)) {
        Some(t) => pk.set_ivs(t.ivs()),
        None => pk.set_ivs(core::array::from_fn(|_| (r.next_u32() % 32) as u8)),
    }
    pk.set_fateful_gender_form(e.has(flag::FATEFUL), gender_from_pid(pid, ratio), form);
    let trade_names = if trade {
        let ed = if bw_trade { &ctx.enc_bw } else { &ctx.enc_b2w2 };
        Some(ed.trade_names(e, names_language).ok_or(Error5::Illegal)?)
    } else {
        None
    };
    match trade_names {
        Some((_, ot)) => {
            let text: alloc::vec::Vec<u16> = ot.iter().collect();
            let mut raw = [0u16; 8];
            crate::text::encode5(&text, 7, language, &mut raw, crate::text::Clear::Zero);
            pk.set_ot_name_raw(&raw);
        }
        None => pk.set_ot_name_raw(&tr.name_raw),
    }
    pk.set_met_location(e.location());
    pk.set_met_level(level);
    pk.set_met_date(q.date);
    pk.set_ball(ball);
    let learn = if matches!(version, 20 | 21) {
        &ctx.bw
    } else {
        &ctx.b2w2
    };
    let moves = match template {
        Some(t) if t.has(flag::HAS_MOVES) => t.moves(),
        _ => encounter_moves(learn, species, form, level),
    };
    pk.set_moves_with_pp(moves, &ctx.moves);
    let fixed_nickname = trade_names
        .filter(|_| e.has(flag::FIXED_NICKNAME))
        .map(|(nick, _)| nick);
    pk.set_is_nicknamed(fixed_nickname.is_some());
    let mut name = [0u16; 11];
    let mut n = 0;
    let species_name = ctx.names.get(names_language as u16, species as usize);
    for c in fixed_nickname.unwrap_or(species_name).iter().take(10) {
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
