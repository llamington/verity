use std::fs;
use std::path::PathBuf;

use serde_json::Value;
use verity::data::GameData;
use verity::legality::evo::{Evo4, EvoCriteria, EvoPkm};
use verity::pkm::{Format, Pkm};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn crit(v: &Value) -> Vec<[i64; 6]> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|e| core::array::from_fn(|i| e[i].as_i64().unwrap()))
        .collect()
}

fn ours(c: &[EvoCriteria]) -> Vec<[i64; 6]> {
    c.iter()
        .map(|e| {
            [
                e.species as i64,
                e.form as i64,
                e.level_min as i64,
                e.level_max as i64,
                e.level_up_required as i64,
                e.method as i64,
            ]
        })
        .collect()
}

#[test]
fn gen4_evolution_chains_match_pkhex() {
    let b = root().join("data/blobs");
    let (Ok(dp), Ok(hgss)) = (fs::read(b.join("dp.bin")), fs::read(b.join("hgss.bin"))) else {
        return;
    };
    let (dp, hg) = (GameData::load(&dp).unwrap(), GameData::load(&hgss).unwrap());
    let evo = Evo4::new(dp, hg);
    let reports = root().join("corpus/reports/oracle-smoke");
    let Ok(rd) = fs::read_dir(&reports) else {
        return;
    };
    let (mut checked, mut bad) = (0, Vec::new());
    for e in rd.flatten() {
        let p = e.path();
        let name = p.file_name().unwrap().to_string_lossy().to_string();
        let Some(stem) = name.strip_suffix(".dump.json") else {
            continue;
        };
        let dump: Value = serde_json::from_slice(&fs::read(&p).unwrap()).unwrap();
        for s in dump["slots"].as_array().unwrap() {
            if s["evo_origin"].is_null() {
                continue;
            }
            let sname = s["name"].as_str().unwrap();
            let pk = Pkm::from_decrypted(
                Format::Pk4,
                &fs::read(reports.join(format!("{stem}.pkm/{sname}.pk4"))).unwrap(),
            )
            .unwrap();
            let ep = EvoPkm {
                species: pk.species(),
                form: pk.form(),
                gender: pk.gender(),
                contest_beauty: pk.contest()[1],
                current_level: s["level"].as_u64().unwrap() as u8,
                met_level: pk.met_level(),
                is_egg: pk.is_egg(),
                egg_location: pk.egg_location(),
            };
            let origin = ours(&evo.encounter_origin_chain(&ep));
            if origin != crit(&s["evo_origin"]) {
                bad.push(format!(
                    "{stem}/{sname} origin: ours {origin:?} pkhex {:?}",
                    crit(&s["evo_origin"])
                ));
            }
            let enc_species = s["enc_species"].as_u64().unwrap() as u16;
            let enc_min = s["enc_level_min"].as_u64().unwrap() as u8;
            if s["enc_kind"] != "EncounterInvalid" {
                let g4 = ours(&evo.chain_for_encounter(&ep, enc_species, enc_min));
                if g4 != crit(&s["evo_gen4"]) {
                    bad.push(format!(
                        "{stem}/{sname} gen4 ({}): ours {g4:?} pkhex {:?}",
                        s["enc_kind"],
                        crit(&s["evo_gen4"])
                    ));
                }
            }
            checked += 1;
        }
    }
    for b in bad.iter().take(15) {
        eprintln!("{b}");
    }
    eprintln!("checked {checked}");
    assert!(bad.is_empty(), "{} mismatches", bad.len());
}
