//! B2W2 Key System: receives keys from another B2W2 save, under the rules of the game's Unova Link
//! exchange. Findings: `docs/memlink-findings.md`.
//!
//! A key is *unlocked* when that save earned it and *enabled* when its setting can be used.
//! Receiving only enables: a received key can't be passed on, and the sender's save doesn't change.
//! The city key only passes between Black 2 and White 2.

use super::{MemLinkError, is_valid};
use crate::save::gen5::Layout5;
use crate::save::{SaveKind, read_u32};

pub const KEY_COUNT: usize = 5;

/// Index of the key that swaps Black City and White Forest.
pub const KEY_CITY: usize = 2;

/// Key info in the main save, which the game reads whenever a save exists.
const KEY_INFO: usize = 0x25800 + 0x28;
const ENABLED: usize = KEY_INFO + 0x14;
const SEED: usize = KEY_INFO + 0x34;
const VERSION: usize = 0x19400 + 0x1F;

/// Each value is stored exclusive-ored with the seed. Order: Easy, Challenge, City, Iron, Iceberg.
const UNLOCKED_MAGIC: [u32; KEY_COUNT] = [0x35691, 0x18256, 0x59389, 0x48292, 0x09892];
const ENABLED_MAGIC: [u32; KEY_COUNT] = [0x93389, 0x22843, 0x34771, 0xAB031, 0xB3818];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyState {
    pub unlocked: [bool; KEY_COUNT],
    pub enabled: [bool; KEY_COUNT],
    pub version: u8,
}

/// Reads the keys of a valid B2W2 save.
pub fn read(save: &[u8]) -> Result<KeyState, MemLinkError> {
    if !is_valid(save, SaveKind::B2W2) {
        return Err(MemLinkError::NotBlack2White2);
    }
    let seed = read_u32(save, SEED);
    let held = |base: usize, magic: &[u32; KEY_COUNT]| {
        core::array::from_fn(|i| read_u32(save, base + i * 4) ^ seed == magic[i])
    };
    Ok(KeyState {
        unlocked: held(KEY_INFO, &UNLOCKED_MAGIC),
        enabled: held(ENABLED, &ENABLED_MAGIC),
        version: save[VERSION],
    })
}

/// The keys `target` would newly enable by receiving from `partner`.
pub fn receivable(target: &KeyState, partner: &KeyState) -> [bool; KEY_COUNT] {
    core::array::from_fn(|i| {
        !target.enabled[i]
            && partner.unlocked[i]
            && (i != KEY_CITY || target.version != partner.version)
    })
}

fn write_enabled(target: &mut [u8], new: [bool; KEY_COUNT]) {
    if !new.contains(&true) {
        return;
    }
    let seed = read_u32(target, SEED);
    for (i, _) in new.iter().enumerate().filter(|(_, new)| **new) {
        let ofs = ENABLED + i * 4;
        target[ofs..ofs + 4].copy_from_slice(&(ENABLED_MAGIC[i] ^ seed).to_le_bytes());
    }
    Layout5::new(SaveKind::B2W2).set_checksums(target);
}

/// Enables on `target` every key it can receive from `partner`, and returns which. Nothing is
/// written when there are none.
pub fn receive(target: &mut [u8], partner: &[u8]) -> Result<[bool; KEY_COUNT], MemLinkError> {
    let new = receivable(&read(target)?, &read(partner)?);
    write_enabled(target, new);
    Ok(new)
}

/// The keys `enable_all` would newly enable.
pub fn missing(keys: &KeyState) -> [bool; KEY_COUNT] {
    keys.enabled.map(|enabled| !enabled)
}

/// Enables every key without a partner save, and returns which were new. This goes beyond the
/// game's exchange, but leaves the state an exchange would: nothing is marked as earned, so the
/// keys can't be sent on.
pub fn enable_all(target: &mut [u8]) -> Result<[bool; KEY_COUNT], MemLinkError> {
    let new = missing(&read(target)?);
    write_enabled(target, new);
    Ok(new)
}
