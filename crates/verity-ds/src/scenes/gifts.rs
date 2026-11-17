//! Mystery Gifts for the open save.

use alloc::boxed::Box;
use alloc::vec::Vec;

use super::{LIST_X, ListView, NAME_SPECIES, item_names, spaced_list};
use crate::ffi::{self, EventView, Zeroed};
use crate::text::{Prec, Text, Zero};
use crate::ui::{self, Badge, Input, KEY_B, MUTED, Scene, bottom, top};
use crate::{data, t};

const MAX_EVENTS: usize = 500;

struct Gifts {
    events: Vec<EventView>,
    count: i32,
    list: ListView,
    message: Text,
}

pub fn gifts() -> Box<dyn Scene> {
    Box::new(Gifts {
        events: Vec::new(),
        count: 0,
        list: spaced_list(),
        message: Text::new(),
    })
}

impl Scene for Gifts {
    fn name(&self) -> &'static str {
        "gifts"
    }

    fn enter(&mut self) {
        ui::clear(bottom());
        ui::title(bottom(), b"Mystery gifts");
        ui::hints(bottom(), b"A:Deliver|L/R:Page|B:Back");
        if !data::ensure_events_data() {
            ui::clear(top());
            ui::title(top(), b"Mystery gifts");
            ui::panel(top(), 1, 2, 30, 3);
            let mut l = ui::Lines::new(2);
            ui::text(top(), ui::INSET_TOP, l.line(), b"Gift data is missing.");
            ui::text(top(), ui::INSET_TOP, l.line(), &t!(MUTED, "B goes back."));
            return;
        }
        ui::panel(bottom(), 0, self.list.first_row, 32, 20);
        self.refresh();
        self.draw_list();
        self.draw_top();
    }

    fn update(&mut self, input: &Input) -> bool {
        if input.down & KEY_B != 0 {
            return false;
        }
        let (a, changed) = self.list.update(input);
        if changed {
            self.message = Text::new();
            self.draw_list();
            self.draw_top();
        }
        ui::bar(0, self.list.bar_y(), self.count > 0, self.list.pitch);
        if a >= 0 {
            self.deliver(a as usize);
        }
        true
    }
}

fn kind_name(t: u8) -> &'static str {
    match t {
        1 => "Pokemon",
        3 => "Item",
        5 => "Decoration",
        7 => "Pokemon Egg",
        8 => "Member Card",
        9 => "Oak's Letter",
        11 => "Poketch app",
        12 => "Secret Key",
        14 => "Pokewalker course",
        _ => "Gift",
    }
}

fn region_tag(r: u8) -> &'static str {
    ["", "NA", "UK", "AU", "EU"]
        .get(usize::from(r))
        .copied()
        .unwrap_or("")
}

/// What the card gives: the Pokémon, the item, or the kind of gift.
fn gives(e: &EventView) -> Text {
    if e.ranger != 0 {
        Text::from(b"Manaphy Egg")
    } else if e.species != 0 {
        ui::name(NAME_SPECIES, e.species.into())
    } else if e.item != 0 {
        ui::name(item_names(), e.item.into())
    } else {
        t!(kind_name(e.gift_type))
    }
}

impl Gifts {
    fn refresh(&mut self) {
        if self.events.is_empty() {
            self.events = alloc::vec![EventView::zeroed(); MAX_EVENTS];
        }
        let mut d = data::doc();
        self.count = ffi::events_list(&mut d.buf, &mut self.events) as i32;
        self.list.count = self.count;
        self.list.clamp();
    }

    fn deliver(&mut self, i: usize) {
        let e = self.events[i];
        // The card stores the date it was received, as days since 2000-01-01.
        let now = blocksds::time::unix_time();
        let days = if now > 946_684_800 {
            ((now - 946_684_800) / 86400) as u16
        } else {
            0
        };
        let r = {
            let mut d = data::doc();
            let r = ffi::event_inject(&mut d.buf, e.blob, e.record, days);
            if r == 0 {
                d.dirty = true;
            }
            r
        };
        self.message = if r == 0 {
            Text::from(b"Delivered. Save, then collect it.")
        } else {
            Text::from(ffi::event_status_text(r)).cap(40)
        };
        self.refresh();
        self.draw_list();
        self.draw_top();
    }

    /// One row per card: what it gives, then what tells releases of one event apart (the
    /// original trainer's name and ID, and the region). Gifts that can't be delivered now are
    /// muted.
    fn draw_list(&mut self) {
        ui::clear_px(bottom(), 16, 176);
        for r in 0..self.list.rows {
            let i = self.list.scroll + r;
            if i >= self.count {
                break;
            }
            let e = &self.events[i as usize];
            let y = self.list.text_y(i);
            let m = if e.status != 0 { MUTED } else { "" };
            let mut what = gives(e).cap(40);
            ui::fit(&mut what, 96 - LIST_X - 4);
            ui::text(bottom(), LIST_X, y, &t!(m, &what));
            let ot = ui::utf16_to_text(&e.ot, 24);
            if !ot.is_empty() {
                ui::text(bottom(), 96, y, &t!(MUTED, &ot)); // at most 7 characters
                ui::text(bottom(), 168, y, &t!(MUTED, Zero(e.tid.into(), 5)));
            }
            if !region_tag(e.region).is_empty() {
                ui::text_right(bottom(), 244, y, &t!(MUTED, region_tag(e.region)));
            }
        }
    }

    fn draw_top(&mut self) {
        ui::clear(top());
        ui::frame_clear(top());
        ui::title(top(), b"Mystery gifts");
        ui::title_right(top(), &t!(self.count, " for this game").cap(20));
        if self.count == 0 {
            ui::panel(top(), 1, 2, 30, 3);
            let mut l = ui::Lines::new(2);
            ui::text(top(), ui::INSET_TOP, l.line(), b"None for this language.");
            ui::text(top(), ui::INSET_TOP, l.line(), &t!(MUTED, "B goes back."));
            return;
        }
        let e = self.events[self.list.selected as usize];
        let title = if e.ranger != 0 {
            Text::from(b"Ranger Manaphy Egg")
        } else {
            ui::utf16_to_text(&e.title, 37)
        };
        let (x, v) = (ui::INSET_TOP, 88); // labels, values
        // The title and whether it can be delivered, then the card's details.
        ui::panel(top(), 1, 2, 30, 12);
        let mut l = ui::Lines::new(2);
        ui::text(top(), x, l.line(), &t!(Prec(&title, 32)));
        // The status on a row of its own: some are long ("12 cards held: discard one in-game").
        let badge = if e.status != 0 {
            Badge::Fishy
        } else {
            Badge::Legal
        };
        ui::badge(
            top(),
            x / 8,
            l.grid_row(),
            badge,
            ffi::event_status_text(e.status),
        );
        l.block();
        let mut field = |label: &str, value: &[u8]| {
            let y = l.line();
            ui::text(top(), x, y, &t!(MUTED, label));
            ui::text(top(), v, y, value);
        };
        field("Type", kind_name(e.gift_type).as_bytes());
        if e.species != 0 || e.item != 0 {
            field(if e.species != 0 { "Pokemon" } else { "Item" }, &gives(&e));
        }
        let ot = ui::utf16_to_text(&e.ot, 24);
        if !ot.is_empty() {
            field(
                "OT",
                &t!(&ot, "   ", MUTED, "ID", MUTED, " ", Zero(e.tid.into(), 5)),
            );
        }
        const REGIONS: [&str; 5] = ["", "North America", "United Kingdom", "Australia", "Europe"];
        if (1..=4).contains(&e.region) {
            field("Region", REGIONS[usize::from(e.region)].as_bytes());
        }
        if e.card_id != 0 {
            field("Card", &t!(e.card_id));
        }
        if !self.message.is_empty() {
            ui::panel(top(), 1, 16, 30, 2);
            ui::text(top(), x, 16 * 8 + 4, &self.message);
        }
        ui::panel(top(), 1, 20, 30, 3);
        let mut c = ui::Lines::new(20);
        ui::text(
            top(),
            x,
            c.line(),
            &t!(MUTED, "Collect it in-game from the"),
        );
        let place = if data::doc().info.generation == 5 {
            "deliveryman in any Poke Center."
        } else {
            "deliveryman in any Poke Mart."
        };
        ui::text(top(), x, c.line(), &t!(MUTED, place));
    }
}
