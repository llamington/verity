// SPDX-License-Identifier: GPL-3.0-only
//! Browser bindings for the verity core (PLAN P12).
//!
//! The WASM module holds code only. The page fetches the data blobs it needs
//! (`data/<name>.bin`) and hands them over with [`attach`]; they stay attached for the page's
//! lifetime, shared by every [`Session`].

use std::cell::{Cell, RefCell};
use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use tsify::{Ts, Tsify};
use wasm_bindgen::prelude::*;

use verity::data::encounters::EncounterData;
use verity::data::{string_table as st, GameData, Strings};
use verity::edit::{self, Edit, Met, Nickname};
use verity::gen::status::GenStatus;
use verity::gen::wild4::Lead;
use verity::gen::wizard;
use verity::legality::analysis4::{Context4, Matched, Report};
use verity::legality::analysis5::Context5;
use verity::legality::moves4::MoveResult;
use verity::legality::verdict;
use verity::pkm::{Format, Pkm};
use verity::safety::{self, Rules};
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
    /// By [`Choices::ribbons`] order.
    pub ribbons: Vec<bool>,
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

/// What the editor's pickers offer.
#[derive(Serialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct Choices {
    /// Items this game lets a Pokémon hold, `0` (none) first.
    pub held_items: Vec<Named>,
    pub moves: Vec<Named>,
    pub balls: Vec<Named>,
    pub locations: Vec<Named>,
    /// Ribbon names, in [`EditRequest::ribbons`] order.
    pub ribbons: Vec<String>,
    /// Longest nickname (5 for Japanese and Korean Pokémon, enforced by `preview`).
    pub nickname_max: u8,
    /// Nature names by id (0-24).
    pub natures: Vec<String>,
    /// Balls the add-Pokémon wizard offers (as the DS).
    pub wild_balls: Vec<Named>,
}

/// A met location, level and date.
#[derive(Deserialize, Tsify, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MetRequest {
    pub location: u16,
    pub level: u8,
    /// `YYYY-MM-DD`, 2000-2099.
    pub date: String,
}

/// An edit from the page; absent fields stay as they are.
#[derive(Deserialize, Tsify, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct EditRequest {
    #[tsify(optional, type = "number | undefined")]
    pub held_item: Option<u16>,
    /// Four move ids (0 for empty). PP are refilled and PP Ups cleared.
    #[tsify(optional, type = "number[] | undefined")]
    pub moves: Option<Vec<u16>>,
    /// The PID from [`Session::closest_ivs`], sent with its IVs.
    #[tsify(optional, type = "number | undefined")]
    pub pid: Option<u32>,
    /// HP, Atk, Def, Spe, SpA, SpD.
    #[tsify(optional, type = "number[] | undefined")]
    pub ivs: Option<Vec<u8>>,
    #[tsify(optional, type = "number[] | undefined")]
    pub evs: Option<Vec<u8>>,
    #[tsify(optional, type = "number | undefined")]
    pub level: Option<u8>,
    #[tsify(optional, type = "number | undefined")]
    pub friendship: Option<u8>,
    #[tsify(optional, type = "number | undefined")]
    pub ball: Option<u8>,
    #[tsify(optional, type = "MetRequest | undefined")]
    pub met: Option<MetRequest>,
    /// Cool, Beauty, Cute, Smart, Tough, Sheen.
    #[tsify(optional, type = "number[] | undefined")]
    pub contest: Option<Vec<u8>>,
    /// One flag per [`Choices::ribbons`] entry.
    #[tsify(optional, type = "boolean[] | undefined")]
    pub ribbons: Option<Vec<bool>>,
    #[tsify(optional, type = "string | undefined")]
    pub nickname: Option<String>,
    /// Back to the species name (ignored when `nickname` is set).
    #[tsify(optional, type = "boolean | undefined")]
    pub reset_nickname: Option<bool>,
}

fn array<T: Copy + Default, const N: usize>(v: &[T], what: &str) -> Result<[T; N], JsError> {
    v.try_into()
        .map_err(|_| JsError::new(&format!("{what}: {N} values expected")))
}

impl EditRequest {
    fn to_edit(&self, pk: &Pkm) -> Result<Edit, JsError> {
        let met = match &self.met {
            None => None,
            Some(m) => Some(Met {
                location: m.location,
                level: m.level,
                date: parse_date(&m.date)?,
            }),
        };
        let ribbons = match &self.ribbons {
            None => None,
            Some(flags) => {
                let mut r = pk.ribbons();
                for (i, &on) in flags.iter().enumerate().take(edit::RIBBONS.len()) {
                    let (byte, bit) = edit::ribbon_pos(i);
                    r[byte] = (r[byte] & !(1 << bit)) | ((on as u8) << bit);
                }
                Some(r)
            }
        };
        let nickname = match (&self.nickname, self.reset_nickname) {
            (Some(n), _) => Some(nickname(n, pk)?),
            (None, Some(true)) => Some(Nickname::Reset),
            _ => None,
        };
        Ok(Edit {
            held_item: self.held_item,
            moves: self
                .moves
                .as_deref()
                .map(|m| array(m, "moves"))
                .transpose()?,
            pid: self.pid,
            ivs: self
                .ivs
                .as_deref()
                .map(|v| array(v, "IVs"))
                .transpose()?
                .map(|v: [u8; 6]| v.map(|x| x.min(31))),
            evs: self.evs.as_deref().map(|v| array(v, "EVs")).transpose()?,
            level: self.level,
            friendship: self.friendship,
            ball: self.ball,
            met,
            contest: self
                .contest
                .as_deref()
                .map(|v| array(v, "contest"))
                .transpose()?,
            ribbons,
            nickname,
        })
    }
}

/// Checks a typed nickname against what the game can store.
fn nickname(n: &str, pk: &Pkm) -> Result<Nickname, JsError> {
    let units: Vec<u16> = n.encode_utf16().collect();
    let cjk = matches!(pk.language(), 1 | 8);
    let max = if cjk { 5 } else { edit::NICKNAME_MAX };
    if units.is_empty() {
        return Err(JsError::new("nickname: empty"));
    }
    if units.len() > max {
        return Err(JsError::new(&format!("nickname: at most {max} characters")));
    }
    if matches!(pk.format(), Format::Pk4) {
        if let Some(c) = n
            .chars()
            .find(|c| !text::is_encodable_g4(&c.encode_utf16(&mut [0; 2])[..]))
        {
            return Err(JsError::new(&format!("nickname: Gen 4 can't store “{c}”")));
        }
    }
    Ok(Nickname::new(&units))
}

/// Balls the DS wizard offers: Poké, Great, Ultra, Master, then Net to Premier.
const WIZARD_BALLS: [u8; 14] = [4, 3, 2, 1, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15];

/// One wild slot for the wizard.
#[derive(Serialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct WildSlot {
    /// Pass as [`WildAdd::encounter`].
    pub index: u16,
    pub location: Named,
    pub form: u8,
    pub level_min: u8,
    pub level_max: u8,
    /// Grass, Surf, Old Rod, ...
    pub kind: String,
    pub slot_number: u8,
    pub can_radar: bool,
}

/// A wild Pokémon to add; absent fields mean any.
#[derive(Deserialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct WildAdd {
    pub version: u8,
    pub species: u16,
    /// The chosen slot's form (0 with no slot chosen).
    #[serde(default)]
    pub form: u8,
    #[tsify(optional, type = "number | undefined")]
    pub encounter: Option<u16>,
    #[tsify(optional, type = "number | undefined")]
    pub level: Option<u8>,
    #[tsify(optional, type = "number | undefined")]
    pub nature: Option<u8>,
    /// 0 male, 1 female.
    #[tsify(optional, type = "number | undefined")]
    pub gender: Option<u8>,
    #[serde(default)]
    pub shiny: bool,
    /// `none`, `synchronize` (needs `nature`), `cuteCharm` (needs `gender`) or `radar`
    /// (Gen 4 only; a radar chain makes it shiny).
    pub lead: String,
    #[tsify(optional, type = "number | undefined")]
    pub ball: Option<u8>,
    /// `YYYY-MM-DD`.
    pub met_date: String,
    pub seed: u32,
}

/// A bred Gen 4 egg to add; absent fields mean any.
#[derive(Deserialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct EggAdd {
    pub version: u8,
    pub species: u16,
    #[tsify(optional, type = "number | undefined")]
    pub nature: Option<u8>,
    #[tsify(optional, type = "number | undefined")]
    pub gender: Option<u8>,
    #[serde(default)]
    pub shiny: bool,
    /// Already hatched rather than still an egg.
    #[serde(default)]
    pub hatched: bool,
    /// `YYYY-MM-DD`: the egg date (and met date when hatched).
    pub date: String,
    pub seed: u32,
}

fn list<T: Tsify + Serialize>(items: impl Iterator<Item = T>) -> Result<Vec<Ts<T>>, JsError> {
    items.map(|t| Ok(t.into_ts()?)).collect()
}

fn wizard_data(version: u8) -> Result<&'static EncounterData<'static>, JsError> {
    let (c4, c5) = if version >= 20 {
        (None, context5())
    } else {
        (context4(), None)
    };
    wizard::wild_data(c4, c5, version).ok_or_else(|| JsError::new("legality data missing"))
}

fn slot_kind(gen: u8, t: u8) -> &'static str {
    const G4: [&str; 15] = [
        "Grass",
        "Surf",
        "Old Rod",
        "Good Rod",
        "Super Rod",
        "Rock Smash",
        "Headbutt",
        "Headbutt",
        "Bug Contest",
        "Honey Tree",
        "Safari",
        "Safari",
        "Safari rod",
        "Safari rod",
        "Safari rod",
    ];
    const G5: [&str; 6] = [
        "Standard",
        "Grass",
        "Surf",
        "Super Rod",
        "Swarm",
        "Hidden Grotto",
    ];
    let list: &[&str] = if gen == 4 { &G4 } else { &G5 };
    list.get(t as usize).copied().unwrap_or("?")
}

/// `YYYY-MM-DD` (2000-2099) as years since 2000, month, day.
fn parse_date(s: &str) -> Result<[u8; 3], JsError> {
    let bad = || JsError::new("date: YYYY-MM-DD between 2000 and 2099 expected");
    let parts: Vec<u16> = s
        .split('-')
        .map(|p| p.parse().map_err(|_| bad()))
        .collect::<Result<_, _>>()?;
    let [y, m, d] = parts[..] else {
        return Err(bad());
    };
    if !(2000..=2099).contains(&y) || !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return Err(bad());
    }
    Ok([(y - 2000) as u8, m as u8, d as u8])
}

/// The result of [`Session::preview`].
#[derive(Serialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct Preview {
    /// Why the game-safe validator would refuse it.
    pub issue: Option<String>,
    pub legality: Legality,
    pub summary: Summary,
}

#[derive(Serialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct IvFix {
    pub ivs: Vec<u8>,
    pub pid: u32,
    /// Total IV points away from the request.
    pub distance: u32,
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
    dirty: bool,
}

#[wasm_bindgen]
impl Session {
    /// Opens a raw 512 KB save or a DeSmuME `.dsv`.
    #[wasm_bindgen(constructor)]
    pub fn new(bytes: Vec<u8>) -> Result<Session, JsError> {
        let mut save = bytes;
        Save::open(&mut save).map_err(err)?;
        Ok(Session { save, dirty: false })
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
        Ok(judge(gen, &pk).into_ts()?)
    }

    /// The game-safe validator over the whole save (needs the game blob and `moves4`/`moves5`).
    #[wasm_bindgen(js_name = saveCheck)]
    pub fn save_check(&mut self) -> Result<Ts<SaveCheck>, JsError> {
        Ok(self.check_save().into_ts()?)
    }

    /// What the editor's pickers offer for this save's game.
    pub fn choices(&mut self) -> Result<Ts<Choices>, JsError> {
        let kind = self.open().kind();
        let gen = self.open().generation();
        let rules = rules(kind).ok_or_else(|| JsError::new("game data missing"))?;
        let items = if gen == 4 { st::ITEMS4 } else { st::ITEMS5 };
        let held = core::iter::once(0)
            .chain(rules.game.held_items().iter())
            .map(|i| named(items, i))
            .collect();
        let moves = (1..=rules.moves.max_move())
            .map(|m| named(st::MOVES, m))
            .filter(|n| n.name.is_some())
            .collect();
        let balls = (1..=if gen == 4 { 24 } else { 25 })
            .map(|b| Named {
                id: b,
                name: named(items, ball_item(b as u8)).name,
            })
            .collect();
        Ok(Choices {
            held_items: held,
            moves,
            balls,
            locations: locations(gen),
            ribbons: edit::RIBBONS.iter().map(|r| (*r).into()).collect(),
            nickname_max: 10,
            natures: (0..25)
                .map(|n| named(st::NATURES, n).name.unwrap_or_default())
                .collect(),
            wild_balls: WIZARD_BALLS
                .iter()
                .map(|&b| Named {
                    id: b as u16,
                    name: named(items, ball_item(b)).name,
                })
                .collect(),
        }
        .into_ts()?)
    }

    /// Applies `edit` to a copy of the Pokémon and reports the game-safe check, legality and
    /// the resulting summary, without writing.
    pub fn preview(
        &mut self,
        box_index: i32,
        slot: u8,
        edit: Ts<EditRequest>,
    ) -> Result<Ts<Preview>, JsError> {
        let (pk, rules) = self.edited(box_index, slot, &edit.to_rust()?)?;
        let issue = safety::check_pkm(&rules, &pk, pk.is_party_format())
            .err()
            .map(|i| i.text().into());
        let gen = self.open().generation();
        Ok(Preview {
            issue,
            legality: judge(gen, &pk),
            summary: summary(rules.kind, &pk),
        }
        .into_ts()?)
    }

    /// Writes `edit` through the game-safe validator. In legal mode a result legality judges
    /// illegal is refused too. Returns why it was refused, or nothing when written.
    pub fn apply(
        &mut self,
        box_index: i32,
        slot: u8,
        edit: Ts<EditRequest>,
        legal_mode: bool,
    ) -> Result<Option<String>, JsError> {
        let (pk, rules) = self.edited(box_index, slot, &edit.to_rust()?)?;
        let gen = self.open().generation();
        if legal_mode && judge(gen, &pk).status == "illegal" {
            return Ok(Some("illegal (legal mode)".into()));
        }
        let at = slot_at(box_index, slot);
        let mut save = self.open();
        match safety::checked_set_pkm(&rules, &mut save, at, &pk) {
            Ok(()) => {
                self.dirty = true;
                Ok(None)
            }
            Err(safety::WriteError::Pkm(i)) => Ok(Some(i.text().into())),
            Err(e) => Err(err(e)),
        }
    }

    /// Whether edits were applied since the save was opened.
    #[wasm_bindgen(getter)]
    pub fn dirty(&self) -> bool {
        self.dirty
    }

    /// The legal IV spread closest to `target` (HP, Atk, Def, Spe, SpA, SpD) for a Gen 4 wild
    /// catch, with the PID that goes with it. Nothing is written. Needs the legality blobs.
    #[wasm_bindgen(js_name = closestIvs)]
    pub fn closest_ivs(
        &mut self,
        box_index: i32,
        slot: u8,
        target: Vec<u8>,
    ) -> Result<Ts<IvFix>, JsError> {
        use verity::gen::status::GenStatus;
        let goal: [u8; 6] = target
            .try_into()
            .map_err(|_| JsError::new("six IVs expected"))?;
        let (_, pk) = self.pkm(box_index, slot)?;
        let ctx = context4().ok_or_else(|| JsError::new("legality data missing"))?;
        match verity::gen::api4::closest_ivs_for(ctx, &pk, goal, CLOSEST_IV_BUDGET) {
            Ok(f) => Ok(IvFix {
                ivs: f.ivs.to_vec(),
                pid: f.pid,
                distance: f.distance,
            }
            .into_ts()?),
            Err(e) => Err(JsError::new(GenStatus::from(e).text())),
        }
    }

    /// Prepares the save for writing, as the DS does: updates the checksums, then runs the
    /// whole-save game-safe check. Returns the bytes to write, or an error saying what the
    /// validator refused. The page writes them, reads the file back and calls
    /// [`Session::mark_saved`] when they match.
    pub fn finalize(&mut self) -> Result<Vec<u8>, JsError> {
        self.open().finalize();
        let c = self.check_save();
        if !c.ok {
            let at = match (c.box_index, c.slot) {
                (Some(b), Some(s)) => format!(" (box {}, slot {})", b + 1, s + 1),
                (None, Some(s)) => format!(" (party slot {})", s + 1),
                _ => String::new(),
            };
            return Err(JsError::new(&format!(
                "not saved: {}{at}",
                c.issue.unwrap_or_default()
            )));
        }
        if !self.open().checksums_valid() {
            return Err(JsError::new("not saved: checksums didn't update"));
        }
        Ok(self.save.clone())
    }

    /// Records that the save was written and read back intact.
    #[wasm_bindgen(js_name = markSaved)]
    pub fn mark_saved(&mut self) {
        self.dirty = false;
    }

    /// The games whose wild slots and eggs the wizard offers, the save's own first (D/P, HG/SS,
    /// B/W and B2/W2 saves don't record which of the pair they are).
    #[wasm_bindgen(js_name = wizardVersions)]
    pub fn wizard_versions(&mut self) -> Result<Vec<Ts<Named>>, JsError> {
        let save = self.open();
        let v: &[u8] = match save.kind() {
            SaveKind::DP => &[10, 11],
            SaveKind::Pt => &[12],
            SaveKind::HGSS => &[7, 8],
            SaveKind::BW if save.game_version() == 21 => &[21, 20],
            SaveKind::BW => &[20, 21],
            SaveKind::B2W2 if save.game_version() == 23 => &[23, 22],
            SaveKind::B2W2 => &[22, 23],
        };
        list(v.iter().map(|&v| Named {
            id: v as u16,
            name: Some(origin_name(v).into()),
        }))
    }

    /// Species with wild slots in `version` (needs the legality blobs).
    #[wasm_bindgen(js_name = wildSpecies)]
    pub fn wild_species(&mut self, version: u8) -> Result<Vec<Ts<Named>>, JsError> {
        let data = wizard_data(version)?;
        list(
            wizard::wild_species(data, version)
                .into_iter()
                .map(|s| named(st::SPECIES, s)),
        )
    }

    /// The wild slots of `species` in `version`.
    #[wasm_bindgen(js_name = wildEncounters)]
    pub fn wild_encounters(
        &mut self,
        version: u8,
        species: u16,
    ) -> Result<Vec<Ts<WildSlot>>, JsError> {
        let data = wizard_data(version)?;
        let gen = if version >= 20 { 5 } else { 4 };
        list(
            wizard::wild_encounters(data, version, species)
                .into_iter()
                .map(|(index, e)| WildSlot {
                    index,
                    location: location(gen, e.location()),
                    form: e.form(),
                    level_min: e.level_min(),
                    level_max: e.level_max(),
                    kind: slot_kind(gen, e.slot_type()).into(),
                    slot_number: e.slot_number(),
                    can_radar: gen == 4 && e.can_radar(),
                }),
        )
    }

    /// Species a Gen 4 egg can be of (empty for Gen 5 saves, which the DS doesn't breed yet).
    #[wasm_bindgen(js_name = eggSpecies)]
    pub fn egg_species(&mut self) -> Result<Vec<Ts<Named>>, JsError> {
        if self.open().generation() != 4 {
            return Ok(vec![]);
        }
        let ctx = context4().ok_or_else(|| JsError::new("legality data missing"))?;
        list(
            wizard::egg_species(ctx)
                .into_iter()
                .map(|s| named(st::SPECIES, s)),
        )
    }

    /// Generates a legal wild Pokémon and writes it to the empty slot at `box_index` (-1 for the
    /// end of the party) and `slot`. Returns why it failed, or nothing when written.
    #[wasm_bindgen(js_name = addWild)]
    pub fn add_wild(
        &mut self,
        box_index: i32,
        slot: u8,
        req: Ts<WildAdd>,
    ) -> Result<Option<String>, JsError> {
        let r = req.to_rust()?;
        let lead = match r.lead.as_str() {
            "synchronize" => match r.nature {
                Some(n) => Lead::Synchronize(n),
                None => return Ok(Some("Synchronize: choose a nature".into())),
            },
            "cuteCharm" => match r.gender {
                Some(g) => Lead::CuteCharm { gender: g },
                None => return Ok(Some("Cute Charm: choose a gender".into())),
            },
            "radar" => Lead::PokeRadarChain,
            _ => Lead::None,
        };
        let w = wizard::WildRequest {
            version: r.version,
            species: r.species,
            form: r.form,
            encounter: r.encounter,
            level: r.level,
            nature: r.nature,
            gender: r.gender,
            shiny: r.shiny,
            lead,
            ball: r.ball,
            met_date: parse_date(&r.met_date)?,
            seed: r.seed,
        };
        let at = self.empty_slot(box_index, slot)?;
        let gen = self.open().generation();
        let (c4, c5) = if gen == 5 {
            (None, context5())
        } else {
            (context4(), None)
        };
        let mut save = self.open();
        let out = wizard::insert_wild(&mut save, c4, c5, &w, at);
        Ok(self.added(out))
    }

    /// Builds a legal bred Gen 4 egg (or the Pokémon hatched from it) into the empty slot.
    #[wasm_bindgen(js_name = addEgg)]
    pub fn add_egg(
        &mut self,
        box_index: i32,
        slot: u8,
        req: Ts<EggAdd>,
    ) -> Result<Option<String>, JsError> {
        let r = req.to_rust()?;
        let e = wizard::EggRequest {
            version: r.version,
            species: r.species,
            nature: r.nature,
            gender: r.gender,
            shiny: r.shiny,
            hatched: r.hatched,
            date: parse_date(&r.date)?,
            seed: r.seed,
        };
        let at = self.empty_slot(box_index, slot)?;
        let ctx = context4().ok_or_else(|| JsError::new("legality data missing"))?;
        let mut save = self.open();
        let out = wizard::insert_egg(&mut save, ctx, &e, at);
        Ok(self.added(out))
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
        let at = slot_at(box_index, slot);
        if let Slot::Party(i) = at {
            if i as usize >= save.party_count() {
                return Err(JsError::new("no Pokémon there"));
            }
        }
        let pk = save.pkm(at).map_err(err)?;
        if pk.species() == 0 {
            return Err(JsError::new("no Pokémon there"));
        }
        Ok((save.kind(), pk))
    }

    /// The empty slot to add to: a box slot, or the party's next slot.
    fn empty_slot(&mut self, box_index: i32, slot: u8) -> Result<Slot, JsError> {
        let save = self.open();
        let at = if box_index < 0 {
            let n = save.party_count();
            if n >= 6 {
                return Err(JsError::new("the party is full"));
            }
            Slot::Party(n as u8)
        } else {
            Slot::Box(box_index as u8, slot)
        };
        if save.pkm(at).map_err(err)?.species() != 0 {
            return Err(JsError::new("that slot isn't empty"));
        }
        let _ = slot;
        Ok(at)
    }

    fn added(&mut self, out: Result<(), GenStatus>) -> Option<String> {
        match out {
            Ok(()) => {
                self.dirty = true;
                None
            }
            Err(s) => Some(s.text().into()),
        }
    }

    fn check_save(&mut self) -> SaveCheck {
        use verity::safety::SaveIssue;
        let save = self.open();
        let mut c = SaveCheck {
            ok: false,
            issue: None,
            box_index: None,
            slot: None,
        };
        let Some(rules) = rules(save.kind()) else {
            c.issue = Some("validator data missing".into());
            return c;
        };
        match safety::check_save(&rules, &save) {
            Ok(()) => c.ok = true,
            Err(SaveIssue::PartyCount) => c.issue = Some("party count out of range".into()),
            Err(SaveIssue::NoUsablePartyMember) => {
                c.issue = Some("no party Pokémon that can battle".into())
            }
            Err(SaveIssue::Checksums) => c.issue = Some("checksums".into()),
            Err(SaveIssue::Pkm(at, why)) => {
                c.issue = Some(why.text().into());
                (c.box_index, c.slot) = match at {
                    Slot::Party(i) => (None, Some(i)),
                    Slot::Box(b, s) => (Some(b), Some(s)),
                };
            }
        }
        c
    }

    fn edited(
        &mut self,
        box_index: i32,
        slot: u8,
        req: &EditRequest,
    ) -> Result<(Pkm, Rules<'static>), JsError> {
        let (kind, pk) = self.pkm(box_index, slot)?;
        let rules = rules(kind).ok_or_else(|| JsError::new("game data missing"))?;
        let e = req.to_edit(&pk)?;
        let names = match e.nickname {
            Some(Nickname::Reset) => Some(
                &context4()
                    .ok_or_else(|| JsError::new("legality data missing"))?
                    .names4,
            ),
            _ => None,
        };
        let save = self.open();
        let pk = edit::edited(&save, &rules, slot_at(box_index, slot), &e, names)
            .ok_or_else(|| JsError::new("no Pokémon there"))?;
        Ok((pk, rules))
    }
}

/// Spreads tested per lead (as on the DS).
const CLOSEST_IV_BUDGET: u32 = 30_000;

fn slot_at(box_index: i32, slot: u8) -> Slot {
    if box_index < 0 {
        Slot::Party(slot)
    } else {
        Slot::Box(box_index as u8, slot)
    }
}

fn rules(kind: SaveKind) -> Option<Rules<'static>> {
    let moves = blob(if matches!(kind, SaveKind::BW | SaveKind::B2W2) {
        "moves5"
    } else {
        "moves4"
    })
    .and_then(|b| verity::data::MoveData::load(b).ok())?;
    Some(Rules {
        kind,
        game: game_data(kind)?,
        moves,
    })
}

/// The legality verdict for `pk` in a save of `gen`.
fn judge(gen: u8, pk: &Pkm) -> Legality {
    let c5 = if gen == 5 { context5() } else { None };
    match verdict::check(gen, pk, context4(), c5) {
        Ok(r) => legality(pk, &r),
        Err(why) => Legality {
            status: "unchecked".into(),
            reason: Some(why.text().into()),
            checks: vec![],
            moves: vec![],
            origin: None,
        },
    }
}

/// Every named met location of a generation, in id order.
fn locations(gen: u8) -> Vec<Named> {
    let Some(s) = strings() else {
        return vec![];
    };
    let banks: &[(u16, u16)] = if gen == 4 {
        &[
            (st::LOC4_0, 0),
            (st::LOC4_2000, 2000),
            (st::LOC4_3000, 3000),
        ]
    } else {
        &[
            (st::LOC5_0, 0),
            (st::LOC5_30000, 30000),
            (st::LOC5_40000, 40000),
            (st::LOC5_60000, 60000),
        ]
    };
    let mut out = vec![];
    for &(cat, base) in banks {
        for i in 0..s.count(cat) {
            let id = base + i as u16;
            if let Some(name) = utf16(s.location(gen, id)) {
                out.push(Named {
                    id,
                    name: Some(name),
                });
            }
        }
    }
    out
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
        ribbons: {
            let r = pk.ribbons();
            (0..edit::RIBBONS.len())
                .map(|i| {
                    let (byte, bit) = edit::ribbon_pos(i);
                    r[byte] & (1 << bit) != 0
                })
                .collect()
        },
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
