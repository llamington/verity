//! Contest stat checks for Gen 3 and Gen 4 origins in the Gen 4 format. Ported from PKHeX
//! `Verifiers/ContestStatVerifier.cs` and `Verifiers/Misc/ContestStatInfo.cs`.
//!
//! Pokéblocks and Poffins raise the five stats and Sheen together, so Sheen limits what the
//! stats can be. Too little Sheen for the stats gained, or too much, can't come from feeding.

const COOL: usize = 0;
const BEAUTY: usize = 1;
const CUTE: usize = 2;
const SMART: usize = 3;
const TOUGH: usize = 4;
const SHEEN: usize = 5;

const MAX: i32 = 255;
/// All-Nutpea Pokéblocks can max every stat at this Sheen.
const BEST_SHEEN_3: i32 = 214;
const LOWEST_FEEL_BLOCK_3: i32 = 1;
const LOWEST_FEEL_POFFIN_4: i32 = 11;
const HIGHEST_FEEL_POFFIN: i32 = 79;

/// Contest condition raised by the flavour each stat likes, in nature index order (Atk, Def,
/// Spe, SpA, SpD: spicy, sour, sweet, dry, bitter).
const AMP_ORDER: [usize; 5] = [COOL, TOUGH, CUTE, BEAUTY, SMART];

fn amps(nature: u8) -> [i32; 5] {
    let nature = if nature >= 25 { 0 } else { nature as usize };
    let (up, down) = (nature / 5, nature % 5);
    if up == down {
        return [0; 5];
    }
    core::array::from_fn(|i| {
        if i == up {
            1
        } else if i == down {
            -1
        } else {
            0
        }
    })
}

/// `GetStatAdjustment`: undoes the 10% a liked or disliked flavour applied.
fn adjustment(gain: i32, amp: i32) -> i32 {
    let factor = if amp == 1 { 11 } else { 9 };
    let mut boost = gain / factor;
    if gain % factor >= 5 {
        boost += 1;
    }
    if amp == -1 { boost } else { -boost }
}

fn gained_sum(s: &[u8; 6], nature: u8, initial: &[u8; 6]) -> i32 {
    let table = amps(nature);
    let mut sum = 0;
    for (amp, stat) in table.into_iter().zip(AMP_ORDER) {
        let gain = s[stat] as i32 - initial[stat] as i32;
        if gain > 0 {
            sum += gain + if amp == 0 { 0 } else { adjustment(gain, amp) };
        }
    }
    sum
}

fn average_feel(s: &[u8; 6], nature: u8, initial: &[u8; 6]) -> i32 {
    (gained_sum(s, nature, initial) + 4).div_euclid(5)
}

fn minimum_sheen(s: &[u8; 6], nature: u8, initial: &[u8; 6], gen3: bool) -> i32 {
    if s == initial {
        return initial[SHEEN] as i32;
    }
    let raw = average_feel(s, 0, initial);
    if raw == MAX {
        return if gen3 { BEST_SHEEN_3 } else { MAX };
    }
    let flavoured = if nature.is_multiple_of(6) {
        raw
    } else {
        average_feel(s, nature, initial)
    };
    let avg = flavoured.max(1).min(raw);
    if gen3 {
        (BEST_SHEEN_3 * avg / MAX).clamp(LOWEST_FEEL_BLOCK_3, BEST_SHEEN_3)
    } else {
        avg.clamp(LOWEST_FEEL_POFFIN_4, MAX)
    }
}

fn maximum_sheen(s: &[u8; 6], nature: u8, initial: &[u8; 6], gen3: bool) -> i32 {
    if s[..SHEEN].contains(&255) {
        return MAX;
    }
    if s == initial {
        return initial[SHEEN] as i32;
    }
    if gen3 {
        let sum = gained_sum(s, nature, initial);
        if sum == 0 {
            return 0;
        }
        let gained = [COOL, BEAUTY, CUTE, SMART, TOUGH]
            .iter()
            .filter(|&&i| s[i] as i32 - initial[i] as i32 >= 2)
            .count();
        // Black Pokéblocks if at least three stats rose by 2 or more, else the best regular one.
        let permit = if gained >= 3 {
            sum * 83 / 6
        } else {
            sum * 19 / 9
        };
        return permit.clamp(LOWEST_FEEL_BLOCK_3, MAX);
    }
    let avg = average_feel(s, nature, initial);
    match avg {
        i32::MIN..=0 => initial[SHEEN] as i32,
        1 | 2 => 59,
        _ => MAX.min(avg * HIGHEST_FEEL_POFFIN),
    }
}

/// True when the stats and Sheen could have come from feeding. `s` and `initial` are Cool,
/// Beauty, Cute, Smart, Tough, Sheen; `initial` is what the encounter started with.
pub fn sheen_valid(s: &[u8; 6], nature: u8, initial: &[u8; 6], gen3: bool) -> bool {
    if s.iter().all(|&v| v == 0) {
        return true;
    }
    let sheen = s[SHEEN] as i32;
    sheen >= minimum_sheen(s, nature, initial, gen3)
        && sheen <= maximum_sheen(s, nature, initial, gen3)
}

#[cfg(test)]
mod tests {
    use super::sheen_valid;

    const NONE: [u8; 6] = [0; 6];

    #[test]
    fn untouched_is_valid() {
        assert!(sheen_valid(&NONE, 0, &NONE, false));
    }

    #[test]
    fn sheen_needs_stats_and_stats_need_sheen() {
        assert!(!sheen_valid(&[0, 0, 0, 0, 0, 50], 0, &NONE, false));
        assert!(!sheen_valid(&[10, 10, 10, 10, 10, 0], 0, &NONE, false));
        assert!(!sheen_valid(&[10, 10, 10, 10, 10, 0], 0, &NONE, true));
    }

    #[test]
    fn gen4_poffin_limits() {
        // Average gain 10: Sheen must be at least the lowest Poffin (11), at most 10 * 79.
        assert!(!sheen_valid(&[10, 10, 10, 10, 10, 10], 0, &NONE, false));
        assert!(sheen_valid(&[10, 10, 10, 10, 10, 11], 0, &NONE, false));
        assert!(sheen_valid(&[10, 10, 10, 10, 10, 255], 0, &NONE, false));
        // One or two points on average: burnt Poffins top out at 59.
        assert!(sheen_valid(&[5, 0, 0, 0, 0, 59], 0, &NONE, false));
        assert!(!sheen_valid(&[5, 0, 0, 0, 0, 60], 0, &NONE, false));
    }

    #[test]
    fn maxed_stat_lifts_the_cap() {
        assert!(sheen_valid(&[255, 0, 0, 0, 0, 255], 0, &NONE, false));
    }

    #[test]
    fn a_trades_starting_stats_need_no_sheen() {
        let start = [30, 5, 5, 5, 5, 10];
        assert!(sheen_valid(&start, 0, &start, true));
    }
}
