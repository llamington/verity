//! `memlink::link`, the "Memory Link and keys" screen's entry points (DS and web), on real saves.
//! The link itself is checked against the games in `p10_memlink` / `p10_keys` and the emulator
//! runs; these tests cover the glue. Skips without the corpus saves.

use std::fs;
use std::path::PathBuf;

use verity::memlink::link::{self, Mode, Status};
use verity::memlink::{MemLinkError, build, preview};
use verity::save::Save;

fn real(rel: &str) -> Option<Vec<u8>> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../corpus/real");
    fs::read(root.join(rel)).ok()
}

fn saves() -> Option<(Vec<u8>, Vec<u8>)> {
    Some((
        real("pkhex-avalonia/gen5_white2.sav")?,
        real("pkhex-avalonia/gen5_black.sav")?,
    ))
}

/// An unsaved edit: a bag count changed without recomputing the checksums.
fn edit(save: &mut [u8]) {
    let mut s = Save::open(save).unwrap();
    let (item, count) = s.bag_item(0, 0).unwrap();
    s.set_bag_item(0, 0, item, count ^ 1).unwrap();
    assert!(!s.checksums_valid());
}

#[test]
fn memory_link_plans_then_links() {
    let Some((mut w2, black)) = saves() else {
        return;
    };
    let p = link::plan(&mut w2, &black);
    assert_eq!(p.mode, Mode::MemoryLink);
    assert!(p.status.ready(), "{:?}", p.status);
    assert_eq!(link::link(&mut w2, &black, 0x1234_5678), Ok(()));
    // Linked again from the same save: nothing new to unlock.
    let again = link::plan(&mut w2, &black);
    assert_eq!(again.status, Status::Ready);
    assert!(!again.flashback);
}

/// Unsaved edits leave the checksums stale; the link code alone would call the save corrupt.
#[test]
fn unsaved_edits_dont_block_linking() {
    let Some((mut w2, black)) = saves() else {
        return;
    };
    edit(&mut w2);
    let data = build(&black).unwrap();
    assert_eq!(
        preview(&w2, &data, false).err(),
        Some(MemLinkError::NotBlack2White2)
    );
    assert!(link::plan(&mut w2, &black).status.ready());
    assert_eq!(link::link(&mut w2, &black, 7), Ok(()));
    edit(&mut w2);
    assert_eq!(link::enable_plan(&mut w2).mode, Mode::EnableAll);
    assert_ne!(link::enable_plan(&mut w2).status, Status::KeysCorrupt);
}

#[test]
fn enable_all_once() {
    let Some((mut w2, _)) = saves() else { return };
    let p = link::enable_plan(&mut w2);
    assert_eq!(p.mode, Mode::EnableAll);
    if p.status == Status::Ready {
        assert!(p.keys.contains(&true));
        assert_eq!(link::enable_all(&mut w2), Ok(()));
    }
    assert_eq!(link::enable_plan(&mut w2).status, Status::NoNewKeys);
    assert_eq!(link::enable_all(&mut w2), Err(Status::NoNewKeys));
}

#[test]
fn other_files_are_refused() {
    let Some((mut w2, _)) = saves() else { return };
    let Some(pt) = real("pkhex-avalonia/gen4_platinum.sav") else {
        return;
    };
    for other in [pt, vec![0; 16], vec![0xFF; 0x80000]] {
        let p = link::plan(&mut w2, &other);
        assert_eq!((p.mode, p.status), (Mode::Unusable, Status::WrongFile));
        assert_eq!(link::link(&mut w2, &other, 1), Err(Status::WrongFile));
    }
    // A Black 2 or White 2 save from the same game sends no new keys.
    let copy = w2.clone();
    let p = link::plan(&mut w2, &copy);
    assert_eq!((p.mode, p.status), (Mode::Keys, Status::NoNewKeys));
}

#[test]
fn status_codes_and_texts() {
    for code in 0..=5 {
        let s = Status::from_code(code);
        assert_eq!(s as u8, code);
        assert!(s.text().is_ascii() && s.text().len() <= 36);
        assert!(s.c_text().ends_with('\0'));
    }
    assert_eq!(Status::from_code(99), Status::WrongFile);
    assert!(Status::ReplacesOther.ready() && !Status::NoNewKeys.ready());
}
