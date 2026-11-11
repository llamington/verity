// SPDX-License-Identifier: GPL-3.0-only
//! Browser bindings for the verity core (PLAN P12).
//!
//! The WASM module holds code only. The page fetches the data blobs it needs
//! (`data/blobs/*.bin`) and hands them over with [`Session::attach`].

use serde::Serialize;
use tsify::{Ts, Tsify};
use wasm_bindgen::prelude::*;

use verity::data::{string_table, GameData, Strings};
use verity::pkm::Pkm;
use verity::save::{Save, SaveKind, Slot, BOX_SLOTS};
use verity::text;

/// What the page shows for one Pokémon.
#[derive(Serialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct Mon {
    pub species: u16,
    /// From the strings blob; `None` until it is attached.
    pub species_name: Option<String>,
    pub nickname: String,
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
    pub badges: u8,
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

/// An open save plus the data blobs attached so far.
#[wasm_bindgen]
pub struct Session {
    save: Vec<u8>,
    game: Option<Vec<u8>>,
    strings: Option<Vec<u8>>,
}

#[wasm_bindgen]
impl Session {
    /// Opens a raw 512 KB save or a DeSmuME `.dsv`.
    #[wasm_bindgen(constructor)]
    pub fn new(bytes: Vec<u8>) -> Result<Session, JsError> {
        let mut save = bytes;
        Save::open(&mut save).map_err(err)?;
        Ok(Session { save, game: None, strings: None })
    }

    /// The data blob this save's game needs (fetch `data/<name>.bin`).
    #[wasm_bindgen(js_name = gameBlob)]
    pub fn game_blob(&mut self) -> String {
        game_id(self.open().kind()).into()
    }

    /// Attaches a fetched blob: `strings` (a `strings_<lang>.bin`) or the game blob.
    pub fn attach(&mut self, which: &str, bytes: Vec<u8>) -> Result<(), JsError> {
        match which {
            "strings" => {
                Strings::load(&bytes).map_err(err)?;
                self.strings = Some(bytes);
            }
            "game" => {
                GameData::load(&bytes).map_err(err)?;
                self.game = Some(bytes);
            }
            _ => return Err(JsError::new("unknown blob")),
        }
        Ok(())
    }

    pub fn overview(&mut self) -> Result<Ts<Overview>, JsError> {
        let (game, strings) = (self.game.clone(), self.strings.clone());
        let ctx = Ctx::new(game.as_deref(), strings.as_deref());
        let save = self.open();
        let t = save.trainer();
        let gen = save.generation();
        let party = (0..save.party_count())
            .filter_map(|i| save.pkm(Slot::Party(i as u8)).ok())
            .map(|pk| ctx.mon(&pk, gen))
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
                badges: t.badges,
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
        let (game, strings) = (self.game.clone(), self.strings.clone());
        let ctx = Ctx::new(game.as_deref(), strings.as_deref());
        let save = self.open();
        let gen = save.generation();
        let name = decode(&save.box_name_raw(index as usize).map_err(err)?, gen);
        let mut slots = Vec::with_capacity(BOX_SLOTS);
        for s in 0..BOX_SLOTS as u8 {
            let pk = save.pkm(Slot::Box(index, s)).map_err(err)?;
            slots.push((pk.species() != 0).then(|| ctx.mon(&pk, gen)));
        }
        Ok(BoxView { name, slots }.into_ts()?)
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
}

/// Lookups that work with whichever blobs are attached.
struct Ctx<'a> {
    game: Option<GameData<'a>>,
    strings: Option<Strings<'a>>,
}

impl<'a> Ctx<'a> {
    fn new(game: Option<&'a [u8]>, strings: Option<&'a [u8]>) -> Self {
        Ctx {
            game: game.and_then(|b| GameData::load(b).ok()),
            strings: strings.and_then(|b| Strings::load(b).ok()),
        }
    }

    fn mon(&self, pk: &Pkm, gen: u8) -> Mon {
        let species = pk.species();
        let species_name = self.strings.as_ref().and_then(|s| {
            ((species as usize) < s.count(string_table::SPECIES))
                .then(|| String::from_utf16_lossy(&s.get(string_table::SPECIES, species as usize).iter().collect::<Vec<_>>()))
        });
        let level = self.game.as_ref().and_then(|g| {
            (species <= g.max_species())
                .then(|| pk.level(&g.species_form(species, pk.form()).base_info()))
        });
        Mon {
            species,
            species_name,
            nickname: decode(&pk.nickname_raw(), gen),
            level,
            form: pk.form(),
            egg: pk.is_egg(),
            shiny: pk.is_shiny(),
            checksum_ok: pk.checksum_valid(),
        }
    }
}

fn decode(raw: &[u16], gen: u8) -> String {
    let mut out = [0u16; 32];
    let n = if gen == 4 { text::decode4(raw, &mut out) } else { text::decode5(raw, &mut out) };
    String::from_utf16_lossy(&out[..n])
}
