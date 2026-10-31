use std::fs;
use std::path::PathBuf;

use serde_json::Value;
use verity::data::encounters::EncounterData;
use verity::data::GameData;
use verity::pkm::{Format, Pkm};
use verity::rng::finder::{analyze, AnalyzeInput};
use verity::rng::method_j::{lead_info4_dppt, Slot4};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn dppt_leads_match_pkhex() {
    let blobs = root().join("data/blobs");
    let (Ok(enc), Ok(hgss), Ok(b2w2)) = (
        fs::read(blobs.join("enc_dppt.bin")),
        fs::read(blobs.join("hgss.bin")),
        fs::read(blobs.join("b2w2.bin")),
    ) else {
        eprintln!("skipping: build data blobs first");
        return;
    };
    let ed = EncounterData::load(&enc).unwrap();
    let (g4, g5) = (
        GameData::load(&hgss).unwrap(),
        GameData::load(&b2w2).unwrap(),
    );
    let ratio = |s: u16| {
        if s <= 493 {
            g4.personal(s as usize).gender_ratio()
        } else {
            g5.personal(s as usize).gender_ratio()
        }
    };
    let reports = root().join("corpus/reports/oracle-smoke");
    let Ok(rd) = fs::read_dir(&reports) else {
        return;
    };
    let (mut checked, mut leads) = (0, std::collections::BTreeMap::new());
    let mut bad = Vec::new();
    for e in rd.flatten() {
        let p = e.path();
        let name = p.file_name().unwrap().to_string_lossy().to_string();
        let Some(stem) = name.strip_suffix(".dump.json") else {
            continue;
        };
        let dump: Value = serde_json::from_slice(&fs::read(&p).unwrap()).unwrap();
        if dump["version"].as_str().unwrap().contains(['B', 'W']) {
            continue;
        }
        for s in dump["slots"].as_array().unwrap() {
            let v = s["enc_version"].as_u64().unwrap() as u8;
            if s["enc_kind"] != "EncounterSlot4" || !(10..=12).contains(&v) {
                continue;
            }
            let group: &[u8] = match v {
                10 => b"SlotsD",
                11 => b"SlotsP",
                _ => b"SlotsPt",
            };
            let n = |k: &str| s[k].as_i64().unwrap();
            let slot = ed
                .group(group)
                .find(|x| {
                    x.location() as i64 == n("enc_location")
                        && x.slot_type() as i64 == n("enc_slot_type")
                        && x.slot_number() as i64 == n("enc_slot_number")
                        && x.species() as i64 == n("enc_species")
                        && x.form() as i64 == n("enc_form")
                        && x.level_min() as i64 == n("enc_level_min")
                        && x.level_max() as i64 == n("enc_level_max")
                        && x.can_radar() == s["enc_can_radar"].as_bool().unwrap()
                        && x.pressure_level() as i64 == n("enc_pressure")
                        && x.ground_tile() as i64 == n("enc_ground")
                        && x.area_rate() as i64 == n("enc_rate")
                        && [x.magnet_pull().0 as i64, x.magnet_pull().1 as i64]
                            == [
                                s["enc_magnet"][0].as_i64().unwrap(),
                                s["enc_magnet"][1].as_i64().unwrap(),
                            ]
                        && [x.static_slot().0 as i64, x.static_slot().1 as i64]
                            == [
                                s["enc_static"][0].as_i64().unwrap(),
                                s["enc_static"][1].as_i64().unwrap(),
                            ]
                })
                .unwrap_or_else(|| panic!("{stem}/{}: slot not found", s["name"]));
            let slot = Slot4::from_encounter(&slot).unwrap();
            let sname = s["name"].as_str().unwrap();
            let pk = Pkm::from_decrypted(
                Format::Pk4,
                &fs::read(reports.join(format!("{stem}.pkm/{sname}.pk4"))).unwrap(),
            )
            .unwrap();
            let input = AnalyzeInput::from_pkm(&pk);
            let pidiv = analyze(&input, ratio);
            let ml = pk.met_level();
            let r = lead_info4_dppt(&input, &slot, pidiv, ml, ml, 4, ratio);
            *leads.entry(format!("{:?}", r.lead)).or_insert(0) += 1;
            let (wl, ws) = (n("lead"), n("lead_seed"));
            if r.lead as i64 != wl || (r.is_valid() && r.seed as i64 != ws) {
                bad.push(format!(
                    "{stem}/{sname}: ours {:?} {:08X} pkhex {wl} {ws:08X}",
                    r.lead, r.seed
                ));
            }
            checked += 1;
        }
    }
    for b in bad.iter().take(20) {
        eprintln!("{b}");
    }
    eprintln!("checked {checked}: {leads:?}");
    assert!(checked > 0);
    assert!(bad.is_empty(), "{} mismatches", bad.len());
}
