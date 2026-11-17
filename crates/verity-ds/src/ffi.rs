//! Safe calls into the core through verity-ffi's C ABI functions. Each wrapper passes buffers it
//! borrows for the length it gives, which is all those functions need.

use core::ffi::c_void;

use blocksds::fs::{File, SeekFrom};
pub use verity_ffi::bag::{VerityBagItem as BagItem, VerityPouchView as PouchView};
pub use verity_ffi::edit::{
    VerityEdit as Edit, VerityEggRequest as EggRequest, VerityEncounter as Encounter,
    VerityGenRequest as GenRequest, VerityIvFix as IvFix, VeritySaveCheck as SaveCheck,
};
pub use verity_ffi::events::VerityEventView as EventView;
pub use verity_ffi::rom::{VerityGlyph as Glyph, VerityMoveInfo as MoveInfo};
pub use verity_ffi::ui::{
    BlobKind, VerityLegality as Legality, VerityLoc as Loc, VerityPkmView as PkmView,
};
pub use verity_ffi::unova::VerityUnovaPreview as UnovaPreview;
pub use verity_ffi::{VerityErr as Err, VeritySaveInfo as SaveInfo};
use verity_ffi::{bag, edit, events, keyboard, rom, ui, unova};

/// Plain C structs: integers and arrays of them, for which all zero bytes are a valid value.
///
/// # Safety
/// Implement only for such types.
pub unsafe trait Zeroed: Sized {
    fn zeroed() -> Self {
        // SAFETY: the trait's contract: all zero bytes are a valid `Self`.
        unsafe { core::mem::zeroed() }
    }
}

// SAFETY: each is a `#[repr(C)]` struct of integers, bools and arrays of them.
unsafe impl Zeroed for PkmView {}
// SAFETY: as above.
unsafe impl Zeroed for Legality {}
// SAFETY: as above.
unsafe impl Zeroed for EventView {}
// SAFETY: as above.
unsafe impl Zeroed for PouchView {}
// SAFETY: as above.
unsafe impl Zeroed for BagItem {}
// SAFETY: as above.
unsafe impl Zeroed for GenRequest {}
// SAFETY: as above.
unsafe impl Zeroed for EggRequest {}
// SAFETY: as above.
unsafe impl Zeroed for MoveInfo {}
// SAFETY: as above.
unsafe impl Zeroed for Glyph {}
// SAFETY: as above.
unsafe impl Zeroed for Encounter {}

/// A static C string from the core (status texts), without its nul.
fn static_text(p: *const u8) -> &'static [u8] {
    if p.is_null() {
        return b"";
    }
    // SAFETY: the core's status texts are nul-terminated statics.
    unsafe { core::ffi::CStr::from_ptr(p.cast()) }.to_bytes()
}

// ---- data and the save -----------------------------------------------------------------------

/// Attaches a blob, which the core keeps for the program's lifetime.
pub fn data_attach(kind: BlobKind, blob: &'static [u8]) -> Err {
    // SAFETY: `blob` lives for the program, as the core needs.
    unsafe { ui::verity_data_attach(kind as u32, blob.as_ptr(), blob.len()) }
}

pub fn save_probe(save: &mut [u8]) -> Option<SaveInfo> {
    let mut info = SaveInfo::default();
    // SAFETY: a borrowed buffer and an output struct.
    let r = unsafe { verity_ffi::verity_save_probe(save.as_mut_ptr(), save.len(), &mut info) };
    (r == Err::Ok).then_some(info)
}

pub fn box_count(save: &mut [u8]) -> u32 {
    // SAFETY: a borrowed buffer.
    unsafe { ui::verity_box_count(save.as_mut_ptr(), save.len()) }
}

pub fn box_name(save: &mut [u8], index: u32, out: &mut [u16; 21]) -> bool {
    // SAFETY: a borrowed buffer, and the 21 units the function writes at most.
    unsafe {
        ui::verity_box_name(save.as_mut_ptr(), save.len(), index, out.as_mut_ptr()) == Err::Ok
    }
}

pub fn slot_view(save: &mut [u8], loc: Loc) -> Option<PkmView> {
    let mut v = PkmView::zeroed();
    // SAFETY: a borrowed buffer and an output struct.
    let r = unsafe { ui::verity_slot_view(save.as_mut_ptr(), save.len(), loc, &mut v) };
    (r == Err::Ok).then_some(v)
}

pub fn legality(save: &mut [u8], loc: Loc) -> Option<Legality> {
    let mut l = Legality::zeroed();
    // SAFETY: a borrowed buffer and an output struct.
    let r = unsafe { ui::verity_legality(save.as_mut_ptr(), save.len(), loc, &mut l) };
    (r == Err::Ok).then_some(l)
}

pub fn name(category: u32, id: u32, out: &mut [u16]) -> usize {
    // SAFETY: `out` for its length.
    unsafe { ui::verity_name(category, id, out.as_mut_ptr(), out.len()) }
}

pub fn location_name(generation: u8, id: u16, out: &mut [u16]) -> usize {
    // SAFETY: `out` for its length.
    unsafe { ui::verity_location_name(generation, id, out.as_mut_ptr(), out.len()) }
}

pub fn save_finalize(save: &mut [u8]) -> Err {
    // SAFETY: a borrowed buffer.
    unsafe { edit::verity_save_finalize(save.as_mut_ptr(), save.len()) }
}

pub fn save_check(save: &mut [u8]) -> (Err, SaveCheck) {
    let mut c = SaveCheck::default();
    // SAFETY: a borrowed buffer and an output struct.
    let r = unsafe { edit::verity_save_check(save.as_mut_ptr(), save.len(), &mut c) };
    (r, c)
}

// ---- editing and generating --------------------------------------------------------------

pub fn wild_species(version: u8, out: &mut [u16]) -> usize {
    // SAFETY: `out` for its length.
    unsafe { edit::verity_wild_species(version, out.as_mut_ptr(), out.len()) }
}

pub fn wild_encounters(version: u8, species: u16, out: &mut [Encounter]) -> usize {
    // SAFETY: `out` for its length.
    unsafe { edit::verity_wild_encounters(version, species, out.as_mut_ptr(), out.len()) }
}

pub fn gen_insert(save: &mut [u8], req: &GenRequest, loc: Loc) -> u8 {
    // SAFETY: a borrowed buffer and request.
    unsafe { edit::verity_gen_insert(save.as_mut_ptr(), save.len(), req, loc) }
}

pub fn gen_status_text(code: u8) -> &'static [u8] {
    static_text(edit::verity_gen_status_text(code))
}

pub fn egg_species(version: u8, out: &mut [u16]) -> usize {
    // SAFETY: `out` for its length.
    unsafe { edit::verity_egg_species(version, out.as_mut_ptr(), out.len()) }
}

pub fn egg_insert(save: &mut [u8], req: &EggRequest, loc: Loc) -> u8 {
    // SAFETY: a borrowed buffer and request.
    unsafe { edit::verity_egg_insert(save.as_mut_ptr(), save.len(), req, loc) }
}

pub fn closest_ivs(save: &mut [u8], loc: Loc, target: &[u8; 6]) -> (u8, IvFix) {
    let mut fix = IvFix::default();
    // SAFETY: a borrowed buffer, the 6 target IVs and an output struct.
    let r = unsafe {
        edit::verity_closest_ivs(
            save.as_mut_ptr(),
            save.len(),
            loc,
            target.as_ptr(),
            &mut fix,
        )
    };
    (r, fix)
}

/// The legality an edit would give, and the game-safe issue code (0 none).
pub fn edit_preview(save: &mut [u8], loc: Loc, e: &Edit) -> (Err, Legality, u8) {
    let (mut l, mut issue) = (Legality::zeroed(), 0u8);
    // SAFETY: a borrowed buffer, edit and output values.
    let r = unsafe {
        edit::verity_edit_preview(save.as_mut_ptr(), save.len(), loc, e, &mut l, &mut issue)
    };
    (r, l, issue)
}

pub fn edit_apply(save: &mut [u8], loc: Loc, e: &Edit, mode: u8) -> u8 {
    // SAFETY: a borrowed buffer and edit.
    unsafe { edit::verity_edit_apply(save.as_mut_ptr(), save.len(), loc, e, mode) }
}

pub fn issue_text(code: u8) -> &'static [u8] {
    static_text(edit::verity_issue_text(code))
}

pub fn char_encodable(generation: u8, c: u16) -> bool {
    edit::verity_char_encodable(generation, c) != 0
}

/// Types `key` into UTF-16 `text` (`len` used, at most `max`); returns the core's result code.
pub fn keyboard_type(
    text: &mut [u16],
    len: &mut usize,
    max: usize,
    key: u16,
    generation: u8,
) -> u8 {
    // SAFETY: `text` for its length; `len` is a valid count in it.
    unsafe {
        keyboard::verity_keyboard_type(text.as_mut_ptr(), len, text.len(), max, key, generation)
    }
}

// ---- Mystery Gifts, the bag, Unova -----------------------------------------------------------

pub fn events_list(save: &mut [u8], out: &mut [EventView]) -> usize {
    // SAFETY: a borrowed buffer, and `out` for its length.
    unsafe {
        events::verity_events_list(save.as_mut_ptr(), save.len(), out.as_mut_ptr(), out.len())
    }
}

pub fn event_inject(save: &mut [u8], which: u8, record: u16, days: u16) -> u8 {
    // SAFETY: a borrowed buffer.
    unsafe { events::verity_event_inject(save.as_mut_ptr(), save.len(), which, record, days) }
}

pub fn event_status_text(code: u8) -> &'static [u8] {
    static_text(events::verity_event_status_text(code))
}

pub fn bag_pouches(save: &mut [u8], out: &mut [PouchView]) -> usize {
    // SAFETY: a borrowed buffer, and `out` for its length.
    unsafe { bag::verity_bag_pouches(save.as_mut_ptr(), save.len(), out.as_mut_ptr(), out.len()) }
}

pub fn bag_items(save: &mut [u8], pouch: u8, out: &mut [BagItem]) -> usize {
    // SAFETY: a borrowed buffer, and `out` for its length.
    unsafe {
        bag::verity_bag_items(
            save.as_mut_ptr(),
            save.len(),
            pouch,
            out.as_mut_ptr(),
            out.len(),
        )
    }
}

pub fn bag_addable(save: &mut [u8], pouch: u8, out: &mut [u16]) -> usize {
    // SAFETY: a borrowed buffer, and `out` for its length.
    unsafe {
        bag::verity_bag_addable(
            save.as_mut_ptr(),
            save.len(),
            pouch,
            out.as_mut_ptr(),
            out.len(),
        )
    }
}

pub fn bag_add(save: &mut [u8], pouch: u8, item: u16, count: u16) -> u8 {
    // SAFETY: a borrowed buffer.
    unsafe { bag::verity_bag_add(save.as_mut_ptr(), save.len(), pouch, item, count) }
}

pub fn bag_set_count(save: &mut [u8], pouch: u8, index: u16, count: u16) -> u8 {
    // SAFETY: a borrowed buffer.
    unsafe { bag::verity_bag_set_count(save.as_mut_ptr(), save.len(), pouch, index, count) }
}

pub fn bag_remove(save: &mut [u8], pouch: u8, index: u16) -> u8 {
    // SAFETY: a borrowed buffer.
    unsafe { bag::verity_bag_remove(save.as_mut_ptr(), save.len(), pouch, index) }
}

pub fn bag_status_text(code: u8) -> &'static [u8] {
    static_text(bag::verity_bag_status_text(code))
}

pub fn unova_preview(save: &mut [u8], other: &[u8]) -> UnovaPreview {
    let mut p = UnovaPreview::default();
    // SAFETY: borrowed buffers and an output struct.
    unsafe {
        unova::verity_unova_preview(
            save.as_mut_ptr(),
            save.len(),
            other.as_ptr(),
            other.len(),
            &mut p,
        )
    };
    p
}

pub fn unova_apply(save: &mut [u8], other: &[u8], seed: u32) -> u8 {
    // SAFETY: borrowed buffers.
    unsafe {
        unova::verity_unova_apply(
            save.as_mut_ptr(),
            save.len(),
            other.as_ptr(),
            other.len(),
            seed,
        )
    }
}

pub fn unova_enable_preview(save: &mut [u8]) -> UnovaPreview {
    let mut p = UnovaPreview::default();
    // SAFETY: a borrowed buffer and an output struct.
    unsafe { unova::verity_unova_enable_preview(save.as_mut_ptr(), save.len(), &mut p) };
    p
}

pub fn unova_enable_all(save: &mut [u8]) -> u8 {
    // SAFETY: a borrowed buffer.
    unsafe { unova::verity_unova_enable_all(save.as_mut_ptr(), save.len()) }
}

pub fn unova_status_text(code: u8) -> &'static [u8] {
    static_text(unova::verity_unova_status_text(code))
}

// ---- ROM caches ----------------------------------------------------------------------------

pub fn rom_font_rank(code: &[u8; 4]) -> u8 {
    // SAFETY: the 4 bytes of a game code.
    unsafe { rom::verity_rom_font_rank(code.as_ptr()) }
}

/// `ctx` is a `File` the builders read the ROM from.
unsafe extern "C" fn read_rom(ctx: *mut c_void, offset: u32, buf: *mut u8, len: u32) -> bool {
    // SAFETY: the builders pass back the `&mut File` `build` gave them, and a buffer of `len`
    // bytes.
    let (f, buf) = unsafe {
        (
            &mut *ctx.cast::<File>(),
            core::slice::from_raw_parts_mut(buf, len as usize),
        )
    };
    f.seek(SeekFrom::Start(offset)).is_ok() && f.read_exact(buf).is_ok()
}

type Builder = unsafe extern "C" fn(Option<rom::ReadFn>, *mut c_void, *mut usize) -> *mut u8;

/// A cache the core built from a ROM, freed by the core when dropped.
pub struct Built(*mut u8, usize);

impl Built {
    pub fn bytes(&self) -> &[u8] {
        // SAFETY: the core returned `self.1` bytes at `self.0`.
        unsafe { core::slice::from_raw_parts(self.0, self.1) }
    }
}

impl Drop for Built {
    fn drop(&mut self) {
        // SAFETY: a buffer `verity_rom_build_*` returned, freed once.
        unsafe { rom::verity_rom_free(self.0, self.1) }
    }
}

fn build(builder: Builder, rom: &mut File) -> Option<Built> {
    let mut len = 0;
    // SAFETY: `read_rom` with the `File` it expects as its context, and an output length.
    let p = unsafe { builder(Some(read_rom), (rom as *mut File).cast(), &mut len) };
    (!p.is_null()).then_some(Built(p, len))
}

pub fn build_font_cache(rom: &mut File) -> Option<Built> {
    build(rom::verity_rom_build_font_cache, rom)
}

pub fn build_icon_cache(rom: &mut File) -> Option<Built> {
    build(rom::verity_rom_build_icon_cache, rom)
}

pub fn build_ribbon_cache(rom: &mut File) -> Option<Built> {
    build(rom::verity_rom_build_ribbon_cache, rom)
}

pub fn build_items_cache(rom: &mut File) -> Option<Built> {
    build(rom::verity_rom_build_items_cache, rom)
}

pub fn build_moves_cache(rom: &mut File) -> Option<Built> {
    build(rom::verity_rom_build_moves_cache, rom)
}

/// `ctx` is the `File` the sprite cache is written to.
unsafe extern "C" fn write_file(ctx: *mut c_void, buf: *const u8, len: u32) -> bool {
    // SAFETY: the builder passes back the `&mut File` given it, and `len` bytes at `buf`.
    let (f, buf) = unsafe {
        (
            &mut *ctx.cast::<File>(),
            core::slice::from_raw_parts(buf, len as usize),
        )
    };
    f.write_all(buf).is_ok()
}

/// Streams the sprite cache from `rom` to `out`; the bytes written (0 on failure).
pub fn build_sprite_cache(rom: &mut File, out: &mut File) -> u32 {
    // SAFETY: the callbacks with the `File`s they expect as their contexts.
    unsafe {
        rom::verity_rom_build_sprite_cache(
            Some(read_rom),
            (rom as *mut File).cast(),
            Some(write_file),
            (out as *mut File).cast(),
        )
    }
}

pub fn font_check(cache: &[u8]) -> bool {
    // SAFETY: a borrowed buffer.
    unsafe { rom::verity_font_check(cache.as_ptr(), cache.len()) }
}

pub fn font_glyph(cache: &[u8], c: u16) -> Option<Glyph> {
    let mut g = Glyph::zeroed();
    // SAFETY: a borrowed buffer and an output struct.
    unsafe { rom::verity_font_glyph(cache.as_ptr(), cache.len(), c, &mut g) }.then_some(g)
}

pub fn icons_head_len(head: &[u8]) -> usize {
    // SAFETY: a borrowed buffer.
    unsafe { rom::verity_icons_head_len(head.as_ptr(), head.len()) }
}

pub fn icons_palette_count(head: &[u8]) -> u8 {
    // SAFETY: a borrowed buffer.
    unsafe { rom::verity_icons_palette_count(head.as_ptr(), head.len()) }
}

pub fn icons_palette(head: &[u8], index: u8, out: &mut [u16; 16]) -> bool {
    // SAFETY: a borrowed buffer and the 16 colours written.
    unsafe { rom::verity_icons_palette(head.as_ptr(), head.len(), index, out.as_mut_ptr()) }
}

/// The palette and tile offset of a box icon.
pub fn icons_find(
    head: &[u8],
    species: u16,
    form: u8,
    female: bool,
    egg: bool,
) -> Option<(u8, u32)> {
    let (mut palette, mut at) = (0, 0);
    // SAFETY: a borrowed buffer and output values.
    let ok = unsafe {
        rom::verity_icons_find(
            head.as_ptr(),
            head.len(),
            species,
            form,
            female.into(),
            egg.into(),
            &mut palette,
            &mut at,
        )
    };
    ok.then_some((palette, at))
}

pub fn ribbons_check(cache: &[u8]) -> bool {
    // SAFETY: a borrowed buffer.
    unsafe { rom::verity_ribbons_check(cache.as_ptr(), cache.len()) }
}

pub fn ribbon_icon(cache: &[u8], index: u8, tiles: &mut [u8; 512], colors: &mut [u16; 16]) -> bool {
    // SAFETY: a borrowed buffer, and the 512 tile bytes and 16 colours written.
    unsafe {
        rom::verity_ribbon_icon(
            cache.as_ptr(),
            cache.len(),
            index,
            tiles.as_mut_ptr(),
            colors.as_mut_ptr(),
        )
    }
}

pub fn items_head_len(head: &[u8]) -> usize {
    // SAFETY: a borrowed buffer.
    unsafe { rom::verity_items_head_len(head.as_ptr(), head.len()) }
}

pub fn items_generation(head: &[u8]) -> u8 {
    // SAFETY: a borrowed buffer.
    unsafe { rom::verity_items_generation(head.as_ptr(), head.len()) }
}

/// The palette and tile offsets of an item icon.
pub fn items_find(head: &[u8], item: u16) -> Option<(u32, u32)> {
    let (mut pal, mut at) = (0, 0);
    // SAFETY: a borrowed buffer and output values.
    unsafe { rom::verity_items_find(head.as_ptr(), head.len(), item, &mut pal, &mut at) }
        .then_some((pal, at))
}

pub const SPRITES_FOOTER: usize = 24;

pub fn sprites_tail_start(footer: &[u8; SPRITES_FOOTER], file_len: u32) -> u32 {
    // SAFETY: a borrowed buffer.
    unsafe { rom::verity_sprites_tail_start(footer.as_ptr(), footer.len(), file_len) }
}

pub fn sprites_generation(tail: &[u8], start: u32) -> u8 {
    // SAFETY: a borrowed buffer.
    unsafe { rom::verity_sprites_generation(tail.as_ptr(), tail.len(), start) }
}

/// The palette offset, tile offset and tile length of a Pokémon's sprite.
pub fn sprites_find(
    tail: &[u8],
    start: u32,
    species: u16,
    form: u8,
    female: bool,
    egg: bool,
) -> Option<(u32, u32, u32)> {
    let (mut pal, mut at, mut len) = (0, 0, 0);
    // SAFETY: a borrowed buffer and output values.
    let ok = unsafe {
        rom::verity_sprites_find(
            tail.as_ptr(),
            tail.len(),
            start,
            species,
            form,
            female,
            egg,
            &mut pal,
            &mut at,
            &mut len,
        )
    };
    ok.then_some((pal, at, len))
}

pub fn sprite_decode(blob: &[u8], out: &mut [u8; 4608]) -> bool {
    // SAFETY: a borrowed buffer and the 4608 bytes written.
    unsafe { rom::verity_sprite_decode(blob.as_ptr(), blob.len(), out.as_mut_ptr()) }
}

pub fn sprites_icon(
    tail: &[u8],
    start: u32,
    kind: u8,
    index: u8,
    tiles: &mut [u8; 256],
    colors: &mut [u16; 16],
) -> bool {
    // SAFETY: a borrowed buffer, and the 256 tile bytes and 16 colours written.
    unsafe {
        rom::verity_sprites_icon(
            tail.as_ptr(),
            tail.len(),
            start,
            kind,
            index,
            tiles.as_mut_ptr(),
            colors.as_mut_ptr(),
        )
    }
}

pub fn moves_generation(cache: &[u8]) -> u8 {
    // SAFETY: a borrowed buffer.
    unsafe { rom::verity_moves_generation(cache.as_ptr(), cache.len()) }
}

pub fn move_info(generation: u8, mv: u16, cache: &[u8]) -> Option<MoveInfo> {
    let mut m = MoveInfo::zeroed();
    // SAFETY: a borrowed (possibly empty) buffer and an output struct.
    unsafe { rom::verity_move_info(generation, mv, cache.as_ptr(), cache.len(), &mut m) }
        .then_some(m)
}

pub fn move_description(generation: u8, mv: u16, cache: &[u8], out: &mut [u8]) -> bool {
    // SAFETY: a borrowed (possibly empty) buffer, and `out` for its length.
    unsafe {
        rom::verity_move_description(
            generation,
            mv,
            cache.as_ptr(),
            cache.len(),
            out.as_mut_ptr(),
            out.len(),
        )
    }
}
