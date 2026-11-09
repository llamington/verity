use std::fs;
use std::path::PathBuf;

use serde_json::Value;
use verity::data::encounters::EncounterData;
use verity::data::GameData;
use verity::legality::enc4::{wild_slots, MatchPkm};
use verity::legality::evo::{Evo4, EvoPkm};
use verity::pkm::{Format, Pkm};
use verity::rng::finder::{analyze, AnalyzeInput};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn dppt_wild_slot_match_equals_pkhex() {
    let b = root().join("data/blobs");
    let (Ok(enc), Ok(dp), Ok(hgss), Ok(b2w2)) = (
        fs::read(b.join("enc_dppt.bin")),
        fs::read(b.join("dp.bin")),
        fs::read(b.join("hgss.bin")),
        fs::read(b.join("b2w2.bin")),
    ) else {
        return;
    };
    let ed = EncounterData::load(&enc).unwrap();
    let (dpg, hg, g5) = (
        GameData::load(&dp).unwrap(),
        GameData::load(&hgss).unwrap(),
        GameData::load(&b2w2).unwrap(),
    );
    let evo = Evo4::new(dpg, hg);
    let ratio = |s: u16| {
        if s <= 493 {
            hg.personal(s as usize).gender_ratio()
        } else {
            g5.personal(s as usize).gender_ratio()
        }
    };
    let reports = root().join("corpus/reports/oracle-smoke");
    let Ok(rd) = fs::read_dir(&reports) else {
        return;
    };
    let (mut checked, mut bad, mut not_first) = (0, Vec::new(), Vec::<String>::new());
    for e in rd.flatten() {
        let p = e.path();
        let name = p.file_name().unwrap().to_string_lossy().to_string();
        let Some(stem) = name.strip_suffix(".dump.json") else {
            continue;
        };
        let dump: Value = serde_json::from_slice(&fs::read(&p).unwrap()).unwrap();
        for s in dump["slots"].as_array().unwrap() {
            let v = s["enc_version"].as_u64().unwrap();
            if s["enc_kind"] != "EncounterSlot4"
                || !(10..=12).contains(&v)
                || s["evo_origin"].is_null()
            {
                continue;
            }
            let sname = s["name"].as_str().unwrap();
            let pk = Pkm::from_decrypted(
                Format::Pk4,
                &fs::read(reports.join(format!("{stem}.pkm/{sname}.pk4"))).unwrap(),
            )
            .unwrap();
            let level = s["level"].as_u64().unwrap() as u8;
            let ep = EvoPkm {
                species: pk.species(),
                form: pk.form(),
                gender: pk.gender(),
                contest_beauty: pk.contest()[1],
                current_level: level,
                met_level: pk.met_level(),
                is_egg: pk.is_egg(),
                egg_location: pk.egg_location(),
            };
            let chain = evo.encounter_origin_chain(&ep);
            let input = AnalyzeInput::from_pkm(&pk);
            let pidiv = analyze(&input, ratio);
            let mp = MatchPkm {
                analyze: input,
                form: pk.form(),
                met_location: pk.met_location(),
                egg_location: pk.egg_location(),
                is_egg: pk.is_egg(),
                ball: pk.ball(),
                ground_tile: pk.ground_tile(),
                is_shiny: pk.is_shiny(),
                contest: pk.contest(),
                unown_form_ok: true,
            };
            let group: &[u8] = match pk.version() {
                10 => b"SlotsD",
                11 => b"SlotsP",
                _ => b"SlotsPt",
            };
            let m = wild_slots(&mp, &chain, pidiv, &ed, group, ratio, None);
            let n = |k: &str| s[k].as_i64().unwrap();
            let same = |x: &verity::data::encounters::Encounter| {
                x.location() as i64 == n("enc_location")
                    && x.slot_number() as i64 == n("enc_slot_number")
                    && x.slot_type() as i64 == n("enc_slot_type")
                    && x.species() as i64 == n("enc_species")
                    && x.level_min() as i64 == n("enc_level_min")
                    && x.can_radar() == s["enc_can_radar"].as_bool().unwrap()
                    && x.area_rate() as i64 == n("enc_rate")
            };
            if n("lead") == 0 {
                if !m
                    .fallback
                    .as_ref()
                    .and_then(|(x, _)| x.data())
                    .is_some_and(same)
                    || !m.candidates.is_empty()
                {
                    bad.push(format!("{stem}/{sname}: fallback mismatch"));
                }
                checked += 1;
                continue;
            }
            let pos = m.candidates.iter().position(|c| {
                c.encounter.data().is_some_and(same)
                    && c.lead.lead as i64 == n("lead")
                    && c.lead.seed as i64 == n("lead_seed")
            });
            match pos {
                Some(0) => {}
                Some(_) => not_first.push(format!("{stem}/{sname}")),
                None => bad.push(format!(
                    "{stem}/{sname}: not among {} candidates",
                    m.candidates.len()
                )),
            }
            checked += 1;
        }
    }
    for b in bad.iter().take(15) {
        eprintln!("{b}");
    }
    eprintln!("checked {checked}; PKHeX chose a later candidate (move checks) for {not_first:?}");
    assert!(checked > 0);
    assert!(bad.is_empty(), "{} mismatches", bad.len());
}
