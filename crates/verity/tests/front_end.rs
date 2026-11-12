//! The entry points both front ends (DS and web) call: `legality::verdict`, `edit` and
//! `generate::wizard`, on real saves. The generators and the legality analysis themselves are
//! checked against PKHeX elsewhere (`p5_fuzz`, `p6_*`, `oracle_*`); these tests cover the glue.
//! Each test skips without the data blobs (`data/blobs`) or the corpus saves.

use std::fs;
use std::path::PathBuf;

use verity::data::{GameData, MoveData};
use verity::edit::{self, Edit, Met, Nickname};
use verity::generate::status::GenStatus;
use verity::generate::wild4::Lead;
use verity::generate::wizard::{self, EggRequest, WildRequest};
use verity::gift::catalog::{self, Databases};
use verity::gift::pcd::Gifts4;
use verity::gift::pgf::{Events5, Gifts5};
use verity::legality::analysis4::Context4;
use verity::legality::analysis5::Context5;
use verity::legality::verdict::{self, Blob, Unchecked};
use verity::pkm::Pkm;
use verity::safety::{self, Rules};
use verity::save::{Save, SaveKind, Slot};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn blob(name: &str) -> Option<&'static [u8]> {
    let bytes = fs::read(root().join("data/blobs").join(format!("{name}.bin"))).ok()?;
    Some(Box::leak(bytes.into_boxed_slice()))
}

/// Both contexts, or `None` (skip) without the blobs.
fn contexts() -> Option<(&'static Context4<'static>, &'static Context5<'static>)> {
    let get = |b: Blob| blob(b.file());
    let c4 = verdict::context4(get)?;
    let c5 = verdict::context5(get)?;
    Some((Box::leak(Box::new(c4)), Box::leak(Box::new(c5))))
}

fn save_bytes(name: &str) -> Option<Vec<u8>> {
    fs::read(root().join("corpus/real/pkhex-avalonia").join(name)).ok()
}

fn rules(kind: SaveKind) -> Rules<'static> {
    let (game, moves) = match kind {
        SaveKind::DP => ("dp", "moves4"),
        SaveKind::Pt => ("pt", "moves4"),
        SaveKind::HGSS => ("hgss", "moves4"),
        SaveKind::BW => ("bw", "moves5"),
        SaveKind::B2W2 => ("b2w2", "moves5"),
    };
    Rules {
        kind,
        game: GameData::load(blob(game).unwrap()).unwrap(),
        moves: MoveData::load(blob(moves).unwrap()).unwrap(),
    }
}

fn judge(
    save: &Save,
    pk: &Pkm,
    c4: &'static Context4<'static>,
    c5: &'static Context5<'static>,
) -> Result<bool, Unchecked> {
    verdict::check(save.generation(), pk, Some(c4), Some(c5)).map(|r| r.valid())
}

fn first_empty_box_slot(save: &Save) -> Slot {
    (0..save.box_count() as u8)
        .flat_map(|b| (0..30).map(move |s| Slot::Box(b, s)))
        .find(|&s| save.pkm(s).unwrap().species() == 0)
        .expect("an empty box slot")
}

// ---- legality::verdict -----------------------------------------------------------------------

#[test]
fn verdict_blob_lists() {
    let gen4: Vec<_> = verdict::blobs_for(4).collect();
    let gen5: Vec<_> = verdict::blobs_for(5).collect();
    assert!(gen4.contains(&Blob::EncDppt) && !gen4.contains(&Blob::EncBw));
    // Gen 5 saves judge Poké Transfer origins with the Gen 4 data too.
    assert!(gen5.contains(&Blob::EncBw) && gen5.contains(&Blob::EncDppt));
    let mut dedup = gen5.clone();
    dedup.dedup();
    assert_eq!(dedup.len(), gen5.len(), "no blob listed twice");
}

#[test]
fn verdict_judges_every_party_pokemon_of_the_corpus_saves() {
    let Some((c4, c5)) = contexts() else { return };
    for name in [
        "gen4_platinum.sav",
        "gen4_heartgold.sav",
        "gen5_black.sav",
        "gen5_white2.sav",
    ] {
        let Some(mut bytes) = save_bytes(name) else {
            continue;
        };
        let save = Save::open(&mut bytes).unwrap();
        for i in 0..save.party_count() as u8 {
            let pk = save.pkm(Slot::Party(i)).unwrap();
            assert_eq!(judge(&save, &pk, c4, c5), Ok(true), "{name} party {i}");
        }
    }
}

#[test]
fn verdict_says_why_it_did_not_judge() {
    let Some((c4, c5)) = contexts() else { return };
    let Some(mut bytes) = save_bytes("gen5_white2.sav") else {
        return;
    };
    let save = Save::open(&mut bytes).unwrap();
    let pk = save.pkm(Slot::Party(0)).unwrap();
    assert_eq!(
        verdict::check(5, &pk, None, None).err(),
        Some(Unchecked::DataMissing)
    );
    assert_eq!(
        verdict::check(3, &pk, Some(c4), Some(c5)).err(),
        Some(Unchecked::Generation)
    );
    // Colosseum/XD origins are out of scope (user decision 2026-11-09).
    let mut raw = pk.decrypted().to_vec();
    raw[0x5F] = 15; // origin game byte
    let cxd = Pkm::from_decrypted(pk.format(), &raw).unwrap();
    assert_eq!(
        verdict::check(5, &cxd, Some(c4), Some(c5)).err(),
        Some(Unchecked::Origin)
    );
    for why in [
        Unchecked::Origin,
        Unchecked::DataMissing,
        Unchecked::Generation,
        Unchecked::Unmatched,
    ] {
        assert!(!why.text().is_empty());
    }
}

// ---- edit -------------------------------------------------------------------------------------

#[test]
fn edit_applies_only_the_fields_set() {
    let Some((c4, _)) = contexts() else { return };
    let Some(mut bytes) = save_bytes("gen4_platinum.sav") else {
        return;
    };
    let save = Save::open(&mut bytes).unwrap();
    let r = rules(SaveKind::Pt);
    let before = save.pkm(Slot::Party(0)).unwrap();

    let none = edit::edited(&save, &r, Slot::Party(0), &Edit::default(), None).unwrap();
    assert_eq!(
        none.decrypted(),
        before.decrypted(),
        "an empty edit changes nothing"
    );

    let e = Edit {
        held_item: Some(234),
        friendship: Some(70),
        level: Some(50),
        evs: Some([4, 0, 0, 0, 0, 0]),
        contest: Some([1, 2, 3, 4, 5, 6]),
        met: Some(Met {
            location: before.met_location(),
            level: before.met_level(),
            date: [10, 1, 2],
        }),
        ..Edit::default()
    };
    let pk = edit::edited(&save, &r, Slot::Party(0), &e, None).unwrap();
    assert_eq!(pk.held_item(), 234);
    assert_eq!(pk.friendship(), 70);
    let info = r.game.species_form(pk.species(), pk.form()).base_info();
    assert_eq!(pk.level(&info), 50);
    assert_eq!(pk.contest(), [1, 2, 3, 4, 5, 6]);
    assert_eq!(pk.met_date(), [10, 1, 2]);
    assert_eq!(
        (pk.pid(), pk.ivs(), pk.moves()),
        (before.pid(), before.ivs(), before.moves())
    );
    assert!(pk.checksum_valid());
    // Party stats follow the new level and EVs, so the game-safe check passes.
    assert_eq!(safety::check_pkm(&r, &pk, true), Ok(()));
    let _ = c4;
}

#[test]
fn edit_unlearnable_move_is_game_safe_but_illegal() {
    let Some((c4, c5)) = contexts() else { return };
    let Some(mut bytes) = save_bytes("gen4_platinum.sav") else {
        return;
    };
    let save = Save::open(&mut bytes).unwrap();
    let r = rules(SaveKind::Pt);
    let mut moves = save.pkm(Slot::Party(0)).unwrap().moves();
    moves[0] = 147; // Spore
    let e = Edit {
        moves: Some(moves),
        ..Edit::default()
    };
    let pk = edit::edited(&save, &r, Slot::Party(0), &e, None).unwrap();
    assert_eq!(safety::check_pkm(&r, &pk, true), Ok(()));
    assert_eq!(judge(&save, &pk, c4, c5), Ok(false));
}

#[test]
fn edit_that_breaks_a_matched_origin_is_illegal() {
    let Some((c4, c5)) = contexts() else { return };
    let Some(mut bytes) = save_bytes("gen4_platinum.sav") else {
        return;
    };
    let save = Save::open(&mut bytes).unwrap();
    let r = rules(SaveKind::Pt);
    // The party Floatzel: wild, met at Lv. 11. At Lv. 5 it can't have evolved yet, so no
    // encounter matches.
    let orig = save.pkm(Slot::Party(0)).unwrap();
    let e = Edit {
        level: Some(5),
        ..Edit::default()
    };
    let pk = edit::edited(&save, &r, Slot::Party(0), &e, None).unwrap();
    assert_eq!(safety::check_pkm(&r, &pk, true), Ok(()));
    // On its own nothing matches, which reads as an origin verity doesn't model.
    assert_eq!(
        verdict::check(4, &pk, Some(c4), Some(c5)).err(),
        Some(Unchecked::Unmatched)
    );
    // As an edit of a Pokémon whose origin matched, it's illegal.
    let edited = verdict::check_edit(4, &orig, &pk, Some(c4), Some(c5)).unwrap();
    assert!(!edited.valid());
    // An edit that keeps the match is judged as usual.
    assert!(
        verdict::check_edit(4, &orig, &orig, Some(c4), Some(c5))
            .unwrap()
            .valid()
    );
}

#[test]
fn edit_nickname_set_and_reset() {
    let Some((c4, _)) = contexts() else { return };
    let Some(mut bytes) = save_bytes("gen4_platinum.sav") else {
        return;
    };
    let save = Save::open(&mut bytes).unwrap();
    let r = rules(SaveKind::Pt);
    let units: Vec<u16> = "Zappy".encode_utf16().collect();
    let set = Edit {
        nickname: Some(Nickname::new(&units)),
        ..Edit::default()
    };
    let pk = edit::edited(&save, &r, Slot::Party(0), &set, None).unwrap();
    assert!(pk.is_nicknamed());

    let reset = Edit {
        nickname: Some(Nickname::Reset),
        ..Edit::default()
    };
    // A reset needs the species names; without them there is nothing to write.
    assert!(edit::edited(&save, &r, Slot::Party(0), &reset, None).is_none());
    let pk = edit::edited(&save, &r, Slot::Party(0), &reset, Some(&c4.names4)).unwrap();
    assert!(!pk.is_nicknamed());
    assert_eq!(safety::check_pkm(&r, &pk, true), Ok(()));
}

#[test]
fn edit_refuses_empty_and_out_of_range_slots() {
    let Some(_) = contexts() else { return };
    let Some(mut bytes) = save_bytes("gen4_platinum.sav") else {
        return;
    };
    let save = Save::open(&mut bytes).unwrap();
    let r = rules(SaveKind::Pt);
    let e = Edit::default();
    assert!(edit::edited(&save, &r, Slot::Party(6), &e, None).is_none());
    assert!(edit::edited(&save, &r, first_empty_box_slot(&save), &e, None).is_none());
}

#[test]
fn ribbon_positions_cover_the_three_sets_without_overlap() {
    let mut seen = std::collections::HashSet::new();
    for i in 0..edit::RIBBONS.len() {
        let (byte, bit) = edit::ribbon_pos(i);
        assert!(byte < 12 && bit < 8);
        assert!(seen.insert((byte, bit)), "ribbon {i} reuses a bit");
    }
    assert_eq!(edit::ribbon_pos(0), (0, 0)); // Sinnoh set from byte 0
    assert_eq!(edit::ribbon_pos(28), (4, 0)); // Hoenn set from byte 4
    assert_eq!(edit::ribbon_pos(60), (8, 0)); // Sinnoh contests from byte 8
}

// ---- generate::wizard ----------------------------------------------------------------------

#[test]
fn wizard_lists_wild_species_and_slots_per_game() {
    let Some((c4, c5)) = contexts() else { return };
    for v in [10, 11, 12, 7, 8, 20, 21, 22, 23] {
        let data = wizard::wild_data(Some(c4), Some(c5), v).unwrap();
        let species = wizard::wild_species(data, v);
        assert!(species.len() > 100, "version {v}: {}", species.len());
        assert!(
            species.windows(2).all(|w| w[0] < w[1]),
            "ascending, distinct"
        );
        let slots = wizard::wild_encounters(data, v, species[0]);
        assert!(!slots.is_empty());
        assert!(slots.iter().all(|(_, e)| e.species() == species[0]));
    }
    assert!(
        wizard::wild_data(Some(c4), Some(c5), 1).is_none(),
        "Gen 3 has no wizard"
    );
    assert!(wizard::wild_data(None, Some(c5), 12).is_none());
}

fn wild(version: u8, species: u16) -> WildRequest {
    WildRequest {
        version,
        species,
        form: 0,
        encounter: None,
        level: None,
        nature: None,
        gender: None,
        shiny: false,
        lead: Lead::None,
        ball: None,
        met_date: [26, 11, 11],
        seed: 0x1234_5678,
    }
}

/// Inserts, then checks the result is legal and the whole save still passes the validator.
fn insert_and_check(name: &str, kind: SaveKind, req: &WildRequest) -> Option<Pkm> {
    let (c4, c5) = contexts()?;
    let mut bytes = save_bytes(name)?;
    let mut save = Save::open(&mut bytes).unwrap();
    let slot = first_empty_box_slot(&save);
    let (a, b) = if kind == SaveKind::B2W2 {
        (None, Some(c5))
    } else {
        (Some(c4), None)
    };
    assert_eq!(wizard::insert_wild(&mut save, a, b, req, slot), Ok(()));
    let pk = save.pkm(slot).unwrap();
    assert_eq!(pk.species(), req.species);
    assert_eq!(judge(&save, &pk, c4, c5), Ok(true));
    save.finalize();
    assert_eq!(safety::check_save(&rules(kind), &save), Ok(()));
    Some(pk)
}

#[test]
fn wizard_adds_a_legal_platinum_wild_pokemon_to_a_chosen_slot() {
    let Some((c4, _)) = contexts() else { return };
    let data = wizard::wild_data(Some(c4), None, 12).unwrap();
    let (index, slot) = wizard::wild_encounters(data, 12, 399)[0];
    let req = WildRequest {
        encounter: Some(index),
        form: slot.form(),
        ..wild(12, 399)
    };
    let Some(pk) = insert_and_check("gen4_platinum.sav", SaveKind::Pt, &req) else {
        return;
    };
    let level = pk.met_level();
    assert!((slot.level_min()..=slot.level_max()).contains(&level));
}

#[test]
fn wizard_synchronize_keeps_the_nature() {
    let req = WildRequest {
        nature: Some(10), // Timid
        lead: Lead::Synchronize(10),
        ..wild(7, 161) // Sentret, HeartGold
    };
    if let Some(pk) = insert_and_check("gen4_heartgold.sav", SaveKind::HGSS, &req) {
        assert_eq!(pk.nature(), 10);
    }
}

#[test]
fn wizard_adds_a_legal_shiny_gen5_wild_pokemon() {
    let req = WildRequest {
        shiny: true,
        ..wild(22, 504) // Patrat, White 2
    };
    if let Some(pk) = insert_and_check("gen5_white2.sav", SaveKind::B2W2, &req) {
        assert!(pk.is_shiny());
    }
}

#[test]
fn wizard_refuses_what_it_cannot_do() {
    let Some((c4, c5)) = contexts() else { return };
    let Some(mut bytes) = save_bytes("gen4_platinum.sav") else {
        return;
    };
    let mut save = Save::open(&mut bytes).unwrap();
    let slot = first_empty_box_slot(&save);
    // A Gen 5 game on a Gen 4 save, and a slot index for another species.
    assert_eq!(
        wizard::insert_wild(&mut save, Some(c4), None, &wild(22, 504), slot),
        Err(GenStatus::Unsupported)
    );
    let data = wizard::wild_data(Some(c4), None, 12).unwrap();
    let (other, _) = wizard::wild_encounters(data, 12, 396)[0]; // a Starly slot
    let wrong = WildRequest {
        encounter: Some(other),
        ..wild(12, 399)
    };
    assert!(wizard::insert_wild(&mut save, Some(c4), None, &wrong, slot).is_err());
    assert_eq!(
        save.pkm(slot).unwrap().species(),
        0,
        "nothing written on failure"
    );
    // No Gen 5 eggs yet.
    let Some(mut w2) = save_bytes("gen5_white2.sav") else {
        return;
    };
    let mut w2 = Save::open(&mut w2).unwrap();
    let egg = EggRequest {
        version: 22,
        species: 495,
        nature: None,
        gender: None,
        shiny: false,
        hatched: false,
        date: [26, 11, 11],
        seed: 1,
    };
    let s = first_empty_box_slot(&w2);
    assert_eq!(
        wizard::insert_egg(&mut w2, c4, &egg, s),
        Err(GenStatus::Unsupported)
    );
    let _ = c5;
}

#[test]
fn wizard_adds_legal_gen4_eggs_unhatched_and_hatched() {
    let Some((c4, c5)) = contexts() else { return };
    assert!(wizard::egg_species(c4).contains(&393)); // Piplup
    for hatched in [false, true] {
        let Some(mut bytes) = save_bytes("gen4_platinum.sav") else {
            return;
        };
        let mut save = Save::open(&mut bytes).unwrap();
        let slot = first_empty_box_slot(&save);
        let req = EggRequest {
            version: 12,
            species: 393,
            nature: None,
            gender: None,
            shiny: false,
            hatched,
            date: [26, 11, 11],
            seed: 7,
        };
        assert_eq!(wizard::insert_egg(&mut save, c4, &req, slot), Ok(()));
        let pk = save.pkm(slot).unwrap();
        assert_eq!(pk.is_egg(), !hatched);
        assert_eq!(judge(&save, &pk, c4, c5), Ok(true), "hatched: {hatched}");
    }
}

// ---- gift::catalog --------------------------------------------------------------------------

fn gift_databases() -> Option<Databases<'static>> {
    Some(Databases {
        gifts4: Some(Gifts4::load(blob("gifts4")?).ok()?),
        events4: Some(Gifts4::load(blob("events4")?).ok()?),
        gen5: Some((
            Gifts5::load(blob("gifts5")?).ok()?,
            Events5::load(blob("events5")?).ok()?,
        )),
    })
}

/// Lists, delivers the first ready Pokémon card, and checks the list then says it's taken.
fn deliver_first_ready(name: &str) -> Option<()> {
    let db = gift_databases()?;
    let mut bytes = save_bytes(name)?;
    let mut save = Save::open(&mut bytes).unwrap();
    let cards = catalog::list(&save, &db);
    assert!(cards.len() > 5, "{name}: {} cards", cards.len());
    assert!(cards.iter().all(|c| !c.title.is_empty()));
    let card = cards
        .iter()
        .find(|c| c.status == 0 && c.species != 0 && !c.ranger)
        .expect("a ready Pokémon card")
        .clone();
    assert!(!card.ot.is_empty(), "Pokémon cards carry their OT");
    let days = catalog::days_of(2026, 11, 11).unwrap();
    assert_eq!(
        catalog::deliver(&mut save, &db, card.source, card.record, days),
        0
    );
    let after = catalog::list(&save, &db);
    let again = after
        .iter()
        .find(|c| c.source == card.source && c.record == card.record)
        .unwrap();
    assert_ne!(again.status, 0, "{name}: delivered twice");
    save.finalize();
    assert!(save.checksums_valid());
    Some(())
}

#[test]
fn gifts_list_and_deliver_gen4() {
    deliver_first_ready("gen4_platinum.sav");
    deliver_first_ready("gen4_heartgold.sav");
}

#[test]
fn gifts_list_and_deliver_gen5() {
    deliver_first_ready("gen5_white2.sav");
}

#[test]
fn gifts_refuse_cards_from_elsewhere() {
    let (Some(db), Some(mut bytes)) = (gift_databases(), save_bytes("gen4_platinum.sav")) else {
        return;
    };
    let mut save = Save::open(&mut bytes).unwrap();
    let listed = catalog::list(&save, &db);
    let gifts4 = db.gifts4.as_ref().unwrap();
    // A Gen 4 record this save isn't offered, and a Gen 5 card on a Gen 4 save.
    let unlisted = (0..gifts4.iter().count() as u16)
        .find(|&r| {
            !listed
                .iter()
                .any(|c| c.source == catalog::Source::Gifts4 && c.record == r)
        })
        .expect("some card isn't offered to this save");
    let unavailable = 5;
    assert_eq!(
        catalog::deliver(&mut save, &db, catalog::Source::Gifts4, unlisted, 0),
        unavailable
    );
    assert_eq!(
        catalog::deliver(&mut save, &db, catalog::Source::Gen5, 0, 0),
        unavailable
    );
    assert_eq!(catalog::list(&save, &db).len(), listed.len());
}

// ---- texts -----------------------------------------------------------------------------------

#[test]
fn status_and_issue_texts_are_complete() {
    for code in 0..=20u8 {
        let c = GenStatus::c_text(code);
        assert!(c.ends_with('\0') && c.len() > 1, "code {code}");
    }
    assert_eq!(
        GenStatus::c_text(200),
        GenStatus::c_text(GenStatus::Internal as u8)
    );
    assert_eq!(GenStatus::Ok.text(), "OK");
    assert_eq!(safety::PkmIssue::Pp.text(), "PP out of range");
    assert!(safety::PkmIssue::PartyStats.c_text().ends_with('\0'));
}
