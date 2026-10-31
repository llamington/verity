//! Gen 4 save layout. Ported from PKHeX `PKHeX.Core/Saves/SAV4.cs`, `SAV4Sinnoh.cs`,
//! `SAV4DP.cs`, `SAV4Pt.cs`, `SAV4HGSS.cs`, `Blocks/BlockInfo4.cs` and
//! `Util/SAV4BlockDetection.cs`.

use super::{crc16_ccitt, read_u32, write_u16, SaveKind, BOX_SLOTS};
use crate::pkm::{SIZE_4PARTY, SIZE_4STORED};

pub const PARTITION_SIZE: usize = 0x40000;
pub const MAGIC_JAPAN_INTL: u32 = 0x2006_0623;
pub const MAGIC_KOREAN: u32 = 0x2007_0903;

pub const GENERAL_SIZE_DP: usize = 0xC100;
pub const GENERAL_SIZE_PT: usize = 0xCF2C;
pub const GENERAL_SIZE_HGSS: usize = 0xF628;
const STORAGE_SIZE_DP: usize = 0x121E0;
const STORAGE_SIZE_PT: usize = 0x121E4;
const STORAGE_SIZE_HGSS: usize = 0x12310;
const GENERAL_GAP_HGSS: usize = 0xD8;

pub const BOX_COUNT: usize = 18;
const BOX_DATA_LEN_SINNOH: usize = BOX_SLOTS * SIZE_4STORED;

const EXTRA_FOOTER: usize = 0x10;

#[derive(Clone, Copy)]
struct Extra {
    id: u32,
    offset: usize,
    length: usize,
}

const fn extra(id: u32, offset: usize, length: usize) -> Extra {
    Extra { id, offset, length }
}

static EXTRA_DP: [Extra; 1] = [extra(0, 0x20000, 0x2AC0)];
static EXTRA_PT: [Extra; 6] = [
    extra(0, 0x20000, 0x2AC0),
    extra(1, 0x23000, 0x0BB0),
    extra(2, 0x24000, 0x1D60),
    extra(3, 0x26000, 0x1D60),
    extra(4, 0x28000, 0x1D60),
    extra(5, 0x2A000, 0x1D60),
];
static EXTRA_HGSS: [Extra; 6] = [
    extra(0, 0x23000, 0x2AC0),
    extra(1, 0x26000, 0x0BB0),
    extra(2, 0x27000, 0x1D60),
    extra(3, 0x29000, 0x1D60),
    extra(4, 0x2B000, 0x1D60),
    extra(5, 0x2D000, 0x1D60),
];

impl Extra {
    fn footer(&self) -> usize {
        self.offset + self.length - EXTRA_FOOTER
    }

    fn is_initialized(&self, data: &[u8]) -> bool {
        if self.id == 0 {
            read_u32(data, self.footer() + 4) != 0xFFFF_FFFF
        } else {
            read_u32(data, self.offset) != 0xFFFF_FFFF
        }
    }

    fn checksum(&self, data: &[u8]) -> u16 {
        crc16_ccitt(&data[self.offset..self.offset + self.length - 2])
    }

    fn checksum_valid(&self, data: &[u8]) -> bool {
        !self.is_initialized(data)
            || self.checksum(data) == super::read_u16(data, self.footer() + 14)
    }

    fn set_checksum(&self, data: &mut [u8]) {
        if self.is_initialized(data) {
            let chk = self.checksum(data);
            write_u16(data, self.footer() + 14, chk);
        }
    }
}

pub struct Layout4 {
    kind: SaveKind,
    general: usize,
    storage: usize,
    backup_general: usize,
    backup_storage: usize,
    general_size: usize,
    storage_size: usize,
    footer_size: usize,
    trainer: usize,
    party: usize,
    extra: &'static [Extra],
}

impl Layout4 {
    pub fn new(kind: SaveKind, data: &[u8]) -> Self {
        let (general_size, storage_size, storage_start, footer_size, trainer, party, extra): (
            _,
            _,
            _,
            _,
            _,
            _,
            &'static [Extra],
        ) = match kind {
            SaveKind::DP => (
                GENERAL_SIZE_DP,
                STORAGE_SIZE_DP,
                GENERAL_SIZE_DP,
                0x14,
                0x64,
                0x98,
                &EXTRA_DP,
            ),
            SaveKind::Pt => (
                GENERAL_SIZE_PT,
                STORAGE_SIZE_PT,
                GENERAL_SIZE_PT,
                0x14,
                0x68,
                0xA0,
                &EXTRA_PT,
            ),
            SaveKind::HGSS => (
                GENERAL_SIZE_HGSS,
                STORAGE_SIZE_HGSS,
                GENERAL_SIZE_HGSS + GENERAL_GAP_HGSS,
                0x10,
                0x64,
                0x98,
                &EXTRA_HGSS,
            ),
            _ => unreachable!(),
        };

        let g = active_block(data, 0, general_size);
        let s = active_block(data, storage_start, storage_size);
        let pick = |second: bool| if second { PARTITION_SIZE } else { 0 };

        Self {
            kind,
            general: pick(g),
            storage: pick(s) + storage_start,
            backup_general: pick(!g),
            backup_storage: pick(!s) + storage_start,
            general_size,
            storage_size,
            footer_size,
            trainer,
            party,
            extra,
        }
    }

    pub fn party_count_offset(&self) -> usize {
        self.general + self.party - 4
    }

    pub fn party_offset(&self, slot: usize) -> usize {
        self.general + self.party + SIZE_4PARTY * slot
    }

    pub fn box_slot_offset(&self, b: usize, s: usize) -> usize {
        let box_ofs = match self.kind {
            SaveKind::HGSS => b * 0x1000,
            _ => 4 + b * BOX_DATA_LEN_SINNOH,
        };
        self.storage + box_ofs + s * SIZE_4STORED
    }

    pub fn ot_offset(&self) -> usize {
        self.general + self.trainer
    }

    pub fn tid_offset(&self) -> usize {
        self.general + self.trainer + 0x10
    }

    fn block_checksum(&self, data: &[u8], start: usize, size: usize) -> u16 {
        crc16_ccitt(&data[start..start + size - self.footer_size])
    }

    fn block_valid(&self, data: &[u8], start: usize, size: usize) -> bool {
        self.block_checksum(data, start, size) == super::read_u16(data, start + size - 2)
    }

    pub fn checksums_valid(&self, data: &[u8]) -> bool {
        self.block_valid(data, self.general, self.general_size)
            && self.block_valid(data, self.storage, self.storage_size)
            && self.extra.iter().all(|e| e.checksum_valid(data))
            && self
                .extra
                .iter()
                .all(|e| e.checksum_valid(&data[PARTITION_SIZE..]))
    }

    pub fn set_checksums(&self, data: &mut [u8]) {
        let blocks = [
            (self.general, self.general_size, false),
            (self.storage, self.storage_size, false),
            (self.backup_general, self.general_size, true),
            (self.backup_storage, self.storage_size, true),
        ];
        for (start, size, backup) in blocks {
            if backup && read_u32(data, start + size - 8) == 0xFFFF_FFFF {
                continue;
            }
            let chk = self.block_checksum(data, start, size);
            write_u16(data, start + size - 2, chk);
        }
        for e in self.extra {
            e.set_checksum(data);
            e.set_checksum(&mut data[PARTITION_SIZE..]);
        }
    }
}

/// Returns true when the second partition holds the newer copy.
fn active_block(data: &[u8], begin: usize, length: usize) -> bool {
    let ofs = begin + length - 0x14;
    compare_footers(data, ofs, ofs + PARTITION_SIZE)
}

fn compare_footers(data: &[u8], ofs1: usize, ofs2: usize) -> bool {
    match compare_counters(read_u32(data, ofs1), read_u32(data, ofs2)) {
        Some(second) => second,
        None => compare_counters(read_u32(data, ofs1 + 4), read_u32(data, ofs2 + 4)) == Some(true),
    }
}

/// `Some(false)` = first is newer, `Some(true)` = second, `None` = same.
fn compare_counters(c1: u32, c2: u32) -> Option<bool> {
    if c1 == u32::MAX && c2 != u32::MAX - 1 {
        return Some(true);
    }
    if c2 == u32::MAX && c1 != u32::MAX - 1 {
        return Some(false);
    }
    match c1.cmp(&c2) {
        core::cmp::Ordering::Greater => Some(false),
        core::cmp::Ordering::Less => Some(true),
        core::cmp::Ordering::Equal => None,
    }
}
