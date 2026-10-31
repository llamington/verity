//! PID/IV correlation detection. Ported from PKHeX `Legality/RNG/MethodFinder.cs` (`Analyze`)
//! with its helpers from `CommonEvent3.cs`, `CuteCharm4.cs`, `PokewalkerRNG.GetPID`,
//! `MonochromeRNG.GetShinyPID` and `MethodCXD.TryGetSeedStarterColo`.

use super::lcrng::{next15, next_n, prev16, prev_seed};
use super::reversal::{
    seeds, seeds_ivs, seeds_skip, MAX_SEEDS_IV, MAX_SEEDS_PID, MAX_SEEDS_PID_SKIP,
};
use super::xdrng::{self, arng};

/// PKHeX `PIDType`, same discriminants.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum PidType {
    None = 0,
    Method1,
    Method2,
    Method3,
    Method4,
    Method1Unown,
    Method2Unown,
    Method3Unown,
    Method4Unown,
    Method1Roamer,
    Bacd,
    BacdA,
    BacdAx,
    BacdS,
    BacdEa,
    BacdEax,
    BacdEs,
    BacdU,
    BacdUAx,
    BacdR,
    BacdRA,
    BacdRbcd,
    BacdTa,
    BacdTs,
    BacdM,
    CuteCharm,
    ChainShiny,
    Cxd,
    CxdAnti,
    CxdColoStarter,
    Channel,
    PokeSpot,
    G4MgAntiShiny,
    G5MgShiny,
    Pokewalker,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PidIv {
    pub kind: PidType,
    pub seed: u32,
}

impl PidIv {
    const NONE: Self = Self::new(PidType::None, 0);

    const fn new(kind: PidType, seed: u32) -> Self {
        Self { kind, seed }
    }
}

const VERSION_S: u8 = 1;
const VERSION_R: u8 = 2;
const VERSION_CXD: u8 = 15;
const GEN4_VERSIONS: [u8; 5] = [7, 8, 10, 11, 12];

const SHEDINJA: u16 = 292;
const NINCADA: u16 = 290;
const MARILL: u16 = 183;
const AZUMARILL: u16 = 184;
const AZURILL: u16 = 298;
const FROSLASS: u16 = 478;
const SNORUNT: u16 = 361;
const UNOWN: u16 = 201;
const ESPEON: u16 = 196;
const UMBREON: u16 = 197;
const RATIO_FEMALE: u8 = 254;
const RATIO_GENDERLESS: u8 = 255;
const RATIO_MALE: u8 = 0;

/// The PKM fields `MethodFinder.Analyze` reads.
#[derive(Clone, Copy, Debug)]
pub struct AnalyzeInput {
    pub pid: u32,
    /// IVs in PKM order (HP, Atk, Def, Spe, SpA, SpD), 5 bits each.
    pub iv32: u32,
    pub tid: u16,
    pub sid: u16,
    pub species: u16,
    pub gender: u8,
    pub version: u8,
    pub met_level: u8,
    pub ot_gender: u8,
}

impl AnalyzeInput {
    pub fn from_pkm(pk: &crate::pkm::Pkm) -> Self {
        Self {
            pid: pk.pid(),
            iv32: pk.iv32() & 0x3FFF_FFFF,
            tid: pk.tid(),
            sid: pk.sid(),
            species: pk.species(),
            gender: pk.gender(),
            version: pk.version(),
            met_level: pk.met_level(),
            ot_gender: pk.ot_gender(),
        }
    }

    fn id_xor(&self) -> u32 {
        (self.tid ^ self.sid) as u32
    }

    fn id32(&self) -> u32 {
        (self.sid as u32) << 16 | self.tid as u32
    }

    fn is_shiny(&self) -> bool {
        shiny_xor(self.pid ^ self.id32()) < 8
    }
}

fn shiny_xor(x: u32) -> u32 {
    (x >> 16) ^ (x & 0xFFFF)
}

fn is_shiny3(id32: u32, pid: u32) -> bool {
    shiny_xor(id32 ^ pid) < 8
}

/// `ratio(species)` must return the Gen 4 (HGSS) gender ratio for species <= 493 and the
/// PKHeX `EntityGender` ratio otherwise.
pub fn analyze(pk: &AnalyzeInput, ratio: impl Fn(u16) -> u8) -> PidIv {
    let pid = pk.pid;
    let top = pid & 0xFFFF_0000;
    let bot = pid << 16;
    let iv1 = pk.iv32 & 0x7FFF;
    let iv2 = pk.iv32 >> 15;

    if let Some(r) = lcrng_match(top, bot, iv1, iv2) {
        return r;
    }
    if pk.species == UNOWN {
        if let Some(r) = lcrng_unown_match(top, bot, iv1, iv2) {
            return r;
        }
    }
    if let Some(r) = colo_starter_match(pk, iv1, iv2) {
        return r;
    }
    if let Some(r) = xdrng_match(pk, top, bot, iv1, iv2) {
        return r;
    }
    if let Some(r) = roamer_match(top, bot, pk.iv32) {
        return r;
    }
    if let Some(r) = channel_match(pk, top, bot) {
        return r;
    }
    if let Some(r) = mg4_match(pid, iv1, iv2) {
        return r;
    }
    if let Some(r) = bacd_match(pk, iv1, iv2) {
        return r;
    }
    if let Some(r) = modified_pid_match(pk, &ratio, iv1, iv2) {
        return r;
    }
    PidIv::NONE
}

fn lcrng_match(top: u32, bot: u32, iv1: u32, iv2: u32) -> Option<PidIv> {
    let mut buf = [0u32; MAX_SEEDS_PID];
    let n = seeds(&mut buf, bot, top);
    for &seed in &buf[..n] {
        let mut s = next_n::<2>(seed);
        if iv1 == next15(&mut s) as u32 {
            if iv2 == next15(&mut s) as u32 {
                return Some(PidIv::new(PidType::Method1, seed));
            }
            if iv2 == next15(&mut s) as u32 {
                return Some(PidIv::new(PidType::Method4, seed));
            }
        } else {
            if iv1 != next15(&mut s) as u32 {
                continue;
            }
            if iv2 == next15(&mut s) as u32 {
                return Some(PidIv::new(PidType::Method2, seed));
            }
        }
    }
    let mut buf = [0u32; MAX_SEEDS_PID_SKIP];
    let n = seeds_skip(&mut buf, bot, top);
    for &seed in &buf[..n] {
        let mut s = next_n::<3>(seed);
        if iv1 != next15(&mut s) as u32 || iv2 != next15(&mut s) as u32 {
            continue;
        }
        return Some(PidIv::new(PidType::Method3, seed));
    }
    None
}

fn lcrng_unown_match(top: u32, bot: u32, iv1: u32, iv2: u32) -> Option<PidIv> {
    let mut buf = [0u32; MAX_SEEDS_PID];
    let n = seeds(&mut buf, top, bot);
    for &seed in &buf[..n] {
        let mut s = next_n::<2>(seed);
        if iv1 == next15(&mut s) as u32 {
            if iv2 == next15(&mut s) as u32 {
                return Some(PidIv::new(PidType::Method1Unown, seed));
            }
            if iv2 == next15(&mut s) as u32 {
                return Some(PidIv::new(PidType::Method4Unown, seed));
            }
        } else {
            if iv1 != next15(&mut s) as u32 {
                continue;
            }
            if iv2 == next15(&mut s) as u32 {
                return Some(PidIv::new(PidType::Method2Unown, seed));
            }
        }
    }
    let mut buf = [0u32; MAX_SEEDS_PID_SKIP];
    let n = seeds_skip(&mut buf, top, bot);
    for &seed in &buf[..n] {
        let mut s = next_n::<3>(seed);
        if iv1 != next15(&mut s) as u32 || iv2 != next15(&mut s) as u32 {
            continue;
        }
        return Some(PidIv::new(PidType::Method3Unown, seed));
    }
    None
}

fn roamer_match(top: u32, bot: u32, iv32: u32) -> Option<PidIv> {
    if iv32 > 0xFF {
        return None;
    }
    let mut buf = [0u32; MAX_SEEDS_PID];
    let n = seeds(&mut buf, bot, top);
    buf[..n]
        .iter()
        .find(|&&seed| iv32 == (next_n::<3>(seed) >> 16) & 0xFF)
        .map(|&seed| PidIv::new(PidType::Method1Roamer, seed))
}

fn xd_ivs_match(iv1: u32, iv2: u32, mut seed: u32) -> bool {
    iv1 == xdrng::next15(&mut seed) && iv2 == xdrng::next15(&mut seed)
}

fn colo_starter_pid(seed: &mut u32, id32: u32) -> u32 {
    loop {
        let a = xdrng::next16(seed);
        let b = xdrng::next16(seed);
        let pid = a << 16 | b;
        if (pid & 0xFF) >= 31 && !is_shiny3(id32, pid) {
            return pid;
        }
    }
}

fn colo_starter_match(pk: &AnalyzeInput, iv1: u32, iv2: u32) -> Option<PidIv> {
    let starter = pk.version == VERSION_CXD
        && matches!((pk.species, pk.met_level), (ESPEON, 25..) | (UMBREON, 26..));
    if !starter {
        return None;
    }
    let id32 = pk.id32();
    let mut buf = [0u32; xdrng::MAX_SEEDS_PID];
    let n = xdrng::seeds(&mut buf, (pk.tid as u32) << 16, (pk.sid as u32) << 16);
    for &seed in &buf[..n] {
        if pk.species == UMBREON && !xd_ivs_match(iv1, iv2, xdrng::next_n(seed, 4)) {
            continue;
        }
        let mut u = xdrng::next_n(seed, 7);
        let pid = colo_starter_pid(&mut u, id32);
        if pk.species == UMBREON {
            if pid != pk.pid {
                continue;
            }
            return Some(PidIv::new(PidType::CxdColoStarter, seed));
        }
        let after_fake = xdrng::next_n(u, 2);
        if !xd_ivs_match(iv1, iv2, after_fake) {
            continue;
        }
        let mut e = xdrng::next_n(after_fake, 3);
        if colo_starter_pid(&mut e, id32) != pk.pid {
            continue;
        }
        return Some(PidIv::new(PidType::CxdColoStarter, seed));
    }
    None
}

fn xdrng_match(pk: &AnalyzeInput, top: u32, bot: u32, iv1: u32, iv2: u32) -> Option<PidIv> {
    let mut buf = [0u32; xdrng::MAX_SEEDS_PID];
    let n = xdrng::seeds(&mut buf, top, bot);
    for &seed in &buf[..n] {
        let mut b = xdrng::prev(seed);
        let mut a = xdrng::prev(b);
        let hi = (a >> 16) & 0x7FFF;
        let lo = (b >> 16) & 0x7FFF;
        if hi == iv1 && lo == iv2 {
            return Some(PidIv::new(PidType::Cxd, xdrng::prev(a)));
        }
        let tsv = pk.id_xor() >> 3;
        let psv = (top ^ bot) >> (16 + 3);
        if psv == tsv {
            continue;
        }
        let mut p2 = seed;
        let mut p1 = b;
        if (p2 ^ p1) >> 19 != tsv {
            continue;
        }
        loop {
            b = xdrng::prev(a);
            a = xdrng::prev(b);
            if (a >> 16) & 0x7FFF == iv1 && (b >> 16) & 0x7FFF == iv2 {
                return Some(PidIv::new(PidType::CxdAnti, xdrng::prev(a)));
            }
            p2 = xdrng::prev(p1);
            p1 = xdrng::prev(p2);
            if (p2 ^ p1) >> 19 != tsv {
                break;
            }
        }
    }
    None
}

fn channel_match(pk: &AnalyzeInput, mut top: u32, bot: u32) -> Option<PidIv> {
    if pk.version != VERSION_R && pk.version != VERSION_S {
        return None;
    }
    let undo = (top >> 16) ^ 0x8000;
    if (if undo > 7 { 0 } else { 1 }) != ((bot >> 16) ^ pk.sid as u32 ^ 40122) {
        top = undo << 16;
    }
    let mut buf = [0u32; xdrng::MAX_SEEDS_PID];
    let n = xdrng::seeds(&mut buf, top, bot);
    for &seed in &buf[..n] {
        let c = xdrng::next_n(seed, 3);
        let d = xdrng::next(c);
        if (d >> 31) + 1 != pk.version as u32 {
            continue;
        }
        let e = xdrng::next(d);
        if e >> 31 != pk.ot_gender as u32 {
            continue;
        }
        if xdrng::sequential_iv32(e) != pk.iv32 {
            continue;
        }
        if seed >> 16 != pk.sid as u32 {
            continue;
        }
        return Some(PidIv::new(PidType::Channel, xdrng::prev(seed)));
    }
    None
}

fn mg4_match(pid: u32, iv1: u32, iv2: u32) -> Option<PidIv> {
    let psv = |v: u32| ((v >> 16) ^ (v & 0xFFFF)) >> 3;
    let current = psv(pid);
    let mut pid = arng::prev(pid);
    let original = psv(pid);
    if original == current {
        return None;
    }
    for _ in 0..3 {
        let mut buf = [0u32; MAX_SEEDS_PID];
        let n = seeds(&mut buf, pid << 16, pid & 0xFFFF_0000);
        for &seed in &buf[..n] {
            let mut s = next_n::<2>(seed);
            if iv1 == next15(&mut s) as u32 && iv2 == next15(&mut s) as u32 {
                return Some(PidIv::new(PidType::G4MgAntiShiny, seed));
            }
        }
        pid = arng::prev(pid);
        if psv(pid) != original {
            break;
        }
    }
    None
}

fn antishiny(a16: u32, b16: u32, id_xor: u32) -> u32 {
    ((a16 ^ (id_xor ^ b16)) << 16) | b16
}

fn force_shiny(x16: u32, b16: u32, id_xor: u32) -> u32 {
    (x16 << 16) | (((id_xor ^ x16) & 0xFFF8) | (b16 & 0b111))
}

fn bacd_match(pk: &AnalyzeInput, iv1: u32, iv2: u32) -> Option<PidIv> {
    let actual = pk.pid;
    let mut buf = [0u32; MAX_SEEDS_IV];
    let n = seeds_ivs(&mut buf, iv1 << 16, iv2 << 16);
    let id_xor = pk.id_xor();
    let mut kind = PidType::Bacd;
    for &x in &buf[..n] {
        let mut seed = x;
        let b16 = seed >> 16;
        let a16 = prev16(&mut seed) as u32;
        let expect = a16 << 16 | b16;
        if expect != actual {
            let shiny = shiny_xor(id_xor ^ actual) < 8;
            if !shiny {
                if (expect.wrapping_add(8) & 0xFFFF_FFF8) == actual {
                    kind = PidType::BacdA;
                } else if a16 & !0b111 != 0 && antishiny(a16, b16, id_xor) == actual {
                    kind = PidType::BacdAx;
                } else {
                    let x16 = prev16(&mut seed) as u32;
                    let low = (id_xor ^ x16) & 0xFFF8;
                    if x16 == actual >> 16
                        && (actual & 0b111) == (b16 & 0b111)
                        && low != (actual & 0xFFF8)
                    {
                        kind = PidType::BacdEs;
                    } else {
                        continue;
                    }
                }
            } else {
                let delta = actual.wrapping_sub(expect);
                if actual & 0b111 == 0 && delta != 0 && delta < 8 {
                    kind = PidType::BacdEa;
                } else if a16 & !0b111 != 0
                    && (actual & 0xFFFF) == b16
                    && antishiny(a16, b16, id_xor) >> 19 != actual >> 19
                {
                    kind = PidType::BacdEax;
                } else {
                    let x16 = prev16(&mut seed) as u32;
                    if x16 == actual >> 16 && force_shiny(x16, b16, id_xor) == actual {
                        kind = PidType::BacdS;
                    } else {
                        continue;
                    }
                }
            }
        }
        return Some(PidIv::new(kind, prev_seed(seed)));
    }
    None
}

fn modified_pid_match(
    pk: &AnalyzeInput,
    ratio: &impl Fn(u16) -> u8,
    iv1: u32,
    iv2: u32,
) -> Option<PidIv> {
    let pid = pk.pid;
    if pk.is_shiny() {
        if let Some(r) = chain_shiny_match(pk, iv1, iv2) {
            return Some(r);
        }
        let gen4 = GEN4_VERSIONS.contains(&pk.version);
        let cute = || pid <= 0xFF && is_cute_charm(pk, ratio);
        let g5 = || g5_mg_shiny(pk);
        // PKHeX `GetModified8BitMatch`: Gen 4 origins prefer Cute Charm, others the Gen 5 shiny.
        let kind = match (gen4, cute(), g5()) {
            (true, true, _) | (false, true, false) => Some(PidType::CuteCharm),
            (_, _, true) => Some(PidType::G5MgShiny),
            _ => None,
        };
        if let Some(kind) = kind {
            return Some(PidIv::new(kind, 0));
        }
    } else if pid <= 0xFF && is_cute_charm(pk, ratio) {
        return Some(PidIv::new(PidType::CuteCharm, 0));
    }
    pokewalker_match(pk, ratio).then_some(PidIv::new(PidType::Pokewalker, 0))
}

fn g5_mg_shiny(pk: &AnalyzeInput) -> bool {
    let low = pk.pid & 0xFFFF;
    if low > 0xFF {
        return false;
    }
    let ability_bit = (pk.pid >> 16) & 1;
    let mut expect = ((low ^ pk.tid as u32 ^ pk.sid as u32) << 16) | low;
    if (expect & 0x10000) != ability_bit << 16 {
        expect ^= 0x10000;
    }
    expect == pk.pid
}

fn chain_shiny_match(pk: &AnalyzeInput, iv1: u32, iv2: u32) -> Option<PidIv> {
    let mut buf = [0u32; MAX_SEEDS_IV];
    let n = seeds_ivs(&mut buf, iv1 << 16, iv2 << 16);
    buf[..n]
        .iter()
        .find_map(|&seed| chain_shiny_origin(pk.tid, pk.sid, pk.pid, seed))
        .map(|s| PidIv::new(PidType::ChainShiny, s))
}

/// PKHeX `MethodFinder.IsChainShinyValid`; returns the origin seed.
pub fn chain_shiny_origin(tid: u16, sid: u16, pid: u32, seed: u32) -> Option<u32> {
    let mut s = seed;
    let mut i = 15;
    loop {
        if (s >> 16) & 1 != (pid >> i) & 1 {
            break;
        }
        s = prev_seed(s);
        i -= 1;
        if i == 2 {
            break;
        }
    }
    if i != 2 {
        return None;
    }
    let upper = s;
    if (upper >> 16) & 7 != (pid >> 16) & 7 {
        return None;
    }
    let lower = prev_seed(upper);
    if (lower >> 16) & 7 != pid & 7 {
        return None;
    }
    let upid = (((pid & 0xFFFF) ^ tid as u32 ^ sid as u32) & 0xFFF8) | ((upper >> 16) & 7);
    if upid != pid >> 16 {
        return None;
    }
    Some(prev_seed(prev_seed(lower)))
}

fn gender_from_pid(pid: u32, ratio: u8) -> u8 {
    match ratio {
        RATIO_GENDERLESS => 2,
        RATIO_FEMALE => 1,
        RATIO_MALE => 0,
        r => ((pid & 0xFF) < r as u32) as u8,
    }
}

fn is_azurill_male(pid: u32) -> bool {
    (0xC8..=0xE0).contains(&pid)
}

/// PKHeX `CuteCharm4.IsCuteCharm`.
pub fn is_cute_charm(pk: &AnalyzeInput, ratio: &impl Fn(u16) -> u8) -> bool {
    let pid = pk.pid;
    if pid > 0xFF {
        return false;
    }
    let (species, gender) = match pk.species {
        SHEDINJA => (NINCADA, gender_from_pid(pid, ratio(NINCADA))),
        413 => (412, 1),
        414 => (412, 0),
        416 => (415, 1),
        475 => (281, 0),
        FROSLASS => (SNORUNT, 1),
        MARILL | AZUMARILL if is_azurill_male(pid) => (AZURILL, 0),
        s => (gen4_species(s), pk.gender),
    };
    let gr = ratio(species);
    match gender {
        0 => gr < RATIO_FEMALE && pid == 25 * (gr as u32 / 25 + 1) + pid % 25,
        1 => pid < 25 && gr < RATIO_FEMALE,
        _ => false,
    }
}

fn gen4_species(species: u16) -> u16 {
    match species {
        700 => 133,
        866 => 122,
        899 => 234,
        900 => 123,
        903 => 215,
        901 => 217,
        979 => 57,
        s => s,
    }
}

/// PKHeX `PokewalkerRNG.GetPID`.
pub fn pokewalker_pid(tid: u16, sid: u16, mut nature: u32, gender: u8, ratio: u8) -> u32 {
    if nature >= 24 {
        nature = 0;
    }
    let mut pid = ((((tid ^ sid) as u32) >> 8) ^ 0xFF) << 24;
    pid = pid.wrapping_add(nature.wrapping_sub(pid % 25));
    if ratio == 0 || ratio >= 0xFE {
        return pid;
    }
    let pid_gender = ((pid & 0xFF) < ratio as u32) as u8;
    if gender == pid_gender {
        return pid;
    }
    if gender == 0 {
        pid = pid.wrapping_add((((ratio as u32).wrapping_sub(pid & 0xFF) / 25) + 1) * 25);
        if (nature & 1) != (pid & 1) {
            pid = pid.wrapping_add(25);
        }
    } else {
        pid = pid.wrapping_sub((((pid & 0xFF).wrapping_sub(ratio as u32) / 25) + 1) * 25);
        if (nature & 1) != (pid & 1) {
            pid = pid.wrapping_sub(25);
        }
    }
    pid
}

fn pokewalker_match(pk: &AnalyzeInput, ratio: &impl Fn(u16) -> u8) -> bool {
    let actual = pk.pid;
    let mid = actual & 0x00FF_FF00;
    if mid != 0 && mid != 0x00FF_FF00 {
        return false;
    }
    let nature = actual % 25;
    if nature == 24 {
        return false;
    }
    let gr = if pk.species == FROSLASS {
        0x7F
    } else {
        ratio(pk.species)
    };
    if pokewalker_pid(pk.tid, pk.sid, nature, pk.gender, gr) == actual {
        return true;
    }
    pk.gender == 0
        && matches!(pk.species, MARILL | AZUMARILL)
        && gender_from_pid(actual, 191) == 1
        && pokewalker_pid(pk.tid, pk.sid, nature, 1, 191) == actual
}

#[cfg(test)]
mod tests {
    use super::super::lcrng::next_seed;
    use super::*;

    fn input(pid: u32, iv32: u32) -> AnalyzeInput {
        AnalyzeInput {
            pid,
            iv32,
            tid: 12345,
            sid: 54321,
            species: 25,
            gender: 0,
            version: 12,
            met_level: 5,
            ot_gender: 0,
        }
    }

    #[test]
    fn method1_roundtrip() {
        let mut seed = 0xACE0_1234u32;
        for _ in 0..500 {
            seed = next_seed(seed.rotate_left(3));
            let origin = seed;
            let mut s = origin;
            let lo = super::super::lcrng::next16(&mut s) as u32;
            let hi = super::super::lcrng::next16(&mut s) as u32;
            let iv1 = next15(&mut s) as u32;
            let iv2 = next15(&mut s) as u32;
            let r = analyze(&input(hi << 16 | lo, iv1 | iv2 << 15), |_| 127);
            assert_eq!(r.kind, PidType::Method1);
            assert_eq!(r.seed, origin);
        }
    }
}
