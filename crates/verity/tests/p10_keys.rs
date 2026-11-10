//! P10 Key System: reading keys from real saves and the game's exchange rules.
use std::fs;
use std::path::PathBuf;

use verity::memlink::keys::{enable_all, read, receivable, receive, KeyState, KEY_CITY, KEY_COUNT};
use verity::memlink::MemLinkError;
use verity::save::Save;

const KEY_INFO: usize = 0x25800 + 0x28;
const VERSION: usize = 0x19400 + 0x1F;
const BLACK2: u8 = 23;
const WHITE2: u8 = 22;

fn real(rel: &str) -> Option<Vec<u8>> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../corpus/real");
    fs::read(root.join(rel)).ok()
}

fn state(unlocked: [bool; KEY_COUNT], enabled: [bool; KEY_COUNT], version: u8) -> KeyState {
    KeyState {
        unlocked,
        enabled,
        version,
    }
}

/// The save with its key info zeroed apart from the seed, as another version, with checksums fixed.
fn blank(save: &[u8], version: u8) -> Vec<u8> {
    let mut out = save.to_vec();
    out[KEY_INFO..KEY_INFO + 0x28].fill(0);
    out[VERSION] = version;
    Save::open(&mut out).unwrap().finalize();
    out
}

/// Marks every key as earned, using the unlocked magic numbers the game stores.
fn all_unlocked(save: &[u8], version: u8) -> Vec<u8> {
    const MAGIC: [u32; KEY_COUNT] = [0x35691, 0x18256, 0x59389, 0x48292, 0x09892];
    let mut out = blank(save, version);
    let seed = u32::from_le_bytes(out[KEY_INFO + 0x34..KEY_INFO + 0x38].try_into().unwrap());
    for (i, m) in MAGIC.iter().enumerate() {
        out[KEY_INFO + i * 4..KEY_INFO + i * 4 + 4].copy_from_slice(&(m ^ seed).to_le_bytes());
    }
    Save::open(&mut out).unwrap().finalize();
    out
}

#[test]
fn p10_keys_read_real_saves() {
    let Some(save) = real("pkhex-avalonia/gen5_white2.sav") else {
        return;
    };
    let keys = read(&save).unwrap();
    assert!(keys.unlocked[0] && keys.enabled[0], "{keys:?}");
    assert_eq!(keys.version, WHITE2);
}

#[test]
fn p10_keys_city_needs_the_other_version() {
    let partner = state([true; 5], [false; 5], BLACK2);
    let same = state([false; 5], [false; 5], BLACK2);
    let other = state([false; 5], [false; 5], WHITE2);
    let expected = core::array::from_fn(|i| i != KEY_CITY);
    assert_eq!(receivable(&same, &partner), expected);
    assert_eq!(receivable(&other, &partner), [true; 5]);
}

#[test]
fn p10_keys_only_unlocked_keys_pass_and_enabled_ones_are_skipped() {
    let partner = state(
        [true, false, false, false, true],
        [false, true, true, true, false],
        BLACK2,
    );
    let target = state([false; 5], [true, false, false, false, false], WHITE2);
    assert_eq!(
        receivable(&target, &partner),
        [false, false, false, false, true]
    );
}

#[test]
fn p10_keys_receive_enables_without_unlocking() {
    let Some(save) = real("pkhex-avalonia/gen5_white2.sav") else {
        return;
    };
    let mut target = blank(&save, WHITE2);
    let partner = all_unlocked(&save, BLACK2);
    let before = partner.clone();
    assert_eq!(receive(&mut target, &partner).unwrap(), [true; 5]);
    let keys = read(&target).unwrap();
    assert_eq!((keys.enabled, keys.unlocked), ([true; 5], [false; 5]));
    assert_eq!(partner, before);
    let once = target.clone();
    assert_eq!(receive(&mut target, &partner).unwrap(), [false; 5]);
    assert_eq!(target, once, "a second exchange writes nothing");
}

#[test]
fn p10_keys_received_keys_cannot_be_passed_on() {
    let Some(save) = real("pkhex-avalonia/gen5_white2.sav") else {
        return;
    };
    let mut middle = blank(&save, WHITE2);
    receive(&mut middle, &all_unlocked(&save, BLACK2)).unwrap();
    let mut last = blank(&save, BLACK2);
    assert_eq!(receive(&mut last, &middle).unwrap(), [false; 5]);
}

#[test]
fn p10_keys_refuse_other_games_and_corrupt_saves() {
    let (Some(b2w2), Some(bw)) = (
        real("pkhex-avalonia/gen5_white2.sav"),
        real("pkhex-avalonia/gen5_black.sav"),
    ) else {
        return;
    };
    let mut target = b2w2.clone();
    assert_eq!(
        receive(&mut target, &bw),
        Err(MemLinkError::NotBlack2White2)
    );
    let mut corrupt = b2w2.clone();
    corrupt[KEY_INFO] ^= 1;
    assert_eq!(read(&corrupt), Err(MemLinkError::NotBlack2White2));
    assert_eq!(target, b2w2);
}

#[test]
fn p10_keys_enable_all_needs_no_partner_and_marks_nothing_earned() {
    let Some(save) = real("pkhex-avalonia/gen5_white2.sav") else {
        return;
    };
    let mut target = blank(&save, WHITE2);
    assert_eq!(enable_all(&mut target).unwrap(), [true; 5]);
    let keys = read(&target).unwrap();
    assert_eq!((keys.enabled, keys.unlocked), ([true; 5], [false; 5]));
    let once = target.clone();
    assert_eq!(enable_all(&mut target).unwrap(), [false; 5]);
    assert_eq!(target, once, "a second run writes nothing");
    let mut next = blank(&save, BLACK2);
    assert_eq!(receive(&mut next, &target).unwrap(), [false; 5]);
}

#[test]
fn p10_keys_enable_all_only_adds_what_is_missing() {
    let Some(save) = real("pkhex-avalonia/gen5_white2.sav") else {
        return;
    };
    let mut target = save.clone();
    assert_eq!(
        enable_all(&mut target).unwrap(),
        [false, true, true, true, true]
    );
    assert!(
        read(&target).unwrap().unlocked[0],
        "earned keys stay earned"
    );
}

#[test]
fn p10_keys_enable_all_refuses_a_corrupt_save() {
    let Some(save) = real("pkhex-avalonia/gen5_white2.sav") else {
        return;
    };
    let mut corrupt = save.clone();
    corrupt[KEY_INFO] ^= 1;
    let before = corrupt.clone();
    assert_eq!(enable_all(&mut corrupt), Err(MemLinkError::NotBlack2White2));
    assert_eq!(corrupt, before);
}
