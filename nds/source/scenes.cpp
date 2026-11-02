// SPDX-License-Identifier: GPL-3.0-only
// P9.1 scenes, first slice: file picker, save overview, box view with party panel.
#include "scenes.h"

#include <dirent.h>
#include <stdio.h>
#include <string.h>
#include <strings.h>

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
        ui::clear(bottom());
        at(bottom(), 0, 0, " Save");
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
            ui::push(box_view());
        if (a == 2)
            return false;
        return true;
    }

private:
    static constexpr const char* kItems[3] = {"Boxes and party", "Legality (soon)", "Close"};
    ListView list_;
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
        at(bottom(), 0, 23, party_ ? " Select/tap here: boxes   B back"
                                   : " L/R box  Select: party  B back");
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
        at(top(), 0, 17, "Met Lv %u  loc %u  ball %u", v.met_level, v.met_location, v.ball);
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

}  // namespace

ui::Scene* file_picker() { return new FilePicker(); }
ui::Scene* overview() { return new Overview(); }
ui::Scene* box_view() { return new BoxView(); }

}  // namespace scenes
