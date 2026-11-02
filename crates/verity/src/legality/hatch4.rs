//! Gen 4 egg hatch locations. Generated from PKHeX 25.12.21
//! `Legality/Encounters/Verifiers/EggHatchLocation4.cs` (`LocationPermitted4`).

const MASK_DP: u8 = 1;
const MASK_PT: u8 = 2;
const MASK_HGSS: u8 = 4;

#[rustfmt::skip]
static PERMITTED: [u8; 235] = [
    0, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3,
    3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3,
    3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3,
    3, 3, 3, 2, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 2,
    7, 3, 3, 3, 3, 2, 0, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3,
    3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 6, 6, 6, 6, 6, 2, 6, 2,
    2, 2, 2, 2, 2, 2, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
    4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
    4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 0, 4, 4, 4, 4, 4, 4, 4, 4,
    4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
    4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
    4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 0, 4,
];

fn flag(mask: u8, location: u16) -> bool {
    PERMITTED
        .get(location as usize)
        .is_some_and(|f| f & mask != 0)
}

/// PKHeX `EggHatchLocation4.IsValidMet4`.
pub fn valid_met4(location: u16, version: u8) -> bool {
    match version {
        10 | 11 => flag(MASK_DP, location),
        12 => flag(MASK_PT, location),
        7 | 8 => flag(MASK_HGSS, location),
        _ => false,
    }
}

/// PKHeX `EggHatchLocation4.IsValidMet4Any`.
pub fn valid_met4_any(location: u16) -> bool {
    flag(MASK_DP | MASK_PT | MASK_HGSS, location)
}
