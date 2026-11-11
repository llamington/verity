//! Icon extraction against the owner's ROMs in `roms/` (each test skips a missing ROM).

use std::fs::File;
use std::os::unix::fs::FileExt;
use std::path::PathBuf;

use verity_rom::nds::Rom;
use verity_rom::ribbons::{self, COUNT, CachedRibbons};
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

#[test]
fn ribbon_images_from_each_rom() {
    for name in ["platinum.nds", "heartgold.nds", "white.nds", "black2.nds"] {
        let Some(mut r) = rom(name) else { continue };
        let set = ribbons::load(&mut r).unwrap();
        // The contest ribbons share four images per set (one per rank), coloured by palette.
        assert!(set.tile_blocks() < 60, "{name}: {}", set.tile_blocks());
        let cache = set.to_cache(r.game_code());
        let c = CachedRibbons::parse(&cache).unwrap();
        for i in 0..COUNT {
            let (tiles, pal) = c.icon(i).unwrap();
            assert!(tiles.iter().any(|&b| b != 0), "{name}: ribbon {i} is blank");
            assert!(
                pal[1..].iter().any(|&x| x != 0),
                "{name}: ribbon {i} palette"
            );
        }
        assert!(c.icon(COUNT).is_none());
        // Hoenn Cool (28) and Hoenn Beauty (32) share an image, not a palette; Sinnoh Cool (60)
        // has its own image.
        let (a, pa) = c.icon(28).unwrap();
        let (b, pb) = c.icon(32).unwrap();
        assert_eq!(a, b, "{name}");
        assert_ne!(pa, pb, "{name}");
        assert_ne!(c.icon(60).unwrap().0, a, "{name}");
        if let Ok(dir) = std::env::var("VERITY_RIBBON_SHEET") {
            sheet(&c, &format!("{dir}/{name}.ppm"));
        }
    }
}

/// The 80 images in list order, 20 per row (VERITY_RIBBON_SHEET=<dir>).
fn sheet(c: &CachedRibbons, path: &str) {
    let (w, h) = (20 * 32, 4 * 32);
    let mut px = vec![255u8; w * h * 3];
    for i in 0..COUNT {
        let (t, pal) = c.icon(i).unwrap();
        for (k, &byte) in t.iter().enumerate() {
            for half in 0..2 {
                let v = usize::from(if half == 0 { byte & 15 } else { byte >> 4 });
                if v == 0 {
                    continue;
                }
                let pixel = 2 * k + half;
                let (tile, within) = (pixel / 64, pixel % 64);
                let x = (tile % 4) * 8 + within % 8;
                let y = (tile / 4) * 8 + within / 8;
                let col = pal[v];
                let o = (((i / 20) * 32 + y) * w + (i % 20) * 32 + x) * 3;
                px[o] = ((col & 31) << 3) as u8;
                px[o + 1] = ((col >> 5 & 31) << 3) as u8;
                px[o + 2] = ((col >> 10 & 31) << 3) as u8;
            }
        }
    }
    let mut out = format!("P6 {w} {h} 255\n").into_bytes();
    out.extend_from_slice(&px);
    std::fs::write(path, out).unwrap();
}
