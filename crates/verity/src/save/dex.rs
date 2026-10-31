//! Pokédex. Ported from PKHeX `Saves/Substructures/PokeDex/Zukan4.cs` and `Zukan5.cs`,
//! including `SetDex` (register a Pokémon as caught). PKHeX quirks are kept for parity; see
//! docs/discrepancies.md.

use super::SaveKind;
use crate::pkm::Pkm;

const MAX_SPECIES_4: u16 = 493;
const MAX_SPECIES_5: u16 = 649;
const FORM_NONE: u8 = 0xFF;

fn get_flag(d: &[u8], ofs: usize, bit: usize) -> bool {
    (d[ofs] >> (bit & 7)) & 1 != 0
}

fn set_flag(d: &mut [u8], ofs: usize, bit: usize, value: bool) {
    let mask = 1u8 << (bit & 7);
    if value {
        d[ofs] |= mask;
    } else {
        d[ofs] &= !mask;
    }
}

pub fn set_dex(kind: SaveKind, dex: &mut [u8], pk: &Pkm) {
    match kind {
        SaveKind::BW | SaveKind::B2W2 => Zukan5::new(kind).set_dex(dex, pk),
        _ => Zukan4::new(kind).set_dex(dex, pk),
    }
}

pub fn caught(kind: SaveKind, dex: &[u8], species: u16) -> bool {
    match kind {
        SaveKind::BW | SaveKind::B2W2 => Zukan5::new(kind).caught(dex, species),
        _ => Zukan4::caught(dex, species),
    }
}

pub fn seen(kind: SaveKind, dex: &[u8], species: u16) -> bool {
    match kind {
        SaveKind::BW | SaveKind::B2W2 => Zukan5::new(kind).seen_any(dex, species),
        _ => Zukan4::seen(dex, species),
    }
}

struct Zukan4 {
    dp: bool,
    hgss: bool,
}

const Z4_REGION: usize = 0x40;
const Z4_FORM1: usize = 4 + 4 * Z4_REGION + 4;
static DP_LANG_SPECIES: [u16; 14] = [
    23, 25, 54, 77, 120, 129, 202, 214, 215, 216, 228, 278, 287, 315,
];

impl Zukan4 {
    fn new(kind: SaveKind) -> Self {
        Self {
            dp: kind == SaveKind::DP,
            hgss: kind == SaveKind::HGSS,
        }
    }

    fn region_flag(d: &[u8], region: usize, index: usize) -> bool {
        get_flag(d, 4 + region * Z4_REGION + (index >> 3), index)
    }

    fn set_region_flag(d: &mut [u8], region: usize, index: usize, v: bool) {
        set_flag(d, 4 + region * Z4_REGION + (index >> 3), index, v)
    }

    fn caught(d: &[u8], species: u16) -> bool {
        species != 0 && Self::region_flag(d, 0, species as usize - 1)
    }

    fn seen(d: &[u8], species: u16) -> bool {
        species != 0 && Self::region_flag(d, 1, species as usize - 1)
    }

    fn lang_flags(&self) -> usize {
        Z4_FORM1 + if self.hgss { 0x3C } else { 0x20 }
    }

    fn form2(&self) -> usize {
        self.lang_flags() + 0x1F4
    }

    fn set_dex(&self, d: &mut [u8], pk: &Pkm) {
        let species = pk.species();
        if species == 0 || species > MAX_SPECIES_4 || pk.is_egg() {
            return;
        }
        let i = species as usize - 1;
        let gender = pk.gender();
        Self::set_region_flag(d, 0, i, true);
        // SetSeenGender
        if !Self::region_flag(d, 1, i) {
            Self::set_region_flag(d, 2, i, gender == 1);
            Self::set_region_flag(d, 3, i, gender == 1);
        } else if Self::region_flag(d, 2, i) == Self::region_flag(d, 3, i) {
            Self::set_region_flag(d, 3, i, gender == 1);
        }
        Self::set_region_flag(d, 1, i, true);
        self.set_forms_for(d, species, pk.form(), gender);
        self.set_language(d, species, pk.language());
    }

    fn set_forms_for(&self, d: &mut [u8], species: u16, form: u8, gender: u8) {
        if species == 201 {
            let ofs = Z4_FORM1 + 4;
            for i in 0..0x1C {
                let v = d[ofs + i];
                if v == form {
                    return;
                }
                if v == FORM_NONE {
                    d[ofs + i] = form;
                    return;
                }
            }
            return;
        }
        let mut forms = [0u8; 8];
        let Some(n) = self.get_forms(d, species, &mut forms) else {
            return;
        };
        let forms = &mut forms[..n];
        let id = if species == 172 && self.hgss {
            if form == 1 {
                2
            } else {
                gender
            }
        } else {
            form
        };
        if forms.contains(&id) {
            return;
        }
        let Some(slot) = forms.iter().position(|&f| f == FORM_NONE) else {
            return;
        };
        forms[slot] = id;
        self.set_forms(d, species, forms);
    }

    fn get_forms(&self, d: &[u8], species: u16, out: &mut [u8; 8]) -> Option<usize> {
        let (value, bits, count) = match species {
            386 => {
                let v = d[4 + Z4_REGION - 1] as u32 | (d[4 + 2 * Z4_REGION - 1] as u32) << 8;
                (v, 4, 4)
            }
            422 => (d[Z4_FORM1] as u32, 1, 2),
            423 => (d[Z4_FORM1 + 1] as u32, 1, 2),
            412 => (d[Z4_FORM1 + 2] as u32, 2, 3),
            413 => (d[Z4_FORM1 + 3] as u32, 2, 3),
            _ if self.dp => return None,
            479 => {
                let o = self.form2();
                (
                    u32::from_le_bytes([d[o], d[o + 1], d[o + 2], d[o + 3]]),
                    3,
                    6,
                )
            }
            492 => (d[self.form2() + 4] as u32, 1, 2),
            487 => (d[self.form2() + 5] as u32, 1, 2),
            172 if self.hgss => (d[self.form2() + 6] as u32, 2, 3),
            _ => return None,
        };
        let mask = 0xFFu32 >> (8 - bits);
        for (i, f) in out.iter_mut().take(count).enumerate() {
            let v = (value >> (i * bits)) & mask;
            *f = if v == mask && bits > 1 {
                FORM_NONE
            } else {
                v as u8
            };
        }
        if bits == 1 && out[0] == out[1] && out[0] == 1 {
            out[0] = FORM_NONE;
            out[1] = FORM_NONE;
        }
        Some(count)
    }

    /// `SetDexFormValues`, including PKHeX's unmasked shift of 0xFF for empty slots.
    fn pack(forms: &[u8], bits: u32, count: u32) -> u32 {
        let mut value = 0xFFFF_FFFFu32.wrapping_shl(count * bits);
        for (i, &f) in forms.iter().enumerate() {
            value |= (f as u32).wrapping_shl(bits * i as u32);
        }
        value
    }

    fn set_forms(&self, d: &mut [u8], species: u16, forms: &[u8]) {
        match species {
            386 => {
                let v = Self::pack(forms, 4, 4);
                d[4 + Z4_REGION - 1] = v as u8;
                d[4 + 2 * Z4_REGION - 1] = (v >> 8) as u8;
            }
            422 => d[Z4_FORM1] = Self::pack(forms, 1, 2) as u8,
            423 => d[Z4_FORM1 + 1] = Self::pack(forms, 1, 2) as u8,
            412 => d[Z4_FORM1 + 2] = Self::pack(forms, 2, 3) as u8,
            413 => d[Z4_FORM1 + 3] = Self::pack(forms, 2, 3) as u8,
            _ if self.dp => {}
            479 => {
                let o = self.form2();
                d[o..o + 4].copy_from_slice(&Self::pack(forms, 3, 6).to_le_bytes());
            }
            492 => d[self.form2() + 4] = Self::pack(forms, 1, 2) as u8,
            487 => d[self.form2() + 5] = Self::pack(forms, 1, 2) as u8,
            172 if self.hgss => d[self.form2() + 6] = Self::pack(forms, 2, 3) as u8,
            _ => {}
        }
    }

    fn set_language(&self, d: &mut [u8], species: u16, language: u8) {
        let lang = match language as i32 - 1 {
            3 => 4,
            4 => 3,
            l if l > 5 => 0,
            l if l < 0 => 1,
            l => l,
        } as usize;
        let index = if self.dp {
            match DP_LANG_SPECIES.iter().position(|&s| s == species) {
                Some(p) => p + 1,
                None => return,
            }
        } else {
            species as usize
        };
        set_flag(d, self.lang_flags() + index, lang, true);
    }
}

struct Zukan5 {
    form_len: usize,
    b2w2: bool,
}

const Z5_BITS: usize = 0x54;
const Z5_CAUGHT: usize = 0x8;
const Z5_SEEN: usize = Z5_CAUGHT + Z5_BITS;
const Z5_DISP: usize = Z5_SEEN + Z5_BITS * 4;
const Z5_FORM: usize = Z5_DISP + Z5_BITS * 4;
const Z5_LANG_COUNT: usize = 7;

impl Zukan5 {
    fn new(kind: SaveKind) -> Self {
        let b2w2 = kind == SaveKind::B2W2;
        Self {
            form_len: if b2w2 { 11 } else { 9 },
            b2w2,
        }
    }

    fn lang_ofs(&self) -> usize {
        Z5_FORM + self.form_len * 4
    }

    fn spinda_ofs(&self) -> usize {
        self.lang_ofs() + 432
    }

    fn flag(d: &[u8], base: usize, bit: usize) -> bool {
        get_flag(d, base + (bit >> 3), bit)
    }

    fn set(d: &mut [u8], base: usize, bit: usize) {
        set_flag(d, base + (bit >> 3), bit, true)
    }

    fn caught(&self, d: &[u8], species: u16) -> bool {
        (1..=MAX_SPECIES_5).contains(&species) && Self::flag(d, Z5_CAUGHT, species as usize - 1)
    }

    fn seen_any(&self, d: &[u8], species: u16) -> bool {
        (1..=MAX_SPECIES_5).contains(&species)
            && (0..4).any(|r| Self::flag(d, Z5_SEEN + r * Z5_BITS, species as usize - 1))
    }

    fn form_index(&self, species: u16) -> (usize, usize) {
        if self.b2w2 {
            match species {
                646 => return (72, 3),
                647 => return (75, 2),
                642 => return (77, 2),
                641 => return (79, 2),
                645 => return (81, 2),
                _ => {}
            }
        }
        match species {
            201 => (0, 28),
            386 => (28, 4),
            492 => (32, 2),
            487 => (34, 2),
            479 => (36, 6),
            422 => (42, 2),
            423 => (44, 2),
            412 => (46, 3),
            413 => (49, 3),
            351 => (52, 4),
            421 => (56, 2),
            585 => (58, 4),
            586 => (62, 4),
            648 => (66, 2),
            555 => (68, 2),
            550 => (70, 2),
            _ => (0, 0),
        }
    }

    fn set_dex(&self, d: &mut [u8], pk: &Pkm) {
        let species = pk.species();
        if species == 0 || species > MAX_SPECIES_5 || pk.is_egg() {
            return;
        }
        let bit = species as usize - 1;
        if species == 327 && !self.seen_any(d, species) {
            let o = self.spinda_ofs();
            d[o..o + 4].copy_from_slice(&pk.pid().to_le_bytes());
        }
        let gender = pk.gender();
        let shiny = pk.is_shiny();
        let region = (if shiny { 2 } else { 0 }) | (gender as usize & 1);
        Self::set(d, Z5_SEEN + region * Z5_BITS, bit);
        Self::set(d, Z5_CAUGHT, bit);
        if !(0..4).any(|r| Self::flag(d, Z5_DISP + r * Z5_BITS, bit)) {
            Self::set(d, Z5_DISP + region * Z5_BITS, bit);
        }
        if species <= MAX_SPECIES_4 {
            let mut li = pk.language() as i32 - 1;
            if li > 5 {
                li -= 1;
            }
            if (0..=5).contains(&li) {
                Self::set(d, self.lang_ofs(), bit * Z5_LANG_COUNT + li as usize);
            }
        }

        let (index, count) = self.form_index(species);
        if count == 0 {
            return;
        }
        let limit = self.form_len * 8;
        let shiny_region = shiny as usize;
        if index < limit {
            Self::set(d, Z5_FORM + shiny_region * self.form_len, index);
        }
        let displayed = count > 1
            && index + count < limit
            && (0..count).any(|i| Self::flag(d, Z5_FORM + 2 * self.form_len, index + i));
        if displayed {
            return;
        }
        let form = pk.form() as usize;
        if form >= count {
            return;
        }
        if index + form < limit {
            Self::set(
                d,
                Z5_FORM + 2 * self.form_len + shiny_region * self.form_len,
                index + form,
            );
        }
    }
}
