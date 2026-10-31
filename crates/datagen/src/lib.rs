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

pub fn build_game(j: &Value) -> Vec<u8> {
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

pub const STRING_CATEGORIES: [(u16, &str); 7] = [
    (st::SPECIES, "species"),
    (st::MOVES, "moves"),
    (st::ABILITIES, "abilities"),
    (st::NATURES, "natures"),
    (st::TYPES, "types"),
    (st::ITEMS4, "items4"),
    (st::ITEMS5, "items5"),
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

/// Builds every blob from `exports` into `out`; returns (name, size) pairs.
pub fn build_all(exports: &Path, out: &Path) -> Vec<(String, usize)> {
    fs::create_dir_all(out).unwrap();
    let mut written = Vec::new();
    let mut put = |name: String, bytes: Vec<u8>| {
        fs::write(out.join(&name), &bytes).unwrap();
        written.push((name, bytes.len()));
    };
    for g in GAMES {
        put(
            format!("{g}.bin"),
            build_game(&load(&exports.join(format!("{g}.json")))),
        );
    }
    let tables = load(&exports.join("move_tables.json"));
    for gen in [4, 5] {
        put(
            format!("moves{gen}.bin"),
            build_moves(&load(&exports.join(format!("moves{gen}.json"))), &tables),
        );
    }
    for l in LANGS {
        put(
            format!("strings_{l}.bin"),
            build_strings(&load(&exports.join(format!("strings_{l}.json")))),
        );
    }
    written
}
