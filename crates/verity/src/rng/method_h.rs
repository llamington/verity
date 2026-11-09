//! Method H, the wild encounter RNG of the Gen 3 games. Ported from PKHeX
//! `Legality/RNG/ClassicEra/Gen3/MethodH.cs` and `SlotMethodH.cs`.

use super::lcrng::{next16, next_n, prev16, prev_seed};
use super::method_j::{reversal_window, sequential_pid, LeadRequired, LeadSeed};

pub const FORMAT: u8 = 3;
const RSE_SAFARI: u8 = 57;
const UNOWN: u16 = 201;

/// PKHeX `SlotType3`.
pub mod slot_type {
    pub const GRASS: u8 = 0;
    pub const SURF: u8 = 1;
    pub const OLD_ROD: u8 = 2;
    pub const GOOD_ROD: u8 = 3;
    pub const SUPER_ROD: u8 = 4;
    pub const ROCK_SMASH: u8 = 5;
    pub const SWARM_GRASS: u8 = 6;
    pub const SWARM_FISH: u8 = 7;
}

/// The fields of PKHeX `IEncounterSlot3` that Method H reads.
#[derive(Clone, Copy, Debug)]
pub struct Slot3 {
    pub species: u16,
    pub form: u8,
    pub level_min: u8,
    pub level_max: u8,
    pub slot_number: u8,
    pub magnet_pull: (u8, u8),
    pub static_slot: (u8, u8),
    pub slot_type: u8,
    pub area_rate: u8,
    pub location: u8,
    pub pressure_level: u8,
}

impl Slot3 {
    pub fn from_encounter(e: &crate::data::encounters::Encounter) -> Self {
        Self {
            species: e.species(),
            form: e.form(),
            level_min: e.level_min(),
            level_max: e.level_max(),
            slot_number: e.slot_number(),
            magnet_pull: e.magnet_pull(),
            static_slot: e.static_slot(),
            slot_type: e.slot_type(),
            area_rate: e.area_rate(),
            location: e.location() as u8,
            pressure_level: e.pressure_level(),
        }
    }

    fn is_safari_hoenn(&self) -> bool {
        self.location == RSE_SAFARI
    }

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

/// `EntityPID.GetUnownForm3`.
pub fn unown_form3(pid: u32) -> u8 {
    let v =
        ((pid & 0x0300_0000) >> 18) | ((pid & 0x3_0000) >> 12) | ((pid & 0x300) >> 6) | (pid & 3);
    (v % 28) as u8
}

fn reverse_pid(mut seed: u32) -> u32 {
    let a = next16(&mut seed) as u32;
    let b = next16(&mut seed) as u32;
    a << 16 | b
}

/// `SlotMethodH.GetSlot`; 255 never matches a slot.
pub fn slot_from_rand(t: u8, rand: u32) -> u8 {
    use slot_type::*;
    let r = rand % 100;
    match t {
        GRASS => match r {
            0..=19 => 0,
            20..=39 => 1,
            40..=49 => 2,
            50..=59 => 3,
            60..=69 => 4,
            70..=79 => 5,
            80..=84 => 6,
            85..=89 => 7,
            90..=93 => 8,
            94..=97 => 9,
            98 => 10,
            _ => 11,
        },
        SURF | ROCK_SMASH => match r {
            0..=59 => 0,
            60..=89 => 1,
            90..=94 => 2,
            95..=98 => 3,
            _ => 4,
        },
        OLD_ROD => u8::from(r >= 70),
        GOOD_ROD => match r {
            0..=59 => 0,
            60..=79 => 1,
            _ => 2,
        },
        SUPER_ROD => match r {
            0..=39 => 0,
            40..=79 => 1,
            80..=94 => 2,
            95..=98 => 3,
            _ => 4,
        },
        SWARM_FISH | SWARM_GRASS if r < 50 => 0,
        _ => 255,
    }
}

/// The met-level constraint and the current format.
#[derive(Clone, Copy, Debug)]
pub struct Bounds {
    pub min: u8,
    pub max: u8,
    pub format: u8,
}

struct Frame<'a> {
    enc: &'a Slot3,
    b: Bounds,
    seeds: [u32; 4],
}

impl<'a> Frame<'a> {
    fn new(enc: &'a Slot3, seed: u32, b: Bounds) -> Self {
        let s1 = prev_seed(seed);
        let s2 = prev_seed(s1);
        let s3 = prev_seed(s2);
        let s4 = prev_seed(s3);
        Self {
            enc,
            b,
            seeds: [s1, s2, s3, s4],
        }
    }
    fn prev1(&self) -> u32 {
        self.seeds[0] >> 16
    }
    fn prev2(&self) -> u32 {
        self.seeds[1] >> 16
    }
    fn prev3(&self) -> u32 {
        self.seeds[2] >> 16
    }
    fn seed3(&self) -> u32 {
        self.seeds[2]
    }
    fn seed4(&self) -> u32 {
        self.seeds[3]
    }

    fn original_level_ok(&self, level: u32) -> bool {
        if self.b.format == FORMAT && self.b.min > 1 {
            return level == self.b.min as u32;
        }
        (self.b.min as u32..=self.b.max as u32).contains(&level)
    }

    fn level_ok(&self, rand: u32) -> bool {
        let span = 1 + self.enc.level_max as u32 - self.enc.level_min as u32;
        self.original_level_ok(rand % span + self.enc.level_min as u32)
    }

    fn level_ok_minus1(&self, rand: u32) -> bool {
        let span = 1 + self.enc.level_max as u32 - self.enc.level_min as u32;
        let bias = (rand % span).saturating_sub(1);
        self.original_level_ok(self.enc.level_min as u32 + bias)
    }

    fn slot_ok(&self, rand: u32) -> bool {
        slot_from_rand(self.enc.slot_type, rand) == self.enc.slot_number
    }

    fn regular(&self) -> Option<u32> {
        (self.level_ok(self.prev1()) && self.slot_ok(self.prev2())).then(|| self.seed3())
    }

    fn from1skip(&self) -> Option<u32> {
        (self.level_ok(self.prev2()) && self.slot_ok(self.prev3())).then(|| self.seed4())
    }

    fn static_magnet(&self) -> Option<(u32, LeadRequired)> {
        if self.prev3() & 1 != 0 || !self.level_ok(self.prev1()) {
            return None;
        }
        self.enc
            .static_magnet_lead(self.prev2())
            .map(|l| (self.seed4(), l))
    }

    fn static_magnet_fail(&self) -> Option<u32> {
        if self.prev3() & 1 == 0 {
            return None;
        }
        (self.level_ok(self.prev1()) && self.slot_ok(self.prev2())).then(|| self.seed4())
    }

    fn hustle_vital(&self) -> Option<u32> {
        if self.prev1() & 1 != 1 || !self.original_level_ok(self.enc.pressure_level as u32) {
            return None;
        }
        self.slot_ok(self.prev3()).then(|| self.seed4())
    }

    fn hustle_vital_fail(&self) -> Option<u32> {
        if self.prev1() & 1 == 1 {
            return None;
        }
        (self.level_ok_minus1(self.prev2()) && self.slot_ok(self.prev3())).then(|| self.seed4())
    }

    /// `TryGetMatchNoSync`.
    fn no_sync(&self) -> Option<LeadSeed> {
        use LeadRequired as L;
        let ok = |seed, lead| Some(LeadSeed { seed, lead });
        if let Some(s) = self.regular() {
            return ok(s, L::None);
        }
        // SyncFail: prev1 must fail the proc.
        if self.prev1() & 1 != 0 {
            if let Some(s) = self.from1skip() {
                return ok(s, L::SynchronizeFail);
            }
        }
        if self.prev1().is_multiple_of(3) {
            if let Some(s) = self.from1skip() {
                return ok(s, L::CuteCharmFail);
            }
        }
        if let Some(s) = self.hustle_vital_fail() {
            return ok(s, L::PressureHustleSpiritFail);
        }
        if let Some(s) = self.static_magnet_fail() {
            return ok(s, L::StaticMagnetFail);
        }
        if let Some((s, l)) = self.static_magnet() {
            return ok(s, l);
        }
        if let Some(s) = self.hustle_vital() {
            return ok(s, L::PressureHustleSpirit);
        }
        // Intimidate/Keen Eye: an odd roll is the pass that aborts the encounter.
        if self.prev1() & 1 != 1 {
            if let Some(s) = self.from1skip() {
                return ok(s, L::IntimidateKeenEyeFail);
            }
        }
        if !self.prev1().is_multiple_of(3) {
            if let Some(s) = self.from1skip() {
                return ok(s, L::CuteCharm);
            }
        }
        None
    }
}

/// `IsRockSmashPossible`, with PKHeX's `IsRatePass` assumptions (White Flute only).
fn rock_smash_possible(rate: u8, seed: &mut u32) -> bool {
    let mut enc_rate = rate as u32 * 16;
    enc_rate += enc_rate >> 1;
    if (*seed >> 16) % 2880 < enc_rate {
        *seed = prev_seed(*seed);
        return true;
    }
    false
}

fn activation_ok(enc: &Slot3, seed: &mut u32) -> bool {
    enc.slot_type != slot_type::ROCK_SMASH || rock_smash_possible(enc.area_rate, seed)
}

/// `0xC048C851 * seed + 0x196302B4`: 300 calls back, before the Safari block nature table.
fn safari_block_seed(seed: u32) -> u32 {
    0xC048_C851u32.wrapping_mul(seed).wrapping_add(0x1963_02B4)
}

fn safari_block_proc(seed: u32) -> bool {
    (seed >> 16) % 100 < 80
}

fn no_lead(enc: &Slot3, b: Bounds, mut seed: u32, nature: u8) -> Option<LeadSeed> {
    let none = |s| {
        Some(LeadSeed {
            seed: s,
            lead: LeadRequired::None,
        })
    };
    let p0 = seed >> 16;
    let hoenn = enc.is_safari_hoenn();
    if hoenn {
        let block = safari_block_seed(seed);
        if safari_block_proc(block) {
            if let Some(s) = Frame::new(enc, block, b).regular() {
                return none(s);
            }
        }
    }
    if enc.species == UNOWN {
        // No nature call in the loop; shift the frames so the values line up.
        let shifted = next_n::<1>(seed);
        if let Some(s) = Frame::new(enc, shifted, b).regular() {
            return none(s);
        }
    } else if p0 % 25 == nature as u32 {
        if hoenn {
            seed = prev_seed(seed);
        }
        if let Some(s) = Frame::new(enc, seed, b).regular() {
            return none(s);
        }
    }
    None
}

fn match_emerald(enc: &Slot3, b: Bounds, mut seed: u32, nature: u8) -> Option<LeadSeed> {
    let p0 = seed >> 16;
    if enc.is_safari_hoenn() {
        let block = safari_block_seed(seed);
        if safari_block_proc(block) {
            if let Some(r) = Frame::new(enc, block, b).no_sync() {
                return Some(r);
            }
        }
        seed = prev_seed(seed);
    }
    let frame = Frame::new(enc, seed, b);
    if p0 & 1 == 0 {
        if let Some(s) = frame.regular() {
            return Some(LeadSeed {
                seed: s,
                lead: LeadRequired::Synchronize,
            });
        }
    }
    if p0 % 25 == nature as u32 {
        if let Some(r) = frame.no_sync() {
            return Some(r);
        }
    }
    None
}

/// `GetReversalWindow` for Unown (FR/LG): the form is what the loop rolls for.
fn window_unown(mut seed: u32, form: u8) -> usize {
    let mut ctr = 0;
    let mut b = seed >> 16;
    loop {
        let a = prev16(&mut seed) as u32;
        if unown_form3(a << 16 | b) == form {
            return ctr;
        }
        b = prev16(&mut seed) as u32;
        ctr += 1;
    }
}

/// `GetReversalWindowCute`: (frames with no lead, further frames with a matching Cute Charm).
fn window_cute(mut seed: u32, nature: u8, gmin: u8, gmax: u8) -> (usize, usize) {
    let (mut no_lead, mut ctr) = (0usize, 0usize);
    let mut b = seed >> 16;
    loop {
        let a = prev16(&mut seed) as u32;
        let pid = b << 16 | a;
        if pid % 25 == nature as u32 {
            if no_lead == 0 {
                no_lead = ctr;
            }
            let g = (pid & 0xFF) as u8;
            if gmin <= g && g <= gmax {
                return (no_lead, ctr - no_lead);
            }
        }
        b = prev16(&mut seed) as u32;
        ctr += 1;
    }
}

/// PKHeX `PersonalInfo.GetGenderMinMax`.
fn gender_min_max(gender: u8, ratio: u8) -> (u8, u8) {
    match ratio {
        0 | 254 | 255 => (0, 255),
        _ if gender == 0 => (ratio, 255),
        _ => (0, ratio - 1),
    }
}

/// `MethodH.GetSeed`.
pub fn get_seed(
    enc: &Slot3,
    seed: u32,
    b: Bounds,
    emerald: bool,
    gender: u8,
    ratio: impl Fn(u16) -> u8,
) -> LeadSeed {
    let pid = if enc.species != UNOWN {
        sequential_pid(seed)
    } else {
        reverse_pid(seed)
    };
    let nature = (pid % 25) as u8;
    if emerald {
        let r = ratio(enc.species);
        let (regular, cute) = if r == 255 {
            (reversal_window(seed, nature), 0)
        } else {
            let (lo, hi) = gender_min_max(gender, r);
            window_cute(seed, nature, lo, hi)
        };
        return origin_emerald(enc, b, seed, nature, (regular, cute));
    }
    let count = if enc.species == UNOWN {
        window_unown(seed, enc.form)
    } else {
        reversal_window(seed, nature)
    };
    origin_regular(enc, b, seed, nature, count)
}

fn origin_regular(
    enc: &Slot3,
    b: Bounds,
    mut seed: u32,
    nature: u8,
    mut reverse: usize,
) -> LeadSeed {
    loop {
        if let Some(mut r) = no_lead(enc, b, seed, nature) {
            if activation_ok(enc, &mut r.seed) {
                return r;
            }
        }
        if reverse == 0 {
            return LeadSeed::default();
        }
        reverse -= 1;
        seed = prev_seed(prev_seed(seed));
    }
}

fn origin_emerald(
    enc: &Slot3,
    b: Bounds,
    mut seed: u32,
    nature: u8,
    (mut reverse, mut cute): (usize, usize),
) -> LeadSeed {
    let mut prefer = LeadSeed::default();
    loop {
        if let Some(mut r) = match_emerald(enc, b, seed, nature) {
            if activation_ok(enc, &mut r.seed) {
                if r.lead == LeadRequired::None {
                    return r;
                }
                if r.lead > prefer.lead {
                    prefer = r;
                }
            }
        }
        if reverse == 0 {
            if cute == 0 || prefer.is_valid() {
                return prefer;
            }
            break;
        }
        reverse -= 1;
        seed = prev_seed(prev_seed(seed));
    }
    loop {
        if let Some(mut r) = match_emerald(enc, b, seed, nature) {
            if r.lead == LeadRequired::None {
                r.lead = LeadRequired::CuteCharm;
                if activation_ok(enc, &mut r.seed) {
                    if r.lead == LeadRequired::None {
                        return r;
                    }
                    if r.lead > prefer.lead {
                        prefer = r;
                    }
                }
            }
        }
        cute -= 1;
        if cute == 0 {
            return prefer;
        }
        seed = prev_seed(prev_seed(seed));
    }
}
