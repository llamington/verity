//! Converts `pkhex-oracle export-data` JSON into the blobs read by `verity::data`.

use std::fs;
use std::path::Path;

use serde_json::Value;
use verity::data::blob::{BlobKind, DIR_ENTRY_LEN, HEADER_LEN, MAGIC, VERSION};
use verity::data::{game_table as gt, move_table as mt, string_table as st};

pub const GAMES: [&str; 5] = ["dp", "pt", "hgss", "bw", "b2w2"];
pub const LANGS: [&str; 7] = ["ja", "en", "fr", "it", "de", "es", "ko"];

pub struct BlobWriter {
    generation: u8,
    kind: BlobKind,
    tables: Vec<(u16, u16, Vec<u8>)>,
}

impl BlobWriter {
    pub fn new(generation: u8, kind: BlobKind) -> Self {
        Self {
            generation,
            kind,
            tables: Vec::new(),
        }
    }

    /// A writer holding every table of an already built blob.
    pub fn reopen(data: &[u8]) -> Self {
        let b = verity::data::blob::Blob::parse(data).unwrap();
        let mut w = Self::new(b.generation(), b.kind_raw().try_into().unwrap());
        for (id, elem, bytes) in b.tables() {
            w.table(id, elem, bytes.to_vec());
        }
        w
    }

    pub fn table(&mut self, id: u16, elem_size: u16, bytes: Vec<u8>) {
        self.tables.push((id, elem_size, bytes));
    }

    pub fn u16s(&mut self, id: u16, values: &[u16]) {
        self.table(id, 2, values.iter().flat_map(|v| v.to_le_bytes()).collect());
    }

    /// `count + 1` u32 offsets (relative to the table) then the records.
    pub fn indexed(&mut self, id: u16, records: &[Vec<u8>]) {
        let head = 4 * (records.len() + 1);
        let mut out = Vec::new();
        let mut ofs = head;
        for r in records {
            out.extend((ofs as u32).to_le_bytes());
            ofs += r.len();
        }
        out.extend((ofs as u32).to_le_bytes());
        for r in records {
            out.extend(r);
        }
        self.table(id, 0, out);
    }

    pub fn finish(self) -> Vec<u8> {
        let dir_end = HEADER_LEN + DIR_ENTRY_LEN * self.tables.len();
        let mut ofs = dir_end;
        let mut dir = Vec::new();
        for (id, elem, bytes) in &self.tables {
            ofs = (ofs + 3) & !3;
            dir.extend(id.to_le_bytes());
            dir.extend(elem.to_le_bytes());
            dir.extend((ofs as u32).to_le_bytes());
            dir.extend((bytes.len() as u32).to_le_bytes());
            ofs += bytes.len();
        }
        let mut out = Vec::with_capacity(ofs);
        out.extend(MAGIC);
        out.extend(VERSION.to_le_bytes());
        out.push(self.generation);
        out.push(self.kind as u8);
        out.extend((self.tables.len() as u16).to_le_bytes());
        out.extend(0u16.to_le_bytes());
        out.extend((ofs as u32).to_le_bytes());
        out.extend(dir);
        for (_, _, bytes) in &self.tables {
            while out.len() % 4 != 0 {
                out.push(0);
            }
            out.extend(bytes);
        }
        out
    }
}

pub fn load(path: &Path) -> Value {
    serde_json::from_slice(&fs::read(path).unwrap_or_else(|e| panic!("{}: {e}", path.display())))
        .unwrap()
}

fn ints(v: &Value) -> Vec<i64> {
    v.as_array()
        .map(|a| a.iter().map(|x| x.as_i64().unwrap()).collect())
        .unwrap_or_default()
}

pub fn build_game(j: &Value, evolutions: &Value) -> Vec<u8> {
    let generation = j["generation"].as_u64().unwrap() as u8;
    let mut w = BlobWriter::new(generation, BlobKind::Game);
    let size = j["personal_size"].as_u64().unwrap() as u16;
    let personal: Vec<u8> = j["personal"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|e| ints(e).into_iter().map(|b| b as u8))
        .collect();
    let count = personal.len() / size as usize;
    w.table(gt::PERSONAL, size, personal);
    w.u16s(
        gt::MAX_SPECIES,
        &[j["max_species"].as_u64().unwrap() as u16],
    );

    if let Some(t) = j["type_tutors"].as_array() {
        if t.iter().any(|e| !ints(e).is_empty()) {
            let mut bits = vec![0u8; count * 8];
            for (i, e) in t.iter().enumerate() {
                for (b, v) in ints(e).iter().enumerate() {
                    if *v != 0 && b < 64 {
                        bits[i * 8 + b / 8] |= 1 << (b % 8);
                    }
                }
            }
            w.table(gt::TYPE_TUTORS, 8, bits);
        }
    }

    let mut levelup = vec![Vec::new(); count];
    for e in j["levelup"].as_array().unwrap() {
        let idx = e["index"].as_u64().unwrap() as usize;
        let rec = &mut levelup[idx];
        for m in e["moves"].as_array().unwrap() {
            let m = ints(m);
            rec.extend((m[1] as u16).to_le_bytes());
            rec.push(m[0] as u8);
            rec.push(0);
        }
    }
    w.indexed(gt::LEVELUP, &levelup);

    let mut eggs = vec![Vec::new(); count];
    let forms: Vec<Vec<i64>> = j["forms"].as_array().unwrap().iter().map(ints).collect();
    for e in j["egg_moves"].as_array().unwrap() {
        let (sp, f) = (e["species"].as_i64().unwrap(), e["form"].as_i64().unwrap());
        let idx = forms
            .iter()
            .find(|x| x[0] == sp && x[1] == f)
            .map(|x| x[2] as usize)
            .unwrap();
        if eggs[idx].is_empty() {
            eggs[idx] = ints(&e["moves"])
                .iter()
                .flat_map(|m| (*m as u16).to_le_bytes())
                .collect();
        }
    }
    w.indexed(gt::EGG_MOVES, &eggs);
    let held: Vec<u16> = ints(&j["held_items"]).iter().map(|v| *v as u16).collect();
    w.u16s(gt::HELD_ITEMS, &held);

    let mut evos = vec![Vec::new(); count];
    for e in evolutions["evolutions"].as_array().unwrap() {
        let (sp, f) = (e["species"].as_i64().unwrap(), e["form"].as_i64().unwrap());
        let Some(idx) = forms
            .iter()
            .find(|x| x[0] == sp && x[1] == f)
            .map(|x| x[2] as usize)
        else {
            continue;
        };
        if !evos[idx].is_empty() {
            continue;
        }
        for m in e["evolutions"].as_array().unwrap() {
            let rec = &mut evos[idx];
            rec.extend((m["Species"].as_u64().unwrap() as u16).to_le_bytes());
            rec.extend((m["Argument"].as_u64().unwrap() as u16).to_le_bytes());
            rec.push(m["Form"].as_u64().unwrap() as u8);
            rec.push(m["method"].as_u64().unwrap() as u8);
            rec.push(m["Level"].as_u64().unwrap() as u8);
            rec.push(m["LevelUp"].as_u64().unwrap() as u8);
        }
    }
    w.indexed(gt::EVOLUTIONS, &evos);
    w.finish()
}

pub fn build_moves(j: &Value, tables: &Value) -> Vec<u8> {
    let generation = j["generation"].as_u64().unwrap() as u8;
    let mut w = BlobWriter::new(generation, BlobKind::Moves);
    let moves = j["moves"].as_array().unwrap();
    w.table(
        mt::PP,
        1,
        moves
            .iter()
            .map(|m| m["pp"].as_u64().unwrap() as u8)
            .collect(),
    );
    w.table(
        mt::TYPE,
        1,
        moves
            .iter()
            .map(|m| m["type"].as_u64().unwrap() as u8)
            .collect(),
    );
    let list = |name: &str| -> Vec<u16> { ints(&tables[name]).iter().map(|v| *v as u16).collect() };
    w.u16s(
        mt::RELEASED_HELD,
        &list(if generation == 4 {
            "g4_released_held"
        } else {
            "g5_released_held"
        }),
    );
    if generation == 4 {
        for (id, name) in [
            (mt::G4_TUTOR, "g4_tutor_moves"),
            (mt::G4_TM, "g4_tm"),
            (mt::G4_HM_DPPT, "g4_hm_dppt"),
            (mt::G4_HM_HGSS, "g4_hm_hgss"),
            (mt::G4_SPECIAL_BLAST_BURN, "g4_special_blast_burn"),
            (mt::G4_SPECIAL_HYDRO_CANNON, "g4_special_hydro_cannon"),
            (mt::G4_SPECIAL_FRENZY_PLANT, "g4_special_frenzy_plant"),
            (mt::G4_SPECIAL_DRACO_METEOR, "g4_special_draco_meteor"),
        ] {
            w.u16s(id, &list(name));
        }
    } else {
        w.u16s(mt::G5_TMHM, &list("g5_tmhm"));
        w.u16s(mt::G5_TYPE_TUTOR, &list("g5_type_tutor_moves"));
        let b2 = tables["b2w2_tutor_moves"].as_array().unwrap();
        for (i, id) in [
            mt::B2W2_TUTOR_1,
            mt::B2W2_TUTOR_2,
            mt::B2W2_TUTOR_3,
            mt::B2W2_TUTOR_4,
        ]
        .into_iter()
        .enumerate()
        {
            let v: Vec<u16> = ints(&b2[i]).iter().map(|v| *v as u16).collect();
            w.u16s(id, &v);
        }
    }
    w.finish()
}

pub const STRING_CATEGORIES: [(u16, &str); 14] = [
    (st::SPECIES, "species"),
    (st::MOVES, "moves"),
    (st::ABILITIES, "abilities"),
    (st::NATURES, "natures"),
    (st::TYPES, "types"),
    (st::ITEMS4, "items4"),
    (st::ITEMS5, "items5"),
    (st::LOC4_0, "loc4_0"),
    (st::LOC4_2000, "loc4_2000"),
    (st::LOC4_3000, "loc4_3000"),
    (st::LOC5_0, "loc5_0"),
    (st::LOC5_30000, "loc5_30000"),
    (st::LOC5_40000, "loc5_40000"),
    (st::LOC5_60000, "loc5_60000"),
];

pub fn build_strings(j: &Value) -> Vec<u8> {
    let mut w = BlobWriter::new(0, BlobKind::Strings);
    for (id, name) in STRING_CATEGORIES {
        let recs: Vec<Vec<u8>> = j[name]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| {
                s.as_str()
                    .unwrap()
                    .encode_utf16()
                    .flat_map(|u| u.to_le_bytes())
                    .collect()
            })
            .collect();
        w.indexed(id, &recs);
    }
    w.finish()
}

pub const ENCOUNTER_SOURCES: [(&str, &str); 5] = [
    ("dppt", "Encounters4DPPt"),
    ("hgss", "Encounters4HGSS"),
    ("bw", "Encounters5BW"),
    ("b2w2", "Encounters5B2W2"),
    ("dr", "Encounters5DR"),
];

fn num(v: &Value, key: &str, default: i64) -> i64 {
    match &v[key] {
        Value::Bool(b) => *b as i64,
        Value::Number(n) => n.as_i64().unwrap(),
        _ => default,
    }
}

fn truthy(v: &Value, key: &str) -> bool {
    v[key].as_bool().unwrap_or(false)
}

/// Encodes one PKHeX template (exported by `Describe`) as a 68-byte record.
pub fn encode_template(t: &Value, area: Option<(u16, &Value)>) -> Vec<u8> {
    use verity::data::encounters::{flag, EncounterKind, NO_AREA, TEMPLATE_LEN};
    let kind = EncounterKind::from_pkhex_name(t["kind"].as_str().unwrap())
        .unwrap_or_else(|| panic!("unknown kind {}", t["kind"]));
    let mut r = vec![0u8; TEMPLATE_LEN];
    let put16 =
        |r: &mut Vec<u8>, o: usize, v: i64| r[o..o + 2].copy_from_slice(&(v as u16).to_le_bytes());
    let put32 =
        |r: &mut Vec<u8>, o: usize, v: i64| r[o..o + 4].copy_from_slice(&(v as u32).to_le_bytes());
    r[0] = kind as u8;
    r[1] = num(t, "Version", 0) as u8;
    put16(&mut r, 2, num(t, "Species", 0));
    r[4] = num(t, "Form", 0) as u8;
    r[5] = num(t, "LevelMin", 0) as u8;
    r[6] = num(t, "LevelMax", 0) as u8;
    r[7] = num(t, "Ability", 0) as u8;
    r[8] = num(t, "Shiny", 0) as u8;
    r[9] = num(t, "Gender", 255) as u8;
    r[10] = num(t, "Nature", 25) as u8;
    r[11] = num(t, "FixedBall", 0) as u8;
    put16(&mut r, 12, num(t, "Location", 0));
    put16(&mut r, 14, num(t, "EggLocation", 0));
    put16(&mut r, 16, num(t, "HeldItem", 0));
    let mut flags = 0u16;
    for (key, f) in [
        ("FatefulEncounter", flag::FATEFUL),
        ("IsEgg", flag::EGG),
        ("IsRoaming", flag::ROAMING),
        ("IsWildCorrelationPID", flag::WILD_CORRELATION_PID),
        ("IsHiddenGrotto", flag::HIDDEN_GROTTO),
        ("IsFixedTrainer", flag::FIXED_TRAINER),
        ("IsFixedNickname", flag::FIXED_NICKNAME),
        ("IsLanguageSwap", flag::LANGUAGE_SWAP),
        ("IsRandomUnspecificForm", flag::RANDOM_UNSPECIFIC_FORM),
    ] {
        if truthy(t, key) {
            flags |= f;
        }
    }
    if t["Moves"]["HasMoves"].as_bool().unwrap_or(false) {
        flags |= flag::HAS_MOVES;
    }
    if t["IVs"]["IsSpecified"].as_bool().unwrap_or(false) {
        flags |= flag::HAS_IVS;
    }
    if t.get("PID").is_some() {
        flags |= flag::HAS_PID;
    }
    if let Some((_, a)) = area {
        if truthy(a, "IsCoronetFeebasArea") {
            flags |= flag::CORONET_FEEBAS;
        }
    }
    if truthy(t, "RibbonNational") {
        flags |= flag::RIBBON_NATIONAL;
    }
    if truthy(t, "IsJapaneseBonusDisk") {
        flags |= flag::JAPANESE_BONUS_DISK;
    }
    put16(&mut r, 18, flags as i64);
    for (i, k) in ["Move1", "Move2", "Move3", "Move4"].iter().enumerate() {
        put16(&mut r, 20 + 2 * i, num(&t["Moves"], k, 0));
    }
    put32(&mut r, 28, num(t, "GroundTile", 0));
    put32(&mut r, 32, num(t, "PID", 0));
    put16(&mut r, 36, num(t, "TID16", 0));
    put16(&mut r, 38, num(t, "SID16", 0));
    r[40] = num(t, "OTGender", num(t, "OriginalTrainerGender", 0)) as u8;
    r[41] = num(t, "MetLevel", 0) as u8;
    put16(&mut r, 42, num(t, "MetLocation", 0));
    for (i, k) in ["HP", "ATK", "DEF", "SPE", "SPA", "SPD"].iter().enumerate() {
        r[44 + i] = num(&t["IVs"], k, 0) as u8;
    }
    for (i, k) in [
        "ContestCool",
        "ContestBeauty",
        "ContestCute",
        "ContestSmart",
        "ContestTough",
        "ContestSheen",
    ]
    .iter()
    .enumerate()
    {
        r[50 + i] = num(t, k, 0) as u8;
    }
    r[56] = num(t, "Course", 0) as u8;
    r[57] = num(t, "SlotNumber", 0) as u8;
    r[58] = num(t, "MagnetPullIndex", 0) as u8;
    r[59] = num(t, "MagnetPullCount", 0) as u8;
    r[60] = num(t, "StaticIndex", 0) as u8;
    r[61] = num(t, "StaticCount", 0) as u8;
    r[62] = num(t, "Type", 0) as u8;
    r[63] = num(t, "AreaRate", 0) as u8;
    if matches!(
        kind,
        EncounterKind::Gift3
            | EncounterKind::Gift3NY
            | EncounterKind::Gift3JPN
            | EncounterKind::Gift3Colo
    ) {
        r[58] = num(t, "Language", 0) as u8;
        r[59] = num(t, "Distribution", 0) as u8;
        r[66] = num(t, "VersionMask", 0) as u8;
        r[67] = num(t, "Method", 0) as u8;
    }
    put16(&mut r, 64, area.map_or(NO_AREA as i64, |(i, _)| i as i64));
    r
}

fn encode_slot(t: &Value, area: u16) -> Vec<u8> {
    use verity::data::encounters::{EncounterKind, SLOT_LEN};
    let kind = EncounterKind::from_pkhex_name(t["kind"].as_str().unwrap()).unwrap();
    let mut r = vec![0u8; SLOT_LEN];
    r[0] = kind as u8;
    r[1] = num(t, "Form", 0) as u8;
    r[2..4].copy_from_slice(&(num(t, "Species", 0) as u16).to_le_bytes());
    r[4] = num(t, "LevelMin", 0) as u8;
    r[5] = num(t, "LevelMax", 0) as u8;
    r[6] = num(t, "SlotNumber", 0) as u8;
    r[7] = num(t, "MagnetPullIndex", 0) as u8;
    r[8] = num(t, "MagnetPullCount", 0) as u8;
    r[9] = num(t, "StaticIndex", 0) as u8;
    r[10] = num(t, "StaticCount", 0) as u8;
    use verity::data::encounters::{SLOT_CAN_RADAR, SLOT_HIDDEN_GROTTO, SLOT_RANDOM_FORM};
    let mut f = 0u8;
    if truthy(t, "IsRandomUnspecificForm") {
        f |= SLOT_RANDOM_FORM;
    }
    if truthy(t, "IsHiddenGrotto") {
        f |= SLOT_HIDDEN_GROTTO;
    }
    if truthy(t, "CanUseRadar") {
        f |= SLOT_CAN_RADAR;
    }
    r[11] = f;
    r[12..14].copy_from_slice(&area.to_le_bytes());
    r[14] = num(t, "Type", 0) as u8;
    r[15] = num(t, "FixedBall", 0) as u8;
    r[16] = num(t, "Ability", 0) as u8;
    r[17] = num(t, "Shiny", 0) as u8;
    r[18] = num(t, "PressureLevel", num(t, "LevelMax", 0)) as u8;
    r
}

/// Gen 3 origin data in one blob: Emerald's game tables, the other games' level-up moves and
/// the encounter blob as an embedded table.
pub fn build_g3_origin(j: &Value, evolutions: &Value, encounters: &Value) -> Vec<u8> {
    let game = build_game(j, evolutions);
    let enc = build_encounters(encounters);
    let mut w = BlobWriter::reopen(&game);
    let count = j["personal"].as_array().unwrap().len();
    for (key, id) in [
        ("levelup_rs", gt::LEVELUP_RS),
        ("levelup_fr", gt::LEVELUP_FR),
        ("levelup_lg", gt::LEVELUP_LG),
    ] {
        let mut levelup = vec![Vec::new(); count];
        for e in j[key].as_array().unwrap() {
            let rec = &mut levelup[e["index"].as_u64().unwrap() as usize];
            for m in e["moves"].as_array().unwrap() {
                let m = ints(m);
                rec.extend((m[1] as u16).to_le_bytes());
                rec.push(m[0] as u8);
                rec.push(0);
            }
        }
        w.indexed(id, &levelup);
    }
    w.table(gt::ENCOUNTERS, 0, enc);
    w.finish()
}

pub fn build_encounters(j: &Value) -> Vec<u8> {
    use verity::data::encounters::{table, AREA_LEN, RANGE_SLOTS, SLOT_LEN, TEMPLATE_LEN};
    let mut templates = Vec::new();
    let mut slots = Vec::new();
    let mut areas = Vec::new();
    let mut names = Vec::new();
    let mut ranges = Vec::new();
    let mut trade_names: Vec<Vec<u8>> = Vec::new();
    let mut swarm_moves: Vec<u8> = Vec::new();
    let mut generation = 0;
    for (name, group) in j.as_object().unwrap() {
        let items = group["items"].as_array().unwrap();
        let (start, count) = if group["type"] == "areas" {
            let start = slots.len() / SLOT_LEN;
            for a in items {
                let ai = (areas.len() / AREA_LEN) as u16;
                let mut rec = vec![0u8; AREA_LEN];
                rec[0..2].copy_from_slice(&(num(a, "Location", 0) as u16).to_le_bytes());
                rec[2] = num(a, "Type", 0) as u8;
                rec[3] = num(a, "Rate", 0) as u8;
                rec[4..8].copy_from_slice(&(num(a, "GroundTile", 0) as u32).to_le_bytes());
                rec[8] = num(a, "Version", 0) as u8;
                let f = if truthy(a, "IsCoronetFeebasArea") {
                    verity::data::encounters::flag::CORONET_FEEBAS
                } else {
                    0
                };
                rec[9..11].copy_from_slice(&f.to_le_bytes());
                areas.extend(rec);
                for slot in a["slots"].as_array().unwrap() {
                    generation = num(slot, "Generation", generation);
                    let mut rec = encode_slot(slot, ai);
                    if slot["kind"] == "EncounterSlot3Swarm" {
                        rec[19] = (swarm_moves.len() / 8) as u8;
                        for k in ["Move1", "Move2", "Move3", "Move4"] {
                            swarm_moves.extend((num(&slot["Moves"], k, 0) as u16).to_le_bytes());
                        }
                    }
                    slots.extend(rec);
                }
            }
            (start, (slots.len() / SLOT_LEN - start) as u32 | RANGE_SLOTS)
        } else {
            let start = templates.len() / TEMPLATE_LEN;
            for t in items {
                generation = num(t, "Generation", generation);
                templates.extend(encode_template(t, None));
                trade_names.push(encode_trade_names(t));
            }
            (start, (templates.len() / TEMPLATE_LEN - start) as u32)
        };
        names.push(name.as_bytes().to_vec());
        ranges.extend((start as u32).to_le_bytes());
        ranges.extend(count.to_le_bytes());
    }
    let mut w = BlobWriter::new(generation as u8, BlobKind::Encounters);
    w.table(table::TEMPLATES, TEMPLATE_LEN as u16, templates);
    w.table(table::SLOTS, SLOT_LEN as u16, slots);
    w.table(table::AREAS, AREA_LEN as u16, areas);
    w.indexed(table::GROUP_NAMES, &names);
    w.table(table::GROUP_RANGES, 8, ranges);
    w.indexed(table::TRADE_NAMES, &trade_names);
    if !swarm_moves.is_empty() {
        w.table(table::SWARM_MOVES, 8, swarm_moves);
    }
    w.finish()
}

/// Per language: nickname then OT name, each as a u8 length (UTF-16 units) and UTF-16 LE.
/// Empty for templates without fixed trade names.
fn encode_trade_names(t: &Value) -> Vec<u8> {
    if let Some(name) = t["OriginalTrainerName"].as_str() {
        // Gen 3 event gift: the OT in language slot 0, no nickname.
        let s: Vec<u16> = name.encode_utf16().collect();
        let mut out = vec![0u8, s.len() as u8];
        out.extend(s.iter().flat_map(|u| u.to_le_bytes()));
        return out;
    }
    let Some(ot) = t["TrainerNames"].as_array() else {
        return Vec::new();
    };
    let empty = Vec::new();
    let nick = t["Nicknames"].as_array().unwrap_or(&empty);
    let mut out = Vec::new();
    for i in 0..nick.len().max(ot.len()) {
        for list in [nick, ot] {
            let s: Vec<u16> = list
                .get(i)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .encode_utf16()
                .collect();
            out.push(s.len() as u8);
            out.extend(s.iter().flat_map(|u| u.to_le_bytes()));
        }
    }
    out
}

/// Gen 4-formatted species names (PKHeX `SpeciesName.GetSpeciesNameGeneration(_, lang, 4)`);
/// the table id is the language ID.
pub fn build_names4(j: &Value) -> Vec<u8> {
    let mut w = BlobWriter::new(4, BlobKind::Strings);
    for (lang, names) in j.as_object().unwrap() {
        let recs: Vec<Vec<u8>> = names
            .as_array()
            .unwrap()
            .iter()
            .map(|s| {
                s.as_str()
                    .unwrap()
                    .encode_utf16()
                    .flat_map(|u| u.to_le_bytes())
                    .collect()
            })
            .collect();
        w.indexed(lang.parse().unwrap(), &recs);
    }
    w.finish()
}

/// M6: the Gen 4 event database (`gifts4.json`, PKHeX `MGDB_G4`) as table 1 of fixed-size
/// records: u16 MGDB index, u16 reserved, then the 0x358-byte PCD with its PK4 decrypted.
/// Only Pokémon gifts are kept (items and other card types cannot match an entity).
pub fn build_gifts4(j: &Value, manifest: &str, entity: bool) -> Vec<u8> {
    const REC: usize = 8 + 0x358;
    let meta = parse_events4(manifest);
    let mut w = BlobWriter::new(4, BlobKind::Gifts);
    let mut out = Vec::new();
    let mut rec = |index: u16, m: [u8; 4], raw: &[u8]| {
        assert_eq!(raw.len(), 0x358);
        out.extend(index.to_le_bytes());
        out.extend(m);
        out.extend(0u16.to_le_bytes());
        out.extend(raw);
    };
    let db = j.as_array().unwrap();
    assert_eq!(db.len(), meta.len(), "data/events4.tsv is out of date");
    for g in db {
        let index = g["index"].as_u64().unwrap() as u16;
        let hex = g["raw"].as_str().unwrap();
        let raw: Vec<u8> = (0..hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
            .collect();
        let m = &meta[index as usize];
        assert_eq!(
            (m.card_id, m.fnv64),
            (g["card_id"].as_u64().unwrap() as u16, fnv64(&raw)),
            "data/events4.tsv does not match card {index}; rerun scripts/gen-events4-manifest.py"
        );
        if g["entity"].as_bool().unwrap() == entity {
            rec(index, [m.language, m.region, m.games, m.injectable], &raw);
        }
    }
    if !entity {
        // Ranger Manaphy: a bare type 7 gift with no Wonder Card, any game, any language but
        // Korean (PKHeX `PGT.IsRangerManaphy`).
        let mut raw = [0u8; 0x358];
        raw[0] = 7;
        rec(0xFFFF, [0, 0, 0x1F, 1], &raw);
    }
    w.table(1, REC as u16, out);
    w.finish()
}

struct EventMeta {
    card_id: u16,
    language: u8,
    region: u8,
    games: u8,
    injectable: u8,
    fnv64: u64,
}

/// `data/events4.tsv`, in index order.
fn parse_events4(text: &str) -> Vec<EventMeta> {
    let mut out = Vec::new();
    for line in text.lines().filter(|l| !l.starts_with('#')) {
        let f: Vec<&str> = line.split('\t').collect();
        assert_eq!(f[0].parse::<usize>().unwrap(), out.len());
        out.push(EventMeta {
            card_id: f[1].parse().unwrap(),
            language: f[2].parse().unwrap(),
            region: f[3].parse().unwrap(),
            games: f[4].parse().unwrap(),
            injectable: f[5].parse().unwrap(),
            fnv64: u64::from_str_radix(f[6], 16).unwrap(),
        });
    }
    out
}

fn fnv64(data: &[u8]) -> u64 {
    data.iter().fold(0xCBF2_9CE4_8422_2325, |h, &b| {
        (h ^ b as u64).wrapping_mul(0x0000_0100_0000_01B3)
    })
}

/// Builds every blob from `exports` into `out`; returns (name, size) pairs.
pub fn build_all(exports: &Path, out: &Path) -> Vec<(String, usize)> {
    fs::create_dir_all(out).unwrap();
    let mut written = Vec::new();
    let mut put = |name: String, bytes: Vec<u8>| {
        fs::write(out.join(&name), &bytes).unwrap();
        written.push((name, bytes.len()));
    };
    let evo4 = load(&exports.join("evolutions4.json"));
    let evo5 = load(&exports.join("evolutions5.json"));
    for g in GAMES {
        let evo = if matches!(g, "bw" | "b2w2") {
            &evo5
        } else {
            &evo4
        };
        put(
            format!("{g}.bin"),
            build_game(&load(&exports.join(format!("{g}.json"))), evo),
        );
    }
    let mut enc3 = load(&exports.join("enc_Encounters3RSE.json"));
    let frlg = load(&exports.join("enc_Encounters3FRLG.json"));
    let groups = enc3.as_object_mut().unwrap();
    groups.extend(frlg.as_object().unwrap().clone());
    groups.extend(
        load(&exports.join("enc_EncountersWC3.json"))
            .as_object()
            .unwrap()
            .clone(),
    );
    put(
        "g3_origin.bin".into(),
        build_g3_origin(
            &load(&exports.join("g3.json")),
            &load(&exports.join("evolutions3.json")),
            &enc3,
        ),
    );
    for (key, src) in ENCOUNTER_SOURCES {
        put(
            format!("enc_{key}.bin"),
            build_encounters(&load(&exports.join(format!("enc_{src}.json")))),
        );
    }
    let tables = load(&exports.join("move_tables.json"));
    for gen in [4, 5] {
        put(
            format!("moves{gen}.bin"),
            build_moves(&load(&exports.join(format!("moves{gen}.json"))), &tables),
        );
    }
    put(
        "names4.bin".into(),
        build_names4(&load(&exports.join("names4.json"))),
    );
    let gifts = load(&exports.join("gifts4.json"));
    let events4 = fs::read_to_string(exports.parent().unwrap().join("events4.tsv")).unwrap();
    put("gifts4.bin".into(), build_gifts4(&gifts, &events4, true));
    put("events4.bin".into(), build_gifts4(&gifts, &events4, false));
    for l in LANGS {
        put(
            format!("strings_{l}.bin"),
            build_strings(&load(&exports.join(format!("strings_{l}.json")))),
        );
    }
    written
}
