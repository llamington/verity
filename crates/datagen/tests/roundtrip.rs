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

fn evo_for(ex: &std::path::Path, g: &str) -> Value {
    let gen = if matches!(g, "bw" | "b2w2") { 5 } else { 4 };
    datagen::load(&ex.join(format!("evolutions{gen}.json")))
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
        let evo = evo_for(&ex, g);
        let bytes = datagen::build_game(&j, &evo);
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
        for e in evo["evolutions"].as_array().unwrap() {
            let (sp, f) = (
                e["species"].as_u64().unwrap() as u16,
                e["form"].as_u64().unwrap() as u8,
            );
            if !j["forms"]
                .as_array()
                .unwrap()
                .iter()
                .any(|x| ints(x)[0] == sp as i64 && ints(x)[1] == f as i64)
            {
                continue;
            }
            let got: Vec<_> = gd.evolutions(gd.form_index(sp, f)).collect();
            let want: Vec<_> = e["evolutions"]
                .as_array()
                .unwrap()
                .iter()
                .map(|m| verity::data::Evolution {
                    species: m["Species"].as_u64().unwrap() as u16,
                    argument: m["Argument"].as_u64().unwrap() as u16,
                    form: m["Form"].as_u64().unwrap() as u8,
                    method: m["method"].as_u64().unwrap() as u8,
                    level: m["Level"].as_u64().unwrap() as u8,
                    level_up: m["LevelUp"].as_u64().unwrap() as u8,
                })
                .collect();
            if gd.form_index(sp, f) == gd.form_index(sp, 0) && f != 0 {
                continue;
            }
            assert_eq!(got, want, "{g} evolutions {sp}-{f}");
        }
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
                datagen::build_game(
                    &datagen::load(&ex.join(format!("{g}.json"))),
                    &evo_for(&ex, g),
                ),
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

#[test]
fn encounter_blobs_roundtrip() {
    use verity::data::encounters::{EncounterData, EncounterKind};
    let Some(ex) = exports() else { return };
    for (key, src) in datagen::ENCOUNTER_SOURCES {
        let j = datagen::load(&ex.join(format!("enc_{src}.json")));
        let bytes = datagen::build_encounters(&j);
        let ed = EncounterData::load(&bytes).unwrap();
        let mut total = 0;
        for (name, group) in j.as_object().unwrap() {
            let want: Vec<(Value, Option<&Value>)> = if group["type"] == "areas" {
                group["items"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .flat_map(|a| {
                        a["slots"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(move |s| (s.clone(), Some(a)))
                    })
                    .collect()
            } else {
                group["items"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|t| (t.clone(), None))
                    .collect()
            };
            let got: Vec<_> = ed.group(name.as_bytes()).collect();
            assert_eq!(got.len(), want.len(), "{key}/{name}");
            for (t, (w, area)) in got.iter().zip(&want) {
                let n = |k: &str, d: i64| w[k].as_i64().unwrap_or(d);
                assert_eq!(
                    t.kind(),
                    EncounterKind::from_pkhex_name(w["kind"].as_str().unwrap())
                );
                assert_eq!(t.species() as i64, n("Species", 0), "{key}/{name}");
                assert_eq!(t.form() as i64, n("Form", 0));
                assert_eq!(
                    (t.level_min() as i64, t.level_max() as i64),
                    (n("LevelMin", 0), n("LevelMax", 0))
                );
                assert_eq!(t.ability() as i64, n("Ability", 0));
                assert_eq!(t.shiny() as i64, n("Shiny", 0));
                assert_eq!(t.ball() as i64, n("FixedBall", 0));
                assert_eq!(
                    t.location() as i64,
                    n("Location", 0),
                    "{key}/{name} location"
                );
                assert_eq!(t.version() as i64, n("Version", 0));
                assert_eq!(t.ground_tile() as i64, n("GroundTile", 0));
                assert_eq!(
                    t.has(verity::data::encounters::flag::RANDOM_UNSPECIFIC_FORM),
                    w["IsRandomUnspecificForm"].as_bool().unwrap_or(false)
                );
                assert_eq!(
                    t.has(verity::data::encounters::flag::HIDDEN_GROTTO),
                    w["IsHiddenGrotto"].as_bool().unwrap_or(false)
                );
                if let Some(a) = area {
                    let ar = t.area().unwrap();
                    assert_eq!(ar.location() as i64, a["Location"].as_i64().unwrap());
                    assert_eq!(ar.slot_type() as i64, a["Type"].as_i64().unwrap());
                    assert_eq!(ar.rate() as i64, a["Rate"].as_i64().unwrap_or(0));
                    assert_eq!(t.slot_type() as i64, n("Type", 0));
                    assert_eq!(
                        t.area_rate() as i64,
                        n("AreaRate", a["Rate"].as_i64().unwrap_or(0))
                    );
                    assert_eq!(t.slot_number() as i64, n("SlotNumber", 0));
                    assert_eq!(
                        t.magnet_pull(),
                        (n("MagnetPullIndex", 0) as u8, n("MagnetPullCount", 0) as u8)
                    );
                    assert_eq!(
                        t.static_slot(),
                        (n("StaticIndex", 0) as u8, n("StaticCount", 0) as u8)
                    );
                    assert!(t.template().is_none());
                    continue;
                }
                let t = t.template().unwrap();
                assert_eq!(t.gender() as i64, n("Gender", 255));
                assert_eq!(t.nature() as i64, n("Nature", 25));
                assert_eq!(t.held_item() as i64, n("HeldItem", 0));
                assert_eq!(t.egg_location() as i64, n("EggLocation", 0));
                assert_eq!(t.pid() as i64, n("PID", 0));
                assert_eq!(
                    (t.tid() as i64, t.sid() as i64),
                    (n("TID16", 0), n("SID16", 0))
                );
                let mv: Vec<i64> = t.moves().iter().map(|m| *m as i64).collect();
                let wm: Vec<i64> = ["Move1", "Move2", "Move3", "Move4"]
                    .iter()
                    .map(|k| w["Moves"][*k].as_i64().unwrap_or(0))
                    .collect();
                assert_eq!(mv, wm);
                let ivs: Vec<i64> = t.ivs().iter().map(|v| *v as i64).collect();
                let wi: Vec<i64> = ["HP", "ATK", "DEF", "SPE", "SPA", "SPD"]
                    .iter()
                    .map(|k| w["IVs"][*k].as_i64().unwrap_or(0))
                    .collect();
                assert_eq!(ivs, wi);
                let ct: Vec<i64> = t.contest().iter().map(|v| *v as i64).collect();
                let wc: Vec<i64> = [
                    "ContestCool",
                    "ContestBeauty",
                    "ContestCute",
                    "ContestSmart",
                    "ContestTough",
                    "ContestSheen",
                ]
                .iter()
                .map(|k| n(k, 0))
                .collect();
                assert_eq!(ct, wc);
                assert_eq!(t.met_location() as i64, n("MetLocation", 0));
                assert_eq!(t.met_level() as i64, n("MetLevel", 0));
                assert_eq!(t.ot_gender() as i64, n("OTGender", 0));
                assert_eq!(t.course() as i64, n("Course", 0));
                assert!(t.area().is_none());
            }
            total += got.len();
        }
        eprintln!("ok enc_{key}: {total} templates, {} bytes", bytes.len());
    }
}
