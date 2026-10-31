//! Legal Gen 4 D/P/Pt wild generation (Method J), built by inverting the RNG rather than
//! searching frames: choose the outcome (PID or IVs), recover the seeds that produce it, then
//! validate the frame backwards with [`method_j`], the PKHeX-equivalent checker.

use crate::rng::finder::{is_cute_charm, AnalyzeInput, PidIv, PidType};
use crate::rng::lcrng::{next15, next16, next_n};
use crate::rng::method_j::{self, LeadRequired, LeadSeed, Slot4};
use crate::rng::reversal::{seeds, seeds_from_ivs, MAX_SEEDS_IV, MAX_SEEDS_PID};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lead {
    /// Any lead whose effect is not required (PKHeX `LeadRequired.None` or a failed proc).
    None,
    Synchronize(u8),
    /// Cute Charm lead of the opposite gender: the result has `gender`.
    CuteCharm {
        gender: u8,
    },
}

#[derive(Clone, Copy, Debug)]
pub struct Request {
    pub slot: Slot4,
    pub tid: u16,
    pub sid: u16,
    pub species: u16,
    pub gender_ratio: u8,
    pub met_level: Option<u8>,
    pub nature: Option<u8>,
    pub gender: Option<u8>,
    /// PID bit 0 (ability slot).
    pub ability_bit: Option<u8>,
    pub shiny: bool,
    pub lead: Lead,
    pub hidden_power_type: Option<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Generated {
    pub pid: u32,
    pub iv32: u32,
    pub level: u8,
    pub gender: u8,
    /// PID-IV origin seed (PKHeX `PIDIV.OriginSeed`).
    pub pidiv_seed: u32,
    pub lead: LeadSeed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GenError {
    LevelOutsideSlot,
    NatureConflictsWithSynchronize,
    ShinyImpossibleForCuteCharm,
    GenderImpossible,
    CuteCharmNeedsGenderedSpecies,
    UnownFormUnsupported,
    BudgetExhausted,
}

/// Small deterministic generator for candidate choice (xorshift64*).
pub struct Rand(u64);

impl Rand {
    pub fn new(seed: u64) -> Self {
        Self(seed | 1)
    }
    pub fn next_u32(&mut self) -> u32 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        (self.0.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 32) as u32
    }
}

fn gender_of(pid: u32, ratio: u8) -> u8 {
    crate::pkm::derived::gender_from_pid(pid, ratio)
}

fn ivs_after_pid(pid_seed: u32) -> u32 {
    let mut s = next_n::<2>(pid_seed);
    let iv1 = next15(&mut s) as u32;
    let iv2 = next15(&mut s) as u32;
    iv1 | iv2 << 15
}

fn hidden_power_ok(req: &Request, iv32: u32) -> bool {
    req.hidden_power_type.is_none_or(|t| {
        let ivs = core::array::from_fn(|i| ((iv32 >> (5 * i)) & 0x1F) as u16);
        crate::pkm::derived::hidden_power_type(&ivs) == t
    })
}

fn level_bounds(req: &Request) -> Result<(u8, u8), GenError> {
    let s = &req.slot;
    match req.met_level {
        Some(l) if l < s.level_min || l > s.pressure_level.max(s.level_max) => {
            Err(GenError::LevelOutsideSlot)
        }
        Some(l) => Ok((l, l)),
        None => Ok((s.level_min, s.level_max)),
    }
}

/// Final level for a validated frame (PKHeX `GenerateMethodJ.SetFromIVsJ`).
fn level_for(slot: &Slot4, lead: &LeadSeed, fallback: u8) -> u8 {
    if !slot.slot_type.is_level_rand_dppt() {
        return if lead.lead == LeadRequired::PressureHustleSpirit {
            slot.pressure_level
        } else {
            fallback
        };
    }
    if lead.lead == LeadRequired::PressureHustleSpirit {
        return slot.pressure_level;
    }
    let r = method_j::skip_to_level_rand(slot, lead.seed) >> 16;
    let modulo = 1 + slot.level_max as u32 - slot.level_min as u32;
    (r % modulo + slot.level_min as u32) as u8
}

fn lead_matches(req: &Request, lead: LeadRequired) -> bool {
    use LeadRequired as L;
    match req.lead {
        Lead::None => matches!(lead, L::None | L::SynchronizeFail | L::CuteCharmFail),
        Lead::Synchronize(_) => lead == L::Synchronize,
        Lead::CuteCharm { .. } => lead == L::CuteCharm,
    }
}

fn analyze_input(req: &Request, pid: u32, iv32: u32, level: u8, gender: u8) -> AnalyzeInput {
    AnalyzeInput {
        pid,
        iv32,
        tid: req.tid,
        sid: req.sid,
        species: req.species,
        gender,
        version: 12,
        met_level: level,
        ot_gender: 0,
    }
}

/// Validates a candidate exactly as legality will: PID-IV method, then the lead/frame check.
fn validate(
    req: &Request,
    pid: u32,
    iv32: u32,
    level_hint: (u8, u8),
    ratio: &impl Fn(u16) -> u8,
) -> Option<Generated> {
    let gender = gender_of(pid, req.gender_ratio);
    let input = analyze_input(req, pid, iv32, level_hint.0, gender);
    let pidiv = crate::rng::finder::analyze(&input, ratio);
    if !matches!(pidiv.kind, PidType::Method1 | PidType::CuteCharm) {
        return None;
    }
    // Range pass (format 0 checks min <= level <= max), then the exact format-4 check.
    let lead = method_j::lead_info4_dppt(
        &input,
        &req.slot,
        pidiv,
        level_hint.0,
        level_hint.1,
        0,
        ratio,
    );
    if !lead.is_valid() || !lead_matches(req, lead.lead) {
        return None;
    }
    let level = level_for(&req.slot, &lead, level_hint.0);
    if level < level_hint.0 || level > level_hint.1 {
        return None;
    }
    // Re-check with the exact met level, as legality will see it.
    let input = analyze_input(req, pid, iv32, level, gender);
    let final_lead = method_j::lead_info4_dppt(&input, &req.slot, pidiv, level, level, 4, ratio);
    if !final_lead.is_valid() || !lead_matches(req, final_lead.lead) {
        return None;
    }
    Some(Generated {
        pid,
        iv32,
        level,
        gender,
        pidiv_seed: pidiv.seed,
        lead: final_lead,
    })
}

fn pid_ok(req: &Request, pid: u32, nature: Option<u8>) -> bool {
    let id = (req.tid ^ req.sid) as u32;
    let shiny = ((pid >> 16) ^ (pid & 0xFFFF) ^ id) < 8;
    shiny == req.shiny
        && nature.is_none_or(|n| pid % 25 == n as u32)
        && req
            .gender
            .is_none_or(|g| gender_of(pid, req.gender_ratio) == g)
        && req.ability_bit.is_none_or(|a| pid & 1 == a as u32)
}

fn effective_nature(req: &Request) -> Result<Option<u8>, GenError> {
    match req.lead {
        Lead::Synchronize(n) => {
            if req.nature.is_some_and(|x| x != n) {
                return Err(GenError::NatureConflictsWithSynchronize);
            }
            Ok(Some(n))
        }
        _ => Ok(req.nature),
    }
}

/// Generates a legal Method J wild Pokémon. `budget` caps candidate PIDs/IV sets tried.
pub fn generate(
    req: &Request,
    rand: &mut Rand,
    budget: u32,
    ratio: impl Fn(u16) -> u8,
) -> Result<Generated, GenError> {
    if req.species == 201 {
        return Err(GenError::UnownFormUnsupported);
    }
    let bounds = level_bounds(req)?;
    let nature = effective_nature(req)?;
    if let Lead::CuteCharm { gender } = req.lead {
        return generate_cute_charm(req, gender, nature, bounds, rand, budget, &ratio);
    }
    if req.gender.is_some()
        && matches!(req.gender_ratio, 0 | 254 | 255)
        && req.gender != Some(gender_fixed(req.gender_ratio))
    {
        return Err(GenError::GenderImpossible);
    }
    for _ in 0..budget {
        let pid = if req.shiny {
            shiny_pid_candidate(req, nature, rand)
        } else {
            random_pid_candidate(req, nature, rand)
        };
        let Some(pid) = pid else { continue };
        let mut buf = [0u32; MAX_SEEDS_PID];
        let n = seeds(&mut buf, pid << 16, pid & 0xFFFF_0000);
        for &seed in &buf[..n] {
            let iv32 = ivs_after_pid(seed);
            if !hidden_power_ok(req, iv32) {
                continue;
            }
            if let Some(g) = validate(req, pid, iv32, bounds, &ratio) {
                return Ok(g);
            }
        }
    }
    Err(GenError::BudgetExhausted)
}

fn gender_fixed(ratio: u8) -> u8 {
    match ratio {
        0 => 0,
        254 => 1,
        _ => 2,
    }
}

fn low_ok(req: &Request, low: u32) -> bool {
    req.ability_bit.is_none_or(|a| low & 1 == a as u32)
        && req
            .gender
            .is_none_or(|g| gender_of(low, req.gender_ratio) == g)
}

fn random_pid_candidate(req: &Request, nature: Option<u8>, rand: &mut Rand) -> Option<u32> {
    for _ in 0..64 {
        let pid = rand.next_u32();
        if pid_ok(req, pid, nature) {
            return Some(pid);
        }
    }
    None
}

/// A shiny PID: random low half meeting gender/ability, high half from the 8 shiny values.
fn shiny_pid_candidate(req: &Request, nature: Option<u8>, rand: &mut Rand) -> Option<u32> {
    let id = (req.tid ^ req.sid) as u32;
    for _ in 0..64 {
        let low = rand.next_u32() & 0xFFFF;
        if !low_ok(req, low) {
            continue;
        }
        let start = rand.next_u32() & 7;
        for k in 0..8 {
            let high = (id ^ low) ^ ((start + k) & 7);
            let pid = high << 16 | low;
            if pid_ok(req, pid, nature) {
                return Some(pid);
            }
        }
    }
    None
}

fn generate_cute_charm(
    req: &Request,
    gender: u8,
    nature: Option<u8>,
    bounds: (u8, u8),
    rand: &mut Rand,
    budget: u32,
    ratio: &impl Fn(u16) -> u8,
) -> Result<Generated, GenError> {
    let gr = req.gender_ratio;
    if gr == 0 || gr >= 254 {
        return Err(GenError::CuteCharmNeedsGenderedSpecies);
    }
    if req.gender.is_some_and(|g| g != gender) {
        return Err(GenError::GenderImpossible);
    }
    let cc_pid = |nat: u32| {
        if gender == 1 {
            nat
        } else {
            25 * (gr as u32 / 25 + 1) + nat
        }
    };
    let cc_req = Request {
        gender: Some(gender),
        ..*req
    };
    let natures: [bool; 25] =
        core::array::from_fn(|n| pid_ok(&cc_req, cc_pid(n as u32), Some(n as u8)));
    if !natures
        .iter()
        .zip(0..)
        .any(|(&ok, n)| ok && nature.is_none_or(|x| x == n))
    {
        return Err(if req.shiny {
            GenError::ShinyImpossibleForCuteCharm
        } else {
            GenError::GenderImpossible
        });
    }
    for _ in 0..budget {
        let nat = nature.map_or(rand.next_u32() % 25, |n| n as u32);
        if !natures[nat as usize] {
            continue;
        }
        let pid = cc_pid(nat);
        let iv32 = rand.next_u32() & 0x3FFF_FFFF;
        if !hidden_power_ok(req, iv32) {
            continue;
        }
        let input = analyze_input(req, pid, iv32, bounds.0, gender);
        if !is_cute_charm(&input, ratio) {
            continue;
        }
        let ivs: [u8; 6] = core::array::from_fn(|i| ((iv32 >> (5 * i)) & 0x1F) as u8);
        let mut buf = [0u32; MAX_SEEDS_IV];
        let n = seeds_from_ivs(&mut buf, ivs);
        if n == 0 {
            continue;
        }
        let pidiv = PidIv {
            kind: PidType::CuteCharm,
            seed: 0,
        };
        let lead =
            method_j::lead_info4_dppt(&input, &req.slot, pidiv, bounds.0, bounds.1, 0, ratio);
        if lead.lead != LeadRequired::CuteCharm {
            continue;
        }
        let level = level_for(&req.slot, &lead, bounds.0);
        let input = analyze_input(req, pid, iv32, level, gender);
        let fin = method_j::lead_info4_dppt(&input, &req.slot, pidiv, level, level, 4, ratio);
        if fin.lead != LeadRequired::CuteCharm
            || crate::rng::finder::analyze(&input, ratio).kind != PidType::CuteCharm
        {
            continue;
        }
        return Ok(Generated {
            pid,
            iv32,
            level,
            gender,
            pidiv_seed: 0,
            lead: fin,
        });
    }
    Err(GenError::BudgetExhausted)
}

/// The IVs a Method 1 frame yields; exposed for tests.
pub fn method1_ivs(pid_seed: u32) -> u32 {
    ivs_after_pid(pid_seed)
}

/// PID from a Method 1 origin seed.
pub fn method1_pid(mut seed: u32) -> u32 {
    let a = next16(&mut seed) as u32;
    let b = next16(&mut seed) as u32;
    b << 16 | a
}
