//! The add-Pokémon wizard: a legal wild encounter or a bred egg, placed at (box_index, slot)
//! (0xFF box = party).

use alloc::boxed::Box;
use alloc::vec;
use alloc::vec::Vec;

use super::{LIST_X, ListView, NAME_ITEMS4, NAME_NATURES, NAME_SPECIES, spaced_list};
use crate::cell::Var;
use crate::ffi::{self, EggRequest, Encounter, GenRequest, Loc, Zeroed};
use crate::text::{Text, Zero};
use crate::ui::{
    self, Input, KEY_A, KEY_B, KEY_DOWN, KEY_LEFT, KEY_RIGHT, KEY_SELECT, KEY_START, KEY_UP, KEY_Y,
    MUTED, Scene, bottom, top,
};
use crate::{data, t};

const OPTS: i32 = 7;
const EGG_OPTS: i32 = 5;
const BALLS: [u8; 14] = [4, 3, 2, 1, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15];

static EGG_COUNTER: Var<u32> = Var::new(0);
static GEN_COUNTER: Var<u32> = Var::new(0);

struct AddWizard {
    box_index: u8,
    slot: u8,
    loaded: bool,
    supported: bool,
    version: u8,
    species: Vec<u16>,
    species_count: i32,
    species_ix: i32,
    enc: Vec<Encounter>,
    enc_count: i32,
    enc_ix: i32,
    step: i32,
    opt: i32,
    nature: i32, // -1 any
    shiny: bool,
    level: i32,  // 0 any, else the level
    gender: i32, // -1 any, 0 male, 1 female
    lead: i32,   // 0 none, 1 Synchronize, 2 Cute Charm, 3 Poke Radar chain
    ball: i32,   // 0 default, else a ball ID from BALLS
    egg: bool,
    hatched: bool,
    status: Text,
    list: ListView,
    enc_list: ListView,
}

pub fn add_wizard(box_index: u8, slot: u8) -> Box<dyn Scene> {
    Box::new(AddWizard {
        box_index,
        slot,
        loaded: false,
        supported: false,
        version: 12,
        species: vec![0; 649],
        species_count: 0,
        species_ix: 0,
        enc: vec![Encounter::zeroed(); 64],
        enc_count: 0,
        enc_ix: -1,
        step: 0,
        opt: 0,
        nature: -1,
        shiny: false,
        level: 0,
        gender: -1,
        lead: 0,
        ball: 0,
        egg: false,
        hatched: false,
        status: Text::new(),
        list: spaced_list(),
        enc_list: spaced_list(),
    })
}

/// Option rows on the grid pitch; the highlight fills the row.
fn opt_y(r: i32) -> i32 {
    2 * 8 + 4 + ui::GRID_PITCH * r
}

fn opt_bar_y(r: i32) -> i32 {
    opt_y(r) - (ui::GRID_PITCH - 8) / 2
}

fn opt_at(tap_y: i32) -> i32 {
    let off = tap_y - 2 * 8 - 4 + (ui::GRID_PITCH - 8) / 2;
    if off < 0 { -1 } else { off / ui::GRID_PITCH }
}

fn opt(r: i32, label: &str, value: &[u8]) {
    ui::text(bottom(), LIST_X, opt_y(r), &t!(MUTED, label));
    ui::text(bottom(), 96, opt_y(r), &Text::from(value).cap(48));
}

fn game_name(v: u8) -> &'static str {
    match v {
        10 => "Diamond",
        11 => "Pearl",
        7 => "HeartGold",
        8 => "SoulSilver",
        20 => "White",
        21 => "Black",
        22 => "White 2",
        23 => "Black 2",
        _ => "Platinum",
    }
}

fn slot_type(t: u8) -> &'static str {
    const NAMES: [&str; 15] = [
        "Grass",
        "Surf",
        "Old Rod",
        "Good Rod",
        "Super Rod",
        "Rock",
        "Headbutt",
        "Headbutt",
        "Contest",
        "Honey",
        "Safari",
        "Safari",
        "Saf rod",
        "Saf rod",
        "Saf rod",
    ];
    NAMES.get(usize::from(t)).copied().unwrap_or("?")
}

fn slot_type5(t: u8) -> &'static str {
    const NAMES: [&str; 6] = ["Standard", "Grass", "Surf", "Super Rod", "Swarm", "Grotto"];
    NAMES.get(usize::from(t)).copied().unwrap_or("?")
}

fn lead_name(l: i32) -> &'static str {
    ["None", "Synchronize", "Cute Charm", "Poke Radar"][l as usize]
}

fn gender_name(g: i32) -> &'static str {
    match g {
        g if g < 0 => "Any",
        0 => "Male",
        _ => "Female",
    }
}

fn yes_no(b: bool) -> &'static str {
    if b { "Yes" } else { "No" }
}

fn nature_name(n: i32) -> Text {
    if n < 0 {
        Text::from(b"Any")
    } else {
        ui::name(NAME_NATURES, n as u32)
    }
}

/// Today as (years since 2000, month, day).
fn today() -> [u8; 3] {
    let t = blocksds::time::now();
    if t.year != 0 {
        [(t.year - 2000) as u8, t.month, t.day]
    } else {
        [26, 1, 1]
    }
}

/// A seed from the clock and a count of the seeds taken.
fn seed(counter: &Var<u32>) -> u32 {
    counter.set(counter.get().wrapping_add(1));
    (blocksds::time::unix_time() as u32)
        .wrapping_mul(2_654_435_761)
        .wrapping_add(counter.get())
}

impl Scene for AddWizard {
    fn name(&self) -> &'static str {
        "add"
    }

    fn enter(&mut self) {
        if !self.loaded {
            self.loaded = true;
            self.version = data::wizard_version();
            self.supported = data::ensure_save_data();
            if self.supported {
                self.load_species();
            }
        }
        self.draw();
    }

    fn update(&mut self, input: &Input) -> bool {
        if !self.supported {
            return input.down & (KEY_B | KEY_A) == 0;
        }
        if self.step == 0 {
            self.list.count = self.species_count;
            let (a, changed) = self.list.update(input);
            ui::bar(0, self.list.bar_y(), true, self.list.pitch);
            if input.down & KEY_B != 0 {
                return false;
            }
            if input.down & KEY_Y != 0 && !self.gen5() {
                self.egg = !self.egg;
                self.load_species();
                self.list = spaced_list();
                self.draw();
                return true;
            }
            if input.down & KEY_SELECT != 0 && self.other_game() != 0 {
                self.version = self.other_game();
                self.load_species();
                self.list = spaced_list();
                self.draw();
                return true;
            }
            if a >= 0 {
                self.species_ix = a;
                if self.egg {
                    self.step = 2;
                    self.opt = 0;
                    self.draw();
                    ui::bar(0, opt_bar_y(0), true, ui::GRID_PITCH);
                    return true;
                }
                self.enc_count =
                    ffi::wild_encounters(self.version, self.species[a as usize], &mut self.enc)
                        as i32;
                self.step = 1;
                self.enc_list = spaced_list();
                self.enc_list.count = self.enc_count + 1; // row 0 is "any slot"
                self.draw();
            } else if changed {
                self.draw_list();
            }
        } else if self.step == 1 {
            let (a, changed) = self.enc_list.update(input);
            ui::bar(0, self.enc_list.bar_y(), true, self.enc_list.pitch);
            if input.down & KEY_B != 0 {
                self.step = 0;
                self.draw();
                return true;
            }
            if a >= 0 {
                self.enc_ix = a - 1;
                self.step = 2;
                self.opt = 0;
                self.level = 0;
                self.draw();
            } else if changed {
                self.draw_list();
            }
        } else {
            if input.down & KEY_B != 0 {
                self.step = if self.egg { 0 } else { 1 };
                self.draw();
                return true;
            }
            let before = self.opt;
            let opts = if self.egg { EGG_OPTS } else { OPTS };
            if input.rep & KEY_UP != 0 {
                self.opt = (self.opt + opts - 1) % opts;
            }
            if input.rep & KEY_DOWN != 0 {
                self.opt = (self.opt + 1) % opts;
            }
            // Gen 5 PIDs aren't tied to a lead, so that row is skipped.
            if !self.egg && self.gen5() && self.opt == 4 {
                self.opt = if input.rep & KEY_UP != 0 { 3 } else { 5 };
            }
            let mut dir = if input.rep & KEY_RIGHT != 0 {
                1
            } else if input.rep & KEY_LEFT != 0 {
                -1
            } else {
                0
            };
            if input.tapped {
                let row = opt_at(input.tap_y);
                if row >= 0 && row < opts {
                    if row == self.opt && row < opts - 1 {
                        dir = 1;
                    }
                    self.opt = row;
                }
            }
            if dir != 0 {
                if self.egg {
                    self.change_egg(dir);
                } else {
                    self.change(dir);
                }
            }
            let go = (self.opt == opts - 1
                && (input.down & KEY_A != 0 || (input.tapped && opt_at(input.tap_y) == opts - 1)))
                || input.down & KEY_START != 0;
            if go
                && (if self.egg {
                    self.generate_egg()
                } else {
                    self.generate()
                })
            {
                return false;
            }
            if dir != 0 || self.opt != before {
                self.draw_options();
            }
            ui::bar(0, opt_bar_y(self.opt), true, ui::GRID_PITCH);
        }
        true
    }
}

impl AddWizard {
    fn load_species(&mut self) {
        self.species_count = if self.egg {
            ffi::egg_species(self.version, &mut self.species) as i32
        } else {
            ffi::wild_species(self.version, &mut self.species) as i32
        };
    }

    fn gen5(&self) -> bool {
        (20..=23).contains(&self.version)
    }

    /// The other game of a pair that shares a save type (the save doesn't record which it is).
    fn other_game(&self) -> u8 {
        match self.version {
            10 => 11,
            11 => 10,
            7 => 8,
            8 => 7,
            20 => 21,
            21 => 20,
            22 => 23,
            23 => 22,
            _ => 0,
        }
    }

    fn change(&mut self, dir: i32) {
        match self.opt {
            0 => self.nature = (self.nature + 1 + dir + 26) % 26 - 1,
            1 => self.shiny = !self.shiny,
            2 if self.enc_ix >= 0 => {
                let e = self.enc[self.enc_ix as usize];
                let (lo, hi) = (i32::from(e.level_min), i32::from(e.level_max));
                self.level = if self.level == 0 {
                    if dir > 0 { lo } else { hi }
                } else {
                    self.level + dir
                };
                if self.level > hi || self.level < lo {
                    self.level = 0;
                }
            }
            3 => self.gender = (self.gender + 1 + dir + 3) % 3 - 1,
            4 if !self.gen5() => self.lead = (self.lead + dir + 4) % 4,
            5 => {
                let n = BALLS.len() as i32 + 1;
                self.ball = (self.ball + dir + n) % n;
            }
            _ => {}
        }
    }

    fn change_egg(&mut self, dir: i32) {
        match self.opt {
            0 => self.nature = (self.nature + 1 + dir + 26) % 26 - 1,
            1 => self.gender = (self.gender + 1 + dir + 3) % 3 - 1,
            2 => self.shiny = !self.shiny,
            3 => self.hatched = !self.hatched,
            _ => {}
        }
    }

    fn loc(&self) -> Loc {
        Loc {
            box_index: self.box_index,
            slot: self.slot,
        }
    }

    /// After a successful insert: the save changed, and its party count may have.
    fn inserted(&self) {
        let mut d = data::doc();
        d.dirty = true;
        if let Some(info) = ffi::save_probe(&mut d.buf) {
            d.info = info;
        }
    }

    fn generate_egg(&mut self) -> bool {
        let mut r = EggRequest::zeroed();
        r.version = self.version;
        r.species = self.species[self.species_ix as usize];
        r.nature = if self.nature < 0 {
            0xFF
        } else {
            self.nature as u8
        };
        r.gender = if self.gender < 0 {
            0xFF
        } else {
            self.gender as u8
        };
        r.shiny = self.shiny.into();
        r.hatched = self.hatched.into();
        r.date = today();
        r.seed = seed(&EGG_COUNTER);
        let code = ffi::egg_insert(&mut data::doc().buf, &r, self.loc());
        if code == 0 {
            self.inserted();
            return true;
        }
        self.status = Text::from(ffi::gen_status_text(code)).cap(48);
        self.draw_options();
        false
    }

    fn generate(&mut self) -> bool {
        let enc = (self.enc_ix >= 0).then(|| self.enc[self.enc_ix as usize]);
        let mut r = GenRequest::zeroed();
        r.version = self.version;
        r.species = self.species[self.species_ix as usize];
        r.form = enc.map_or(0, |e| e.form);
        r.encounter_index = enc.map_or(0xFFFF, |e| e.index);
        r.level = if self.level != 0 {
            self.level as u8
        } else {
            0xFF
        };
        r.nature = if self.nature < 0 {
            0xFF
        } else {
            self.nature as u8
        };
        r.gender = if self.gender < 0 {
            0xFF
        } else {
            self.gender as u8
        };
        r.shiny = self.shiny.into();
        r.ball = if self.ball != 0 {
            BALLS[self.ball as usize - 1]
        } else {
            0xFF
        };
        r.lead = self.lead as u8;
        if self.lead == 1 {
            if self.nature < 0 {
                self.status = Text::from(b"Synchronize: choose a nature");
                self.draw_options();
                return false;
            }
            r.lead_param = self.nature as u8;
        }
        if self.lead == 2 {
            if self.gender < 0 {
                self.status = Text::from(b"Cute Charm: choose a gender");
                self.draw_options();
                return false;
            }
            r.lead_param = self.gender as u8;
        }
        r.met_date = today();
        r.seed = seed(&GEN_COUNTER);
        let code = ffi::gen_insert(&mut data::doc().buf, &r, self.loc());
        if code == 0 {
            self.inserted();
            return true;
        }
        self.status = Text::from(ffi::gen_status_text(code)).cap(48);
        self.draw_options();
        false
    }

    fn draw(&mut self) {
        ui::clear(top());
        ui::clear(bottom());
        ui::frame_clear(top());
        ui::frame_clear(bottom());
        if !self.supported {
            ui::title(top(), b"Add Pokemon");
            ui::panel(top(), 1, 2, 30, 3);
            let mut l = ui::Lines::new(2);
            ui::text(top(), ui::INSET_TOP, l.line(), b"Only Gen 4 saves for now");
            ui::text(
                top(),
                ui::INSET_TOP,
                l.line(),
                &t!(MUTED, "(D/P/Pt wild encounters)."),
            );
            ui::hints(bottom(), b"B:Back");
            return;
        }
        ui::title(top(), &t!("Add ", if self.egg { "Egg" } else { "Pokemon" }));
        ui::title_right(top(), &t!("legal, ", game_name(self.version)).cap(32));
        let x = ui::INSET_TOP;
        ui::panel(top(), 1, 2, 30, 3);
        let mut w = ui::Lines::new(2);
        let party = self.box_index == 0xFF;
        let n = if party {
            u32::from(self.slot)
        } else {
            u32::from(self.box_index) * 30 + u32::from(self.slot)
        };
        let to = if party { "party slot" } else { "box slot" };
        ui::text(
            top(),
            x,
            w.line(),
            &t!(
                "Step ",
                self.step + 1,
                " of 3  ",
                MUTED,
                "to ",
                to,
                " ",
                n + 1
            ),
        );
        ui::panel(top(), 1, 7, 30, 7);
        let mut l = ui::Lines::new(7);
        let species = || ui::name(NAME_SPECIES, self.species[self.species_ix as usize].into());
        if self.step == 0 {
            ui::text(
                top(),
                x,
                l.line(),
                if self.egg { b"Egg species" } else { b"Species" },
            );
            let what = if self.egg {
                "can be bred here"
            } else {
                "with wild slots"
            };
            ui::text(
                top(),
                x,
                l.line(),
                &t!(MUTED, self.species_count, " ", what),
            );
            l.block();
            if self.other_game() != 0 {
                ui::text(top(), x, l.line(), &t!("SELECT ", MUTED, "switches game"));
            }
            if !self.gen5() {
                let what = if self.egg {
                    "makes a wild encounter instead"
                } else {
                    "makes a bred egg instead"
                };
                ui::text(top(), x, l.line(), &t!("Y ", MUTED, what));
            }
            self.draw_list();
        } else if self.step == 1 {
            ui::text(top(), x, l.line(), b"Encounter");
            ui::text(top(), x, l.line(), &t!(MUTED, "for ", &species()));
            self.draw_list();
        } else {
            ui::text(top(), x, l.line(), b"Options");
            ui::text(top(), x, l.line(), &t!(MUTED, "for ", &species()));
            self.draw_options();
        }
        let hints: &[u8] = if self.step == 2 {
            b"A:Next|B:Back|START:Generate"
        } else {
            b"A:Next|L/R:Page|B:Back"
        };
        ui::hints(bottom(), hints);
    }

    fn draw_list(&mut self) {
        ui::clear_px(bottom(), 16, 176);
        ui::panel(bottom(), 0, 2, 32, 20);
        if self.step == 0 {
            ui::title(bottom(), b"Species");
            for r in 0..self.list.rows {
                let i = self.list.scroll + r;
                if i >= self.species_count {
                    break;
                }
                let y = self.list.text_y(i);
                let s = self.species[i as usize];
                ui::text(bottom(), LIST_X, y, &t!(MUTED, "#", Zero(s.into(), 3)));
                ui::text(bottom(), LIST_X + 44, y, &ui::name(NAME_SPECIES, s.into()));
            }
        } else {
            ui::title(bottom(), b"Encounter");
            for r in 0..self.enc_list.rows {
                let i = self.enc_list.scroll + r - 1;
                let y = self.enc_list.text_y(self.enc_list.scroll + r);
                if i == -1 {
                    ui::text(bottom(), LIST_X, y, b"Any wild slot");
                } else if i < self.enc_count {
                    let e = self.enc[i as usize];
                    let lv = if e.level_min == e.level_max {
                        t!(e.level_min)
                    } else {
                        t!(e.level_min, "-", e.level_max)
                    }
                    .cap(8);
                    let mut place =
                        ui::location(if self.gen5() { 5 } else { 4 }, e.location).cap(40);
                    ui::fit(&mut place, 140 - LIST_X - 4);
                    ui::text(bottom(), LIST_X, y, &place);
                    let lv_x = ui::text_right(bottom(), 244, y, &t!(MUTED, "Lv ", MUTED, &lv));
                    let kind = if self.gen5() {
                        slot_type5(e.slot_type)
                    } else {
                        slot_type(e.slot_type)
                    };
                    ui::text_in(bottom(), 140, lv_x - 6, y, &t!(MUTED, kind));
                }
            }
        }
    }

    fn draw_status(&self, rows: i32) {
        if self.status.is_empty() {
            return;
        }
        let row = 3 + (rows * ui::GRID_PITCH + 8) / 8 + 2;
        ui::panel(bottom(), 1, row, 30, 2);
        ui::text(bottom(), ui::INSET_TOP, row * 8 + 4, &self.status);
    }

    fn draw_egg_options(&self) {
        ui::title(bottom(), b"Options");
        ui::clear_px(bottom(), 16, 176);
        ui::panel(bottom(), 0, 2, 32, (5 * ui::GRID_PITCH + 8) / 8);
        opt(0, "Nature", &t!("< ", &nature_name(self.nature), " >"));
        opt(1, "Gender", &t!("< ", gender_name(self.gender), " >"));
        opt(2, "Shiny", &t!("< ", yes_no(self.shiny), " >"));
        opt(3, "Hatched", &t!("< ", yes_no(self.hatched), " >"));
        ui::text(bottom(), LIST_X, opt_y(4), b"Generate and place");
        self.draw_status(5);
    }

    fn draw_options(&self) {
        if self.egg {
            self.draw_egg_options();
            return;
        }
        ui::title(bottom(), b"Options");
        ui::clear_px(bottom(), 16, 176);
        ui::panel(bottom(), 0, 2, 32, (7 * ui::GRID_PITCH + 8) / 8);
        opt(0, "Nature", &t!("< ", &nature_name(self.nature), " >"));
        opt(1, "Shiny", &t!("< ", yes_no(self.shiny), " >"));
        if self.level != 0 {
            opt(2, "Level", &t!("< ", self.level, " >"));
        } else {
            opt(2, "Level", b"< Any >");
        }
        opt(3, "Gender", &t!("< ", gender_name(self.gender), " >"));
        if self.gen5() {
            opt(4, "Lead", &t!(MUTED, "not used in Gen 5"));
        } else {
            opt(4, "Lead", &t!("< ", lead_name(self.lead), " >"));
        }
        let ball = if self.ball != 0 {
            ui::name(NAME_ITEMS4, BALLS[self.ball as usize - 1].into())
        } else {
            Text::from(b"Default")
        };
        opt(5, "Ball", &t!("< ", &ball, " >"));
        ui::text(bottom(), LIST_X, opt_y(6), b"Generate and place");
        self.draw_status(7);
    }
}
