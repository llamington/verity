//! The games' fonts (formats in `docs/research/rom-font.md`) and verity's own font cache.
//!
//! Glyphs are kept as one 2-bit-per-pixel bit stream per glyph, row-major, most significant pixel
//! first (the NFTR layout). Pixel values: 0 empty, 1 stroke, 2 shadow, 3 unused. The games draw
//! stroke and shadow together; stroke alone leaves gaps.

use alloc::vec::Vec;

use crate::narc::Narc;
use crate::nds::{Game, Rom};
use crate::{Error, Result, Source, u16_at, u32_at};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Metrics {
    /// Where the game places the bitmap after the pen position (the bitmap's ink starts at its
    /// column 0).
    pub left: u8,
    /// Ink width from column 0.
    pub width: u8,
    /// Pen advance.
    pub advance: u8,
}

/// What the keys of a [`Font`] are.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Keys {
    /// The Gen 4 games' own character codes (map them with the Gen 4 text table).
    Gen4Codes,
    /// Unicode (Gen 5).
    Unicode,
}

/// A font with glyphs sorted by key.
#[derive(Clone, Debug)]
pub struct Font {
    pub cell_w: u8,
    pub cell_h: u8,
    pub keys_are: Keys,
    keys: Vec<u16>,
    metrics: Vec<Metrics>,
    bitmaps: Vec<u8>,
}

fn glyph_size(w: u8, h: u8) -> usize {
    (usize::from(w) * usize::from(h)).div_ceil(4)
}

/// Where each game keeps the font verity uses: (path, NARC member).
pub fn location(game: Game) -> (&'static str, usize) {
    match game {
        Game::Diamond | Game::Pearl => ("graphic/font.narc", 0),
        Game::Platinum => ("graphic/pl_font.narc", 0),
        Game::HeartGold | Game::SoulSilver => ("a/0/1/6", 0),
        // Member 1 is the small font (10x10 cells, Latin capitals 8 px high).
        _ => ("a/0/2/3", 1),
    }
}

/// Loads the font of a supported game.
pub fn load<S: Source>(rom: &mut Rom<S>) -> Result<Font> {
    let game = rom.game().ok_or(Error::NotFound)?;
    let (path, member) = location(game);
    let file = rom.find(path)?;
    let narc = Narc::open(rom.source(), file)?;
    let bytes = narc.read_member(rom.source(), member)?;
    if game.generation() == 4 {
        Font::from_gen4(&bytes)
    } else {
        Font::from_nftr(&bytes)
    }
}

impl Font {
    /// A Gen 4 font (`pokeplatinum src/font_manager.c`); keys are the game's 1-based codes.
    pub fn from_gen4(b: &[u8]) -> Result<Font> {
        let bad = Error::Format("Gen 4 font");
        let header = u32_at(b, 0)? as usize;
        let widths = u32_at(b, 4)? as usize;
        let count = u32_at(b, 8)? as usize;
        let (tw, th) = (*b.get(14).ok_or(bad)?, *b.get(15).ok_or(bad)?);
        if header < 16
            || count == 0
            || count > 0xFFFF
            || !(1..=2).contains(&tw)
            || !(1..=2).contains(&th)
        {
            return Err(bad);
        }
        let (w, h) = (8 * tw, 8 * th);
        let src_size = 16 * usize::from(tw) * usize::from(th);
        let glyphs = b.get(header..header + count * src_size).ok_or(bad)?;
        let advances = b.get(widths..widths + count).ok_or(bad)?;
        let size = glyph_size(w, h);
        let mut font = Font::with_capacity(w, h, Keys::Gen4Codes, count);
        for i in 0..count {
            let g = &glyphs[i * src_size..(i + 1) * src_size];
            let mut out = alloc::vec![0u8; size];
            for y in 0..usize::from(h) {
                for x in 0..usize::from(w) {
                    // Tiles TL, TR, BL, BR; a tile row is a LE u16 read high byte first.
                    let tile = (y / 8) * usize::from(tw) + x / 8;
                    let row = tile * 16 + (y % 8) * 2;
                    let byte = if x % 8 < 4 { g[row + 1] } else { g[row] };
                    let v = (byte >> (6 - 2 * (x % 4))) & 3;
                    // 3 is the cell background.
                    set_pixel(&mut out, usize::from(w), x, y, if v == 3 { 0 } else { v });
                }
            }
            let advance = advances[i];
            font.push(
                (i + 1) as u16,
                Metrics {
                    left: 0,
                    width: advance.min(w),
                    advance,
                },
                &out,
            );
        }
        Ok(font)
    }

    /// A Gen 5 NFTR font; keys are Unicode.
    pub fn from_nftr(b: &[u8]) -> Result<Font> {
        let bad = Error::Format("NFTR");
        if b.get(..4) != Some(b"RTFN") {
            return Err(bad);
        }
        let mut p = usize::from(u16_at(b, 0x0C)?);
        let mut cglp = None;
        let mut map: Vec<(u16, u16)> = Vec::new();
        while p + 8 <= b.len() {
            let tag = &b[p..p + 4];
            let size = u32_at(b, p + 4)? as usize;
            if size < 8 || p + size > b.len() {
                return Err(bad);
            }
            let sec = &b[p..p + size];
            match tag {
                b"PLGC" => cglp = Some(sec),
                b"PAMC" => read_cmap(sec, &mut map)?,
                _ => {}
            }
            p += size;
        }
        let cglp = cglp.ok_or(bad)?;
        let (w, h, rec, bpp) = (cglp[8], cglp[9], usize::from(u16_at(cglp, 10)?), cglp[14]);
        let size = glyph_size(w, h);
        if bpp != 2 || w == 0 || h == 0 || rec != 3 + size {
            return Err(Error::Format("NFTR glyph layout"));
        }
        let data = &cglp[16..];
        let count = data.len() / rec;
        map.retain(|&(_, g)| usize::from(g) < count);
        map.sort_unstable();
        map.dedup_by_key(|e| e.0);
        let mut font = Font::with_capacity(w, h, Keys::Unicode, map.len());
        for (key, g) in map {
            let r = &data[usize::from(g) * rec..(usize::from(g) + 1) * rec];
            let m = Metrics {
                left: r[0],
                width: r[1],
                advance: r[2],
            };
            font.push(key, m, &r[3..]);
        }
        Ok(font)
    }

    fn with_capacity(w: u8, h: u8, keys_are: Keys, n: usize) -> Font {
        Font {
            cell_w: w,
            cell_h: h,
            keys_are,
            keys: Vec::with_capacity(n),
            metrics: Vec::with_capacity(n),
            bitmaps: Vec::with_capacity(n * glyph_size(w, h)),
        }
    }

    fn push(&mut self, key: u16, m: Metrics, bits: &[u8]) {
        self.keys.push(key);
        self.metrics.push(m);
        self.bitmaps.extend_from_slice(bits);
    }

    pub fn len(&self) -> usize {
        self.keys.len()
    }

    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }

    pub fn keys(&self) -> &[u16] {
        &self.keys
    }

    pub fn glyph(&self, key: u16) -> Option<Glyph<'_>> {
        let i = self.keys.binary_search(&key).ok()?;
        let size = glyph_size(self.cell_w, self.cell_h);
        Some(Glyph {
            w: self.cell_w,
            h: self.cell_h,
            metrics: self.metrics[i],
            bits: &self.bitmaps[i * size..(i + 1) * size],
        })
    }

    /// Rekeys the font (e.g. Gen 4 codes to Unicode), dropping glyphs `f` maps to `None` and
    /// keeping the first glyph when two keys collide.
    pub fn remap(&self, keys_are: Keys, mut f: impl FnMut(u16) -> Option<u16>) -> Font {
        let mut order: Vec<(u16, usize)> = self
            .keys
            .iter()
            .enumerate()
            .filter_map(|(i, &k)| f(k).map(|n| (n, i)))
            .collect();
        order.sort_unstable();
        order.dedup_by_key(|e| e.0);
        let size = glyph_size(self.cell_w, self.cell_h);
        let mut out = Font::with_capacity(self.cell_w, self.cell_h, keys_are, order.len());
        for (k, i) in order {
            out.push(k, self.metrics[i], &self.bitmaps[i * size..(i + 1) * size]);
        }
        out
    }

    /// Clears shadow pixels (Gen 4 draws a separate drop shadow under a 1 px stroke).
    pub fn without_shadow(&self) -> Font {
        let mut out = self.clone();
        for b in &mut out.bitmaps {
            // Pixels are 2-bit pairs; 2 (shadow) is the pair 0b10.
            let shadow = *b & 0xAA & !((*b & 0x55) << 1);
            *b &= !shadow;
        }
        out
    }

    /// Fits every glyph into `h` rows: drops empty bottom rows first, then repeatedly merges the
    /// most similar pair of adjacent rows (vertical strokes just get shorter).
    pub fn fit_rows(&self, h: u8) -> Font {
        if self.cell_h <= h {
            return self.clone();
        }
        let (w, old) = (self.cell_w, glyph_size(self.cell_w, self.cell_h));
        let mut out = Font::with_capacity(w, h, self.keys_are, self.len());
        for (i, (&k, &m)) in self.keys.iter().zip(&self.metrics).enumerate() {
            let src = Glyph {
                w,
                h: self.cell_h,
                metrics: m,
                bits: &self.bitmaps[i * old..(i + 1) * old],
            };
            let mut rows: Vec<Vec<u8>> = (0..self.cell_h)
                .map(|y| (0..w).map(|x| src.pixel(x, y)).collect())
                .collect();
            while rows.len() > usize::from(h)
                && rows.last().is_some_and(|r| r.iter().all(|&p| p == 0))
            {
                rows.pop();
            }
            while rows.len() > usize::from(h) {
                let diff = |a: &[u8], b: &[u8]| a.iter().zip(b).filter(|(x, y)| x != y).count();
                let j = (0..rows.len() - 1)
                    .min_by_key(|&j| diff(&rows[j], &rows[j + 1]))
                    .unwrap_or(0);
                let next = rows.remove(j + 1);
                for (p, q) in rows[j].iter_mut().zip(next) {
                    *p = (*p).max(q);
                }
            }
            let mut bits = alloc::vec![0u8; glyph_size(w, h)];
            for (y, row) in rows.iter().enumerate() {
                for (x, &p) in row.iter().enumerate() {
                    set_pixel(&mut bits, usize::from(w), x, y, p);
                }
            }
            out.push(k, m, &bits);
        }
        out
    }

    /// Keeps rows `top..top + height` of every glyph (to fit a fixed line height).
    pub fn crop_rows(&self, top: u8, height: u8) -> Font {
        let height = height.min(self.cell_h.saturating_sub(top));
        let (w, old) = (
            usize::from(self.cell_w),
            glyph_size(self.cell_w, self.cell_h),
        );
        let mut out = Font::with_capacity(self.cell_w, height, self.keys_are, self.len());
        for (i, (&k, &m)) in self.keys.iter().zip(&self.metrics).enumerate() {
            let src = Glyph {
                w: self.cell_w,
                h: self.cell_h,
                metrics: m,
                bits: &self.bitmaps[i * old..(i + 1) * old],
            };
            let mut bits = alloc::vec![0u8; glyph_size(self.cell_w, height)];
            for y in 0..usize::from(height) {
                for x in 0..w {
                    set_pixel(&mut bits, w, x, y, src.pixel(x as u8, top + y as u8));
                }
            }
            out.push(k, m, &bits);
        }
        out
    }

    /// Serializes the font as a verity font cache (see [`CachedFont`]).
    pub fn to_cache(&self, source_game_code: [u8; 4]) -> Vec<u8> {
        let mut v = Vec::with_capacity(CACHE_HEADER + self.len() * 5 + self.bitmaps.len());
        v.extend_from_slice(CACHE_MAGIC);
        v.extend_from_slice(&[CACHE_VERSION, self.cell_w, self.cell_h, self.keys_are as u8]);
        v.extend_from_slice(&(self.len() as u16).to_le_bytes());
        v.extend_from_slice(&(glyph_size(self.cell_w, self.cell_h) as u16).to_le_bytes());
        v.extend_from_slice(&source_game_code);
        for k in &self.keys {
            v.extend_from_slice(&k.to_le_bytes());
        }
        for m in &self.metrics {
            v.extend_from_slice(&[m.left, m.width, m.advance]);
        }
        v.extend_from_slice(&self.bitmaps);
        v
    }
}

fn set_pixel(out: &mut [u8], w: usize, x: usize, y: usize, v: u8) {
    let i = 2 * (y * w + x);
    out[i / 8] |= v << (6 - i % 8);
}

fn read_cmap(sec: &[u8], map: &mut Vec<(u16, u16)>) -> Result<()> {
    let (lo, hi, kind) = (u16_at(sec, 8)?, u16_at(sec, 10)?, u16_at(sec, 12)?);
    let d = 20;
    match kind {
        0 => {
            let base = u16_at(sec, d)?;
            for c in lo..=hi {
                map.push((c, base.wrapping_add(c - lo)));
            }
        }
        1 => {
            for (k, c) in (lo..=hi).enumerate() {
                let g = u16_at(sec, d + 2 * k)?;
                if g != 0xFFFF {
                    map.push((c, g));
                }
            }
        }
        2 => {
            let n = usize::from(u16_at(sec, d)?);
            for k in 0..n {
                map.push((u16_at(sec, d + 2 + 4 * k)?, u16_at(sec, d + 4 + 4 * k)?));
            }
        }
        _ => return Err(Error::Format("NFTR CMAP")),
    }
    Ok(())
}

#[derive(Clone, Copy, Debug)]
pub struct Glyph<'a> {
    pub w: u8,
    pub h: u8,
    pub metrics: Metrics,
    bits: &'a [u8],
}

impl Glyph<'_> {
    /// 0 empty, 1 stroke, 2 shadow.
    pub fn pixel(&self, x: u8, y: u8) -> u8 {
        if x >= self.w || y >= self.h {
            return 0;
        }
        let i = 2 * (usize::from(y) * usize::from(self.w) + usize::from(x));
        (self.bits[i / 8] >> (6 - i % 8)) & 3
    }

    pub fn has_ink(&self) -> bool {
        self.bits.iter().any(|&b| b != 0)
    }
}

// ---- cache ---------------------------------------------------------------------------------

const CACHE_MAGIC: &[u8; 4] = b"VFNT";
const CACHE_VERSION: u8 = 1;
const CACHE_HEADER: usize = 16;

/// A font cache file read in place (the DS keeps it in `/_nds/verity/`). Layout, little-endian:
/// `"VFNT"`, version `u8` (1), cell width `u8`, cell height `u8`, key kind `u8` (0 Gen 4 codes,
/// 1 Unicode), glyph count `u16`, glyph size `u16`, source game code `[u8; 4]`; then the sorted
/// keys (`u16` each), the metrics (left, width, advance per glyph) and the glyph bit streams.
pub struct CachedFont<'a> {
    pub cell_w: u8,
    pub cell_h: u8,
    pub keys_are: Keys,
    pub source_game_code: [u8; 4],
    keys: &'a [u8],
    metrics: &'a [u8],
    bitmaps: &'a [u8],
    size: usize,
}

impl<'a> CachedFont<'a> {
    pub fn parse(b: &'a [u8]) -> Result<CachedFont<'a>> {
        let bad = Error::Format("font cache");
        if b.len() < CACHE_HEADER || &b[..4] != CACHE_MAGIC || b[4] != CACHE_VERSION {
            return Err(bad);
        }
        let (w, h) = (b[5], b[6]);
        let keys_are = match b[7] {
            0 => Keys::Gen4Codes,
            1 => Keys::Unicode,
            _ => return Err(bad),
        };
        let n = usize::from(u16_at(b, 8)?);
        let size = usize::from(u16_at(b, 10)?);
        if w == 0 || h == 0 || size != glyph_size(w, h) || b.len() != CACHE_HEADER + n * (5 + size)
        {
            return Err(bad);
        }
        let keys = &b[CACHE_HEADER..CACHE_HEADER + 2 * n];
        let metrics = &b[CACHE_HEADER + 2 * n..CACHE_HEADER + 5 * n];
        let bitmaps = &b[CACHE_HEADER + 5 * n..];
        Ok(CachedFont {
            cell_w: w,
            cell_h: h,
            keys_are,
            source_game_code: [b[12], b[13], b[14], b[15]],
            keys,
            metrics,
            bitmaps,
            size,
        })
    }

    /// [`CachedFont::parse`] checks the layout only, which keeps lookups in bounds; this also
    /// checks that the keys are sorted (lookups would miss glyphs otherwise). O(glyphs).
    pub fn validate(&self) -> Result<()> {
        let key = |i: usize| u16::from_le_bytes([self.keys[2 * i], self.keys[2 * i + 1]]);
        if (1..self.len()).any(|i| key(i - 1) >= key(i)) {
            return Err(Error::Format("font cache keys"));
        }
        Ok(())
    }

    pub fn len(&self) -> usize {
        self.keys.len() / 2
    }

    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }

    pub fn glyph(&self, key: u16) -> Option<Glyph<'a>> {
        let (mut lo, mut hi) = (0, self.len());
        while lo < hi {
            let mid = (lo + hi) / 2;
            let k = u16::from_le_bytes([self.keys[2 * mid], self.keys[2 * mid + 1]]);
            if k == key {
                let m = &self.metrics[3 * mid..3 * mid + 3];
                return Some(Glyph {
                    w: self.cell_w,
                    h: self.cell_h,
                    metrics: Metrics {
                        left: m[0],
                        width: m[1],
                        advance: m[2],
                    },
                    bits: &self.bitmaps[mid * self.size..(mid + 1) * self.size],
                });
            }
            if k < key {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        None
    }
}
