//! P7: the game-safe validator accepts every real save and rejects the curated crash classes.
use std::fs;
use std::path::{Path, PathBuf};

use verity::data::{GameData, MoveData};
use verity::pkm::{Format, Pkm};
use verity::safety::{
    check_pkm, check_save, checked_set_pkm, PkmIssue, Rules, SaveIssue, WriteError,
};
use verity::save::{Save, SaveKind, Slot};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn load(name: &str) -> &'static [u8] {
    Box::leak(
        fs::read(root().join("data/blobs").join(name))
            .unwrap()
            .into_boxed_slice(),
    )
}

fn rules(kind: SaveKind) -> Rules<'static> {
    let (game, moves) = match kind {
        SaveKind::DP => ("dp.bin", "moves4.bin"),
        SaveKind::Pt => ("pt.bin", "moves4.bin"),
        SaveKind::HGSS => ("hgss.bin", "moves4.bin"),
        SaveKind::BW => ("bw.bin", "moves5.bin"),
        SaveKind::B2W2 => ("b2w2.bin", "moves5.bin"),
    };
    Rules {
        kind,
        game: GameData::load(load(game)).unwrap(),
        moves: MoveData::load(load(moves)).unwrap(),
    }
}

fn real_saves() -> Vec<PathBuf> {
    fn walk(p: &Path, out: &mut Vec<PathBuf>) {
        let Ok(rd) = fs::read_dir(p) else { return };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, out);
            } else if matches!(p.extension().and_then(|x| x.to_str()), Some("sav" | "dsv")) {
                out.push(p);
            }
        }
    }
    let mut v = Vec::new();
    walk(&root().join("corpus/real"), &mut v);
    v.sort();
    v
}

fn ready() -> bool {
    root().join("data/blobs/pt.bin").exists()
}

/// Slots in PKMDS's editor test save that a real game cannot produce: Gen 4 met locations and
/// Gen 4 PP kept in PK5s (Poké Transfer sets met location 30001 and heals PP; PKHeX
/// `PK4.ConvertToPK5`). The validator must keep rejecting these.
const EDITOR_ARTIFACTS: [(&str, Slot, PkmIssue); 7] = [
    (
        "Test-Save-White-2.sav",
        Slot::Box(4, 24),
        PkmIssue::MetLocation,
    ),
    (
        "Test-Save-White-2.sav",
        Slot::Box(8, 29),
        PkmIssue::MetLocation,
    ),
    (
        "Test-Save-White-2.sav",
        Slot::Box(9, 0),
        PkmIssue::MetLocation,
    ),
    (
        "Test-Save-White-2.sav",
        Slot::Box(13, 17),
        PkmIssue::MetLocation,
    ),
    (
        "Test-Save-White-2.sav",
        Slot::Box(13, 18),
        PkmIssue::MetLocation,
    ),
    ("Test-Save-White-2.sav", Slot::Box(13, 19), PkmIssue::Pp),
    ("Test-Save-White-2.sav", Slot::Box(15, 28), PkmIssue::Pp),
];

#[test]
fn editor_artifacts_still_rejected() {
    if !ready() {
        return;
    }
    let path = root().join("corpus/real/pkmds-blazor/Test-Save-White-2.sav");
    let Ok(mut data) = fs::read(&path) else {
        return;
    };
    let save = Save::open(&mut data).unwrap();
    let r = rules(save.kind());
    for (_, slot, want) in EDITOR_ARTIFACTS {
        assert_eq!(
            check_pkm(&r, &save.pkm(slot).unwrap(), false),
            Err(want),
            "{slot:?}"
        );
    }
}

#[test]
fn real_saves_pass() {
    if !ready() {
        return;
    }
    let mut checked = 0;
    let mut bad = Vec::new();
    for p in real_saves() {
        let mut data = fs::read(&p).unwrap();
        let Ok(save) = Save::open(&mut data) else {
            continue;
        };
        let r = rules(save.kind());
        // Every individual slot, so one problem does not hide the rest.
        let mut slots: Vec<Slot> = (0..save.party_count() as u8).map(Slot::Party).collect();
        for b in 0..save.box_count() as u8 {
            for s in 0..30 {
                slots.push(Slot::Box(b, s));
            }
        }
        for slot in slots {
            let pk = save.pkm(slot).unwrap();
            if pk.species() == 0 {
                continue;
            }
            checked += 1;
            let name = p.file_name().unwrap().to_string_lossy();
            if EDITOR_ARTIFACTS
                .iter()
                .any(|(f, s, _)| *f == name && *s == slot)
            {
                continue;
            }
            if let Err(e) = check_pkm(&r, &pk, matches!(slot, Slot::Party(_))) {
                bad.push(format!(
                    "{}: {slot:?} sp {} f {} {e:?}",
                    p.display(),
                    pk.species(),
                    pk.form()
                ));
            }
        }
        if let Err(e) = check_save(&r, &save) {
            if !matches!(e, SaveIssue::Pkm(..)) {
                bad.push(format!("{}: {e:?}", p.display()));
            }
        }
    }
    eprintln!("checked {checked} Pokémon");
    assert!(
        bad.is_empty(),
        "{} rejected:\n{}",
        bad.len(),
        bad.join("\n")
    );
}

/// A valid party and box Pokémon from a real save of each generation.
fn samples(kind_file: &str) -> Option<(SaveKind, Pkm, Pkm)> {
    let mut data = fs::read(root().join(kind_file)).ok()?;
    let save = Save::open(&mut data).unwrap();
    let party = save.pkm(Slot::Party(0)).unwrap();
    let boxed = (0..save.box_count() as u8)
        .flat_map(|b| (0..30u8).map(move |s| Slot::Box(b, s)))
        .map(|s| save.pkm(s).unwrap())
        .find(|p| p.species() != 0 && !p.is_egg())?;
    Some((save.kind(), party, boxed))
}

fn raw_edit(pk: &Pkm, f: impl FnOnce(&mut Vec<u8>)) -> Pkm {
    let mut d = pk.decrypted().to_vec();
    f(&mut d);
    let mut p = Pkm::from_decrypted(pk.format(), &d).unwrap();
    p.refresh_checksum();
    p
}

#[test]
fn curated_crash_classes_rejected() {
    if !ready() {
        return;
    }
    for file in [
        "corpus/real/pkhex-avalonia/gen4_platinum.sav",
        "corpus/real/pkhex-avalonia/gen5_white2.sav",
    ] {
        let Some((kind, party, boxed)) = samples(file) else {
            continue;
        };
        let r = rules(kind);
        let gen4 = boxed.format() == Format::Pk4;
        assert_eq!(check_pkm(&r, &boxed, false), Ok(()), "{file} sample");
        assert_eq!(check_pkm(&r, &party, true), Ok(()), "{file} party sample");
        let max_species = if gen4 { 493 } else { 649 };
        let max_move = if gen4 { 467 } else { 559 };
        let cases: Vec<(&str, Pkm, bool, PkmIssue)> = vec![
            (
                "checksum",
                {
                    let mut d = boxed.decrypted().to_vec();
                    d[0x06] ^= 1;
                    Pkm::from_decrypted(boxed.format(), &d).unwrap()
                },
                false,
                PkmIssue::ChecksumMismatch,
            ),
            (
                "sanity",
                raw_edit(&boxed, |d| d[0x04] = 4),
                false,
                PkmIssue::SanityFlags,
            ),
            (
                "glitch species",
                raw_edit(&boxed, |d| {
                    d[0x08..0x0A].copy_from_slice(&(max_species + 1u16).to_le_bytes())
                }),
                false,
                PkmIssue::Species,
            ),
            (
                "species 0",
                raw_edit(&boxed, |d| d[0x08..0x0A].fill(0)),
                false,
                PkmIssue::Species,
            ),
            (
                "form",
                raw_edit(&boxed, |d| d[0x40] |= 0xF8),
                false,
                PkmIssue::Form,
            ),
            (
                "ability 0",
                raw_edit(&boxed, |d| d[0x15] = 0),
                false,
                PkmIssue::Ability,
            ),
            (
                "ability 255",
                raw_edit(&boxed, |d| d[0x15] = 255),
                false,
                PkmIssue::Ability,
            ),
            (
                "held item",
                raw_edit(&boxed, |d| {
                    d[0x0A..0x0C].copy_from_slice(&999u16.to_le_bytes())
                }),
                false,
                PkmIssue::HeldItem,
            ),
            (
                "move id",
                raw_edit(&boxed, |d| {
                    d[0x28..0x2A].copy_from_slice(&(max_move + 1u16).to_le_bytes())
                }),
                false,
                PkmIssue::Move,
            ),
            (
                "no moves",
                raw_edit(&boxed, |d| {
                    d[0x28..0x30].fill(0);
                    d[0x30..0x38].fill(0)
                }),
                false,
                PkmIssue::NoMoves,
            ),
            (
                "duplicate move",
                raw_edit(&boxed, |d| {
                    let m = [d[0x28], d[0x29]];
                    d[0x2A..0x2C].copy_from_slice(&m);
                    d[0x32] = 1
                }),
                false,
                PkmIssue::DuplicateMove,
            ),
            (
                "pp",
                raw_edit(&boxed, |d| d[0x30] = 200),
                false,
                PkmIssue::Pp,
            ),
            (
                "pp ups",
                raw_edit(&boxed, |d| d[0x34] = 4),
                false,
                PkmIssue::Pp,
            ),
            (
                "exp",
                raw_edit(&boxed, |d| {
                    d[0x10..0x14].copy_from_slice(&5_000_000u32.to_le_bytes())
                }),
                false,
                PkmIssue::Exp,
            ),
            (
                "nickname unterminated",
                raw_edit(&boxed, |d| {
                    for i in 0..11 {
                        d[0x48 + 2 * i..0x4A + 2 * i].copy_from_slice(&0x0123u16.to_le_bytes())
                    }
                }),
                false,
                PkmIssue::Nickname,
            ),
            (
                "ot unterminated",
                raw_edit(&boxed, |d| {
                    for i in 0..8 {
                        d[0x68 + 2 * i..0x6A + 2 * i].copy_from_slice(&0x0123u16.to_le_bytes())
                    }
                }),
                false,
                PkmIssue::OtName,
            ),
            (
                "language",
                raw_edit(&boxed, |d| d[0x17] = 6),
                false,
                PkmIssue::Language,
            ),
            (
                "origin game",
                raw_edit(&boxed, |d| d[0x5F] = 0),
                false,
                PkmIssue::OriginGame,
            ),
            (
                "ball 0",
                raw_edit(&boxed, |d| d[0x83] = 0),
                false,
                PkmIssue::Ball,
            ),
            (
                "ball 200",
                raw_edit(&boxed, |d| d[0x83] = 200),
                false,
                PkmIssue::Ball,
            ),
            (
                "met location",
                raw_edit(&boxed, |d| {
                    d[0x80..0x82].copy_from_slice(&1500u16.to_le_bytes())
                }),
                false,
                PkmIssue::MetLocation,
            ),
            (
                "party stat 0",
                raw_edit(&party, |d| d[0x92..0x94].fill(0)),
                true,
                PkmIssue::PartyStats,
            ),
            (
                "party hp",
                raw_edit(&party, |d| {
                    d[0x8E..0x90].copy_from_slice(&999u16.to_le_bytes())
                }),
                true,
                PkmIssue::PartyStats,
            ),
            (
                "party level",
                raw_edit(&party, |d| d[0x8C] = d[0x8C].wrapping_add(1)),
                true,
                PkmIssue::PartyStats,
            ),
            ("box in party", boxed.clone(), true, PkmIssue::WrongFormat),
        ];
        let mut cases = cases;
        if gen4 {
            cases.push((
                "gen4 bad char",
                raw_edit(&boxed, |d| {
                    d[0x48..0x4A].copy_from_slice(&0u16.to_le_bytes())
                }),
                false,
                PkmIssue::Nickname,
            ));
            cases.push((
                "hgss ball",
                raw_edit(&boxed, |d| d[0x86] = 30),
                false,
                PkmIssue::Ball,
            ));
        } else {
            cases.push((
                "nature",
                raw_edit(&boxed, |d| d[0x41] = 25),
                false,
                PkmIssue::Nature,
            ));
        }
        let stale = raw_edit(&party, |d| d[0x92] ^= 1);
        assert_eq!(
            check_pkm(&r, &stale, true),
            Ok(()),
            "stale stats are game behaviour"
        );
        assert!(!verity::safety::party_stats_exact(&r, &stale));
        for (name, pk, party, want) in cases {
            assert_eq!(check_pkm(&r, &pk, party), Err(want), "{file}: {name}");
        }
    }
}

#[test]
fn rejected_writes_leave_save_unchanged() {
    if !ready() {
        return;
    }
    let path = root().join("corpus/real/pkhex-avalonia/gen4_platinum.sav");
    let Ok(mut data) = fs::read(&path) else {
        return;
    };
    let orig = data.clone();
    let mut save = Save::open(&mut data).unwrap();
    let r = rules(save.kind());
    let party = save.pkm(Slot::Party(0)).unwrap();
    // An invalid Pokémon is refused before anything is written.
    let bad = raw_edit(&party.to_stored(), |d| d[0x15] = 0);
    assert_eq!(
        checked_set_pkm(&r, &mut save, Slot::Box(0, 0), &bad),
        Err(WriteError::Pkm(PkmIssue::Ability))
    );
    // A party gap is refused.
    let n = save.party_count();
    if n < 5 {
        assert_eq!(
            checked_set_pkm(&r, &mut save, Slot::Party(n as u8 + 1), &party),
            Err(WriteError::Save(SaveIssue::PartyCount))
        );
    }
    // Turning a one-Pokémon party into eggs only is rolled back.
    if n == 1 {
        let egg = raw_edit(&party, |d| d[0x3B] |= 0x40);
        if check_pkm(&r, &egg, true).is_ok() {
            assert_eq!(
                checked_set_pkm(&r, &mut save, Slot::Party(0), &egg),
                Err(WriteError::Save(SaveIssue::NoUsablePartyMember))
            );
        }
    }
    assert!(data == orig, "a rejected write changed the save");
}

/// A White 2 save with its party and boxes emptied, using a real empty slot from the save. With
/// `keep_one`, the first box slot keeps a Pokémon.
fn emptied_white2(keep_one: bool) -> Option<Vec<u8>> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../corpus/real/pkhex-avalonia/gen5_white2.sav");
    let mut data = fs::read(path).ok()?;
    let mut save = Save::open(&mut data).unwrap();
    let slots: Vec<Slot> = (0..save.box_count() as u8)
        .flat_map(|b| (0..30u8).map(move |s| Slot::Box(b, s)))
        .collect();
    let empty_at = *slots
        .iter()
        .find(|s| save.pkm(**s).unwrap().species() == 0)
        .unwrap();
    let empty = save.slot_raw(empty_at).unwrap().to_vec();
    let occupied = *slots
        .iter()
        .find(|s| save.pkm(**s).unwrap().species() != 0)
        .unwrap();
    for slot in slots {
        if !(keep_one && slot == occupied) {
            save.set_slot_raw(slot, &empty).unwrap();
        }
    }
    save.set_party_count(0);
    save.finalize();
    Some(data)
}

#[test]
fn p7_empty_party_is_only_safe_before_the_first_pokemon() {
    let r = rules(SaveKind::B2W2);
    let Some(mut fresh) = emptied_white2(false) else {
        return;
    };
    assert_eq!(check_save(&r, &Save::open(&mut fresh).unwrap()), Ok(()));
    let mut in_progress = emptied_white2(true).unwrap();
    assert_eq!(
        check_save(&r, &Save::open(&mut in_progress).unwrap()),
        Err(SaveIssue::PartyCount)
    );
}
