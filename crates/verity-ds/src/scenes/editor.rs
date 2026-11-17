//! The Pokémon editor for an occupied slot (box_index, slot).

use alloc::boxed::Box;

use super::boxes::show_pokemon;
use super::{
    LIST_X, ListView, NAME_ITEMS4, NAME_ITEMS5, NAME_MOVES, NAME_SPECIES, NAME_TYPES,
    failed_checks, spaced_list,
};
use crate::ffi::{self, Edit, IvFix, Legality, Loc, PkmView, Zeroed};
use crate::text::{Pad, Text, Zero};
use crate::ui::{
    self, Badge, Input, KEY_A, KEY_B, KEY_DOWN, KEY_L, KEY_LEFT, KEY_R, KEY_RIGHT, KEY_SELECT,
    KEY_START, KEY_UP, KEY_X, KEY_Y, MUTED, RED, Scene, bottom, top,
};
use crate::{data, t};

const TABS: i32 = 5;
const TAB_NAMES: [&str; 5] = ["Main", "Stats", "Moves", "Met", "Misc"];
const STATS: [&str; 6] = ["HP", "Atk", "Def", "Spe", "SpA", "SpD"];

// Spacing prototype (top screen): content starts at column 2 (8 px inside the panel), lines that
// carry a badge or pill sit on the 8 px grid 16 px apart, plain text uses a 12 px pitch.
const X: i32 = 16; // text x on top-screen panels
const DETAIL: i32 = 2; // first content row of the details panel
const EDIT: i32 = 15; // first content row of the edit panel (rows 15-21)

struct Editor {
    box_index: u8,
    slot: u8,
    loaded: bool,
    gen4: bool,
    legal_mode: bool,
    orig: PkmView,
    e: Edit,
    tab: i32,
    row: i32,
    items: i32,
    moves: i32,
    picking: i32, // 0 none, 1 item, 2..5 move slot
    showing_fix: bool,
    fix: IvFix,
    typing: bool,
    ribbon_list: bool,
    name: [u16; 11],
    name_len: i32,
    key_row: i32,
    key_col: i32,
    key_page: i32,
    ribbons: ListView,
    pick: ListView,
    status: Text,
    report: Legality,
    issue: u8,
    preview_ok: bool,
}

pub fn editor(box_index: u8, slot: u8) -> Box<dyn Scene> {
    Box::new(Editor {
        box_index,
        slot,
        loaded: false,
        gen4: true,
        legal_mode: true,
        orig: PkmView::zeroed(),
        e: Edit::default(),
        tab: 0,
        row: 0,
        items: 0,
        moves: 0,
        picking: 0,
        showing_fix: false,
        fix: IvFix::default(),
        typing: false,
        ribbon_list: false,
        name: [0; 11],
        name_len: 0,
        key_row: 0,
        key_col: 0,
        key_page: 0,
        ribbons: ListView::default(),
        pick: ListView::default(),
        status: Text::new(),
        report: Legality::zeroed(),
        issue: 0,
        preview_ok: false,
    })
}

/// The first id without a name in a names category.
fn count(cat: u32) -> i32 {
    let mut w = [0u16; 4];
    let (mut lo, mut hi) = (0, 4096);
    while lo < hi {
        let mid = (lo + hi) / 2;
        if ffi::name(cat, mid as u32, &mut w) != 0 || mid == 0 {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    lo
}

// ---- nickname keyboard -------------------------------------------------------------------

// Keys are 3 cells wide from column KEY_LEFT: ten of them centred in the 32 columns.
const KEY_ROWS: i32 = 4;
const KEY_COLS: i32 = 10;
const KEY_TOP: i32 = 8;
const KEY_LEFT_COL: i32 = 1;
const DAKUTEN: u16 = 0x309B;
const HANDAKUTEN: u16 = 0x309C;

/// Ten UTF-16 keys from UTF-8 (all in the BMP), 0 for no key.
const fn keys(s: &str) -> [u16; 10] {
    let b = s.as_bytes();
    let mut out = [0u16; 10];
    let (mut i, mut n) = (0, 0);
    while i < b.len() && n < 10 {
        let c = b[i] as u32;
        let (cp, len) = if c < 0x80 {
            (c, 1)
        } else if c & 0xE0 == 0xC0 {
            ((c & 0x1F) << 6 | (b[i + 1] as u32 & 0x3F), 2)
        } else {
            (
                (c & 0x0F) << 12 | (b[i + 1] as u32 & 0x3F) << 6 | (b[i + 2] as u32 & 0x3F),
                3,
            )
        };
        out[n] = cp as u16;
        n += 1;
        i += len;
    }
    out
}

// Four rows of ten keys per page; 0 is no key. Japanese and Korean follow the games' keyboards:
// kana with ゛/゜ marks that change the last kana, fullwidth Latin for Japanese, and Hangul typed
// as jamo that compose into syllables (keyboard_type).
const LATIN: [[u16; 10]; 12] = [
    keys("ABCDEFGHIJ"),
    keys("KLMNOPQRST"),
    keys("UVWXYZ .,-"),
    keys("0123456789"),
    keys("abcdefghij"),
    keys("klmnopqrst"),
    keys("uvwxyz .,-"),
    keys("0123456789"),
    keys("!?/:'&()+="),
    keys("*%@#~;_<>^"),
    keys("\0\0\0\0\0\0\0\0.,"),
    keys("0123456789"),
];
const JAPANESE: [[u16; 10]; 24] = [
    keys("あいうえおかきくけこ"),
    keys("さしすせそたちつてと"),
    keys("なにぬねのはひふへほ"),
    keys("まみむめもやゆよ゛゜"),
    keys("らりるれろわをんー　"),
    keys("ぁぃぅぇぉっゃゅょ・"),
    keys("！？…「」、。\0゛゜"),
    keys("０１２３４５６７８９"),
    keys("アイウエオカキクケコ"),
    keys("サシスセソタチツテト"),
    keys("ナニヌネノハヒフヘホ"),
    keys("マミムメモヤユヨ゛゜"),
    keys("ラリルレロワヲンー　"),
    keys("ァィゥェォッャュョ・"),
    keys("！？…「」、。\0゛゜"),
    keys("０１２３４５６７８９"),
    keys("ＡＢＣＤＥＦＧＨＩＪ"),
    keys("ＫＬＭＮＯＰＱＲＳＴ"),
    keys("ＵＶＷＸＹＺ　．，－"),
    keys("０１２３４５６７８９"),
    keys("ａｂｃｄｅｆｇｈｉｊ"),
    keys("ｋｌｍｎｏｐｑｒｓｔ"),
    keys("ｕｖｗｘｙｚ　．，－"),
    keys("０１２３４５６７８９"),
];
const KOREAN: [[u16; 10]; 4] = [
    keys("ㄱㄲㄴㄷㄸㄹㅁㅂㅃㅅ"),
    keys("ㅆㅇㅈㅉㅊㅋㅌㅍㅎ "),
    keys("ㅏㅐㅑㅒㅓㅔㅕㅖㅗㅛ"),
    keys("ㅜㅠㅡㅣ.,!?-\0"),
];

fn page_row(layout: i32, page: i32, row: i32) -> [u16; 10] {
    match layout {
        1 => JAPANESE[(page * KEY_ROWS + row) as usize],
        2 if page == 0 => KOREAN[row as usize],
        2 => LATIN[((page - 1) * KEY_ROWS + row) as usize],
        _ => LATIN[(page * KEY_ROWS + row) as usize],
    }
}

// ---- ribbons -----------------------------------------------------------------------------

const RIBBONS: i32 = 80;

/// Byte and bit (in Edit::ribbons) of a ribbon, by list position.
fn ribbon_pos(i: i32) -> (usize, u8) {
    // Sinnoh set: 28 bits from byte 0; Hoenn set: 32 bits from byte 4; contest: 20 from byte 8.
    let off = if i < 28 {
        i
    } else if i < 60 {
        32 + i - 28
    } else {
        64 + i - 60
    };
    ((off / 8) as usize, (off % 8) as u8)
}

const RIBBON_NAMES: [&str; 80] = [
    "Champion Sinnoh",
    "Ability",
    "Great Ability",
    "Double Ability",
    "Multi Ability",
    "Pair Ability",
    "World Ability",
    "Alert",
    "Shock",
    "Downcast",
    "Careless",
    "Relax",
    "Snooze",
    "Smile",
    "Gorgeous",
    "Royal",
    "Gorgeous Royal",
    "Footprint",
    "Record",
    "Event",
    "Legend",
    "Champion World",
    "Birthday",
    "Special",
    "Souvenir",
    "Wishing",
    "Classic",
    "Premier",
    "Cool (G3)",
    "Cool Super",
    "Cool Hyper",
    "Cool Master",
    "Beauty (G3)",
    "Beauty Super",
    "Beauty Hyper",
    "Beauty Master",
    "Cute (G3)",
    "Cute Super",
    "Cute Hyper",
    "Cute Master",
    "Smart (G3)",
    "Smart Super",
    "Smart Hyper",
    "Smart Master",
    "Tough (G3)",
    "Tough Super",
    "Tough Hyper",
    "Tough Master",
    "Champion G3",
    "Winning",
    "Victory",
    "Artist",
    "Effort",
    "Battle Champion",
    "Regional Champion",
    "National Champion",
    "Country",
    "National",
    "Earth",
    "World",
    "Cool (G4)",
    "Cool Great",
    "Cool Ultra",
    "Cool Master",
    "Beauty (G4)",
    "Beauty Great",
    "Beauty Ultra",
    "Beauty Master",
    "Cute (G4)",
    "Cute Great",
    "Cute Ultra",
    "Cute Master",
    "Smart (G4)",
    "Smart Great",
    "Smart Ultra",
    "Smart Master",
    "Tough (G4)",
    "Tough Great",
    "Tough Ultra",
    "Tough Master",
];

fn ball_name(b: u8) -> Text {
    const APRICORN: [&str; 8] = [
        "Fast", "Level", "Lure", "Heavy", "Love", "Friend", "Moon", "Sport",
    ];
    match b {
        17..=24 => t!(APRICORN[usize::from(b - 17)], " Ball").cap(20),
        25 => Text::from(b"Dream Ball"),
        _ => ui::name(NAME_ITEMS4, b.into()),
    }
}

impl Scene for Editor {
    fn name(&self) -> &'static str {
        "editor"
    }

    fn enter(&mut self) {
        if !self.loaded {
            self.loaded = true;
            data::ensure_save_data();
            self.orig = data::slot(self.box_index, self.slot);
            let o = self.orig;
            self.e = Edit::default();
            self.e.held_item = o.held_item;
            self.e.moves = o.moves;
            self.e.ivs = o.ivs;
            self.e.evs = o.evs;
            self.e.level = if o.level != 0 { o.level } else { 1 };
            self.e.friendship = o.friendship;
            self.load_fields();
            self.gen4 = data::doc().info.generation == 4;
            self.items = count(if self.gen4 { NAME_ITEMS4 } else { NAME_ITEMS5 });
            self.moves = count(NAME_MOVES);
            if self.gen4 {
                self.moves = self.moves.min(468);
            }
        }
        ui::clear(top());
        ui::clear(bottom());
        self.draw();
        self.preview();
    }

    fn update(&mut self, input: &Input) -> bool {
        if self.picking != 0 {
            return self.update_picker(input);
        }
        if self.showing_fix {
            return self.update_fix(input);
        }
        if self.typing {
            return self.update_keyboard(input);
        }
        if self.ribbon_list {
            return self.update_ribbons(input);
        }
        if input.down & KEY_B != 0 {
            return false;
        }
        if self.tab == 1 && self.gen4 && input.down & KEY_X != 0 {
            self.find_closest();
            return true;
        }
        let rows = self.tab_rows();
        let (before_row, before_tab) = (self.row, self.tab);
        let mut changed = false;
        if input.down & KEY_L != 0 {
            self.tab = (self.tab + TABS - 1) % TABS;
        }
        if input.down & KEY_R != 0 {
            self.tab = (self.tab + 1) % TABS;
        }
        if input.down & KEY_SELECT != 0 {
            self.legal_mode = !self.legal_mode;
            changed = true;
        }
        if self.tab != before_tab {
            self.row = 0;
        }
        if input.rep & KEY_UP != 0 {
            self.row = (self.row + rows - 1) % rows;
        }
        if input.rep & KEY_DOWN != 0 {
            self.row = (self.row + 1) % rows;
        }
        let mut dir = if input.rep & KEY_RIGHT != 0 {
            1
        } else if input.rep & KEY_LEFT != 0 {
            -1
        } else {
            0
        };
        if input.held & KEY_Y != 0 {
            dir *= 10; // fast adjust
        }
        if input.tapped {
            let top0 = self.row_bar_y(0);
            let r = (input.tap_y - top0) / self.row_pitch();
            if input.tap_y < 16 {
                self.tab = input.tap_x * TABS / 256;
                self.row = 0;
            } else if r >= 0 && input.tap_y >= top0 && r < rows {
                if r == self.row {
                    dir = if input.tap_x < 128 { -1 } else { 1 };
                }
                self.row = r;
            } else if input.tap_y >= 176 {
                self.apply();
            }
        }
        if dir != 0 {
            changed |= self.adjust(dir);
        }
        if input.down & KEY_A != 0 && self.open_picker() {
            return true;
        }
        if input.down & KEY_START != 0 {
            self.apply();
        }
        if changed {
            self.preview();
        }
        if changed || self.row != before_row || self.tab != before_tab {
            self.draw();
        }
        ui::bar(0, self.row_bar_y(self.row), true, self.row_pitch());
        true
    }
}

impl Editor {
    fn loc(&self) -> Loc {
        Loc {
            box_index: self.box_index,
            slot: self.slot,
        }
    }

    fn generation(&self) -> u8 {
        if self.gen4 { 4 } else { 5 }
    }

    fn item_names(&self) -> u32 {
        if self.gen4 { NAME_ITEMS4 } else { NAME_ITEMS5 }
    }

    // Bottom rows from y 24: Stats has 12 rows on the 12 px pitch, the other tabs the grid pitch.
    // The highlight fills the row.
    fn row_pitch(&self) -> i32 {
        if self.tab == 1 {
            ui::PITCH
        } else {
            ui::GRID_PITCH
        }
    }

    fn row_text_y(&self, r: i32) -> i32 {
        24 + 4 + self.row_pitch() * r
    }

    fn row_bar_y(&self, r: i32) -> i32 {
        self.row_text_y(r) - (self.row_pitch() - 8) / 2
    }

    fn put_row(&self, r: i32, label: &[u8], value: &[u8]) {
        ui::text(bottom(), LIST_X, self.row_text_y(r), &t!(MUTED, label));
        ui::text(
            bottom(),
            104,
            self.row_text_y(r),
            &Text::from(value).cap(64),
        );
    }

    fn tab_rows(&self) -> i32 {
        [3, 12, 4, 6, 8][self.tab as usize]
    }

    fn load_fields(&mut self) {
        let o = &self.orig;
        self.e.ball = o.ball;
        self.e.met_location = o.met_location;
        self.e.met_level = o.met_level;
        self.e.met_date = o.met_date;
        self.e.contest = o.contest;
        self.e.ribbons = o.ribbons;
    }

    fn adjust(&mut self, dir: i32) -> bool {
        let e = &mut self.e;
        let row = self.row as usize;
        match self.tab {
            0 => {
                if row == 0 {
                    e.held_item = ((i32::from(e.held_item) + dir + self.items) % self.items) as u16;
                    e.mask |= 1;
                } else if row == 1 {
                    e.level = (i32::from(e.level) + dir).clamp(1, 100) as u8;
                    e.mask |= 16;
                } else {
                    e.friendship = (i32::from(e.friendship) + dir).clamp(0, 255) as u8;
                    e.mask |= 32;
                }
            }
            1 => {
                if row < 6 {
                    e.ivs[row] = (i32::from(e.ivs[row]) + dir).clamp(0, 31) as u8;
                    e.mask |= 4;
                    e.mask &= !64; // a hand-set spread no longer goes with the proposed PID
                } else {
                    e.evs[row - 6] = (i32::from(e.evs[row - 6]) + dir * 4).clamp(0, 255) as u8;
                    e.mask |= 8;
                }
            }
            2 => {
                e.moves[row] = ((i32::from(e.moves[row]) + dir + self.moves) % self.moves) as u16;
                e.mask |= 2;
            }
            3 => {
                if row == 0 {
                    let max = if self.gen4 { 24 } else { 25 };
                    e.ball = (i32::from(e.ball) + if dir > 0 { 1 } else { -1 }).clamp(1, max) as u8;
                    e.mask |= 128;
                    return true;
                }
                match row {
                    1 => e.met_location = (i32::from(e.met_location) + dir) as u16,
                    2 => e.met_level = (i32::from(e.met_level) + dir).clamp(0, 100) as u8,
                    3 => e.met_date[0] = (i32::from(e.met_date[0]) + dir).clamp(0, 99) as u8,
                    4 => e.met_date[1] = (i32::from(e.met_date[1]) + dir).clamp(1, 12) as u8,
                    _ => e.met_date[2] = (i32::from(e.met_date[2]) + dir).clamp(1, 31) as u8,
                }
                e.mask |= 256;
            }
            _ => {
                if !(1..=6).contains(&row) {
                    return false;
                }
                e.contest[row - 1] = (i32::from(e.contest[row - 1]) + dir).clamp(0, 255) as u8;
                e.mask |= 512;
            }
        }
        true
    }

    // ---- nickname keyboard -------------------------------------------------------------------

    /// 0 Latin, 1 Japanese, 2 Korean (by the Pokémon's language).
    fn layout(&self) -> i32 {
        match self.orig.language {
            1 => 1,
            8 => 2,
            _ => 0,
        }
    }

    fn page_count(&self) -> i32 {
        match self.layout() {
            1 => 6,
            2 => 4,
            _ => 3,
        }
    }

    fn max_name(&self) -> usize {
        if self.layout() != 0 { 5 } else { 10 }
    }

    fn key_ok(&self, c: u16) -> bool {
        if c == 0 {
            return false;
        }
        if c == u16::from(b' ') || c == DAKUTEN || c == HANDAKUTEN {
            return true;
        }
        ffi::char_encodable(data::doc().info.generation, c)
    }

    fn open_keyboard(&mut self) -> bool {
        if self.layout() != 0 && !ui::has_rom_font() {
            self.status = Text::from(b"Keyboard: needs a ROM font");
            return false;
        }
        if self.key_page >= self.page_count() {
            self.key_page = 0;
        }
        self.typing = true;
        ui::bar(0, 0, false, 8); // the editor's row highlight
        if self.e.mask & 2048 != 0 {
            self.name = self.e.nickname;
        } else {
            self.name = [0; 11];
            self.name[..10].copy_from_slice(&self.orig.nickname[..10]);
        }
        self.name_len = 0;
        while self.name_len < 10 && self.name[self.name_len as usize] != 0 {
            self.name_len += 1;
        }
        ui::clear(bottom());
        self.draw_keyboard();
        true
    }

    fn press_key(&mut self, row: i32, col: i32) {
        let c = page_row(self.layout(), self.key_page, row)[col as usize];
        if !self.key_ok(c) {
            return;
        }
        let mut len = self.name_len as usize;
        let (generation, max) = (data::doc().info.generation, self.max_name());
        ffi::keyboard_type(&mut self.name[..10], &mut len, max, c, generation);
        self.name_len = len as i32;
        self.name[len] = 0;
    }

    fn update_keyboard(&mut self, input: &Input) -> bool {
        let mut redraw = false;
        if input.rep & KEY_UP != 0 {
            self.key_row = (self.key_row + KEY_ROWS - 1) % KEY_ROWS;
            redraw = true;
        }
        if input.rep & KEY_DOWN != 0 {
            self.key_row = (self.key_row + 1) % KEY_ROWS;
            redraw = true;
        }
        if input.rep & KEY_LEFT != 0 {
            self.key_col = (self.key_col + KEY_COLS - 1) % KEY_COLS;
            redraw = true;
        }
        if input.rep & KEY_RIGHT != 0 {
            self.key_col = (self.key_col + 1) % KEY_COLS;
            redraw = true;
        }
        if input.down & KEY_A != 0 {
            self.press_key(self.key_row, self.key_col);
            redraw = true;
        }
        if input.tapped {
            let r = (input.tap_y / 8 - KEY_TOP) / 2;
            let c = (input.tap_x - KEY_LEFT_COL * 8) / 24;
            if input.tap_y / 8 >= KEY_TOP && r < KEY_ROWS && (0..KEY_COLS).contains(&c) {
                self.key_row = r;
                self.key_col = c;
                self.press_key(r, c);
                redraw = true;
            }
        }
        if input.down & KEY_B != 0 {
            if self.name_len > 0 {
                self.name_len -= 1;
                self.name[self.name_len as usize] = 0;
                redraw = true;
            } else {
                self.typing = false;
                ui::clear(bottom());
                self.draw();
                return true;
            }
        }
        if input.down & KEY_Y != 0 {
            self.key_page = (self.key_page + 1) % self.page_count();
            redraw = true;
        }
        if input.down & KEY_X != 0 {
            // Back to the species name, which also clears the nicknamed flag.
            self.e.mask = (self.e.mask & !2048) | 4096;
            self.typing = false;
            ui::clear(bottom());
            self.preview();
            self.draw();
            return true;
        }
        if input.down & KEY_START != 0 {
            if self.name_len == 0 {
                ui::clear_px(bottom(), 28, 40);
                ui::text(bottom(), ui::INSET_TOP, 28, b"A name can't be empty");
                return true;
            }
            self.e.nickname = [0; 11];
            let n = self.name_len as usize;
            self.e.nickname[..n].copy_from_slice(&self.name[..n]);
            self.e.mask = (self.e.mask & !4096) | 2048;
            self.typing = false;
            ui::clear(bottom());
            self.preview();
            self.draw();
            return true;
        }
        if redraw {
            self.draw_keyboard();
        }
        true
    }

    fn draw_keyboard(&mut self) {
        let text = ui::utf16_to_text(&self.name, 48);
        ui::frame_clear(bottom());
        ui::title(bottom(), b"Nickname");
        ui::title_right(bottom(), &t!(self.name_len, "/", self.max_name()).cap(12));
        ui::panel(bottom(), 1, 2, 30, 3);
        // Text is drawn over what's there: clear the field, or erasing leaves the old characters.
        ui::clear_px(bottom(), 16, 40);
        let mut l = ui::Lines::new(2);
        ui::text(bottom(), ui::INSET_TOP, l.line(), &t!(&text, "_"));
        let help = match self.layout() {
            1 => "Japanese (Y: next page)",
            2 => "Korean (Y: next page)",
            _ => "Y: next page",
        };
        ui::text(bottom(), ui::INSET_TOP, l.line(), &t!(MUTED, help));
        ui::panel(bottom(), 0, KEY_TOP, 32, KEY_ROWS * 2 - 1); // keys span the full width
        for r in 0..KEY_ROWS {
            let mut line = [u16::from(b' '); 33];
            line[32] = 0;
            let keys = page_row(self.layout(), self.key_page, r);
            for c in 0..KEY_COLS {
                let x = KEY_LEFT_COL + c * 3;
                let mut k = if self.key_ok(keys[c as usize]) {
                    keys[c as usize]
                } else {
                    u16::from(b' ')
                };
                // Gen 4 fonts have no standalone voicing marks.
                if (k == DAKUTEN || k == HANDAKUTEN) && !ui::has_glyph(k) {
                    k = if k == DAKUTEN {
                        u16::from(b'"')
                    } else {
                        u16::from(b'o')
                    };
                }
                line[(x + 1) as usize] = k;
                if r == self.key_row && c == self.key_col {
                    ui::key_highlight(bottom(), x, KEY_TOP + r * 2);
                }
            }
            let text_line = ui::utf16_to_text(&line, 128);
            ui::at(bottom(), 0, KEY_TOP + r * 2, &text_line);
        }
        ui::hints(bottom(), b"A:Key|B:Erase|Y:Page|START:Done");
    }

    // ---- ribbons -----------------------------------------------------------------------------

    fn ribbon_on(&self, i: i32) -> bool {
        let (byte, bit) = ribbon_pos(i);
        self.e.ribbons[byte] & (1 << bit) != 0
    }

    fn ribbon_count(&self) -> i32 {
        (0..RIBBONS).filter(|&i| self.ribbon_on(i)).count() as i32
    }

    fn update_ribbons(&mut self, input: &Input) -> bool {
        let (a, mut changed) = self.ribbons.update(input);
        ui::bar(0, self.ribbons.bar_y(), true, self.ribbons.pitch);
        if input.down & KEY_B != 0 {
            self.ribbon_list = false;
            ui::clear(bottom());
            self.preview();
            self.draw();
            return true;
        }
        if a >= 0 {
            let (byte, bit) = ribbon_pos(a);
            self.e.ribbons[byte] ^= 1 << bit;
            self.e.mask |= 1024;
            changed = true;
        }
        if changed {
            self.draw_ribbons();
        }
        true
    }

    /// The highlighted ribbon in the details panel (rows row..row+5): its image from the ROM, name
    /// and whether this Pokémon has it.
    fn ribbon_details(&self, row: i32, i: i32) {
        ui::panel(top(), 1, row, 30, 6);
        let y = row * 8;
        ui::clear_px(top(), y, y + 48);
        let (mut tiles, mut colors) = ([0u8; 512], [0u16; 16]);
        let icon = data::ribbon_icon(i, &mut tiles, &mut colors);
        ui::item_icon(true, 14, y + 8, icon.then_some((&tiles[..], &colors[..])));
        let x = if icon { 56 } else { X };
        ui::text(top(), x, y + 6, RIBBON_NAMES[i as usize].as_bytes());
        let on = self.ribbon_on(i);
        let (b, what): (Badge, &[u8]) = if on {
            (Badge::Legal, b"Has it")
        } else {
            (Badge::Fishy, b"Not set")
        };
        ui::badge(top(), x / 8, row + 3, b, what);
        if !icon {
            ui::text(
                top(),
                X,
                y + 38,
                &t!(MUTED, "A ROM on the SD card adds images."),
            );
        }
    }

    fn draw_ribbons(&mut self) {
        ui::top_art_hide();
        ui::frame_clear_rows(top(), 1, EDIT - 1); // the taller key-hint panel
        self.ribbon_details(DETAIL, self.ribbons.selected);
        ui::frame_clear(bottom());
        ui::title(bottom(), b"Ribbons");
        ui::title_right(bottom(), &t!(self.ribbon_count(), " set").cap(16));
        ui::panel(bottom(), 0, 2, 32, 20);
        ui::hints(bottom(), b"A:Toggle|L/R:Page|B:Done");
        ui::clear_px(bottom(), 16, 176);
        for r in 0..self.ribbons.rows {
            let i = self.ribbons.scroll + r;
            if i >= RIBBONS {
                break;
            }
            let y = self.ribbons.text_y(i);
            let on = self.ribbon_on(i);
            ui::text(
                bottom(),
                LIST_X,
                y,
                &if on { t!("[x]") } else { t!(MUTED, "[ ]") },
            );
            ui::text(
                bottom(),
                LIST_X + 24,
                y,
                &t!(if on { "" } else { MUTED }, RIBBON_NAMES[i as usize]),
            );
        }
    }

    // ---- closest legal IVs -------------------------------------------------------------------

    fn find_closest(&mut self) {
        let y = (EDIT + 2) * 8 + 14;
        ui::clear_px(top(), y, y + 10);
        ui::text(top(), X, y, b"Searching for legal IVs...");
        ui::present();
        let (code, fix) = ffi::closest_ivs(&mut data::doc().buf, self.loc(), &self.e.ivs);
        ui::clear_px(top(), y, y + 10);
        if code != 0 {
            self.status = if code == 12 {
                Text::from(b"No legal IVs found nearby")
            } else {
                Text::from(ffi::gen_status_text(code)).cap(48)
            };
            self.draw();
            return;
        }
        self.fix = fix;
        self.showing_fix = true;
        ui::bar(0, 0, false, 8);
        self.draw_fix();
    }

    fn draw_fix(&self) {
        ui::clear(bottom());
        ui::frame_clear(bottom());
        ui::title(bottom(), b"Closest legal IVs");
        let d = self.fix.distance;
        ui::title_right(
            bottom(),
            &t!(d, " point", if d == 1 { "" } else { "s" }, " away").cap(24),
        );
        // Columns: stat, yours, legal (right-aligned numbers), and a note on changed rows.
        ui::panel(bottom(), 1, 2, 30, 11);
        let mut l = ui::Lines::new(2);
        let (x, yours, legal) = (ui::INSET_TOP, 96, 150);
        let y = l.line();
        ui::text_right(bottom(), yours, y, &t!(MUTED, "Yours"));
        ui::text_right(bottom(), legal, y, &t!(MUTED, "Legal"));
        for (i, st) in STATS.iter().enumerate() {
            let y = l.line();
            let same = self.e.ivs[i] == self.fix.ivs[i];
            let m = if same { MUTED } else { "" };
            ui::text(bottom(), x, y, &t!(MUTED, *st));
            ui::text_right(bottom(), yours, y, &t!(m, self.e.ivs[i]));
            ui::text_right(bottom(), legal, y, &t!(m, self.fix.ivs[i]));
            if !same {
                ui::text(bottom(), legal + 16, y, &t!(MUTED, "changes"));
            }
        }
        ui::panel(bottom(), 1, 16, 30, 3);
        let mut n = ui::Lines::new(16);
        ui::text(bottom(), x, n.line(), b"Its PID changes too; nature,");
        ui::text(bottom(), x, n.line(), b"gender and ability stay.");
        ui::hints(bottom(), b"A:Use these|B:Keep mine");
    }

    fn update_fix(&mut self, input: &Input) -> bool {
        if input.down & (KEY_A | KEY_B) != 0 {
            if input.down & KEY_A != 0 {
                self.e.ivs = self.fix.ivs;
                self.e.pid = self.fix.pid;
                self.e.mask |= 4 | 64;
            }
            self.showing_fix = false;
            ui::clear(bottom());
            self.preview();
            self.draw();
        }
        true
    }

    // ---- pickers -----------------------------------------------------------------------------

    fn open_picker(&mut self) -> bool {
        if self.tab == 4 && self.row == 0 {
            if !self.open_keyboard() {
                self.draw();
            }
            return true;
        }
        if self.tab == 4 && self.row == 7 {
            self.ribbon_list = true;
            self.ribbons = spaced_list();
            self.ribbons.count = RIBBONS;
            ui::clear(bottom());
            self.draw_ribbons();
            return true;
        }
        if self.tab == 0 && self.row == 0 {
            self.picking = 1;
            self.pick = spaced_list();
            self.pick.count = self.items;
            self.pick.selected = self.e.held_item.into();
        } else if self.tab == 2 {
            self.picking = 2 + self.row;
            self.pick = spaced_list();
            self.pick.count = self.moves;
            self.pick.selected = self.e.moves[self.row as usize].into();
        } else {
            return false;
        }
        self.pick.clamp();
        self.draw_picker();
        true
    }

    fn update_picker(&mut self, input: &Input) -> bool {
        let (a, changed) = self.pick.update(input);
        ui::bar(0, self.pick.bar_y(), true, 12);
        if input.down & KEY_B != 0 {
            self.picking = 0;
            ui::clear(bottom());
            self.draw();
            return true;
        }
        if a >= 0 {
            if self.picking == 1 {
                self.e.held_item = a as u16;
                self.e.mask |= 1;
            } else {
                self.e.moves[(self.picking - 2) as usize] = a as u16;
                self.e.mask |= 2;
            }
            self.picking = 0;
            ui::clear(bottom());
            self.preview();
            self.draw();
            return true;
        }
        if changed {
            self.draw_picker();
        }
        true
    }

    /// A move in the details panel (rows row..row+9): name, PP and type pill; category pill,
    /// power and accuracy 16 px below; the description in the label shade on a 12 px pitch.
    fn move_details(&self, row: i32, mv: u16) {
        ui::panel(top(), 1, row, 30, 10);
        let y = row * 8;
        ui::clear_px(top(), y, y + 80);
        if mv == 0 {
            ui::text(top(), X, y, &t!(MUTED, "No move in this slot."));
            return;
        }
        let generation = self.generation();
        // The name, then PP and the type pill (or PP alone, beside the type icon) at the right.
        let mut head = ui::Row::new(top(), row, X);
        head.text(&ui::name(NAME_MOVES, mv.into()));
        let Some(m) = data::move_info(generation, mv) else {
            head.draw();
            ui::text(
                top(),
                X,
                y + 16,
                &t!(MUTED, "Not in Gen ", generation, " games."),
            );
            return;
        };
        // The game's type and category icons when the sprite cache has them, else pills.
        let (mut tt, mut ct) = ([0u8; 256], [0u8; 256]);
        let (mut tc, mut cc) = ([0u16; 16], [0u16; 16]);
        let icons = m.type_ != 0xFF
            && m.from_rom != 0
            && data::type_icon(generation, 0, m.type_.into(), &mut tt, &mut tc)
            && data::type_icon(generation, 1, (m.category % 3).into(), &mut ct, &mut cc);
        if m.type_ != 0xFF {
            head.push_right().text(&t!(m.pp, " ", MUTED, "PP"));
            if !icons {
                head.pill(
                    ui::type_color(m.type_.into()),
                    &ui::name(NAME_TYPES, m.type_.into()),
                );
            }
        }
        head.draw();
        if m.from_rom == 0 {
            ui::text(
                top(),
                X,
                y + 20,
                &t!(MUTED, "A Gen ", generation, " ROM on the SD card adds"),
            );
            ui::text(
                top(),
                X,
                y + 32,
                &t!(MUTED, "power, accuracy and description."),
            );
            return;
        }
        const CAT: [&str; 3] = ["Physical", "Special", "Status"];
        const CAT_COLOR: [u16; 3] = [ui_rgb(27, 9, 5), ui_rgb(7, 12, 26), ui_rgb(11, 11, 13)];
        let power = match m.power {
            0 => Text::from(b"-"),
            1 => Text::from(b"varies"),
            p => t!(p),
        };
        let acc = if m.accuracy == 0 || m.accuracy == 101 {
            Text::from(b"sure hit")
        } else {
            t!(m.accuracy, "%")
        };
        let col = if icons {
            // Centred on the stats line (y + 16).
            ui::small_icon(0, X, y + 12, Some((&tt[..], &tc[..])));
            ui::small_icon(1, X + 36, y + 12, Some((&ct[..], &cc[..])));
            (X + 36 + 32) / 8
        } else {
            let c = usize::from(m.category % 3);
            ui::pill(top(), 2, row + 2, CAT_COLOR[c], CAT[c].as_bytes())
        };
        let x = col * 8 + 8;
        ui::text(
            top(),
            x,
            y + 16,
            &t!(MUTED, "Power", MUTED, " ", &power).cap(24),
        );
        // Accuracy at the panel's right edge ("sure hit" is wide).
        ui::text_right(top(), 244, y + 16, &t!(MUTED, "Acc", MUTED, " ", &acc));
        if let Some(desc) = data::move_description(generation, mv, 192) {
            // 12 px below the stats row (1.5x the line pitch); four lines end at the panel's edge.
            ui::wrap_px(top(), X, y + 36, 12, 30 * 8 - X, 4, &desc, true);
        }
    }

    /// The held item's icon from a ROM of the save's generation, if there is one; false otherwise
    /// (the sprite is hidden).
    fn show_item_icon(&self, on_top: bool, x: i32, y: i32, item: u16) -> bool {
        let (mut tiles, mut colors) = ([0u8; 512], [0u16; 16]);
        let ok = item != 0 && data::item_icon(self.generation(), item, &mut tiles, &mut colors);
        ui::item_icon(on_top, x, y, ok.then_some((&tiles[..], &colors[..])));
        ok
    }

    /// An item in the details panel (rows row..row+5): its icon, name and number.
    fn item_details(&self, row: i32, item: u16) {
        ui::panel(top(), 1, row, 30, 6);
        let y = row * 8;
        ui::clear_px(top(), y, y + 48);
        // The art is the top-left 24x24 of the frame: centred in the panel's 48 px.
        let icon = self.show_item_icon(true, 16, y + 12, item);
        if item == 0 {
            ui::text(top(), X, y + 4, &t!(MUTED, "No held item."));
            return;
        }
        let (x, ty) = if icon { (52, y + 10) } else { (X, y + 4) };
        ui::text(top(), x, ty, &ui::name(self.item_names(), item.into()));
        ui::text(top(), x, ty + 14, &t!(MUTED, "Item ", item));
        if !icon {
            ui::text(
                top(),
                X,
                ty + 28,
                &t!(MUTED, "A Gen ", self.generation(), " ROM on SD adds icons."),
            );
        }
    }

    fn draw_picker(&mut self) {
        ui::top_art_hide();
        ui::frame_clear_rows(top(), 1, EDIT - 1); // the taller key-hint panel
        let cat = if self.picking == 1 {
            self.item_names()
        } else {
            NAME_MOVES
        };
        ui::item_icon(false, 0, 0, None);
        ui::frame_clear(bottom());
        if self.picking == 1 {
            ui::title(bottom(), b"Held item");
        } else {
            ui::title(bottom(), &t!("Move ", self.picking - 1));
        }
        ui::panel(bottom(), 0, 2, 32, 20);
        ui::hints(bottom(), b"A:Choose|L/R:Page|B:Cancel");
        ui::clear_px(bottom(), 16, 176);
        for r in 0..self.pick.rows {
            let i = self.pick.scroll + r;
            if i >= self.pick.count {
                break;
            }
            let y = self.pick.text_y(i);
            let info = if self.picking >= 2 && i != 0 {
                data::move_info(self.generation(), i as u16)
            } else {
                None
            };
            match info.filter(|m| m.type_ != 0xFF) {
                Some(m) => {
                    ui::text(bottom(), 12, y, &ui::name(cat, i as u32));
                    ui::text(
                        bottom(),
                        136,
                        y,
                        &t!(MUTED, &ui::name(NAME_TYPES, m.type_.into())),
                    );
                    let pp = t!(m.pp, " ", MUTED, "PP").cap(12);
                    ui::text(bottom(), 244 - ui::text_width(&pp), y, &pp);
                }
                None => {
                    ui::text(bottom(), 12, y, &t!(MUTED, i));
                    let name = if i != 0 {
                        ui::name(cat, i as u32)
                    } else {
                        Text::from(b"(none)")
                    };
                    ui::text(bottom(), 40, y, &name);
                }
            }
        }
        if self.picking >= 2 {
            self.move_details(DETAIL, self.pick.selected as u16);
        } else {
            self.item_details(DETAIL, self.pick.selected as u16);
        }
    }

    fn apply(&mut self) {
        let mode = if self.legal_mode { 0 } else { 1 };
        let code = {
            let mut d = data::doc();
            let code = ffi::edit_apply(&mut d.buf, self.loc(), &self.e, mode);
            if code == 0 {
                d.dirty = true;
            }
            code
        };
        if code == 0 {
            self.orig = data::slot(self.box_index, self.slot);
            self.e.mask = 0;
            self.load_fields();
            self.status = Text::from(b"applied (save from the menu)");
        } else {
            self.status = t!("refused: ", ffi::issue_text(code)).cap(48);
        }
        self.draw();
        self.preview();
    }

    /// Recomputes the verdict for the edit as it stands, then shows it.
    fn preview(&mut self) {
        let (r, report, issue) = ffi::edit_preview(&mut data::doc().buf, self.loc(), &self.e);
        self.preview_ok = r == ffi::Err::Ok;
        self.report = report;
        self.issue = issue;
        self.show_preview();
    }

    /// The edit panel: mode, verdict if applied now, status, and what is wrong.
    fn show_preview(&self) {
        // Seven rows: the badges, then status and what is wrong on two 12 px lines.
        ui::panel(top(), 1, EDIT, 30, 7);
        ui::clear_px(top(), EDIT * 8, (EDIT + 7) * 8);
        let (b, mode): (Badge, &[u8]) = if self.legal_mode {
            (Badge::Legal, b"Legal mode")
        } else {
            (Badge::Safe, b"Game-safe mode")
        };
        let c = ui::badge(top(), 2, EDIT, b, mode);
        ui::text(top(), c * 8 + 8, EDIT * 8, &t!(MUTED, "SEL switches"));
        let y = (EDIT + 2) * 8;
        let status: Text = if !self.status.is_empty() {
            self.status
        } else if self.e.mask != 0 {
            Text::from(b"* Edited, not applied yet.")
        } else {
            Text::from(b"No changes yet.")
        };
        if !self.preview_ok {
            ui::text(top(), X, y, b"Preview unavailable");
            ui::text(top(), X, y + 14, &t!(MUTED, &status));
            return;
        }
        let r = &self.report;
        // The verdict (no label, as on the summary), then Game-safe at the panel's right edge.
        let mut verdict = ui::Row::new(top(), EDIT + 2, X);
        match r.status {
            1 => verdict.badge(Badge::Legal, b"LEGAL"),
            2 => verdict.badge(Badge::Invalid, b"ILLEGAL"),
            _ => verdict.badge(Badge::Fishy, b"UNCHECKED"),
        };
        verdict.push_right().text(&t!(MUTED, "Game-safe"));
        if self.issue != 0 {
            verdict.badge(Badge::Invalid, b"NO");
        } else {
            verdict.badge(Badge::Legal, b"OK");
        }
        verdict.draw();
        ui::text(top(), X, y + 14, &t!(MUTED, &status));
        let why = if self.issue != 0 {
            Text::from(ffi::issue_text(self.issue)).cap(48)
        } else if r.status == 2 {
            failed_checks(r)
        } else {
            Text::new()
        };
        if !why.is_empty() {
            ui::text(top(), X, y + 26, &why);
        }
    }

    fn draw(&mut self) {
        let nick = ui::utf16_to_text(&self.orig.nickname, 32);
        ui::clear(top());
        ui::frame_clear(top());
        ui::title(top(), &t!("Edit  ", &nick));
        ui::title_right(top(), &ui::name(NAME_SPECIES, self.orig.species.into()));
        ui::item_icon(true, 0, 0, None);
        ui::top_art_hide();
        if self.tab == 2 {
            self.move_details(
                DETAIL,
                self.e.moves[if self.row < 4 { self.row as usize } else { 0 }],
            );
        } else {
            // What the keys do, beside the Pokémon (as saved: edits show after Apply).
            ui::panel(top(), 1, DETAIL, 30, 11);
            show_pokemon(&self.orig, 148, 12, false);
            let y = DETAIL * 8 + 4;
            ui::text(top(), X, y, &t!("Left/Right ", MUTED, "changes it"));
            ui::text(top(), X, y + 12, &t!("Y+Left/Right ", MUTED, "by tens"));
            let mut next = y + 36;
            if self.tab == 1 {
                let total: i32 = self.e.evs.iter().map(|&v| i32::from(v)).sum();
                ui::text(
                    top(),
                    X,
                    y + 24,
                    &t!(MUTED, "EV total ", MUTED, total, " / 510"),
                );
                if total > 510 {
                    ui::text(top(), X, next, &t!(RED, "Over the limit", RED));
                    next += 12;
                }
            } else {
                ui::text(top(), X, y + 24, &t!("A ", MUTED, "picks from a list"));
            }
            let help = if self.tab == 1 && self.gen4 {
                t!("X ", MUTED, "closest legal IVs")
            } else {
                t!("Tap twice ", MUTED, "steps a row")
            };
            ui::text(top(), X, next, &help);
        }
        self.show_preview();

        ui::clear(bottom());
        ui::frame_clear(bottom());
        ui::tabs(bottom(), &TAB_NAMES, self.tab);
        let rows = self.tab_rows();
        ui::panel(
            bottom(),
            0,
            3,
            32,
            (rows * self.row_pitch() + 8) / 8 + if self.tab == 3 { 2 } else { 0 },
        );
        let hints: &[u8] = if self.tab == 1 && self.gen4 {
            b"L/R:Tab|X:Closest IVs|START:Apply"
        } else {
            b"L/R:Tab|A:List|START:Apply"
        };
        ui::hints(bottom(), hints);
        let items = self.item_names();
        // Beside the Main tab's three rows, at the panel's right edge.
        self.show_item_icon(
            false,
            220,
            34,
            if self.tab == 0 { self.e.held_item } else { 0 },
        );
        let e = self.e;
        match self.tab {
            0 => {
                let item = if e.held_item != 0 {
                    ui::name(items, e.held_item.into())
                } else {
                    Text::from(b"-")
                };
                self.put_row(0, b"Item", &t!("< ", &item, " >"));
                self.put_row(1, b"Level", &t!("< ", e.level, " >"));
                self.put_row(2, b"Friendship", &t!("< ", e.friendship, " >"));
            }
            1 => {
                for (i, st) in STATS.iter().enumerate() {
                    let max = if e.ivs[i] == 31 { "max" } else { "" };
                    let label = t!("IV  ", *st).cap(12);
                    self.put_row(
                        i as i32,
                        &label,
                        &t!("< ", Pad(e.ivs[i].into(), 2), " >  ", MUTED, max),
                    );
                }
                for (i, st) in STATS.iter().enumerate() {
                    let label = t!("EV  ", *st).cap(12);
                    self.put_row(
                        6 + i as i32,
                        &label,
                        &t!("< ", Pad(e.evs[i].into(), 3), " >"),
                    );
                }
            }
            2 => {
                for (m, &mv) in e.moves.iter().enumerate() {
                    let label = t!("Move ", m + 1).cap(8);
                    let name = if mv != 0 {
                        ui::name(NAME_MOVES, mv.into())
                    } else {
                        Text::from(b"-")
                    };
                    self.put_row(m as i32, &label, &t!("< ", &name, " >"));
                }
            }
            3 => {
                self.put_row(0, b"Ball", &t!("< ", &ball_name(e.ball), " >"));
                let place = ui::location(self.generation(), e.met_location);
                self.put_row(1, b"Location", &t!("< ", &place, " >"));
                self.put_row(2, b"Met level", &t!("< ", e.met_level, " >"));
                self.put_row(3, b"Year", &t!("< ", 2000 + u32::from(e.met_date[0]), " >"));
                self.put_row(4, b"Month", &t!("< ", e.met_date[1], " >"));
                self.put_row(5, b"Day", &t!("< ", e.met_date[2], " >"));
                let ids = t!(
                    MUTED,
                    "OT ID  ",
                    Zero(self.orig.tid.into(), 5),
                    " / ",
                    Zero(self.orig.sid.into(), 5)
                );
                ui::text(bottom(), LIST_X, self.row_text_y(6) + 4, &ids);
            }
            _ => {
                const CS: [&str; 6] = ["Cool", "Beauty", "Cute", "Smart", "Tough", "Sheen"];
                let nick2 = if e.mask & 2048 != 0 {
                    ui::utf16_to_text(&e.nickname, 32)
                } else if e.mask & 4096 != 0 {
                    ui::name(NAME_SPECIES, self.orig.species.into()).cap(32)
                } else {
                    ui::utf16_to_text(&self.orig.nickname, 32)
                };
                self.put_row(0, b"Nickname", &t!(&nick2, "   ", MUTED, "A to type"));
                for (i, cs) in CS.iter().enumerate() {
                    self.put_row(
                        1 + i as i32,
                        cs.as_bytes(),
                        &t!("< ", Pad(e.contest[i].into(), 3), " >"),
                    );
                }
                self.put_row(
                    7,
                    b"Ribbons",
                    &t!(self.ribbon_count(), " set   ", MUTED, "A to list"),
                );
            }
        }
    }
}

const fn ui_rgb(r: u16, g: u16, b: u16) -> u16 {
    blocksds::video::rgb15(r, g, b)
}
