//! The web editor's logic (`Editor`), natively: request parsing, the nickname and date rules,
//! preview/apply, adding Pokémon and saving. The page's JS wrapper (`Session`) only converts
//! types. Tests skip without the data blobs (`data/blobs`) or the corpus saves.

use std::fs;
use std::path::PathBuf;

use verity_wasm::{EditRequest, Editor, EggAdd, Invalid, MetRequest, WildAdd, attach_blob};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Attaches every blob the editor reads (each test runs on its own thread, so its own store).
fn attach_all(game: &str) -> Option<()> {
    let dir = root().join("data/blobs");
    attach_blob("strings", fs::read(dir.join("strings_en.bin")).ok()?).unwrap();
    for name in [
        game,
        "dp",
        "pt",
        "hgss",
        "bw",
        "b2w2",
        "moves4",
        "moves5",
        "enc_dppt",
        "enc_hgss",
        "enc_bw",
        "enc_b2w2",
        "enc_dr",
        "names4",
        "names5",
        "gifts4",
        "gifts5",
        "g3_origin",
    ] {
        attach_blob(name, fs::read(dir.join(format!("{name}.bin"))).ok()?).unwrap();
    }
    Some(())
}

fn open(file: &str, game: &str) -> Option<Editor> {
    attach_all(game)?;
    let bytes = fs::read(root().join("corpus/real/pkhex-avalonia").join(file)).ok()?;
    Some(Editor::new(bytes).unwrap())
}

fn err(why: &str) -> Invalid {
    Invalid(why.into())
}

fn first_empty_box_slot(ed: &mut Editor) -> (i32, u8) {
    for b in 0..ed.overview().unwrap().box_count as u8 {
        let view = ed.box_view(b).unwrap();
        if let Some(s) = view.slots.iter().position(Option::is_none) {
            return (i32::from(b), s as u8);
        }
    }
    panic!("no empty box slot")
}

// ---- blobs and opening -------------------------------------------------------------------------

#[test]
fn attach_checks_what_it_can_and_keeps_the_first() {
    assert!(attach_blob("strings", vec![0; 16]).is_err());
    assert!(attach_blob("pt", vec![0; 16]).is_err());
    // Other blobs are checked when a context is built from them.
    assert!(attach_blob("enc_dppt", vec![1, 2, 3]).is_ok());
    assert!(
        attach_blob("enc_dppt", vec![]).is_ok(),
        "a second attach is ignored"
    );
}

#[test]
fn junk_is_not_a_save() {
    assert!(Editor::new(vec![0; 1024]).is_err());
    assert!(Editor::new(vec![0xFF; 512 * 1024]).is_err());
}

// ---- read side ---------------------------------------------------------------------------------

#[test]
fn summary_names_everything_and_keeps_the_stored_stat_order() {
    let Some(mut ed) = open("gen4_platinum.sav", "pt") else {
        return;
    };
    let s = ed.summary(-1, 0).unwrap();
    assert!(s.mon.species_name.is_some() && s.nature.name.is_some());
    assert!(s.ability.name.is_some() && s.met_location.name.is_some());
    assert_eq!((s.ivs.len(), s.evs.len()), (6, 6));
    assert_eq!(s.stats.as_ref().map(Vec::len), Some(6));
    assert_eq!(s.ribbons.len(), 80);
    assert_eq!(
        s.ribbon_count as usize,
        s.ribbons.iter().filter(|r| **r).count()
    );
    assert!(s.moves.iter().all(|m| m.id == 0 || m.move_type.is_some()));
    assert!(ed.summary(-1, 6).is_err());
    let (b, slot) = first_empty_box_slot(&mut ed);
    assert_eq!(ed.summary(b, slot).err(), Some(err("no Pokémon there")));
}

#[test]
fn choices_match_the_game() {
    let Some(mut ed) = open("gen4_heartgold.sav", "hgss") else {
        return;
    };
    let c = ed.choices().unwrap();
    assert_eq!(c.held_items[0].id, 0, "none first");
    assert!(c.held_items.len() > 100 && c.moves.len() > 400);
    assert_eq!(c.balls.len(), 24, "Gen 4 has the HG/SS balls");
    assert_eq!((c.natures.len(), c.ribbons.len()), (25, 80));
    assert!(
        c.locations
            .iter()
            .any(|l| l.name.as_deref() == Some("New Bark Town"))
    );
    assert_eq!(
        ed.wizard_versions()
            .unwrap()
            .iter()
            .map(|v| v.id)
            .collect::<Vec<_>>(),
        [7, 8]
    );
}

// ---- edit requests -----------------------------------------------------------------------------

#[test]
fn an_empty_request_previews_the_pokemon_unchanged() {
    let Some(mut ed) = open("gen4_platinum.sav", "pt") else {
        return;
    };
    let before = ed.summary(-1, 0).unwrap();
    let p = ed.preview(-1, 0, &EditRequest::default()).unwrap();
    assert_eq!(p.issue, None);
    assert_eq!(p.legality.status, "legal");
    assert_eq!(
        (p.summary.pid, p.summary.ivs.clone()),
        (before.pid, before.ivs)
    );
}

#[test]
fn malformed_requests_are_refused_with_a_reason() {
    let Some(mut ed) = open("gen4_platinum.sav", "pt") else {
        return;
    };
    let mut bad = |e: EditRequest| ed.preview(-1, 0, &e).err().map(|e| e.0);
    let r = |f: fn(&mut EditRequest)| {
        let mut e = EditRequest::default();
        f(&mut e);
        e
    };
    assert_eq!(
        bad(r(|e| e.moves = Some(vec![1, 2, 3]))),
        Some("moves: 4 values expected".into())
    );
    assert_eq!(
        bad(r(|e| e.ivs = Some(vec![31; 5]))),
        Some("IVs: 6 values expected".into())
    );
    let date = "met date: YYYY-MM-DD between 2000 and 2099 expected";
    for d in ["2026-13-01", "1999-01-01", "2026-01", "soon", "2026-02-32"] {
        let e = EditRequest {
            met: Some(MetRequest {
                location: 1,
                level: 5,
                date: d.into(),
            }),
            ..EditRequest::default()
        };
        assert_eq!(bad(e).as_deref(), Some(date), "{d}");
    }
}

#[test]
fn nickname_rules() {
    let Some(mut ed) = open("gen4_platinum.sav", "pt") else {
        return;
    };
    let mut try_name = |n: &str| {
        let e = EditRequest {
            nickname: Some(n.into()),
            ..EditRequest::default()
        };
        ed.preview(-1, 0, &e).map(|p| p.summary.mon.nickname)
    };
    assert_eq!(try_name("Zappy"), Ok("Zappy".into()));
    assert_eq!(try_name("ABCDEFGHIJ").map(|n| n.len()), Ok(10));
    assert_eq!(
        try_name("ABCDEFGHIJK"),
        Err(err("nickname: at most 10 characters"))
    );
    assert_eq!(try_name(""), Err(err("nickname: empty")));
    assert_eq!(
        try_name("Zap€"),
        Err(err("nickname: Gen 4 can't store “€”"))
    );
    // Gen 5 stores UTF-16, so the same character is fine there.
    if let Some(mut w2) = open("gen5_white2.sav", "b2w2") {
        let e = EditRequest {
            nickname: Some("Zap€".into()),
            ..EditRequest::default()
        };
        assert!(w2.preview(-1, 0, &e).is_ok());
    }
}

#[test]
fn ribbon_flags_round_trip() {
    let Some(mut ed) = open("gen4_platinum.sav", "pt") else {
        return;
    };
    let mut flags = ed.summary(-1, 0).unwrap().ribbons;
    for i in [0, 17, 28, 59, 60, 79] {
        flags[i] = !flags[i];
    }
    let e = EditRequest {
        ribbons: Some(flags.clone()),
        ..EditRequest::default()
    };
    assert_eq!(ed.preview(-1, 0, &e).unwrap().summary.ribbons, flags);
}

#[test]
fn legal_mode_refuses_an_illegal_edit_and_game_safe_mode_writes_it() {
    let Some(mut ed) = open("gen4_platinum.sav", "pt") else {
        return;
    };
    let mut moves: Vec<u16> = ed
        .summary(-1, 0)
        .unwrap()
        .moves
        .iter()
        .map(|m| m.id)
        .collect();
    moves[0] = 147; // Spore
    let e = EditRequest {
        moves: Some(moves),
        ..EditRequest::default()
    };
    let p = ed.preview(-1, 0, &e).unwrap();
    assert_eq!((p.issue, p.legality.status.as_str()), (None, "illegal"));
    assert_eq!(p.legality.moves[0].how, "can't be learnt");

    assert_eq!(
        ed.apply(-1, 0, &e, true),
        Ok(Some("illegal (legal mode)".into()))
    );
    assert!(!ed.dirty());
    assert_eq!(ed.apply(-1, 0, &e, false), Ok(None));
    assert!(ed.dirty());
    assert_eq!(ed.summary(-1, 0).unwrap().moves[0].id, 147);

    let bytes = ed.finalize().unwrap();
    let mut reopened = Editor::new(bytes).unwrap();
    assert!(reopened.overview().unwrap().checksums_ok);
    assert_eq!(reopened.summary(-1, 0).unwrap().moves[0].id, 147);
}

#[test]
fn closest_ivs_needs_six_values_and_a_gen4_wild_catch() {
    let Some(mut ed) = open("gen4_platinum.sav", "pt") else {
        return;
    };
    assert_eq!(
        ed.closest_ivs(-1, 0, vec![31; 5]).err(),
        Some(err("six IVs expected"))
    );
    let fix = ed.closest_ivs(-1, 0, vec![31; 6]).unwrap();
    let e = EditRequest {
        ivs: Some(fix.ivs.clone()),
        pid: Some(fix.pid),
        ..EditRequest::default()
    };
    assert_eq!(ed.preview(-1, 0, &e).unwrap().legality.status, "legal");
}

// ---- adding --------------------------------------------------------------------------------------

fn wild(version: u8, species: u16) -> WildAdd {
    WildAdd {
        version,
        species,
        form: 0,
        encounter: None,
        level: None,
        nature: None,
        gender: None,
        shiny: false,
        lead: "none".into(),
        ball: None,
        met_date: "2026-11-11".into(),
        seed: 42,
    }
}

#[test]
fn add_wild_needs_what_the_lead_keeps() {
    let Some(mut ed) = open("gen4_heartgold.sav", "hgss") else {
        return;
    };
    let (b, s) = first_empty_box_slot(&mut ed);
    let mut r = wild(7, 161);
    r.lead = "synchronize".into();
    assert_eq!(
        ed.add_wild(b, s, &r),
        Ok(Some("Synchronize: choose a nature".into()))
    );
    r.lead = "cuteCharm".into();
    assert_eq!(
        ed.add_wild(b, s, &r),
        Ok(Some("Cute Charm: choose a gender".into()))
    );
    r.lead = "synchronize".into();
    r.nature = Some(10);
    assert_eq!(ed.add_wild(b, s, &r), Ok(None));
    assert_eq!(
        ed.summary(b, s).unwrap().nature.name.as_deref(),
        Some("Timid")
    );
    assert!(ed.dirty());
}

#[test]
fn add_refuses_full_and_occupied_slots() {
    let Some(mut ed) = open("gen4_platinum.sav", "pt") else {
        return;
    };
    assert_eq!(
        ed.add_wild(-1, 0, &wild(12, 399)).err(),
        Some(err("the party is full"))
    );
    assert_eq!(
        ed.add_wild(0, 0, &wild(12, 399)).err(),
        Some(err("that slot isn't empty"))
    );
    let (b, s) = first_empty_box_slot(&mut ed);
    let mut bad_date = wild(12, 399);
    bad_date.met_date = "tomorrow".into();
    assert!(ed.add_wild(b, s, &bad_date).is_err());
    assert!(!ed.dirty());
}

#[test]
fn eggs_are_gen4_only() {
    let egg = |version| EggAdd {
        version,
        species: 393,
        nature: None,
        gender: None,
        shiny: false,
        hatched: false,
        date: "2026-11-11".into(),
        seed: 9,
    };
    if let Some(mut pt) = open("gen4_platinum.sav", "pt") {
        assert!(!pt.egg_species().unwrap().is_empty());
        let (b, s) = first_empty_box_slot(&mut pt);
        assert_eq!(pt.add_egg(b, s, &egg(12)), Ok(None));
        assert!(pt.summary(b, s).unwrap().mon.egg);
    }
    if let Some(mut w2) = open("gen5_white2.sav", "b2w2") {
        assert!(w2.egg_species().unwrap().is_empty());
        let (b, s) = first_empty_box_slot(&mut w2);
        assert!(w2.add_egg(b, s, &egg(22)).unwrap().is_some());
    }
}

// ---- saving --------------------------------------------------------------------------------------

#[test]
fn finalize_refuses_a_save_the_validator_rejects() {
    let path = root().join("sd/saves/Test-Save-White-2.sav");
    let (Some(()), Ok(bytes)) = (attach_all("b2w2"), fs::read(path)) else {
        return;
    };
    let mut ed = Editor::new(bytes).unwrap();
    let why = ed.finalize().unwrap_err().0;
    assert!(why.starts_with("not saved: met location (box "), "{why}");
}
