//! Gen 4 HG/SS wild encounter RNG (Method K) frame validation. Ported line by line from PKHeX
//! `Legality/RNG/ClassicEra/Gen4/MethodK.cs` and `SlotMethodK.cs` (25.12.21); shared types and
//! frame helpers come from [`super::method_j`].

use super::lcrng::{next_n, prev16, prev_seed};
use super::method_j::{
    original_level_valid, reversal_window, sequential_pid, slot_regular, slot_surf, Frame,
    LeadRequired, LeadSeed, Slot4, SlotType4,
};

// Summary of random determinations (Method K):
// Nature: rand % 25; Cute Charm: rand % 3 != 0; Sync: rand & 1 == 0;
// Static/Magnet Pull: rand & 1 == 0; Pressure/Hustle/Vital Spirit: rand & 1 == 1;
// Intimidate/Keen Eye: rand & 1 == 1 (0 rejects the encounter).
fn is_cute_charm_fail(r: u32) -> bool {
    r % 3 == 0
}
fn is_cute_charm_pass(r: u32) -> bool {
    r % 3 != 0
}
fn is_sync_pass(r: u32) -> bool {
    r & 1 == 0
}
fn is_static_magnet_fail(r: u32) -> bool {
    r & 1 != 0
}
fn is_static_magnet_pass(r: u32) -> bool {
    r & 1 == 0
}
fn is_hustle_vital_fail(r: u32) -> bool {
    r & 1 != 1
}
fn is_hustle_vital_pass(r: u32) -> bool {
    r & 1 == 1
}
fn is_intimidate_keen_eye_pass(r: u32) -> bool {
    r & 1 == 1
}

pub fn nature_from_rand(r: u32) -> u32 {
    r % 25
}

const INVALID: u8 = 255;

/// PKHeX `SlotMethodK.GetSuperRod`.
pub fn slot_super_rod(roll: u32) -> u8 {
    match roll {
        0..40 => 0,
        40..70 => 1,
        70..85 => 2,
        85..95 => 3,
        95..100 => 4,
        _ => INVALID,
    }
}

pub fn slot_rock_smash(roll: u32) -> u8 {
    if roll >= 80 {
        1
    } else {
        0
    }
}

pub fn slot_bug_contest(roll: u32) -> u8 {
    match roll {
        0..5 => 9,
        5..10 => 8,
        10..15 => 7,
        15..20 => 6,
        20..30 => 5,
        30..40 => 4,
        40..50 => 3,
        50..60 => 2,
        60..80 => 1,
        80..100 => 0,
        _ => INVALID,
    }
}

pub fn slot_headbutt(roll: u32) -> u8 {
    match roll {
        0..50 => 0,
        50..65 => 1,
        65..80 => 2,
        80..90 => 3,
        90..95 => 4,
        95..100 => 5,
        _ => INVALID,
    }
}

/// PKHeX `SlotMethodK.GetSlot` (`rand` is the high 16 bits).
pub fn slot_from_rand(t: SlotType4, rand: u32) -> u8 {
    use SlotType4 as T;
    match t {
        T::Grass => slot_regular(rand % 100),
        T::Surf => slot_surf(rand % 100),
        T::OldRod | T::GoodRod | T::SuperRod => slot_super_rod(rand % 100),
        T::RockSmash => slot_rock_smash(rand % 100),
        T::Headbutt | T::HeadbuttSpecial => slot_headbutt(rand % 100),
        T::BugContest => slot_bug_contest(rand % 100),
        T::SafariGrass | T::SafariSurf | T::SafariOldRod | T::SafariGoodRod | T::SafariSuperRod => {
            (rand % 10) as u8
        }
        T::HoneyTree => INVALID,
    }
}

/// PKHeX `SlotMethodK.GetRange`: the roll range [min, max] (out of 100, or 10 for Safari).
pub fn slot_range(t: SlotType4, slot: u8) -> (u8, u8) {
    use SlotType4 as T;
    let grass: [(u8, u8); 12] = [
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
    ];
    let pick = |r: &[(u8, u8)]| r.get(slot as usize).copied().unwrap_or((INVALID, INVALID));
    match t {
        T::Grass => pick(&grass),
        T::Surf => pick(&[(0, 59), (60, 89), (90, 94), (95, 98), (99, 99)]),
        T::OldRod | T::GoodRod | T::SuperRod => {
            pick(&[(0, 39), (40, 69), (70, 84), (85, 94), (95, 99)])
        }
        T::RockSmash => pick(&[(0, 79), (80, 99)]),
        T::Headbutt | T::HeadbuttSpecial => {
            pick(&[(0, 49), (50, 64), (65, 79), (80, 89), (90, 94), (95, 99)])
        }
        T::BugContest => match slot {
            9 => (0, 4),
            8 => (5, 9),
            7 => (10, 14),
            6 => (15, 19),
            5 => (20, 29),
            4 => (30, 39),
            3 => (40, 49),
            2 => (50, 59),
            1 => (60, 79),
            0 => (80, 99),
            _ => (INVALID, INVALID),
        },
        T::SafariGrass | T::SafariSurf | T::SafariOldRod | T::SafariGoodRod | T::SafariSuperRod => {
            (slot, slot)
        }
        T::HoneyTree => (INVALID, INVALID),
    }
}

fn is_level_rand(enc: &Slot4) -> bool {
    enc.slot_type.is_level_rand_hgss()
}

/// PKHeX `MethodK.GetSeed`. `seed` is the PID-IV origin seed.
pub fn get_seed(enc: &Slot4, seed: u32, level_min: u8, level_max: u8, format: u8) -> LeadSeed {
    get_seed_depth(enc, seed, level_min, level_max, format, 0)
}

fn get_seed_depth(
    enc: &Slot4,
    seed: u32,
    level_min: u8,
    level_max: u8,
    format: u8,
    depth: u32,
) -> LeadSeed {
    let pid = sequential_pid(seed);
    let nature = (pid % 25) as u8;
    let frames = reversal_window(seed, nature);
    origin_seed(
        enc, seed, nature, frames, level_min, level_max, format, depth,
    )
}

#[allow(clippy::too_many_arguments)]
fn origin_seed(
    enc: &Slot4,
    mut seed: u32,
    nature: u8,
    mut reverse_count: usize,
    level_min: u8,
    level_max: u8,
    format: u8,
    depth: u32,
) -> LeadSeed {
    let mut prefer = LeadSeed::default();
    loop {
        if let Some(result) = try_match(enc, level_min, level_max, seed, nature, format, depth) {
            if result.lead == LeadRequired::None {
                return result;
            }
            if result.lead > prefer.lead {
                prefer = result;
            }
        }
        if reverse_count == 0 {
            return prefer;
        }
        reverse_count -= 1;
        seed = prev_seed(prev_seed(seed));
    }
}

/// PKHeX `MethodK.SkipToLevelRand`: from the origin seed to the state whose high half is the
/// level roll.
pub fn skip_to_level_rand(enc: &Slot4, seed: u32, lead: LeadRequired) -> u32 {
    if enc.slot_type.is_fishing_rod_hgss() || enc.slot_type == SlotType4::RockSmash {
        return next_n::<3>(seed);
    }
    if enc.slot_type == SlotType4::BugContest && !able_to_sweet_scent(lead) {
        return next_n::<4>(seed);
    }
    next_n::<2>(seed)
}

fn able_to_sweet_scent(lead: LeadRequired) -> bool {
    use LeadRequired as L;
    matches!(
        lead,
        L::None | L::IntimidateKeenEyeFail | L::PressureHustleSpirit | L::PressureHustleSpiritFail
    )
}

fn rod_rate(t: SlotType4) -> u32 {
    use SlotType4 as T;
    match t {
        T::OldRod | T::SafariOldRod => 25,
        T::GoodRod | T::SafariGoodRod => 50,
        T::SuperRod | T::SafariSuperRod => 75,
        _ => 0,
    }
}

/// PKHeX `MethodK.CheckEncounterActivation`.
fn check_activation(enc: &Slot4, result: &mut LeadSeed) -> bool {
    if enc.slot_type.is_fishing_rod_hgss() {
        return fish_possible(enc.slot_type, &mut result.seed, Some(&mut result.lead));
    }
    if enc.slot_type == SlotType4::RockSmash {
        return rock_smash_possible(enc.area_rate, &mut result.seed, Some(&mut result.lead));
    }
    if enc.slot_type == SlotType4::BugContest && !able_to_sweet_scent(result.lead) {
        return bug_contest_deadlock(enc.area_rate, &mut result.seed);
    }
    true
}

fn check_activation_cute_charm(enc: &Slot4, seed: &mut u32) -> bool {
    if enc.slot_type.is_fishing_rod_hgss() {
        return fish_possible(enc.slot_type, seed, None);
    }
    if enc.slot_type == SlotType4::RockSmash {
        return rock_smash_possible(enc.area_rate, seed, None);
    }
    if enc.slot_type == SlotType4::BugContest {
        return bug_contest_deadlock(enc.area_rate, seed);
    }
    true
}

fn bug_contest_deadlock(area_rate: u8, seed: &mut u32) -> bool {
    let rate = area_rate as u32 + (area_rate as u32 >> 1); // +50% White Flute
    if (*seed >> 16) % 100 >= rate {
        return false;
    }
    *seed = prev_seed(prev_seed(*seed));
    true
}

/// Both PKHeX `IsFishPossible` overloads: `lead` is `None` for the Cute Charm variant.
fn fish_possible(t: SlotType4, seed: &mut u32, lead: Option<&mut LeadRequired>) -> bool {
    let rate = rod_rate(t) + 50; // following Pokémon with high friendship: best case
    let roll = (*seed >> 16) % 100;
    if roll < rate {
        *seed = prev_seed(*seed);
        return true;
    }
    let Some(lead) = lead else {
        return false;
    };
    if *lead != LeadRequired::None {
        return false;
    }
    if roll < rate * 2 {
        *seed = prev_seed(*seed);
        *lead = LeadRequired::SuctionCups;
        return true;
    }
    false
}

fn rock_smash_possible(area_rate: u8, seed: &mut u32, lead: Option<&mut LeadRequired>) -> bool {
    let roll = (*seed >> 16) % 100;
    if roll < area_rate as u32 {
        *seed = prev_seed(*seed);
        return true;
    }
    let Some(lead) = lead else {
        return false;
    };
    if *lead != LeadRequired::None {
        return false;
    }
    if roll < area_rate as u32 * 2 {
        *seed = prev_seed(*seed);
        *lead = LeadRequired::Illuminate;
        return true;
    }
    false
}

/// PKHeX `MethodK.TryGetMatchCuteCharm` over candidate IV seeds.
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
        if !check_activation_cute_charm(enc, &mut s) {
            continue;
        }
        return Some(LeadSeed {
            seed: s,
            lead: LeadRequired::CuteCharm,
        });
    }
    None
}

const MAX_SAFARI_CONTEST: u32 = 4;

fn try_match(
    enc: &Slot4,
    level_min: u8,
    level_max: u8,
    seed: u32,
    nature: u8,
    format: u8,
    depth: u32,
) -> Option<LeadSeed> {
    let p0 = seed >> 16;
    if nature_from_rand(p0) == nature as u32 {
        let ctx = Frame::new(enc, seed, level_min, level_max, format);
        if let Some(mut r) = try_match_no_sync(&ctx) {
            if check_activation(enc, &mut r) {
                return Some(r);
            }
        }
        if depth != 4 && enc.rerolls_for_31 {
            return recurse_4x(enc, level_min, level_max, seed, nature, format, depth + 1);
        }
    } else if is_sync_pass(p0) && !(enc.slot_type == SlotType4::Grass && enc.level_max < level_min)
    {
        let ctx = Frame::new(enc, seed, level_min, level_max, format);
        // PKHeX passes `out seed` to IsSlotValidRegular, so the recursion below sees the slot
        // seed (or 0 when no slot matched), not the original seed. Mirrored for parity; see
        // docs/discrepancies.md.
        let slot_seed = slot_valid_regular(&ctx);
        if let Some(s) = slot_seed {
            let mut r = LeadSeed {
                seed: s,
                lead: LeadRequired::Synchronize,
            };
            if check_activation(enc, &mut r) {
                return Some(r);
            }
        }
        if depth != 4 && enc.rerolls_for_31 {
            let s = slot_seed.unwrap_or(0);
            return recurse_4x(enc, level_min, level_max, s, nature, format, depth + 1);
        }
    }
    None
}

fn is_any_31(iv16: u32) -> bool {
    iv16 & 0x1F == 0x1F || (iv16 >> 5) & 0x1F == 0x1F || (iv16 >> 10) & 0x1F == 0x1F
}

/// PKHeX `MethodK.Recurse4x`: Safari Zone and Bug-Catching Contest encounters re-roll
/// {nature, PID/IV} up to 3 times while no IV is 31.
fn recurse_4x(
    enc: &Slot4,
    level_min: u8,
    level_max: u8,
    mut seed: u32,
    nature: u8,
    format: u8,
    depth: u32,
) -> Option<LeadSeed> {
    if depth == MAX_SAFARI_CONTEST {
        return None;
    }
    let mut break_next = false;
    loop {
        let iv2 = seed >> 16;
        if is_any_31(iv2) {
            return None;
        }
        let iv1 = prev16(&mut seed) as u32;
        if is_any_31(iv1) {
            return None;
        }
        let sanity_nature = sequential_pid(seed) % 25;
        if sanity_nature == nature as u32 {
            break_next = true;
        }
        let origin = prev_seed(prev_seed(seed));
        let r = get_seed_depth(enc, origin, level_min, level_max, format, depth);
        if r.is_valid() {
            return Some(r);
        }
        if break_next {
            return None;
        }
    }
}

fn lead(seed: u32, lead: LeadRequired) -> Option<LeadSeed> {
    Some(LeadSeed { seed, lead })
}

fn try_match_no_sync(ctx: &Frame) -> Option<LeadSeed> {
    use LeadRequired as L;
    let enc = ctx.enc;
    if enc.slot_type == SlotType4::Grass && enc.level_max < ctx.level_min {
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
    None
}

fn slot_valid(enc: &Slot4, rand: u32) -> bool {
    slot_from_rand(enc.slot_type, rand) == enc.slot_number
}

fn random_level(enc: &Slot4, rand: u32) -> u32 {
    let modulo = 1 + enc.level_max as u32 - enc.level_min as u32;
    rand % modulo + enc.level_min as u32
}

/// PKHeX `MethodK.GetRandomLevel(enc, seed, lead)`.
pub fn level_for(enc: &Slot4, rand: u32, lead: LeadRequired) -> u32 {
    if lead == LeadRequired::PressureHustleSpirit {
        return enc.pressure_level as u32;
    }
    random_level(enc, rand)
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
    if is_level_rand(enc) {
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
    if is_level_rand(enc) {
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
    if is_level_rand(enc) {
        slot_valid(enc, ctx.prev3()).then(|| ctx.seed4())
    } else {
        slot_valid(enc, ctx.prev2()).then_some(ctx.seed3)
    }
}

fn slot_valid_static_magnet(ctx: &Frame) -> Option<(u32, LeadRequired)> {
    let enc = ctx.enc;
    if is_level_rand(enc) {
        if is_static_magnet_fail(ctx.prev3()) {
            return None;
        }
        if enc.is_fixed_level() || level_valid(ctx, ctx.prev1()) {
            if let Some(l) = enc.static_magnet_lead(ctx.prev2()) {
                return Some((ctx.seed4(), l));
            }
        }
    } else {
        if is_static_magnet_fail(ctx.prev2()) {
            return None;
        }
        if let Some(l) = enc.static_magnet_lead(ctx.prev1()) {
            return Some((ctx.seed3, l));
        }
    }
    None
}

fn slot_valid_static_magnet_fail(ctx: &Frame) -> Option<u32> {
    let enc = ctx.enc;
    if is_level_rand(enc) {
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
            return Some(ctx.seed2);
        }
    }
    None
}

/// PKHeX `LeadFinder.GetLeadInfo4` for HG/SS origins (no Poké Radar in HG/SS).
pub fn lead_info4_hgss(
    pk: &super::finder::AnalyzeInput,
    enc: &Slot4,
    pidiv: super::finder::PidIv,
    level_min: u8,
    level_max: u8,
    format: u8,
    ratio: impl Fn(u16) -> u8,
) -> LeadSeed {
    use super::finder::{is_cute_charm, PidType};
    let mut kind = pidiv.kind;
    match kind {
        PidType::ChainShiny => {
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
