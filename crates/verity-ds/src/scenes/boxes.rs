//! The box view with the party panel, and the summary of the selected Pokémon.

use alloc::boxed::Box;

use super::{
    NAME_ABILITIES, NAME_ITEMS4, NAME_ITEMS5, NAME_MOVES, NAME_NATURES, NAME_SPECIES, NAME_TYPES,
};
use super::{add_wizard, editor};
use crate::cell::Global;
use crate::ffi::PkmView;
use crate::text::{Prec, Right, Text, Zero};
use crate::ui::{
    self, Badge, Input, KEY_A, KEY_B, KEY_DOWN, KEY_L, KEY_LEFT, KEY_R, KEY_RIGHT, KEY_SELECT,
    KEY_UP, KEY_X, MUTED, Scene, bottom, top,
};
use crate::{data, t};

static SPRITE_TILES: Global<[u8; 4608]> = Global::new([0; 4608]);

/// The Pokémon's front sprite at x, y with its ball behind the top-left corner, from the sprite
/// and item caches of the save's generation; false (nothing drawn) without them.
pub(crate) fn show_pokemon(v: &PkmView, x: i32, y: i32, ball: bool) -> bool {
    let mut tiles = SPRITE_TILES.borrow_mut();
    let mut colors = [0u16; 16];
    let generation = data::doc().info.generation;
    let shiny = v.is_shiny != 0 && v.is_egg == 0;
    if !data::pokemon_sprite(
        generation,
        v.species,
        v.form,
        v.gender == 1,
        v.is_egg != 0,
        shiny,
        &mut tiles,
        &mut colors,
    ) {
        return false;
    }
    ui::pokemon_sprite(x, y, Some((&tiles[..], &colors[..])));
    let (mut ball_tiles, mut bc) = ([0u8; 512], [0u16; 16]);
    if ball && v.is_egg == 0 && data::ball_icon(generation, v.ball, &mut ball_tiles, &mut bc) {
        ui::item_icon(true, x - 2, y - 4, Some((&ball_tiles[..], &bc[..]))); // its outline clear of the panel border
    }
    true
}

/// Text beside the sprite starts here (the sprite is 96 px wide from x 8).
const SIDE_X: i32 = 112;

struct BoxView {
    box_index: i32,
    cur: i32,
    party: bool,
    dirty: bool,
}

pub fn box_view() -> Box<dyn Scene> {
    Box::new(BoxView {
        box_index: 0,
        cur: 0,
        party: false,
        dirty: true,
    })
}

impl Scene for BoxView {
    fn name(&self) -> &'static str {
        "boxes"
    }

    fn enter(&mut self) {
        // Drawn now, behind the fade: waiting for the first update let the fade-in show the
        // previous scene's top screen for a few frames.
        ui::clear(bottom());
        self.draw();
        self.dirty = false;
    }

    fn update(&mut self, input: &Input) -> bool {
        if input.down & KEY_B != 0 {
            return false;
        }
        let boxes = data::doc().boxes as i32;
        let n = if self.party { 6 } else { 30 };
        let cols = if self.party { 2 } else { 6 };
        let (before_box, before_cur, before_party) = (self.box_index, self.cur, self.party);

        if input.rep & KEY_LEFT != 0 {
            self.cur = if self.cur % cols == 0 {
                self.cur + cols - 1
            } else {
                self.cur - 1
            };
        }
        if input.rep & KEY_RIGHT != 0 {
            self.cur = if self.cur % cols == cols - 1 {
                self.cur - cols + 1
            } else {
                self.cur + 1
            };
        }
        if input.rep & KEY_UP != 0 {
            self.cur = (self.cur - cols + n) % n;
        }
        if input.rep & KEY_DOWN != 0 {
            self.cur = (self.cur + cols) % n;
        }
        if !self.party && input.rep & KEY_L != 0 {
            self.box_index = (self.box_index + boxes - 1) % boxes;
        }
        if !self.party && input.rep & KEY_R != 0 {
            self.box_index = (self.box_index + 1) % boxes;
        }
        if input.down & KEY_SELECT != 0 {
            self.party = !self.party;
            self.cur = 0;
        }
        if input.tapped {
            self.tap(input.tap_x, input.tap_y, boxes);
        }
        if input.down & KEY_A != 0 && self.at_cell(self.cur).species != 0 {
            ui::push(editor(self.box_byte(), self.cur as u8));
            return true;
        }
        if input.down & KEY_X != 0
            || (input.down & KEY_A != 0 && self.at_cell(self.cur).species == 0)
        {
            let mut target = if self.at_cell(self.cur).species == 0 {
                self.cur
            } else {
                -1
            };
            for i in 0..n {
                if target >= 0 {
                    break;
                }
                if self.at_cell(i).species == 0 {
                    target = i;
                }
            }
            let party_count = i32::from(data::doc().info.party_count);
            if target >= 0 && !(self.party && target > party_count) {
                let slot = if self.party { party_count } else { target };
                ui::push(add_wizard(self.box_byte(), slot as u8));
                return true;
            }
        }

        if self.box_index != before_box
            || self.cur != before_cur
            || self.party != before_party
            || self.dirty
        {
            self.draw();
            self.dirty = false;
        }
        true
    }
}

impl BoxView {
    fn box_byte(&self) -> u8 {
        if self.party {
            0xFF
        } else {
            self.box_index as u8
        }
    }

    fn cell_pos(&self, i: i32) -> (i32, i32) {
        // On the cell grid, so each slot's backing is whole frame tiles.
        if self.party {
            (48 + (i % 2) * 112, 24 + (i / 2) * 48)
        } else {
            (8 + (i % 6) * 40, 16 + (i / 6) * 32)
        }
    }

    fn tap(&mut self, x: i32, y: i32, boxes: i32) {
        if y < 16 {
            if !self.party && x < 64 {
                self.box_index = (self.box_index + boxes - 1) % boxes;
            } else if !self.party && x >= 192 {
                self.box_index = (self.box_index + 1) % boxes;
            }
            return;
        }
        if y >= 176 {
            self.party = !self.party;
            self.cur = 0;
            return;
        }
        let n = if self.party { 6 } else { 30 };
        for i in 0..n {
            let (cx, cy) = self.cell_pos(i);
            if x >= cx && x < cx + 40 && y >= cy && y < cy + 32 {
                self.cur = i;
            }
        }
    }

    fn at_cell(&self, i: i32) -> PkmView {
        data::slot(self.box_byte(), i as u8)
    }

    fn draw(&mut self) {
        ui::clear(bottom());
        ui::frame_clear(bottom());
        if self.party {
            ui::title(bottom(), b"Party");
        } else {
            let name = data::box_name(self.box_index as u32, 64);
            let pad = ((30 - ui::text_len(&name)) / 2 - 1).max(0);
            ui::title(bottom(), &t!("<", Right(b"", pad as usize), &name));
            let boxes = data::doc().boxes;
            ui::title_right(bottom(), &t!(self.box_index + 1, "/", boxes, "  >").cap(24));
        }
        let hints: &[u8] = if self.party {
            b"SEL:Boxes|A:Edit|X:Add|B:Back"
        } else {
            b"L/R:Box|SEL:Party|A:Edit|X:Add"
        };
        ui::hints(bottom(), hints);
        ui::sprites_hide_all();
        let n = if self.party { 6 } else { 30 };
        for i in 0..n {
            let v = self.at_cell(i);
            let (x, y) = self.cell_pos(i);
            let style = if i == self.cur {
                2
            } else if v.species != 0 {
                0
            } else {
                1
            };
            ui::slot(bottom(), x / 8, y / 8, style);
            let mut tiles = [0u8; 512];
            let icon = if v.species != 0 {
                data::icon_tiles(v.species, v.form, v.gender == 1, v.is_egg != 0, &mut tiles)
            } else {
                None
            };
            if let Some(pal) = icon {
                let shiny = v.is_shiny != 0 && v.is_egg == 0;
                ui::icon_rom(
                    2 + 2 * i,
                    x + ui::SLOT_ICON_X,
                    y + ui::SLOT_ICON_Y,
                    &tiles,
                    pal,
                    shiny,
                );
            } else if v.species != 0 {
                // The placeholders are drawn centred in their frame, unlike the game's icons.
                let ty = if v.types[0] == 0xFF { 17 } else { v.types[0] };
                ui::icon(
                    2 + 2 * i,
                    x + ui::SLOT_ICON_X,
                    y - 1,
                    ty,
                    v.is_egg != 0,
                    v.is_shiny != 0,
                );
            }
        }
        self.details(&self.at_cell(self.cur));
    }

    /// The top screen while browsing, laid out like the games' summary: the Pokémon's sprite (its
    /// ball behind it) with the verdict and who it is beside it, then its moves and IVs across the
    /// panel. EVs and the PID are in the editor.
    fn details(&self, v: &PkmView) {
        ui::clear(top());
        ui::frame_clear(top());
        ui::top_art_hide();
        ui::item_icon(true, 0, 0, None);
        let (x, rx) = (ui::INSET_TOP, SIDE_X);
        if v.species == 0 {
            ui::title(top(), b"Empty slot");
            ui::panel(top(), 1, 2, 30, 3);
            let mut l = ui::Lines::new(2);
            ui::text(top(), x, l.line(), b"Nothing here.");
            ui::text(
                top(),
                x,
                l.line(),
                &t!(MUTED, "X or A adds a Pokémon here."),
            );
            return;
        }
        let nick = ui::utf16_to_text(&v.nickname, 32);
        let mut ot = ui::utf16_to_text(&v.ot_name, 32);
        ui::fit(&mut ot, 80);
        let ot = ot.cap(32);
        let sp = ui::name(NAME_SPECIES, v.species.into());
        let generation = data::doc().info.generation;
        let art = show_pokemon(v, 8, 16, true);
        // Without a ROM there is no sprite: the text starts at the panel's edge.
        let cx = if art { rx } else { x };
        if v.is_egg != 0 {
            ui::title(top(), b"Egg");
            ui::panel(top(), 1, 2, 30, 11);
            let mut l = ui::Lines::new(2);
            self.legality_line(&mut l, v, cx / 8);
            l.block();
            ui::text(
                top(),
                cx,
                l.line(),
                &t!(MUTED, "Will hatch into", MUTED, " ", &sp),
            );
            ui::text(top(), cx, l.line(), &t!(MUTED, "OT", MUTED, "  ", &ot));
            ui::text(
                top(),
                cx,
                l.line(),
                &t!(
                    MUTED,
                    "ID",
                    MUTED,
                    "  ",
                    Zero(v.tid.into(), 5),
                    " / ",
                    Zero(v.sid.into(), 5)
                ),
            );
            return;
        }
        let lv = t!("Lv ", v.level).cap(12);
        ui::title(top(), b"");
        ui::title_right(top(), &lv);
        // The box markings in the title bar, set ones bright; the name is cut short of them.
        let mx = 250 - ui::text_width(&lv) - 10 - 6 * 9;
        for k in 0..6 {
            ui::shape(
                top(),
                mx + 9 * k,
                0,
                k,
                if v.markings >> k & 1 != 0 { 0 } else { 1 },
            );
        }
        if v.is_nicknamed != 0 && *nick != *sp {
            ui::text_in(top(), 4, mx - 6, 0, &t!(&nick, "  ", MUTED, &sp));
        } else {
            ui::text_in(top(), 4, mx - 6, 0, &nick);
        }
        ui::panel(top(), 1, 2, 30, 20);
        let mut l = ui::Lines::new(2);
        self.legality_line(&mut l, v, cx / 8);
        // Types (the game's icons, else names), then the gender and the shiny star.
        let (ty, mut gx) = (34, cx);
        for k in 0..2 {
            if v.types[k] == 0xFF || (k == 1 && v.types[1] == v.types[0]) {
                continue;
            }
            let (mut t, mut c) = ([0u8; 256], [0u16; 16]);
            if data::type_icon(generation, 0, v.types[k].into(), &mut t, &mut c) {
                ui::small_icon(k, gx, ty, Some((&t[..], &c[..])));
                gx += 36;
            } else {
                let tn = ui::name(NAME_TYPES, v.types[k].into());
                ui::text(top(), gx, ty + 4, &tn);
                gx += ui::text_width(&tn) + 6;
            }
        }
        if v.gender < 2 {
            let (shape, tone) = if v.gender == 0 {
                (ui::MALE, 3)
            } else {
                (ui::FEMALE, 4)
            };
            ui::shape(top(), gx + 2, ty + 4, shape, tone);
            gx += 2 + 7 + 4;
        }
        if v.is_shiny != 0 {
            ui::shape(top(), gx, ty + 4, ui::SHINY, 2);
        }
        let y = 54;
        let line = t!(
            &ui::name(NAME_NATURES, v.nature.into()),
            " ",
            MUTED,
            "nature"
        )
        .cap(64);
        ui::text(top(), cx, y, &line);
        let mut line = ui::name(NAME_ABILITIES, v.ability.into()).cap(64);
        ui::fit(&mut line, 244 - cx);
        ui::text(top(), cx, y + 12, &line);
        let items = if generation == 4 {
            NAME_ITEMS4
        } else {
            NAME_ITEMS5
        };
        let held = if v.held_item != 0 {
            ui::name(items, v.held_item.into())
        } else {
            Text::from(b"nothing")
        };
        let mut line = t!(MUTED, "Holds", MUTED, " ", &held).cap(64);
        ui::fit(&mut line, 244 - cx);
        ui::text(top(), cx, y + 24, &line);
        // The OT name is what gets cut (a wide ROM font), never the ID.
        let id = t!("  ", MUTED, "ID", MUTED, " ", Zero(v.tid.into(), 5)).cap(24);
        let mut ot_fit = ot.cap(32);
        ui::fit(
            &mut ot_fit,
            244 - cx - ui::text_width(&t!(MUTED, "OT", MUTED, " ")) - ui::text_width(&id),
        );
        let line = t!(MUTED, "OT", MUTED, " ", &ot_fit, &id).cap(64);
        ui::text(top(), cx, y + 40, &line);
        // The level first, so a long place name is what gets cut.
        let mut line = t!(
            MUTED,
            "Met",
            MUTED,
            " Lv ",
            v.met_level,
            ", ",
            &ui::location(generation, v.met_location)
        )
        .cap(64);
        ui::fit(&mut line, 244 - cx);
        ui::text(top(), cx, y + 52, &line);
        // Across the panel, under the sprite.
        let mut y = 120;
        for m in (0..4).step_by(2) {
            for k in 0..2 {
                let mv = v.moves[m + k];
                let name = if mv != 0 {
                    ui::name(NAME_MOVES, mv.into())
                } else {
                    Text::from(b"-")
                };
                ui::text(top(), x + k as i32 * 116, y, &t!(Prec(&name, 15)));
            }
            y += ui::PITCH;
        }
        const ST: [&str; 6] = ["HP", "Atk", "Def", "Spe", "SpA", "SpD"];
        let (hy, iy) = (y + 8, y + 8 + ui::PITCH);
        ui::text(top(), x, iy, &t!(MUTED, "IVs"));
        for (k, st) in ST.iter().enumerate() {
            let cx2 = x + 56 + k as i32 * 30; // right edge of each column
            ui::text_right(top(), cx2, hy, &t!(MUTED, *st));
            ui::text_right(top(), cx2, iy, &t!(v.ivs[k]));
        }
    }

    /// The verdict on one grid line: a badge, then the time or what failed.
    fn legality_line(&self, l: &mut ui::Lines, v: &PkmView, col: i32) {
        legality_line(l, v, col, self.box_byte(), self.cur as u8);
    }
}

/// The verdict for (box_index, slot) on one grid line: a badge, then the time or what failed.
pub(crate) fn legality_line(l: &mut ui::Lines, v: &PkmView, col: i32, box_index: u8, slot: u8) {
    let (r, ms) = data::legality(box_index, slot);
    const WHY: [&str; 7] = [
        "",
        "egg",
        "origin game",
        "data missing",
        "Gen 5 save",
        "hatched",
        "event/trade/walker",
    ];
    let mut line = ui::Row::new(top(), l.grid_row(), col * 8);
    let mut rest = if r.status == 1 {
        line.badge(Badge::Legal, b"LEGAL");
        if col > 2 {
            t!(MUTED, ms, " ms")
        } else {
            t!(MUTED, "checked in ", ms, " ms")
        }
    } else if r.status == 2 {
        line.badge(Badge::Invalid, b"ILLEGAL");
        super::failed_checks(&r)
    } else {
        line.badge(Badge::Fishy, b"NOT CHECKED");
        t!(WHY.get(usize::from(r.unsupported)).copied().unwrap_or("?"))
    };
    if v.checksum_ok == 0 {
        rest.push_bytes(b"  ! checksum");
    }
    line.shrink(&rest.cap(48));
    line.draw();
}
