//! The front ends' legality entry point: builds the Gen 4 and Gen 5 contexts from data blobs and
//! picks the right analysis for a Pokémon, or says why it can't be judged. Shared by the DS
//! (`verity-ffi`) and the browser (`verity-wasm`).

use super::analysis3::{Context3, is_gba_version};
use super::analysis4::{Context4, Matched, Report, analyze_transfer4, analyze4};
use super::analysis5::{Context5, analyze5, is_gen5_version};
use super::{evo::Evo4, moves4::Learn4};
use crate::data::encounters::EncounterData;
use crate::data::{GameData, MoveData, Strings};
use crate::gift::{pcd::Gifts4, pgf::Gifts5};
use crate::pkm::Pkm;

/// A data blob the legality check reads; [`Blob::file`] is its name under `data/blobs/`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Blob {
    Dp,
    Pt,
    Hgss,
    Bw,
    B2w2,
    Moves4,
    Moves5,
    EncDppt,
    EncHgss,
    EncBw,
    EncB2w2,
    EncDr,
    Names4,
    Names5,
    Gifts4,
    Gifts5,
    G3Origin,
}

impl Blob {
    pub fn file(self) -> &'static str {
        use Blob::*;
        match self {
            Dp => "dp",
            Pt => "pt",
            Hgss => "hgss",
            Bw => "bw",
            B2w2 => "b2w2",
            Moves4 => "moves4",
            Moves5 => "moves5",
            EncDppt => "enc_dppt",
            EncHgss => "enc_hgss",
            EncBw => "enc_bw",
            EncB2w2 => "enc_b2w2",
            EncDr => "enc_dr",
            Names4 => "names4",
            Names5 => "names5",
            Gifts4 => "gifts4",
            Gifts5 => "gifts5",
            G3Origin => "g3_origin",
        }
    }
}

/// Every blob [`context4`] reads (the last two are optional).
pub const GEN4: [Blob; 10] = [
    Blob::Hgss,
    Blob::Pt,
    Blob::Dp,
    Blob::B2w2,
    Blob::Moves4,
    Blob::EncDppt,
    Blob::EncHgss,
    Blob::Names4,
    Blob::G3Origin,
    Blob::Gifts4,
];

/// Every blob [`context5`] reads (Gifts5 is optional).
pub const GEN5: [Blob; 8] = [
    Blob::Bw,
    Blob::B2w2,
    Blob::Moves5,
    Blob::EncBw,
    Blob::EncB2w2,
    Blob::EncDr,
    Blob::Names5,
    Blob::Gifts5,
];

/// The blobs a save of this generation needs for [`check`]: Gen 5 saves need the Gen 4 data too,
/// for Poké Transfer origins.
pub fn blobs_for(generation: u8) -> impl Iterator<Item = Blob> {
    let gen5: &[Blob] = if generation == 5 { &GEN5 } else { &[] };
    let mut seen = alloc::vec::Vec::new();
    for &b in GEN4.iter().chain(gen5) {
        if !seen.contains(&b) {
            seen.push(b);
        }
    }
    seen.into_iter()
}

/// The Gen 4 context, or `None` when a required blob is missing or malformed. Without G3Origin,
/// Pal Park Pokémon stay unchecked; without Gifts4, Wonder Cards aren't matched.
pub fn context4<'a>(blob: impl Fn(Blob) -> Option<&'a [u8]>) -> Option<Context4<'a>> {
    let g = |k| blob(k).and_then(|b| GameData::load(b).ok());
    let (hgss, pt, dp, gen5) = (g(Blob::Hgss)?, g(Blob::Pt)?, g(Blob::Dp)?, g(Blob::B2w2)?);
    Some(Context4 {
        hgss,
        learn: Learn4 {
            hgss,
            pt,
            dp,
            moves: MoveData::load(blob(Blob::Moves4)?).ok()?,
        },
        evo: Evo4::new(dp, hgss),
        enc_dppt: EncounterData::load(blob(Blob::EncDppt)?).ok()?,
        enc_hgss: EncounterData::load(blob(Blob::EncHgss)?).ok()?,
        gen5,
        names4: Strings::load(blob(Blob::Names4)?).ok()?,
        g3: blob(Blob::G3Origin).and_then(|b| Context3::load(b).ok()),
        gifts: blob(Blob::Gifts4)
            .and_then(|b| Gifts4::load(b).ok())
            .unwrap_or(Gifts4::empty()),
    })
}

/// The Gen 5 context; without Gifts5, Wonder Card Pokémon aren't matched.
pub fn context5<'a>(blob: impl Fn(Blob) -> Option<&'a [u8]>) -> Option<Context5<'a>> {
    let g = |k| blob(k).and_then(|b| GameData::load(b).ok());
    let enc = |k| blob(k).and_then(|b| EncounterData::load(b).ok());
    let (bw, b2w2) = (g(Blob::Bw)?, g(Blob::B2w2)?);
    Some(Context5 {
        bw,
        b2w2,
        moves: MoveData::load(blob(Blob::Moves5)?).ok()?,
        enc_bw: enc(Blob::EncBw)?,
        enc_b2w2: enc(Blob::EncB2w2)?,
        enc_dr: enc(Blob::EncDr)?,
        evo: Evo4::new(bw, b2w2),
        names: Strings::load(blob(Blob::Names5)?).ok()?,
        gifts: blob(Blob::Gifts5)
            .and_then(|b| Gifts5::load(b).ok())
            .unwrap_or(Gifts5::empty()),
    })
}

/// Why a Pokémon wasn't judged. The values are the DS's `unsupported` codes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Unchecked {
    /// An origin game verity doesn't model (Colosseum/XD, or Gen 3 without its data).
    Origin = 2,
    DataMissing = 3,
    /// Not a Gen 4 or Gen 5 save.
    Generation = 4,
    /// No modelled origin matched (events, trades, Pokéwalker and so on).
    Unmatched = 6,
}

impl Unchecked {
    pub fn text(self) -> &'static str {
        match self {
            Unchecked::Origin => "origin game not checked yet",
            Unchecked::DataMissing => "legality data missing",
            Unchecked::Generation => "not a Gen 4 or Gen 5 save",
            Unchecked::Unmatched => "origin not modelled (event, trade or Pokéwalker)",
        }
    }
}

/// Judges `pk` from a save of `generation`. Pass the contexts that could be built.
pub fn check<'a>(
    generation: u8,
    pk: &Pkm,
    c4: Option<&'a Context4<'a>>,
    c5: Option<&'a Context5<'a>>,
) -> Result<Report<'a>, Unchecked> {
    // Without a match the origin is something verity can't judge yet, so it says nothing rather
    // than a false "illegal".
    let r = analyze(generation, pk, c4, c5)?;
    if matches!(r.matched, Matched::Invalid) {
        return Err(Unchecked::Unmatched);
    }
    Ok(r)
}

/// Judges `edited`, an edit of `original`, for the editors' previews and legal mode. As
/// [`check`], except when `original` matched a modelled origin and `edited` matches none: the
/// edit broke the origin (a level below the evolution, a met location that doesn't fit), so the
/// report stands and is illegal, as in PKHeX.
pub fn check_edit<'a>(
    generation: u8,
    original: &Pkm,
    edited: &Pkm,
    c4: Option<&'a Context4<'a>>,
    c5: Option<&'a Context5<'a>>,
) -> Result<Report<'a>, Unchecked> {
    let r = analyze(generation, edited, c4, c5)?;
    if matches!(r.matched, Matched::Invalid) && check(generation, original, c4, c5).is_err() {
        return Err(Unchecked::Unmatched);
    }
    Ok(r)
}

/// The analysis for `pk`'s origin game, unmatched results included.
fn analyze<'a>(
    generation: u8,
    pk: &Pkm,
    c4: Option<&'a Context4<'a>>,
    c5: Option<&'a Context5<'a>>,
) -> Result<Report<'a>, Unchecked> {
    let r = match generation {
        5 => {
            let c5 = c5.ok_or(Unchecked::DataMissing)?;
            if is_gen5_version(pk.version()) {
                analyze5(c5, pk)
            } else {
                // Poké Transfer: judged with the Gen 3 and 4 data. Colosseum and XD are out of
                // scope.
                let c4 = c4.ok_or(Unchecked::DataMissing)?;
                if pk.version() == 15 {
                    return Err(Unchecked::Origin);
                }
                analyze_transfer4(c4, c5, pk)
            }
        }
        4 => {
            let gba = is_gba_version(pk.version());
            if !gba && !matches!(pk.version(), 7 | 8 | 10..=12) {
                return Err(Unchecked::Origin);
            }
            let c4 = c4.ok_or(Unchecked::DataMissing)?;
            if gba && c4.g3.is_none() {
                return Err(Unchecked::Origin);
            }
            analyze4(c4, pk)
        }
        _ => return Err(Unchecked::Generation),
    };
    Ok(r)
}
