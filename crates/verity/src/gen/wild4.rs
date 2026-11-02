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
    /// Poké Radar chain shiny (D/P/Pt grass slots that allow the radar). Always shiny.
    PokeRadarChain,
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
    /// Met location (needed for honey-tree Munchlax).
    pub location: u16,
    /// Requested form. Only Unown's form depends on the frame; other species ignore it.
    pub form: u8,
    /// Origin game: 10 D, 11 P, 12 Pt (Method J) or 7 HG, 8 SS (Method K).
    pub version: u8,
}

fn is_hgss(req: &Request) -> bool {
    matches!(req.version, 7 | 8)
}

/// PKHeX `LeadFinder.GetLeadInfo4`, Method J or K by origin game.
fn lead_info(
    req: &Request,
    input: &AnalyzeInput,
    pidiv: PidIv,
    level_min: u8,
    level_max: u8,
    format: u8,
    ratio: &impl Fn(u16) -> u8,
) -> LeadSeed {
    if is_hgss(req) {
        crate::rng::method_k::lead_info4_hgss(
            input, &req.slot, pidiv, level_min, level_max, format, ratio,
        )
    } else {
        method_j::lead_info4_dppt(input, &req.slot, pidiv, level_min, level_max, format, ratio)
    }
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
    /// The request's form (meaningful for slots with a random form, i.e. Unown).
    pub form: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GenError {
    LevelOutsideSlot,
    NatureConflictsWithSynchronize,
    ShinyImpossibleForCuteCharm,
    GenderImpossible,
    CuteCharmNeedsGenderedSpecies,
    /// The requested Unown form cannot appear here.
    FormUnavailable,
    RadarNotAllowedHere,
    /// Munchlax only appears in the four honey trees chosen by the trainer's ID.
    MunchlaxTreeUnavailable,
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

const UNOWN: u16 = 201;

fn unown_form_possible(req: &Request) -> Result<(), GenError> {
    if req.species == UNOWN && req.form >= 28 {
        return Err(GenError::FormUnavailable);
    }
    Ok(())
}

/// PKHeX `SolaceonRuins4.IsFormValid`: Unown's form comes from the frame after the IVs.
fn form_ok(req: &Request, pid: u32, iv32: u32) -> bool {
    use crate::rng::unown4 as u;
    req.species != UNOWN
        || if req.location == u::RUINS_OF_ALPH {
            u::alph_form_valid(pid, iv32, req.form)
        } else {
            u::form_valid(pid, iv32, req.form)
        }
}

fn level_bounds(req: &Request) -> Result<(u8, u8), GenError> {
    let s = &req.slot;
    match req.met_level {
        // PKHeX `EncounterSlot4.IsMatchExact`: the slot's range, or exactly the Pressure level.
        Some(l) if !(s.level_min..=s.level_max).contains(&l) && !pressure_only(req, l) => {
            Err(GenError::LevelOutsideSlot)
        }
        Some(l) => Ok((l, l)),
        None => Ok((s.level_min, s.level_max)),
    }
}

/// Final level for a validated frame (PKHeX `GenerateMethodJ.SetFromIVsJ` /
/// `GenerateMethodK.SetFromIVsK`).
fn level_for_req(req: &Request, lead: &LeadSeed, fallback: u8) -> u8 {
    let slot = &req.slot;
    if !is_hgss(req) {
        return level_for(slot, lead, fallback);
    }
    if lead.lead == LeadRequired::PressureHustleSpirit {
        return slot.pressure_level;
    }
    if !slot.slot_type.is_level_rand_hgss() {
        return fallback;
    }
    let r = crate::rng::method_k::skip_to_level_rand(slot, lead.seed, lead.lead) >> 16;
    crate::rng::method_k::level_for(slot, r, lead.lead) as u8
}

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

/// A met level only a Pressure/Hustle/Vital Spirit lead reaches (grass slots).
fn pressure_only(req: &Request, level: u8) -> bool {
    let s = &req.slot;
    s.slot_type == method_j::SlotType4::Grass
        && level == s.pressure_level
        && !(s.level_min..=s.level_max).contains(&level)
}

fn lead_matches(req: &Request, lead: LeadRequired) -> bool {
    use LeadRequired as L;
    match req.lead {
        Lead::None if req.met_level.is_some_and(|l| pressure_only(req, l)) => {
            lead == L::PressureHustleSpirit
        }
        Lead::None => matches!(lead, L::None | L::SynchronizeFail | L::CuteCharmFail),
        Lead::Synchronize(_) => lead == L::Synchronize,
        Lead::CuteCharm { .. } => lead == L::CuteCharm,
        Lead::PokeRadarChain => lead == L::None,
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
        version: req.version,
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
    if !form_ok(req, pid, iv32) {
        return None;
    }
    let gender = gender_of(pid, req.gender_ratio);
    let input = analyze_input(req, pid, iv32, level_hint.0, gender);
    let pidiv = crate::rng::finder::analyze(&input, ratio);
    if !matches!(pidiv.kind, PidType::Method1 | PidType::CuteCharm) {
        return None;
    }
    // Range pass (format 0 checks min <= level <= max), then the exact format-4 check.
    let lead = lead_info(req, &input, pidiv, level_hint.0, level_hint.1, 0, ratio);
    if !lead.is_valid() || !lead_matches(req, lead.lead) {
        return None;
    }
    let level = level_for_req(req, &lead, level_hint.0);
    if level < level_hint.0 || level > level_hint.1 {
        return None;
    }
    // Re-check with the exact met level, as legality will see it.
    let input = analyze_input(req, pid, iv32, level, gender);
    let final_lead = lead_info(req, &input, pidiv, level, level, 4, ratio);
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
        form: req.form,
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
    unown_form_possible(req)?;
    if req.species == 446
        && req.slot.slot_type == method_j::SlotType4::HoneyTree
        && !munchlax_ok(req)
    {
        return Err(GenError::MunchlaxTreeUnavailable);
    }
    let bounds = level_bounds(req)?;
    let nature = effective_nature(req)?;
    if let Lead::CuteCharm { gender } = req.lead {
        return generate_cute_charm(req, gender, nature, bounds, rand, budget, &ratio);
    }
    if req.lead == Lead::PokeRadarChain {
        return generate_radar_chain(req, nature, bounds, rand, budget, &ratio);
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

fn munchlax_ok(req: &Request) -> bool {
    #[cfg(feature = "alloc")]
    {
        crate::legality::enc4::is_munchlax_tree(req.tid, req.sid, req.location)
    }
    #[cfg(not(feature = "alloc"))]
    {
        let _ = req;
        false
    }
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
        if !hidden_power_ok(req, iv32) || !form_ok(req, pid, iv32) {
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
        let lead = lead_info(req, &input, pidiv, bounds.0, bounds.1, 0, ratio);
        if lead.lead != LeadRequired::CuteCharm {
            continue;
        }
        let level = level_for_req(req, &lead, bounds.0);
        let input = analyze_input(req, pid, iv32, level, gender);
        let fin = lead_info(req, &input, pidiv, level, level, 4, ratio);
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
            form: req.form,
        });
    }
    Err(GenError::BudgetExhausted)
}

/// Forward chain-shiny roll (inverse of PKHeX `MethodFinder.IsChainShinyValid`): returns
/// (pid, iv32) from an origin seed.
pub fn chain_shiny_roll(origin: u32, tid: u16, sid: u16) -> (u32, u32) {
    let mut s = next_n::<2>(origin);
    let low3 = (s >> 16) & 7;
    s = crate::rng::lcrng::next_seed(s);
    let high3 = (s >> 16) & 7;
    let mut low = low3;
    for bit in 3..16 {
        s = crate::rng::lcrng::next_seed(s);
        low |= ((s >> 16) & 1) << bit;
    }
    let high = ((low ^ tid as u32 ^ sid as u32) & 0xFFF8) | high3;
    let iv1 = next15(&mut s) as u32;
    let iv2 = next15(&mut s) as u32;
    (high << 16 | low, iv1 | iv2 << 15)
}

fn generate_radar_chain(
    req: &Request,
    nature: Option<u8>,
    bounds: (u8, u8),
    rand: &mut Rand,
    budget: u32,
    ratio: &impl Fn(u16) -> u8,
) -> Result<Generated, GenError> {
    if !(req.slot.can_radar && req.slot.slot_type == method_j::SlotType4::Grass) {
        return Err(GenError::RadarNotAllowedHere);
    }
    let req = Request {
        shiny: true,
        ..*req
    };
    let (lo, hi) = (
        bounds.0.max(req.slot.level_min),
        bounds.1.min(req.slot.level_max),
    );
    if lo > hi {
        return Err(GenError::LevelOutsideSlot);
    }
    for _ in 0..budget.saturating_mul(64) {
        let origin = rand.next_u32();
        let (pid, iv32) = chain_shiny_roll(origin, req.tid, req.sid);
        if !pid_ok(&req, pid, nature) || !hidden_power_ok(&req, iv32) || !form_ok(&req, pid, iv32) {
            continue;
        }
        let level = lo + (rand.next_u32() % (hi - lo + 1) as u32) as u8;
        let gender = gender_of(pid, req.gender_ratio);
        let input = analyze_input(&req, pid, iv32, level, gender);
        let pidiv = crate::rng::finder::analyze(&input, ratio);
        if pidiv.kind != PidType::ChainShiny {
            continue;
        }
        let lead = lead_info(&req, &input, pidiv, level, level, 4, ratio);
        if lead.lead != LeadRequired::None {
            continue;
        }
        return Ok(Generated {
            pid,
            iv32,
            level,
            gender,
            pidiv_seed: pidiv.seed,
            lead,
            form: req.form,
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

/// Generates with exact IVs (HP, Atk, Def, Spe, SpA, SpD), if any legal frame yields them.
pub fn generate_from_ivs(
    req: &Request,
    ivs: [u8; 6],
    ratio: impl Fn(u16) -> u8,
) -> Result<Generated, GenError> {
    unown_form_possible(req)?;
    if req.species == 446
        && req.slot.slot_type == method_j::SlotType4::HoneyTree
        && !munchlax_ok(req)
    {
        return Err(GenError::MunchlaxTreeUnavailable);
    }
    let bounds = level_bounds(req)?;
    let nature = effective_nature(req)?;
    let iv32 = ivs
        .iter()
        .enumerate()
        .fold(0u32, |a, (i, v)| a | ((*v as u32) & 0x1F) << (5 * i));
    if !hidden_power_ok(req, iv32) {
        return Err(GenError::BudgetExhausted);
    }
    if let Lead::CuteCharm { gender } = req.lead {
        let gr = req.gender_ratio;
        if gr == 0 || gr >= 254 {
            return Err(GenError::CuteCharmNeedsGenderedSpecies);
        }
        for nat in 0..25u32 {
            if nature.is_some_and(|n| n as u32 != nat) {
                continue;
            }
            let pid = if gender == 1 {
                nat
            } else {
                25 * (gr as u32 / 25 + 1) + nat
            };
            if !pid_ok(
                &Request {
                    gender: Some(gender),
                    ..*req
                },
                pid,
                Some(nat as u8),
            ) {
                continue;
            }
            if let Some(g) = validate_cute_charm(req, pid, iv32, gender, bounds, &ratio) {
                return Ok(g);
            }
        }
        return Err(GenError::BudgetExhausted);
    }
    let mut buf = [0u32; MAX_SEEDS_IV];
    let n = seeds_from_ivs(&mut buf, ivs);
    for &s in &buf[..n] {
        let origin = crate::rng::lcrng::prev_seed(crate::rng::lcrng::prev_seed(s));
        let pid = method1_pid(origin);
        if !pid_ok(req, pid, nature) {
            continue;
        }
        if let Some(g) = validate(req, pid, iv32, bounds, &ratio) {
            return Ok(g);
        }
    }
    Err(GenError::BudgetExhausted)
}

fn validate_cute_charm(
    req: &Request,
    pid: u32,
    iv32: u32,
    gender: u8,
    bounds: (u8, u8),
    ratio: &impl Fn(u16) -> u8,
) -> Option<Generated> {
    let input = analyze_input(req, pid, iv32, bounds.0, gender);
    if !form_ok(req, pid, iv32)
        || crate::rng::finder::analyze(&input, ratio).kind != PidType::CuteCharm
    {
        return None;
    }
    let pidiv = PidIv {
        kind: PidType::CuteCharm,
        seed: 0,
    };
    let lead = lead_info(req, &input, pidiv, bounds.0, bounds.1, 0, ratio);
    if lead.lead != LeadRequired::CuteCharm {
        return None;
    }
    let level = level_for_req(req, &lead, bounds.0);
    let input = analyze_input(req, pid, iv32, level, gender);
    let fin = lead_info(req, &input, pidiv, level, level, 4, ratio);
    (fin.lead == LeadRequired::CuteCharm).then_some(Generated {
        pid,
        iv32,
        level,
        gender,
        pidiv_seed: 0,
        lead: fin,
        form: req.form,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IvProposal {
    pub generated: Generated,
    pub ivs: [u8; 6],
    /// Per-stat change from the request (proposed - requested).
    pub diff: [i8; 6],
    /// False if the budget ran out before distance `distance` was fully searched.
    pub exhaustive: bool,
    pub distance: u32,
}

/// Closest legal IV spread to `target` (PLAN P4.6): ascending L1 distance, then fewest changed
/// stats. `budget` caps the number of spreads tested.
pub fn closest_legal_ivs(
    req: &Request,
    target: [u8; 6],
    budget: u32,
    ratio: impl Fn(u16) -> u8,
) -> Result<IvProposal, GenError> {
    let target = target.map(|v| v.min(31));
    let mut tested = 0u32;
    for d in 0..=(31 * 6) {
        let ks = if d == 0 { 0..=0 } else { 1..=6.min(d) };
        for k in ks {
            let mut found = None;
            let completed = for_each_spread(target, d, k, &mut |ivs| {
                tested += 1;
                if tested > budget {
                    return false;
                }
                if let Ok(g) = generate_from_ivs(req, ivs, &ratio) {
                    found = Some((g, ivs));
                    return false;
                }
                true
            });
            if let Some((g, ivs)) = found {
                return Ok(IvProposal {
                    generated: g,
                    ivs,
                    diff: core::array::from_fn(|i| ivs[i] as i8 - target[i] as i8),
                    exhaustive: true,
                    distance: d as u32,
                });
            }
            if !completed {
                return Err(GenError::BudgetExhausted);
            }
        }
    }
    Err(GenError::BudgetExhausted)
}

/// Calls `f` for every spread at L1 distance `d` from `t` changing exactly `k` stats, until `f`
/// returns false. Returns whether the enumeration completed.
fn for_each_spread(t: [u8; 6], d: usize, k: usize, f: &mut impl FnMut([u8; 6]) -> bool) -> bool {
    if k == 0 {
        return d != 0 || f(t);
    }
    for mask in 0u8..64 {
        if mask.count_ones() as usize != k {
            continue;
        }
        let mut idx = [0usize; 6];
        let mut n = 0;
        for (i, slot) in (0..6).filter(|i| mask >> i & 1 != 0).zip(0..) {
            idx[slot] = i;
            n += 1;
        }
        if !distribute(t, &idx[..n], d, t, f) {
            return false;
        }
    }
    true
}

fn distribute(
    t: [u8; 6],
    stats: &[usize],
    remaining: usize,
    cur: [u8; 6],
    f: &mut impl FnMut([u8; 6]) -> bool,
) -> bool {
    let Some((&s, rest)) = stats.split_first() else {
        return remaining != 0 || f(cur);
    };
    if remaining < stats.len() {
        return true;
    }
    let max_here = remaining - rest.len();
    let min_here = if rest.is_empty() { remaining } else { 1 };
    for amount in min_here..=max_here {
        for sign in [1i32, -1] {
            let v = t[s] as i32 + sign * amount as i32;
            if !(0..=31).contains(&v) {
                continue;
            }
            let mut next = cur;
            next[s] = v as u8;
            if !distribute(t, rest, remaining - amount, next, f) {
                return false;
            }
        }
    }
    true
}
