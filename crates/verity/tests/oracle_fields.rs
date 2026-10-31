use std::fs;
use std::path::PathBuf;

use serde_json::Value;
use verity::pkm::{BaseInfo, Format, Pkm};
use verity::text;

fn reports() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../corpus/reports/oracle-smoke")
}

fn arr16(v: &Value) -> Vec<u16> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_u64().unwrap() as u16)
        .collect()
}

fn n(v: &Value) -> u64 {
    match v {
        Value::Bool(b) => *b as u64,
        _ => v.as_u64().unwrap(),
    }
}

#[test]
fn fields_match_pkhex() {
    let Ok(rd) = fs::read_dir(reports()) else {
        eprintln!("skipping: no oracle reports");
        return;
    };
    let mut dumps: Vec<_> = rd
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.to_string_lossy().ends_with(".dump.json"))
        .collect();
    dumps.sort();
    let mut checked = 0;
    let mut failures = Vec::new();

    for dump_path in dumps {
        let stem = dump_path
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .trim_end_matches(".dump.json")
            .to_string();
        let dump: Value = serde_json::from_slice(&fs::read(&dump_path).unwrap()).unwrap();
        let gen5 = dump["version"].as_str().unwrap().contains(['B', 'W']);
        let (format, ext) = if gen5 {
            (Format::Pk5, "pk5")
        } else {
            (Format::Pk4, "pk4")
        };

        for s in dump["slots"].as_array().unwrap() {
            let name = s["name"].as_str().unwrap();
            let bytes = fs::read(reports().join(format!("{stem}.pkm/{name}.{ext}"))).unwrap();
            let pk = Pkm::from_decrypted(format, &bytes).unwrap();
            let info = BaseInfo {
                base: arr16(&s["base_stats"]).try_into().unwrap(),
                gender_ratio: n(&s["gender_ratio"]) as u8,
                growth: n(&s["exp_growth"]) as u8,
            };
            let mut fail = |field: &str, ours: String, theirs: String| {
                if ours != theirs {
                    failures.push(format!("{stem}/{name} {field}: ours {ours} pkhex {theirs}"));
                }
            };
            macro_rules! check {
                ($field:literal, $ours:expr) => {
                    fail(
                        $field,
                        format!("{}", $ours as u64),
                        format!("{}", n(&s[$field])),
                    )
                };
            }
            check!("pid", pk.pid());
            check!("checksum_valid", pk.checksum_valid());
            check!("species", pk.species());
            check!("form", pk.form());
            check!("held_item", pk.held_item());
            check!("tid", pk.tid());
            check!("sid", pk.sid());
            check!("exp", pk.exp());
            check!("level", pk.level(&info));
            check!("friendship", pk.friendship());
            check!("ability", pk.ability());
            check!("language", pk.language());
            check!("is_egg", pk.is_egg());
            check!("is_nicknamed", pk.is_nicknamed());
            check!("fateful", pk.fateful());
            check!("gender", pk.gender());
            check!("nature", pk.nature());
            check!("version", pk.version());
            check!("egg_location", pk.egg_location());
            check!("met_location", pk.met_location());
            check!("ball", pk.ball());
            check!("met_level", pk.met_level());
            check!("ot_gender", pk.ot_gender());
            check!("is_shiny", pk.is_shiny());
            check!("hp_type", pk.hidden_power_type());
            check!("hp_power", pk.hidden_power_power());
            fail(
                "evs",
                format!("{:?}", pk.evs().to_vec()),
                format!("{:?}", arr16(&s["evs"])),
            );
            fail(
                "ivs",
                format!("{:?}", pk.ivs().to_vec()),
                format!("{:?}", arr16(&s["ivs"])),
            );
            fail(
                "moves",
                format!("{:?}", pk.moves().to_vec()),
                format!("{:?}", arr16(&s["moves"])),
            );
            fail(
                "pp",
                format!("{:?}", pk.pp().map(u16::from).to_vec()),
                format!("{:?}", arr16(&s["pp"])),
            );
            fail(
                "pp_ups",
                format!("{:?}", pk.pp_ups().map(u16::from).to_vec()),
                format!("{:?}", arr16(&s["pp_ups"])),
            );
            fail(
                "calc_stats",
                format!("{:?}", pk.calc_stats(&info).to_vec()),
                format!("{:?}", arr16(&s["calc_stats"])),
            );
            if s["party"].as_bool().unwrap() {
                fail(
                    "stored_stats",
                    format!("{:?}", pk.stored_stats().unwrap().to_vec()),
                    format!("{:?}", arr16(&s["stored_stats"])),
                );
                fail(
                    "stored_level",
                    format!("{:?}", pk.stat_level()),
                    format!("Some({})", n(&s["stored_level"])),
                );
            }
            let decode = |raw: &[u16]| {
                let mut buf = [0u16; 16];
                let len = if gen5 {
                    text::decode5(raw, &mut buf)
                } else {
                    text::decode4(raw, &mut buf)
                };
                String::from_utf16_lossy(&buf[..len])
            };
            let nick = decode(&pk.nickname_raw());
            let ot = decode(&pk.ot_name_raw());
            fail(
                "nickname",
                nick.clone(),
                s["nickname"].as_str().unwrap().to_string(),
            );
            fail(
                "ot_name",
                ot.clone(),
                s["ot_name"].as_str().unwrap().to_string(),
            );

            let raw = pk.nickname_raw();
            let units: Vec<u16> = nick.encode_utf16().collect();
            let mut enc = [0u16; 11];
            let n = if gen5 {
                text::encode5(&units, 10, pk.language(), &mut enc, text::Clear::Zero)
            } else {
                text::encode4(&units, 10, pk.language(), &mut enc, text::Clear::Zero)
            };
            let used = raw
                .iter()
                .position(|&c| c == 0xFFFF || (gen5 && c == 0))
                .map_or(11, |p| p + 1);
            if !units.contains(&0xFFFF) && !(gen5 && raw[..used].contains(&0)) {
                fail(
                    "nickname_reencode",
                    format!("{:04X?}", &enc[..n]),
                    format!("{:04X?}", &raw[..used.min(n.max(used))]),
                );
            }
            checked += 1;
        }
    }
    for f in failures.iter().take(30) {
        eprintln!("{f}");
    }
    assert!(
        failures.is_empty(),
        "{} field mismatches over {checked} pkm",
        failures.len()
    );
    eprintln!("checked {checked} pkm");
}
