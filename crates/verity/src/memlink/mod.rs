//! B2W2 Memory Link: reads what a Black or White save contributes and merges it into a B2W2 save,
//! as the game does over DS Download Play. Findings and evidence: `docs/memlink-findings.md`.
//!
//! The region sits outside the checksummed save blocks, so writing it changes nothing else.

pub mod keys;
pub mod link;

use crate::save::gen5::Layout5;
use crate::save::{SaveKind, crc16_ccitt, detect, read_u16, read_u32, write_u16};

const RAW_SAVE_LEN: usize = 0x80000;

// Black/White save blocks read by the link.
const BW_PLAYER: usize = 0x19404;
const BW_EVENT_WORK: usize = 0x20100;
const BW_EVENT_FLAGS: usize = BW_EVENT_WORK + 0x27C;
const BW_UNLOCK_WORD: usize = 0x21204;
const BW_HALL_OF_FAME: usize = 0x23B00;

// The musical block has the same layout and offset in BW and B2W2.
const MUSICAL: usize = 0x1F700;
const MUSICAL_SHOW_COUNT: usize = MUSICAL + 0x252;
const MUSICAL_TOTAL_POINTS: usize = MUSICAL + 0x256;
const MUSICAL_PROPS: usize = MUSICAL + 0x258;
const MUSICAL_DOWNLOADED_PROPS: usize = MUSICAL + 0x2A0;
const MUSICAL_PROP_COUNT: usize = 100;

const GIFTS: usize = 0x7E000;
const GIFTS_BACKUP: usize = 0x7E400;
const KEYS: usize = 0x7E800;

const MAGIC: u32 = 0xA10F_49AE;
const HEADER: usize = 0xC;
const GIFTS_SIZE: usize = 0x368;
const KEYS_SIZE: usize = 0x214;
const GIFTS_BLOCK: usize = HEADER + GIFTS_SIZE;
const KEYS_BLOCK: usize = HEADER + KEYS_SIZE;

// Offsets within the keys block.
const KEY_INFO_SEED: usize = HEADER + 0x34;
const PLAYERS: usize = HEADER + 0x38;
const PLAYER_SIZE: usize = 0x18;
const PLAYER_COUNT: usize = 3;
const FLAGS: usize = PLAYERS + PLAYER_SIZE * PLAYER_COUNT;
const VARS: usize = FLAGS + 2;
const PROPS: usize = FLAGS + 8;
const TOTAL_POINTS: usize = FLAGS + 0x28;
const SHOW_COUNT: usize = FLAGS + 0x2A;
const HALL_OF_FAME: usize = FLAGS + 0x2C;

const FLAG_BITS: usize = 16;
const PROPS_SIZE: usize = 0x20;
const HALL_OF_FAME_SIZE: usize = 0x168;

const PART_EVENTS: usize = 0;
const PART_MUSICAL: usize = 1;
const PART_HALL_OF_FAME: usize = 2;

/// (event flag, bit set when the flag has this value, value).
const EVENT_FLAG_BITS: [(usize, u8, bool); 8] = [
    (2400, 0, true),
    (2401, 1, true),
    (2427, 2, true),
    (681, 3, false),
    (682, 4, false),
    (683, 5, false),
    (684, 6, false),
    (685, 7, false),
];

/// The event work variables copied, by index in the save's work array.
const EVENT_VARS: [usize; 3] = [0x30, 0x8E, 0xBC];

/// A flashback's unlock condition.
enum Unlock {
    Flag(usize),
    PlayerLinked(usize),
}

const FLASHBACKS: [Unlock; 8] = [
    Unlock::Flag(8),
    Unlock::Flag(13),
    Unlock::Flag(0),
    Unlock::Flag(15),
    Unlock::Flag(12),
    Unlock::Flag(0),
    Unlock::PlayerLinked(PART_MUSICAL),
    Unlock::Flag(0),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MemLinkError {
    /// The source is not a valid Black or White save.
    NotBlackWhite,
    /// The target is not a valid Black 2 or White 2 save.
    NotBlack2White2,
    /// Stored outside-save gifts fail their CRC. The game would clear them.
    GiftsCorrupt,
    /// Stored keys and Memory Link data fail their CRC. The game would clear them.
    KeysCorrupt,
    /// The save already holds data from a different player. The game asks before replacing it.
    DifferentPlayer,
}

/// What a Black or White save gives the Memory Link.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MemoryLinkData {
    id: u32,
    name: [u8; 16],
    gender: u8,
    version: u8,
    flags: [u8; 2],
    vars: [u16; 3],
    props: [u8; PROPS_SIZE],
    total_points: u16,
    show_count: u16,
    hall_of_fame: [u8; HALL_OF_FAME_SIZE],
}

/// The values the game draws at random when it first creates these blocks.
#[derive(Clone, Copy, Debug)]
pub struct Seeds {
    /// Repeated into the top half of the gift block's seed. The game uses its frame counter.
    pub gifts: u8,
    pub key_info: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Applied {
    /// Whether this link unlocked a flashback that wasn't unlocked before.
    pub unlocked_flashback: bool,
}

fn is_valid(save: &[u8], kind: SaveKind) -> bool {
    save.len() >= RAW_SAVE_LEN
        && detect(&save[..RAW_SAVE_LEN]) == Some(kind)
        && Layout5::new(kind).checksums_valid(save)
}

/// Reads the Memory Link's data from a Black or White save (`WBSaveConvert` in the game).
pub fn build(bw: &[u8]) -> Result<MemoryLinkData, MemLinkError> {
    if !is_valid(bw, SaveKind::BW) {
        return Err(MemLinkError::NotBlackWhite);
    }
    let mut flags = [0u8; 2];
    for (flag, bit, value) in EVENT_FLAG_BITS {
        let set = bw[BW_EVENT_FLAGS + flag / 8] & (1 << (flag % 8)) != 0;
        if set == value {
            flags[0] |= 1 << bit;
        }
    }
    flags[1] = bw[BW_UNLOCK_WORD];
    let mut vars = [0u16; 3];
    for (var, index) in vars.iter_mut().zip(EVENT_VARS) {
        *var = read_u16(bw, BW_EVENT_WORK + index * 2);
    }
    let mut name = [0u8; 16];
    name.copy_from_slice(&bw[BW_PLAYER..BW_PLAYER + 16]);
    let mut props = [0u8; PROPS_SIZE];
    props.copy_from_slice(&bw[MUSICAL_PROPS..MUSICAL_PROPS + PROPS_SIZE]);
    let mut hall_of_fame = [0u8; HALL_OF_FAME_SIZE];
    hall_of_fame.copy_from_slice(&bw[BW_HALL_OF_FAME..BW_HALL_OF_FAME + HALL_OF_FAME_SIZE]);
    Ok(MemoryLinkData {
        id: read_u32(bw, BW_PLAYER + 0x10),
        name,
        gender: bw[BW_PLAYER + 0x1D],
        version: bw[BW_PLAYER + 0x1B],
        flags,
        vars,
        props,
        total_points: read_u16(bw, MUSICAL_TOTAL_POINTS),
        show_count: read_u16(bw, MUSICAL_SHOW_COUNT),
        hall_of_fame,
    })
}

fn write_u32(data: &mut [u8], ofs: usize, value: u32) {
    data[ofs..ofs + 4].copy_from_slice(&value.to_le_bytes());
}

fn has_magic(block: &[u8]) -> bool {
    read_u32(block, 0) == MAGIC
}

fn crc_ok(block: &[u8]) -> bool {
    read_u16(block, 8) == crc16_ccitt(&block[HEADER..])
}

/// The block both gift copies get: the newer valid copy, or a new empty one.
fn next_gifts(save: &[u8], seed: u8) -> Result<[u8; GIFTS_BLOCK], MemLinkError> {
    let main = &save[GIFTS..GIFTS + GIFTS_BLOCK];
    let backup = &save[GIFTS_BACKUP..GIFTS_BACKUP + GIFTS_BLOCK];
    let main_ok = has_magic(main) && crc_ok(main);
    let backup_ok = has_magic(backup) && crc_ok(backup);
    if (has_magic(main) || has_magic(backup)) && !main_ok && !backup_ok {
        return Err(MemLinkError::GiftsCorrupt);
    }
    // A copy that has the magic but fails its CRC loses to the valid one; so does a missing copy.
    let mut out = [0u8; GIFTS_BLOCK];
    let latest = match (main_ok, backup_ok) {
        (true, true) => {
            let (count, backup_count) = (read_u32(main, 4), read_u32(backup, 4));
            let main_newer = count >= backup_count || (count == 0 && backup_count == 0xFFFF_FFFF);
            Some(if main_newer { main } else { backup })
        }
        (true, false) => Some(main),
        (false, true) => Some(backup),
        (false, false) => None,
    };
    match latest {
        Some(block) => {
            out.copy_from_slice(block);
            write_u32(&mut out, 4, read_u32(block, 4).wrapping_add(1));
            write_u16(&mut out, 0xA, GIFTS_SIZE as u16);
        }
        None => {
            // Zeroed gifts, encrypted. Their checksum is 0, so it adds nothing to the seed.
            let tail = match u16::from_le_bytes([seed, seed]) {
                0 => 1,
                tail => tail,
            };
            write_u16(&mut out, GIFTS_BLOCK - 2, tail);
            let mut state = u32::from(tail) << 16;
            for chunk in out[HEADER..GIFTS_BLOCK - 4].as_chunks_mut::<2>().0 {
                state = state.wrapping_mul(0x41C6_4E6D).wrapping_add(0x6073);
                *chunk = ((state >> 16) as u16).to_le_bytes();
            }
            write_u32(&mut out, 4, 1);
        }
    }
    write_u32(&mut out, 0, MAGIC);
    let crc = crc16_ccitt(&out[HEADER..]);
    write_u16(&mut out, 8, crc);
    Ok(out)
}

fn flag_set(keys: &[u8], index: usize) -> bool {
    keys[FLAGS + index / 8] & (1 << (index % 8)) != 0
}

fn player_linked(keys: &[u8], part: usize) -> bool {
    keys[PLAYERS + part * PLAYER_SIZE + 0x16] != 0
}

/// One bit per unlocked flashback.
fn unlocked_flashbacks(keys: &[u8]) -> u32 {
    FLASHBACKS.iter().enumerate().fold(0, |mask, (i, unlock)| {
        let open = match *unlock {
            Unlock::Flag(index) => flag_set(keys, index),
            Unlock::PlayerLinked(part) => player_linked(keys, part),
        };
        mask | (u32::from(open) << i)
    })
}

fn has_all_props(save: &[u8]) -> bool {
    (0..MUSICAL_PROP_COUNT).all(|i| save[MUSICAL_PROPS + i / 8] & (1 << (i % 8)) != 0)
}

/// Merges `data` into `keys`, writing only if `apply` is set. Returns which parts changed.
fn merge(keys: &mut [u8], data: &MemoryLinkData, save: &[u8], apply: bool) -> [bool; PLAYER_COUNT] {
    let mut changed = [false; PLAYER_COUNT];
    for i in 0..FLAG_BITS {
        if data.flags[i / 8] & (1 << (i % 8)) != 0 {
            if !flag_set(keys, i) {
                changed[PART_EVENTS] = true;
            }
            if apply {
                keys[FLAGS + i / 8] |= 1 << (i % 8);
            }
        }
    }
    for (i, &value) in data.vars.iter().enumerate() {
        let ofs = VARS + i * 2;
        let current = read_u16(keys, ofs);
        if (i != 1 || value != 0) && value != current {
            if current == 0 {
                changed[PART_EVENTS] = true;
            }
            if apply {
                write_u16(keys, ofs, value);
            }
        }
    }
    if data.show_count != 0 && save[MUSICAL_DOWNLOADED_PROPS] == 0 {
        let differs = data.props[..] != save[MUSICAL_PROPS..MUSICAL_PROPS + PROPS_SIZE];
        if differs || has_all_props(save) {
            changed[PART_MUSICAL] = true;
            if apply {
                write_u16(keys, TOTAL_POINTS, data.total_points);
                write_u16(keys, SHOW_COUNT, data.show_count);
                keys[PROPS..PROPS + PROPS_SIZE].copy_from_slice(&data.props);
            }
        }
    }
    if data.flags[0] & 1 != 0
        && data.hall_of_fame[..] != keys[HALL_OF_FAME..HALL_OF_FAME + HALL_OF_FAME_SIZE]
    {
        if apply {
            keys[HALL_OF_FAME..HALL_OF_FAME + HALL_OF_FAME_SIZE]
                .copy_from_slice(&data.hall_of_fame);
        }
        changed[PART_HALL_OF_FAME] = true;
    }
    if apply {
        for (part, _) in changed.iter().enumerate().filter(|(_, c)| **c) {
            let slot = &mut keys[PLAYERS + part * PLAYER_SIZE..][..PLAYER_SIZE];
            slot.fill(0);
            write_u32(slot, 0, data.id);
            slot[4..20].copy_from_slice(&data.name);
            slot[0x14] = data.gender;
            slot[0x15] = data.version;
            slot[0x16] = 1;
        }
    }
    changed
}

/// Compares two names up to their terminator.
fn same_name(a: &[u8], b: &[u8]) -> bool {
    for (x, y) in a.as_chunks::<2>().0.iter().zip(b.as_chunks::<2>().0) {
        if x != y {
            return false;
        }
        if *x == [0xFF, 0xFF] {
            break;
        }
    }
    true
}

/// `MemoryLinkSave_CanMerge`: true when nothing is linked yet, or every part this link would
/// change already belongs to the same player.
fn can_merge(keys: &[u8], data: &MemoryLinkData, changed: [bool; PLAYER_COUNT]) -> bool {
    if (0..PLAYER_COUNT).all(|part| !player_linked(keys, part)) {
        return true;
    }
    changed
        .iter()
        .enumerate()
        .filter(|(_, c)| **c)
        .all(|(part, _)| {
            let slot = &keys[PLAYERS + part * PLAYER_SIZE..][..PLAYER_SIZE];
            read_u32(slot, 0) == data.id
                && same_name(&slot[4..20], &data.name)
                && slot[0x14] == data.gender
                && slot[0x15] == data.version
        })
}

/// Merges `data` into a Black 2 or White 2 save and rewrites the region as the game does. On an
/// error the save is untouched. `replace` answers yes to the game's prompt about replacing another
/// player's data.
pub fn apply(
    b2w2: &mut [u8],
    data: &MemoryLinkData,
    seeds: Seeds,
    replace: bool,
) -> Result<Applied, MemLinkError> {
    let (gifts, keys, applied) = plan(b2w2, data, seeds, replace)?;
    b2w2[GIFTS..GIFTS + GIFTS_BLOCK].copy_from_slice(&gifts);
    b2w2[GIFTS_BACKUP..GIFTS_BACKUP + GIFTS_BLOCK].copy_from_slice(&gifts);
    b2w2[KEYS..KEYS + KEYS_BLOCK].copy_from_slice(&keys);
    Ok(applied)
}

/// What `apply` would do, without writing.
pub fn preview(b2w2: &[u8], data: &MemoryLinkData, replace: bool) -> Result<Applied, MemLinkError> {
    let seeds = Seeds {
        gifts: 0,
        key_info: 0,
    };
    plan(b2w2, data, seeds, replace).map(|(_, _, applied)| applied)
}

fn plan(
    b2w2: &[u8],
    data: &MemoryLinkData,
    seeds: Seeds,
    replace: bool,
) -> Result<([u8; GIFTS_BLOCK], [u8; KEYS_BLOCK], Applied), MemLinkError> {
    if !is_valid(b2w2, SaveKind::B2W2) {
        return Err(MemLinkError::NotBlack2White2);
    }
    let gifts = next_gifts(b2w2, seeds.gifts)?;

    let mut keys = [0u8; KEYS_BLOCK];
    let stored = &b2w2[KEYS..KEYS + KEYS_BLOCK];
    let count = read_u32(stored, 4);
    if has_magic(stored) {
        if !crc_ok(stored) {
            return Err(MemLinkError::KeysCorrupt);
        }
        keys.copy_from_slice(stored);
    } else {
        write_u32(&mut keys, KEY_INFO_SEED, seeds.key_info);
    }

    let changed = merge(&mut keys, data, b2w2, false);
    if !replace && !can_merge(&keys, data, changed) {
        return Err(MemLinkError::DifferentPlayer);
    }
    let before = unlocked_flashbacks(&keys);
    merge(&mut keys, data, b2w2, true);
    let after = unlocked_flashbacks(&keys);

    write_u32(&mut keys, 0, MAGIC);
    write_u32(&mut keys, 4, count.wrapping_add(1));
    write_u16(&mut keys, 0xA, KEYS_SIZE as u16);
    let crc = crc16_ccitt(&keys[HEADER..]);
    write_u16(&mut keys, 8, crc);

    let applied = Applied {
        unlocked_flashback: after & (before ^ after) != 0,
    };
    Ok((gifts, keys, applied))
}
