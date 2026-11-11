//! Gen 5 save layout. Ported from PKHeX `PKHeX.Core/Saves/SAV5.cs`,
//! `Blocks/BlockInfoNDS.cs`, `Substructures/Gen5/PlayerData5.cs`.

use super::gen5_blocks::{BLOCKS_B2W2, BLOCKS_BW, Block5};
use super::{BOX_SLOTS, SaveKind, crc16_ccitt, read_u16, write_u16};
use crate::pkm::{SIZE_5PARTY, SIZE_5STORED};

pub const BOX_COUNT: usize = 24;
const BOX: usize = 0x400;
const PARTY: usize = 0x18E00;
pub(super) const PLAYER_DATA: usize = 0x19400;
/// PKHeX block 34 (`SaveBlockAccessor5*`), the same in B/W and B2/W2.
pub(super) const MYSTERY_RANGE: core::ops::Range<usize> = 0x1C800..0x1C800 + 0xA94;
const MISC_BLOCK: usize = 52;
const ITEMS_BLOCK: usize = 25;
const EVENT_BLOCK: usize = 45;

pub struct Layout5 {
    blocks: &'static [Block5],
    dex_block: usize,
    b2w2: bool,
}

impl Layout5 {
    pub fn new(kind: SaveKind) -> Self {
        let blocks: &'static [Block5] = match kind {
            SaveKind::BW => &BLOCKS_BW,
            SaveKind::B2W2 => &BLOCKS_B2W2,
            _ => unreachable!(),
        };
        // `SaveBlockAccessor5BW.Zukan` is block 55, `SaveBlockAccessor5B2W2.Zukan` block 54.
        let dex_block = if kind == SaveKind::B2W2 { 54 } else { 55 };
        Self {
            blocks,
            dex_block,
            b2w2: kind == SaveKind::B2W2,
        }
    }

    pub fn party_count_offset(&self) -> usize {
        PARTY + 4
    }

    pub fn party_offset(&self, slot: usize) -> usize {
        PARTY + 8 + SIZE_5PARTY * slot
    }

    pub fn box_slot_offset(&self, b: usize, s: usize) -> usize {
        BOX + SIZE_5STORED * BOX_SLOTS * b + b * 0x10 + s * SIZE_5STORED
    }

    pub fn dex_offset(&self) -> usize {
        self.blocks[self.dex_block].offset as usize
    }

    pub fn event_layout(&self) -> (usize, usize, usize, usize) {
        let base = self.blocks[EVENT_BLOCK].offset as usize;
        let (work, flags) = if self.b2w2 {
            (0x1AF, 0xBF8)
        } else {
            (0x13E, 0xB60)
        };
        (base + work * 2, flags, base, work)
    }

    pub fn items_offset(&self) -> usize {
        self.blocks[ITEMS_BLOCK].offset as usize
    }

    pub fn misc_offset(&self) -> usize {
        self.blocks[MISC_BLOCK].offset as usize
    }

    pub fn ot_offset(&self) -> usize {
        PLAYER_DATA + 4
    }

    pub fn tid_offset(&self) -> usize {
        PLAYER_DATA + 0x14
    }

    pub fn checksums_valid(&self, data: &[u8]) -> bool {
        self.blocks.iter().all(|b| {
            let chk = checksum(data, b);
            chk == read_u16(data, b.chk as usize) && chk == read_u16(data, b.chk_mirror as usize)
        })
    }

    pub fn set_checksums(&self, data: &mut [u8]) {
        for b in self.blocks {
            let chk = checksum(data, b);
            write_u16(data, b.chk as usize, chk);
            write_u16(data, b.chk_mirror as usize, chk);
        }
    }
}

/// `BoxLayout5.GetBoxNameOffset` within block 0.
pub fn box_name_offset(b: usize) -> usize {
    0x28 * b + 4
}

fn checksum(data: &[u8], b: &Block5) -> u16 {
    crc16_ccitt(&data[b.offset as usize..(b.offset + b.length) as usize])
}
