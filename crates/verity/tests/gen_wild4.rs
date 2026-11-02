use std::fs;
use std::path::PathBuf;
use std::process::Command;

use verity::data::encounters::{Encounter, EncounterData};
use verity::data::GameData;
use verity::gen::wild4::{closest_legal_ivs, generate, GenError, Lead, Rand, Request};
use verity::rng::method_j::Slot4;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[derive(Clone, Copy, Debug)]
enum Case {
    Regular,
    Shiny,
    Sync,
    SyncShiny,
    CuteCharm,
    ClosestIvs,
    RadarChain,
}

fn run(per_case: usize) {
    let b = root().join("data/blobs");
    let (Ok(enc), Ok(pt)) = (fs::read(b.join("enc_dppt.bin")), fs::read(b.join("pt.bin"))) else {
        eprintln!("skipping: build data blobs first");
        return;
    };
    let oracle = root().join("tools/pkhex-oracle/bin/Release/net9.0/PkhexOracle");
    if !oracle.exists() {
        eprintln!("skipping: oracle not built");
        return;
    }
    let ed = EncounterData::load(&enc).unwrap();
    let gd = GameData::load(&pt).unwrap();
    let ratio = |s: u16| gd.personal(s as usize).gender_ratio();
    let slots: Vec<Encounter> = ed
        .group(b"SlotsPt")
        .filter(|e| e.slot_type() == 0 && e.species() != 201)
        .collect();
    let mut rand = Rand::new(0xC0DE);
    let tmp = std::env::temp_dir().join(format!("verity-wild4-{}", std::process::id()));
    fs::create_dir_all(&tmp).unwrap();

    for case in [
        Case::Regular,
        Case::Shiny,
        Case::Sync,
        Case::SyncShiny,
        Case::CuteCharm,
        Case::ClosestIvs,
        Case::RadarChain,
    ] {
        let mut dist_hist = std::collections::BTreeMap::new();
        let mut out = Vec::new();
        let mut skipped = 0;
        while out.len() < per_case {
            let e = slots[rand.next_u32() as usize % slots.len()];
            let slot = Slot4::from_encounter(&e).unwrap();
            let tid = rand.next_u32() as u16;
            let sid = rand.next_u32() as u16;
            let nat = (rand.next_u32() % 25) as u8;
            let gr = ratio(e.species());
            let (shiny, lead) = match case {
                Case::Regular => (false, Lead::None),
                Case::Shiny => (true, Lead::None),
                Case::Sync => (false, Lead::Synchronize(nat)),
                Case::SyncShiny => (true, Lead::Synchronize(nat)),
                Case::CuteCharm => (
                    false,
                    Lead::CuteCharm {
                        gender: (rand.next_u32() & 1) as u8,
                    },
                ),
                Case::ClosestIvs => (false, Lead::None),
                Case::RadarChain => (true, Lead::PokeRadarChain),
            };
            let req = Request {
                slot,
                tid,
                sid,
                species: e.species(),
                gender_ratio: gr,
                met_level: None,
                nature: matches!(case, Case::ClosestIvs).then_some(nat),
                gender: None,
                ability_bit: None,
                shiny,
                lead,
                hidden_power_type: None,
                location: e.location(),
            };
            let result = if let Case::ClosestIvs = case {
                let target: [u8; 6] = core::array::from_fn(|_| (rand.next_u32() % 32) as u8);
                closest_legal_ivs(&req, target, 200_000, ratio).map(|p| {
                    *dist_hist.entry(p.distance).or_insert(0u32) += 1;
                    let got: [u8; 6] =
                        core::array::from_fn(|i| ((p.generated.iv32 >> (5 * i)) & 31) as u8);
                    assert_eq!(got, p.ivs);
                    assert_eq!(p.generated.pid % 25, nat as u32);
                    p.generated
                })
            } else {
                generate(&req, &mut rand, 50_000, ratio)
            };
            let g = match result {
                Ok(g) => g,
                Err(GenError::CuteCharmNeedsGenderedSpecies | GenError::RadarNotAllowedHere) => {
                    skipped += 1;
                    continue;
                }
                Err(err) => panic!("{case:?} {req:?}: {err:?}"),
            };
            let x = (g.pid >> 16) ^ (g.pid & 0xFFFF) ^ (tid ^ sid) as u32;
            assert_eq!(x < 8, shiny, "{case:?}");
            if let Lead::Synchronize(n) = lead {
                assert_eq!(g.pid % 25, n as u32);
            }
            let a = e.area().unwrap();
            out.push(serde_json::json!({
                "group": "SlotsPt", "location": e.location(), "slot_type": e.slot_type(),
                "slot_number": e.slot_number(), "species": e.species(), "form": e.form(),
                "level_min": e.level_min(), "level_max": e.level_max(),
                "ground": a.ground_tile(), "rate": a.rate(),
                "tid": tid, "sid": sid, "pid": g.pid, "iv32": g.iv32, "level": g.level,
            }));
        }
        let path = tmp.join(format!("{case:?}.json"));
        fs::write(&path, serde_json::to_vec(&out).unwrap()).unwrap();
        let res = Command::new(&oracle)
            .env(
                "DOTNET_ROOT",
                format!("{}/.dotnet", std::env::var("HOME").unwrap()),
            )
            .arg("check-wild4")
            .arg(&path)
            .output()
            .unwrap();
        assert!(
            res.status.success(),
            "{}",
            String::from_utf8_lossy(&res.stderr)
        );
        let v: serde_json::Value = serde_json::from_slice(&res.stdout).unwrap();
        eprintln!(
            "{case:?}: {}/{} legal (skipped {skipped} slots that cannot use this method)",
            v["valid"], v["total"]
        );
        assert_eq!(v["valid"], v["total"], "{case:?}: {:#}", v["failures"]);
    }
    let _ = fs::remove_dir_all(&tmp);
}

#[test]
fn generated_pt_grass_is_legal() {
    run(300);
}

/// PLAN P4 acceptance: 10^4 per method. `cargo test --release -- --ignored`.
#[test]
#[ignore]
fn generated_pt_grass_is_legal_1e4() {
    run(10_000);
}

#[test]
#[ignore]
fn closest_ivs_timing() {
    let b = root().join("data/blobs");
    let (enc, pt) = (
        fs::read(b.join("enc_dppt.bin")).unwrap(),
        fs::read(b.join("pt.bin")).unwrap(),
    );
    let ed = EncounterData::load(&enc).unwrap();
    let gd = GameData::load(&pt).unwrap();
    let ratio = |s: u16| gd.personal(s as usize).gender_ratio();
    let slots: Vec<Encounter> = ed
        .group(b"SlotsPt")
        .filter(|e| e.slot_type() == 0 && e.species() != 201)
        .collect();
    let mut rand = Rand::new(7);
    for i in 0..20 {
        let e = slots[rand.next_u32() as usize % slots.len()];
        let req = Request {
            slot: Slot4::from_encounter(&e).unwrap(),
            tid: 1,
            sid: 2,
            species: e.species(),
            gender_ratio: ratio(e.species()),
            met_level: None,
            nature: Some((rand.next_u32() % 25) as u8),
            gender: None,
            ability_bit: None,
            shiny: false,
            lead: Lead::None,
            hidden_power_type: None,
            location: e.location(),
        };
        let target: [u8; 6] = core::array::from_fn(|_| (rand.next_u32() % 32) as u8);
        let t = std::time::Instant::now();
        let r = closest_legal_ivs(&req, target, 200_000, ratio);
        eprintln!("{i}: {:?} in {:?}", r.map(|p| p.distance), t.elapsed());
    }
}
