//! P6 acceptance for the D/P/Pt wild request API: random requests (all three games, Unown
//! forms, constraint mixes) either fail with a specific error or give a Pokémon that PKHeX
//! finds legal; generated Pokémon inserted into real saves dump as legal with valid checksums
//! and correct party stats.
fn root_for_g3() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

use std::fs;
use std::path::PathBuf;
use std::process::Command;

use serde_json::Value;
use verity::data::encounters::EncounterData;
use verity::data::{GameData, MoveData, Strings};
use verity::generate::api4::{self, EncounterChoice, Error, GenRequest, IvRequest};
use verity::generate::build4::Trainer4;
use verity::generate::wild4::{GenError, Lead, Rand};
use verity::legality::analysis4::Context4;
use verity::legality::{evo::Evo4, moves4::Learn4};
use verity::save::{Save, SaveKind, Slot};

fn root() -> PathBuf {
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

fn load(name: &str) -> &'static [u8] {
    Box::leak(
        fs::read(root().join("data/blobs").join(name))
            .unwrap()
            .into_boxed_slice(),
    )
}

fn ready() -> bool {
    root().join("data/blobs/enc_dppt.bin").exists()
        && root()
            .join("tools/pkhex-oracle/bin/Release/net9.0/PkhexOracle")
            .exists()
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

const BALLS: [u8; 14] = [1, 2, 3, 4, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15];
const KURT: [u8; 7] = [17, 18, 19, 20, 21, 22, 23];

fn random_request<'a>(ctx: &Context4<'a>, r: &mut Rand) -> GenRequest<'a> {
    let version = [10u8, 11, 12, 7, 8][r.next_u32() as usize % 5];
    let (group, ed): (&[u8], _) = match version {
        10 => (b"SlotsD", &ctx.enc_dppt),
        11 => (b"SlotsP", &ctx.enc_dppt),
        12 => (b"SlotsPt", &ctx.enc_dppt),
        7 => (b"SlotsHG", &ctx.enc_hgss),
        _ => (b"SlotsSS", &ctx.enc_hgss),
    };
    let slots: Vec<_> = ed.group(group).collect();
    // One Unown request in eight so the Solaceon Ruins path is well covered.
    let e = if r.next_u32().is_multiple_of(8) {
        *slots.iter().find(|e| e.species() == 201).unwrap()
    } else {
        slots[r.next_u32() as usize % slots.len()]
    };
    let species = e.species();
    let form = if species == 201 {
        (r.next_u32() % 28) as u8
    } else {
        e.form()
    };
    let pick = |r: &mut Rand, k: u32| r.next_u32().is_multiple_of(k);
    let ratio = ctx.ratio(species);
    let lead = match r.next_u32() % 5 {
        0 => Lead::Synchronize((r.next_u32() % 25) as u8),
        1 if ratio != 0 && ratio < 254 => Lead::CuteCharm {
            gender: (r.next_u32() & 1) as u8,
        },
        2 if e.can_radar() && e.slot_type() == 0 => Lead::PokeRadarChain,
        _ => Lead::None,
    };
    let [a1, a2, _] = ctx.hgss.species_form(species, form).abilities();
    GenRequest {
        version,
        species,
        form,
        encounter: if pick(r, 2) {
            EncounterChoice::Any
        } else {
            EncounterChoice::Slot(e)
        },
        level: pick(r, 4).then(|| e.level_min() + (r.next_u32() % 3) as u8),
        nature: pick(r, 3).then(|| (r.next_u32() % 25) as u8),
        gender: pick(r, 5).then(|| (r.next_u32() & 1) as u8),
        ability: pick(r, 4).then(|| if r.next_u32() & 1 == 0 { a1 } else { a2 }),
        shiny: lead == Lead::PokeRadarChain || pick(r, 6),
        lead,
        hidden_power_type: pick(r, 10).then(|| (r.next_u32() % 16) as u8),
        ivs: match r.next_u32() % 6 {
            0 => IvRequest::Exact(core::array::from_fn(|_| (r.next_u32() % 32) as u8)),
            1 => IvRequest::Nearest(core::array::from_fn(|_| (r.next_u32() % 32) as u8)),
            _ => IvRequest::Random,
        },
        ball: (verity::generate::build4::required_ball(&e) == 0 && pick(r, 2)).then(|| {
            if matches!(version, 7 | 8) && pick(r, 3) {
                KURT[r.next_u32() as usize % KURT.len()]
            } else {
                BALLS[r.next_u32() as usize % BALLS.len()]
            }
        }),
        moves: None,
        met_date: [
            9,
            1 + (r.next_u32() % 12) as u8,
            1 + (r.next_u32() % 28) as u8,
        ],
        seed: r.next_u32() as u64,
        budget: 20_000,
    }
}

fn trainer(r: &mut Rand) -> Trainer4 {
    let lang = [1u8, 2, 3, 4, 5, 7, 8][r.next_u32() as usize % 7];
    let mut name_raw = [0u16; 8];
    let units: Vec<u16> = ["Verity", "Bob", "Dawn"][r.next_u32() as usize % 3]
        .encode_utf16()
        .collect();
    verity::text::encode4(&units, 7, lang, &mut name_raw, verity::text::Clear::Zero);
    Trainer4 {
        name_raw,
        tid: r.next_u32() as u16,
        sid: r.next_u32() as u16,
        gender: (r.next_u32() & 1) as u8,
        language: lang,
    }
}

/// Returns (made, unown made, error counts).
fn random_requests(n: usize, seed: u64) -> (usize, usize, Vec<(Error, u32)>) {
    let ctx = context();
    let mut r = Rand::new(seed);
    let tmp = std::env::temp_dir().join(format!("verity-p6api-{}-{seed}", std::process::id()));
    fs::create_dir_all(&tmp).unwrap();
    let (mut made, mut unown) = (0, 0);
    let mut errors: Vec<(Error, u32)> = Vec::new();
    for i in 0..n {
        let tr = trainer(&mut r);
        let req = random_request(ctx, &mut r);
        match api4::generate_for(ctx, &tr, &req) {
            Ok(g) => {
                let pk = &g.pkm;
                assert_eq!(pk.species(), req.species);
                assert_eq!(pk.version(), req.version);
                if req.species == 201 {
                    assert_eq!(pk.form(), req.form);
                    unown += 1;
                }
                if let Some(nat) = req.nature {
                    assert_eq!(pk.pid() % 25, nat as u32);
                }
                if let Some(b) = req.ball {
                    assert_eq!(pk.ball(), b);
                }
                if let IvRequest::Exact(ivs) = req.ivs {
                    assert_eq!(pk.ivs(), ivs.map(u16::from));
                }
                assert_eq!(pk.is_shiny(), req.shiny);
                for kind in [SaveKind::DP, SaveKind::Pt, SaveKind::HGSS] {
                    let rules = api4::rules4(ctx, kind).unwrap();
                    assert_eq!(
                        verity::safety::check_pkm(&rules, pk, false),
                        Ok(()),
                        "{kind:?}"
                    );
                }
                fs::write(tmp.join(format!("{i:06}.pk4")), &pk.decrypted()[..136]).unwrap();
                made += 1;
            }
            Err(e @ Error::Illegal(_)) => panic!(
                "generator produced an illegal mon: {e:?} v{} sp{} f{} lv{:?} nat{:?} g{:?} ab{:?} shiny{} {:?} hp{:?} {:?} ball{:?} any{}",
                req.version,
                req.species,
                req.form,
                req.level,
                req.nature,
                req.gender,
                req.ability,
                req.shiny,
                req.lead,
                req.hidden_power_type,
                req.ivs,
                req.ball,
                matches!(req.encounter, EncounterChoice::Any)
            ),
            Err(e) => match errors.iter_mut().find(|(k, _)| *k == e) {
                Some((_, c)) => *c += 1,
                None => errors.push((e, 1)),
            },
        }
    }
    let out = oracle(&["legality-batch".as_ref(), tmp.as_os_str()]);
    let mut bad = Vec::new();
    for line in String::from_utf8_lossy(&out).lines() {
        let v: Value = serde_json::from_str(line).unwrap();
        if !v["valid"].as_bool().unwrap() {
            bad.push(format!(
                "{}: {}",
                v["file"],
                v["report"].as_str().unwrap_or("").replace('\n', " | ")
            ));
        }
    }
    let _ = fs::remove_dir_all(&tmp);
    assert!(
        bad.is_empty(),
        "{} PKHeX-illegal:\n{}",
        bad.len(),
        bad.join("\n")
    );
    (made, unown, errors)
}

#[test]
fn p6_api_random_requests() {
    if !ready() {
        return;
    }
    let (made, unown, errors) = random_requests(1500, 11);
    eprintln!("made {made}, unown {unown}, errors {errors:?}");
    assert!(made >= 900, "only {made} of 1500 requests succeeded");
    assert!(unown >= 50);
}

#[test]
#[ignore]
fn p6_api_random_requests_1e4() {
    let (made, unown, errors) = random_requests(16_000, 12);
    eprintln!("made {made}, unown {unown}, errors {errors:?}");
    assert!(made >= 10_000);
}

#[test]
fn p6_api_specific_errors() {
    if !ready() {
        return;
    }
    let ctx = context();
    let mut r = Rand::new(5);
    let tr = trainer(&mut r);
    let base = |species: u16| GenRequest {
        version: 12,
        species,
        form: 0,
        encounter: EncounterChoice::Any,
        level: None,
        nature: None,
        gender: None,
        ability: None,
        shiny: false,
        lead: Lead::None,
        hidden_power_type: None,
        ivs: IvRequest::Random,
        ball: None,
        moves: None,
        met_date: [9, 1, 1],
        seed: 1,
        budget: 20_000,
    };
    // Mew has no wild slot.
    assert_eq!(
        api4::generate_for(ctx, &tr, &base(151)).err(),
        Some(Error::NoEncounter)
    );
    // Bidoof (Pt Route 201 etc.) with Synchronize Adamant but Modest requested.
    let req = GenRequest {
        lead: Lead::Synchronize(3),
        nature: Some(15),
        ..base(399)
    };
    assert_eq!(
        api4::generate_for(ctx, &tr, &req).err(),
        Some(Error::Gen(GenError::NatureConflictsWithSynchronize))
    );
    // Bidoof's abilities are Simple/Unaware; Levitate is impossible.
    let req = GenRequest {
        ability: Some(26),
        ..base(399)
    };
    assert_eq!(
        api4::generate_for(ctx, &tr, &req).err(),
        Some(Error::AbilityUnavailable)
    );
    // Shiny with a Cute Charm lead needs a matching low PID: rejected specifically.
    let req = GenRequest {
        lead: Lead::CuteCharm { gender: 0 },
        shiny: true,
        ..base(399)
    };
    assert!(matches!(
        api4::generate_for(ctx, &tr, &req).err(),
        Some(Error::Gen(
            GenError::ShinyImpossibleForCuteCharm | GenError::BudgetExhausted
        ))
    ));
    // Level 100 is outside every Bidoof slot.
    let req = GenRequest {
        level: Some(100),
        ..base(399)
    };
    assert_eq!(
        api4::generate_for(ctx, &tr, &req).err(),
        Some(Error::NoEncounter)
    );
    // Master Ball is fine; Safari Ball outside the Great Marsh is not.
    let e = api4::encounters(ctx, 12, 399, 0).unwrap()[0];
    let req = GenRequest {
        ball: Some(5),
        encounter: EncounterChoice::Slot(e),
        ..base(399)
    };
    assert_eq!(
        api4::generate_for(ctx, &tr, &req).err(),
        Some(Error::BallNotAllowed)
    );
    let req = GenRequest {
        ball: Some(1),
        ..base(399)
    };
    assert_eq!(api4::generate_for(ctx, &tr, &req).unwrap().pkm.ball(), 1);
    // Unown form 28 does not exist; a slot of another species is refused.
    let req = GenRequest {
        form: 28,
        ..base(201)
    };
    assert!(api4::generate_for(ctx, &tr, &req).is_err());
    let req = GenRequest {
        encounter: EncounterChoice::Slot(e),
        ..base(396)
    };
    assert_eq!(
        api4::generate_for(ctx, &tr, &req).err(),
        Some(Error::NotInEncounter)
    );
    // Illegal moves (Bidoof with Psychic) are refused, never returned.
    let req = GenRequest {
        moves: Some([94, 0, 0, 0]),
        ..base(399)
    };
    assert_eq!(
        api4::generate_for(ctx, &tr, &req).err(),
        Some(Error::MovesIllegal)
    );
}

/// Inserts generated Pokémon into real D/P and Pt saves and checks PKHeX's view of the result.
#[test]
fn p6_api_insert_into_saves() {
    if !ready() {
        return;
    }
    let ctx = context();
    for (file, version) in [
        ("corpus/real/pkhex-avalonia/gen4_platinum.sav", 12u8),
        ("corpus/real/pkhex-avalonia/gen4_diamond.sav", 10),
    ] {
        let path = root().join(file);
        let Ok(mut data) = fs::read(&path) else {
            continue;
        };
        let mut save = Save::open(&mut data).unwrap();
        let party_before = save.party_count();
        let mut targets: Vec<Slot> = Vec::new();
        'find: for b in 0..save.box_count() as u8 {
            for s in 0..30u8 {
                if save.pkm(Slot::Box(b, s)).unwrap().species() == 0 {
                    targets.push(Slot::Box(b, s));
                    if targets.len() == 20 {
                        break 'find;
                    }
                }
            }
        }
        if party_before < 6 {
            targets.push(Slot::Party(party_before as u8));
        }
        let mut r = Rand::new(99);
        let mut inserted = Vec::new();
        for &slot in &targets {
            let req = loop {
                let mut q = random_request(ctx, &mut r);
                q.version = version;
                q.encounter = EncounterChoice::Any;
                if let Ok(g) = api4::generate(ctx, &save, &q) {
                    break g;
                }
            };
            api4::insert(ctx, &mut save, slot, &req.pkm).unwrap();
            assert!(save.dex_caught(req.pkm.species()));
            inserted.push((slot, req.pkm.species()));
        }
        // A party gap is refused.
        if save.party_count() < 5 {
            let gap = Slot::Party(save.party_count() as u8 + 1);
            let pk = save.pkm(inserted[0].0).unwrap();
            assert_eq!(
                api4::insert(ctx, &mut save, gap, &pk).err(),
                Some(Error::PartyGap)
            );
        }
        save.finalize();
        assert!(save.checksums_valid());
        let tmp =
            std::env::temp_dir().join(format!("verity-p6ins-{}-{version}", std::process::id()));
        fs::create_dir_all(&tmp).unwrap();
        let out_sav = tmp.join("out.sav");
        fs::write(&out_sav, save.data()).unwrap();
        let dump_dir = tmp.join("dump");
        let dump: Value = serde_json::from_slice(&oracle(&[
            "sav-dump".as_ref(),
            out_sav.as_os_str(),
            dump_dir.as_os_str(),
        ]))
        .unwrap();
        let slots = dump["slots"].as_array().unwrap();
        for (slot, species) in &inserted {
            let name = match slot {
                Slot::Party(i) => format!("party_{i}"),
                Slot::Box(b, s) => format!("box{b:02}_{s:02}"),
            };
            let d = slots
                .iter()
                .find(|d| d["name"] == name.as_str())
                .unwrap_or_else(|| panic!("{name} missing from dump"));
            assert_eq!(d["species"], *species, "{name}");
            assert!(d["legal"].as_bool().unwrap(), "{name} illegal per PKHeX");
            assert!(d["checksum_valid"].as_bool().unwrap(), "{name}");
            if matches!(slot, Slot::Party(_)) {
                assert_eq!(d["stored_stats"], d["calc_stats"], "{name} party stats");
            }
        }
        let info: Value =
            serde_json::from_slice(&oracle(&["sav-info".as_ref(), out_sav.as_os_str()])).unwrap();
        eprintln!("{file}: {}", info["checksum_info"]);
        let _ = fs::remove_dir_all(&tmp);
    }
}
