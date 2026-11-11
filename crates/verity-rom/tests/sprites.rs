//! Sprite and type icon extraction against the owner's ROMs in `roms/` (each test skips a
//! missing ROM). Writes contact sheets to target/sprites-<rom>.ppm for a visual check.

use std::fs::File;
use std::io::Write;
use std::os::unix::fs::FileExt;
use std::path::PathBuf;

use verity_rom::icons::IconKey;
use verity_rom::nds::Rom;
use verity_rom::sprites::{self, CachedSprites, FOOTER, SPRITE_BYTES};
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

fn k(species: u16, form: u8, female: bool, egg: bool) -> IconKey {
    IconKey {
        species,
        form,
        female,
        egg,
    }
}

fn rgb(c: u16) -> [u8; 3] {
    [
        ((c & 31) * 255 / 31) as u8,
        ((c >> 5 & 31) * 255 / 31) as u8,
        ((c >> 10 & 31) * 255 / 31) as u8,
    ]
}

/// Pixel (x, y) of a decoded sprite.
fn px(s: &[u8; SPRITE_BYTES], x: usize, y: usize) -> u8 {
    let blocks = [
        (0, 0, 64, 64),
        (64, 0, 32, 64),
        (0, 64, 64, 32),
        (64, 64, 32, 32),
    ];
    let mut tile = 0;
    for (bx, by, bw, bh) in blocks {
        if x >= bx && x < bx + bw && y >= by && y < by + bh {
            let (lx, ly) = (x - bx, y - by);
            let t = tile + (ly / 8) * (bw / 8) + lx / 8;
            let b = s[t * 32 + (ly % 8) * 4 + (lx % 8) / 2];
            return if lx % 2 == 0 { b & 15 } else { b >> 4 };
        }
        tile += bw * bh / 64;
    }
    0
}

#[test]
fn sprites_from_roms() {
    for (name, gen, min) in [
        ("platinum.nds", 4u8, 560usize),
        ("heartgold.nds", 4, 560),
        ("white.nds", 5, 700),
        ("black2.nds", 5, 740),
    ] {
        let Some(mut r) = rom(name) else { continue };
        let mut cache = Vec::new();
        let t = std::time::Instant::now();
        let n = sprites::build(&mut r, &mut |b: &[u8]| {
            cache.extend_from_slice(b);
            Ok(())
        })
        .unwrap();
        eprintln!(
            "{name}: {n} sprites, {} KB, {:?}",
            cache.len() / 1024,
            t.elapsed()
        );
        assert!(n >= min, "{name}: {n}");
        let footer = &cache[cache.len() - FOOTER..];
        let start = CachedSprites::tail_start(footer, cache.len() as u32).unwrap();
        let c = CachedSprites::parse(&cache[start as usize..], start).unwrap();
        assert_eq!(c.generation, gen);
        assert!(
            c.icon(0, 9).is_some() && c.icon(1, 2).is_some(),
            "{name} icons"
        );

        // Contact sheet: a row of sprites (normal palette, then shiny for the first) and icons.
        let keys = [
            k(25, 0, false, false),
            k(25, 0, true, false),
            k(6, 0, false, false),
            k(201, 5, false, false),
            k(386, 1, false, false),
            k(479, 3, false, false),
            k(493, 9, false, false),
            k(487, 1, false, false),
            k(0, 0, false, true),
            k(490, 0, false, true),
            k(if gen == 5 { 555 } else { 172 }, 1, false, false),
        ];
        let (w, h) = (96 * keys.len(), 96 * 2 + 40);
        let mut img = vec![200u8; w * h * 3];
        for (i, key) in keys.iter().enumerate() {
            let s = c.find(*key).unwrap_or_else(|| panic!("{name} {key:?}"));
            let tiles = sprites::decode(&cache[s.tiles as usize..(s.tiles + s.tiles_len) as usize])
                .unwrap();
            for shiny in 0..2 {
                let p = s.palette as usize + 32 * shiny;
                let pal: Vec<u16> = (0..16)
                    .map(|j| u16::from_le_bytes([cache[p + 2 * j], cache[p + 2 * j + 1]]))
                    .collect();
                for y in 0..96 {
                    for x in 0..96 {
                        let v = px(&tiles, x, y);
                        if v != 0 {
                            let o = ((y + 96 * shiny) * w + i * 96 + x) * 3;
                            img[o..o + 3].copy_from_slice(&rgb(pal[v as usize]));
                        }
                    }
                }
            }
        }
        for slot in 0..20 {
            let (kind, i) = if slot < 17 { (0, slot) } else { (1, slot - 17) };
            let (tiles, pal) = c.icon(kind, i).unwrap();
            let (ox, oy) = ((slot % 10) * 34, 194 + (slot / 10) * 18);
            for y in 0..16 {
                for x in 0..32 {
                    let t = (y / 8) * 4 + x / 8;
                    let b = tiles[t * 32 + (y % 8) * 4 + (x % 8) / 2];
                    let v = if x % 2 == 0 { b & 15 } else { b >> 4 };
                    if v != 0 {
                        let o = ((oy + y) * w + ox + x) * 3;
                        img[o..o + 3].copy_from_slice(&rgb(pal[v as usize]));
                    }
                }
            }
        }
        let out = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../target")
            .join(format!("sprites-{}.ppm", name.trim_end_matches(".nds")));
        let mut f = File::create(out).unwrap();
        write!(f, "P6 {w} {h} 255\n").unwrap();
        f.write_all(&img).unwrap();
    }
}
