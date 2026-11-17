//! B2W2: Memory Link from a BW save, and keys from another B2W2 save (or all of them).

use alloc::boxed::Box;
use alloc::vec::Vec;

use super::{LIST_X, ListView, scan_saves, spaced_list};
use crate::ffi::{self, UnovaPreview};
use crate::text::{LongText, Prec, Text, file_name};
use crate::ui::{self, Badge, Input, KEY_A, KEY_B, MUTED, Scene, bottom, top};
use crate::{data, t};

struct UnovaLink {
    files: Vec<LongText>,
    scanned: bool,
    list: ListView,
    other: Option<Vec<u8>>,
    other_path: LongText,
    enabling: bool,
    preview: UnovaPreview,
    trainer: Text,
    message: Text,
}

pub fn unova_link() -> Box<dyn Scene> {
    Box::new(UnovaLink {
        files: Vec::new(),
        scanned: false,
        list: spaced_list(),
        other: None,
        other_path: LongText::new(),
        enabling: false,
        preview: UnovaPreview::default(),
        trainer: Text::new(),
        message: Text::new(),
    })
}

impl Scene for UnovaLink {
    fn name(&self) -> &'static str {
        "link"
    }

    fn enter(&mut self) {
        if !self.scanned {
            self.scanned = true;
            for root in [&b"sd:/"[..], b"fat:/"] {
                scan_saves(&mut self.files, root, 3);
            }
            let open = data::doc().path;
            self.files.retain(|f| **f != *open);
            self.list.count = self.files.len() as i32 + 1; // row 0 is "Enable all keys"
            self.list.clamp();
        }
        self.draw();
    }

    fn update(&mut self, input: &Input) -> bool {
        if self.other.is_some() || self.enabling {
            if input.down & KEY_B != 0 {
                self.close_other();
            } else if input.down & KEY_A != 0 && self.ready() {
                self.link();
            }
            return true;
        }
        let (pick, changed) = self.list.update(input);
        if changed {
            self.draw_list();
        }
        ui::bar(0, self.list.bar_y(), true, self.list.pitch);
        if input.down & KEY_B != 0 {
            return false;
        }
        if pick == 0 {
            self.open_enable_all();
        } else if pick > 0 {
            let path = self.files[pick as usize - 1];
            self.open_other(path);
        }
        true
    }
}

impl UnovaLink {
    /// Status 1 is ready, with a note that the link replaces another player's.
    fn ready(&self) -> bool {
        self.preview.status <= 1
    }

    fn replaces(&self) -> bool {
        self.preview.status == 1
    }

    fn open_other(&mut self, path: LongText) {
        let Some(mut other) = data::read_save_file(&path) else {
            self.message = Text::from(b"Could not read that file.");
            self.draw();
            return;
        };
        self.other_path = path;
        self.message = Text::new();
        self.trainer = Text::new();
        self.preview = ffi::unova_preview(&mut data::doc().buf, &other);
        if let Some(info) = ffi::save_probe(&mut other) {
            self.trainer = ui::utf16_to_text(&info.ot_name, 32);
        }
        self.other = Some(other);
        self.draw();
    }

    fn open_enable_all(&mut self) {
        self.preview = ffi::unova_enable_preview(&mut data::doc().buf);
        self.enabling = true;
        self.message = Text::new();
        self.draw();
    }

    fn close_other(&mut self) {
        self.other = None;
        self.enabling = false;
        self.draw();
    }

    fn link(&mut self) {
        let seed = (blocksds::time::unix_time() as u32)
            .wrapping_mul(1_103_515_245)
            .wrapping_add(12345);
        let r = {
            let mut d = data::doc();
            let r = if self.enabling {
                ffi::unova_enable_all(&mut d.buf)
            } else {
                ffi::unova_apply(&mut d.buf, self.other.as_deref().unwrap_or(&[]), seed)
            };
            if r == 0 {
                d.dirty = true;
            }
            r
        };
        self.message = if r == 0 {
            Text::from(b"Done. Save to SD to keep it.")
        } else {
            Text::from(ffi::unova_status_text(r)).cap(40)
        };
        self.close_other();
    }

    fn draw(&mut self) {
        ui::clear(top());
        ui::clear(bottom());
        ui::frame_clear(top());
        ui::frame_clear(bottom());
        ui::title(top(), b"Memory Link and keys");
        if self.other.is_some() || self.enabling {
            self.draw_preview();
        } else {
            ui::panel(top(), 1, 2, 30, 8);
            let mut l = ui::Lines::new(2);
            let x = ui::INSET_TOP;
            ui::text(top(), x, l.line(), b"Pick a Black or White save for");
            ui::text(top(), x, l.line(), b"Memory Link, or a Black 2 or");
            ui::text(top(), x, l.line(), b"White 2 save to receive its keys.");
            l.block();
            ui::text(
                top(),
                x,
                l.line(),
                &t!(MUTED, "Or enable every key, no partner."),
            );
            if !self.message.is_empty() {
                ui::panel(top(), 1, 12, 30, 2);
                ui::text(top(), x, 12 * 8 + 4, &self.message);
            }
            self.draw_list();
        }
    }

    fn draw_preview(&mut self) {
        const KEYS: [&str; 5] = [
            "Easy Mode",
            "Challenge Mode",
            "City swap",
            "Iron Chamber",
            "Iceberg Chamber",
        ];
        let x = ui::INSET_TOP;
        let mut top_row = 2;
        if self.other.is_some() {
            ui::panel(top(), 1, 2, 30, 3);
            let mut l = ui::Lines::new(2);
            ui::text(
                top(),
                x,
                l.line(),
                &t!(Prec(file_name(&self.other_path), 30)),
            );
            ui::text(
                top(),
                x,
                l.line(),
                &t!(MUTED, "Trainer", MUTED, "  ", &self.trainer),
            );
            top_row = 7;
        }
        ui::bar(0, 0, false, 8);
        ui::panel(top(), 1, top_row, 30, 21 - top_row);
        let mut l = ui::Lines::new(top_row);
        let p = self.preview;
        if !self.ready() {
            ui::badge(
                top(),
                2,
                l.grid_row(),
                Badge::Invalid,
                ffi::unova_status_text(p.status),
            );
        } else if p.mode == 1 {
            ui::text(top(), x, l.line(), b"Memory Link");
            let flashback = if p.flashback != 0 {
                "Unlocks a new flashback"
            } else {
                "No new flashbacks"
            };
            ui::text(top(), x, l.line(), &t!(MUTED, flashback));
            if self.replaces() {
                l.block();
                ui::text(top(), x, l.line(), ffi::unova_status_text(1));
            }
        } else {
            let heading = if p.mode == 3 {
                "Keys this will enable:"
            } else {
                "Keys this save will receive:"
            };
            ui::text(top(), x, l.line(), heading.as_bytes());
            for (i, key) in KEYS.iter().enumerate() {
                if p.keys[i] != 0 {
                    ui::badge(top(), 3, l.grid_row(), Badge::Legal, key.as_bytes());
                }
            }
            if p.mode == 3 {
                l.block();
                ui::text(
                    top(),
                    x,
                    l.line(),
                    &t!(MUTED, "These can't be sent to other games."),
                );
            }
        }
        ui::title(bottom(), b"Link");
        ui::panel(bottom(), 1, 3, 30, 4);
        let mut b = ui::Lines::new(3);
        let ready = self.ready();
        ui::text(
            bottom(),
            x,
            b.line(),
            if ready {
                b"Ready."
            } else {
                b"Can't link these saves."
            },
        );
        ui::text(
            bottom(),
            x,
            b.line(),
            &t!(MUTED, "Nothing changes until you link."),
        );
        ui::hints(bottom(), if ready { b"A:Link|B:Back" } else { b"B:Back" });
    }

    fn draw_list(&mut self) {
        ui::title(bottom(), b"Link with");
        ui::panel(bottom(), 0, self.list.first_row, 32, 20);
        ui::hints(bottom(), b"A:Choose|L/R:Page|B:Back");
        ui::clear_px(bottom(), 16, 176);
        for r in 0..self.list.rows {
            let i = self.list.scroll + r - 1;
            let y = self.list.text_y(self.list.scroll + r);
            if i < 0 {
                ui::text(bottom(), LIST_X, y, b"Enable all keys");
                ui::text_right(bottom(), 244, y, &t!(MUTED, "no partner"));
            } else if let Some(f) = self.files.get(i as usize) {
                ui::text(bottom(), LIST_X, y, &t!(Prec(file_name(f), 30)));
            }
        }
    }
}
