//! Icon extraction against the owner's ROMs in `roms/` (each test skips a missing ROM).

use std::fs::File;
use std::os::unix::fs::FileExt;
use std::path::PathBuf;

use verity_rom::icons::{self, CachedIcons, IconKey};
use verity_rom::narc::Narc;
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

fn rom(name: &str) -> Option<Rom<F>> {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../roms")
        .join(name);
    let Ok(f) = File::open(&p) else {
        eprintln!("skipping: {} not found", p.display());
        return None;
    };
    Some(Rom::open(F(f)).unwrap())
}

fn key(species: u16, form: u8) -> IconKey {
    IconKey {
        species,
        form,
        female: false,
        egg: false,
    }
}

fn tiles(cache: &[u8], k: IconKey) -> (u8, Vec<u8>) {
    let c = CachedIcons::parse(cache).unwrap();
    let r = c.find(k).unwrap_or_else(|| panic!("{k:?}"));
    (
        r.palette,
        cache[r.tiles_at as usize..r.tiles_at as usize + 512].to_vec(),
    )
}

/// Builds the cache, checks the common facts, and returns it (also written to $VERITY_ICON_DUMP).
fn build(name: &str) -> Option<Vec<u8>> {
    let mut r = rom(name)?;
    let set = icons::load(&mut r).unwrap();
    let cache = set.to_cache(r.game_code());
    if let Ok(dir) = std::env::var("VERITY_ICON_DUMP") {
        std::fs::write(PathBuf::from(dir).join(format!("{name}.vicn")), &cache).unwrap();
    }
    let c = CachedIcons::parse(&cache).unwrap();
    assert_eq!(CachedIcons::head_len(&cache), Some(c.tiles_offset as usize));
    assert!(
        CachedIcons::parse(&cache[..c.tiles_offset as usize]).is_ok(),
        "the head alone parses"
    );
    assert!(cache.len() < 512 * 1024, "{} bytes", cache.len());
    // Bulbasaur line palette 1, Charmander 0, Blastoise 2 (the anchor itself).
    assert_eq!([1, 4, 9].map(|s| tiles(&cache, key(s, 0)).0), [1, 0, 2]);
    let egg = tiles(
        &cache,
        IconKey {
            egg: true,
            ..key(25, 0)
        },
    );
    let manaphy_egg = tiles(
        &cache,
        IconKey {
            egg: true,
            ..key(490, 0)
        },
    );
    assert_ne!(egg.1, manaphy_egg.1);
    assert_eq!((egg.0, manaphy_egg.0), (1, 2), "egg palettes");
    assert_ne!(egg.1, tiles(&cache, key(25, 0)).1);
    // Forms have their own icons; a form without one falls back to the base form.
    assert_ne!(
        tiles(&cache, key(479, 1)).1,
        tiles(&cache, key(479, 0)).1,
        "Rotom-Heat"
    );
    assert_ne!(
        tiles(&cache, key(201, 1)).1,
        tiles(&cache, key(201, 0)).1,
        "Unown B"
    );
    assert_eq!(tiles(&cache, key(25, 9)).1, tiles(&cache, key(25, 0)).1);
    Some(cache)
}

#[test]
fn gen4_icons() {
    for (name, path) in [
        ("platinum.nds", "poketool/icongra/pl_poke_icon.narc"),
        ("heartgold.nds", "a/0/2/0"),
    ] {
        let Some(cache) = build(name) else { continue };
        // Rotom-Heat is icon 535 (member 542) in both games.
        let mut r = rom(name).unwrap();
        let f = r.find(path).unwrap();
        let n = Narc::open(r.source(), f).unwrap();
        let m = n.read_member(r.source(), 7 + 535).unwrap();
        assert_eq!(tiles(&cache, key(479, 1)).1, m[0x30..0x30 + 512]);
        assert!(
            CachedIcons::parse(&cache)
                .unwrap()
                .find(key(494, 0))
                .is_none(),
            "no Gen 5 species"
        );
    }
}

#[test]
fn gen5_icons() {
    for name in ["black2.nds", "white.nds"] {
        let Some(cache) = build(name) else { continue };
        let c = CachedIcons::parse(&cache).unwrap();
        assert!(c.find(key(649, 0)).is_some(), "Genesect");
        // Frillish has a female icon; Pikachu's falls back to the male one.
        let fem = |s| IconKey {
            female: true,
            ..key(s, 0)
        };
        assert_ne!(tiles(&cache, fem(592)).1, tiles(&cache, key(592, 0)).1);
        assert_eq!(tiles(&cache, fem(25)).1, tiles(&cache, key(25, 0)).1);
    }
}
