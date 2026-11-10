//! Gen 5 Wonder Card delivery, read back through PKHeX.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use verity::gift::inject5::{available, check, inject, Refusal5};
use verity::gift::pgf::{Events5, Gifts5, Pgf, PGF_SIZE};
use verity::save::Save;

const DATE: (u16, u8, u8) = (2026, 11, 9);
const BLOCK: std::ops::Range<usize> = 0x1C800..0x1C800 + 0xA94;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn blob(name: &str) -> Option<&'static [u8]> {
    let b = fs::read(root().join("data/blobs").join(name)).ok()?;
    Some(Box::leak(b.into_boxed_slice()))
}

fn oracle_gifts(path: &Path) -> serde_json::Value {
    let out = Command::new(root().join("tools/pkhex-oracle/bin/Release/net9.0/PkhexOracle"))
        .env(
            "DOTNET_ROOT",
            format!("{}/.dotnet", std::env::var("HOME").unwrap()),
        )
        .arg("sav-gifts5")
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

/// The card as the save should hold it.
fn expected(g: &Pgf) -> Vec<u8> {
    let mut c = g.0.to_vec();
    c[0xAC] = DATE.2;
    c[0xAD] = DATE.1;
    c[0xAE..0xB0].copy_from_slice(&DATE.0.to_le_bytes());
    c
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02X}")).collect()
}

#[test]
fn p8_gen5_delivery_read_back_by_pkhex() {
    let (Some(g), Some(e)) = (blob("gifts5.bin"), blob("events5.bin")) else {
        return;
    };
    let (cards, events) = (Gifts5::load(g).unwrap(), Events5::load(e).unwrap());
    let tmp = std::env::temp_dir().join(format!("verity-p8g5-{}", std::process::id()));
    fs::create_dir_all(&tmp).unwrap();
    let mut checked = 0;
    for name in [
        "gen5_white2.sav",
        "gen5_black.sav",
        "Test-Save-White-2.sav",
        "Black - Full Completion.sav",
    ] {
        let Ok(original) = fs::read(root().join("sd/saves").join(name)) else {
            continue;
        };
        let mut raw = original.clone();
        let (kind, version, language) = {
            let s = Save::open(&mut raw).unwrap();
            (s.kind(), s.game_version(), s.trainer().language)
        };
        assert!(matches!(version, 20..=23));
        let before = {
            let p = tmp.join("before.sav");
            fs::write(&p, &original).unwrap();
            oracle_gifts(&p)
        };
        let held = before["cards"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|c| c.as_str().unwrap().chars().any(|x| x != '0'))
            .count();
        let offered: Vec<Pgf> = cards
            .iter()
            .enumerate()
            .filter(|(i, _)| available(events.get(*i).unwrap(), kind, version, language))
            .map(|(_, c)| c)
            .collect();
        let mut save = Save::open(&mut raw).unwrap();
        let fresh: Vec<Pgf> = offered
            .iter()
            .copied()
            .filter(|c| check(&save, c).is_ok())
            .collect();
        // One card per ID: several distributions can share an ID.
        let mut ids = Vec::new();
        let distinct: Vec<Pgf> = fresh
            .into_iter()
            .filter(|c| {
                let new = !ids.contains(&c.card_id());
                ids.push(c.card_id());
                new
            })
            .collect();
        let room = (12 - held).min(distinct.len());
        assert!(room > 0, "{name}: nothing to deliver");
        for c in &distinct[..room] {
            inject(&mut save, c, DATE).unwrap();
        }
        // A full album refuses another card.
        if let Some(next) = distinct.get(room) {
            assert_eq!(inject(&mut save, next, DATE), Err(Refusal5::AlbumFull));
        }
        // A delivered card reports as waiting at the deliveryman.
        assert_eq!(check(&save, &distinct[0]), Err(Refusal5::AlreadyPending));
        save.finalize();

        // Only the Mystery Gift block and checksums changed.
        let diff: Vec<usize> = (0..original.len())
            .filter(|&i| original[i] != raw[i] && !BLOCK.contains(&i))
            .collect();
        assert!(diff.len() <= 8, "{name}: {diff:x?}");

        let p = tmp.join("after.sav");
        fs::write(&p, &raw).unwrap();
        let after = oracle_gifts(&p);
        assert_eq!(after["checksums"], true, "{name}");
        let got: Vec<&str> = after["cards"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c.as_str().unwrap())
            .collect();
        let was: Vec<&str> = before["cards"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c.as_str().unwrap())
            .collect();
        for (i, c) in distinct[..room].iter().enumerate() {
            assert_eq!(got[held + i], hex(&expected(c)), "{name} slot {}", held + i);
            assert!(
                after["flags"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|f| f.as_u64() == Some(c.card_id() as u64)),
                "{name}: flag {}",
                c.card_id()
            );
            assert_eq!(got[held + i].len(), PGF_SIZE * 2);
            checked += 1;
        }
        assert_eq!(&got[..held], &was[..held], "{name}: existing cards moved");
    }
    let _ = fs::remove_dir_all(&tmp);
    eprintln!("delivered and read back {checked}");
    assert!(checked > 0 || !root().join("sd/saves/gen5_white2.sav").exists());
}
