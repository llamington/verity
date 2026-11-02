//! P5 fuzz driver: legal generated PK4s built by PKHeX, mutated, re-checked by PKHeX.
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use verity::data::encounters::{Encounter, EncounterData};
use verity::data::GameData;
use verity::gen::wild4::{generate, Lead, Rand, Request};
use verity::pkm::{Format, Pkm};
use verity::rng::method_j::Slot4;

pub fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn oracle(args: &[&std::ffi::OsStr]) -> Vec<u8> {
    let out = Command::new(root().join("tools/pkhex-oracle/bin/Release/net9.0/PkhexOracle"))
        .env(
            "DOTNET_ROOT",
            format!("{}/.dotnet", std::env::var("HOME").unwrap()),
        )
        .args(args)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    out.stdout
}

/// Writes `n` PKHeX-built legal Pt grass PK4s to `dir`.
pub fn build_legal(n: usize, dir: &Path, seed: u64) {
    let b = root().join("data/blobs");
    let enc = fs::read(b.join("enc_dppt.bin")).unwrap();
    let pt = fs::read(b.join("pt.bin")).unwrap();
    let ed = EncounterData::load(&enc).unwrap();
    let gd = GameData::load(&pt).unwrap();
    let ratio = |s: u16| gd.personal(s as usize).gender_ratio();
    let slots: Vec<Encounter> = ed
        .group(b"SlotsPt")
        .filter(|e| e.slot_type() == 0 && e.species() != 201)
        .collect();
    let mut rand = Rand::new(seed);
    let mut out = Vec::new();
    while out.len() < n {
        let e = slots[rand.next_u32() as usize % slots.len()];
        let (tid, sid) = (rand.next_u32() as u16, rand.next_u32() as u16);
        let shiny = rand.next_u32().is_multiple_of(4);
        let lead = if rand.next_u32().is_multiple_of(3) {
            Lead::Synchronize((rand.next_u32() % 25) as u8)
        } else {
            Lead::None
        };
        let req = Request {
            slot: Slot4::from_encounter(&e).unwrap(),
            tid,
            sid,
            species: e.species(),
            gender_ratio: ratio(e.species()),
            met_level: None,
            nature: None,
            gender: None,
            ability_bit: None,
            shiny,
            lead,
            hidden_power_type: None,
            form: 0,
            location: e.location(),
        };
        let Ok(g) = generate(&req, &mut rand, 50_000, ratio) else {
            continue;
        };
        let a = e.area().unwrap();
        out.push(serde_json::json!({
            "group": "SlotsPt", "location": e.location(), "slot_type": e.slot_type(),
            "slot_number": e.slot_number(), "species": e.species(), "form": e.form(),
            "level_min": e.level_min(), "level_max": e.level_max(),
            "ground": a.ground_tile(), "rate": a.rate(),
            "tid": tid, "sid": sid, "pid": g.pid, "iv32": g.iv32, "level": g.level,
        }));
    }
    let req = dir.join("requests.json");
    fs::create_dir_all(dir).unwrap();
    fs::write(&req, serde_json::to_vec(&out).unwrap()).unwrap();
    oracle(&["check-wild4".as_ref(), req.as_os_str(), dir.as_os_str()]);
    fs::remove_file(req).unwrap();
}

pub const MUTATIONS: [&str; 18] = [
    "ivs",
    "evs",
    "level_exp",
    "met_level",
    "ball",
    "met_location",
    "pid",
    "gender",
    "ability",
    "nickname",
    "ot_name",
    "held_item",
    "moves",
    "language",
    "friendship",
    "form",
    "fateful",
    "tid",
];

/// Applies mutation `kind` with randomness `r`.
pub fn mutate(pk: &mut Pkm, kind: &str, r: &mut Rand) {
    match kind {
        "ivs" => pk.set_ivs(core::array::from_fn(|_| (r.next_u32() % 32) as u8)),
        "evs" => pk.set_evs(core::array::from_fn(|_| (r.next_u32() % 256) as u8)),
        "level_exp" => pk.set_exp(r.next_u32() % 2_000_000),
        "met_level" => {
            let mut d = pk.decrypted().to_vec();
            d[0x84] = (d[0x84] & 0x80) | (r.next_u32() % 101) as u8;
            *pk = Pkm::from_decrypted(Format::Pk4, &d).unwrap();
        }
        "ball" => {
            let mut d = pk.decrypted().to_vec();
            d[0x83] = (r.next_u32() % 26) as u8;
            *pk = Pkm::from_decrypted(Format::Pk4, &d).unwrap();
        }
        "met_location" => {
            let mut d = pk.decrypted().to_vec();
            d[0x80..0x82].copy_from_slice(&((r.next_u32() % 120) as u16).to_le_bytes());
            d[0x46..0x48].copy_from_slice(&0u16.to_le_bytes());
            *pk = Pkm::from_decrypted(Format::Pk4, &d).unwrap();
        }
        "pid" => pk.set_pid(r.next_u32()),
        "gender" => {
            let g = (r.next_u32() % 3) as u8;
            pk.set_fateful_gender_form(pk.fateful(), g, pk.form());
        }
        "ability" => pk.set_ability((r.next_u32() % 124) as u8),
        "nickname" => {
            let names = ["Bob", "ABCDEFGHIJ", "!!", "Pika"];
            let n: Vec<u16> = names[r.next_u32() as usize % names.len()]
                .encode_utf16()
                .collect();
            pk.set_is_nicknamed(r.next_u32() & 1 == 0);
            pk.set_nickname(&n);
        }
        "ot_name" => {
            let mut raw = pk.ot_name_raw();
            raw[(r.next_u32() % 7) as usize] = (r.next_u32() % 0x1F0) as u16;
            pk.set_ot_name_raw(&raw);
        }
        "held_item" => pk.set_held_item((r.next_u32() % 540) as u16),
        "moves" => {
            let mut m = pk.moves();
            m[(r.next_u32() % 4) as usize] = (r.next_u32() % 468) as u16;
            pk.set_moves(m);
        }
        "language" => pk.set_language((r.next_u32() % 9) as u8),
        "friendship" => pk.set_friendship(r.next_u32() as u8),
        "form" => pk.set_fateful_gender_form(pk.fateful(), pk.gender(), (r.next_u32() % 4) as u8),
        "fateful" => pk.set_fateful_gender_form(!pk.fateful(), pk.gender(), pk.form()),
        "tid" => pk.set_ids(r.next_u32() as u16, pk.sid()),
        _ => unreachable!(),
    }
    pk.refresh_checksum();
}

#[test]
#[ignore]
fn explore_pkhex_identifiers() {
    let tmp = std::env::temp_dir().join(format!("verity-p5-{}", std::process::id()));
    let legal = tmp.join("legal");
    build_legal(400, &legal, 11);
    let mutated = tmp.join("mutated");
    fs::create_dir_all(&mutated).unwrap();
    let mut r = Rand::new(5);
    let mut kinds = Vec::new();
    for (i, e) in fs::read_dir(&legal).unwrap().flatten().enumerate() {
        let mut pk = Pkm::from_decrypted(Format::Pk4, &fs::read(e.path()).unwrap()).unwrap();
        let kind = MUTATIONS[i % MUTATIONS.len()];
        mutate(&mut pk, kind, &mut r);
        let f = format!("{i:05}_{kind}.pk4");
        fs::write(mutated.join(&f), &pk.decrypted()[..136]).unwrap();
        kinds.push(f);
    }
    let out = oracle(&["legality-batch".as_ref(), mutated.as_os_str()]);
    let mut hist: BTreeMap<String, BTreeMap<String, u32>> = BTreeMap::new();
    for line in String::from_utf8_lossy(&out).lines() {
        let v: serde_json::Value = serde_json::from_str(line).unwrap();
        let f = v["file"].as_str().unwrap();
        let kind = f
            .split('_')
            .nth(1)
            .unwrap()
            .trim_end_matches(".pk4")
            .to_string();
        let mut ids: Vec<String> = v["invalid"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().to_string())
            .collect();
        if v["move_valid"]
            .as_array()
            .unwrap()
            .iter()
            .any(|x| !x.as_bool().unwrap())
        {
            ids.push("CurrentMove".into());
        }
        let key = if ids.is_empty() {
            "LEGAL".to_string()
        } else {
            ids.join("+")
        };
        *hist.entry(kind).or_default().entry(key).or_default() += 1;
    }
    for (k, v) in hist {
        eprintln!("{k}: {v:?}");
    }
    let _ = fs::remove_dir_all(&tmp);
}

fn load(name: &str) -> &'static [u8] {
    Box::leak(
        fs::read(root().join("data/blobs").join(name))
            .unwrap()
            .into_boxed_slice(),
    )
}

fn context() -> verity::legality::analysis4::Context4<'static> {
    use verity::data::MoveData;
    use verity::legality::{analysis4::Context4, evo::Evo4, moves4::Learn4};
    let hgss = GameData::load(load("hgss.bin")).unwrap();
    let pt = GameData::load(load("pt.bin")).unwrap();
    let dp = GameData::load(load("dp.bin")).unwrap();
    Context4 {
        hgss,
        learn: Learn4 {
            hgss,
            pt,
            dp,
            moves: MoveData::load(load("moves4.bin")).unwrap(),
        },
        evo: Evo4::new(dp, hgss),
        enc_dppt: EncounterData::load(load("enc_dppt.bin")).unwrap(),
        enc_hgss: EncounterData::load(load("enc_hgss.bin")).unwrap(),
        gen5: GameData::load(load("b2w2.bin")).unwrap(),
        names4: verity::data::Strings::load(load("names4.bin")).unwrap(),
    }
}

/// Compares our analysis with PKHeX's on mutated generated mons; returns (checked, mismatches).
fn parity(
    n: usize,
    seed: u64,
    mutations_per: u32,
) -> (usize, Vec<String>, BTreeMap<String, (u32, u32)>) {
    parity_with(n, seed, mutations_per, build_legal)
}

/// PKHeX-generated legal HG/SS wild PK4s (half HeartGold, half SoulSilver slots).
fn build_legal_hgss(n: usize, dir: &Path, seed: u64) {
    for (group, part) in [("SlotsHG", 0), ("SlotsSS", 1)] {
        let sub = dir.join(group);
        oracle(&[
            "gen-wild4".as_ref(),
            "Encounters4HGSS".as_ref(),
            group.as_ref(),
            (n / 2).to_string().as_ref(),
            (seed + part).to_string().as_ref(),
            sub.as_os_str(),
        ]);
        for e in fs::read_dir(&sub).unwrap().flatten() {
            let name = format!("{group}-{}", e.file_name().to_string_lossy());
            fs::rename(e.path(), dir.join(name)).unwrap();
        }
        fs::remove_dir(&sub).unwrap();
    }
}

fn parity_with(
    n: usize,
    seed: u64,
    mutations_per: u32,
    build: fn(usize, &Path, u64),
) -> (usize, Vec<String>, BTreeMap<String, (u32, u32)>) {
    let ctx = Box::leak(Box::new(context()));
    let tmp = std::env::temp_dir().join(format!("verity-p5p-{}-{seed}", std::process::id()));
    let legal = tmp.join("legal");
    fs::create_dir_all(&legal).unwrap();
    build(n, &legal, seed);
    let mutated = tmp.join("mutated");
    fs::create_dir_all(&mutated).unwrap();
    let mut r = Rand::new(seed ^ 0xABCD);
    let mut files = Vec::new();
    for (i, e) in fs::read_dir(&legal).unwrap().flatten().enumerate() {
        let mut pk = Pkm::from_decrypted(Format::Pk4, &fs::read(e.path()).unwrap()).unwrap();
        let k = r.next_u32() % (mutations_per + 1);
        for _ in 0..k {
            let kind = MUTATIONS[r.next_u32() as usize % MUTATIONS.len()];
            mutate(&mut pk, kind, &mut r);
        }
        let f = format!("{i:05}.pk4");
        fs::write(mutated.join(&f), &pk.decrypted()[..136]).unwrap();
        files.push((f, pk));
    }
    let out = oracle(&["legality-batch".as_ref(), mutated.as_os_str()]);
    let mut pkhex = std::collections::HashMap::new();
    for line in String::from_utf8_lossy(&out).lines() {
        let v: serde_json::Value = serde_json::from_str(line).unwrap();
        pkhex.insert(v["file"].as_str().unwrap().to_string(), v);
    }
    let mut bad = Vec::new();
    let mut per_id: BTreeMap<String, (u32, u32)> = BTreeMap::new();
    let mut checked = 0;
    for (f, pk) in &files {
        if !matches!(pk.version(), 7 | 8 | 10..=12) {
            continue;
        }
        let v = &pkhex[f];
        let report = verity::legality::analysis4::analyze4(ctx, pk);
        let ours: std::collections::BTreeSet<String> = report
            .invalid
            .iter()
            .map(|c| c.name().to_string())
            .collect();
        let theirs: std::collections::BTreeSet<String> = v["invalid"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().to_string())
            .collect();
        let our_moves: Vec<bool> = report.moves.iter().map(|m| m.valid()).collect();
        let their_moves: Vec<bool> = v["move_valid"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_bool().unwrap())
            .collect();
        for id in theirs.difference(&ours) {
            per_id.entry(id.clone()).or_default().0 += 1;
        }
        for id in ours.difference(&theirs) {
            per_id.entry(id.clone()).or_default().1 += 1;
        }
        if our_moves != their_moves {
            per_id.entry("moves".into()).or_default().0 += 1;
        }
        if ours != theirs
            || our_moves != their_moves
            || report.valid() != v["valid"].as_bool().unwrap()
        {
            bad.push(format!(
                "{f} sp {} ball {} loc {} ml {} tile {}: ours {ours:?} {our_moves:?} pkhex {theirs:?} {their_moves:?} {}",
                pk.species(),
                pk.ball(),
                pk.met_location(),
                pk.met_level(),
                pk.ground_tile(),
                v["report"].as_str().unwrap_or("").replace('\n', " | ")
            ));
        }
        checked += 1;
    }
    let _ = fs::remove_dir_all(&tmp);
    (checked, bad, per_id)
}

#[test]
#[ignore]
fn p5_parity_report() {
    let (checked, bad, per_id) = parity(600, 21, 2);
    for b in bad.iter().take(12) {
        eprintln!("{b}");
    }
    eprintln!(
        "checked {checked}, mismatched {}; per identifier (missing, extra): {per_id:?}",
        bad.len()
    );
}

#[test]
fn p5_parity_fuzz() {
    if !root().join("data/blobs/enc_dppt.bin").exists() {
        eprintln!("skipping: build data blobs first");
        return;
    }
    let (checked, bad, per_id) = parity(600, 77, 3);
    for b in bad.iter().take(10) {
        eprintln!("{b}");
    }
    assert!(
        bad.is_empty(),
        "{} of {checked} mismatched: {per_id:?}",
        bad.len()
    );
}

#[test]
fn p5_parity_fuzz_hgss() {
    if !root().join("data/blobs/enc_hgss.bin").exists() {
        eprintln!("skipping: build data blobs first");
        return;
    }
    let (checked, bad, per_id) = parity_with(600, 91, 3, build_legal_hgss);
    eprintln!("hgss checked {checked}");
    for b in bad.iter().take(10) {
        eprintln!("{b}");
    }
    assert!(checked > 0);
    assert!(
        bad.is_empty(),
        "{} of {checked} mismatched: {per_id:?}",
        bad.len()
    );
}

#[test]
#[ignore]
fn p5_parity_hgss_1e4() {
    let (checked, bad, per_id) = parity_with(10_000, 4242, 4, build_legal_hgss);
    for b in bad.iter().take(10) {
        eprintln!("{b}");
    }
    eprintln!("hgss checked {checked}");
    assert!(
        bad.is_empty(),
        "{} of {checked} mismatched: {per_id:?}",
        bad.len()
    );
}

/// PLAN P5 acceptance scale: >= 10^5 fuzzed. `cargo test --release -- --ignored p5_parity_1e5`.
#[test]
#[ignore]
fn p5_parity_1e5() {
    let mut total = 0;
    for seed in 0..10u64 {
        let (checked, bad, per_id) = parity(10_000, 1000 + seed, 4);
        for b in bad.iter().take(10) {
            eprintln!("{b}");
        }
        assert!(
            bad.is_empty(),
            "seed {seed}: {} of {checked} mismatched: {per_id:?}",
            bad.len()
        );
        total += checked;
    }
    eprintln!("checked {total}");
}

#[test]
fn p5_parity_corpus() {
    if !root().join("data/blobs/enc_dppt.bin").exists() {
        return;
    }
    let reports = root().join("corpus/reports/oracle-smoke");
    let Ok(rd) = fs::read_dir(&reports) else {
        return;
    };
    let ctx = Box::leak(Box::new(context()));
    let (mut checked, mut bad) = (0, Vec::new());
    for e in rd.flatten() {
        let dir = e.path();
        if !dir.is_dir() || !dir.to_string_lossy().ends_with(".pkm") {
            continue;
        }
        let out = oracle(&["legality-batch".as_ref(), dir.as_os_str()]);
        for line in String::from_utf8_lossy(&out).lines() {
            let v: serde_json::Value = serde_json::from_str(line).unwrap();
            let f = v["file"].as_str().unwrap();
            if !f.ends_with(".pk4")
                || !matches!(
                    v["enc_kind"].as_str().unwrap(),
                    "EncounterSlot4"
                        | "EncounterInvalid"
                        | "EncounterStatic4"
                        | "EncounterEgg4"
                        | "EncounterStatic4Pokewalker"
                        | "EncounterTrade4PID"
                )
            {
                continue;
            }
            let pk = Pkm::from_decrypted(Format::Pk4, &fs::read(dir.join(f)).unwrap()).unwrap();
            if !matches!(pk.version(), 7 | 8 | 10..=12) {
                continue;
            }
            // Gift eggs (static encounters with an egg location) are not modelled yet.
            if v["enc_kind"] == "EncounterStatic4" && pk.egg_location() != 0 {
                continue;
            }
            let r = verity::legality::analysis4::analyze4(ctx, &pk);
            let ours: std::collections::BTreeSet<String> =
                r.invalid.iter().map(|c| c.name().to_string()).collect();
            let theirs: std::collections::BTreeSet<String> = v["invalid"]
                .as_array()
                .unwrap()
                .iter()
                .map(|x| x.as_str().unwrap().to_string())
                .collect();
            let mv: Vec<bool> = r.moves.iter().map(|m| m.valid()).collect();
            let tmv: Vec<bool> = v["move_valid"]
                .as_array()
                .unwrap()
                .iter()
                .map(|x| x.as_bool().unwrap())
                .collect();
            if ours != theirs || mv != tmv || r.valid() != v["valid"].as_bool().unwrap() {
                bad.push(format!(
                    "{}/{f}: ours {ours:?} pkhex {theirs:?} {}",
                    dir.display(),
                    v["report"].as_str().unwrap_or("").replace('\n', " | ")
                ));
            }
            checked += 1;
        }
    }
    for b in bad.iter().take(10) {
        eprintln!("{b}");
    }
    eprintln!("corpus checked {checked}");
    assert!(checked > 0);
    assert!(bad.is_empty(), "{} of {checked} mismatched", bad.len());
}

/// P6: Pokémon built entirely by verity (generator + builder) are legal per verity and PKHeX.
fn built_legal(n: usize, seed: u64) -> (usize, usize) {
    use verity::gen::build4::{build_wild4, BuildData, Trainer4};
    let ctx = Box::leak(Box::new(context()));
    let ed = ctx.enc_dppt;
    let pt = GameData::load(load("pt.bin")).unwrap();
    let bd = BuildData {
        hgss: ctx.hgss,
        learn: pt,
        moves: ctx.learn.moves,
        names4: ctx.names4,
    };
    let ratio = |s: u16| ctx.ratio(s);
    let slots: Vec<Encounter> = ed
        .group(b"SlotsPt")
        .filter(|e| e.species() != 201)
        .collect();
    let mut r = Rand::new(seed);
    let tmp = std::env::temp_dir().join(format!("verity-built-{}-{seed}", std::process::id()));
    fs::create_dir_all(&tmp).unwrap();
    let mut ours_valid = 0;
    let mut made = 0;
    let names: [&str; 3] = ["Verity", "Bob", "Lucas"];
    while made < n {
        let e = slots[r.next_u32() as usize % slots.len()];
        let lang = [1u8, 2, 3, 4, 5, 7, 8][r.next_u32() as usize % 7];
        let mut name_raw = [0u16; 8];
        let units: Vec<u16> = names[r.next_u32() as usize % 3].encode_utf16().collect();
        verity::text::encode4(&units, 7, lang, &mut name_raw, verity::text::Clear::Zero);
        let tr = Trainer4 {
            name_raw,
            tid: r.next_u32() as u16,
            sid: r.next_u32() as u16,
            gender: (r.next_u32() & 1) as u8,
            language: lang,
        };
        let lead = match r.next_u32() % 4 {
            0 => Lead::Synchronize((r.next_u32() % 25) as u8),
            1 => Lead::CuteCharm {
                gender: (r.next_u32() & 1) as u8,
            },
            _ => Lead::None,
        };
        let req = Request {
            slot: Slot4::from_encounter(&e).unwrap(),
            tid: tr.tid,
            sid: tr.sid,
            species: e.species(),
            gender_ratio: ratio(e.species()),
            met_level: None,
            nature: None,
            gender: None,
            ability_bit: None,
            shiny: r.next_u32().is_multiple_of(4) && !matches!(lead, Lead::CuteCharm { .. }),
            lead,
            hidden_power_type: None,
            form: 0,
            location: e.location(),
        };
        let g = match generate(&req, &mut r, 50_000, ratio) {
            Ok(g) => g,
            Err(
                verity::gen::wild4::GenError::MunchlaxTreeUnavailable
                | verity::gen::wild4::GenError::CuteCharmNeedsGenderedSpecies,
            ) => continue,
            Err(err) => panic!("{err:?} for {req:?}"),
        };
        let pk = build_wild4(
            &bd,
            &e,
            &g,
            &tr,
            [
                9,
                1 + (r.next_u32() % 12) as u8,
                1 + (r.next_u32() % 28) as u8,
            ],
        );
        let rep = verity::legality::analysis4::analyze4(ctx, &pk);
        if rep.valid() {
            ours_valid += 1;
        } else {
            eprintln!(
                "ours invalid: {:?} {:?} sp {}",
                rep.invalid,
                rep.moves,
                pk.species()
            );
        }
        fs::write(tmp.join(format!("{made:06}.pk4")), &pk.decrypted()[..136]).unwrap();
        made += 1;
    }
    let out = oracle(&["legality-batch".as_ref(), tmp.as_os_str()]);
    let mut pkhex_valid = 0;
    for line in String::from_utf8_lossy(&out).lines() {
        let v: serde_json::Value = serde_json::from_str(line).unwrap();
        if v["valid"].as_bool().unwrap() {
            pkhex_valid += 1;
        } else {
            eprintln!(
                "pkhex invalid {}: {}",
                v["file"],
                v["report"].as_str().unwrap_or("").replace('\n', " | ")
            );
        }
    }
    let _ = fs::remove_dir_all(&tmp);
    assert_eq!(ours_valid, n, "verity flagged its own output");
    (n, pkhex_valid)
}

#[test]
fn p6_built_mons_legal() {
    if !root().join("data/blobs/enc_dppt.bin").exists() {
        return;
    }
    let (n, pkhex) = built_legal(500, 3);
    assert_eq!(pkhex, n);
}

#[test]
#[ignore]
fn p6_built_mons_legal_1e4() {
    let (n, pkhex) = built_legal(10_000, 4);
    assert_eq!(pkhex, n);
}
