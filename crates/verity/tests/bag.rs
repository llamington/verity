//! `verity::bag` (the bag screen's entry points, DS and web) on real saves, and the pouch item
//! lists against PKHeX's export. Skips without the data blobs (`data/blobs`) or corpus saves.

use std::fs;
use std::path::PathBuf;

use verity::bag::{self, Refusal};
use verity::data::GameData;
use verity::save::bag::PouchKind;
use verity::save::{Save, SaveKind};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn game_name(kind: SaveKind) -> &'static str {
    match kind {
        SaveKind::DP => "dp",
        SaveKind::Pt => "pt",
        SaveKind::HGSS => "hgss",
        SaveKind::BW => "bw",
        SaveKind::B2W2 => "b2w2",
    }
}

fn game(kind: SaveKind) -> Option<GameData<'static>> {
    let path = root()
        .join("data/blobs")
        .join(format!("{}.bin", game_name(kind)));
    let bytes: &'static [u8] = Box::leak(fs::read(path).ok()?.into_boxed_slice());
    Some(GameData::load(bytes).unwrap())
}

const SAVES: [&str; 5] = [
    "gen4_diamond.sav",
    "gen4_platinum.sav",
    "gen4_heartgold.sav",
    "gen5_black.sav",
    "gen5_white2.sav",
];

/// Each corpus save with its game data, or nothing (skip).
fn each(mut f: impl FnMut(&str, &mut Save, &GameData)) {
    for name in SAVES {
        let Ok(mut bytes) = fs::read(root().join("corpus/real/pkhex-avalonia").join(name)) else {
            eprintln!("skip {name}: no corpus save");
            continue;
        };
        let mut save = Save::open(&mut bytes).unwrap();
        let Some(game) = game(save.kind()) else {
            eprintln!("skip {name}: no data blobs");
            return;
        };
        f(name, &mut save, &game);
    }
}

fn pouch_of(save: &Save, kind: PouchKind) -> usize {
    save.pouches().iter().position(|p| p.kind == kind).unwrap()
}

fn items(save: &Save, pouch: usize) -> Vec<(u16, u16)> {
    bag::entries(save, pouch)
        .unwrap()
        .map(|e| (e.item, e.count))
        .collect()
}

/// Slots past the listed items are zeroed (the pouch is packed).
fn assert_packed(save: &Save, pouch: usize) {
    let n = items(save, pouch).len();
    for s in n..save.pouches()[pouch].slots as usize {
        assert_eq!(save.bag_item(pouch, s).unwrap(), (0, 0), "slot {s}");
    }
}

#[test]
fn pouch_lists_match_pkhex() {
    for kind in [
        SaveKind::DP,
        SaveKind::Pt,
        SaveKind::HGSS,
        SaveKind::BW,
        SaveKind::B2W2,
    ] {
        let Some(game) = game(kind) else { return };
        let path = root()
            .join("data/exports")
            .join(format!("{}.json", game_name(kind)));
        let json: serde_json::Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
        let want = json["pouch_items"].as_array().unwrap();
        assert_eq!(
            want.len(),
            verity::save::bag::pouches(kind).len(),
            "{kind:?}"
        );
        for (p, w) in want.iter().enumerate() {
            let got: Vec<u64> = (0..game.pouch_items(p).len())
                .map(|i| u64::from(game.pouch_items(p).get(i).unwrap()))
                .collect();
            let w: Vec<u64> = w
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_u64().unwrap())
                .collect();
            assert!(!got.is_empty(), "{kind:?} pouch {p}");
            assert_eq!(got, w, "{kind:?} pouch {p}");
        }
    }
}

/// Every key item a Mystery Gift item card gives is an event item (data, not a guess).
#[test]
fn gift_key_items_are_event_items() {
    let key = |generation: u8, item: u64| {
        let kind = if generation == 4 {
            SaveKind::HGSS
        } else {
            SaveKind::B2W2
        };
        let others = if generation == 4 {
            [SaveKind::DP, SaveKind::Pt]
        } else {
            [SaveKind::BW; 2]
        };
        [kind, others[0], others[1]].iter().any(|&k| {
            let Some(g) = game(k) else { return false };
            let p = verity::save::bag::pouches(k)
                .iter()
                .position(|d| d.kind == PouchKind::KeyItems)
                .unwrap();
            (0..g.pouch_items(p).len())
                .any(|i| g.pouch_items(p).get(i).map(u64::from) == Some(item))
        })
    };
    let Ok(g4) = fs::read(root().join("data/exports/gifts4.json")) else {
        return;
    };
    let g4: serde_json::Value = serde_json::from_slice(&g4).unwrap();
    let mut seen = 0;
    for card in g4.as_array().unwrap() {
        let raw = hex(card["raw"].as_str().unwrap());
        // PKHeX `PGT.ItemID` for item cards (`GiftType4.Item`).
        if card["gift_type"] == 3 {
            let item = u64::from(u32::from_le_bytes(raw[4..8].try_into().unwrap()));
            if key(4, item) {
                assert!(bag::is_event_item(4, item as u16), "Gen 4 item {item}");
                seen += 1;
            }
        }
    }
    let g5: serde_json::Value =
        serde_json::from_slice(&fs::read(root().join("data/exports/gifts5.json")).unwrap())
            .unwrap();
    for card in g5.as_array().unwrap() {
        let raw = hex(card.as_str().unwrap());
        // PKHeX `PGF.CardType` 2 is an item; `PGF.ItemID` is at 0.
        if raw[0xB3] == 2 {
            let item = u64::from(u16::from_le_bytes([raw[0], raw[1]]));
            if key(5, item) {
                assert!(bag::is_event_item(5, item as u16), "Gen 5 item {item}");
                seen += 1;
            }
        }
    }
    assert!(seen > 0, "the Enigma Stone and Liberty Pass cards");
}

fn hex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

#[test]
fn saves_hold_only_listed_items() {
    each(|name, save, game| {
        for p in 0..save.pouches().len() {
            let list = game.pouch_items(p);
            for e in bag::entries(save, p).unwrap() {
                let listed = (0..list.len()).any(|i| list.get(i) == Some(e.item));
                assert!(listed || e.locked, "{name} pouch {p}: item {}", e.item);
            }
        }
    });
}

#[test]
fn add_change_remove() {
    each(|name, save, game| {
        let p = pouch_of(save, PouchKind::Items);
        let before = items(save, p);
        let item = bag::addable(save, game, p).unwrap().next().unwrap();
        assert_eq!(bag::add(save, game, p, item, 0), Err(Refusal::Count));
        let max = bag::max_count(save, p).unwrap();
        assert_eq!(bag::add(save, game, p, item, max + 1), Err(Refusal::Count));
        bag::add(save, game, p, item, 5).unwrap();
        let after = items(save, p);
        assert_eq!(after.len(), before.len() + 1, "{name}");
        assert_eq!(*after.last().unwrap(), (item, 5), "{name}: appended");
        assert_eq!(
            bag::add(save, game, p, item, 1),
            Err(Refusal::AlreadyInPouch)
        );
        assert!(
            !bag::addable(save, game, p).unwrap().any(|i| i == item),
            "{name}"
        );

        let at = after.len() - 1;
        bag::set_count(save, game, p, at, max).unwrap();
        assert_eq!(items(save, p)[at], (item, max));
        assert_eq!(bag::set_count(save, game, p, at, 0), Err(Refusal::Count));

        bag::remove(save, game, p, at).unwrap();
        assert_eq!(items(save, p), before, "{name}: back as it was");
        assert_packed(save, p);
        assert_eq!(bag::remove(save, game, p, 999), Err(Refusal::NoSuchSlot));

        // Removing from the middle keeps the rest in order and packed.
        if before.len() >= 2 {
            bag::remove(save, game, p, 0).unwrap();
            assert_eq!(items(save, p), before[1..]);
            assert_packed(save, p);
        }
    });
}

#[test]
fn wrong_pouch_is_refused() {
    each(|name, save, game| {
        let items = pouch_of(save, PouchKind::Items);
        let medicine = pouch_of(save, PouchKind::Medicine);
        let potion = 17;
        assert_eq!(
            bag::add(save, game, items, potion, 1),
            Err(Refusal::NotInPouch),
            "{name}"
        );
        assert_eq!(
            bag::add(save, game, 99, potion, 1),
            Err(Refusal::NoSuchPouch)
        );
        let _ = bag::remove(save, game, medicine, 0);
        if !bag::entries(save, medicine)
            .unwrap()
            .any(|e| e.item == potion)
        {
            bag::add(save, game, medicine, potion, 1).unwrap();
        }
    });
}

#[test]
fn sorted_pouches_stay_in_game_order() {
    each(|name, save, game| {
        for kind in [PouchKind::TmHms, PouchKind::Berries] {
            let p = pouch_of(save, kind);
            let list = game.pouch_items(p);
            let all: Vec<u16> = (0..list.len()).filter_map(|i| list.get(i)).collect();
            // Clear the pouch, then add the last listed item before the first.
            while bag::entries(save, p).unwrap().next().is_some() {
                bag::remove(save, game, p, 0).unwrap();
            }
            bag::add(save, game, p, *all.last().unwrap(), 1).unwrap();
            bag::add(save, game, p, all[0], 1).unwrap();
            let got: Vec<u16> = items(save, p).iter().map(|e| e.0).collect();
            assert_eq!(got, [all[0], *all.last().unwrap()], "{name} {kind:?}");
        }
    });
}

#[test]
fn event_items_are_locked() {
    each(|name, save, game| {
        let g = save.generation();
        let p = pouch_of(save, PouchKind::KeyItems);
        let event = if g == 4 { 454 } else { 574 };
        assert_eq!(
            bag::add(save, game, p, event, 1),
            Err(Refusal::EventItem),
            "{name}"
        );
        assert!(
            !bag::addable(save, game, p)
                .unwrap()
                .any(|i| bag::is_event_item(g, i))
        );

        // One already in the bag (put there directly) can't be changed or removed.
        let n = items(save, p).len();
        save.set_bag_item(p, n, event, 1).unwrap();
        let e = bag::entries(save, p).unwrap().nth(n).unwrap();
        assert!(e.locked, "{name}");
        assert_eq!(bag::remove(save, game, p, n), Err(Refusal::EventItem));
        assert_eq!(bag::set_count(save, game, p, n, 1), Err(Refusal::EventItem));
        // Other changes to the pouch keep it.
        let unlocked = bag::entries(save, p).unwrap().position(|e| !e.locked);
        if let Some(i) = unlocked {
            bag::remove(save, game, p, i).unwrap();
            assert!(bag::entries(save, p).unwrap().any(|e| e.item == event));
        }
    });
}

/// Real saves can hold one item in two slots (the Black corpus save does); edits go by position.
#[test]
fn duplicates_are_edited_by_position() {
    each(|name, save, game| {
        let p = pouch_of(save, PouchKind::Items);
        let item = bag::addable(save, game, p).unwrap().next().unwrap();
        let n = items(save, p).len();
        save.set_bag_item(p, n, item, 3).unwrap();
        save.set_bag_item(p, n + 1, item, 4).unwrap();
        bag::set_count(save, game, p, n + 1, 9).unwrap();
        assert_eq!(items(save, p)[n..], [(item, 3), (item, 9)], "{name}");
        bag::remove(save, game, p, n).unwrap();
        assert_eq!(items(save, p)[n..], [(item, 9)], "{name}");
    });
}

#[test]
fn refusal_texts_fit_the_ds() {
    for code in 1..=7 {
        let r = Refusal::from_code(code).unwrap();
        assert_eq!(r.code(), code);
        assert!(r.text().len() <= 36 && r.text().is_ascii(), "{r:?}");
        assert!(r.c_text().ends_with('\0'));
    }
    assert_eq!(Refusal::from_code(0), None);
    assert_eq!(Refusal::from_code(8), None);
}
