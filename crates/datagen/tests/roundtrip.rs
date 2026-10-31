use std::path::PathBuf;

use serde_json::Value;
use verity::data::{move_table as mt, GameData, MoveData, Strings};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn exports() -> Option<PathBuf> {
    let p = root().join("data/exports");
    p.join("manifest.json").exists().then_some(p)
}

fn ints(v: &Value) -> Vec<i64> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_i64().unwrap())
        .collect()
}

#[test]
fn game_blobs_roundtrip() {
    let Some(ex) = exports() else {
        eprintln!("skipping: run pkhex-oracle export-data first");
        return;
    };
    for g in datagen::GAMES {
        let j = datagen::load(&ex.join(format!("{g}.json")));
        let bytes = datagen::build_game(&j);
        let gd = GameData::load(&bytes).unwrap();

        let personal = j["personal"].as_array().unwrap();
        assert_eq!(gd.personal_count(), personal.len(), "{g}");
        for (i, raw) in personal.iter().enumerate() {
            let want: Vec<u8> = ints(raw).iter().map(|b| *b as u8).collect();
            assert_eq!(gd.personal(i).raw(), &want[..], "{g} personal {i}");
        }
        for f in j["forms"].as_array().unwrap() {
            let f = ints(f);
            assert_eq!(
                gd.form_index(f[0] as u16, f[1] as u8),
                f[2] as usize,
                "{g} form index {f:?}"
            );
        }
        for e in j["levelup"].as_array().unwrap() {
            let idx = e["index"].as_u64().unwrap() as usize;
            let want: Vec<(u8, u16)> = e["moves"]
                .as_array()
                .unwrap()
                .iter()
                .map(|m| {
                    let m = ints(m);
                    (m[0] as u8, m[1] as u16)
                })
                .collect();
            let got: Vec<_> = gd.levelup(idx).collect();
            assert_eq!(got, want, "{g} levelup {idx}");
        }
        for e in j["egg_moves"].as_array().unwrap() {
            let idx = gd.form_index(
                e["species"].as_u64().unwrap() as u16,
                e["form"].as_u64().unwrap() as u8,
            );
            let got: Vec<u16> = gd.egg_moves(idx).iter().collect();
            let want: Vec<u16> = ints(&e["moves"]).iter().map(|v| *v as u16).collect();
            assert_eq!(got, want, "{g} egg moves {idx}");
        }
        if let Some(t) = j["type_tutors"].as_array() {
            for (i, bits) in t.iter().enumerate() {
                for (b, v) in ints(bits).iter().enumerate() {
                    assert_eq!(gd.type_tutor(i, b), *v != 0, "{g} tutor {i}/{b}");
                }
            }
        }
        let held: Vec<u16> = gd.held_items().iter().collect();
        assert_eq!(
            held,
            ints(&j["held_items"])
                .iter()
                .map(|v| *v as u16)
                .collect::<Vec<_>>(),
            "{g}"
        );
        eprintln!("ok {g}: {} bytes", bytes.len());
    }
}

#[test]
fn move_and_string_blobs_roundtrip() {
    let Some(ex) = exports() else { return };
    let tables = datagen::load(&ex.join("move_tables.json"));
    for gen in [4, 5] {
        let j = datagen::load(&ex.join(format!("moves{gen}.json")));
        let bytes = datagen::build_moves(&j, &tables);
        let md = MoveData::load(&bytes).unwrap();
        let moves = j["moves"].as_array().unwrap();
        assert_eq!(md.max_move() as usize, moves.len() - 1);
        for (i, m) in moves.iter().enumerate() {
            assert_eq!(md.base_pp(i as u16) as u64, m["pp"].as_u64().unwrap());
            assert_eq!(md.move_type(i as u16) as u64, m["type"].as_u64().unwrap());
        }
        let check = |id: u16, name: &str| {
            let got: Vec<i64> = md.list(id).iter().map(i64::from).collect();
            assert_eq!(got, ints(&tables[name]), "{name}");
        };
        if gen == 4 {
            check(mt::G4_TM, "g4_tm");
            check(mt::G4_TUTOR, "g4_tutor_moves");
            check(mt::G4_HM_HGSS, "g4_hm_hgss");
        } else {
            check(mt::G5_TMHM, "g5_tmhm");
        }
    }
    for l in datagen::LANGS {
        let j = datagen::load(&ex.join(format!("strings_{l}.json")));
        let bytes = datagen::build_strings(&j);
        let s = Strings::load(&bytes).unwrap();
        for (id, name) in datagen::STRING_CATEGORIES {
            let want = j[name].as_array().unwrap();
            assert_eq!(s.count(id), want.len(), "{l} {name}");
            for (i, w) in want.iter().enumerate() {
                let got: Vec<u16> = s.get(id, i).iter().collect();
                assert_eq!(
                    String::from_utf16(&got).unwrap(),
                    w.as_str().unwrap(),
                    "{l} {name} {i}"
                );
            }
        }
    }
}

#[test]
fn personal_matches_pkhex_per_pokemon() {
    let Some(ex) = exports() else { return };
    let reports = root().join("corpus/reports/oracle-smoke");
    let Ok(rd) = std::fs::read_dir(&reports) else {
        return;
    };
    let blobs: Vec<(String, Vec<u8>)> = datagen::GAMES
        .iter()
        .map(|g| {
            (
                g.to_string(),
                datagen::build_game(&datagen::load(&ex.join(format!("{g}.json")))),
            )
        })
        .collect();
    let mut checked = 0;
    for e in rd.flatten() {
        let p = e.path();
        if !p.to_string_lossy().ends_with(".dump.json") {
            continue;
        }
        let dump = datagen::load(&p);
        let game = match dump["version"].as_str().unwrap() {
            "DP" | "D" | "P" => "dp",
            "Pt" => "pt",
            "HGSS" | "HG" | "SS" => "hgss",
            "B" | "W" => "bw",
            _ => "b2w2",
        };
        let gd = GameData::load(&blobs.iter().find(|b| b.0 == game).unwrap().1).unwrap();
        for s in dump["slots"].as_array().unwrap() {
            let pi = gd.species_form(
                s["species"].as_u64().unwrap() as u16,
                s["form"].as_u64().unwrap() as u8,
            );
            let base: Vec<i64> = pi.base_stats().iter().map(|v| *v as i64).collect();
            assert_eq!(
                base,
                ints(&s["base_stats"]),
                "{} {}",
                p.display(),
                s["name"]
            );
            assert_eq!(
                pi.gender_ratio() as u64,
                s["gender_ratio"].as_u64().unwrap()
            );
            assert_eq!(pi.growth() as u64, s["exp_growth"].as_u64().unwrap());
            let ab: Vec<i64> = pi.abilities().iter().map(|v| *v as i64).collect();
            assert_eq!(
                ab,
                ints(&s["abilities"]),
                "{} {} abilities",
                p.display(),
                s["name"]
            );
            checked += 1;
        }
    }
    eprintln!("checked {checked} pkm");
}
