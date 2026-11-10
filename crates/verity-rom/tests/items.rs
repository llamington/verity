//! Icon extraction against the owner's ROMs in `roms/` (each test skips a missing ROM).

use std::fs::File;
use std::os::unix::fs::FileExt;
use std::path::PathBuf;

use verity_rom::items::{self, CachedItems};
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

/// (ROM, generation, item IDs covered).
const ROMS: [(&str, u8, usize); 4] = [
    ("platinum.nds", 4, 468),
    ("heartgold.nds", 4, 537),
    ("white.nds", 5, 627),
    ("black2.nds", 5, 639),
];

#[test]
fn item_icons_from_each_rom() {
    for (name, generation, count) in ROMS {
        let Some(mut r) = rom(name) else { continue };
        let set = items::load(&mut r).unwrap();
        assert_eq!((set.generation, set.len()), (generation, count), "{name}");
        // TMs share one disc icon (with a palette per type), so there are fewer blocks than items.
        assert!(
            set.tile_blocks() < count - 50,
            "{name}: {}",
            set.tile_blocks()
        );
        let cache = set.to_cache(r.game_code());
        let head = CachedItems::head_len(&cache[..16]).unwrap();
        let c = CachedItems::parse(&cache[..head]).unwrap();
        assert_eq!((c.generation, c.len()), (generation, count));
        assert_eq!(c.find(0), None, "no item, no icon");
        assert_eq!(c.find(count as u16), None);
        let tiles = |item: u16| {
            let at = c.find(item).unwrap().tiles_at as usize;
            &cache[at..at + 512]
        };
        let pal = |item: u16| c.find(item).unwrap().palette_at;
        // Master Ball and Poké Ball differ; TM01 (328) and TM02 share the disc but not the palette.
        assert_ne!(tiles(1), tiles(4));
        assert_eq!(tiles(328), tiles(329), "{name}");
        assert_ne!(pal(328), pal(329), "{name}");
        for i in 1..count as u16 {
            let r = c.find(i).unwrap();
            assert!(r.tiles_at as usize + 512 <= cache.len() && r.palette_at < r.tiles_at);
            assert!(
                tiles(i).iter().any(|&b| b != 0),
                "{name}: item {i} is blank"
            );
        }
        if let Ok(dir) = std::env::var("VERITY_ITEM_SHEET") {
            sheet(&cache, &c, &format!("{dir}/{name}.ppm"));
        }
    }
}

/// Writes the icons as a PPM contact sheet, 24 per row (VERITY_ITEM_SHEET=<dir>).
fn sheet(cache: &[u8], c: &CachedItems, path: &str) {
    let n = c.len();
    let (cols, rows) = (24, n.div_ceil(24));
    let (w, h) = (cols * 32, rows * 32);
    let mut px = vec![255u8; w * h * 3];
    for i in 1..n {
        let r = c.find(i as u16).unwrap();
        let p = r.palette_at as usize;
        let t = &cache[r.tiles_at as usize..r.tiles_at as usize + 512];
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
                let col = u16::from_le_bytes([cache[p + 2 * v], cache[p + 2 * v + 1]]);
                let o = (((i / cols) * 32 + y) * w + (i % cols) * 32 + x) * 3;
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
