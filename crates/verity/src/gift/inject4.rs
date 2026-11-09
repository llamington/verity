//! Delivers a Gen 4 Mystery Gift the way receiving it over Wi-Fi or local wireless does: the card
//! goes in the album, the gift in a pending slot for the deliveryman, and the card's "received"
//! flag is set. The game then hands the gift over itself and sets its own event flags. Save
//! layout ported from PKHeX `Saves/SAV4.cs` (`MysteryBlock4*`); what the game writes was checked
//! against genuine saves, see `docs/research/events-region.md`.

use crate::gift::pcd::{gift_type, Gift4, PCD_SIZE, PGT_SIZE, RANGER_INDEX};
use crate::pkm::crypto::encrypt_45;
use crate::pkm::SIZE_4PARTY;
use crate::save::{Save, SaveKind};

const FLAGS_SIZE: usize = 0x100;
const PGT_COUNT: usize = 8;
const PCD_COUNT: usize = 3;
/// The received flag the game uses for "the deliveryman has something" (PKHeX).
const FLAG_DELIVERY_ACTIVE: usize = 2047;
/// D/P slot-in-use marker (PKHeX `MysteryGiftDPSlotActive`).
const DP_SLOT_ACTIVE: u32 = 0xEDB8_8320;
const DP_SENTINEL_COUNT: usize = 11;
/// General block byte holding the "Mystery Gift is on the menu" bit (PKHeX `IsMysteryGiftUnlocked`).
const UNLOCK_OFS: usize = 72;
const CARD_ID_OFS: usize = 0x150;
/// Header bit: the card can be shared with other players (`MysteryGiftEventHeader.shareable`).
const CARD_FLAGS_OFS: usize = 0x152;
const FLAG_SHAREABLE: u8 = 0x10;
const SHARES_LEFT_OFS: usize = 0x348;
const TIMES_SHARED_OFS: usize = 0x350;
const DATE_OFS: usize = 0x354;
const PK_OFS: usize = 8;

/// Why a gift can't be delivered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// The card's received flag is set: the game would refuse it too.
    AlreadyReceived = 1,
    /// The same gift is waiting at the deliveryman.
    AlreadyPending = 2,
    /// All 8 pending-gift slots are in use.
    PendingFull = 3,
    /// All 3 Wonder Card slots are in use (the player can discard one in-game).
    AlbumFull = 4,
    /// Not a Gen 4 save, or not a card that can be delivered.
    Unavailable = 5,
}

/// Bit in [`Gift4::games`] covering the save's game. A D/P or HG/SS save does not record which of
/// the pair it is, so either one counts.
fn game_mask(kind: SaveKind) -> u8 {
    match kind {
        SaveKind::DP => 0b00011,
        SaveKind::Pt => 0b00100,
        SaveKind::HGSS => 0b11000,
        _ => 0,
    }
}

/// True when the event belongs on a save of this game and language.
pub fn available(gift: &Gift4, kind: SaveKind, language: u8) -> bool {
    if !gift.injectable() || gift.games() & game_mask(kind) == 0 {
        return false;
    }
    match gift.language() {
        // Ranger Manaphy: any language except Korean (PKHeX `PGT.IsRangerManaphy`).
        0 => language != 8,
        l => l == language,
    }
}

fn u16_at(b: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([b[o], b[o + 1]])
}

/// The game treats any other type as an empty slot (`CheckIsValidWcType`; PKHeX `GiftType4`).
fn valid_type(t: u16) -> bool {
    (1..=15).contains(&t)
}

fn get_flag(flags: &[u8], i: usize) -> bool {
    flags[i / 8] & (1 << (i % 8)) != 0
}

fn set_flag(flags: &mut [u8], i: usize) {
    flags[i / 8] |= 1 << (i % 8);
}

/// Offsets within the Mystery Gift block.
struct Block {
    sentinels: bool,
}

impl Block {
    fn new(kind: SaveKind) -> Self {
        Self {
            sentinels: kind == SaveKind::DP,
        }
    }

    fn pgt(&self, i: usize) -> usize {
        FLAGS_SIZE
            + if self.sentinels {
                DP_SENTINEL_COUNT * 4
            } else {
                0
            }
            + i * PGT_SIZE
    }

    fn pcd(&self, i: usize) -> usize {
        self.pgt(PGT_COUNT) + i * PCD_SIZE
    }

    fn free_pgt(&self, m: &[u8]) -> Option<usize> {
        (0..PGT_COUNT).find(|&i| !valid_type(u16_at(m, self.pgt(i))))
    }

    /// Discarding a card only clears its type, so the type alone decides whether a slot is in
    /// use (`MysteryGift_CheckFreeWcSlotExists`).
    fn free_pcd(&self, m: &[u8]) -> Option<usize> {
        (0..PCD_COUNT).find(|&i| !valid_type(u16_at(m, self.pcd(i))))
    }

    fn ranger_pending(&self, m: &[u8]) -> bool {
        (0..PGT_COUNT).any(|i| u16_at(m, self.pgt(i)) == gift_type::MANAPHY_EGG)
    }
}

/// Whether `gift` can be delivered to this save now; `Err` says why not.
pub fn check(save: &Save, gift: &Gift4) -> Result<(), Refusal> {
    let kind = save.kind();
    let (general, mystery) = save.mystery4().ok_or(Refusal::Unavailable)?;
    if !gift.injectable() {
        return Err(Refusal::Unavailable);
    }
    let m = &general[mystery..];
    let b = Block::new(kind);
    if gift.mgdb_index() == RANGER_INDEX {
        if b.ranger_pending(m) {
            return Err(Refusal::AlreadyPending);
        }
        return b.free_pgt(m).map(|_| ()).ok_or(Refusal::PendingFull);
    }
    let id = gift.card_id();
    if get_flag(m, id as usize) {
        return Err(Refusal::AlreadyReceived);
    }
    let held = |i: usize| {
        let o = b.pcd(i);
        valid_type(u16_at(m, o)) && u16_at(m, o + CARD_ID_OFS) == id
    };
    if (0..PCD_COUNT).any(held) {
        return Err(Refusal::AlreadyPending);
    }
    if b.free_pcd(m).is_none() {
        return Err(Refusal::AlbumFull);
    }
    b.free_pgt(m).map(|_| ()).ok_or(Refusal::PendingFull)
}

/// Delivers `gift`. `days` is the received date (days since 2000-01-01), stored on the card as
/// the game does. The save is not finalised.
pub fn inject(save: &mut Save, gift: &Gift4, days: u16) -> Result<(), Refusal> {
    check(save, gift)?;
    let kind = save.kind();
    let (general, mystery) = save.mystery4_mut().ok_or(Refusal::Unavailable)?;
    let b = Block::new(kind);

    let mut card = [0u8; PCD_SIZE];
    card.copy_from_slice(gift.pcd());
    if matches!(
        gift.card_type(),
        gift_type::POKEMON | gift_type::POKEMON_EGG
    ) {
        // Blob templates are decrypted; PKHeX `PGT.VerifyPKEncryption`.
        encrypt_45(&mut card[PK_OFS..PK_OFS + SIZE_4PARTY]);
    }

    let ranger = gift.mgdb_index() == RANGER_INDEX;
    let (pgt_slot, pcd_slot);
    {
        let m = &general[mystery..];
        pgt_slot = b.free_pgt(m).ok_or(Refusal::PendingFull)?;
        pcd_slot = if ranger {
            None
        } else {
            Some(b.free_pcd(m).ok_or(Refusal::AlbumFull)?)
        };
    }

    // The pending gift points at its card, and one past the last slot means no card. Pt and
    // HG/SS count from 0 (their decompilations; the field is 2 bits wide), D/P from 1 (PKHeX
    // `MysteryBlock4.UpdateSlotPGT`, which applies that to Pt too).
    let base = u8::from(kind == SaveKind::DP);
    let link = base + pcd_slot.unwrap_or(PCD_COUNT) as u8;

    let m = &mut general[mystery..];
    let po = b.pgt(pgt_slot);
    m[po..po + PGT_SIZE].copy_from_slice(&card[..PGT_SIZE]);
    m[po + 2] = link;

    if let Some(c) = pcd_slot {
        // As `SaveReceivedGift` does for a card it has just received.
        if card[CARD_FLAGS_OFS] & FLAG_SHAREABLE == 0 {
            card[SHARES_LEFT_OFS] = 0;
        }
        card[TIMES_SHARED_OFS] = 0;
        card[DATE_OFS..DATE_OFS + 4].copy_from_slice(&(days as u32).to_le_bytes());
        let co = b.pcd(c);
        m[co..co + PCD_SIZE].copy_from_slice(&card);
        set_flag(m, gift.card_id() as usize);
    }
    set_flag(m, FLAG_DELIVERY_ACTIVE);

    if b.sentinels {
        let mut mark = |slot: usize| {
            let o = FLAGS_SIZE + slot * 4;
            m[o..o + 4].copy_from_slice(&DP_SLOT_ACTIVE.to_le_bytes());
        };
        mark(pgt_slot);
        if let Some(c) = pcd_slot {
            mark(PGT_COUNT + c);
        }
    }

    general[UNLOCK_OFS] |= 1;
    Ok(())
}
