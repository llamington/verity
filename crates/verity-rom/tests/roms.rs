//! Runs against the owner's ROMs in `roms/` (gitignored); each test skips a missing ROM.

use std::fs::File;
use std::os::unix::fs::FileExt;
use std::path::PathBuf;

use verity_rom::font::{self, CachedFont, Keys};
use verity_rom::nds::{Game, Rom};
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
    Some(Rom::open(F(f)).expect("NDS header"))
}

fn ascii(g: &verity_rom::font::Glyph) -> String {
    (0..g.h)
        .map(|y| {
            (0..g.w)
                .map(|x| [' ', '#', '.', '?'][g.pixel(x, y) as usize])
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn gen4_fonts() {
    for (name, game) in [
        ("platinum.nds", Game::Platinum),
        ("heartgold.nds", Game::HeartGold),
    ] {
        let Some(mut r) = rom(name) else { continue };
        assert_eq!(r.game(), Some(game));
        let f = font::load(&mut r).unwrap();
        assert_eq!(
            (f.cell_w, f.cell_h, f.keys_are, f.len()),
            (16, 16, Keys::Gen4Codes, 509)
        );
        // Code 3 is あ: present, inked, and nothing but strokes and shadow.
        let a = f.glyph(3).unwrap();
        assert!(a.has_ink());
        assert!((0..16).all(|y| (0..16).all(|x| a.pixel(x, y) < 3)));
        // The middle row of あ's horizontal bar (row 4, see rom-font.md) is solid stroke.
        assert!((0..8).all(|x| a.pixel(x, 4) == 1), "{}", ascii(&a));
        assert!(
            f.glyph(510).is_none() && f.glyph(0x400).is_none(),
            "no Hangul in a Western Gen 4 ROM"
        );
        // Kana (codes 2..=0xA1; fullwidth digits and letters follow) keep their ink in rows
        // 2..=12; verity-ffi crops to those rows (fullwidth descenders reach row 14 and are cut).
        for code in 2..=0xA1 {
            let g = f.glyph(code).unwrap();
            let outside = (0..16).any(|x| [0, 1, 13, 14, 15].iter().any(|&y| g.pixel(x, y) != 0));
            assert!(!outside, "code {code:#x}\n{}", ascii(&g));
        }
    }
}

#[test]
fn gen5_fonts() {
    for (name, game) in [("black2.nds", Game::Black2), ("white.nds", Game::White)] {
        let Some(mut r) = rom(name) else { continue };
        assert_eq!(r.game(), Some(game));
        let f = font::load(&mut r).unwrap();
        assert_eq!((f.cell_w, f.cell_h, f.keys_are), (10, 10, Keys::Unicode));
        let count = |lo: u16, hi: u16| f.keys().iter().filter(|&&k| (lo..=hi).contains(&k)).count();
        assert_eq!(count(0x3040, 0x30FF), 177, "kana");
        assert_eq!(count(0x4E00, 0x9FFF), 1946, "kanji");
        assert_eq!(count(0xAC00, 0xD7A3), 2355, "Hangul");
        for c in "Aポ漢한".chars() {
            assert!(f.glyph(c as u16).unwrap().has_ink(), "{c}");
        }
        let a = f.glyph('A' as u16).unwrap();
        assert_eq!(
            (a.metrics.left, a.metrics.width, a.metrics.advance),
            (1, 4, 5)
        );
        // 'A' is 8 rows tall in the small font: ink in rows 0..8 only.
        assert!(
            (0..10).any(|x| a.pixel(x, 0) != 0) && (0..10).all(|x| a.pixel(x, 8) == 0),
            "{}",
            ascii(&a)
        );
        let space = f.glyph(' ' as u16).unwrap();
        assert!(!space.has_ink());
    }
}

#[test]
fn cache_round_trip() {
    let Some(mut r) = rom("black2.nds") else {
        return;
    };
    let f = font::load(&mut r).unwrap();
    let bytes = f.to_cache(r.game_code());
    assert!(bytes.len() < 200 * 1024, "{} bytes", bytes.len());
    let c = CachedFont::parse(&bytes).unwrap();
    c.validate().unwrap();
    assert_eq!(
        (c.len(), c.source_game_code, c.keys_are),
        (f.len(), *b"IREO", Keys::Unicode)
    );
    for &k in f.keys() {
        let (a, b) = (f.glyph(k).unwrap(), c.glyph(k).unwrap());
        assert_eq!(a.metrics, b.metrics);
        assert_eq!(ascii(&a), ascii(&b));
    }
    assert!(c.glyph(0xFFFE).is_none());
    // Fitting to 8 rows leaves glyphs that already fit (kana, Latin) unchanged.
    let fit = f.fit_rows(8);
    for c in "Aポ漢g".chars() {
        let (a, b) = (f.glyph(c as u16).unwrap(), fit.glyph(c as u16).unwrap());
        assert!(
            (0..10).all(|x| (0..8).all(|y| a.pixel(x, y) == b.pixel(x, y))),
            "{c}"
        );
    }
    assert!(fit.glyph('한' as u16).unwrap().has_ink());
    // Shadow removal keeps strokes only.
    let ns = f.without_shadow();
    let (a, b) = (f.glyph('A' as u16).unwrap(), ns.glyph('A' as u16).unwrap());
    assert!(
        (0..10).all(|x| (0..10).all(|y| b.pixel(x, y) == if a.pixel(x, y) == 1 { 1 } else { 0 }))
    );
    // Cropping keeps the rows asked for.
    let cropped = f.crop_rows(2, 6);
    assert_eq!((cropped.cell_h, cropped.len()), (6, f.len()));
    let (a, b) = (
        f.glyph('A' as u16).unwrap(),
        cropped.glyph('A' as u16).unwrap(),
    );
    assert!((0..10).all(|x| (0..6).all(|y| a.pixel(x, y + 2) == b.pixel(x, y))));
    let mut unsorted = bytes.clone();
    unsorted.swap(16, 18);
    unsorted.swap(17, 19);
    assert!(CachedFont::parse(&unsorted).unwrap().validate().is_err());
    // Any truncation or bad header is refused.
    assert!(CachedFont::parse(&bytes[..bytes.len() - 1]).is_err());
    let mut bad = bytes.clone();
    bad[4] = 2;
    assert!(CachedFont::parse(&bad).is_err());
}

#[test]
fn missing_paths_and_members() {
    let Some(mut r) = rom("platinum.nds") else {
        return;
    };
    assert_eq!(r.find("graphic/nope.narc"), Err(Error::NotFound));
    assert_eq!(r.find("nope/pl_font.narc"), Err(Error::NotFound));
    assert_eq!(r.find("graphic"), Err(Error::NotFound));
    let file = r.find("graphic/pl_font.narc").unwrap();
    let n = verity_rom::narc::Narc::open(r.source(), file).unwrap();
    assert_eq!(n.len(), 8);
    assert_eq!(n.member(8), Err(Error::NotFound));
    // A file that is not a NARC.
    let ncgr = r.find("data/nfont.NCGR").unwrap();
    assert!(verity_rom::narc::Narc::open(r.source(), ncgr).is_err());
}
