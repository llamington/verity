//! Gen 4 D/P/Pt wild encounter RNG (Method J) frame validation. Ported from PKHeX
//! `Legality/RNG/ClassicEra/Gen4/MethodJ.cs`, `SlotMethodJ.cs`, `Gen3/SlotMethodH.cs`,
//! `Frame/FrameCheckDetails.cs`, `Frame/LeadRequired.cs`, `ClassicEra/LeadSeed.cs` and the
//! Gen 4 part of `ClassicEra/LeadFinder.cs`.

use super::lcrng::{next16, next_n, next_seed, prev16, prev_seed};

/// PKHeX `SlotType4`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum SlotType4 {
    Grass = 0,
    Surf = 1,
    OldRod = 2,
    GoodRod = 3,
    SuperRod = 4,
    RockSmash = 5,
    Headbutt = 6,
    HeadbuttSpecial = 7,
    BugContest = 8,
    HoneyTree = 9,
    SafariGrass = 10,
    SafariSurf = 11,
    SafariOldRod = 12,
    SafariGoodRod = 13,
    SafariSuperRod = 14,
}

impl SlotType4 {
    pub fn from_u8(v: u8) -> Option<Self> {
        use SlotType4::*;
        Some(match v {
            0 => Grass,
            1 => Surf,
            2 => OldRod,
            3 => GoodRod,
            4 => SuperRod,
            5 => RockSmash,
            6 => Headbutt,
            7 => HeadbuttSpecial,
            8 => BugContest,
            9 => HoneyTree,
            10 => SafariGrass,
            11 => SafariSurf,
            12 => SafariOldRod,
            13 => SafariGoodRod,
            14 => SafariSuperRod,
            _ => return None,
        })
    }

    pub fn is_fishing_rod(self) -> bool {
        matches!(self, Self::OldRod | Self::GoodRod | Self::SuperRod)
    }

    pub fn is_level_rand_dppt(self) -> bool {
        self != Self::Grass
    }
}

/// PKHeX `LeadRequired`; ordering matters (`IsBetterThan` compares discriminants).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum LeadRequired {
    Invalid = 0,
    StaticMagnetFail,
    CuteCharmFail,
    SynchronizeFail,
    PressureHustleSpiritFail,
    IntimidateKeenEyeFail,
    Illuminate,
    SuctionCups,
    PressureHustleSpirit,
    MagnetPull,
    Static,
    CuteCharm,
    Synchronize,
    CuteCharmRadar,
    SynchronizeRadar,
    Radar,
    None = 255,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LeadSeed {
    pub seed: u32,
    pub lead: LeadRequired,
}

impl Default for LeadSeed {
    fn default() -> Self {
        Self {
            seed: 0,
            lead: LeadRequired::Invalid,
        }
    }
}

impl LeadSeed {
    pub fn is_valid(&self) -> bool {
        self.lead != LeadRequired::Invalid
    }
}

/// The fields of PKHeX `IEncounterSlot4` that Method J reads.
#[derive(Clone, Copy, Debug)]
pub struct Slot4 {
    pub species: u16,
    pub slot_type: SlotType4,
    pub slot_number: u8,
    pub level_min: u8,
    pub level_max: u8,
    pub pressure_level: u8,
    pub magnet_pull: (u8, u8),
    pub static_slot: (u8, u8),
    pub can_radar: bool,
    pub coronet_feebas: bool,
}

impl Slot4 {
    pub fn from_encounter(e: &crate::data::encounters::Encounter) -> Option<Self> {
        Some(Self {
            species: e.species(),
            slot_type: SlotType4::from_u8(e.slot_type())?,
            slot_number: e.slot_number(),
            level_min: e.level_min(),
            level_max: e.level_max(),
            pressure_level: e.pressure_level(),
            magnet_pull: e.magnet_pull(),
            static_slot: e.static_slot(),
            can_radar: e.can_radar(),
            coronet_feebas: e.has(crate::data::encounters::flag::CORONET_FEEBAS),
        })
    }

    fn is_fixed_level(&self) -> bool {
        self.level_min == self.level_max
    }

    fn can_radar_grass(&self) -> bool {
        self.slot_type == SlotType4::Grass && self.can_radar
    }

    /// `IMagnetStatic.IsSlotValidStaticMagnet`.
    fn static_magnet_lead(&self, rand: u32) -> Option<LeadRequired> {
        let (si, sc) = self.static_slot;
        if sc != 0 && rand % sc as u32 == si as u32 {
            return Some(LeadRequired::Static);
        }
        let (mi, mc) = self.magnet_pull;
        if mc != 0 && rand % mc as u32 == mi as u32 {
            return Some(LeadRequired::MagnetPull);
        }
        None
    }
}

const FORMAT: u8 = 4;

fn is_cute_charm_fail(r: u32) -> bool {
    r / 0x5556 == 0
}
fn is_cute_charm_pass(r: u32) -> bool {
    r / 0x5556 != 0
}
fn is_sync_pass(r: u32) -> bool {
    r >> 15 == 0
}
fn is_static_magnet_fail(r: u32) -> bool {
    r >> 15 != 0
}
fn is_static_magnet_pass(r: u32) -> bool {
    r >> 15 == 0
}
fn is_hustle_vital_fail(r: u32) -> bool {
    r >> 15 != 1
}
fn is_hustle_vital_pass(r: u32) -> bool {
    r >> 15 == 1
}
fn is_intimidate_keen_eye_pass(r: u32) -> bool {
    r >> 15 == 1
}
fn is_feebas_chance(r: u32) -> bool {
    r >> 15 == 1
}

pub fn nature_from_rand(r: u32) -> u32 {
    r / 0xA3E
}

pub fn honey_tree_level(r: u32) -> u32 {
    5 + r / 0x1745
}

/// PKHeX `SlotMethodH.GetRegular`.
pub fn slot_regular(roll: u32) -> u8 {
    match roll {
        0..20 => 0,
        20..40 => 1,
        40..50 => 2,
        50..60 => 3,
        60..70 => 4,
        70..80 => 5,
        80..85 => 6,
        85..90 => 7,
        90..94 => 8,
        94..98 => 9,
        98 => 10,
        99 => 11,
        _ => 255,
    }
}

/// PKHeX `SlotMethodH.GetSurf`.
pub fn slot_surf(roll: u32) -> u8 {
    match roll {
        0..60 => 0,
        60..90 => 1,
        90..95 => 2,
        95..99 => 3,
        99 => 4,
        _ => 255,
    }
}

/// PKHeX `SlotMethodJ.GetSuperRod`.
pub fn slot_super_rod(roll: u32) -> u8 {
    match roll {
        0..40 => 0,
        40..80 => 1,
        80..95 => 2,
        95..99 => 3,
        99 => 4,
        _ => 255,
    }
}

/// PKHeX `SlotMethodJ.GetSlot`.
pub fn slot_from_rand(t: SlotType4, rand: u32) -> u8 {
    let esv = rand / 656;
    match t {
        SlotType4::OldRod | SlotType4::Surf => slot_surf(esv),
        SlotType4::GoodRod | SlotType4::SuperRod => slot_super_rod(esv),
        SlotType4::HoneyTree => 0,
        _ => slot_regular(esv),
    }
}

/// PKHeX `SlotMethodJ.GetRange`: the ESV roll range [min, max] selecting a slot.
pub fn slot_range(t: SlotType4, slot: u8) -> (u8, u8) {
    let ranges: &[(u8, u8)] = match t {
        SlotType4::OldRod | SlotType4::Surf => &[(0, 59), (60, 89), (90, 94), (95, 98), (99, 99)],
        SlotType4::GoodRod | SlotType4::SuperRod => {
            &[(0, 39), (40, 79), (80, 94), (95, 98), (99, 99)]
        }
        SlotType4::HoneyTree => return (0, 99),
        _ => &[
            (0, 19),
            (20, 39),
            (40, 49),
            (50, 59),
            (60, 69),
            (70, 79),
            (80, 84),
            (85, 89),
            (90, 93),
            (94, 97),
            (98, 98),
            (99, 99),
        ],
    };
    ranges.get(slot as usize).copied().unwrap_or((255, 255))
}

/// `FrameCheckDetails`: seeds 1..3 before the nature roll.
#[derive(Clone, Copy)]
struct Frame<'a> {
    enc: &'a Slot4,
    level_min: u8,
    level_max: u8,
    format: u8,
    seed1: u32,
    seed2: u32,
    seed3: u32,
}

impl<'a> Frame<'a> {
    fn new(enc: &'a Slot4, seed: u32, level_min: u8, level_max: u8, format: u8) -> Self {
        let seed1 = prev_seed(seed);
        let seed2 = prev_seed(seed1);
        let seed3 = prev_seed(seed2);
        Self {
            enc,
            level_min,
            level_max,
            format,
            seed1,
            seed2,
            seed3,
        }
    }
    fn seed4(&self) -> u32 {
        prev_seed(self.seed3)
    }
    fn prev1(&self) -> u32 {
        self.seed1 >> 16
    }
    fn prev2(&self) -> u32 {
        self.seed2 >> 16
    }
    fn prev3(&self) -> u32 {
        self.seed3 >> 16
    }
}

fn sequential_pid(mut seed: u32) -> u32 {
    let r1 = next16(&mut seed) as u32;
    let r2 = next16(&mut seed) as u32;
    r2 << 16 | r1
}

/// PKHeX `MethodJ.GetSeed`. `seed` is the PID-IV origin seed (PKHeX `PIDIV.OriginSeed`);
/// `level_min`/`level_max` is the met-level constraint.
pub fn get_seed(enc: &Slot4, seed: u32, level_min: u8, level_max: u8, format: u8) -> LeadSeed {
    let pid = sequential_pid(seed);
    let nature = (pid % 25) as u8;
    let frames = reversal_window(seed, nature);
    origin_seed(enc, seed, nature, frames, level_min, level_max, format)
}

pub fn origin_seed(
    enc: &Slot4,
    mut seed: u32,
    nature: u8,
    mut reverse_count: usize,
    level_min: u8,
    level_max: u8,
    format: u8,
) -> LeadSeed {
    let mut prefer = LeadSeed::default();
    loop {
        if let Some(mut result) = try_match(enc, level_min, level_max, seed, nature, format) {
            if check_activation(enc, &mut result.seed) {
                if result.lead == LeadRequired::None {
                    return result;
                }
                if result.lead > prefer.lead {
                    prefer = result;
                }
            }
        }
        if reverse_count == 0 {
            return prefer;
        }
        reverse_count -= 1;
        seed = prev_seed(prev_seed(seed));
    }
}

/// PKHeX `MethodJ.GetReversalWindow`.
pub fn reversal_window(mut seed: u32, nature: u8) -> usize {
    let mut ctr = 0;
    let mut b = seed >> 16;
    loop {
        let a = prev16(&mut seed) as u32;
        let pid = b << 16 | a;
        if pid % 25 == nature as u32 {
            break;
        }
        b = prev16(&mut seed) as u32;
        ctr += 1;
    }
    ctr
}

/// PKHeX `MethodJ.SkipToLevelRand`.
pub fn skip_to_level_rand(enc: &Slot4, seed: u32) -> u32 {
    if enc.slot_type == SlotType4::HoneyTree {
        return next_seed(seed);
    }
    if !enc.slot_type.is_fishing_rod() {
        return next_n::<2>(seed);
    }
    if enc.coronet_feebas {
        return next_n::<4>(seed);
    }
    next_n::<3>(seed)
}

fn check_activation(enc: &Slot4, seed: &mut u32) -> bool {
    if enc.slot_type.is_fishing_rod() {
        if enc.coronet_feebas {
            if enc.species == 349 && !is_feebas_chance(*seed >> 16) {
                return false;
            }
            *seed = prev_seed(*seed);
        }
        let rate = match enc.slot_type {
            SlotType4::OldRod => 25,
            SlotType4::GoodRod => 50,
            SlotType4::SuperRod => 75,
            _ => 0,
        };
        if (*seed >> 16) / 656 < rate {
            *seed = prev_seed(*seed);
            return true;
        }
        return false;
    }
    if enc.slot_type == SlotType4::HoneyTree {
        *seed = next_seed(*seed);
    }
    true
}

/// PKHeX `MethodJ.TryGetMatchCuteCharm` over candidate IV seeds.
pub fn try_match_cute_charm(
    enc: &Slot4,
    seeds: &[u32],
    nature: u8,
    level_min: u8,
    level_max: u8,
    format: u8,
) -> Option<LeadSeed> {
    for &seed in seeds {
        if nature_from_rand(seed >> 16) != nature as u32 {
            continue;
        }
        let ctx = Frame::new(enc, seed, level_min, level_max, format);
        if is_cute_charm_fail(ctx.prev1()) {
            continue;
        }
        let Some(mut s) = slot_valid_from_1_skip(&ctx) else {
            continue;
        };
        if !check_activation(enc, &mut s) {
            continue;
        }
        return Some(LeadSeed {
            seed: s,
            lead: LeadRequired::CuteCharm,
        });
    }
    if enc.can_radar_grass() {
        for &seed in seeds {
            if nature_from_rand(seed >> 16) != nature as u32 {
                continue;
            }
            let ctx = Frame::new(enc, seed, level_min, level_max, format);
            if is_cute_charm_fail(ctx.prev1()) {
                continue;
            }
            return Some(LeadSeed {
                seed: ctx.seed2,
                lead: LeadRequired::CuteCharmRadar,
            });
        }
    }
    None
}

fn try_match(
    enc: &Slot4,
    level_min: u8,
    level_max: u8,
    seed: u32,
    nature: u8,
    format: u8,
) -> Option<LeadSeed> {
    let p0 = seed >> 16;
    if nature_from_rand(p0) == nature as u32 {
        let ctx = Frame::new(enc, seed, level_min, level_max, format);
        return try_match_no_sync(&ctx);
    }
    if is_sync_pass(p0) && !(enc.slot_type == SlotType4::Grass && enc.level_max < level_min) {
        let ctx = Frame::new(enc, seed, level_min, level_max, format);
        if let Some(s) = slot_valid_regular(&ctx) {
            return Some(LeadSeed {
                seed: s,
                lead: LeadRequired::Synchronize,
            });
        }
        if enc.can_radar_grass() {
            return Some(LeadSeed {
                seed: ctx.seed1,
                lead: LeadRequired::SynchronizeRadar,
            });
        }
    }
    None
}

fn lead(seed: u32, lead: LeadRequired) -> Option<LeadSeed> {
    Some(LeadSeed { seed, lead })
}

fn try_match_no_sync(ctx: &Frame) -> Option<LeadSeed> {
    use LeadRequired as L;
    let enc = ctx.enc;
    if enc.slot_type == SlotType4::Grass && enc.level_max > ctx.level_min {
        return slot_valid_hustle_vital(ctx).and_then(|s| lead(s, L::PressureHustleSpirit));
    }
    if let Some(s) = slot_valid_regular(ctx) {
        return lead(s, L::None);
    }
    if let Some(s) = (!is_sync_pass(ctx.prev1()))
        .then(|| slot_valid_from_1_skip(ctx))
        .flatten()
    {
        return lead(s, L::SynchronizeFail);
    }
    if let Some(s) = (!is_cute_charm_pass(ctx.prev1()))
        .then(|| slot_valid_from_1_skip(ctx))
        .flatten()
    {
        return lead(s, L::CuteCharmFail);
    }
    if let Some(s) = (!is_hustle_vital_pass(ctx.prev1()))
        .then(|| slot_valid_from_1_skip(ctx))
        .flatten()
    {
        return lead(s, L::PressureHustleSpiritFail);
    }
    if let Some(s) = slot_valid_static_magnet_fail(ctx) {
        return lead(s, L::StaticMagnetFail);
    }
    if let Some((s, l)) = slot_valid_static_magnet(ctx) {
        return lead(s, l);
    }
    if let Some(s) = (!is_intimidate_keen_eye_pass(ctx.prev1()))
        .then(|| slot_valid_from_1_skip(ctx))
        .flatten()
    {
        return lead(s, L::IntimidateKeenEyeFail);
    }
    if enc.pressure_level <= ctx.level_max {
        if let Some(s) = slot_valid_hustle_vital(ctx) {
            return lead(s, L::PressureHustleSpirit);
        }
    }
    if enc.can_radar_grass() {
        return lead(ctx.seed1, L::Radar);
    }
    None
}

fn slot_valid(enc: &Slot4, rand: u32) -> bool {
    enc.slot_type == SlotType4::HoneyTree || slot_from_rand(enc.slot_type, rand) == enc.slot_number
}

fn random_level(enc: &Slot4, rand: u32) -> u32 {
    if enc.slot_type == SlotType4::HoneyTree {
        return honey_tree_level(rand);
    }
    let modulo = 1 + enc.level_max as u32 - enc.level_min as u32;
    rand % modulo + enc.level_min as u32
}

fn original_level_valid(min: u8, max: u8, format: u8, level: u32) -> bool {
    if format == FORMAT && min > 1 {
        return level == min as u32;
    }
    (min as u32..=max as u32).contains(&(level as u8 as u32))
}

fn level_valid(ctx: &Frame, rand: u32) -> bool {
    original_level_valid(
        ctx.level_min,
        ctx.level_max,
        ctx.format,
        random_level(ctx.enc, rand),
    )
}

fn slot_valid_from_1_skip(ctx: &Frame) -> Option<u32> {
    let enc = ctx.enc;
    if enc.slot_type.is_level_rand_dppt() {
        if (enc.is_fixed_level() || level_valid(ctx, ctx.prev2())) && slot_valid(enc, ctx.prev3()) {
            return Some(ctx.seed4());
        }
    } else if slot_valid(enc, ctx.prev2()) {
        return Some(ctx.seed3);
    }
    None
}

fn slot_valid_regular(ctx: &Frame) -> Option<u32> {
    let enc = ctx.enc;
    if enc.slot_type.is_level_rand_dppt() {
        if (enc.is_fixed_level() || level_valid(ctx, ctx.prev1())) && slot_valid(enc, ctx.prev2()) {
            return Some(ctx.seed3);
        }
    } else if slot_valid(enc, ctx.prev1()) {
        return Some(ctx.seed2);
    }
    None
}

fn slot_valid_hustle_vital(ctx: &Frame) -> Option<u32> {
    let enc = ctx.enc;
    if is_hustle_vital_fail(ctx.prev1()) {
        return None;
    }
    if !original_level_valid(
        ctx.level_min,
        ctx.level_max,
        ctx.format,
        enc.pressure_level as u32,
    ) {
        return None;
    }
    if enc.slot_type.is_level_rand_dppt() {
        slot_valid(enc, ctx.prev3()).then(|| ctx.seed4())
    } else {
        slot_valid(enc, ctx.prev2()).then_some(ctx.seed3)
    }
}

fn slot_valid_static_magnet(ctx: &Frame) -> Option<(u32, LeadRequired)> {
    let enc = ctx.enc;
    if is_static_magnet_fail(ctx.prev3()) {
        return None;
    }
    if enc.slot_type.is_level_rand_dppt() {
        if enc.is_fixed_level() || level_valid(ctx, ctx.prev1()) {
            if let Some(l) = enc.static_magnet_lead(ctx.prev2()) {
                return Some((ctx.seed4(), l));
            }
        }
    } else if let Some(l) = enc.static_magnet_lead(ctx.prev1()) {
        return Some((ctx.seed3, l));
    }
    None
}

fn slot_valid_static_magnet_fail(ctx: &Frame) -> Option<u32> {
    let enc = ctx.enc;
    if enc.slot_type.is_level_rand_dppt() {
        if is_static_magnet_pass(ctx.prev3()) {
            return None;
        }
        if (enc.is_fixed_level() || level_valid(ctx, ctx.prev1())) && slot_valid(enc, ctx.prev2()) {
            return Some(ctx.seed4());
        }
    } else {
        if is_static_magnet_pass(ctx.prev2()) {
            return None;
        }
        if slot_valid(enc, ctx.prev1()) {
            return Some(ctx.seed3);
        }
    }
    None
}

/// PKHeX `LeadFinder.GetLeadInfo4` for D/P/Pt origins.
///
/// `TryGetMatchRadarShiny` is omitted: in PKHeX 25.12.21 it always returns false (its result is
/// overwritten before returning; see docs/discrepancies.md).
pub fn lead_info4_dppt(
    pk: &super::finder::AnalyzeInput,
    enc: &Slot4,
    pidiv: super::finder::PidIv,
    level_min: u8,
    level_max: u8,
    format: u8,
    ratio: impl Fn(u16) -> u8,
) -> LeadSeed {
    use super::finder::{is_cute_charm, PidType};
    let shiny = {
        let x = pk.pid ^ ((pk.sid as u32) << 16 | pk.tid as u32);
        ((x >> 16) ^ (x & 0xFFFF)) < 8
    };
    let mut kind = pidiv.kind;
    match kind {
        PidType::ChainShiny => {
            if shiny && enc.can_radar {
                return LeadSeed {
                    seed: pidiv.seed,
                    lead: LeadRequired::None,
                };
            }
            if is_cute_charm(pk, &ratio) {
                kind = PidType::CuteCharm;
            }
        }
        PidType::Method1 => {
            let r = get_seed(enc, pidiv.seed, level_min, level_max, format);
            if r.is_valid() {
                return r;
            }
            if is_cute_charm(pk, &ratio) {
                kind = PidType::CuteCharm;
            }
        }
        _ => {}
    }
    if kind == PidType::CuteCharm {
        let mut buf = [0u32; super::reversal::MAX_SEEDS_IV];
        let ivs = pk.iv32;
        let iv = |i: u32| ((ivs >> (5 * i)) & 0x1F) as u8;
        let n =
            super::reversal::seeds_from_ivs(&mut buf, [iv(0), iv(1), iv(2), iv(3), iv(4), iv(5)]);
        let nature = (pk.pid % 25) as u8;
        if let Some(r) = try_match_cute_charm(enc, &buf[..n], nature, level_min, level_max, format)
        {
            return r;
        }
    }
    LeadSeed::default()
}
