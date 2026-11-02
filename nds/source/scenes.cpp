// SPDX-License-Identifier: GPL-3.0-only
// P9.1 scenes, first slice: file picker, save overview, box view with party panel.
#include "scenes.h"

#include <dirent.h>
#include <stdio.h>
#include <string.h>
#include <strings.h>
#include <time.h>

#include <initializer_list>

#include "core.h"

using ui::at;
using ui::bottom;
using ui::top;

namespace scenes {

namespace {

// ---- shared list widget --------------------------------------------------------------------

struct ListView
{
    int first_row = 2;  // console row of the first item
    int rows = 20;
    int selected = 0;
    int scroll = 0;
    int count = 0;
    int drag_scroll = 0;

    void clamp()
    {
        if (count == 0)
        {
            selected = scroll = 0;
            return;
        }
        if (selected < 0)
            selected = 0;
        if (selected >= count)
            selected = count - 1;
        if (selected < scroll)
            scroll = selected;
        if (selected >= scroll + rows)
            scroll = selected - rows + 1;
        int max_scroll = count > rows ? count - rows : 0;
        if (scroll > max_scroll)
            scroll = max_scroll;
        if (scroll < 0)
            scroll = 0;
    }

    // Returns the activated index (A, or a tap on the selected row), or -1.
    int update(const ui::Input& in, bool* changed)
    {
        int before_sel = selected, before_scroll = scroll;
        if (in.rep & KEY_UP)
            selected--;
        if (in.rep & KEY_DOWN)
            selected++;
        if (in.rep & KEY_L)
            selected -= rows;
        if (in.rep & KEY_R)
            selected += rows;
        int activated = -1;
        if ((in.down & KEY_A) && count > 0)
            activated = selected;
        if (in.down & KEY_TOUCH)
            drag_scroll = scroll;
        if (in.touching && in.dragging)
        {
            scroll = drag_scroll - in.drag_dy / 8;
            int max_scroll = count > rows ? count - rows : 0;
            if (scroll < 0)
                scroll = 0;
            if (scroll > max_scroll)
                scroll = max_scroll;
            if (selected < scroll)
                selected = scroll;
            if (selected >= scroll + rows)
                selected = scroll + rows - 1;
        }
        else
            clamp();
        if (in.tapped)
        {
            int row = in.tap_y / 8 - first_row;
            int i = scroll + row;
            if (row >= 0 && row < rows && i < count)
            {
                if (i == selected)
                    activated = i;
                selected = i;
            }
        }
        *changed = selected != before_sel || scroll != before_scroll;
        return activated;
    }

    int bar_y() const { return (first_row + selected - scroll) * 8; }
};

// ---- file picker ---------------------------------------------------------------------------

constexpr int kMaxFiles = 256;
constexpr int kPathLen = 192;

class FilePicker : public ui::Scene
{
public:
    void enter() override
    {
        if (!scanned_)
        {
            scanned_ = true;
            for (const char* root : {"sd:/", "fat:/"})
                scan(root, 3);
            list_.count = count_;
            char last[kPathLen];
            if (core::last(last, sizeof(last)))
                for (int i = 0; i < count_; i++)
                    if (strcmp(files_[i], last) == 0)
                        list_.selected = i;
            list_.clamp();
        }
        draw_top("");
        draw_list();
    }

    bool update(const ui::Input& in) override
    {
        bool changed;
        int open = list_.update(in, &changed);
        if (changed)
            draw_list();
        ui::bar(0, list_.bar_y(), count_ > 0);
        if (open >= 0)
        {
            const char* err = nullptr;
            if (core::open(files_[open], &err))
            {
                core::remember(files_[open]);
                core::load_data_for(core::doc().info.kind);
                ui::push(overview());
            }
            else
                draw_top(err);
        }
        return true;  // the picker is the root scene
    }

private:
    char files_[kMaxFiles][kPathLen];
    int count_ = 0;
    bool scanned_ = false;
    ListView list_;

    static bool is_save(const char* name)
    {
        const char* dot = strrchr(name, '.');
        return dot && (strcasecmp(dot, ".sav") == 0 || strcasecmp(dot, ".dsv") == 0);
    }

    void scan(const char* dir, int depth)
    {
        DIR* d = opendir(dir);
        if (!d)
            return;
        struct dirent* e;
        while ((e = readdir(d)) != nullptr && count_ < kMaxFiles)
        {
            if (e->d_name[0] == '.')
                continue;
            char path[kPathLen];
            int n = snprintf(path, sizeof(path), "%s%s%s", dir,
                             dir[strlen(dir) - 1] == '/' ? "" : "/", e->d_name);
            if (n <= 0 || n >= kPathLen)
                continue;
            if (e->d_type == DT_DIR)
            {
                if (depth > 0)
                    scan(path, depth - 1);
            }
            else if (is_save(e->d_name))
                strcpy(files_[count_++], path);
        }
        closedir(d);
    }

    void draw_top(const char* message)
    {
        ui::clear(top());
        at(top(), 0, 0, "verity");
        at(top(), 0, 1, "Gen 4/5 save editor");
        at(top(), 0, 3, "%d save file(s) found", count_);
        if (core::doc().loaded())
            at(top(), 0, 5, "Open: %.26s", strrchr(core::doc().path, '/') + 1);
        if (message[0])
            at(top(), 0, 8, "! %s", message);
        at(top(), 0, 21, "A / tap twice: open");
        at(top(), 0, 22, "Up/Down/L/R or drag: scroll");
    }

    void draw_list()
    {
        at(bottom(), 0, 0, " Saves");
        for (int r = 0; r < list_.rows; r++)
        {
            int i = list_.scroll + r;
            if (i < count_)
            {
                const char* slash = strrchr(files_[i], '/');
                at(bottom(), 0, list_.first_row + r, " %.30s", slash ? slash + 1 : files_[i]);
            }
            else
                at(bottom(), 0, list_.first_row + r, "%s", "");
        }
    }
};

// ---- overview ------------------------------------------------------------------------------

class Overview : public ui::Scene
{
public:
    void enter() override
    {
        const auto& d = core::doc();
        const auto& i = d.info;
        char ot[16];
        ui::utf16_to_ascii(i.ot_name, ot, sizeof(ot));
        ui::clear(top());
        at(top(), 0, 0, "%.32s", strrchr(d.path, '/') + 1);
        at(top(), 0, 2, "Game     %s", core::game_name(i.kind));
        at(top(), 0, 3, "Trainer  %s", ot);
        at(top(), 0, 4, "ID       %05u / %05u", i.tid, i.sid);
        at(top(), 0, 5, "Played   %u:%02u:%02u", i.played_hours, i.played_minutes,
           i.played_seconds);
        at(top(), 0, 6, "Party    %u", i.party_count);
        at(top(), 0, 7, "Boxes    %lu", (unsigned long)d.boxes);
        at(top(), 0, 8, "Checksum %s", i.checksums_valid ? "OK" : "BAD");
        if (!core::data_available())
            at(top(), 0, 10, "(data missing: showing IDs)");
        if (!message_[0] && d.dirty)
            snprintf(message_, sizeof(message_), "unsaved changes");
        if (message_[0])
            at(top(), 0, 12, "%s %s", d.dirty ? "*" : "-", message_);
        ui::clear(bottom());
        at(bottom(), 0, 0, " Save%s", d.dirty ? " *" : "");
        for (int r = 0; r < 3; r++)
            at(bottom(), 0, list_.first_row + r, " %s", kItems[r]);
        list_.count = 3;
        list_.rows = 3;
    }

    bool update(const ui::Input& in) override
    {
        bool changed;
        int a = list_.update(in, &changed);
        ui::bar(0, list_.bar_y(), true);
        if (in.down & KEY_B)
            return false;
        if (a == 0)
        {
            message_[0] = 0;
            ui::push(box_view());
        }
        if (a == 1)
        {
            const char* m = "";
            core::save_to_disk(&m);
            snprintf(message_, sizeof(message_), "%s", m);
            enter();
        }
        if (a == 2)
            return false;
        return true;
    }

private:
    static constexpr const char* kItems[3] = {"Boxes and party", "Save to SD card", "Close"};
    ListView list_;
    char message_[48] = "";
};

// ---- box view ------------------------------------------------------------------------------

class BoxView : public ui::Scene
{
public:
    void enter() override
    {
        ui::clear(bottom());
        dirty_ = true;
    }

    bool update(const ui::Input& in) override
    {
        if (in.down & KEY_B)
            return false;
        int boxes = (int)core::doc().boxes;
        int n = party_ ? 6 : 30;
        int cols = party_ ? 2 : 6;
        int before_box = box_, before_cur = cur_;
        bool before_party = party_;

        if (in.rep & KEY_LEFT)
            cur_ = cur_ % cols == 0 ? cur_ + cols - 1 : cur_ - 1;
        if (in.rep & KEY_RIGHT)
            cur_ = cur_ % cols == cols - 1 ? cur_ - cols + 1 : cur_ + 1;
        if (in.rep & KEY_UP)
            cur_ = (cur_ - cols + n) % n;
        if (in.rep & KEY_DOWN)
            cur_ = (cur_ + cols) % n;
        if (!party_ && (in.rep & KEY_L))
            box_ = (box_ + boxes - 1) % boxes;
        if (!party_ && (in.rep & KEY_R))
            box_ = (box_ + 1) % boxes;
        if (in.down & KEY_SELECT)
        {
            party_ = !party_;
            cur_ = 0;
        }
        if (in.tapped)
            tap(in.tap_x, in.tap_y, boxes);
        if ((in.down & KEY_X) || ((in.down & KEY_A) && at_cell(cur_).species == 0))
        {
            int target = at_cell(cur_).species == 0 ? cur_ : -1;
            for (int i = 0; i < n && target < 0; i++)
                if (at_cell(i).species == 0)
                    target = i;
            if (target >= 0 && !(party_ && target > (int)core::doc().info.party_count))
            {
                ui::push(add_wizard(party_ ? 0xFF : box_, party_ ? core::doc().info.party_count
                                                                  : target));
                return true;
            }
        }

        if (box_ != before_box || cur_ != before_cur || party_ != before_party || dirty_)
        {
            draw();
            dirty_ = false;
        }
        return true;
    }

private:
    int box_ = 0, cur_ = 0;
    bool party_ = false, dirty_ = true;

    void cell_pos(int i, int* x, int* y) const
    {
        if (party_)
        {
            *x = 56 + (i % 2) * 112;
            *y = 24 + (i / 2) * 48;
        }
        else
        {
            *x = 12 + (i % 6) * 40;
            *y = 20 + (i / 6) * 32;
        }
    }

    void tap(int x, int y, int boxes)
    {
        if (y < 16)
        {
            if (!party_ && x < 64)
                box_ = (box_ + boxes - 1) % boxes;
            else if (!party_ && x >= 192)
                box_ = (box_ + 1) % boxes;
            return;
        }
        if (y >= 176)
        {
            party_ = !party_;
            cur_ = 0;
            return;
        }
        int n = party_ ? 6 : 30;
        for (int i = 0; i < n; i++)
        {
            int cx, cy;
            cell_pos(i, &cx, &cy);
            if (x >= cx - 4 && x < cx + 36 && y >= cy && y < cy + 32)
                cur_ = i;
        }
    }

    verity_pkm_view at_cell(int i) const
    {
        return party_ ? core::slot(0xFF, i) : core::slot(box_, i);
    }

    void draw()
    {
        char name[24];
        if (party_)
        {
            at(bottom(), 0, 0, "           Party");
            at(bottom(), 0, 1, "%s", "");
        }
        else
        {
            core::box_name(box_, name, sizeof(name));
            int pad = (32 - (int)strlen(name)) / 2 - 2;
            at(bottom(), 0, 0, " <%*s%s%*s> ", pad > 0 ? pad : 0, "", name,
               pad > 0 ? pad : 0, "");
            at(bottom(), 26, 1, "%2d/%-2d", box_ + 1, (int)core::doc().boxes);
        }
        at(bottom(), 0, 23, party_ ? " Select/tap: boxes  X add  B back"
                                   : " L/R box Select party X add B");
        ui::sprites_hide_all();
        int n = party_ ? 6 : 30;
        for (int i = 0; i < n; i++)
        {
            verity_pkm_view v = at_cell(i);
            int x, y;
            cell_pos(i, &x, &y);
            if (v.species != 0)
                ui::icon(2 + 2 * i, x, y, v.types[0] == 0xFF ? 17 : v.types[0], v.is_egg,
                         v.is_shiny);
            if (i == cur_)
                ui::cursor(0, x, y);
        }
        details(at_cell(cur_));
    }

    void details(const verity_pkm_view& v)
    {
        ui::clear(top());
        if (v.species == 0)
        {
            at(top(), 0, 0, "(empty)");
            return;
        }
        char nick[16], ot[12];
        ui::utf16_to_ascii(v.nickname, nick, sizeof(nick));
        ui::utf16_to_ascii(v.ot_name, ot, sizeof(ot));
        const char* sp = ui::name(VERITY_NAME_SPECIES, v.species);
        const char* g = v.gender == 0 ? "M" : v.gender == 1 ? "F" : "-";
        if (v.is_egg)
        {
            at(top(), 0, 0, "Egg (%s)", sp);
            at(top(), 0, 2, "OT %s  %05u/%05u", ot, v.tid, v.sid);
            return;
        }
        at(top(), 0, 0, "%s%s", nick, v.is_shiny ? " *" : "");
        at(top(), 0, 1, "#%03u %s%s  %s  Lv %u", v.species, sp, v.form ? "-f" : "", g, v.level);
        if (v.form)
            at(top(), 0, 1, "#%03u %s (form %u)  %s  Lv %u", v.species, sp, v.form, g, v.level);
        at(top(), 0, 3, "Nature  %s", ui::name(VERITY_NAME_NATURES, v.nature));
        at(top(), 0, 4, "Ability %s", ui::name(VERITY_NAME_ABILITIES, v.ability));
        uint32_t items = core::doc().info.generation == 4 ? VERITY_NAME_ITEMS4 : VERITY_NAME_ITEMS5;
        at(top(), 0, 5, "Item    %s", v.held_item ? ui::name(items, v.held_item) : "-");
        for (int m = 0; m < 4; m++)
            at(top(), 0, 7 + m, "  %s", v.moves[m] ? ui::name(VERITY_NAME_MOVES, v.moves[m]) : "-");
        at(top(), 0, 12, "      HP  At  Df  Sp  SA  SD");
        at(top(), 0, 13, "IVs %4u%4u%4u%4u%4u%4u", v.ivs[0], v.ivs[1], v.ivs[2], v.ivs[3],
           v.ivs[4], v.ivs[5]);
        at(top(), 0, 14, "EVs %4u%4u%4u%4u%4u%4u", v.evs[0], v.evs[1], v.evs[2], v.evs[3],
           v.evs[4], v.evs[5]);
        at(top(), 0, 16, "OT %s  %05u/%05u", ot, v.tid, v.sid);
        at(top(), 0, 17, "Met %.20s Lv %u", ui::location(core::doc().info.generation, v.met_location),
           v.met_level);
        at(top(), 0, 18, "PID %08lX", (unsigned long)v.pid);
        if (!v.checksum_ok)
            at(top(), 0, 19, "! checksum mismatch");
        legality_line();
    }

    void legality_line()
    {
        uint32_t ms = 0;
        verity_legality_report r = party_ ? core::legality(0xFF, cur_, &ms)
                                          : core::legality(box_, cur_, &ms);
        static const char* why[] = {"",        "egg",       "origin game",
                                    "data missing", "Gen 5 save", "hatched",
                                    "gift/static/trade"};
        if (r.status == 1)
            at(top(), 0, 21, "LEGAL  (wild slot, %lu ms)", (unsigned long)ms);
        else if (r.status == 2)
        {
            char list[40] = "";
            for (int i = 0; i < r.check_count && strlen(list) < 24; i++)
            {
                strcat(list, i ? " " : "");
                strcat(list, core::check_name(r.checks[i]));
            }
            bool moves = !(r.move_ok[0] && r.move_ok[1] && r.move_ok[2] && r.move_ok[3]);
            at(top(), 0, 21, "ILLEGAL (%lu ms)", (unsigned long)ms);
            at(top(), 0, 22, " %s%s", list, moves ? " Moves" : "");
        }
        else
        {
            at(top(), 0, 21, "Not checked yet:");
            at(top(), 0, 22, " %s", r.unsupported < 7 ? why[r.unsupported] : "?");
        }
    }
};


// ---- add-Pokémon wizard --------------------------------------------------------------------

class AddWizard : public ui::Scene
{
public:
    AddWizard(uint8_t box_index, uint8_t slot) : box_(box_index), slot_(slot) {}

    void enter() override
    {
        if (!loaded_)
        {
            loaded_ = true;
            version_ = core::wizard_version();
            supported_ = core::doc().info.generation == 4 && core::ensure_gen4_data();
            if (supported_)
                species_count_ = (int)verity_wild_species(version_, species_, 494);
        }
        draw();
    }

    bool update(const ui::Input& in) override
    {
        if (!supported_)
            return !(in.down & (KEY_B | KEY_A));
        bool changed = false;
        if (step_ == 0)
        {
            list_.count = species_count_;
            int a = list_.update(in, &changed);
            ui::bar(0, list_.bar_y(), true);
            if (in.down & KEY_B)
                return false;
            if (a >= 0)
            {
                species_ix_ = a;
                enc_count_ = (int)verity_wild_encounters(version_, species_[a], enc_, 64);
                step_ = 1;
                enc_list_ = ListView();
                enc_list_.count = enc_count_ + 1;  // row 0 is "any slot"
                draw();
            }
            else if (changed)
                draw_list();
        }
        else if (step_ == 1)
        {
            int a = enc_list_.update(in, &changed);
            ui::bar(0, enc_list_.bar_y(), true);
            if (in.down & KEY_B)
            {
                step_ = 0;
                draw();
                return true;
            }
            if (a >= 0)
            {
                enc_ix_ = a - 1;
                step_ = 2;
                opt_ = 0;
                level_ = 0;
                draw();
            }
            else if (changed)
                draw_list();
        }
        else
        {
            if (in.down & KEY_B)
            {
                step_ = 1;
                draw();
                return true;
            }
            int before = opt_;
            if (in.rep & KEY_UP)
                opt_ = (opt_ + kOpts - 1) % kOpts;
            if (in.rep & KEY_DOWN)
                opt_ = (opt_ + 1) % kOpts;
            int dir = (in.rep & KEY_RIGHT) ? 1 : (in.rep & KEY_LEFT) ? -1 : 0;
            if (in.tapped)
            {
                int row = in.tap_y / 8 - 2;
                if (row >= 0 && row < kOpts)
                {
                    if (row == opt_ && row < kOpts - 1)
                        dir = 1;
                    opt_ = row;
                }
            }
            if (dir)
                change(dir);
            bool go = (opt_ == kOpts - 1 &&
                       ((in.down & KEY_A) || (in.tapped && in.tap_y / 8 - 2 == kOpts - 1))) ||
                      (in.down & KEY_START);
            if (go && generate())
                return false;
            if (dir || opt_ != before)
                draw_options();
            ui::bar(0, (2 + opt_) * 8, true);
        }
        return true;
    }

private:
    uint8_t box_, slot_;
    bool loaded_ = false, supported_ = false;
    uint8_t version_ = 12;
    uint16_t species_[494];
    int species_count_ = 0, species_ix_ = 0;
    verity_encounter enc_[64];
    int enc_count_ = 0, enc_ix_ = -1;
    int step_ = 0, opt_ = 0;
    int nature_ = -1;  // -1 any
    bool shiny_ = false;
    int level_ = 0;  // 0 any, else the level
    int gender_ = -1;  // -1 any, 0 male, 1 female
    int lead_ = 0;     // 0 none, 1 Synchronize, 2 Cute Charm, 3 Poke Radar chain
    int ball_ = 0;     // 0 default, else a ball ID from kBalls
    static constexpr int kOpts = 7;
    static constexpr uint8_t kBalls[] = {4, 3, 2, 1, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15};
    char status_[48] = "";
    ListView list_, enc_list_;

    static const char* slot_type(uint8_t t)
    {
        static const char* names[] = {"Grass",   "Surf",     "Old Rod", "Good Rod", "Super Rod",
                                      "Rock",    "Headbutt", "Headbutt", "Contest", "Honey",
                                      "Safari",  "Safari",   "Saf rod", "Saf rod", "Saf rod"};
        return t < 15 ? names[t] : "?";
    }

    void change(int dir)
    {
        if (opt_ == 0)
            nature_ = (nature_ + 1 + dir + 26) % 26 - 1;
        else if (opt_ == 1)
            shiny_ = !shiny_;
        else if (opt_ == 2 && enc_ix_ >= 0)
        {
            const auto& e = enc_[enc_ix_];
            int lo = e.level_min, hi = e.level_max;
            level_ = level_ == 0 ? (dir > 0 ? lo : hi) : level_ + dir;
            if (level_ > hi || level_ < lo)
                level_ = 0;
        }
        else if (opt_ == 3)
            gender_ = (gender_ + 1 + dir + 3) % 3 - 1;
        else if (opt_ == 4)
            lead_ = (lead_ + dir + 4) % 4;
        else if (opt_ == 5)
        {
            int n = (int)(sizeof(kBalls) / sizeof(kBalls[0])) + 1;
            ball_ = (ball_ + dir + n) % n;
        }
    }

    static const char* lead_name(int l)
    {
        static const char* names[] = {"None", "Synchronize", "Cute Charm", "Poke Radar"};
        return names[l];
    }

    bool generate()
    {
        time_t now = time(nullptr);
        struct tm* t = localtime(&now);
        static uint32_t counter = 0;
        verity_gen_request r;
        memset(&r, 0, sizeof(r));
        r.version = version_;
        r.species = species_[species_ix_];
        r.form = enc_ix_ >= 0 ? enc_[enc_ix_].form : 0;
        r.encounter_index = enc_ix_ >= 0 ? enc_[enc_ix_].index : 0xFFFF;
        r.level = level_ ? (uint8_t)level_ : 0xFF;
        r.nature = nature_ < 0 ? 0xFF : (uint8_t)nature_;
        r.gender = gender_ < 0 ? 0xFF : (uint8_t)gender_;
        r.shiny = shiny_;
        r.ball = ball_ ? kBalls[ball_ - 1] : 0xFF;
        r.lead = (uint8_t)lead_;
        if (lead_ == 1)
        {
            if (nature_ < 0)
            {
                snprintf(status_, sizeof(status_), "Synchronize: choose a nature");
                draw_options();
                return false;
            }
            r.lead_param = (uint8_t)nature_;
        }
        if (lead_ == 2)
        {
            if (gender_ < 0)
            {
                snprintf(status_, sizeof(status_), "Cute Charm: choose a gender");
                draw_options();
                return false;
            }
            r.lead_param = (uint8_t)gender_;
        }
        r.met_date[0] = t ? (uint8_t)(t->tm_year - 100) : 26;
        r.met_date[1] = t ? (uint8_t)(t->tm_mon + 1) : 1;
        r.met_date[2] = t ? (uint8_t)t->tm_mday : 1;
        r.seed = (uint32_t)now * 2654435761u + ++counter;
        uint8_t code = verity_gen_insert(core::doc().buf, core::doc().len, &r,
                                         verity_loc{box_, slot_});
        if (code == 0)
        {
            core::doc().dirty = true;
            verity_save_probe(core::doc().buf, core::doc().len, &core::doc().info);
            return true;
        }
        snprintf(status_, sizeof(status_), "%s", verity_gen_status_text(code));
        draw_options();
        return false;
    }

    void draw()
    {
        ui::clear(top());
        ui::clear(bottom());
        if (!supported_)
        {
            at(top(), 0, 0, "Add Pokemon");
            at(top(), 0, 2, "Only Gen 4 saves for now");
            at(top(), 0, 3, "(D/P/Pt wild encounters).");
            at(bottom(), 0, 0, " B: back");
            return;
        }
        at(top(), 0, 0, "Add Pokemon  (legal, %s)", version_ == 10 ? "Diamond" : "Platinum");
        at(top(), 0, 1, "to %s %u", box_ == 0xFF ? "party slot" : "box slot",
           (box_ == 0xFF ? slot_ : box_ * 30 + slot_) + 1);
        if (step_ == 0)
        {
            at(top(), 0, 3, "1. Species (%d with wild slots)", species_count_);
            draw_list();
        }
        else if (step_ == 1)
        {
            at(top(), 0, 3, "2. Encounter: %s", ui::name(VERITY_NAME_SPECIES, species_[species_ix_]));
            draw_list();
        }
        else
        {
            at(top(), 0, 3, "3. Options: %s", ui::name(VERITY_NAME_SPECIES, species_[species_ix_]));
            draw_options();
        }
        at(top(), 0, 22, "A next  B back  Start generate");
    }

    void draw_list()
    {
        if (step_ == 0)
        {
            at(bottom(), 0, 0, " Species");
            for (int r = 0; r < list_.rows; r++)
            {
                int i = list_.scroll + r;
                if (i < species_count_)
                    at(bottom(), 0, 2 + r, " #%03u %s", species_[i], ui::name(VERITY_NAME_SPECIES, species_[i]));
                else
                    at(bottom(), 0, 2 + r, "%s", "");
            }
        }
        else
        {
            at(bottom(), 0, 0, " Encounter");
            for (int r = 0; r < enc_list_.rows; r++)
            {
                int i = enc_list_.scroll + r - 1;
                if (i == -1)
                    at(bottom(), 0, 2 + r, " Any wild slot");
                else if (i < enc_count_)
                {
                    const auto& e = enc_[i];
                    char lv[8];
                    if (e.level_min == e.level_max)
                        snprintf(lv, sizeof(lv), "%u", e.level_min);
                    else
                        snprintf(lv, sizeof(lv), "%u-%u", e.level_min, e.level_max);
                    at(bottom(), 0, 2 + r, " %-15.15s %-8.8s %s", ui::location(4, e.location),
                       slot_type(e.slot_type), lv);
                }
                else
                    at(bottom(), 0, 2 + r, "%s", "");
            }
        }
    }

    void draw_options()
    {
        at(bottom(), 0, 0, " Options");
        at(bottom(), 0, 2, " Nature  < %s >", nature_ < 0 ? "Any" : ui::name(VERITY_NAME_NATURES, nature_));
        at(bottom(), 0, 3, " Shiny   < %s >", shiny_ ? "Yes" : "No");
        if (level_)
            at(bottom(), 0, 4, " Level   < %d >", level_);
        else
            at(bottom(), 0, 4, " Level   < Any >");
        at(bottom(), 0, 5, " Gender  < %s >", gender_ < 0 ? "Any" : gender_ ? "Female" : "Male");
        at(bottom(), 0, 6, " Lead    < %s >", lead_name(lead_));
        at(bottom(), 0, 7, " Ball    < %s >",
           ball_ ? ui::name(VERITY_NAME_ITEMS4, kBalls[ball_ - 1]) : "Default");
        at(bottom(), 0, 8, " Generate and place");
        at(bottom(), 0, 10, " %s", status_);
    }
};

}  // namespace

ui::Scene* file_picker() { return new FilePicker(); }
ui::Scene* overview() { return new Overview(); }
ui::Scene* box_view() { return new BoxView(); }
ui::Scene* add_wizard(uint8_t b, uint8_t s) { return new AddWizard(b, s); }

}  // namespace scenes
