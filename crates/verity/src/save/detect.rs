//! Ported from PKHeX `PKHeX.Core/Saves/Util/SaveUtil.cs` (`IsG4*`, `IsG5*`).

use super::{crc16_ccitt, gen4, read_u16, read_u32};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaveKind {
    DP,
    Pt,
    HGSS,
    BW,
    B2W2,
}

impl SaveKind {
    pub fn generation(self) -> u8 {
        match self {
            SaveKind::DP | SaveKind::Pt | SaveKind::HGSS => 4,
            SaveKind::BW | SaveKind::B2W2 => 5,
        }
    }
}

pub const SIZE_G4RAW: usize = 0x80000;
pub const SIZE_G5RAW: usize = 0x80000;
pub const SIZE_G5BW: usize = 0x24000;
pub const SIZE_G5B2W2: usize = 0x26000;

pub fn detect(data: &[u8]) -> Option<SaveKind> {
    if data.len() != SIZE_G4RAW {
        return None;
    }
    if is_valid_general_footer4(data, gen4::GENERAL_SIZE_DP) {
        return Some(SaveKind::DP);
    }
    if is_valid_general_footer4(data, gen4::GENERAL_SIZE_PT) {
        return Some(SaveKind::Pt);
    }
    if is_valid_general_footer4(data, gen4::GENERAL_SIZE_HGSS) {
        return Some(SaveKind::HGSS);
    }
    if is_valid_footer5(data, SIZE_G5BW, 0x8C) {
        return Some(SaveKind::BW);
    }
    if is_valid_footer5(data, SIZE_G5B2W2, 0x94) {
        return Some(SaveKind::B2W2);
    }
    None
}

/// Checks the second partition's general block footer (size, then SDK magic).
fn is_valid_general_footer4(data: &[u8], length: usize) -> bool {
    let general = &data[gen4::PARTITION_SIZE..gen4::PARTITION_SIZE + length];
    let size = read_u32(general, length - 0xC);
    if size as usize != length {
        return false;
    }
    let sdk = read_u32(general, length - 0x8);
    sdk == gen4::MAGIC_JAPAN_INTL || sdk == gen4::MAGIC_KOREAN
}

fn is_valid_footer5(data: &[u8], main_size: usize, info_length: usize) -> bool {
    let start = main_size - 0x100;
    let footer = &data[start..start + info_length + 0x10];
    let stored = read_u16(footer, footer.len() - 2);
    stored == crc16_ccitt(&footer[..info_length])
}
