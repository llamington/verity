//! P6 acceptance for bred eggs: random requests in every Gen 4 game either fail with a specific
//! error or give an egg PKHeX finds legal, hatched or not.
fn root_for_g3() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

use std::fs;
use std::path::PathBuf;
use std::process::Command;

use serde_json::Value;
use verity::data::encounters::EncounterData;
use verity::data::{GameData, MoveData, Strings};
use verity::generate::api4::{self, Error, IvRequest};
use verity::generate::build4::Trainer4;
use verity::generate::egg4::{EggRequest, generate};
use verity::generate::wild4::{GenError, Rand};
use verity::legality::analysis4::Context4;
use verity::legality::{evo::Evo4, moves4::Learn4};
use verity::save::{Save, SaveKind, Slot};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn oracle_path() -> PathBuf {
    root().join("tools/pkhex-oracle/bin/Release/net9.0/PkhexOracle")
}

fn load(name: &str) -> &'static [u8] {
    Box::leak(
        fs::read(root().join("data/blobs").join(name))
            .unwrap()
            .into_boxed_slice(),
    )
}

fn ready() -> bool {
    root().join("data/blobs/enc_dppt.bin").exists() && oracle_path().exists()
}

fn context() -> &'static Context4<'static> {
    let hgss = GameData::load(load("hgss.bin")).unwrap();
    let pt = GameData::load(load("pt.bin")).unwrap();
    let dp = GameData::load(load("dp.bin")).unwrap();
    Box::leak(Box::new(Context4 {
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
        names4: Strings::load(load("names4.bin")).unwrap(),
        g3: std::fs::read(root_for_g3().join("data/blobs/g3_origin.bin"))
            .ok()
            .map(|b| {
                verity::legality::analysis3::Context3::load(Box::leak(b.into_boxed_slice()))
                    .unwrap()
            }),
        gifts: verity::gift::pcd::Gifts4::load(load("gifts4.bin")).unwrap(),
    }))
}

fn trainer(r: &mut Rand) -> Trainer4 {
    let mut name_raw = [0xFFFFu16; 8];
    name_raw[..3].copy_from_slice(&[0x0135, 0x0136, 0x0137]);
    Trainer4 {
        name_raw,
        tid: r.next_u32() as u16,
        sid: r.next_u32() as u16,
        gender: (r.next_u32() & 1) as u8,
        language: [1u8, 2, 3, 4, 5, 7, 8][r.next_u32() as usize % 7],
    }
}

fn request(ctx: &Context4, r: &mut Rand, seed: u64) -> EggRequest {
    let version = [10u8, 11, 12, 7, 8][r.next_u32() as usize % 5];
    let species = 1 + (r.next_u32() % 490) as u16;
    let idx = species as usize;
    let table = verity::legality::breed4::game(&ctx.learn, version);
    let egg_table = if matches!(version, 7 | 8) {
        &ctx.learn.hgss
    } else {
        &ctx.learn.dp
    };
    let moves = (r.next_u32().is_multiple_of(2)).then(|| {
        let (mut m, n) = verity::legality::breed4::base_egg_moves(table, idx, 1);
        let egg: Vec<u16> = egg_table.egg_moves(idx).iter().collect();
        let mut at = n.min(3);
        if !egg.is_empty() {
            let pick = egg[r.next_u32() as usize % egg.len()];
            if !m.contains(&pick) {
                m[at] = pick;
                at += 1;
            }
        }
        m[at.min(4)..].fill(0);
        m
    });
    let pi = ctx.hgss.species_form(species, 0);
    let [a1, a2, _] = pi.abilities();
    EggRequest {
        version,
        species,
        nature: (r.next_u32().is_multiple_of(3)).then(|| (r.next_u32() % 25) as u8),
        gender: (r.next_u32().is_multiple_of(4)).then(|| (r.next_u32() % 2) as u8),
        ability: (r.next_u32().is_multiple_of(4))
            .then(|| if r.next_u32() & 1 == 0 { a1 } else { a2 }),
        shiny: r.next_u32().is_multiple_of(5),
        ivs: if r.next_u32().is_multiple_of(3) {
            IvRequest::Exact(core::array::from_fn(|_| (r.next_u32() % 32) as u8))
        } else {
            IvRequest::Random
        },
        moves,
        hatched: r.next_u32() & 1 == 0,
        hatch_location: None,
        egg_date: [10, 4, 15],
        met_date: [10, 4, 16],
        seed,
        budget: 20_000,
    }
}

fn random_requests(n: usize, seed: u64) -> (usize, usize) {
    let ctx = context();
    let tmp = std::env::temp_dir().join(format!("verity-p6egg-{}-{seed}", std::process::id()));
    fs::create_dir_all(&tmp).unwrap();
    let mut r = Rand::new(seed);
    let (mut made, mut hatched) = (0, 0);
    let mut info = std::collections::BTreeMap::new();
    let mut refused = std::collections::BTreeMap::new();
    for i in 0..n {
        let tr = trainer(&mut r);
        let req = request(ctx, &mut r, seed ^ i as u64);
        match generate(ctx, &tr, &req) {
            Ok(pk) => {
                assert_eq!(pk.is_egg(), !req.hatched);
                assert_eq!(pk.is_shiny(), req.shiny);
                for kind in [SaveKind::DP, SaveKind::Pt, SaveKind::HGSS] {
                    let rules = api4::rules4(ctx, kind).unwrap();
                    assert_eq!(
                        verity::safety::check_pkm(&rules, &pk, false),
                        Ok(()),
                        "{kind:?}"
                    );
                }
                let name = format!("{i:06}.pk4");
                info.insert(
                    name.clone(),
                    format!("v{} sp{} hatched {}", req.version, req.species, req.hatched),
                );
                fs::write(tmp.join(name), &pk.decrypted()[..136]).unwrap();
                made += 1;
                hatched += req.hatched as usize;
            }
            Err(e @ Error::Illegal(_)) => panic!(
                "illegal egg: {e:?} v{} sp{} {:?}",
                req.version, req.species, req.moves
            ),
            Err(e) => *refused.entry(format!("{e:?}")).or_insert(0) += 1,
        }
    }
    eprintln!("made {made} (hatched {hatched}), refused {refused:?}");
    let out = Command::new(oracle_path())
        .env(
            "DOTNET_ROOT",
            format!("{}/.dotnet", std::env::var("HOME").unwrap()),
        )
        .arg("legality-batch")
        .arg(&tmp)
        .output()
        .unwrap();
    assert!(out.status.success());
    let bad: Vec<String> = String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(|l| serde_json::from_str::<Value>(l).unwrap())
        .filter(|v| !v["valid"].as_bool().unwrap())
        .map(|v| {
            format!(
                "{} {}: {}",
                v["file"],
                info[v["file"].as_str().unwrap()],
                v["report"].as_str().unwrap_or("").replace('\n', " | ")
            )
        })
        .collect();
    let _ = fs::remove_dir_all(&tmp);
    assert!(
        bad.is_empty(),
        "{} PKHeX-illegal:\n{}",
        bad.len(),
        bad.iter().take(15).cloned().collect::<Vec<_>>().join("\n")
    );
    (made, hatched)
}

#[test]
fn p6_eggs_random_requests() {
    if !ready() {
        return;
    }
    let (made, hatched) = random_requests(1200, 21);
    assert!(made >= 300, "only {made} made");
    assert!(hatched > 0 && hatched < made, "hatched {hatched} of {made}");
}

#[test]
#[ignore]
fn p6_eggs_random_requests_1e4() {
    let (made, _) = random_requests(30_000, 22);
    assert!(made >= 10_000, "only {made} made");
}

#[test]
fn p6_eggs_refuse_what_cannot_be_bred() {
    if !ready() {
        return;
    }
    let ctx = context();
    let tr = trainer(&mut Rand::new(1));
    let base = EggRequest {
        version: 12,
        species: 172,
        nature: None,
        gender: None,
        ability: None,
        shiny: false,
        ivs: IvRequest::Random,
        moves: None,
        hatched: false,
        hatch_location: None,
        egg_date: [10, 4, 15],
        met_date: [10, 4, 16],
        seed: 5,
        budget: 20_000,
    };
    assert!(generate(ctx, &tr, &base).is_ok());
    // Mewtwo has no egg; Pikachu's egg is Pichu.
    for species in [150, 25] {
        let req = EggRequest { species, ..base };
        assert_eq!(
            generate(ctx, &tr, &req).err(),
            Some(Error::NoEncounter),
            "{species}"
        );
    }
    // Surf is not something a Pichu can inherit.
    let req = EggRequest {
        moves: Some([57, 0, 0, 0]),
        ..base
    };
    assert_eq!(generate(ctx, &tr, &req).err(), Some(Error::MovesIllegal));
    let req = EggRequest {
        ability: Some(1),
        ..base
    };
    assert_eq!(
        generate(ctx, &tr, &req).err(),
        Some(Error::AbilityUnavailable)
    );
    // Magnemite is genderless.
    let req = EggRequest {
        species: 81,
        gender: Some(0),
        ..base
    };
    assert_eq!(
        generate(ctx, &tr, &req).err(),
        Some(Error::Gen(GenError::GenderImpossible))
    );
}

#[test]
fn p6_eggs_insert_into_real_saves() {
    let path = root().join("corpus/real/pkmds-blazor/Pokemon Platinum.sav");
    if !ready() || !path.exists() {
        return;
    }
    let ctx = context();
    let mut data = fs::read(path).unwrap();
    let mut save = Save::open(&mut data).unwrap();
    let t = save.trainer();
    let tr = Trainer4 {
        name_raw: t.name_raw,
        tid: t.tid,
        sid: t.sid,
        gender: t.gender,
        language: 2,
    };
    let req = EggRequest {
        version: 12,
        species: 172,
        nature: Some(3),
        gender: None,
        ability: None,
        shiny: true,
        ivs: IvRequest::Random,
        moves: Some([84, 204, 344, 0]),
        hatched: false,
        hatch_location: None,
        egg_date: [10, 4, 15],
        met_date: [10, 4, 16],
        seed: 9,
        budget: 200_000,
    };
    let pk = generate(ctx, &tr, &req).unwrap();
    assert_eq!(pk.moves()[2], 344, "Volt Tackle stays last");
    let slot = (0..18)
        .flat_map(|b| (0..30).map(move |s| Slot::Box(b, s)))
        .find(|&s| save.pkm(s).is_ok_and(|p| p.species() == 0))
        .expect("free box slot");
    api4::insert(ctx, &mut save, slot, &pk).unwrap();
    let back = save.pkm(slot).unwrap();
    assert!(back.is_egg());
    assert!(verity::legality::analysis4::analyze4(ctx, &back).valid());
}
