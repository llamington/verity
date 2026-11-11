//! Delivers a Gen 5 Wonder Card the way receiving it does: the card goes in the album still
//! uncollected and its received flag is set, and the deliveryman in any Pokémon Center hands the
//! gift over and marks it collected. Block layout from PKHeX `MysteryBlock5`; what the game writes
//! was checked against genuine saves, see `docs/research/events-region.md`.

use super::pgf::{Event5, PGF_SIZE, Pgf};
use crate::save::{Save, SaveKind};

const FLAGS_SIZE: usize = 0x100;
const CARD_COUNT: usize = 12;
/// Everything before the trailing seed is encrypted as one stream (`MysteryBlock5.DataRegion`).
const DATA_SIZE: usize = 0xA90;
const SEED_OFS: usize = DATA_SIZE;
const DATE_OFS: usize = 0xAC;
const CARD_ID_OFS: usize = 0xB0;
const CARD_TYPE_OFS: usize = 0xB3;
const STATE_OFS: usize = 0xB4;
/// `PGF.GiftUsed`: the deliveryman has handed this card's gift over.
const STATE_COLLECTED: u8 = 2;

/// Why a card can't be delivered. The values are shared with the Gen 4 status codes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal5 {
    /// The card's received flag is set: the game would refuse it too.
    AlreadyReceived = 1,
    /// The card is in the album, not yet collected.
    AlreadyPending = 2,
    /// Not a Gen 5 save, or the save isn't one the card was offered to.
    Unavailable = 5,
    /// All 12 album slots are in use (the player can discard one in-game).
    AlbumFull = 6,
}

/// Bit in [`Event5::games`] for this save. A save with an unexpected version byte counts as
/// either game of its pair.
fn game_mask(kind: SaveKind, version: u8) -> u8 {
    match (kind, version) {
        (SaveKind::BW, 20) => 1,
        (SaveKind::BW, 21) => 2,
        (SaveKind::BW, _) => 3,
        (SaveKind::B2W2, 22) => 4,
        (SaveKind::B2W2, 23) => 8,
        (SaveKind::B2W2, _) => 12,
        _ => 0,
    }
}

/// True when the card was distributed to this game and language.
pub fn available(event: Event5, kind: SaveKind, version: u8, language: u8) -> bool {
    event.games & game_mask(kind, version) != 0 && event.language == language
}

/// `PokeCrypto.CryptArray`; applying it twice restores the input.
fn crypt(data: &mut [u8], mut seed: u32) {
    for w in data.as_chunks_mut::<2>().0 {
        seed = seed.wrapping_mul(0x41C6_4E6D).wrapping_add(0x6073);
        *w = ((seed >> 16) as u16 ^ u16::from_le_bytes(*w)).to_le_bytes();
    }
}

fn card(m: &[u8], i: usize) -> &[u8] {
    &m[FLAGS_SIZE + i * PGF_SIZE..FLAGS_SIZE + (i + 1) * PGF_SIZE]
}

fn card_id(c: &[u8]) -> u16 {
    u16::from_le_bytes([c[CARD_ID_OFS], c[CARD_ID_OFS + 1]])
}

fn decrypted(block: &[u8]) -> ([u8; DATA_SIZE], u32) {
    let seed = u32::from_le_bytes([
        block[SEED_OFS],
        block[SEED_OFS + 1],
        block[SEED_OFS + 2],
        block[SEED_OFS + 3],
    ]);
    let mut m = [0u8; DATA_SIZE];
    m.copy_from_slice(&block[..DATA_SIZE]);
    crypt(&mut m, seed);
    (m, seed)
}

fn verdict(m: &[u8], gift: &Pgf) -> Result<usize, Refusal5> {
    let id = gift.card_id();
    // The album first: a card that's still waiting has its flag set too.
    for i in 0..CARD_COUNT {
        let c = card(m, i);
        if c[CARD_TYPE_OFS] != 0 && card_id(c) == id {
            return Err(if c[STATE_OFS] & STATE_COLLECTED == 0 {
                Refusal5::AlreadyPending
            } else {
                Refusal5::AlreadyReceived
            });
        }
    }
    if m[id as usize / 8] & (1 << (id % 8)) != 0 {
        return Err(Refusal5::AlreadyReceived);
    }
    (0..CARD_COUNT)
        .find(|&i| card(m, i)[CARD_TYPE_OFS] == 0)
        .ok_or(Refusal5::AlbumFull)
}

/// Whether `gift` can be delivered to this save now; `Err` says why not.
pub fn check(save: &Save, gift: &Pgf) -> Result<(), Refusal5> {
    let block = save.mystery5().ok_or(Refusal5::Unavailable)?;
    verdict(&decrypted(block).0, gift).map(|_| ())
}

/// Delivers `gift`, dated `date` (year, month, day) as the game dates a card it receives. The
/// save is not finalised.
pub fn inject(save: &mut Save, gift: &Pgf, date: (u16, u8, u8)) -> Result<(), Refusal5> {
    let block = save.mystery5_mut().ok_or(Refusal5::Unavailable)?;
    let (mut m, seed) = decrypted(block);
    let slot = verdict(&m, gift)?;
    let ofs = FLAGS_SIZE + slot * PGF_SIZE;
    m[ofs..ofs + PGF_SIZE].copy_from_slice(gift.0);
    m[ofs + DATE_OFS] = date.2;
    m[ofs + DATE_OFS + 1] = date.1;
    m[ofs + DATE_OFS + 2..ofs + DATE_OFS + 4].copy_from_slice(&date.0.to_le_bytes());
    let id = gift.card_id() as usize;
    m[id / 8] |= 1 << (id % 8);
    crypt(&mut m, seed);
    block[..DATA_SIZE].copy_from_slice(&m);
    Ok(())
}
