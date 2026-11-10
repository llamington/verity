//! Move tables from the owner's ROMs (skip a missing ROM), checked against PKHeX's PP tables.

use std::fs::File;
use std::os::unix::fs::FileExt;
use std::path::PathBuf;

use verity_rom::moves::{self, CachedMoves, Category};
use verity_rom::nds::Rom;
use verity_rom::{Error, Source};

struct F(File);

impl Source for F {
    fn read_at(&mut self, offset: u32, buf: &mut [u8]) -> Result<(), Error> {
        self.0
            .read_exact_at(buf, offset.into())
            .map_err(|_| Error::Io)
    }
}

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn rom(name: &str) -> Option<Rom<F>> {
    let f = File::open(root().join("roms").join(name)).ok()?;
    Some(Rom::open(F(f)).unwrap())
}

/// PKHeX's base PP per move (data/exports, gitignored; None when not built).
fn pkhex_pp(generation: u8) -> Option<Vec<u8>> {
    let text = std::fs::read_to_string(root().join(format!("data/exports/moves{generation}.json")))
        .ok()?;
    let v: serde_json::Value = serde_json::from_str(&text).ok()?;
    Some(
        v["moves"]
            .as_array()?
            .iter()
            .map(|m| m["pp"].as_u64().unwrap() as u8)
            .collect(),
    )
}

#[test]
fn gen4_moves() {
    for name in ["platinum.nds", "heartgold.nds"] {
        let Some(mut r) = rom(name) else { continue };
        let t = moves::load(&mut r).unwrap();
        assert_eq!(t.generation, 4);
        let tackle = t.get(33).unwrap();
        assert_eq!(
            (tackle.category, tackle.power, tackle.accuracy, tackle.pp),
            (Category::Physical, 35, 95, 35)
        );
        let tb = t.get(85).unwrap();
        assert_eq!(
            (tb.category, tb.power, tb.accuracy, tb.pp),
            (Category::Special, 95, 100, 15)
        );
        assert_eq!(t.get(45).unwrap().category, Category::Status, "Growl");
        assert_eq!(t.get(129).unwrap().accuracy, 0, "Swift never misses");
        if let Some(pp) = pkhex_pp(4) {
            for (mv, &want) in pp.iter().enumerate().take(468) {
                assert_eq!(t.get(mv as u16).unwrap().pp, want, "{name} move {mv}");
            }
        }
    }
}

#[test]
fn gen5_moves() {
    for name in ["black2.nds", "white.nds"] {
        let Some(mut r) = rom(name) else { continue };
        let t = moves::load(&mut r).unwrap();
        assert_eq!(t.generation, 5);
        let tackle = t.get(33).unwrap();
        assert_eq!(
            (tackle.category, tackle.power, tackle.accuracy),
            (Category::Physical, 50, 100)
        );
        assert_eq!(t.get(129).unwrap().accuracy, 101, "Swift never misses");
        if let Some(pp) = pkhex_pp(5) {
            for (mv, &want) in pp.iter().enumerate().take(t.len()) {
                assert_eq!(t.get(mv as u16).unwrap().pp, want, "{name} move {mv}");
            }
        }
        let cache = t.to_cache(r.game_code());
        let c = CachedMoves::parse(&cache).unwrap();
        assert_eq!((c.generation, c.len()), (5, t.len()));
        for mv in 0..t.len() as u16 {
            assert_eq!(c.get(mv), t.get(mv));
        }
        assert!(c.get(t.len() as u16).is_none());
        assert!(CachedMoves::parse(&cache[..cache.len() - 1]).is_err());
    }
}
