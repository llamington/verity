//! P10: `memlink` against real saves, and byte for byte against genuine links recorded by
//! `tools/emu-harness/memlink_accept.py`. Tests skip when those files are absent.

use std::fs;
use std::path::{Path, PathBuf};

use verity::memlink::{Applied, MemLinkError, Seeds, apply, build};
use verity::save::crc16_ccitt;

const GIFTS: usize = 0x7E000;
const BACKUP: usize = 0x7E400;
const KEYS: usize = 0x7E800;
const GIFTS_BLOCK: usize = 0x374;
const KEYS_BLOCK: usize = 0x220;
const SEEDS: Seeds = Seeds {
    gifts: 0x7D,
    key_info: 0x909A_F8E0,
};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn real(rel: &str) -> Option<Vec<u8>> {
    fs::read(root().join("corpus/real").join(rel)).ok()
}

fn saves() -> Option<(Vec<u8>, Vec<u8>, Vec<u8>)> {
    Some((
        real("pkhex-avalonia/gen5_black.sav")?,
        real("pkmds-blazor/Black - Full Completion.sav")?,
        real("pkhex-avalonia/gen5_white2.sav")?,
    ))
}

fn crc_ok(block: &[u8]) -> bool {
    u16::from_le_bytes([block[8], block[9]]) == crc16_ccitt(&block[0xC..])
}

#[test]
fn p10_apply_writes_only_the_region_with_valid_blocks() {
    let Some((black, _, b2)) = saves() else {
        return;
    };
    let mut out = b2.clone();
    apply(&mut out, &build(&black).unwrap(), SEEDS, false).unwrap();
    let region = GIFTS..KEYS + KEYS_BLOCK;
    assert!((0..b2.len()).all(|i| region.contains(&i) || b2[i] == out[i]));
    assert!(crc_ok(&out[GIFTS..GIFTS + GIFTS_BLOCK]));
    assert!(crc_ok(&out[KEYS..KEYS + KEYS_BLOCK]));
    assert_eq!(
        out[GIFTS..GIFTS + GIFTS_BLOCK],
        out[BACKUP..BACKUP + GIFTS_BLOCK]
    );
}

#[test]
fn p10_another_players_data_needs_replace() {
    let Some((black, full, b2)) = saves() else {
        return;
    };
    let mut out = b2;
    apply(&mut out, &build(&black).unwrap(), SEEDS, false).unwrap();
    let before = out.clone();
    let lynx = build(&full).unwrap();
    assert_eq!(
        apply(&mut out, &lynx, SEEDS, false),
        Err(MemLinkError::DifferentPlayer)
    );
    assert_eq!(out, before);
    apply(&mut out, &lynx, SEEDS, true).unwrap();
    assert_ne!(out, before);
}

#[test]
fn p10_counts_advance_and_existing_data_is_kept_on_relink() {
    let Some((black, _, b2)) = saves() else {
        return;
    };
    let data = build(&black).unwrap();
    let mut out = b2;
    apply(&mut out, &data, SEEDS, false).unwrap();
    let first = out.clone();
    let result: Applied = apply(
        &mut out,
        &data,
        Seeds {
            gifts: 1,
            key_info: 2,
        },
        false,
    )
    .unwrap();
    assert!(!result.unlocked_flashback);
    // The gift count rises by one, and later seeds aren't used on existing blocks.
    assert_eq!(out[GIFTS + 4], first[GIFTS + 4] + 1);
    assert_eq!(
        out[GIFTS + 0xC..GIFTS + GIFTS_BLOCK],
        first[GIFTS + 0xC..GIFTS + GIFTS_BLOCK]
    );
    assert_eq!(
        out[KEYS + 0xC..KEYS + KEYS_BLOCK],
        first[KEYS + 0xC..KEYS + KEYS_BLOCK]
    );
}

#[test]
fn p10_rejects_bad_inputs_without_writing() {
    let Some((black, _, b2)) = saves() else {
        return;
    };
    let data = build(&black).unwrap();
    assert_eq!(build(&b2), Err(MemLinkError::NotBlackWhite));
    let mut wrong = black.clone();
    assert_eq!(
        apply(&mut wrong, &data, SEEDS, false),
        Err(MemLinkError::NotBlack2White2)
    );
    assert_eq!(wrong, black);

    let mut linked = b2;
    apply(&mut linked, &data, SEEDS, false).unwrap();
    for (offset, expected) in [
        (KEYS + 0x100, MemLinkError::KeysCorrupt),
        (GIFTS + 0x100, MemLinkError::GiftsCorrupt),
    ] {
        let mut broken = linked.clone();
        broken[offset] ^= 1;
        if offset == GIFTS + 0x100 {
            broken[BACKUP + 0x100] ^= 1;
        }
        let snapshot = broken.clone();
        assert_eq!(apply(&mut broken, &data, SEEDS, false), Err(expected));
        assert_eq!(broken, snapshot);
    }
}

#[test]
fn p10_matches_genuine_links() {
    let Ok(cases) = fs::read_dir(root().join("build/emu/memlink-accept")) else {
        return;
    };
    let (mut compared, mut failures) = (0, Vec::new());
    for case in cases.flatten() {
        let dir = case.path();
        let Ok(genuine) = fs::read(dir.join("inst0/b2-after.sav")) else {
            continue;
        };
        let name = case.file_name().into_string().unwrap();
        let bw = fs::read(dir.join("bw.sav")).unwrap();
        let mut ours = fs::read(dir.join("b2-before.sav")).unwrap();
        let seeds = Seeds {
            gifts: genuine[GIFTS + GIFTS_BLOCK - 1],
            key_info: u32::from_le_bytes(genuine[KEYS + 0x40..KEYS + 0x44].try_into().unwrap()),
        };
        apply(&mut ours, &build(&bw).unwrap(), seeds, false).unwrap();
        compared += 1;
        if ours != genuine {
            let first = (0..ours.len()).find(|&i| ours[i] != genuine[i]).unwrap();
            let count = (0..ours.len()).filter(|&i| ours[i] != genuine[i]).count();
            failures.push(format!("{name}: {count} bytes differ, first at {first:#x}"));
        }
    }
    assert!(failures.is_empty(), "{compared} compared: {failures:#?}");
    println!("{compared} genuine links match");
}
