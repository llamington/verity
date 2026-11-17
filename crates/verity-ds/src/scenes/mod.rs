//! Scenes: file picker, save overview, box view with party panel, and the screens they open.

use alloc::boxed::Box;
use alloc::vec::Vec;

use blocksds::fs;

use crate::data::{self, CPath};
use crate::text::{LongText, Text, Zero, eq_ignore_case, file_name};
use crate::ui::{
    self, Badge, Input, KEY_A, KEY_B, KEY_DOWN, KEY_L, KEY_R, KEY_TOUCH, KEY_UP, MUTED, Scene,
    bottom, top,
};
use crate::{ffi, long, t};

mod bag;
mod boxes;
mod editor;
mod gifts;
mod unova;
mod wizard;

pub use bag::bag;
pub use boxes::box_view;
pub use editor::editor;
pub use gifts::gifts;
pub use unova::unova_link;
pub use wizard::add_wizard;

// ---- shared list widget --------------------------------------------------------------------

#[derive(Clone, Copy)]
pub(crate) struct ListView {
    pub first_row: i32, // console row of the first item
    pub rows: i32,
    /// Line pitch in px (8: one console row per item). On a wider pitch item i's text is at
    /// text_y(i) and the highlight is `pitch` tall from text_y - 2.
    pub pitch: i32,
    pub pad: i32, // px from the top of first_row to the first item's text
    pub selected: i32,
    pub scroll: i32,
    pub count: i32,
    pub drag_scroll: i32,
}

impl Default for ListView {
    fn default() -> Self {
        Self {
            first_row: 2,
            rows: 20,
            pitch: 8,
            pad: 0,
            selected: 0,
            scroll: 0,
            count: 0,
            drag_scroll: 0,
        }
    }
}

impl ListView {
    pub fn clamp(&mut self) {
        if self.count == 0 {
            self.selected = 0;
            self.scroll = 0;
            return;
        }
        self.selected = self.selected.clamp(0, self.count - 1);
        if self.selected < self.scroll {
            self.scroll = self.selected;
        }
        if self.selected >= self.scroll + self.rows {
            self.scroll = self.selected - self.rows + 1;
        }
        let max_scroll = if self.count > self.rows {
            self.count - self.rows
        } else {
            0
        };
        self.scroll = self.scroll.min(max_scroll).max(0);
    }

    /// Returns the activated index (A, or a tap on the selected row), or -1, and whether the
    /// selection or scroll changed.
    pub fn update(&mut self, input: &Input) -> (i32, bool) {
        let (before_sel, before_scroll) = (self.selected, self.scroll);
        if input.rep & KEY_UP != 0 {
            self.selected -= 1;
        }
        if input.rep & KEY_DOWN != 0 {
            self.selected += 1;
        }
        if input.rep & KEY_L != 0 {
            self.selected -= self.rows;
        }
        if input.rep & KEY_R != 0 {
            self.selected += self.rows;
        }
        let mut activated = -1;
        if input.down & KEY_A != 0 && self.count > 0 {
            activated = self.selected;
        }
        if input.down & KEY_TOUCH != 0 {
            self.drag_scroll = self.scroll;
        }
        if input.touching && input.dragging {
            self.scroll = self.drag_scroll - input.drag_dy / self.pitch;
            let max_scroll = if self.count > self.rows {
                self.count - self.rows
            } else {
                0
            };
            self.scroll = self.scroll.max(0).min(max_scroll);
            if self.selected < self.scroll {
                self.selected = self.scroll;
            }
            if self.selected >= self.scroll + self.rows {
                self.selected = self.scroll + self.rows - 1;
            }
        } else {
            self.clamp();
        }
        if input.tapped {
            let off = input.tap_y - self.first_row * 8 - self.pad + (self.pitch - 8) / 2;
            let row = if self.pitch == 8 {
                input.tap_y / 8 - self.first_row
            } else if off < 0 {
                -1
            } else {
                off / self.pitch
            };
            let i = self.scroll + row;
            if row >= 0 && row < self.rows && i < self.count {
                if i == self.selected {
                    activated = i;
                }
                self.selected = i;
            }
        }
        (
            activated,
            self.selected != before_sel || self.scroll != before_scroll,
        )
    }

    pub fn bar_y(&self) -> i32 {
        // Centred on the 8 px text.
        if self.pitch == 8 {
            (self.first_row + self.selected - self.scroll) * 8
        } else {
            self.text_y(self.selected) - (self.pitch - 8) / 2
        }
    }

    pub fn text_y(&self, i: i32) -> i32 {
        self.first_row * 8 + self.pad + (i - self.scroll) * self.pitch
    }
}

/// A list on a wider pitch in a bottom panel whose content spans `cells` rows from first_row (the
/// whole screen between the title and hint bars by default). Short menus use GRID_PITCH.
pub(crate) fn spaced_list_at(pitch: i32, first_row: i32, cells: i32) -> ListView {
    let pad = 4;
    ListView {
        pitch,
        pad,
        first_row,
        rows: (cells * 8 - 2 * pad - 8) / pitch + 1,
        ..ListView::default()
    }
}

pub(crate) fn spaced_list() -> ListView {
    spaced_list_at(ui::PITCH, 2, 20)
}

/// Text x on a full-width bottom panel.
pub(crate) const LIST_X: i32 = ui::INSET_BOTTOM;

// ---- file picker ---------------------------------------------------------------------------

const MAX_FILES: usize = 256;
const PATH_LEN: usize = 192;

fn is_save(name: &[u8]) -> bool {
    name.iter()
        .rposition(|&b| b == b'.')
        .is_some_and(|i| eq_ignore_case(&name[i..], b".sav") || eq_ignore_case(&name[i..], b".dsv"))
}

/// Adds the save files under dir to files.
pub(crate) fn scan_saves(files: &mut Vec<LongText>, dir: &[u8], depth: i32) {
    let Ok(entries) = fs::read_dir(CPath::new(dir).as_cstr()) else {
        return;
    };
    for e in entries {
        if files.len() >= MAX_FILES {
            break;
        }
        let name = e.name().to_bytes();
        if name.first() == Some(&b'.') {
            continue;
        }
        let sep: &[u8] = if dir.last() == Some(&b'/') { b"" } else { b"/" };
        let path = long!(dir, sep, name);
        if path.is_empty() || path.len() >= PATH_LEN {
            continue;
        }
        if e.is_dir() {
            if depth > 0 {
                scan_saves(files, &path, depth - 1);
            }
        } else if is_save(name) {
            files.push(path);
        }
    }
}

struct FilePicker {
    files: Vec<LongText>,
    scanned: bool,
    framed: bool,
    list: ListView,
}

pub fn file_picker() -> Box<dyn Scene> {
    Box::new(FilePicker {
        files: Vec::new(),
        scanned: false,
        framed: false,
        list: spaced_list(),
    })
}

impl Scene for FilePicker {
    fn name(&self) -> &'static str {
        "files"
    }

    fn enter(&mut self) {
        if !self.scanned {
            self.scanned = true;
            for root in [&b"sd:/"[..], b"fat:/"] {
                scan_saves(&mut self.files, root, 3);
            }
            self.list.count = self.files.len() as i32;
            if let Some(last) = data::last(PATH_LEN) {
                for (i, f) in self.files.iter().enumerate() {
                    if **f == *last {
                        self.list.selected = i as i32;
                    }
                }
            }
            self.list.clamp();
        }
        self.framed = false;
        self.draw_top(b"");
        self.draw_list();
    }

    fn update(&mut self, input: &Input) -> bool {
        let (open, changed) = self.list.update(input);
        if changed {
            self.draw_list();
        }
        ui::bar(
            0,
            self.list.bar_y(),
            !self.files.is_empty(),
            self.list.pitch,
        );
        if open >= 0 {
            let path = self.files[open as usize];
            match data::open(&path) {
                Ok(()) => {
                    data::remember(&path);
                    let kind = data::doc().info.kind;
                    data::load_data_for(kind);
                    ui::push(overview());
                }
                Err(e) => self.draw_top(e.as_bytes()),
            }
        }
        true // the picker is the root scene
    }
}

impl FilePicker {
    fn draw_top(&mut self, message: &[u8]) {
        let count = self.files.len();
        ui::clear(top());
        ui::frame_clear(top());
        ui::title(top(), b"Verity");
        ui::title_right(top(), b"Gen 4/5 save editor");
        let has_message = !message.is_empty();
        ui::panel(top(), 1, 2, 30, if has_message { 6 } else { 3 });
        // The logo (48 px inside its 64 px sprite) centred in the gap between the panels: from
        // below this one's border to the bottom panel's border at row 17.
        let (gap_top, gap_bottom) = (if has_message { 9 } else { 6 } * 8, 17 * 8);
        ui::logo(128 - 32, (gap_top + gap_bottom) / 2 - 32);
        let mut l = ui::Lines::new(2);
        ui::text(
            top(),
            ui::INSET_TOP,
            l.line(),
            &t!(
                count,
                " save file",
                if count == 1 { "" } else { "s" },
                " found"
            ),
        );
        let open = {
            let d = data::doc();
            d.loaded().then(|| Text::from(file_name(&d.path)))
        };
        if let Some(name) = open {
            let name = crate::text::Prec(&name, 26);
            ui::text(
                top(),
                ui::INSET_TOP,
                l.line(),
                &t!(MUTED, "Open ", MUTED, " ", name),
            );
        }
        if has_message {
            l.block();
            ui::text(top(), ui::INSET_TOP, l.line(), &t!("! ", message));
        }
        ui::panel(top(), 1, 18, 30, 3);
        let mut r = ui::Lines::new(18);
        if ui::has_rom_font() {
            ui::text(top(), ui::INSET_TOP, r.line(), b"Names and icons come from");
            ui::text(
                top(),
                ui::INSET_TOP,
                r.line(),
                &t!("your ROM ", MUTED, "(", &data::font_game(), ")"),
            );
        } else {
            ui::text(
                top(),
                ui::INSET_TOP,
                r.line(),
                b"Put a Gen 4/5 ROM (.nds) on the SD",
            );
            ui::text(
                top(),
                ui::INSET_TOP,
                r.line(),
                b"card for JP/KR names and icons.",
            );
        }
    }

    fn draw_list(&mut self) {
        let list = self.list;
        if !self.framed {
            self.framed = true;
            ui::frame_clear(bottom());
            ui::panel(bottom(), 0, list.first_row, 32, 20);
            ui::hints(bottom(), b"A:Open|L/R:Page|Drag:Scroll");
        }
        ui::title(bottom(), b"Choose a save");
        ui::title_right(bottom(), &t!(self.files.len()).cap(16));
        ui::clear_px(bottom(), 16, 176);
        for r in 0..list.rows {
            let i = list.scroll + r;
            let Some(path) = self.files.get(i as usize) else {
                continue;
            };
            // File name, then its folder as a muted label at the right.
            let slash = path.iter().rposition(|&b| b == b'/');
            let name = slash.map_or(&path[..], |s| &path[s + 1..]);
            let mut dir = Text::new();
            if let Some(s) = slash.filter(|&s| s > 0) {
                let mut d = s - 1;
                while d > 0 && path[d] != b'/' && path[d] != b':' {
                    d -= 1;
                }
                dir = Text::from(&path[d + 1..s]).cap(24);
            }
            let y = list.text_y(i);
            let mut shown = LongText::from(name).cap(PATH_LEN);
            ui::fit(&mut shown, 244 - ui::text_width(&dir) - 10 - LIST_X);
            ui::text(bottom(), LIST_X, y, &shown);
            ui::text_right(bottom(), 244, y, &t!(MUTED, &dir));
        }
    }
}

// ---- overview ------------------------------------------------------------------------------

const BOXES: usize = 0;
const GIFTS: usize = 1;
const SAVE: usize = 2;
const CLOSE: usize = 3;
const UNOVA: usize = 4;
const BAG: usize = 5;
const ITEMS: [&str; 6] = [
    "Boxes and party",
    "Mystery gifts",
    "Save to SD card",
    "Close",
    "Memory Link and keys",
    "Bag",
];
const BLACK2_WHITE2: u8 = 5;

struct Overview {
    confirm_leave: bool,
    items: [usize; 6],
    count: usize,
    list: ListView,
    message: Text,
}

pub fn overview() -> Box<dyn Scene> {
    Box::new(Overview {
        confirm_leave: false,
        items: [0; 6],
        count: 0,
        list: ListView::default(),
        message: Text::new(),
    })
}

impl Scene for Overview {
    fn name(&self) -> &'static str {
        "overview"
    }

    fn enter(&mut self) {
        let (path, info, boxes, dirty, file_ok) = {
            let d = data::doc();
            (d.path, d.info, d.boxes, d.dirty, d.file_checksums_ok)
        };
        let ot = ui::utf16_to_text(&info.ot_name, 32);
        ui::clear(top());
        ui::frame_clear(top());
        ui::title(top(), &t!(crate::text::Prec(file_name(&path), 30)));
        // Trainer, then the save's numbers, then its checksum state.
        ui::panel(top(), 1, 2, 30, 12);
        let x = ui::INSET_TOP;
        let mut l = ui::Lines::new(2);
        ui::text(top(), x, l.line(), &ot);
        ui::text(top(), x, l.line(), &t!(MUTED, data::game_name(info.kind)));
        l.block();
        ui::text(
            top(),
            x,
            l.line(),
            &t!(
                MUTED,
                "ID",
                MUTED,
                "  ",
                Zero(info.tid.into(), 5),
                " / ",
                Zero(info.sid.into(), 5)
            ),
        );
        let played = t!(
            MUTED,
            "Played",
            MUTED,
            "  ",
            info.played_hours,
            ":",
            Zero(info.played_minutes.into(), 2),
            ":",
            Zero(info.played_seconds.into(), 2)
        );
        ui::text(top(), x, l.line(), &played);
        let party = t!(
            MUTED,
            "Party",
            MUTED,
            "  ",
            info.party_count,
            "     ",
            MUTED,
            "Boxes",
            MUTED,
            "  ",
            boxes
        );
        ui::text(top(), x, l.line(), &party);
        l.block();
        // Unsaved edits leave the checksums in memory stale; saving recomputes them, so show the
        // file's state and say they'll be updated.
        let mut sums = ui::Row::new(top(), l.grid_row(), x);
        sums.text(&t!(MUTED, "Checksums"));
        if dirty && file_ok {
            sums.badge(Badge::Safe, b"updated on save");
        } else if dirty {
            sums.badge(Badge::Invalid, b"BAD in file, fixed on save");
        } else if info.checksums_valid != 0 {
            sums.badge(Badge::Legal, b"OK");
        } else {
            sums.badge(Badge::Invalid, b"BAD");
        }
        sums.draw();
        if self.message.is_empty() && dirty {
            self.message = Text::from(b"Unsaved changes");
        }
        let data_ok = data::data_available();
        if !self.message.is_empty() || !data_ok {
            ui::panel(
                top(),
                1,
                17,
                30,
                if !self.message.is_empty() && !data_ok {
                    4
                } else {
                    2
                },
            );
            let mut m = ui::Lines::new(17);
            if !self.message.is_empty() {
                ui::text(
                    top(),
                    x,
                    m.line() + 2,
                    &t!(if dirty { "*" } else { "-" }, " ", &self.message),
                );
            }
            if !data_ok {
                ui::text(
                    top(),
                    x,
                    m.line() + 2,
                    &t!(MUTED, "Game data missing: showing IDs"),
                );
            }
        }
        ui::clear(bottom());
        ui::frame_clear(bottom());
        ui::title(bottom(), b"Save");
        if dirty {
            ui::title_right(bottom(), b"* unsaved");
        }
        self.count = 0;
        let add = |s: &mut Self, item| {
            s.items[s.count] = item;
            s.count += 1;
        };
        add(self, BOXES);
        add(self, BAG);
        add(self, GIFTS);
        if info.kind == BLACK2_WHITE2 {
            add(self, UNOVA);
        }
        add(self, SAVE);
        add(self, CLOSE);
        let count = self.count as i32;
        let selected = self.list.selected;
        let cells = (count * ui::GRID_PITCH + 8) / 8;
        self.list = spaced_list_at(ui::GRID_PITCH, 2, cells);
        self.list.count = count;
        self.list.selected = selected;
        self.list.clamp();
        ui::panel(bottom(), 0, self.list.first_row, 32, cells);
        for r in 0..count {
            ui::text(
                bottom(),
                LIST_X,
                self.list.text_y(r),
                ITEMS[self.items[r as usize]].as_bytes(),
            );
        }
        ui::hints(
            bottom(),
            if self.confirm_leave {
                b"A:Open|B:Discard edits"
            } else {
                b"A:Open|B:Files"
            },
        );
    }

    fn update(&mut self, input: &Input) -> bool {
        let (a, changed) = self.list.update(input);
        ui::bar(0, self.list.bar_y(), true, self.list.pitch);
        let item = if a >= 0 {
            Some(self.items[a as usize])
        } else {
            None
        };
        // Leaving drops unsaved edits (the file is read again when opened), so the first B or
        // Close only warns; anything else cancels the warning.
        if input.down & KEY_B != 0 || item == Some(CLOSE) {
            if !data::doc().dirty || self.confirm_leave {
                return false;
            }
            self.confirm_leave = true;
            self.message = Text::from(b"Not saved. B again to discard.");
            self.enter();
            return true;
        }
        if self.confirm_leave && (input.down != 0 || changed) {
            self.confirm_leave = false;
            self.message = Text::new();
            self.enter();
        }
        match item {
            Some(BOXES) => {
                self.message = Text::new();
                ui::push(box_view());
            }
            Some(BAG) => {
                self.message = Text::new();
                ui::push(bag());
            }
            Some(GIFTS) => {
                self.message = Text::new();
                ui::push(gifts());
            }
            Some(UNOVA) => {
                self.message = Text::new();
                ui::push(unova_link());
            }
            Some(SAVE) => {
                self.message = data::save_to_disk().cap(48);
                self.enter();
            }
            _ => {}
        }
        true
    }
}

/// What failed in a legality report: the checks' names, then " Moves" when a move did.
pub(crate) fn failed_checks(r: &ffi::Legality) -> Text {
    let mut why = Text::new();
    let count = usize::from(r.check_count);
    for (i, &check) in r.checks.iter().take(count).enumerate() {
        if why.len() >= 30 {
            break;
        }
        if i != 0 {
            why.push_bytes(b" ");
        }
        why.push_bytes(data::check_name(check).as_bytes());
    }
    if !r.move_ok.iter().all(|&ok| ok != 0) {
        why.push_bytes(b" Moves");
    }
    why.cap(48)
}

/// The names category for items in the open save's generation.
pub(crate) fn item_names() -> u32 {
    if data::doc().info.generation == 5 {
        NAME_ITEMS5
    } else {
        NAME_ITEMS4
    }
}

pub(crate) const NAME_SPECIES: u32 = 1;
pub(crate) const NAME_MOVES: u32 = 2;
pub(crate) const NAME_ABILITIES: u32 = 3;
pub(crate) const NAME_NATURES: u32 = 4;
pub(crate) const NAME_TYPES: u32 = 5;
pub(crate) const NAME_ITEMS4: u32 = 6;
pub(crate) const NAME_ITEMS5: u32 = 7;

#[allow(unused_imports)]
pub(crate) use ffi::Loc;
