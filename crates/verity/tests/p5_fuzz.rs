//! P5 fuzz driver: legal generated PK4s built by PKHeX, mutated, re-checked by PKHeX.
fn root_for_g3() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

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
            version: 12,
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

pub const MUTATIONS: [&str; 20] = [
    "contest",
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
    "ribbon",
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
            *pk = Pkm::from_decrypted(pk.format(), &d).unwrap();
        }
        "ball" => {
            let mut d = pk.decrypted().to_vec();
            d[0x83] = (r.next_u32() % 26) as u8;
            *pk = Pkm::from_decrypted(pk.format(), &d).unwrap();
        }
        "met_location" => {
            let mut d = pk.decrypted().to_vec();
            d[0x80..0x82].copy_from_slice(&((r.next_u32() % 120) as u16).to_le_bytes());
            d[0x46..0x48].copy_from_slice(&0u16.to_le_bytes());
            *pk = Pkm::from_decrypted(pk.format(), &d).unwrap();
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
        "nature5" => {
            let mut d = pk.decrypted().to_vec();
            d[0x41] = (r.next_u32() % 25) as u8;
            *pk = Pkm::from_decrypted(pk.format(), &d).unwrap();
        }
        "flags5" => {
            // Hidden Ability and N's sparkle.
            let mut d = pk.decrypted().to_vec();
            d[0x42] ^= 1 << (r.next_u32() % 2);
            *pk = Pkm::from_decrypted(pk.format(), &d).unwrap();
        }
        "fame5" => {
            let mut d = pk.decrypted().to_vec();
            d[0x87] = [0, 25, 30, 50, 55, 255, 7][r.next_u32() as usize % 7];
            *pk = Pkm::from_decrypted(pk.format(), &d).unwrap();
        }
        "contest" => {
            // Fully random, Sheen alone, stats alone, or stats with a plausible Sheen near each limit.
            let mut d = pk.decrypted().to_vec();
            let mut stats: [u8; 6] = core::array::from_fn(|_| (r.next_u32() % 256) as u8);
            match r.next_u32() % 6 {
                0 => {}
                1 => stats[..5].fill(0),
                2 => stats[5] = 0,
                3 => {
                    stats[..5]
                        .iter_mut()
                        .for_each(|s| *s = (r.next_u32() % 12) as u8);
                    stats[5] = (r.next_u32() % 70) as u8;
                }
                4 => {
                    let sum: u32 = stats[..5].iter().map(|&s| s as u32).sum();
                    let delta = r.next_u32() % 9;
                    stats[5] = (sum.div_ceil(5) + delta).saturating_sub(4).min(255) as u8;
                }
                _ => {
                    stats[(r.next_u32() % 5) as usize] = 255;
                    stats[5] = (r.next_u32() % 256) as u8;
                }
            }
            d[0x1E..0x24].copy_from_slice(&stats);
            *pk = Pkm::from_decrypted(pk.format(), &d).unwrap();
        }
        "ribbon" => {
            // Any ribbon bit (PK4 ribbon bytes 0x24-0x27, 0x3C-0x3F, 0x60-0x63).
            let mut d = pk.decrypted().to_vec();
            let ofs = [
                0x24, 0x25, 0x26, 0x27, 0x3C, 0x3D, 0x3E, 0x3F, 0x60, 0x61, 0x62, 0x63,
            ][r.next_u32() as usize % 12];
            d[ofs] ^= 1 << (r.next_u32() % 8);
            *pk = Pkm::from_decrypted(pk.format(), &d).unwrap();
        }
        "version" => {
            let mut d = pk.decrypted().to_vec();
            d[0x5F] = [7, 8, 10, 11, 12][r.next_u32() as usize % 5];
            *pk = Pkm::from_decrypted(pk.format(), &d).unwrap();
        }
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
        g3: std::fs::read(root_for_g3().join("data/blobs/g3_origin.bin"))
            .ok()
            .map(|b| {
                verity::legality::analysis3::Context3::load(Box::leak(b.into_boxed_slice()))
                    .unwrap()
            }),
        gifts: verity::gift::pcd::Gifts4::load(load("gifts4.bin")).unwrap(),
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
    parity_with_muts(n, seed, mutations_per, build, &MUTATIONS)
}

/// M6: PKHeX-generated legal Gen 4 gift PK4s (Wonder Cards and Ranger Manaphy eggs).
fn build_legal_gifts(n: usize, dir: &Path, seed: u64) {
    oracle(&[
        "gen-gift4".as_ref(),
        n.to_string().as_ref(),
        seed.to_string().as_ref(),
        dir.as_os_str(),
    ]);
}

const GIFT_MUTATIONS: [&str; 21] = [
    "contest",
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
    "ribbon",
    "version",
];

fn parity_with_muts(
    n: usize,
    seed: u64,
    mutations_per: u32,
    build: fn(usize, &Path, u64),
    muts: &[&str],
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
            let kind = muts[r.next_u32() as usize % muts.len()];
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
    let mut flagged_memory = 0;
    for (f, pk) in &files {
        if !matches!(pk.version(), 1..=5 | 7 | 8 | 10..=12) {
            continue;
        }
        let v = &pkhex[f];
        // PKHeX's word filter is not ported (docs/discrepancies.md).
        if v["report"]
            .as_str()
            .unwrap_or("")
            .contains("Flagged pattern")
        {
            continue;
        }
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
        if theirs.contains("Memory") {
            flagged_memory += 1;
        }
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
                "{f} v{} lang {} sp {} ball {} loc {} ml {} tile {} enc {} ({}): ours {ours:?} {our_moves:?} pkhex {theirs:?} {their_moves:?} {}",
                pk.version(), pk.language(),
                pk.species(),
                pk.ball(),
                pk.met_location(),
                pk.met_level(),
                pk.ground_tile(),
                match report.matched {
                    verity::legality::analysis4::Matched::Slot { encounter, .. } => format!("{}/{:?}", encounter.species(), encounter.kind()),
                    verity::legality::analysis4::Matched::Egg { species } => format!("egg {species}"),
                    verity::legality::analysis4::Matched::Invalid => "invalid".into(),
                    _ => "other".into(),
                },
                v["enc_kind"].as_str().unwrap_or(""),
                v["report"].as_str().unwrap_or("").replace('\n', " | ")
            ));
        }
        checked += 1;
    }
    if muts == ["contest"] {
        eprintln!("PKHeX flagged contest stats on {flagged_memory} of {checked}");
    }
    if std::env::var_os("VERITY_KEEP_FUZZ").is_some() {
        eprintln!("kept {}", tmp.display());
    } else {
        let _ = fs::remove_dir_all(&tmp);
    }
    (checked, bad, per_id)
}

fn build_origin3(mode: &str, n: usize, dir: &Path, seed: u64) {
    oracle(&[
        "gen-slot3".as_ref(),
        n.to_string().as_ref(),
        seed.to_string().as_ref(),
        dir.as_os_str(),
        mode.as_ref(),
    ]);
}

fn build_legal_slot3(n: usize, dir: &Path, seed: u64) {
    build_origin3("slot", n, dir, seed);
}

fn build_legal_static3(n: usize, dir: &Path, seed: u64) {
    build_origin3("static", n, dir, seed);
}

fn build_legal_trade3(n: usize, dir: &Path, seed: u64) {
    build_origin3("trade", n, dir, seed);
}

fn build_legal_egg3(n: usize, dir: &Path, seed: u64) {
    build_origin3("egg", n, dir, seed);
}

fn build_legal_gift3(n: usize, dir: &Path, seed: u64) {
    build_origin3("gift", n, dir, seed);
}

fn check_origin3(build: fn(usize, &Path, u64), n: usize, seed: u64, muts: u32) {
    if !root().join("data/blobs/g3_origin.bin").exists() {
        eprintln!("skipping: build data blobs first");
        return;
    }
    let (checked, bad, per_id) = parity_with_muts(n, seed, muts, build, &MUTATIONS);
    for b in bad.iter().take(20) {
        eprintln!("{b}");
    }
    assert!(
        bad.is_empty(),
        "{} of {checked} mismatched: {per_id:?}",
        bad.len()
    );
    eprintln!("checked {checked}");
}

#[test]
fn p5_parity_fuzz_static3() {
    check_origin3(build_legal_static3, 300, 311, 3);
}

#[test]
fn p5_parity_fuzz_trade3() {
    check_origin3(build_legal_trade3, 300, 321, 3);
}

fn check_contest(build: fn(usize, &Path, u64), n: usize, seed: u64) {
    if !root().join("data/blobs/g3_origin.bin").exists() {
        eprintln!("skipping: build data blobs first");
        return;
    }
    let (checked, bad, per_id) = parity_with_muts(n, seed, 2, build, &["contest"]);
    eprintln!("seed {seed}: checked {checked}");
    for b in bad.iter().take(20) {
        eprintln!("{b}");
    }
    assert!(
        bad.is_empty(),
        "{} of {checked} mismatched: {per_id:?}",
        bad.len()
    );
}

const CONTEST_BUILDERS: [fn(usize, &Path, u64); 7] = [
    build_legal,
    build_legal_hgss,
    build_legal_gifts,
    build_legal_slot3,
    build_legal_trade3,
    build_legal_egg3,
    build_legal_gift3,
];

/// Closest-legal IVs on PKHeX-built wild catches: PKHeX must accept every proposal, and a target
/// the Pokémon already has must come straight back.
fn closest_ivs(build: fn(usize, &Path, u64), n: usize, seed: u64) -> (usize, usize) {
    let ctx = Box::leak(Box::new(context()));
    let tmp = std::env::temp_dir().join(format!("verity-iv-{}-{seed}", std::process::id()));
    let (legal, fixed) = (tmp.join("legal"), tmp.join("fixed"));
    fs::create_dir_all(&legal).unwrap();
    fs::create_dir_all(&fixed).unwrap();
    build(n, &legal, seed);
    let mut r = Rand::new(seed);
    let (mut proposed, mut refused) = (0, 0);
    for e in fs::read_dir(&legal).unwrap().flatten() {
        let mut pk = Pkm::from_decrypted(Format::Pk4, &fs::read(e.path()).unwrap()).unwrap();
        let same = pk.ivs().map(|v| v as u8);
        // Frames needing an exotic lead (a failed Cute Charm, say) aren't searched, so the
        // Pokémon's own spread isn't always offered back. When it is, nothing should change.
        if let Ok(kept) = verity::gen::api4::closest_ivs_for(ctx, &pk, same, 30_000) {
            if kept.distance == 0 {
                assert_eq!(kept.ivs, same);
            }
        }
        let target: [u8; 6] = core::array::from_fn(|_| (r.next_u32() % 32) as u8);
        let Ok(f) = verity::gen::api4::closest_ivs_for(ctx, &pk, target, 30_000) else {
            refused += 1;
            continue;
        };
        let (nature, shiny) = (pk.nature(), pk.is_shiny());
        pk.set_pid(f.pid);
        pk.set_ivs(f.ivs);
        pk.refresh_checksum();
        assert_eq!((pk.nature(), pk.is_shiny()), (nature, shiny));
        let name = format!("{proposed:05}.pk4");
        fs::write(fixed.join(name), &pk.decrypted()[..136]).unwrap();
        proposed += 1;
    }
    let out = oracle(&["legality-batch".as_ref(), fixed.as_os_str()]);
    for line in String::from_utf8_lossy(&out).lines() {
        let v: serde_json::Value = serde_json::from_str(line).unwrap();
        assert!(v["valid"].as_bool().unwrap(), "{v}");
    }
    let _ = fs::remove_dir_all(&tmp);
    (proposed, refused)
}

fn context5() -> verity::legality::analysis5::Context5<'static> {
    use verity::data::MoveData;
    use verity::legality::{analysis5::Context5, evo::Evo4};
    let bw = GameData::load(load("bw.bin")).unwrap();
    let b2w2 = GameData::load(load("b2w2.bin")).unwrap();
    Context5 {
        bw,
        b2w2,
        moves: MoveData::load(load("moves5.bin")).unwrap(),
        enc_bw: EncounterData::load(load("enc_bw.bin")).unwrap(),
        enc_b2w2: EncounterData::load(load("enc_b2w2.bin")).unwrap(),
        enc_dr: EncounterData::load(load("enc_dr.bin")).unwrap(),
        evo: Evo4::new(bw, b2w2),
        names: verity::data::Strings::load(load("names5.bin")).unwrap(),
        gifts: verity::gift::pgf::Gifts5::load(load("gifts5.bin")).unwrap(),
    }
}

const MUTATIONS5: [&str; 23] = [
    "nature5",
    "flags5",
    "fame5",
    "contest",
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
    "ribbon",
];

fn compare5(
    ctx: &'static verity::legality::analysis5::Context5<'static>,
    pk: &Pkm,
    v: &serde_json::Value,
    label: &str,
    bad: &mut Vec<String>,
    per_id: &mut BTreeMap<String, (u32, u32)>,
) {
    let report = verity::legality::analysis5::analyze5(ctx, pk);
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
    if ours != theirs || our_moves != their_moves || report.valid() != v["valid"].as_bool().unwrap()
    {
        let matched = match report.matched {
            verity::legality::analysis4::Matched::Slot { encounter, .. } => {
                format!("{}/{:?}", encounter.species(), encounter.kind())
            }
            verity::legality::analysis4::Matched::Egg { species } => format!("egg {species}"),
            verity::legality::analysis4::Matched::Invalid => "invalid".into(),
            _ => "other".into(),
        };
        bad.push(format!(
            "{label} v{} lang {} sp {} ball {} loc {} ml {} enc {matched} ({}): ours {ours:?} {our_moves:?} pkhex {theirs:?} {their_moves:?} {}",
            pk.version(),
            pk.language(),
            pk.species(),
            pk.ball(),
            pk.met_location(),
            pk.met_level(),
            v["enc_kind"].as_str().unwrap_or(""),
            v["report"].as_str().unwrap_or("").replace('\n', " | ")
        ));
    }
}

/// Kinds verity doesn't model for a Gen 5 origin yet.
fn gen5_unmodelled(kind: &str) -> bool {
    matches!(kind, "EncounterInvalid")
}

fn parity5(mode: &str, n: usize, seed: u64, muts: u32) {
    if !root().join("data/blobs/names5.bin").exists() {
        eprintln!("skipping: build data blobs first");
        return;
    }
    let ctx = Box::leak(Box::new(context5()));
    let tmp = std::env::temp_dir().join(format!("verity-p5g5-{}-{seed}", std::process::id()));
    let (legal, mutated) = (tmp.join("legal"), tmp.join("mutated"));
    fs::create_dir_all(&mutated).unwrap();
    oracle(&[
        "gen-origin5".as_ref(),
        n.to_string().as_ref(),
        seed.to_string().as_ref(),
        legal.as_os_str(),
        mode.as_ref(),
    ]);
    let mut r = Rand::new(seed ^ 0xABCD);
    let mut files = Vec::new();
    for (i, e) in fs::read_dir(&legal).unwrap().flatten().enumerate() {
        let mut pk = Pkm::from_decrypted(Format::Pk5, &fs::read(e.path()).unwrap()).unwrap();
        for _ in 0..r.next_u32() % (muts + 1) {
            let kind = MUTATIONS5[r.next_u32() as usize % MUTATIONS5.len()];
            mutate(&mut pk, kind, &mut r);
        }
        let f = format!("{i:05}.pk5");
        fs::write(mutated.join(&f), &pk.decrypted()[..136]).unwrap();
        files.push((f, pk));
    }
    let out = oracle(&["legality-batch".as_ref(), mutated.as_os_str()]);
    let mut pkhex = std::collections::HashMap::new();
    for line in String::from_utf8_lossy(&out).lines() {
        let v: serde_json::Value = serde_json::from_str(line).unwrap();
        pkhex.insert(v["file"].as_str().unwrap().to_string(), v);
    }
    let (mut checked, mut bad, mut per_id) = (0, Vec::new(), BTreeMap::new());
    for (f, pk) in &files {
        let v = &pkhex[f];
        if !matches!(pk.version(), 20..=23)
            || gen5_unmodelled(v["enc_kind"].as_str().unwrap_or(""))
            || v["report"]
                .as_str()
                .unwrap_or("")
                .contains("Flagged pattern")
        {
            continue;
        }
        compare5(ctx, pk, v, f, &mut bad, &mut per_id);
        checked += 1;
    }
    let _ = fs::remove_dir_all(&tmp);
    for b in bad.iter().take(16) {
        eprintln!("{b}");
    }
    eprintln!(
        "{mode}: checked {checked}, mismatched {}: {per_id:?}",
        bad.len()
    );
    assert!(bad.is_empty());
}

/// Random Gen 5 wild requests: every Pokémon built must be PKHeX-legal.
fn gen5_wild_legal(n: usize, seed: u64) -> usize {
    use verity::gen::build4::Trainer4;
    use verity::gen::wild5::{generate, Request5};
    let ctx = Box::leak(Box::new(context5()));
    let tmp = std::env::temp_dir().join(format!("verity-g5w-{}-{seed}", std::process::id()));
    fs::create_dir_all(&tmp).unwrap();
    let mut r = Rand::new(seed);
    let groups: [(&[u8], &EncounterData); 4] = [
        (b"SlotsW", &ctx.enc_bw),
        (b"SlotsB", &ctx.enc_bw),
        (b"SlotsW2", &ctx.enc_b2w2),
        (b"SlotsB2", &ctx.enc_b2w2),
    ];
    let lists: Vec<Vec<Encounter>> = groups
        .iter()
        .map(|(g, d)| d.group(g).filter(|e| e.species() != 0).collect())
        .collect();
    let mut made = 0;
    let mut name = [0xFFFFu16; 8];
    for (i, c) in "Verity".encode_utf16().enumerate() {
        name[i] = c;
    }
    for i in 0..n {
        let list = &lists[r.next_u32() as usize % 4];
        let encounter = list[r.next_u32() as usize % list.len()];
        let tr = Trainer4 {
            name_raw: name,
            tid: r.next_u32() as u16,
            sid: r.next_u32() as u16,
            gender: (r.next_u32() & 1) as u8,
            language: [1u8, 2, 3, 4, 5, 7, 8][r.next_u32() as usize % 7],
        };
        let q = Request5 {
            encounter,
            level: None,
            nature: (r.next_u32() & 1 == 0).then_some((r.next_u32() % 25) as u8),
            gender: None,
            shiny: !encounter.is_hidden_grotto() && r.next_u32().is_multiple_of(4),
            ball: (r.next_u32() & 1 == 0)
                .then_some([1u8, 2, 3, 4, 6, 12, 15][r.next_u32() as usize % 7]),
            date: [12, 3, 4],
            seed: r.next_u32() as u64 | (i as u64) << 32,
        };
        let pk =
            generate(ctx, &tr, &q).unwrap_or_else(|e| panic!("{e:?} sp {}", encounter.species()));
        if q.shiny {
            assert!(pk.is_shiny());
        }
        fs::write(tmp.join(format!("{made:05}.pk5")), &pk.decrypted()[..136]).unwrap();
        made += 1;
    }
    let out = oracle(&["legality-batch".as_ref(), tmp.as_os_str()]);
    let (mut bad, mut seen) = (0, 0);
    for line in String::from_utf8_lossy(&out).lines() {
        let v: serde_json::Value = serde_json::from_str(line).unwrap();
        seen += 1;
        if !v["valid"].as_bool().unwrap() {
            eprintln!("{v}");
            bad += 1;
        }
    }
    let _ = fs::remove_dir_all(&tmp);
    assert_eq!(seen, made, "PKHeX should judge every file");
    assert_eq!(bad, 0);
    made
}

#[test]
fn p6_gen5_wild_legal() {
    if root().join("data/blobs/names5.bin").exists() {
        eprintln!("built {}", gen5_wild_legal(400, 77));
    }
}

#[test]
#[ignore]
fn p6_gen5_wild_legal_1e4() {
    eprintln!("built {}", gen5_wild_legal(10_000, 78));
}

#[test]
fn p5_parity_fuzz_gen5() {
    for (i, mode) in [
        "slot",
        "static",
        "entree",
        "radar",
        "n",
        "trade",
        "egg",
        "unhatched",
        "gift",
    ]
    .iter()
    .enumerate()
    {
        parity5(mode, 150, 900 + i as u64, 3);
    }
}

#[test]
#[ignore]
fn p5_parity_gen5_1e4() {
    for (i, mode) in [
        "slot",
        "static",
        "entree",
        "radar",
        "n",
        "trade",
        "egg",
        "unhatched",
        "gift",
    ]
    .iter()
    .enumerate()
    {
        parity5(mode, 10_000, 9000 + i as u64, 4);
    }
}

#[test]
fn p5_parity_corpus_gen5() {
    if !root().join("data/blobs/names5.bin").exists() {
        return;
    }
    let Ok(rd) = fs::read_dir(root().join("corpus/reports/oracle-smoke")) else {
        return;
    };
    let ctx = Box::leak(Box::new(context5()));
    let (mut checked, mut bad, mut per_id) = (0, Vec::new(), BTreeMap::new());
    for e in rd.flatten() {
        let dir = e.path();
        if !dir.is_dir() || !dir.to_string_lossy().ends_with(".pkm") {
            continue;
        }
        let out = oracle(&["legality-batch".as_ref(), dir.as_os_str()]);
        for line in String::from_utf8_lossy(&out).lines() {
            let v: serde_json::Value = serde_json::from_str(line).unwrap();
            let f = v["file"].as_str().unwrap();
            if !f.ends_with(".pk5") || gen5_unmodelled(v["enc_kind"].as_str().unwrap_or("")) {
                continue;
            }
            let pk = Pkm::from_decrypted(Format::Pk5, &fs::read(dir.join(f)).unwrap()).unwrap();
            if !matches!(pk.version(), 20..=23) {
                continue;
            }
            compare5(ctx, &pk, &v, f, &mut bad, &mut per_id);
            checked += 1;
        }
    }
    for b in bad.iter().take(16) {
        eprintln!("{b}");
    }
    eprintln!(
        "corpus checked {checked}, mismatched {}: {per_id:?}",
        bad.len()
    );
    assert!(checked > 0);
    assert!(bad.is_empty());
}

#[test]
fn p6_closest_ivs_legal() {
    if !root().join("data/blobs/enc_dppt.bin").exists() {
        return;
    }
    let pt = closest_ivs(build_legal, 100, 41);
    let hgss = closest_ivs(build_legal_hgss, 100, 42);
    eprintln!("proposed/refused: Pt {pt:?}, HG/SS {hgss:?}");
    assert!(pt.0 > 0 && hgss.0 > 0);
}

#[test]
fn p5_parity_fuzz_contest() {
    for (i, b) in CONTEST_BUILDERS.iter().enumerate() {
        check_contest(*b, 200, 500 + i as u64);
    }
}

#[test]
#[ignore]
fn p5_parity_contest_1e4() {
    for (i, b) in CONTEST_BUILDERS.iter().enumerate() {
        check_contest(*b, 10_000, 5000 + i as u64);
    }
}

#[test]
fn p5_parity_fuzz_gift3() {
    check_origin3(build_legal_gift3, 300, 341, 3);
}

#[test]
fn p5_parity_fuzz_egg3() {
    check_origin3(build_legal_egg3, 300, 331, 3);
}

#[test]
#[ignore]
fn p5_parity_origin3_1e4() {
    for (b, seed) in [
        (build_legal_slot3 as fn(usize, &Path, u64), 3001),
        (build_legal_static3, 3002),
        (build_legal_egg3, 3004),
        (build_legal_gift3, 3005),
    ] {
        check_origin3(b, 10_000, seed, 4);
    }
}

#[test]
fn p5_parity_fuzz_slot3() {
    if !root().join("data/blobs/g3_origin.bin").exists() {
        eprintln!("skipping: build data blobs first");
        return;
    }
    let (checked, bad, per_id) = parity_with_muts(600, 301, 3, build_legal_slot3, &MUTATIONS);
    for b in bad.iter().take(20) {
        eprintln!("{b}");
    }
    assert!(
        bad.is_empty(),
        "{} of {checked} mismatched: {per_id:?}",
        bad.len()
    );
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

fn build_legal_eggs(n: usize, dir: &Path, seed: u64) {
    oracle(&[
        "gen-egg4".as_ref(),
        n.to_string().as_ref(),
        seed.to_string().as_ref(),
        dir.as_os_str(),
    ]);
}

fn build_legal_ranch(n: usize, dir: &Path, seed: u64) {
    oracle(&[
        "gen-ranch4".as_ref(),
        n.to_string().as_ref(),
        seed.to_string().as_ref(),
        dir.as_os_str(),
    ]);
}

#[test]
fn p5_parity_fuzz_ranch() {
    if !root().join("data/blobs/gifts4.bin").exists() {
        eprintln!("skipping: build data blobs first");
        return;
    }
    let (checked, bad, per_id) = parity_with_muts(600, 81, 3, build_legal_ranch, &GIFT_MUTATIONS);
    eprintln!("ranch checked {checked}");
    for b in bad.iter().take(15) {
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
fn p5_parity_ranch_1e4() {
    let (checked, bad, per_id) =
        parity_with_muts(10_000, 8181, 4, build_legal_ranch, &GIFT_MUTATIONS);
    for b in bad.iter().take(10) {
        eprintln!("{b}");
    }
    eprintln!("ranch checked {checked}");
    assert!(
        bad.is_empty(),
        "{} of {checked} mismatched: {per_id:?}",
        bad.len()
    );
}

#[test]
fn p5_parity_fuzz_unhatched_eggs() {
    if !root().join("data/blobs/gifts4.bin").exists() {
        eprintln!("skipping: build data blobs first");
        return;
    }
    let (checked, bad, per_id) = parity_with_muts(600, 71, 3, build_legal_eggs, &GIFT_MUTATIONS);
    eprintln!("eggs checked {checked}");
    for b in bad.iter().take(15) {
        eprintln!("{b}");
    }
    assert!(checked > 0);
    assert!(
        bad.is_empty(),
        "{} of {checked} mismatched: {per_id:?}",
        bad.len()
    );
}

/// M4 scale run: `cargo test --release -- --ignored p5_parity_eggs_1e4`.
#[test]
#[ignore]
fn p5_parity_eggs_1e4() {
    let (checked, bad, per_id) =
        parity_with_muts(10_000, 7171, 4, build_legal_eggs, &GIFT_MUTATIONS);
    for b in bad.iter().take(10) {
        eprintln!("{b}");
    }
    eprintln!("eggs checked {checked}");
    assert!(
        bad.is_empty(),
        "{} of {checked} mismatched: {per_id:?}",
        bad.len()
    );
}

#[test]
fn p5_parity_fuzz_gifts() {
    if !root().join("data/blobs/gifts4.bin").exists() {
        eprintln!("skipping: build data blobs first");
        return;
    }
    let (checked, bad, per_id) = parity_with_muts(600, 61, 3, build_legal_gifts, &GIFT_MUTATIONS);
    eprintln!("gifts checked {checked}");
    for b in bad.iter().take(15) {
        eprintln!("{b}");
    }
    assert!(checked > 0);
    assert!(
        bad.is_empty(),
        "{} of {checked} mismatched: {per_id:?}",
        bad.len()
    );
}

/// M6 scale run: `cargo test --release -- --ignored p5_parity_gifts_1e4`.
#[test]
#[ignore]
fn p5_parity_gifts_1e4() {
    let (checked, bad, per_id) =
        parity_with_muts(10_000, 6161, 4, build_legal_gifts, &GIFT_MUTATIONS);
    for b in bad.iter().take(10) {
        eprintln!("{b}");
    }
    eprintln!("gifts checked {checked}");
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
                        | "EncounterTrade4RanchGift"
                        | "EncounterSlot3"
                        | "EncounterSlot3Swarm"
                        | "EncounterStatic3"
                        | "EncounterTrade3"
                        | "EncounterEgg3"
                        | "EncounterGift3"
                        | "EncounterGift3NY"
                        | "EncounterGift3JPN"
                        | "EncounterGift3Colo"
                        | "PCD"
                        | "PGT"
                )
            {
                continue;
            }
            let pk = Pkm::from_decrypted(Format::Pk4, &fs::read(dir.join(f)).unwrap()).unwrap();
            if !matches!(pk.version(), 1..=5 | 7 | 8 | 10..=12) {
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
            // Gifts: the same Wonder Card (PKHeX MGDB index) or the Ranger Manaphy.
            let kind_ok = match v["enc_kind"].as_str().unwrap() {
                "PCD" => matches!(r.matched, verity::legality::analysis4::Matched::Gift { gift }
                    if gift.mgdb_index() as i64 == v["enc_index"].as_i64().unwrap()),
                "PGT" => matches!(
                    r.matched,
                    verity::legality::analysis4::Matched::RangerManaphy
                ),
                _ => !matches!(
                    r.matched,
                    verity::legality::analysis4::Matched::Gift { .. }
                        | verity::legality::analysis4::Matched::RangerManaphy
                ),
            };
            if !kind_ok {
                bad.push(format!(
                    "{}/{f}: encounter kind/index differs",
                    dir.display()
                ));
            }
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
            version: 12,
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
