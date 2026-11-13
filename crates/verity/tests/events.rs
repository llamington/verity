//! Gen 4 Mystery Gift delivery: every event available to a game is written to
//! the slots PKHeX reads it from, and duplicates are refused.
use std::fs;
use std::path::PathBuf;
use std::process::Command;

use serde_json::Value;
use verity::gift::inject4::{Refusal, available, check, inject};
use verity::gift::pcd::{Gift4, Gifts4, RANGER_INDEX};
use verity::save::{Save, SaveKind};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn oracle_path() -> PathBuf {
    root().join("tools/pkhex-oracle/bin/Release/net9.0/PkhexOracle")
}

fn sav_info(path: &std::path::Path) -> Value {
    let out = Command::new(oracle_path())
        .env(
            "DOTNET_ROOT",
            format!("{}/.dotnet", std::env::var("HOME").unwrap()),
        )
        .arg("sav-info")
        .arg(path)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).unwrap()
}

fn load(name: &str) -> Option<&'static [u8]> {
    let b = fs::read(root().join("data/blobs").join(name)).ok()?;
    Some(Box::leak(b.into_boxed_slice()))
}

fn ready() -> bool {
    root().join("data/blobs/events4.bin").exists()
        && oracle_path().exists()
        && root().join("corpus/synthetic/platinum.sav").exists()
}

fn events() -> Vec<Gift4<'static>> {
    let mut out = Vec::new();
    for f in ["gifts4.bin", "events4.bin"] {
        out.extend(Gifts4::load(load(f).unwrap()).unwrap().iter());
    }
    out
}

fn synthetic(name: &str) -> Vec<u8> {
    fs::read(root().join("corpus/synthetic").join(name)).unwrap()
}

const SAVES: [(&str, SaveKind); 3] = [
    ("diamond.sav", SaveKind::DP),
    ("platinum.sav", SaveKind::Pt),
    ("heartgold.sav", SaveKind::HGSS),
];

const DAYS: u16 = 0x1234;

fn verify_against_pkhex(step: usize) {
    if !ready() {
        eprintln!("skipping: needs blobs, oracle and corpus");
        return;
    }
    let tmp = std::env::temp_dir().join(format!("verity-p8-{}-{step}", std::process::id()));
    fs::create_dir_all(&tmp).unwrap();
    let all = events();
    let mut done = 0;
    for (name, kind) in SAVES {
        let original = synthetic(name);
        let eligible: Vec<_> = all
            .iter()
            .filter(|g| available(g, kind, if g.language() == 0 { 2 } else { g.language() }))
            .collect();
        assert!(eligible.len() > 100, "{name}: {}", eligible.len());
        for g in eligible.iter().step_by(step) {
            let mut data = original.clone();
            let mut save = Save::open(&mut data).unwrap();
            inject(&mut save, g, DAYS).unwrap();
            save.finalize();
            let path = tmp.join(name);
            fs::write(&path, &data).unwrap();
            let info = sav_info(&path);
            let m = &info["mystery"];
            let ctx = format!("{name} index {}", g.mgdb_index());
            assert_eq!(m["unlocked"], true, "{ctx}");
            assert_eq!(m["delivery_active"], true, "{ctx}");
            assert_eq!(
                m["pgt"][0]["type"].as_u64().unwrap(),
                g.card_type() as u64,
                "{ctx}"
            );
            assert_eq!(
                m["pgt"][0]["species"].as_u64().unwrap() != 0,
                g.is_entity(),
                "{ctx}"
            );
            if g.is_entity() {
                assert_eq!(
                    m["pgt"][0]["species"].as_u64().unwrap(),
                    g.species() as u64,
                    "{ctx}"
                );
            }
            let ranger = g.mgdb_index() == RANGER_INDEX;
            let dp = kind == SaveKind::DP;
            let link = match (ranger, dp) {
                (true, true) => 4,
                (true, false) => 3,
                (false, true) => 1,
                (false, false) => 0,
            };
            assert_eq!(m["pgt"][0]["slot"].as_u64().unwrap(), link, "{ctx}");
            let flags: Vec<u64> = m["received_flags"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_u64().unwrap())
                .collect();
            if ranger {
                assert!(flags.is_empty(), "{ctx}");
                assert_eq!(m["pcd"][0]["card_id"].as_u64().unwrap(), 0, "{ctx}");
            } else {
                assert_eq!(flags, [g.card_id() as u64], "{ctx}");
                assert_eq!(
                    m["pcd"][0]["card_id"].as_u64().unwrap(),
                    g.card_id() as u64,
                    "{ctx}"
                );
                let raw = m["pcd"][0]["raw"].as_str().unwrap();
                let date = &raw[0x354 * 2..0x356 * 2];
                assert_eq!(date, "3412", "{ctx}");
            }
            if kind == SaveKind::DP {
                let s = m["sentinels"].as_array().unwrap();
                assert_eq!(s[0].as_u64().unwrap(), 0xEDB8_8320, "{ctx}");
                assert_eq!(
                    s[8].as_u64().unwrap(),
                    if ranger { 0 } else { 0xEDB8_8320 },
                    "{ctx}"
                );
            }
            let checksum = info["checksum_info"].as_str().unwrap_or("");
            assert!(!checksum.contains("Invalid"), "{ctx}: {checksum}");
            done += 1;
        }
    }
    eprintln!("verified {done} injections against PKHeX");
    let _ = fs::remove_dir_all(&tmp);
}

#[test]
fn inject_sample_matches_pkhex() {
    verify_against_pkhex(40);
}

#[test]
#[ignore = "runs PKHeX once per event (minutes)"]
fn inject_every_event_matches_pkhex() {
    verify_against_pkhex(1);
}

fn pick(all: &[Gift4<'static>], kind: SaveKind, n: usize) -> Vec<Gift4<'static>> {
    // Distinct card IDs: the same card exists for several regions.
    let mut out: Vec<Gift4> = Vec::new();
    for g in all {
        if g.mgdb_index() != RANGER_INDEX
            && available(g, kind, 2)
            && !out.iter().any(|o| o.card_id() == g.card_id())
        {
            out.push(*g);
            if out.len() == n {
                break;
            }
        }
    }
    assert_eq!(out.len(), n);
    out
}

#[test]
fn duplicates_and_full_slots_are_refused() {
    if !ready() {
        eprintln!("skipping: needs blobs, oracle and corpus");
        return;
    }
    let all = events();
    for (name, kind) in SAVES {
        let mut data = synthetic(name);
        let mut save = Save::open(&mut data).unwrap();
        let cards = pick(&all, kind, 5);

        assert_eq!(check(&save, &cards[0]), Ok(()), "{name}");
        inject(&mut save, &cards[0], DAYS).unwrap();
        let before = save.data().to_vec();
        assert_eq!(
            inject(&mut save, &cards[0], DAYS),
            Err(Refusal::AlreadyReceived),
            "{name}"
        );
        assert_eq!(save.data(), &before[..], "{name}: refused write changed");

        inject(&mut save, &cards[1], DAYS).unwrap();
        inject(&mut save, &cards[2], DAYS).unwrap();
        let before = save.data().to_vec();
        assert_eq!(
            inject(&mut save, &cards[3], DAYS),
            Err(Refusal::AlbumFull),
            "{name}"
        );
        assert_eq!(save.data(), &before[..], "{name}: refused write changed");

        let ranger = all
            .iter()
            .find(|g| g.mgdb_index() == RANGER_INDEX)
            .expect("Ranger Manaphy record");
        inject(&mut save, ranger, DAYS).unwrap();
        assert_eq!(check(&save, ranger), Err(Refusal::AlreadyPending), "{name}");
    }
}

#[test]
fn pending_slots_full_is_refused() {
    if !ready() {
        eprintln!("skipping: needs blobs, oracle and corpus");
        return;
    }
    let all = events();
    let cards = pick(&all, SaveKind::Pt, 1);
    let mut data = synthetic("platinum.sav");
    let mut save = Save::open(&mut data).unwrap();
    let (general, m) = save.mystery4_mut().unwrap();
    for i in 0..8 {
        general[m + 0x100 + i * 0x104] = 3;
    }
    assert_eq!(check(&save, &cards[0]), Err(Refusal::PendingFull));
}

#[test]
fn discarded_card_slot_is_reused_but_stays_received() {
    if !ready() {
        eprintln!("skipping: needs blobs, oracle and corpus");
        return;
    }
    // Discarding a Wonder Card clears its type and leaves the received flag set.
    let all = events();
    let cards = pick(&all, SaveKind::Pt, 4);
    let mut data = synthetic("platinum.sav");
    let mut save = Save::open(&mut data).unwrap();
    for c in &cards[..3] {
        inject(&mut save, c, DAYS).unwrap();
    }
    assert_eq!(check(&save, &cards[3]), Err(Refusal::AlbumFull));
    let (general, m) = save.mystery4_mut().unwrap();
    let album = m + 0x100 + 8 * 0x104;
    general[album] = 0;
    general[album + 1] = 0;
    assert_eq!(check(&save, &cards[3]), Ok(()));
    assert_eq!(check(&save, &cards[0]), Err(Refusal::AlreadyReceived));
}

#[test]
fn every_card_stores_a_pending_gift() {
    if !ready() {
        eprintln!("skipping: needs blobs, oracle and corpus");
        return;
    }
    // `hasWonderCard` and `savePgt` (0x152 bits 2 and 3): every card in the database has both,
    // so `inject` needs no card-only or gift-only path.
    for g in events() {
        if g.mgdb_index() != RANGER_INDEX {
            assert_eq!(g.pcd()[0x152] & 0x0C, 0x0C, "index {}", g.mgdb_index());
        }
    }
}

#[test]
fn received_flags_on_a_genuine_save_block_redelivery() {
    let path = root().join("corpus/real/pkmds-blazor/Pokemon Platinum.sav");
    if !ready() || !path.exists() {
        eprintln!("skipping: needs blobs, oracle and the real save");
        return;
    }
    let mut data = fs::read(path).unwrap();
    let save = Save::open(&mut data).unwrap();
    let member_card = events()
        .into_iter()
        .find(|g| g.card_id() == 18 && g.language() == 2)
        .expect("English Member Card");
    assert_eq!(check(&save, &member_card), Err(Refusal::AlreadyReceived));
}

#[test]
fn filter_matches_game_and_language() {
    if !ready() {
        eprintln!("skipping: needs blobs, oracle and corpus");
        return;
    }
    let all = events();
    for g in &all {
        if g.mgdb_index() == RANGER_INDEX {
            for l in [1, 2, 3, 4, 5, 7] {
                assert!(available(g, SaveKind::Pt, l));
            }
            assert!(!available(g, SaveKind::Pt, 8), "no Korean Ranger Manaphy");
            continue;
        }
        let l = g.language();
        assert_ne!(l, 0, "only Ranger Manaphy is language-free");
        let other = if l == 2 { 1 } else { 2 };
        for kind in [SaveKind::DP, SaveKind::Pt, SaveKind::HGSS] {
            assert!(!available(g, kind, other), "index {}", g.mgdb_index());
        }
        let mask = g.games();
        assert_eq!(
            available(g, SaveKind::Pt, l),
            g.injectable() && mask & 4 != 0
        );
        assert_eq!(
            available(g, SaveKind::DP, l),
            g.injectable() && mask & 3 != 0
        );
        assert_eq!(
            available(g, SaveKind::HGSS, l),
            g.injectable() && mask & 24 != 0
        );
    }
    // Filtered counts are large enough to be a real database, and cover every language.
    for l in [1, 2, 3, 4, 5, 7, 8] {
        let n = all.iter().filter(|g| g.language() == l).count();
        assert!(n >= 30, "language {l}: {n}");
    }
}
