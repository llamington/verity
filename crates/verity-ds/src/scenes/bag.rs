//! The bag: every pouch, item counts, adding and tossing.

use alloc::boxed::Box;
use alloc::vec;
use alloc::vec::Vec;

use super::{LIST_X, ListView, item_names, spaced_list};
use crate::ffi::{self, BagItem, PouchView, Zeroed};
use crate::text::Text;
use crate::ui::{
    self, Input, KEY_A, KEY_B, KEY_DOWN, KEY_LEFT, KEY_RIGHT, KEY_UP, KEY_X, MUTED, Scene, bottom,
    top,
};
use crate::{data, t};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    List,
    Count,
    Add,
}

const MAX_ITEMS: usize = 300;
/// bag_status_text code for an event item (checked in verity-ffi's bag tests).
const EVENT_ITEM: u8 = 4;
const POUCH_NAMES: [&str; 8] = [
    "Items",
    "Key Items",
    "TMs & HMs",
    "Mail",
    "Medicine",
    "Berries",
    "Poke Balls",
    "Battle Items",
];

struct Bag {
    mode: Mode,
    pv: [PouchView; 8],
    pouches: i32,
    pouch: i32,
    items: Vec<BagItem>,
    count: i32,
    addable: Vec<u16>,
    addable_count: i32,
    count_edit: i32,
    list: ListView,
    add_list: ListView,
    message: Text,
}

pub fn bag() -> Box<dyn Scene> {
    Box::new(Bag {
        mode: Mode::List,
        pv: [PouchView::zeroed(); 8],
        pouches: 0,
        pouch: 0,
        items: vec![BagItem::zeroed(); MAX_ITEMS],
        count: 0,
        addable: vec![0; MAX_ITEMS],
        addable_count: 0,
        count_edit: 1,
        list: spaced_list(),
        add_list: spaced_list(),
        message: Text::new(),
    })
}

impl Scene for Bag {
    fn name(&self) -> &'static str {
        "bag"
    }

    fn enter(&mut self) {
        if !data::data_available() {
            ui::clear(top());
            ui::frame_clear(top());
            ui::title(top(), b"Bag");
            ui::panel(top(), 1, 2, 30, 3);
            let mut l = ui::Lines::new(2);
            ui::text(top(), ui::INSET_TOP, l.line(), b"Game data is missing.");
            ui::text(top(), ui::INSET_TOP, l.line(), &t!(MUTED, "B goes back."));
            ui::clear(bottom());
            ui::title(bottom(), b"Bag");
            ui::hints(bottom(), b"B:Back");
            return;
        }
        self.load();
        self.draw();
    }

    fn update(&mut self, input: &Input) -> bool {
        if !data::data_available() {
            return input.down & KEY_B == 0;
        }
        match self.mode {
            Mode::Count => return self.update_count(input),
            Mode::Add => return self.update_add(input),
            Mode::List => {}
        }
        if input.down & KEY_B != 0 {
            ui::item_icon(true, 0, 0, None);
            return false;
        }
        if input.rep & (KEY_LEFT | KEY_RIGHT) != 0 {
            let step = if input.rep & KEY_LEFT != 0 {
                self.pouches - 1
            } else {
                1
            };
            self.pouch = (self.pouch + step) % self.pouches;
            self.list.selected = 0;
            self.list.scroll = 0;
            self.message = Text::new();
            self.load();
            self.draw();
            return true;
        }
        let (a, changed) = self.list.update(input);
        if changed {
            self.message = Text::new();
            self.draw();
        }
        ui::bar(0, self.list.bar_y(), self.list.count > 0, self.list.pitch);
        let sel = self.list.selected;
        if input.down & KEY_X != 0 && sel < self.count {
            self.remove(sel);
        } else if a >= 0 && a == self.count {
            self.open_add();
        } else if a >= 0 && self.items[a as usize].locked != 0 {
            self.report(EVENT_ITEM, b"");
        } else if a >= 0 && self.max_count() > 1 {
            self.mode = Mode::Count;
            self.count_edit = self.items[a as usize].count.into();
            self.draw();
        }
        true
    }
}

impl Bag {
    fn max_count(&self) -> i32 {
        self.pv[self.pouch as usize].max_count.into()
    }

    fn load(&mut self) {
        let mut d = data::doc();
        self.pouches = ffi::bag_pouches(&mut d.buf, &mut self.pv) as i32;
        if self.pouch >= self.pouches {
            self.pouch = 0;
        }
        self.count = ffi::bag_items(&mut d.buf, self.pouch as u8, &mut self.items) as i32;
        self.list.count = self.count + 1; // the last row adds an item
        self.list.clamp();
    }

    fn report(&mut self, code: u8, done: &[u8]) {
        self.message = Text::from(if code != 0 {
            ffi::bag_status_text(code)
        } else {
            done
        })
        .cap(40);
    }

    fn changed(&mut self, code: u8, done: &[u8]) {
        if code == 0 {
            data::doc().dirty = true;
        }
        self.report(code, done);
        self.load();
    }

    fn remove(&mut self, i: i32) {
        let what = t!(
            "Removed ",
            &ui::name(item_names(), self.items[i as usize].item.into())
        )
        .cap(40);
        let code = ffi::bag_remove(&mut data::doc().buf, self.pouch as u8, i as u16);
        self.changed(code, &what);
        self.draw();
    }

    fn update_count(&mut self, input: &Input) -> bool {
        let before = self.count_edit;
        if input.rep & KEY_RIGHT != 0 {
            self.count_edit += 1;
        }
        if input.rep & KEY_LEFT != 0 {
            self.count_edit -= 1;
        }
        if input.rep & KEY_UP != 0 {
            self.count_edit += 10;
        }
        if input.rep & KEY_DOWN != 0 {
            self.count_edit -= 10;
        }
        self.count_edit = self.count_edit.max(1).min(self.max_count());
        if input.down & (KEY_A | KEY_B) != 0 {
            if input.down & KEY_A != 0 {
                let i = self.list.selected;
                let code = ffi::bag_set_count(
                    &mut data::doc().buf,
                    self.pouch as u8,
                    i as u16,
                    self.count_edit as u16,
                );
                self.changed(code, b"Count set");
            }
            self.mode = Mode::List;
            self.draw();
        } else if self.count_edit != before {
            self.draw();
        }
        ui::bar(0, self.list.bar_y(), true, self.list.pitch);
        true
    }

    fn open_add(&mut self) {
        self.addable_count =
            ffi::bag_addable(&mut data::doc().buf, self.pouch as u8, &mut self.addable) as i32;
        if self.addable_count == 0 {
            self.message = Text::from(b"Nothing more fits this pouch");
            self.draw();
            return;
        }
        self.mode = Mode::Add;
        self.add_list.selected = 0;
        self.add_list.scroll = 0;
        self.add_list.count = self.addable_count;
        self.message = Text::new();
        self.draw();
    }

    fn update_add(&mut self, input: &Input) -> bool {
        if input.down & KEY_B != 0 {
            self.mode = Mode::List;
            self.draw();
            return true;
        }
        let (a, moved) = self.add_list.update(input);
        if moved {
            self.draw();
        }
        ui::bar(0, self.add_list.bar_y(), true, self.add_list.pitch);
        if a < 0 {
            return true;
        }
        let item = self.addable[a as usize];
        let what = t!("Added ", &ui::name(item_names(), item.into())).cap(40);
        let code = ffi::bag_add(&mut data::doc().buf, self.pouch as u8, item, 1);
        self.changed(code, &what);
        self.mode = Mode::List;
        // Select the new item (sorted pouches put it in order), then set its count.
        for i in 0..self.count {
            if self.items[i as usize].item == item {
                self.list.selected = i;
            }
        }
        self.list.clamp();
        let sel = self.list.selected;
        if self.max_count() > 1 && sel < self.count && self.items[sel as usize].item == item {
            self.mode = Mode::Count;
            self.count_edit = 1;
        }
        self.draw();
        true
    }

    fn draw(&mut self) {
        self.draw_top();
        if self.mode == Mode::Add {
            self.draw_add_list();
        } else {
            self.draw_list();
        }
    }

    fn bottom_frame(&self, title: &str, hints: &str) {
        ui::clear(bottom());
        ui::frame_clear(bottom());
        ui::title(bottom(), title.as_bytes());
        ui::panel(bottom(), 0, self.list.first_row, 32, 20);
        ui::hints(bottom(), hints.as_bytes());
    }

    fn draw_list(&mut self) {
        let sel = self.list.selected;
        let item = sel < self.count;
        let hints = if self.mode == Mode::Count {
            "</>:1|Up/Dn:10|A:Set|B:Cancel"
        } else if !item {
            "A:Add|</>:Pouch|B:Back"
        } else if self.items[sel as usize].locked != 0 {
            "</>:Pouch|B:Back"
        } else if self.max_count() > 1 {
            "A:Count|X:Toss|</>:Pouch|B:Back"
        } else {
            "X:Toss|</>:Pouch|B:Back"
        };
        self.bottom_frame(
            POUCH_NAMES[usize::from(self.pv[self.pouch as usize].kind & 7)],
            hints,
        );
        ui::title_right(bottom(), &t!(self.pouch + 1, "/", self.pouches).cap(24));
        for r in 0..self.list.rows {
            let i = self.list.scroll + r;
            if i > self.count {
                break;
            }
            let y = self.list.text_y(i);
            if i == self.count {
                ui::text(bottom(), LIST_X, y, &t!(MUTED, "+ Add an item"));
                break;
            }
            let it = self.items[i as usize];
            let mut what = ui::name(item_names(), it.item.into()).cap(40);
            ui::fit(&mut what, 200 - LIST_X);
            ui::text(
                bottom(),
                LIST_X,
                y,
                &t!(if it.locked != 0 { MUTED } else { "" }, &what),
            );
            let editing = self.mode == Mode::Count && i == sel;
            let count = if editing {
                self.count_edit
            } else {
                it.count.into()
            };
            if it.locked != 0 {
                ui::text_right(bottom(), 244, y, &t!(MUTED, "event"));
            } else if self.max_count() > 1 {
                let shown = if editing {
                    t!("< ", count, " >")
                } else {
                    t!("x", count)
                };
                ui::text_right(bottom(), 244, y, &shown);
            }
        }
    }

    fn draw_add_list(&mut self) {
        self.bottom_frame("Add an item", "A:Add|L/R:Page|B:Cancel");
        for r in 0..self.add_list.rows {
            let i = self.add_list.scroll + r;
            if i >= self.addable_count {
                break;
            }
            let mut what = ui::name(item_names(), self.addable[i as usize].into()).cap(40);
            ui::fit(&mut what, 244 - LIST_X);
            ui::text(bottom(), LIST_X, self.add_list.text_y(i), &what);
        }
    }

    fn draw_top(&mut self) {
        ui::clear(top());
        ui::frame_clear(top());
        ui::title(top(), b"Bag");
        if data::doc().dirty {
            ui::title_right(top(), b"* unsaved");
        }
        let x = ui::INSET_TOP;
        // Every pouch with how many items it holds; the open one is not muted.
        ui::panel(top(), 1, 2, 30, (self.pouches * ui::PITCH + 8 + 7) / 8);
        let mut l = ui::Lines::new(2);
        for p in 0..self.pouches {
            let y = l.line();
            let pv = self.pv[p as usize];
            let m = if p == self.pouch { "" } else { MUTED };
            let mark = if p == self.pouch { ">" } else { " " };
            ui::text(
                top(),
                x,
                y,
                &t!(m, mark, " ", POUCH_NAMES[usize::from(pv.kind & 7)]),
            );
            ui::text_right(top(), 232, y, &t!(m, pv.count, " / ", pv.slots));
        }
        // The selected item: its icon (from a ROM), name and count.
        let mut item = 0;
        if self.mode == Mode::Add && self.add_list.selected < self.addable_count {
            item = self.addable[self.add_list.selected as usize];
        } else if self.mode != Mode::Add && self.list.selected < self.count {
            item = self.items[self.list.selected as usize].item;
        }
        let row = 16;
        let (mut tiles, mut colors) = ([0u8; 512], [0u16; 16]);
        let generation = data::doc().info.generation;
        let icon = item != 0 && data::item_icon(generation, item, &mut tiles, &mut colors);
        ui::item_icon(
            true,
            16,
            row * 8 + 4,
            icon.then_some((&tiles[..], &colors[..])),
        );
        if item != 0 {
            ui::panel(top(), 1, row, 30, 4);
            let tx = if icon { 48 } else { x };
            ui::text(top(), tx, row * 8 + 4, &ui::name(item_names(), item.into()));
            let it = if self.mode == Mode::Add {
                None
            } else {
                Some(self.items[self.list.selected as usize])
            };
            match it {
                Some(it) if it.locked != 0 => {
                    ui::text(
                        top(),
                        tx,
                        row * 8 + 16,
                        &t!(MUTED, "Event item: from Mystery Gift"),
                    );
                }
                Some(it) => {
                    let count = if self.mode == Mode::Count {
                        self.count_edit
                    } else {
                        it.count.into()
                    };
                    ui::text(
                        top(),
                        tx,
                        row * 8 + 16,
                        &t!(MUTED, "Count", MUTED, "  ", count, " / ", self.max_count()),
                    );
                }
                None => ui::text(top(), tx, row * 8 + 16, &t!(MUTED, "A adds one")),
            }
        }
        if !self.message.is_empty() {
            ui::panel(top(), 1, 21, 30, 2);
            ui::text(top(), x, 21 * 8 + 4, &self.message);
        }
    }
}
