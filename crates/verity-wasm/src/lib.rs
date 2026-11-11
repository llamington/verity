// SPDX-License-Identifier: GPL-3.0-only
//! Browser bindings for the verity core (PLAN P12).
//!
//! The WASM module holds code only. The page fetches the data blobs it needs
//! (`data/<name>.bin`) and hands them over with [`attach`]; they stay attached for the page's
//! lifetime, shared by every [`Session`].

use std::cell::{Cell, RefCell};
use std::collections::HashMap;

use serde::Serialize;
use tsify::{Ts, Tsify};
use wasm_bindgen::prelude::*;

use verity::data::{string_table as st, GameData, Strings};
use verity::legality::analysis4::{Context4, Matched, Report};
use verity::legality::analysis5::Context5;
use verity::legality::moves4::MoveResult;
use verity::legality::verdict;
use verity::pkm::{Format, Pkm};
use verity::save::{Save, SaveKind, Slot, BOX_SLOTS};
use verity::text;

// ---- blobs ---------------------------------------------------------------------------------

thread_local! {
    /// Attached blobs by name, leaked so the parsed views (and the cached contexts) can borrow
    /// them for the page's lifetime. Each name is attached at most once.
    static BLOBS: RefCell<HashMap<String, &'static [u8]>> = RefCell::new(HashMap::new());
    static CTX4: Cell<Option<&'static Context4<'static>>> = const { Cell::new(None) };
    static CTX5: Cell<Option<&'static Context5<'static>>> = const { Cell::new(None) };
}

fn blob(name: &str) -> Option<&'static [u8]> {
    BLOBS.with(|b| b.borrow().get(name).copied())
}

/// Attaches a fetched blob by its file name without `.bin` (`strings`, `pt`, `enc_hgss`, ...).
/// `strings` is the strings blob of the page's language. Attaching a name twice keeps the first.
#[wasm_bindgen]
pub fn attach(name: &str, bytes: Vec<u8>) -> Result<(), JsError> {
    if blob(name).is_some() {
        return Ok(());
    }
    if name == "strings" {
        Strings::load(&bytes).map_err(err)?;
    } else if GAMES.contains(&name) {
        GameData::load(&bytes).map_err(err)?;
    }
    let leaked: &'static [u8] = Box::leak(bytes.into_boxed_slice());
    BLOBS.with(|b| b.borrow_mut().insert(name.into(), leaked));
    Ok(())
}

/// Whether `name` is attached already (so the page skips the fetch).
#[wasm_bindgen]
pub fn attached(name: &str) -> bool {
    blob(name).is_some()
}

const GAMES: [&str; 5] = ["dp", "pt", "hgss", "bw", "b2w2"];

fn vblob(b: verdict::Blob) -> Option<&'static [u8]> {
    blob(b.file())
}

fn context4() -> Option<&'static Context4<'static>> {
    if let Some(c) = CTX4.get() {
        return Some(c);
    }
    let c: &'static Context4<'static> = Box::leak(Box::new(verdict::context4(vblob)?));
    CTX4.set(Some(c));
    Some(c)
}

fn context5() -> Option<&'static Context5<'static>> {
    if let Some(c) = CTX5.get() {
        return Some(c);
    }
    let c: &'static Context5<'static> = Box::leak(Box::new(verdict::context5(vblob)?));
    CTX5.set(Some(c));
    Some(c)
}

fn strings() -> Option<Strings<'static>> {
    blob("strings").and_then(|b| Strings::load(b).ok())
}

fn game_data(kind: SaveKind) -> Option<GameData<'static>> {
    blob(game_id(kind)).and_then(|b| GameData::load(b).ok())
}

// ---- views ---------------------------------------------------------------------------------

/// What the page shows for one Pokémon in a list.
#[derive(Serialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct Mon {
    pub species: u16,
    /// From the strings blob; `None` until it is attached.
    pub species_name: Option<String>,
    pub nickname: String,
    /// The game's nickname flag; default names are stored in capitals in Gen 4.
    pub nicknamed: bool,
    /// `None` until the game's data blob is attached.
    pub level: Option<u8>,
    pub form: u8,
    pub egg: bool,
    pub shiny: bool,
    pub checksum_ok: bool,
}

#[derive(Serialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct Trainer {
    pub name: String,
    pub tid: u16,
    pub sid: u16,
    pub money: u32,
    /// Badges earned (the save stores one bit per badge).
    pub badges: u32,
    pub played: String,
}

#[derive(Serialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct Overview {
    /// `dp`, `pt`, `hgss`, `bw` or `b2w2`: also the name of the game's data blob.
    pub game: String,
    pub generation: u8,
    pub checksums_ok: bool,
    pub trainer: Trainer,
    pub box_count: usize,
    pub party: Vec<Mon>,
}

#[derive(Serialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct BoxView {
    pub name: String,
    /// `BOX_SLOTS` entries; `None` for an empty slot.
    pub slots: Vec<Option<Mon>>,
}

/// A named id; `name` is `None` when the strings blob lacks it.
#[derive(Serialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct Named {
    pub id: u16,
    pub name: Option<String>,
}

#[derive(Serialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct MoveSlot {
    pub id: u16,
    pub name: Option<String>,
    pub pp: u8,
    pub pp_ups: u8,
    /// The move's type (`None` without the game blob... or for an empty slot).
    pub move_type: Option<Named>,
}

/// The summary of one Pokémon (the DS summary and box-view panels).
#[derive(Serialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub mon: Mon,
    pub pid: u32,
    pub exp: u32,
    pub nature: Named,
    pub ability: Named,
    pub hidden_ability: bool,
    /// 0 male, 1 female, 2 genderless.
    pub gender: u8,
    pub held_item: Named,
    pub ball: Named,
    pub types: Vec<Named>,
    pub moves: Vec<MoveSlot>,
    pub ivs: Vec<u16>,
    pub evs: Vec<u16>,
    /// HP, Atk, Def, Spe, SpA, SpD (the stored order, as IVs/EVs); `None` without the game blob.
    pub stats: Option<Vec<u16>>,
    pub friendship: u8,
    pub ot: String,
    pub ot_female: bool,
    pub tid: u16,
    pub sid: u16,
    pub language: String,
    pub origin: String,
    pub met_location: Named,
    pub met_level: u8,
    /// `YYYY-MM-DD`.
    pub met_date: String,
    pub egg_location: Named,
    pub egg_date: Option<String>,
    pub fateful: bool,
    /// Cool, Beauty, Cute, Smart, Tough, Sheen.
    pub contest: Vec<u8>,
    /// Circle, triangle, square, heart, star, diamond.
    pub markings: Vec<bool>,
    pub ribbon_count: u32,
}

#[derive(Serialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct MoveCheck {
    pub valid: bool,
    /// How the move is learnt, or what is wrong with it.
    pub how: String,
}

/// The legality verdict (PKHeX-equivalent).
#[derive(Serialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct Legality {
    /// `legal`, `illegal` or `unchecked`.
    pub status: String,
    /// Why it wasn't checked.
    pub reason: Option<String>,
    /// PKHeX check names that failed.
    pub checks: Vec<String>,
    pub moves: Vec<MoveCheck>,
    /// The origin verity matched.
    pub origin: Option<String>,
}

/// The whole-save game-safe check run before saving.
#[derive(Serialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct SaveCheck {
    pub ok: bool,
    pub issue: Option<String>,
    /// The Pokémon at fault: box (`None` for the party) and slot.
    pub box_index: Option<u8>,
    pub slot: Option<u8>,
}

fn game_id(kind: SaveKind) -> &'static str {
    match kind {
        SaveKind::DP => "dp",
        SaveKind::Pt => "pt",
        SaveKind::HGSS => "hgss",
        SaveKind::BW => "bw",
        SaveKind::B2W2 => "b2w2",
    }
}

fn err(e: impl core::fmt::Debug) -> JsError {
    JsError::new(&format!("{e:?}"))
}

// ---- session -------------------------------------------------------------------------------

/// An open save.
#[wasm_bindgen]
pub struct Session {
    save: Vec<u8>,
}

#[wasm_bindgen]
impl Session {
    /// Opens a raw 512 KB save or a DeSmuME `.dsv`.
    #[wasm_bindgen(constructor)]
    pub fn new(bytes: Vec<u8>) -> Result<Session, JsError> {
        let mut save = bytes;
        Save::open(&mut save).map_err(err)?;
        Ok(Session { save })
    }

    /// The data blob this save's game needs (fetch `data/<name>.bin`).
    #[wasm_bindgen(js_name = gameBlob)]
    pub fn game_blob(&mut self) -> String {
        game_id(self.open().kind()).into()
    }

    /// The blobs [`Session::legality`] reads for this save (fetch them all, then attach).
    #[wasm_bindgen(js_name = legalityBlobs)]
    pub fn legality_blobs(&mut self) -> Vec<String> {
        verdict::blobs_for(self.open().generation())
            .map(|b| b.file().into())
            .collect()
    }

    pub fn overview(&mut self) -> Result<Ts<Overview>, JsError> {
        let save = self.open();
        let t = save.trainer();
        let gen = save.generation();
        let party = (0..save.party_count())
            .filter_map(|i| save.pkm(Slot::Party(i as u8)).ok())
            .map(|pk| mon(save.kind(), &pk))
            .collect();
        Ok(Overview {
            game: game_id(save.kind()).into(),
            generation: gen,
            checksums_ok: save.checksums_valid(),
            trainer: Trainer {
                name: decode(&t.name_raw, gen),
                tid: t.tid,
                sid: t.sid,
                money: t.money,
                badges: t.badges.count_ones(),
                played: format!(
                    "{}:{:02}:{:02}",
                    t.played_hours, t.played_minutes, t.played_seconds
                ),
            },
            box_count: save.box_count(),
            party,
        }
        .into_ts()?)
    }

    #[wasm_bindgen(js_name = boxView)]
    pub fn box_view(&mut self, index: u8) -> Result<Ts<BoxView>, JsError> {
        let save = self.open();
        let gen = save.generation();
        let name = decode(&save.box_name_raw(index as usize).map_err(err)?, gen);
        let mut slots = Vec::with_capacity(BOX_SLOTS);
        for s in 0..BOX_SLOTS as u8 {
            let pk = save.pkm(Slot::Box(index, s)).map_err(err)?;
            slots.push((pk.species() != 0).then(|| mon(save.kind(), &pk)));
        }
        Ok(BoxView { name, slots }.into_ts()?)
    }

    /// The Pokémon at `box_index` (-1 for the party) and `slot`.
    pub fn summary(&mut self, box_index: i32, slot: u8) -> Result<Ts<Summary>, JsError> {
        let (kind, pk) = self.pkm(box_index, slot)?;
        Ok(summary(kind, &pk).into_ts()?)
    }

    /// Legality of the Pokémon at `box_index` (-1 for the party) and `slot`. Attach the
    /// [`Session::legality_blobs`] first, or the verdict is "legality data missing".
    pub fn legality(&mut self, box_index: i32, slot: u8) -> Result<Ts<Legality>, JsError> {
        let (_, pk) = self.pkm(box_index, slot)?;
        let gen = self.open().generation();
        let c5 = if gen == 5 { context5() } else { None };
        let l = match verdict::check(gen, &pk, context4(), c5) {
            Ok(r) => legality(&pk, &r),
            Err(why) => Legality {
                status: "unchecked".into(),
                reason: Some(why.text().into()),
                checks: vec![],
                moves: vec![],
                origin: None,
            },
        };
        Ok(l.into_ts()?)
    }

    /// The game-safe validator over the whole save (needs the game blob and `moves4`/`moves5`).
    #[wasm_bindgen(js_name = saveCheck)]
    pub fn save_check(&mut self) -> Result<Ts<SaveCheck>, JsError> {
        use verity::safety::{self, SaveIssue};
        let save = self.open();
        let kind = save.kind();
        let moves = blob(if save.generation() == 4 { "moves4" } else { "moves5" })
            .and_then(|b| verity::data::MoveData::load(b).ok());
        let mut c = SaveCheck {
            ok: false,
            issue: None,
            box_index: None,
            slot: None,
        };
        let (Some(game), Some(moves)) = (game_data(kind), moves) else {
            c.issue = Some("validator data missing".into());
            return Ok(c.into_ts()?);
        };
        let rules = safety::Rules { kind, game, moves };
        match safety::check_save(&rules, &save) {
            Ok(()) => c.ok = true,
            Err(SaveIssue::PartyCount) => c.issue = Some("party count out of range".into()),
            Err(SaveIssue::NoUsablePartyMember) => {
                c.issue = Some("no party Pokémon that can battle".into())
            }
            Err(SaveIssue::Checksums) => c.issue = Some("checksums".into()),
            Err(SaveIssue::Pkm(at, why)) => {
                c.issue = Some(format!("{why:?}"));
                (c.box_index, c.slot) = match at {
                    Slot::Party(i) => (None, Some(i)),
                    Slot::Box(b, s) => (Some(b), Some(s)),
                };
            }
        }
        Ok(c.into_ts()?)
    }

    /// The save file as it stands, for downloading.
    pub fn bytes(&self) -> Vec<u8> {
        self.save.clone()
    }
}

impl Session {
    fn open(&mut self) -> Save<'_> {
        Save::open(&mut self.save).expect("opened in new()")
    }

    fn pkm(&mut self, box_index: i32, slot: u8) -> Result<(SaveKind, Pkm), JsError> {
        let save = self.open();
        let at = if box_index < 0 {
            if slot as usize >= save.party_count() {
                return Err(JsError::new("no Pokémon there"));
            }
            Slot::Party(slot)
        } else {
            Slot::Box(box_index as u8, slot)
        };
        let pk = save.pkm(at).map_err(err)?;
        if pk.species() == 0 {
            return Err(JsError::new("no Pokémon there"));
        }
        Ok((save.kind(), pk))
    }
}

// ---- conversions ---------------------------------------------------------------------------

fn utf16(s: verity::data::blob::U16s) -> Option<String> {
    let v: Vec<u16> = s.iter().collect();
    (!v.is_empty()).then(|| String::from_utf16_lossy(&v))
}

fn named(cat: u16, id: u16) -> Named {
    let name = strings().and_then(|s| {
        ((id as usize) < s.count(cat))
            .then(|| utf16(s.get(cat, id as usize)))
            .flatten()
    });
    Named { id, name }
}

fn mon(kind: SaveKind, pk: &Pkm) -> Mon {
    let species = pk.species();
    let level = game_data(kind).and_then(|g| {
        (species <= g.max_species())
            .then(|| pk.level(&g.species_form(species, pk.form()).base_info()))
    });
    Mon {
        species,
        species_name: named(st::SPECIES, species).name,
        nickname: decode(&pk.nickname_raw(), gen_of(pk)),
        nicknamed: pk.is_nicknamed(),
        level,
        form: pk.form(),
        egg: pk.is_egg(),
        shiny: pk.is_shiny(),
        checksum_ok: pk.checksum_valid(),
    }
}

fn gen_of(pk: &Pkm) -> u8 {
    if matches!(pk.format(), Format::Pk4) {
        4
    } else {
        5
    }
}

/// The item a ball id stands for (PKHeX `BallItem`).
fn ball_item(ball: u8) -> u16 {
    match ball {
        1..=16 => ball as u16,
        17..=24 => 492 + (ball as u16 - 17),
        25 => 576,
        _ => 4,
    }
}

fn origin_name(v: u8) -> &'static str {
    match v {
        1 => "Sapphire",
        2 => "Ruby",
        3 => "Emerald",
        4 => "FireRed",
        5 => "LeafGreen",
        7 => "HeartGold",
        8 => "SoulSilver",
        10 => "Diamond",
        11 => "Pearl",
        12 => "Platinum",
        15 => "Colosseum/XD",
        20 => "White",
        21 => "Black",
        22 => "White 2",
        23 => "Black 2",
        _ => "unknown",
    }
}

fn language_name(l: u8) -> &'static str {
    match l {
        1 => "Japanese",
        2 => "English",
        3 => "French",
        4 => "Italian",
        5 => "German",
        7 => "Spanish",
        8 => "Korean",
        _ => "unknown",
    }
}

fn date([y, m, d]: [u8; 3]) -> String {
    format!("{:04}-{:02}-{:02}", 2000 + y as u16, m, d)
}

fn location(gen: u8, id: u16) -> Named {
    let name = strings().and_then(|s| utf16(s.location(gen, id)));
    Named { id, name }
}

fn summary(kind: SaveKind, pk: &Pkm) -> Summary {
    let gen = gen_of(pk);
    let items = if gen == 4 { st::ITEMS4 } else { st::ITEMS5 };
    let game = game_data(kind);
    let info = game.as_ref().and_then(|g| {
        (pk.species() <= g.max_species()).then(|| g.species_form(pk.species(), pk.form()))
    });
    let moves_blob = blob(if gen == 4 { "moves4" } else { "moves5" })
        .and_then(|b| verity::data::MoveData::load(b).ok());
    let (pp, ups) = (pk.pp(), pk.pp_ups());
    let moves = pk
        .moves()
        .iter()
        .enumerate()
        .map(|(i, &id)| MoveSlot {
            id,
            name: (id != 0).then(|| named(st::MOVES, id).name).flatten(),
            pp: pp[i],
            pp_ups: ups[i],
            move_type: moves_blob
                .as_ref()
                .filter(|m| id != 0 && id <= m.max_move())
                .map(|m| named(st::TYPES, m.move_type(id) as u16)),
        })
        .collect();
    let types = info
        .map(|p| {
            let [a, b] = p.types();
            let mut v = vec![named(st::TYPES, a as u16)];
            if b != a {
                v.push(named(st::TYPES, b as u16));
            }
            v
        })
        .unwrap_or_default();
    let egg_loc = pk.egg_location();
    let markings = pk.markings();
    Summary {
        mon: mon(kind, pk),
        pid: pk.pid(),
        exp: pk.exp(),
        nature: named(st::NATURES, pk.nature() as u16),
        ability: named(st::ABILITIES, pk.ability() as u16),
        hidden_ability: gen == 5 && pk.hidden_ability(),
        gender: pk.gender(),
        held_item: named(items, pk.held_item()),
        ball: Named {
            id: pk.ball() as u16,
            name: named(items, ball_item(pk.ball())).name,
        },
        types,
        moves,
        ivs: pk.ivs().to_vec(),
        evs: pk.evs().to_vec(),
        stats: info.map(|p| pk.calc_stats(&p.base_info()).to_vec()),
        friendship: pk.friendship(),
        ot: decode(&pk.ot_name_raw(), gen),
        ot_female: pk.ot_gender() == 1,
        tid: pk.tid(),
        sid: pk.sid(),
        language: language_name(pk.language()).into(),
        origin: origin_name(pk.version()).into(),
        met_location: location(gen, pk.met_location()),
        met_level: pk.met_level(),
        met_date: date(pk.met_date()),
        egg_location: location(gen, egg_loc),
        egg_date: (egg_loc != 0).then(|| date(pk.egg_date())),
        fateful: pk.fateful(),
        contest: pk.contest().to_vec(),
        markings: (0..6).map(|i| markings & (1 << i) != 0).collect(),
        ribbon_count: pk.ribbons().iter().map(|b| b.count_ones()).sum(),
    }
}

fn move_how(m: MoveResult) -> String {
    match m {
        MoveResult::Unparsed => "not checked".into(),
        MoveResult::Empty => "".into(),
        MoveResult::EmptyInvalid => "empty slot before a move".into(),
        MoveResult::Duplicate => "duplicate".into(),
        MoveResult::Unobtainable => "can't be learnt".into(),
        MoveResult::LevelUp { level } => format!("level up ({level})"),
        MoveResult::Machine => "TM/HM".into(),
        MoveResult::Tutor => "tutor".into(),
        MoveResult::Initial => "from its origin".into(),
        MoveResult::InheritLevelUp => "inherited".into(),
        MoveResult::Sketch => "Sketch".into(),
    }
}

fn legality(pk: &Pkm, r: &Report) -> Legality {
    // Gen 5 analyses match Gen 5 encounters; Gen 3/4 origins (also after Poké Transfer) match
    // Gen 4 ones, whose locations are Gen 4 met ids.
    let enc_gen = if verity::legality::analysis5::is_gen5_version(pk.version()) {
        5
    } else {
        4
    };
    let origin = match &r.matched {
        Matched::Slot { encounter, .. } => {
            let place = location(enc_gen, encounter.location()).name;
            let what = match encounter {
                verity::data::encounters::Encounter::Slot { .. } => "wild",
                // Gifts, legendaries and other one-off encounters.
                verity::data::encounters::Encounter::Template(_) => "fixed encounter",
            };
            Some(match place {
                Some(p) => format!("{what}, {p}"),
                None => what.into(),
            })
        }
        Matched::Egg { .. } => Some("egg".into()),
        Matched::Gift { .. } | Matched::Gift5 { .. } => Some("Wonder Card".into()),
        Matched::RangerManaphy => Some("Pokémon Ranger Manaphy egg".into()),
        Matched::Ranch { .. } => Some("My Pokémon Ranch".into()),
        Matched::Invalid => None,
    };
    Legality {
        status: if r.valid() { "legal" } else { "illegal" }.into(),
        reason: None,
        checks: r.invalid.iter().map(|c| c.name().into()).collect(),
        moves: r
            .moves
            .iter()
            .map(|&m| MoveCheck {
                valid: m.valid(),
                how: move_how(m),
            })
            .collect(),
        origin,
    }
}

fn decode(raw: &[u16], gen: u8) -> String {
    let mut out = [0u16; 32];
    let n = if gen == 4 {
        text::decode4(raw, &mut out)
    } else {
        text::decode5(raw, &mut out)
    };
    String::from_utf16_lossy(&out[..n])
}
