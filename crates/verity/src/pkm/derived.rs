//! Values derived from PKM fields. Ported from PKHeX `PKM.cs` (`LoadStats`, `IsShiny`),
//! `Editing/HiddenPower.cs`, `PKM/Util/Experience.cs`, `NatureAmp.cs`, `EntityGender.cs`.

use super::entity::{Pkm, Stats};
use super::exp_table::GROWTH;

/// The subset of species data needed here (PKHeX `PersonalInfo`).
#[derive(Clone, Copy, Debug)]
pub struct BaseInfo {
    pub base: Stats,
    pub gender_ratio: u8,
    pub growth: u8,
}

pub const RATIO_MALE: u8 = 0;
pub const RATIO_FEMALE: u8 = 254;
pub const RATIO_GENDERLESS: u8 = 255;

pub fn is_shiny(pid: u32, tid: u16, sid: u16) -> bool {
    shiny_xor(pid, tid, sid) < 8
}

pub fn shiny_xor(pid: u32, tid: u16, sid: u16) -> u16 {
    tid ^ sid ^ (pid >> 16) as u16 ^ pid as u16
}

/// 0 = male, 1 = female, 2 = genderless.
pub fn gender_from_pid(pid: u32, ratio: u8) -> u8 {
    match ratio {
        RATIO_GENDERLESS => 2,
        RATIO_FEMALE => 1,
        RATIO_MALE => 0,
        r => ((pid & 0xFF) < r as u32) as u8,
    }
}

/// Hidden Power type index (0 = Fighting … 15 = Dark) from IVs in stored order.
pub fn hidden_power_type(ivs: &Stats) -> u8 {
    let bits = ivs
        .iter()
        .enumerate()
        .fold(0u32, |acc, (i, iv)| acc | ((*iv as u32 & 1) << i));
    (bits * 15 / 63) as u8
}

pub fn hidden_power_power(ivs: &Stats) -> u8 {
    let bits = ivs
        .iter()
        .enumerate()
        .fold(0u32, |acc, (i, iv)| acc | (((*iv as u32 >> 1) & 1) << i));
    (30 + bits * 40 / 63) as u8
}

pub fn level_from_exp(exp: u32, growth: u8) -> u8 {
    let table = &GROWTH[growth as usize % 6];
    if exp >= table[99] {
        return 100;
    }
    let mut level = 1;
    while exp >= table[level] {
        level += 1;
    }
    level as u8
}

pub fn exp_for_level(level: u8, growth: u8) -> u32 {
    GROWTH[growth as usize % 6][level.clamp(1, 100) as usize - 1]
}

/// (boosted, hindered) stat index into [`Stats`], or `None` for neutral natures.
pub fn nature_amp(nature: u8) -> Option<(usize, usize)> {
    let (up, dn) = (nature as usize / 5, nature as usize % 5);
    (nature < 25 && up != dn).then_some((up + 1, dn + 1))
}

pub fn calc_stats(base: &Stats, ivs: &Stats, evs: &Stats, level: u8, nature: u8) -> Stats {
    let level = level as u32;
    let mut out: Stats = core::array::from_fn(|i| {
        let core = ivs[i] as u32 + 2 * base[i] as u32 + evs[i] as u32 / 4;
        if i == 0 {
            if base[0] == 1 {
                1
            } else {
                ((core + 100) * level / 100 + 10) as u16
            }
        } else {
            (core * level / 100 + 5) as u16
        }
    });
    if let Some((up, dn)) = nature_amp(nature) {
        out[up] = (out[up] as u32 * 11 / 10) as u16;
        out[dn] = (out[dn] as u32 * 9 / 10) as u16;
    }
    out
}

/// Unown letter (0..28) encoded in a Gen 3-style PID.
pub fn unown_form_from_pid(pid: u32) -> u8 {
    let v = ((pid >> 24) & 3) << 6 | ((pid >> 16) & 3) << 4 | ((pid >> 8) & 3) << 2 | (pid & 3);
    (v % 28) as u8
}

/// Wurmple evolves into Silcoon (0) or Cascoon (1).
pub fn wurmple_evolution(pid: u32) -> u8 {
    (((pid >> 16) % 10) >= 5) as u8
}

impl Pkm {
    pub fn is_shiny(&self) -> bool {
        is_shiny(self.pid(), self.tid(), self.sid())
    }

    pub fn hidden_power_type(&self) -> u8 {
        hidden_power_type(&self.ivs())
    }

    pub fn hidden_power_power(&self) -> u8 {
        hidden_power_power(&self.ivs())
    }

    pub fn level(&self, info: &BaseInfo) -> u8 {
        level_from_exp(self.exp(), info.growth)
    }

    pub fn calc_stats(&self, info: &BaseInfo) -> Stats {
        calc_stats(
            &info.base,
            &self.ivs(),
            &self.evs(),
            self.level(info),
            self.nature(),
        )
    }
}
