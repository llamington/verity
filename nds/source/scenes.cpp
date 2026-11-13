// SPDX-License-Identifier: GPL-3.0-only
// P9.1 scenes, first slice: file picker, save overview, box view with party panel.
#include "scenes.h"

#include <dirent.h>
#include <stdarg.h>
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
    // Line pitch in px (8: one console row per item). On a wider pitch item i's text is at
    // text_y(i) and the highlight is `pitch` tall from text_y - 2.
    int pitch = 8;
    int pad = 0;  // px from the top of first_row to the first item's text
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
            scroll = drag_scroll - in.drag_dy / pitch;
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
            int off = in.tap_y - first_row * 8 - pad + (pitch - 8) / 2;
            int row = pitch == 8 ? in.tap_y / 8 - first_row : off < 0 ? -1 : off / pitch;
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

    int bar_y() const
    {
        // Centred on the 8 px text.
        return pitch == 8 ? (first_row + selected - scroll) * 8 : text_y(selected) - (pitch - 8) / 2;
    }
    int text_y(int i) const { return first_row * 8 + pad + (i - scroll) * pitch; }
};

// A list on a wider pitch in a bottom panel whose content spans `cells` rows from first_row (the
// whole screen between the title and hint bars by default). Short menus use kGridPitch.
ListView spaced_list(int pitch = ui::kPitch, int first_row = 2, int cells = 20)
{
    ListView v;
    v.pitch = pitch;
    v.pad = 4;
    v.first_row = first_row;
    v.rows = (cells * 8 - 2 * v.pad - 8) / pitch + 1;
    return v;
}

// Text x on a full-width bottom panel.
constexpr int kListX = ui::kInsetBottom;

// ---- file picker ---------------------------------------------------------------------------

constexpr int kMaxFiles = 256;
constexpr int kPathLen = 192;

static bool is_save(const char* name)
{
    const char* dot = strrchr(name, '.');
    return dot && (strcasecmp(dot, ".sav") == 0 || strcasecmp(dot, ".dsv") == 0);
}

// Adds the save files under dir to files; returns the new count.
static int scan_saves(char (*files)[kPathLen], int count, const char* dir, int depth)
{
    DIR* d = opendir(dir);
    if (!d)
        return count;
    struct dirent* e;
    while ((e = readdir(d)) != nullptr && count < kMaxFiles)
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
                count = scan_saves(files, count, path, depth - 1);
        }
        else if (is_save(e->d_name))
            strcpy(files[count++], path);
    }
    closedir(d);
    return count;
}

class FilePicker : public ui::Scene
{
public:
    const char* name() const override { return "files"; }
    void enter() override
    {
        if (!scanned_)
        {
            scanned_ = true;
            for (const char* root : {"sd:/", "fat:/"})
                count_ = scan_saves(files_, count_, root, 3);
            list_.count = count_;
            char last[kPathLen];
            if (core::last(last, sizeof(last)))
                for (int i = 0; i < count_; i++)
                    if (strcmp(files_[i], last) == 0)
                        list_.selected = i;
            list_.clamp();
        }
        framed_ = false;
        draw_top("");
        draw_list();
    }

    bool update(const ui::Input& in) override
    {
        bool changed;
        int open = list_.update(in, &changed);
        if (changed)
            draw_list();
        ui::bar(0, list_.bar_y(), count_ > 0, list_.pitch);
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
    bool framed_ = false;
    ListView list_ = spaced_list();

    void draw_top(const char* message)
    {
        ui::clear(top());
        ui::frame_clear(top());
        ui::title(top(), "Verity");
        ui::title_right(top(), "Gen 4/5 save editor");
        ui::panel(top(), 1, 2, 30, message[0] ? 6 : 3);
        // The logo (48 px inside its 64 px sprite) centred in the gap between the panels: from
        // below this one's border to the bottom panel's border at row 17.
        int gap_top = (message[0] ? 9 : 6) * 8, gap_bottom = 17 * 8;
        ui::logo(128 - 32, (gap_top + gap_bottom) / 2 - 32);
        ui::Lines l(2);
        ui::text(top(), ui::kInsetTop, l.line(), "%d save file%s found", count_, count_ == 1 ? "" : "s");
        if (core::doc().loaded())
            ui::text(top(), ui::kInsetTop, l.line(), UI_MUTED "Open " UI_MUTED " %.26s", strrchr(core::doc().path, '/') + 1);
        if (message[0])
        {
            l.block();
            ui::text(top(), ui::kInsetTop, l.line(), "! %s", message);
        }
        ui::panel(top(), 1, 18, 30, 3);
        ui::Lines r(18);
        if (ui::has_rom_font())
        {
            ui::text(top(), ui::kInsetTop, r.line(), "Names and icons come from");
            ui::text(top(), ui::kInsetTop, r.line(), "your ROM " UI_MUTED "(%s)", core::font_game());
        }
        else
        {
            ui::text(top(), ui::kInsetTop, r.line(), "Put a Gen 4/5 ROM (.nds) on the SD");
            ui::text(top(), ui::kInsetTop, r.line(), "card for JP/KR names and icons.");
        }
    }

    void draw_list()
    {
        if (!framed_)
        {
            framed_ = true;
            ui::frame_clear(bottom());
            ui::panel(bottom(), 0, list_.first_row, 32, 20);
            ui::hints(bottom(), "A:Open|L/R:Page|Drag:Scroll");
        }
        ui::title(bottom(), "Choose a save");
        char n[16];
        snprintf(n, sizeof(n), "%d", count_);
        ui::title_right(bottom(), n);
        ui::clear_px(bottom(), 16, 176);
        for (int r = 0; r < list_.rows; r++)
        {
            int i = list_.scroll + r;
            if (i < count_)
            {
                // File name, then its folder as a muted label at the right.
                const char* slash = strrchr(files_[i], '/');
                const char* name = slash ? slash + 1 : files_[i];
                char dir[24] = "";
                if (slash && slash > files_[i])
                {
                    const char* d = slash - 1;
                    while (d > files_[i] && *d != '/' && *d != ':')
                        d--;
                    snprintf(dir, sizeof(dir), "%.*s", (int)(slash - d - 1), d + 1);
                }
                int y = list_.text_y(i);
                char shown[kPathLen + 4];
                snprintf(shown, kPathLen, "%s", name);
                ui::fit(shown, 244 - ui::text_width(dir) - 10 - kListX);
                ui::text(bottom(), kListX, y, "%s", shown);
                ui::text_right(bottom(), 244, y, UI_MUTED "%s", dir);
            }
        }
    }
};

// ---- overview ------------------------------------------------------------------------------

class Overview : public ui::Scene
{
public:
    const char* name() const override { return "overview"; }
    void enter() override
    {
        const auto& d = core::doc();
        const auto& i = d.info;
        char ot[32];
        ui::utf16_to_text(i.ot_name, ot, sizeof(ot));
        ui::clear(top());
        ui::frame_clear(top());
        ui::title(top(), "%.30s", strrchr(d.path, '/') + 1);
        // Trainer, then the save's numbers, then its checksum state.
        ui::panel(top(), 1, 2, 30, 12);
        const int x = ui::kInsetTop;
        ui::Lines l(2);
        ui::text(top(), x, l.line(), "%s", ot);
        ui::text(top(), x, l.line(), UI_MUTED "%s", core::game_name(i.kind));
        l.block();
        ui::text(top(), x, l.line(), UI_MUTED "ID" UI_MUTED "  %05u / %05u", i.tid, i.sid);
        ui::text(top(), x, l.line(), UI_MUTED "Played" UI_MUTED "  %u:%02u:%02u", i.played_hours,
                 i.played_minutes, i.played_seconds);
        ui::text(top(), x, l.line(), UI_MUTED "Party" UI_MUTED "  %u     " UI_MUTED "Boxes" UI_MUTED "  %lu",
                 i.party_count, (unsigned long)d.boxes);
        l.block();
        // Unsaved edits leave the checksums in memory stale; saving recomputes them, so show the
        // file's state and say they'll be updated.
        ui::Row sums(top(), l.grid_row(), x);
        sums.text(UI_MUTED "Checksums");
        if (d.dirty && d.file_checksums_ok)
            sums.badge(ui::Badge::Safe, "updated on save");
        else if (d.dirty)
            sums.badge(ui::Badge::Invalid, "BAD in file, fixed on save");
        else
            sums.badge(i.checksums_valid ? ui::Badge::Legal : ui::Badge::Invalid,
                       i.checksums_valid ? "OK" : "BAD");
        sums.draw();
        if (!message_[0] && d.dirty)
            snprintf(message_, sizeof(message_), "Unsaved changes");
        if (message_[0] || !core::data_available())
        {
            ui::panel(top(), 1, 17, 30, message_[0] && !core::data_available() ? 4 : 2);
            ui::Lines m(17);
            if (message_[0])
                ui::text(top(), x, m.line() + 2, "%s %s", d.dirty ? "*" : "-", message_);
            if (!core::data_available())
                ui::text(top(), x, m.line() + 2, UI_MUTED "Game data missing: showing IDs");
        }
        ui::clear(bottom());
        ui::frame_clear(bottom());
        ui::title(bottom(), "Save");
        if (d.dirty)
            ui::title_right(bottom(), "* unsaved");
        count_ = 0;
        items_[count_++] = kBoxes;
        items_[count_++] = kBag;
        items_[count_++] = kGifts;
        if (i.kind == kBlack2White2)
            items_[count_++] = kUnova;
        items_[count_++] = kSave;
        items_[count_++] = kClose;
        int selected = list_.selected;
        list_ = spaced_list(ui::kGridPitch, 2, (count_ * ui::kGridPitch + 8) / 8);
        list_.count = count_;
        list_.selected = selected;
        list_.clamp();
        ui::panel(bottom(), 0, list_.first_row, 32, (count_ * ui::kGridPitch + 8) / 8);
        for (int r = 0; r < count_; r++)
            ui::text(bottom(), kListX, list_.text_y(r), "%s", kItems[items_[r]]);
        ui::hints(bottom(), confirm_leave_ ? "A:Open|B:Discard edits" : "A:Open|B:Files");
    }

    bool update(const ui::Input& in) override
    {
        bool changed;
        int a = list_.update(in, &changed);
        ui::bar(0, list_.bar_y(), true, list_.pitch);
        int item = a >= 0 ? items_[a] : -1;
        // Leaving drops unsaved edits (the file is read again when opened), so the first B or
        // Close only warns; anything else cancels the warning.
        if ((in.down & KEY_B) || item == kClose)
        {
            if (!core::doc().dirty || confirm_leave_)
                return false;
            confirm_leave_ = true;
            snprintf(message_, sizeof(message_), "Not saved. B again to discard.");
            enter();
            return true;
        }
        if (confirm_leave_ && (in.down || changed))
        {
            confirm_leave_ = false;
            message_[0] = 0;
            enter();
        }
        if (item == kBoxes)
        {
            message_[0] = 0;
            ui::push(box_view());
        }
        if (item == kBag)
        {
            message_[0] = 0;
            ui::push(bag());
        }
        if (item == kGifts)
        {
            message_[0] = 0;
            ui::push(gifts());
        }
        if (item == kUnova)
        {
            message_[0] = 0;
            ui::push(unova_link());
        }
        if (item == kSave)
        {
            const char* m = "";
            core::save_to_disk(&m);
            snprintf(message_, sizeof(message_), "%s", m);
            enter();
        }
        return true;
    }

private:
    bool confirm_leave_ = false;
    enum { kBoxes, kGifts, kSave, kClose, kUnova, kBag };
    static constexpr uint8_t kBlack2White2 = 5;
    static constexpr const char* kItems[6] = {"Boxes and party", "Mystery gifts", "Save to SD card",
                                              "Close", "Memory Link and keys", "Bag"};
    int items_[6];
    int count_ = 0;
    ListView list_;
    char message_[48] = "";
};

// ---- Mystery Gifts --------------------------------------------------------------------------

class Gifts : public ui::Scene
{
public:
    const char* name() const override { return "gifts"; }
    void enter() override
    {
        ui::clear(bottom());
        ui::title(bottom(), "Mystery gifts");
        ui::hints(bottom(), "A:Deliver|L/R:Page|B:Back");
        if (!core::ensure_events_data())
        {
            ui::clear(top());
            ui::title(top(), "Mystery gifts");
            ui::panel(top(), 1, 2, 30, 3);
            ui::Lines l(2);
            ui::text(top(), ui::kInsetTop, l.line(), "Gift data is missing.");
            ui::text(top(), ui::kInsetTop, l.line(), UI_MUTED "B goes back.");
            return;
        }
        ui::panel(bottom(), 0, list_.first_row, 32, 20);
        refresh();
        draw_list();
        draw_top();
    }

    bool update(const ui::Input& in) override
    {
        if (in.down & KEY_B)
            return false;
        bool changed;
        int a = list_.update(in, &changed);
        if (changed)
        {
            message_[0] = 0;
            draw_list();
            draw_top();
        }
        ui::bar(0, list_.bar_y(), count_ > 0, list_.pitch);
        if (a >= 0)
            deliver(a);
        return true;
    }

private:
    static constexpr int kMaxEvents = 500;
    verity_event_view events_[kMaxEvents];
    int count_ = 0;
    ListView list_ = spaced_list();
    char message_[40] = "";

    void refresh()
    {
        auto& d = core::doc();
        count_ = (int)verity_events_list(d.buf, d.len, events_, kMaxEvents);
        list_.count = count_;
        list_.clamp();
    }

    void deliver(int i)
    {
        auto& d = core::doc();
        const auto& e = events_[i];
        // The card stores the date it was received, as days since 2000-01-01.
        time_t now = time(nullptr);
        uint16_t days = now > 946684800 ? (uint16_t)((now - 946684800) / 86400) : 0;
        uint8_t r = verity_event_inject(d.buf, d.len, e.blob, e.record, days);
        if (r == 0)
        {
            d.dirty = true;
            snprintf(message_, sizeof(message_), "Delivered. Save, then collect it.");
        }
        else
            snprintf(message_, sizeof(message_), "%s", verity_event_status_text(r));
        refresh();
        draw_list();
        draw_top();
    }

    void title_of(const verity_event_view& e, char* out, int cap)
    {
        if (e.ranger)
            snprintf(out, cap, "Ranger Manaphy Egg");
        else
            ui::utf16_to_text(e.title, out, cap);
    }

    // What the card gives: the Pokémon, the item, or the kind of gift.
    const char* gives(const verity_event_view& e) const
    {
        if (e.ranger)
            return "Manaphy Egg";
        if (e.species)
            return ui::name(VERITY_NAME_SPECIES, e.species);
        if (e.item)
            return ui::name(core::doc().info.generation == 5 ? VERITY_NAME_ITEMS5 : VERITY_NAME_ITEMS4, e.item);
        return kind_name(e.gift_type);
    }

    static const char* region_tag(uint8_t r)
    {
        static const char* kTags[] = {"", "NA", "UK", "AU", "EU"};
        return r <= 4 ? kTags[r] : "";
    }

    // One row per card: what it gives, then what tells releases of one event apart (the
    // original trainer's name and ID, and the region). Gifts that can't be delivered now are
    // muted.
    void draw_list()
    {
        ui::clear_px(bottom(), 16, 176);
        for (int r = 0; r < list_.rows; r++)
        {
            int i = list_.scroll + r;
            if (i >= count_)
                break;
            const auto& e = events_[i];
            int y = list_.text_y(i);
            const char* m = e.status ? UI_MUTED : "";
            char what[40];
            snprintf(what, sizeof(what), "%s", gives(e));
            ui::fit(what, 96 - kListX - 4);
            ui::text(bottom(), kListX, y, "%s%s", m, what);
            char ot[24];
            ui::utf16_to_text(e.ot, ot, sizeof(ot));
            if (ot[0])
            {
                ui::text(bottom(), 96, y, UI_MUTED "%s", ot);  // at most 7 characters
                ui::text(bottom(), 168, y, UI_MUTED "%05u", e.tid);
            }
            if (region_tag(e.region)[0])
                ui::text_right(bottom(), 244, y, UI_MUTED "%s", region_tag(e.region));
        }
    }

    static const char* kind_name(uint8_t t)
    {
        switch (t)
        {
            case 1: return "Pokemon";
            case 3: return "Item";
            case 5: return "Decoration";
            case 7: return "Pokemon Egg";
            case 8: return "Member Card";
            case 9: return "Oak's Letter";
            case 11: return "Poketch app";
            case 12: return "Secret Key";
            case 14: return "Pokewalker course";
            default: return "Gift";
        }
    }

    void draw_top()
    {
        ui::clear(top());
        ui::frame_clear(top());
        ui::title(top(), "Mystery gifts");
        char n[20];
        snprintf(n, sizeof(n), "%d for this game", count_);
        ui::title_right(top(), n);
        if (count_ == 0)
        {
            ui::panel(top(), 1, 2, 30, 3);
            ui::Lines l(2);
            ui::text(top(), ui::kInsetTop, l.line(), "None for this language.");
            ui::text(top(), ui::kInsetTop, l.line(), UI_MUTED "B goes back.");
            return;
        }
        const auto& e = events_[list_.selected];
        char t[37];
        title_of(e, t, sizeof(t));
        const int x = ui::kInsetTop, v = 88;  // labels, values
        // The title and whether it can be delivered, then the card's details.
        ui::panel(top(), 1, 2, 30, 12);
        ui::Lines l(2);
        ui::text(top(), x, l.line(), "%.32s", t);
        // The status on a row of its own: some are long ("12 cards held: discard one in-game").
        ui::badge(top(), x / 8, l.grid_row(), e.status ? ui::Badge::Fishy : ui::Badge::Legal,
                  verity_event_status_text(e.status));
        l.block();
        auto field = [&](const char* label, const char* fmt, auto... args) {
            int y = l.line();
            ui::text(top(), x, y, UI_MUTED "%s", label);
            ui::text(top(), v, y, fmt, args...);
        };
        field("Type", "%s", kind_name(e.gift_type));
        if (e.species || e.item)
            field(e.species ? "Pokemon" : "Item", "%s", gives(e));
        char ot[24];
        ui::utf16_to_text(e.ot, ot, sizeof(ot));
        if (ot[0])
            field("OT", "%s   " UI_MUTED "ID" UI_MUTED " %05u", ot, e.tid);
        static const char* regions[] = {"", "North America", "United Kingdom", "Australia",
                                        "Europe"};
        if (e.region >= 1 && e.region <= 4)
            field("Region", "%s", regions[e.region]);
        if (e.card_id)
            field("Card", "%u", e.card_id);
        if (message_[0])
        {
            ui::panel(top(), 1, 16, 30, 2);
            ui::text(top(), x, 16 * 8 + 4, "%s", message_);
        }
        ui::panel(top(), 1, 20, 30, 3);
        ui::Lines c(20);
        ui::text(top(), x, c.line(), UI_MUTED "Collect it in-game from the");
        ui::text(top(), x, c.line(), UI_MUTED "%s", core::doc().info.generation == 5 ? "deliveryman in any Poke Center."
                                                                                  : "deliveryman in any Poke Mart.");
    }
};

// ---- Bag ------------------------------------------------------------------------------------

class Bag : public ui::Scene
{
public:
    const char* name() const override { return "bag"; }
    void enter() override
    {
        if (!core::data_available())
        {
            ui::clear(top());
            ui::frame_clear(top());
            ui::title(top(), "Bag");
            ui::panel(top(), 1, 2, 30, 3);
            ui::Lines l(2);
            ui::text(top(), ui::kInsetTop, l.line(), "Game data is missing.");
            ui::text(top(), ui::kInsetTop, l.line(), UI_MUTED "B goes back.");
            ui::clear(bottom());
            ui::title(bottom(), "Bag");
            ui::hints(bottom(), "B:Back");
            return;
        }
        load();
        draw();
    }

    bool update(const ui::Input& in) override
    {
        if (!core::data_available())
            return !(in.down & KEY_B);
        if (mode_ == kCount)
            return update_count(in);
        if (mode_ == kAdd)
            return update_add(in);
        if (in.down & KEY_B)
        {
            ui::item_icon(true, 0, 0, nullptr, nullptr);
            return false;
        }
        if (in.rep & (KEY_LEFT | KEY_RIGHT))
        {
            pouch_ = (pouch_ + (in.rep & KEY_LEFT ? pouches_ - 1 : 1)) % pouches_;
            list_.selected = list_.scroll = 0;
            message_[0] = 0;
            load();
            draw();
            return true;
        }
        bool changed;
        int a = list_.update(in, &changed);
        if (changed)
        {
            message_[0] = 0;
            draw();
        }
        ui::bar(0, list_.bar_y(), list_.count > 0, list_.pitch);
        int sel = list_.selected;
        if ((in.down & KEY_X) && sel < count_)
            remove(sel);
        else if (a >= 0 && a == count_)
            open_add();
        else if (a >= 0 && items_[a].locked)
            report(kEventItem);
        else if (a >= 0 && max_count() > 1)
        {
            mode_ = kCount;
            count_edit_ = items_[a].count;
            draw();
        }
        return true;
    }

private:
    enum Mode { kList, kCount, kAdd };
    static constexpr int kMaxItems = 300;
    // verity_bag_status_text code for an event item (checked in verity-ffi's bag tests).
    static constexpr uint8_t kEventItem = 4;
    static constexpr const char* kPouchNames[8] = {"Items",    "Key Items", "TMs & HMs",  "Mail",
                                                   "Medicine", "Berries",   "Poke Balls", "Battle Items"};
    Mode mode_ = kList;
    verity_pouch_view pv_[8];
    int pouches_ = 0, pouch_ = 0;
    verity_bag_item items_[kMaxItems];
    int count_ = 0;
    uint16_t addable_[kMaxItems];
    int addable_count_ = 0;
    int count_edit_ = 1;
    ListView list_ = spaced_list();
    ListView add_list_ = spaced_list();
    char message_[40] = "";

    uint32_t names() const
    {
        return core::doc().info.generation == 5 ? VERITY_NAME_ITEMS5 : VERITY_NAME_ITEMS4;
    }
    int max_count() const { return pv_[pouch_].max_count; }

    void load()
    {
        auto& d = core::doc();
        pouches_ = (int)verity_bag_pouches(d.buf, d.len, pv_, 8);
        if (pouch_ >= pouches_)
            pouch_ = 0;
        count_ = (int)verity_bag_items(d.buf, d.len, (uint8_t)pouch_, items_, kMaxItems);
        list_.count = count_ + 1;  // the last row adds an item
        list_.clamp();
    }

    void report(uint8_t code, const char* done = "")
    {
        snprintf(message_, sizeof(message_), "%s", code ? verity_bag_status_text(code) : done);
    }

    void changed(uint8_t code, const char* done)
    {
        if (code == 0)
            core::doc().dirty = true;
        report(code, done);
        load();
    }

    void remove(int i)
    {
        auto& d = core::doc();
        char what[40];
        snprintf(what, sizeof(what), "Removed %s", ui::name(names(), items_[i].item));
        changed(verity_bag_remove(d.buf, d.len, (uint8_t)pouch_, (uint16_t)i), what);
        draw();
    }

    bool update_count(const ui::Input& in)
    {
        int before = count_edit_;
        if (in.rep & KEY_RIGHT)
            count_edit_++;
        if (in.rep & KEY_LEFT)
            count_edit_--;
        if (in.rep & KEY_UP)
            count_edit_ += 10;
        if (in.rep & KEY_DOWN)
            count_edit_ -= 10;
        if (count_edit_ < 1)
            count_edit_ = 1;
        if (count_edit_ > max_count())
            count_edit_ = max_count();
        if (in.down & (KEY_A | KEY_B))
        {
            if (in.down & KEY_A)
            {
                auto& d = core::doc();
                int i = list_.selected;
                changed(verity_bag_set_count(d.buf, d.len, (uint8_t)pouch_, (uint16_t)i,
                                             (uint16_t)count_edit_),
                        "Count set");
            }
            mode_ = kList;
            draw();
        }
        else if (count_edit_ != before)
            draw();
        ui::bar(0, list_.bar_y(), true, list_.pitch);
        return true;
    }

    void open_add()
    {
        auto& d = core::doc();
        addable_count_ = (int)verity_bag_addable(d.buf, d.len, (uint8_t)pouch_, addable_, kMaxItems);
        if (addable_count_ == 0)
        {
            snprintf(message_, sizeof(message_), "Nothing more fits this pouch");
            draw();
            return;
        }
        mode_ = kAdd;
        add_list_.selected = add_list_.scroll = 0;
        add_list_.count = addable_count_;
        message_[0] = 0;
        draw();
    }

    bool update_add(const ui::Input& in)
    {
        if (in.down & KEY_B)
        {
            mode_ = kList;
            draw();
            return true;
        }
        bool moved;
        int a = add_list_.update(in, &moved);
        if (moved)
            draw();
        ui::bar(0, add_list_.bar_y(), true, add_list_.pitch);
        if (a < 0)
            return true;
        auto& d = core::doc();
        uint16_t item = addable_[a];
        char what[40];
        snprintf(what, sizeof(what), "Added %s", ui::name(names(), item));
        changed(verity_bag_add(d.buf, d.len, (uint8_t)pouch_, item, 1), what);
        mode_ = kList;
        // Select the new item (sorted pouches put it in order), then set its count.
        for (int i = 0; i < count_; i++)
            if (items_[i].item == item)
                list_.selected = i;
        list_.clamp();
        if (max_count() > 1 && list_.selected < count_ && items_[list_.selected].item == item)
        {
            mode_ = kCount;
            count_edit_ = 1;
        }
        draw();
        return true;
    }

    void draw()
    {
        draw_top();
        if (mode_ == kAdd)
            draw_add_list();
        else
            draw_list();
    }

    void bottom_frame(const char* title, const char* hints)
    {
        ui::clear(bottom());
        ui::frame_clear(bottom());
        ui::title(bottom(), "%s", title);
        ui::panel(bottom(), 0, list_.first_row, 32, 20);
        ui::hints(bottom(), hints);
    }

    void draw_list()
    {
        bool item = list_.selected < count_;
        const char* hints = mode_ == kCount              ? "</>:1|Up/Dn:10|A:Set|B:Cancel"
                            : !item                      ? "A:Add|</>:Pouch|B:Back"
                            : items_[list_.selected].locked ? "</>:Pouch|B:Back"
                            : max_count() > 1            ? "A:Count|X:Toss|</>:Pouch|B:Back"
                                                         : "X:Toss|</>:Pouch|B:Back";
        bottom_frame(kPouchNames[pv_[pouch_].kind & 7], hints);
        char n[24];
        snprintf(n, sizeof(n), "%d/%d", pouch_ + 1, pouches_);
        ui::title_right(bottom(), n);
        for (int r = 0; r < list_.rows; r++)
        {
            int i = list_.scroll + r;
            if (i > count_)
                break;
            int y = list_.text_y(i);
            if (i == count_)
            {
                ui::text(bottom(), kListX, y, UI_MUTED "+ Add an item");
                break;
            }
            const auto& it = items_[i];
            char what[40];
            snprintf(what, sizeof(what), "%s", ui::name(names(), it.item));
            ui::fit(what, 200 - kListX);
            ui::text(bottom(), kListX, y, "%s%s", it.locked ? UI_MUTED : "", what);
            int count = mode_ == kCount && i == list_.selected ? count_edit_ : it.count;
            if (it.locked)
                ui::text_right(bottom(), 244, y, UI_MUTED "event");
            else if (max_count() > 1)
                ui::text_right(bottom(), 244, y, mode_ == kCount && i == list_.selected ? "< %d >" : "x%d",
                               count);
        }
    }

    void draw_add_list()
    {
        bottom_frame("Add an item", "A:Add|L/R:Page|B:Cancel");
        for (int r = 0; r < add_list_.rows; r++)
        {
            int i = add_list_.scroll + r;
            if (i >= addable_count_)
                break;
            char what[40];
            snprintf(what, sizeof(what), "%s", ui::name(names(), addable_[i]));
            ui::fit(what, 244 - kListX);
            ui::text(bottom(), kListX, add_list_.text_y(i), "%s", what);
        }
    }

    void draw_top()
    {
        ui::clear(top());
        ui::frame_clear(top());
        ui::title(top(), "Bag");
        if (core::doc().dirty)
            ui::title_right(top(), "* unsaved");
        const int x = ui::kInsetTop;
        // Every pouch with how many items it holds; the open one is not muted.
        ui::panel(top(), 1, 2, 30, (pouches_ * ui::kPitch + 8 + 7) / 8);
        ui::Lines l(2);
        for (int p = 0; p < pouches_; p++)
        {
            int y = l.line();
            const char* m = p == pouch_ ? "" : UI_MUTED;
            ui::text(top(), x, y, "%s%s %s", m, p == pouch_ ? ">" : " ", kPouchNames[pv_[p].kind & 7]);
            ui::text_right(top(), 232, y, "%s%u / %u", m, pv_[p].count, pv_[p].slots);
        }
        // The selected item: its icon (from a ROM), name and count.
        uint16_t item = 0;
        if (mode_ == kAdd && add_list_.selected < addable_count_)
            item = addable_[add_list_.selected];
        else if (mode_ != kAdd && list_.selected < count_)
            item = items_[list_.selected].item;
        int row = 16;
        uint8_t tiles[512];
        uint16_t colors[16];
        bool icon = item && core::item_icon(core::doc().info.generation, item, tiles, colors);
        ui::item_icon(true, 16, row * 8 + 4, icon ? tiles : nullptr, colors);
        if (item)
        {
            ui::panel(top(), 1, row, 30, 4);
            int tx = icon ? 48 : x;
            ui::text(top(), tx, row * 8 + 4, "%s", ui::name(names(), item));
            const auto* it = mode_ == kAdd ? nullptr : &items_[list_.selected];
            if (it && it->locked)
                ui::text(top(), tx, row * 8 + 16, UI_MUTED "Event item: from Mystery Gift");
            else if (it)
                ui::text(top(), tx, row * 8 + 16, UI_MUTED "Count" UI_MUTED "  %d / %d",
                         mode_ == kCount ? count_edit_ : it->count, max_count());
            else
                ui::text(top(), tx, row * 8 + 16, UI_MUTED "A adds one");
        }
        if (message_[0])
        {
            ui::panel(top(), 1, 21, 30, 2);
            ui::text(top(), x, 21 * 8 + 4, "%s", message_);
        }
    }
};

// ---- Memory Link and Key System ---------------------------------------------------------------

class UnovaLink : public ui::Scene
{
public:
    const char* name() const override { return "link"; }
    ~UnovaLink() override { free(other_); }

    void enter() override
    {
        if (!scanned_)
        {
            scanned_ = true;
            for (const char* root : {"sd:/", "fat:/"})
                count_ = scan_saves(files_, count_, root, 3);
            int kept = 0;
            for (int i = 0; i < count_; i++)
                if (strcmp(files_[i], core::doc().path) != 0)
                    memmove(files_[kept++], files_[i], kPathLen);
            count_ = kept;
            list_.count = count_ + 1;  // row 0 is "Enable all keys"
            list_.clamp();
        }
        draw();
    }

    bool update(const ui::Input& in) override
    {
        if (other_ || enabling_)
        {
            if (in.down & KEY_B)
                close_other();
            else if ((in.down & KEY_A) && ready())
                link();
            return true;
        }
        bool changed;
        int pick = list_.update(in, &changed);
        if (changed)
            draw_list();
        ui::bar(0, list_.bar_y(), true, list_.pitch);
        if (in.down & KEY_B)
            return false;
        if (pick == 0)
            open_enable_all();
        else if (pick > 0)
            open_other(files_[pick - 1]);
        return true;
    }

private:
    char files_[kMaxFiles][kPathLen];
    int count_ = 0;
    bool scanned_ = false;
    ListView list_ = spaced_list();
    uint8_t* other_ = nullptr;
    size_t other_len_ = 0;
    const char* other_path_ = "";
    bool enabling_ = false;
    verity_unova_preview_t preview_{};

    // Status 1 is ready, with a note that the link replaces another player's.
    bool ready() const { return preview_.status <= 1; }
    bool replaces() const { return preview_.status == 1; }
    char trainer_[32] = "";
    char message_[40] = "";

    void open_other(const char* path)
    {
        const auto& d = core::doc();
        other_ = core::read_save_file(path, &other_len_);
        if (!other_)
        {
            snprintf(message_, sizeof(message_), "Could not read that file.");
            draw();
            return;
        }
        other_path_ = path;
        message_[0] = 0;
        trainer_[0] = 0;
        verity_unova_preview(d.buf, d.len, other_, other_len_, &preview_);
        auto info = d.info;
        if (verity_save_probe(other_, other_len_, &info) == VERITY_OK)
            ui::utf16_to_text(info.ot_name, trainer_, sizeof(trainer_));
        draw();
    }

    void open_enable_all()
    {
        const auto& d = core::doc();
        verity_unova_enable_preview(d.buf, d.len, &preview_);
        enabling_ = true;
        message_[0] = 0;
        draw();
    }

    void close_other()
    {
        free(other_);
        other_ = nullptr;
        enabling_ = false;
        draw();
    }

    void link()
    {
        auto& d = core::doc();
        uint32_t seed = (uint32_t)time(nullptr) * 1103515245u + 12345u;
        uint8_t r = enabling_ ? verity_unova_enable_all(d.buf, d.len)
                              : verity_unova_apply(d.buf, d.len, other_, other_len_, seed);
        if (r == 0)
        {
            d.dirty = true;
            snprintf(message_, sizeof(message_), "Done. Save to SD to keep it.");
        }
        else
            snprintf(message_, sizeof(message_), "%s", verity_unova_status_text(r));
        close_other();
    }

    void draw()
    {
        ui::clear(top());
        ui::clear(bottom());
        ui::frame_clear(top());
        ui::frame_clear(bottom());
        ui::title(top(), "Memory Link and keys");
        if (other_ || enabling_)
            draw_preview();
        else
        {
            ui::panel(top(), 1, 2, 30, 8);
            ui::Lines l(2);
            const int x = ui::kInsetTop;
            ui::text(top(), x, l.line(), "Pick a Black or White save for");
            ui::text(top(), x, l.line(), "Memory Link, or a Black 2 or");
            ui::text(top(), x, l.line(), "White 2 save to receive its keys.");
            l.block();
            ui::text(top(), x, l.line(), UI_MUTED "Or enable every key, no partner.");
            if (message_[0])
            {
                ui::panel(top(), 1, 12, 30, 2);
                ui::text(top(), x, 12 * 8 + 4, "%s", message_);
            }
            draw_list();
        }
    }

    void draw_preview()
    {
        static constexpr const char* kKeys[5] = {"Easy Mode", "Challenge Mode", "City swap",
                                                 "Iron Chamber", "Iceberg Chamber"};
        const int x = ui::kInsetTop;
        int top_row = 2;
        if (other_)
        {
            const char* slash = strrchr(other_path_, '/');
            ui::panel(top(), 1, 2, 30, 3);
            ui::Lines l(2);
            ui::text(top(), x, l.line(), "%.30s", slash ? slash + 1 : other_path_);
            ui::text(top(), x, l.line(), UI_MUTED "Trainer" UI_MUTED "  %s", trainer_);
            top_row = 7;
        }
        ui::bar(0, 0, false);
        ui::panel(top(), 1, top_row, 30, 21 - top_row);
        ui::Lines l(top_row);
        if (!ready())
            ui::badge(top(), 2, l.grid_row(), ui::Badge::Invalid, verity_unova_status_text(preview_.status));
        else if (preview_.mode == 1)
        {
            ui::text(top(), x, l.line(), "Memory Link");
            ui::text(top(), x, l.line(), UI_MUTED "%s", preview_.flashback ? "Unlocks a new flashback" : "No new flashbacks");
            if (replaces())
            {
                l.block();
                ui::text(top(), x, l.line(), "%s", verity_unova_status_text(1));
            }
        }
        else
        {
            ui::text(top(), x, l.line(), preview_.mode == 3 ? "Keys this will enable:" : "Keys this save will receive:");
            for (int i = 0; i < 5; i++)
                if (preview_.keys[i])
                    ui::badge(top(), 3, l.grid_row(), ui::Badge::Legal, kKeys[i]);
            if (preview_.mode == 3)
            {
                l.block();
                ui::text(top(), x, l.line(), UI_MUTED "These can't be sent to other games.");
            }
        }
        ui::title(bottom(), "Link");
        ui::panel(bottom(), 1, 3, 30, 4);
        ui::Lines b(3);
        ui::text(bottom(), x, b.line(), "%s", ready() ? "Ready." : "Can't link these saves.");
        ui::text(bottom(), x, b.line(), UI_MUTED "Nothing changes until you link.");
        ui::hints(bottom(), ready() ? "A:Link|B:Back" : "B:Back");
    }

    void draw_list()
    {
        ui::title(bottom(), "Link with");
        ui::panel(bottom(), 0, list_.first_row, 32, 20);
        ui::hints(bottom(), "A:Choose|L/R:Page|B:Back");
        ui::clear_px(bottom(), 16, 176);
        for (int r = 0; r < list_.rows; r++)
        {
            int i = list_.scroll + r - 1;
            int y = list_.text_y(list_.scroll + r);
            if (i < 0)
            {
                ui::text(bottom(), kListX, y, "Enable all keys");
                ui::text_right(bottom(), 244, y, UI_MUTED "no partner");
            }
            else if (i < count_)
            {
                const char* slash = strrchr(files_[i], '/');
                ui::text(bottom(), kListX, y, "%.30s", slash ? slash + 1 : files_[i]);
            }
        }
    }
};

// ---- box view ------------------------------------------------------------------------------

// The Pokémon's front sprite at x, y with its ball behind the top-left corner, from the sprite
// and item caches of the save's generation; false (nothing drawn) without them.
static bool show_pokemon(const verity_pkm_view& v, int x, int y, bool ball = true)
{
    static uint8_t tiles[4608];
    uint16_t colors[16];
    uint8_t gen = core::doc().info.generation;
    if (!core::pokemon_sprite(gen, v.species, v.form, v.gender == 1, v.is_egg, v.is_shiny && !v.is_egg, tiles,
                              colors))
        return false;
    ui::pokemon_sprite(x, y, tiles, colors);
    uint8_t ball_tiles[512];
    uint16_t bc[16];
    if (ball && !v.is_egg && core::ball_icon(gen, v.ball, ball_tiles, bc))
        ui::item_icon(true, x - 2, y - 4, ball_tiles, bc);  // its outline clear of the panel border
    return true;
}

class BoxView : public ui::Scene
{
public:
    const char* name() const override { return "boxes"; }
    void enter() override
    {
        // Drawn now, behind the fade: waiting for the first update let the fade-in show the
        // previous scene's top screen for a few frames.
        ui::clear(bottom());
        draw();
        dirty_ = false;
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
        if ((in.down & KEY_A) && at_cell(cur_).species != 0)
        {
            ui::push(editor(party_ ? 0xFF : box_, cur_));
            return true;
        }
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
        // On the cell grid, so each slot's backing is whole frame tiles.
        if (party_)
        {
            *x = 48 + (i % 2) * 112;
            *y = 24 + (i / 2) * 48;
        }
        else
        {
            *x = 8 + (i % 6) * 40;
            *y = 16 + (i / 6) * 32;
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
            if (x >= cx && x < cx + 40 && y >= cy && y < cy + 32)
                cur_ = i;
        }
    }

    verity_pkm_view at_cell(int i) const
    {
        return party_ ? core::slot(0xFF, i) : core::slot(box_, i);
    }

    void draw()
    {
        char name[64];
        ui::clear(bottom());
        ui::frame_clear(bottom());
        if (party_)
            ui::title(bottom(), "Party");
        else
        {
            core::box_name(box_, name, sizeof(name));
            int pad = (30 - ui::text_len(name)) / 2 - 1;
            ui::title(bottom(), "<%*s%s", pad > 0 ? pad : 0, "", name);
            char n[24];
            snprintf(n, sizeof(n), "%d/%d  >", box_ + 1, (int)core::doc().boxes);
            ui::title_right(bottom(), n);
        }
        ui::hints(bottom(), party_ ? "SEL:Boxes|A:Edit|X:Add|B:Back"
                                   : "L/R:Box|SEL:Party|A:Edit|X:Add");
        ui::sprites_hide_all();
        int n = party_ ? 6 : 30;
        for (int i = 0; i < n; i++)
        {
            verity_pkm_view v = at_cell(i);
            int x, y;
            cell_pos(i, &x, &y);
            ui::slot(bottom(), x / 8, y / 8, i == cur_ ? 2 : v.species ? 0 : 1);
            uint8_t tiles[512], pal;
            if (v.species != 0 &&
                core::icon_tiles(v.species, v.form, v.gender == 1, v.is_egg, tiles, &pal))
                ui::icon_rom(2 + 2 * i, x + ui::kSlotIconX, y + ui::kSlotIconY, tiles, pal,
                             v.is_shiny && !v.is_egg);
            else if (v.species != 0)
                // The placeholders are drawn centred in their frame, unlike the game's icons.
                ui::icon(2 + 2 * i, x + ui::kSlotIconX, y - 1,
                         v.types[0] == 0xFF ? 17 : v.types[0], v.is_egg, v.is_shiny);
        }
        details(at_cell(cur_));
    }

    // The top screen while browsing, laid out like the games' summary: the Pokémon's sprite (its
    // ball behind it) with the verdict and who it is beside it, then its moves and IVs across the
    // panel. EVs and the PID are in the editor.
    void details(const verity_pkm_view& v)
    {
        ui::clear(top());
        ui::frame_clear(top());
        ui::top_art_hide();
        ui::item_icon(true, 0, 0, nullptr, nullptr);
        const int x = ui::kInsetTop, rx = kSideX;
        if (v.species == 0)
        {
            ui::title(top(), "Empty slot");
            ui::panel(top(), 1, 2, 30, 3);
            ui::Lines l(2);
            ui::text(top(), x, l.line(), "Nothing here.");
            ui::text(top(), x, l.line(), UI_MUTED "X or A adds a Pokémon here.");
            return;
        }
        char nick[32], ot[32];
        ui::utf16_to_text(v.nickname, nick, sizeof(nick));
        ui::utf16_to_text(v.ot_name, ot, sizeof(ot));
        ui::fit(ot, 80);
        const char* sp = ui::name(VERITY_NAME_SPECIES, v.species);
        uint8_t gen = core::doc().info.generation;
        bool art = show_pokemon(v, 8, 16);
        // Without a ROM there is no sprite: the text starts at the panel's edge.
        int cx = art ? rx : x;
        if (v.is_egg)
        {
            ui::title(top(), "Egg");
            ui::panel(top(), 1, 2, 30, 11);
            ui::Lines l(2);
            legality_line(l, v, cx / 8);
            l.block();
            ui::text(top(), cx, l.line(), UI_MUTED "Will hatch into" UI_MUTED " %s", sp);
            ui::text(top(), cx, l.line(), UI_MUTED "OT" UI_MUTED "  %s", ot);
            ui::text(top(), cx, l.line(), UI_MUTED "ID" UI_MUTED "  %05u / %05u", v.tid, v.sid);
            return;
        }
        char lv[12];
        snprintf(lv, sizeof(lv), "Lv %u", v.level);
        ui::title(top(), "%s", "");
        ui::title_right(top(), lv);
        // The box markings in the title bar, set ones bright; the name is cut short of them.
        int mx = 250 - ui::text_width(lv) - 10 - 6 * 9;
        for (int k = 0; k < 6; k++)
            ui::shape(top(), mx + 9 * k, 0, k, v.markings >> k & 1 ? 0 : 1);
        if (v.is_nicknamed && strcmp(nick, sp) != 0)
            ui::text_in(top(), 4, mx - 6, 0, "%s  " UI_MUTED "%s", nick, sp);
        else
            ui::text_in(top(), 4, mx - 6, 0, "%s", nick);
        ui::panel(top(), 1, 2, 30, 20);
        ui::Lines l(2);
        legality_line(l, v, cx / 8);
        // Types (the game's icons, else names), then the gender and the shiny star.
        int ty = 34, gx = cx;
        for (int k = 0; k < 2; k++)
        {
            if (v.types[k] == 0xFF || (k == 1 && v.types[1] == v.types[0]))
                continue;
            uint8_t t[256];
            uint16_t c[16];
            if (core::type_icon(gen, 0, v.types[k], t, c))
            {
                ui::small_icon(k, gx, ty, t, c);
                gx += 36;
            }
            else
            {
                const char* tn = ui::name(VERITY_NAME_TYPES, v.types[k]);
                ui::text(top(), gx, ty + 4, "%s", tn);
                gx += ui::text_width(tn) + 6;
            }
        }
        if (v.gender < 2)
        {
            ui::shape(top(), gx + 2, ty + 4, v.gender == 0 ? ui::kMale : ui::kFemale, v.gender == 0 ? 3 : 4);
            gx += 2 + 7 + 4;
        }
        if (v.is_shiny)
            ui::shape(top(), gx, ty + 4, ui::kShiny, 2);
        int y = 54;
        char line[64];
        snprintf(line, sizeof(line), "%s " UI_MUTED "nature", ui::name(VERITY_NAME_NATURES, v.nature));
        ui::text(top(), cx, y, "%s", line);
        snprintf(line, sizeof(line), "%s", ui::name(VERITY_NAME_ABILITIES, v.ability));
        ui::fit(line, 244 - cx);
        ui::text(top(), cx, y + 12, "%s", line);
        uint32_t items = gen == 4 ? VERITY_NAME_ITEMS4 : VERITY_NAME_ITEMS5;
        snprintf(line, sizeof(line), UI_MUTED "Holds" UI_MUTED " %s", v.held_item ? ui::name(items, v.held_item) : "nothing");
        ui::fit(line, 244 - cx);
        ui::text(top(), cx, y + 24, "%s", line);
        // The OT name is what gets cut (a wide ROM font), never the ID.
        char id[24];
        snprintf(id, sizeof(id), "  " UI_MUTED "ID" UI_MUTED " %05u", v.tid);
        char ot_fit[32];
        snprintf(ot_fit, sizeof(ot_fit), "%s", ot);
        ui::fit(ot_fit, 244 - cx - ui::text_width(UI_MUTED "OT" UI_MUTED " ") - ui::text_width(id));
        snprintf(line, sizeof(line), UI_MUTED "OT" UI_MUTED " %s%s", ot_fit, id);
        ui::text(top(), cx, y + 40, "%s", line);
        // The level first, so a long place name is what gets cut.
        snprintf(line, sizeof(line), UI_MUTED "Met" UI_MUTED " Lv %u, %s", v.met_level,
                 ui::location(gen, v.met_location));
        ui::fit(line, 244 - cx);
        ui::text(top(), cx, y + 52, "%s", line);
        // Across the panel, under the sprite.
        y = 120;
        for (int m = 0; m < 4; m += 2)
        {
            for (int k = 0; k < 2; k++)
                ui::text(top(), x + k * 116, y, "%.15s", v.moves[m + k] ? ui::name(VERITY_NAME_MOVES, v.moves[m + k]) : "-");
            y += ui::kPitch;
        }
        static const char* st[6] = {"HP", "Atk", "Def", "Spe", "SpA", "SpD"};
        int hy = y + 8, iy = hy + ui::kPitch;
        ui::text(top(), x, iy, UI_MUTED "IVs");
        for (int k = 0; k < 6; k++)
        {
            int cx2 = x + 56 + k * 30;  // right edge of each column
            ui::text_right(top(), cx2, hy, UI_MUTED "%s", st[k]);
            ui::text_right(top(), cx2, iy, "%u", v.ivs[k]);
        }
    }

    // Text beside the sprite starts here (the sprite is 96 px wide from x 8).
    static constexpr int kSideX = 112;


    // The verdict on one grid line: a badge, then the time or what failed.
    void legality_line(ui::Lines& l, const verity_pkm_view& v, int col = 2)
    {
        uint32_t ms = 0;
        verity_legality_report r = party_ ? core::legality(0xFF, cur_, &ms)
                                          : core::legality(box_, cur_, &ms);
        static const char* why[] = {"",        "egg",       "origin game",
                                    "data missing", "Gen 5 save", "hatched",
                                    "event/trade/walker"};
        ui::Row line(top(), l.grid_row(), col * 8);
        char rest[48] = "";
        if (r.status == 1)
        {
            line.badge(ui::Badge::Legal, "LEGAL");
            snprintf(rest, sizeof(rest), col > 2 ? UI_MUTED "%lu ms" : UI_MUTED "checked in %lu ms", (unsigned long)ms);
        }
        else if (r.status == 2)
        {
            line.badge(ui::Badge::Invalid, "ILLEGAL");
            for (int i = 0; i < r.check_count && strlen(rest) < 30; i++)
            {
                strcat(rest, i ? " " : "");
                strcat(rest, core::check_name(r.checks[i]));
            }
            if (!(r.move_ok[0] && r.move_ok[1] && r.move_ok[2] && r.move_ok[3]))
                strcat(rest, " Moves");
        }
        else
        {
            line.badge(ui::Badge::Fishy, "NOT CHECKED");
            snprintf(rest, sizeof(rest), "%s", r.unsupported < 7 ? why[r.unsupported] : "?");
        }
        if (!v.checksum_ok)
            strcat(rest, "  ! checksum");
        line.shrink("%s", rest);
        line.draw();
    }
};


// ---- add-Pokémon wizard --------------------------------------------------------------------

class AddWizard : public ui::Scene
{
public:
    const char* name() const override { return "add"; }
    AddWizard(uint8_t box_index, uint8_t slot) : box_(box_index), slot_(slot) {}

    void enter() override
    {
        if (!loaded_)
        {
            loaded_ = true;
            version_ = core::wizard_version();
            supported_ = core::ensure_save_data();
            if (supported_)
                load_species();
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
            ui::bar(0, list_.bar_y(), true, list_.pitch);
            if (in.down & KEY_B)
                return false;
            if ((in.down & KEY_Y) && !gen5())
            {
                egg_ = !egg_;
                load_species();
                list_ = spaced_list();
                draw();
                return true;
            }
            if ((in.down & KEY_SELECT) && other_game())
            {
                version_ = other_game();
                load_species();
                list_ = spaced_list();
                draw();
                return true;
            }
            if (a >= 0)
            {
                species_ix_ = a;
                if (egg_)
                {
                    step_ = 2;
                    opt_ = 0;
                    draw();
                    ui::bar(0, opt_bar_y(0), true, ui::kGridPitch);
                    return true;
                }
                enc_count_ = (int)verity_wild_encounters(version_, species_[a], enc_, 64);
                step_ = 1;
                enc_list_ = spaced_list();
                enc_list_.count = enc_count_ + 1;  // row 0 is "any slot"
                draw();
            }
            else if (changed)
                draw_list();
        }
        else if (step_ == 1)
        {
            int a = enc_list_.update(in, &changed);
            ui::bar(0, enc_list_.bar_y(), true, enc_list_.pitch);
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
                step_ = egg_ ? 0 : 1;
                draw();
                return true;
            }
            int before = opt_;
            int opts = egg_ ? kEggOpts : kOpts;
            if (in.rep & KEY_UP)
                opt_ = (opt_ + opts - 1) % opts;
            if (in.rep & KEY_DOWN)
                opt_ = (opt_ + 1) % opts;
            // Gen 5 PIDs aren't tied to a lead, so that row is skipped.
            if (!egg_ && gen5() && opt_ == 4)
                opt_ = (in.rep & KEY_UP) ? 3 : 5;
            int dir = (in.rep & KEY_RIGHT) ? 1 : (in.rep & KEY_LEFT) ? -1 : 0;
            if (in.tapped)
            {
                int row = opt_at(in.tap_y);
                if (row >= 0 && row < opts)
                {
                    if (row == opt_ && row < opts - 1)
                        dir = 1;
                    opt_ = row;
                }
            }
            if (dir)
                egg_ ? change_egg(dir) : change(dir);
            bool go = (opt_ == opts - 1 &&
                       ((in.down & KEY_A) || (in.tapped && opt_at(in.tap_y) == opts - 1))) ||
                      (in.down & KEY_START);
            if (go && (egg_ ? generate_egg() : generate()))
                return false;
            if (dir || opt_ != before)
                draw_options();
            ui::bar(0, opt_bar_y(opt_), true, ui::kGridPitch);
        }
        return true;
    }

private:
    uint8_t box_, slot_;
    bool loaded_ = false, supported_ = false;
    uint8_t version_ = 12;
    uint16_t species_[649];
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
    static constexpr int kEggOpts = 5;
    bool egg_ = false, hatched_ = false;
    static constexpr uint8_t kBalls[] = {4, 3, 2, 1, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15};
    char status_[48] = "";
    ListView list_ = spaced_list(), enc_list_ = spaced_list();
    // Option rows on the grid pitch; the highlight fills the row.
    static int opt_y(int r) { return 2 * 8 + 4 + ui::kGridPitch * r; }
    static int opt_bar_y(int r) { return opt_y(r) - (ui::kGridPitch - 8) / 2; }
    static int opt_at(int tap_y) { int off = tap_y - 2 * 8 - 4 + (ui::kGridPitch - 8) / 2; return off < 0 ? -1 : off / ui::kGridPitch; }
    void opt(int r, const char* label, const char* fmt, ...) __attribute__((format(printf, 4, 5)))
    {
        char v[48];
        va_list ap;
        va_start(ap, fmt);
        vsnprintf(v, sizeof(v), fmt, ap);
        va_end(ap);
        ui::text(bottom(), kListX, opt_y(r), UI_MUTED "%s", label);
        ui::text(bottom(), 96, opt_y(r), "%s", v);
    }

    void load_species()
    {
        species_count_ = egg_ ? (int)verity_egg_species(version_, species_, 649)
                              : (int)verity_wild_species(version_, species_, 649);
    }

    bool gen5() const { return version_ >= 20 && version_ <= 23; }

    /// The other game of a pair that shares a save type (the save doesn't record which it is).
    uint8_t other_game() const
    {
        switch (version_)
        {
            case 10: return 11;
            case 11: return 10;
            case 7: return 8;
            case 8: return 7;
            case 20: return 21;
            case 21: return 20;
            case 22: return 23;
            case 23: return 22;
            default: return 0;
        }
    }

    static const char* game_name(uint8_t v)
    {
        switch (v)
        {
            case 10: return "Diamond";
            case 11: return "Pearl";
            case 7: return "HeartGold";
            case 8: return "SoulSilver";
            case 20: return "White";
            case 21: return "Black";
            case 22: return "White 2";
            case 23: return "Black 2";
            default: return "Platinum";
        }
    }

    static const char* slot_type(uint8_t t)
    {
        static const char* names[] = {"Grass",   "Surf",     "Old Rod", "Good Rod", "Super Rod",
                                      "Rock",    "Headbutt", "Headbutt", "Contest", "Honey",
                                      "Safari",  "Safari",   "Saf rod", "Saf rod", "Saf rod"};
        return t < 15 ? names[t] : "?";
    }

    static const char* slot_type5(uint8_t t)
    {
        static const char* names[] = {"Standard", "Grass", "Surf", "Super Rod", "Swarm", "Grotto"};
        return t < 6 ? names[t] : "?";
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
        else if (opt_ == 4 && !gen5())
            lead_ = (lead_ + dir + 4) % 4;
        else if (opt_ == 5)
        {
            int n = (int)(sizeof(kBalls) / sizeof(kBalls[0])) + 1;
            ball_ = (ball_ + dir + n) % n;
        }
    }

    void change_egg(int dir)
    {
        if (opt_ == 0)
            nature_ = (nature_ + 1 + dir + 26) % 26 - 1;
        else if (opt_ == 1)
            gender_ = (gender_ + 1 + dir + 3) % 3 - 1;
        else if (opt_ == 2)
            shiny_ = !shiny_;
        else if (opt_ == 3)
            hatched_ = !hatched_;
    }

    bool generate_egg()
    {
        time_t now = time(nullptr);
        struct tm* t = localtime(&now);
        static uint32_t counter = 0;
        verity_egg_request r;
        memset(&r, 0, sizeof(r));
        r.version = version_;
        r.species = species_[species_ix_];
        r.nature = nature_ < 0 ? 0xFF : (uint8_t)nature_;
        r.gender = gender_ < 0 ? 0xFF : (uint8_t)gender_;
        r.shiny = shiny_;
        r.hatched = hatched_;
        r.date[0] = t ? (uint8_t)(t->tm_year - 100) : 26;
        r.date[1] = t ? (uint8_t)(t->tm_mon + 1) : 1;
        r.date[2] = t ? (uint8_t)t->tm_mday : 1;
        r.seed = (uint32_t)now * 2654435761u + ++counter;
        uint8_t code = verity_egg_insert(core::doc().buf, core::doc().len, &r,
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
        ui::frame_clear(top());
        ui::frame_clear(bottom());
        if (!supported_)
        {
            ui::title(top(), "Add Pokemon");
            ui::panel(top(), 1, 2, 30, 3);
            ui::Lines l(2);
            ui::text(top(), ui::kInsetTop, l.line(), "Only Gen 4 saves for now");
            ui::text(top(), ui::kInsetTop, l.line(), UI_MUTED "(D/P/Pt wild encounters).");
            ui::hints(bottom(), "B:Back");
            return;
        }
        ui::title(top(), "Add %s", egg_ ? "Egg" : "Pokemon");
        char g[32];
        snprintf(g, sizeof(g), "legal, %s", game_name(version_));
        ui::title_right(top(), g);
        const int x = ui::kInsetTop;
        ui::panel(top(), 1, 2, 30, 3);
        ui::Lines w(2);
        ui::text(top(), x, w.line(), "Step %d of 3  " UI_MUTED "to %s %u", step_ + 1, box_ == 0xFF ? "party slot" : "box slot",
                 (box_ == 0xFF ? slot_ : box_ * 30 + slot_) + 1);
        ui::panel(top(), 1, 7, 30, 7);
        ui::Lines l(7);
        if (step_ == 0)
        {
            ui::text(top(), x, l.line(), egg_ ? "Egg species" : "Species");
            ui::text(top(), x, l.line(), UI_MUTED "%d %s", species_count_, egg_ ? "can be bred here" : "with wild slots");
            l.block();
            if (other_game())
                ui::text(top(), x, l.line(), "SELECT " UI_MUTED "switches game");
            if (!gen5())
                ui::text(top(), x, l.line(), "Y " UI_MUTED "%s", egg_ ? "makes a wild encounter instead" : "makes a bred egg instead");
            draw_list();
        }
        else if (step_ == 1)
        {
            ui::text(top(), x, l.line(), "Encounter");
            ui::text(top(), x, l.line(), UI_MUTED "for %s", ui::name(VERITY_NAME_SPECIES, species_[species_ix_]));
            draw_list();
        }
        else
        {
            ui::text(top(), x, l.line(), "Options");
            ui::text(top(), x, l.line(), UI_MUTED "for %s", ui::name(VERITY_NAME_SPECIES, species_[species_ix_]));
            draw_options();
        }
        ui::hints(bottom(), step_ == 2 ? "A:Next|B:Back|START:Generate" : "A:Next|L/R:Page|B:Back");
    }

    void draw_list()
    {
        ui::clear_px(bottom(), 16, 176);
        ui::panel(bottom(), 0, 2, 32, 20);
        if (step_ == 0)
        {
            ui::title(bottom(), "Species");
            for (int r = 0; r < list_.rows; r++)
            {
                int i = list_.scroll + r;
                if (i >= species_count_)
                    break;
                int y = list_.text_y(i);
                ui::text(bottom(), kListX, y, UI_MUTED "#%03u", species_[i]);
                ui::text(bottom(), kListX + 44, y, "%s", ui::name(VERITY_NAME_SPECIES, species_[i]));
            }
        }
        else
        {
            ui::title(bottom(), "Encounter");
            for (int r = 0; r < enc_list_.rows; r++)
            {
                int i = enc_list_.scroll + r - 1;
                int y = enc_list_.text_y(enc_list_.scroll + r);
                if (i == -1)
                    ui::text(bottom(), kListX, y, "Any wild slot");
                else if (i < enc_count_)
                {
                    const auto& e = enc_[i];
                    char lv[8];
                    if (e.level_min == e.level_max)
                        snprintf(lv, sizeof(lv), "%u", e.level_min);
                    else
                        snprintf(lv, sizeof(lv), "%u-%u", e.level_min, e.level_max);
                    char where[40];
                    snprintf(where, sizeof(where), "%s", ui::location(gen5() ? 5 : 4, e.location));
                    ui::fit(where, 140 - kListX - 4);
                    ui::text(bottom(), kListX, y, "%s", where);
                    int lv_x = ui::text_right(bottom(), 244, y, UI_MUTED "Lv " UI_MUTED "%s", lv);
                    ui::text_in(bottom(), 140, lv_x - 6, y, UI_MUTED "%s",
                                gen5() ? slot_type5(e.slot_type) : slot_type(e.slot_type));
                }
            }
        }
    }

    void draw_status(int rows)
    {
        if (!status_[0])
            return;
        int row = 3 + (rows * ui::kGridPitch + 8) / 8 + 2;
        ui::panel(bottom(), 1, row, 30, 2);
        ui::text(bottom(), ui::kInsetTop, row * 8 + 4, "%s", status_);
    }

    void draw_egg_options()
    {
        ui::title(bottom(), "Options");
        ui::clear_px(bottom(), 16, 176);
        ui::panel(bottom(), 0, 2, 32, (5 * ui::kGridPitch + 8) / 8);
        opt(0, "Nature", "< %s >", nature_ < 0 ? "Any" : ui::name(VERITY_NAME_NATURES, nature_));
        opt(1, "Gender", "< %s >", gender_ < 0 ? "Any" : gender_ ? "Female" : "Male");
        opt(2, "Shiny", "< %s >", shiny_ ? "Yes" : "No");
        opt(3, "Hatched", "< %s >", hatched_ ? "Yes" : "No");
        ui::text(bottom(), kListX, opt_y(4), "Generate and place");
        draw_status(5);
    }

    void draw_options()
    {
        if (egg_)
        {
            draw_egg_options();
            return;
        }
        ui::title(bottom(), "Options");
        ui::clear_px(bottom(), 16, 176);
        ui::panel(bottom(), 0, 2, 32, (7 * ui::kGridPitch + 8) / 8);
        opt(0, "Nature", "< %s >", nature_ < 0 ? "Any" : ui::name(VERITY_NAME_NATURES, nature_));
        opt(1, "Shiny", "< %s >", shiny_ ? "Yes" : "No");
        if (level_)
            opt(2, "Level", "< %d >", level_);
        else
            opt(2, "Level", "< Any >");
        opt(3, "Gender", "< %s >", gender_ < 0 ? "Any" : gender_ ? "Female" : "Male");
        if (gen5())
            opt(4, "Lead", UI_MUTED "not used in Gen 5");
        else
            opt(4, "Lead", "< %s >", lead_name(lead_));
        opt(5, "Ball", "< %s >", ball_ ? ui::name(VERITY_NAME_ITEMS4, kBalls[ball_ - 1]) : "Default");
        ui::text(bottom(), kListX, opt_y(6), "Generate and place");
        draw_status(7);
    }
};


// ---- editor --------------------------------------------------------------------------------

class Editor : public ui::Scene
{
public:
    const char* name() const override { return "editor"; }
    Editor(uint8_t box_index, uint8_t slot) : box_(box_index), slot_(slot) {}

    void enter() override
    {
        if (!loaded_)
        {
            loaded_ = true;
            core::ensure_save_data();
            orig_ = core::slot(box_, slot_);
            memset(&e_, 0, sizeof(e_));
            e_.held_item = orig_.held_item;
            memcpy(e_.moves, orig_.moves, sizeof(e_.moves));
            memcpy(e_.ivs, orig_.ivs, 6);
            memcpy(e_.evs, orig_.evs, 6);
            e_.level = orig_.level ? orig_.level : 1;
            e_.friendship = orig_.friendship;
            load_fields();
            gen4_ = core::doc().info.generation == 4;
            items_ = count(gen4_ ? VERITY_NAME_ITEMS4 : VERITY_NAME_ITEMS5);
            moves_ = count(VERITY_NAME_MOVES);
            moves_ = gen4_ ? (moves_ > 468 ? 468 : moves_) : moves_;
        }
        ui::clear(top());
        ui::clear(bottom());
        draw();
        preview();
    }

    bool update(const ui::Input& in) override
    {
        if (picking_)
            return update_picker(in);
        if (showing_fix_)
            return update_fix(in);
        if (typing_)
            return update_keyboard(in);
        if (ribbon_list_)
            return update_ribbons(in);
        if (in.down & KEY_B)
            return false;
        if (tab_ == 1 && gen4_ && (in.down & KEY_X))
        {
            find_closest();
            return true;
        }
        int rows = tab_rows();
        int before_row = row_, before_tab = tab_;
        bool changed = false;
        if (in.down & KEY_L)
            tab_ = (tab_ + kTabs - 1) % kTabs;
        if (in.down & KEY_R)
            tab_ = (tab_ + 1) % kTabs;
        if (in.down & KEY_SELECT)
        {
            legal_mode_ = !legal_mode_;
            changed = true;
        }
        if (tab_ != before_tab)
            row_ = 0;
        if (in.rep & KEY_UP)
            row_ = (row_ + rows - 1) % rows;
        if (in.rep & KEY_DOWN)
            row_ = (row_ + 1) % rows;
        int dir = (in.rep & KEY_RIGHT) ? 1 : (in.rep & KEY_LEFT) ? -1 : 0;
        if (in.held & KEY_Y)
            dir *= 10;  // fast adjust
        if (in.tapped)
        {
            if (in.tap_y < 16)
            {
                tab_ = in.tap_x * kTabs / 256;
                row_ = 0;
            }
            else if ((in.tap_y - row_bar_y(0)) / row_pitch() >= 0 && in.tap_y >= row_bar_y(0) &&
                     (in.tap_y - row_bar_y(0)) / row_pitch() < rows)
            {
                int r = (in.tap_y - row_bar_y(0)) / row_pitch();
                if (r == row_)
                    dir = in.tap_x < 128 ? -1 : 1;
                row_ = r;
            }
            else if (in.tap_y >= 176)
                apply();
        }
        if (dir)
            changed |= adjust(dir);
        if ((in.down & KEY_A) && open_picker())
            return true;
        if (in.down & KEY_START)
            apply();
        if (changed)
            preview();
        if (changed || row_ != before_row || tab_ != before_tab)
            draw();
        ui::bar(0, row_bar_y(row_), true, row_pitch());
        return true;
    }

private:
    uint8_t box_, slot_;
    bool loaded_ = false, gen4_ = true, legal_mode_ = true;
    verity_pkm_view orig_;
    verity_edit e_;
    int tab_ = 0, row_ = 0;
    int items_ = 0, moves_ = 0;
    int picking_ = 0;  // 0 none, 1 item, 2..5 move slot
    bool showing_fix_ = false;
    verity_iv_fix fix_;
    bool typing_ = false, ribbon_list_ = false;
    uint16_t name_[11];
    int name_len_ = 0, key_row_ = 0, key_col_ = 0, key_page_ = 0;
    ListView ribbons_;
    ListView pick_;
    char status_[48] = "";
    verity_legality_report report_{};
    uint8_t issue_ = 0;
    bool preview_ok_ = false;

    static int count(uint32_t cat)
    {
        uint16_t w[4];
        int lo = 0, hi = 4096;
        while (lo < hi)  // first id without a name
        {
            int mid = (lo + hi) / 2;
            if (verity_name(cat, mid, w, 4) || mid == 0)
                lo = mid + 1;
            else
                hi = mid;
        }
        return lo;
    }

    static constexpr int kTabs = 5;
    // Bottom rows from y 24: Stats has 12 rows on the 12 px pitch, the other tabs the grid pitch.
    // The highlight fills the row.
    int row_pitch() const { return tab_ == 1 ? ui::kPitch : ui::kGridPitch; }
    int row_text_y(int r) const { return 24 + 4 + row_pitch() * r; }
    int row_bar_y(int r) const { return row_text_y(r) - (row_pitch() - 8) / 2; }
    void row(int r, const char* label, const char* fmt, ...) __attribute__((format(printf, 4, 5)))
    {
        char v[64];
        va_list ap;
        va_start(ap, fmt);
        vsnprintf(v, sizeof(v), fmt, ap);
        va_end(ap);
        ui::text(bottom(), kListX, row_text_y(r), UI_MUTED "%s", label);
        ui::text(bottom(), 104, row_text_y(r), "%s", v);
    }

    int tab_rows() const
    {
        static constexpr int kRows[kTabs] = {3, 12, 4, 6, 8};
        return kRows[tab_];
    }

    void load_fields()
    {
        e_.ball = orig_.ball;
        e_.met_location = orig_.met_location;
        e_.met_level = orig_.met_level;
        memcpy(e_.met_date, orig_.met_date, 3);
        memcpy(e_.contest, orig_.contest, 6);
        memcpy(e_.ribbons, orig_.ribbons, 12);
    }

    static int clampi(int v, int lo, int hi) { return v < lo ? lo : v > hi ? hi : v; }

    bool adjust(int dir)
    {
        if (tab_ == 0)
        {
            if (row_ == 0)
            {
                e_.held_item = (uint16_t)((e_.held_item + dir + items_) % items_);
                e_.mask |= 1;
            }
            else if (row_ == 1)
            {
                e_.level = (uint8_t)clampi(e_.level + dir, 1, 100);
                e_.mask |= 16;
            }
            else
            {
                e_.friendship = (uint8_t)clampi(e_.friendship + dir, 0, 255);
                e_.mask |= 32;
            }
        }
        else if (tab_ == 1)
        {
            if (row_ < 6)
            {
                e_.ivs[row_] = (uint8_t)clampi(e_.ivs[row_] + dir, 0, 31);
                e_.mask |= 4;
                e_.mask &= ~64u;  // a hand-set spread no longer goes with the proposed PID
            }
            else
            {
                e_.evs[row_ - 6] = (uint8_t)clampi(e_.evs[row_ - 6] + dir * 4, 0, 255);
                e_.mask |= 8;
            }
        }
        else if (tab_ == 2)
        {
            e_.moves[row_] = (uint16_t)((e_.moves[row_] + dir + moves_) % moves_);
            e_.mask |= 2;
        }
        else if (tab_ == 3)
        {
            if (row_ == 0)
            {
                e_.ball = (uint8_t)clampi(e_.ball + (dir > 0 ? 1 : -1), 1, gen4_ ? 24 : 25);
                e_.mask |= 128;
                return true;
            }
            if (row_ == 1)
                e_.met_location = (uint16_t)(e_.met_location + dir);
            else if (row_ == 2)
                e_.met_level = (uint8_t)clampi(e_.met_level + dir, 0, 100);
            else if (row_ == 3)
                e_.met_date[0] = (uint8_t)clampi(e_.met_date[0] + dir, 0, 99);
            else if (row_ == 4)
                e_.met_date[1] = (uint8_t)clampi(e_.met_date[1] + dir, 1, 12);
            else
                e_.met_date[2] = (uint8_t)clampi(e_.met_date[2] + dir, 1, 31);
            e_.mask |= 256;
        }
        else
        {
            if (row_ < 1 || row_ > 6)
                return false;
            e_.contest[row_ - 1] = (uint8_t)clampi(e_.contest[row_ - 1] + dir, 0, 255);
            e_.mask |= 512;
        }
        return true;
    }

    // ---- nickname keyboard -----------------------------------------------------------------

    // Keys are 3 cells wide from column kKeyLeft: ten of them centred in the 32 columns.
    static constexpr int kKeyRows = 4, kKeyCols = 10, kKeyTop = 8, kKeyLeft = 1;
    static constexpr char16_t kDakuten = 0x309B, kHandakuten = 0x309C;

    // Four rows of ten keys per page; 0 is no key. Japanese and Korean follow the games'
    // keyboards: kana with ゛/゜ marks that change the last kana, fullwidth Latin for Japanese, and
    // Hangul typed as jamo that compose into syllables (verity_keyboard_type).
    static const char16_t* page_row(int layout, int page, int row)
    {
        static const char16_t* kLatin[] = {
            u"ABCDEFGHIJ", u"KLMNOPQRST", u"UVWXYZ .,-", u"0123456789",
            u"abcdefghij", u"klmnopqrst", u"uvwxyz .,-", u"0123456789",
            u"!?/:'&()+=", u"*%@#~;_<>^", u"\0\0\0\0\0\0\0\0.,", u"0123456789",
        };
        static const char16_t* kJapanese[] = {
            u"あいうえおかきくけこ", u"さしすせそたちつてと", u"なにぬねのはひふへほ", u"まみむめもやゆよ゛゜",
            u"らりるれろわをんー　", u"ぁぃぅぇぉっゃゅょ・", u"！？…「」、。\0゛゜", u"０１２３４５６７８９",
            u"アイウエオカキクケコ", u"サシスセソタチツテト", u"ナニヌネノハヒフヘホ", u"マミムメモヤユヨ゛゜",
            u"ラリルレロワヲンー　", u"ァィゥェォッャュョ・", u"！？…「」、。\0゛゜", u"０１２３４５６７８９",
            u"ＡＢＣＤＥＦＧＨＩＪ", u"ＫＬＭＮＯＰＱＲＳＴ", u"ＵＶＷＸＹＺ　．，－", u"０１２３４５６７８９",
            u"ａｂｃｄｅｆｇｈｉｊ", u"ｋｌｍｎｏｐｑｒｓｔ", u"ｕｖｗｘｙｚ　．，－", u"０１２３４５６７８９",
        };
        static const char16_t* kKorean[] = {
            u"ㄱㄲㄴㄷㄸㄹㅁㅂㅃㅅ", u"ㅆㅇㅈㅉㅊㅋㅌㅍㅎ ", u"ㅏㅐㅑㅒㅓㅔㅕㅖㅗㅛ", u"ㅜㅠㅡㅣ.,!?-\0",
        };
        if (layout == 1)
            return kJapanese[page * kKeyRows + row];
        if (layout == 2)
            return page == 0 ? kKorean[row] : kLatin[(page - 1) * kKeyRows + row];
        return kLatin[page * kKeyRows + row];
    }

    // 0 Latin, 1 Japanese, 2 Korean (by the Pokémon's language).
    int layout() const { return orig_.language == 1 ? 1 : orig_.language == 8 ? 2 : 0; }
    int page_count() const { return layout() == 1 ? 6 : layout() == 2 ? 4 : 3; }
    int max_name() const { return layout() != 0 ? 5 : 10; }

    bool key_ok(char16_t c) const
    {
        if (c == 0)
            return false;
        if (c == ' ' || c == kDakuten || c == kHandakuten)
            return true;
        return verity_char_encodable(core::doc().info.generation, c) != 0;
    }

    bool open_keyboard()
    {
        if (layout() != 0 && !ui::has_rom_font())
        {
            snprintf(status_, sizeof(status_), "Keyboard: needs a ROM font");
            return false;
        }
        if (key_page_ >= page_count())
            key_page_ = 0;
        typing_ = true;
        ui::bar(0, 0, false);  // the editor's row highlight
        if (e_.mask & 2048)
            memcpy(name_, e_.nickname, sizeof(name_));
        else
        {
            memset(name_, 0, sizeof(name_));
            memcpy(name_, orig_.nickname, 20);
        }
        name_len_ = 0;
        while (name_len_ < 10 && name_[name_len_])
            name_len_++;
        ui::clear(bottom());
        draw_keyboard();
        return true;
    }

    void press_key(int page_row_i, int col)
    {
        char16_t c = page_row(layout(), key_page_, page_row_i)[col];
        if (!key_ok(c))
            return;
        size_t len = (size_t)name_len_;
        verity_keyboard_type(name_, &len, 10, (size_t)max_name(), c, core::doc().info.generation);
        name_len_ = (int)len;
        name_[name_len_] = 0;
    }

    bool update_keyboard(const ui::Input& in)
    {
        bool redraw = false;
        if (in.rep & KEY_UP)
            key_row_ = (key_row_ + kKeyRows - 1) % kKeyRows, redraw = true;
        if (in.rep & KEY_DOWN)
            key_row_ = (key_row_ + 1) % kKeyRows, redraw = true;
        if (in.rep & KEY_LEFT)
            key_col_ = (key_col_ + kKeyCols - 1) % kKeyCols, redraw = true;
        if (in.rep & KEY_RIGHT)
            key_col_ = (key_col_ + 1) % kKeyCols, redraw = true;
        if (in.down & KEY_A)
            press_key(key_row_, key_col_), redraw = true;
        if (in.tapped)
        {
            int r = (in.tap_y / 8 - kKeyTop) / 2;
            int c = (in.tap_x - kKeyLeft * 8) / 24;
            if (in.tap_y / 8 >= kKeyTop && r < kKeyRows && c >= 0 && c < kKeyCols)
            {
                key_row_ = r;
                key_col_ = c;
                press_key(r, c);
                redraw = true;
            }
        }
        if (in.down & KEY_B)
        {
            if (name_len_ > 0)
            {
                name_[--name_len_] = 0;
                redraw = true;
            }
            else
            {
                typing_ = false;
                ui::clear(bottom());
                draw();
                return true;
            }
        }
        if (in.down & KEY_Y)
            key_page_ = (key_page_ + 1) % page_count(), redraw = true;
        if (in.down & KEY_X)
        {
            // Back to the species name, which also clears the nicknamed flag.
            e_.mask = (e_.mask & ~2048u) | 4096;
            typing_ = false;
            ui::clear(bottom());
            preview();
            draw();
            return true;
        }
        if (in.down & KEY_START)
        {
            if (name_len_ == 0)
            {
                ui::clear_px(bottom(), 28, 40);
                ui::text(bottom(), ui::kInsetTop, 28, "A name can't be empty");
                return true;
            }
            memset(e_.nickname, 0, sizeof(e_.nickname));
            memcpy(e_.nickname, name_, sizeof(uint16_t) * (size_t)name_len_);
            e_.mask = (e_.mask & ~4096u) | 2048;
            typing_ = false;
            ui::clear(bottom());
            preview();
            draw();
            return true;
        }
        if (redraw)
            draw_keyboard();
        return true;
    }

    void draw_keyboard()
    {
        char text[48];
        ui::utf16_to_text(name_, text, sizeof(text));
        ui::frame_clear(bottom());
        ui::title(bottom(), "Nickname");
        char n[12];
        snprintf(n, sizeof(n), "%d/%d", name_len_, max_name());
        ui::title_right(bottom(), n);
        ui::panel(bottom(), 1, 2, 30, 3);
        // Text is drawn over what's there: clear the field, or erasing leaves the old characters.
        ui::clear_px(bottom(), 16, 40);
        ui::Lines l(2);
        ui::text(bottom(), ui::kInsetTop, l.line(), "%s_", text);
        ui::text(bottom(), ui::kInsetTop, l.line(), UI_MUTED "%s", layout() == 1 ? "Japanese (Y: next page)"
                                                         : layout() == 2 ? "Korean (Y: next page)"
                                                                         : "Y: next page");
        ui::panel(bottom(), 0, kKeyTop, 32, kKeyRows * 2 - 1);  // keys span the full width
        for (int r = 0; r < kKeyRows; r++)
        {
            uint16_t line[33];
            for (int i = 0; i < 32; i++)
                line[i] = ' ';
            line[32] = 0;
            const char16_t* keys = page_row(layout(), key_page_, r);
            for (int c = 0; c < kKeyCols; c++)
            {
                int x = kKeyLeft + c * 3;
                char16_t k = key_ok(keys[c]) ? keys[c] : u' ';
                // Gen 4 fonts have no standalone voicing marks.
                if ((k == kDakuten || k == kHandakuten) && !ui::has_glyph(k))
                    k = k == kDakuten ? u'"' : u'o';
                line[x + 1] = k;
                if (r == key_row_ && c == key_col_)
                    ui::key_highlight(bottom(), x, kKeyTop + r * 2);
            }
            char text_line[128];
            ui::utf16_to_text(line, text_line, sizeof(text_line));
            at(bottom(), 0, kKeyTop + r * 2, "%s", text_line);
        }
        ui::hints(bottom(), "A:Key|B:Erase|Y:Page|START:Done");
    }

    // ---- ribbons ---------------------------------------------------------------------------

    static constexpr int kRibbons = 80;

    /// Byte and bit (in verity_edit.ribbons) of a ribbon, by list position.
    static void ribbon_pos(int i, int* byte, int* bit)
    {
        // Sinnoh set: 28 bits from byte 0; Hoenn set: 32 bits from byte 4; contest: 20 from byte 8.
        int base = i < 28 ? 0 : i < 60 ? 32 : 64;
        int off = base + (i < 28 ? i : i < 60 ? i - 28 : i - 60);
        *byte = off / 8;
        *bit = off % 8;
    }

    static const char* ribbon_name(int i)
    {
        static const char* kNames[kRibbons] = {
            "Champion Sinnoh", "Ability", "Great Ability", "Double Ability", "Multi Ability",
            "Pair Ability", "World Ability", "Alert", "Shock", "Downcast", "Careless", "Relax",
            "Snooze", "Smile", "Gorgeous", "Royal", "Gorgeous Royal", "Footprint", "Record",
            "Event", "Legend", "Champion World", "Birthday", "Special", "Souvenir", "Wishing",
            "Classic", "Premier",
            "Cool (G3)", "Cool Super", "Cool Hyper", "Cool Master", "Beauty (G3)", "Beauty Super",
            "Beauty Hyper", "Beauty Master", "Cute (G3)", "Cute Super", "Cute Hyper", "Cute Master",
            "Smart (G3)", "Smart Super", "Smart Hyper", "Smart Master", "Tough (G3)", "Tough Super",
            "Tough Hyper", "Tough Master", "Champion G3", "Winning", "Victory", "Artist", "Effort",
            "Battle Champion", "Regional Champion", "National Champion", "Country", "National",
            "Earth", "World",
            "Cool (G4)", "Cool Great", "Cool Ultra", "Cool Master", "Beauty (G4)", "Beauty Great",
            "Beauty Ultra", "Beauty Master", "Cute (G4)", "Cute Great", "Cute Ultra", "Cute Master",
            "Smart (G4)", "Smart Great", "Smart Ultra", "Smart Master", "Tough (G4)", "Tough Great",
            "Tough Ultra", "Tough Master",
        };
        return kNames[i];
    }

    bool ribbon_on(int i) const
    {
        int b, n;
        ribbon_pos(i, &b, &n);
        return e_.ribbons[b] & (1 << n);
    }

    int ribbon_count() const
    {
        int n = 0;
        for (int i = 0; i < kRibbons; i++)
            n += ribbon_on(i);
        return n;
    }

    bool update_ribbons(const ui::Input& in)
    {
        bool changed;
        int a = ribbons_.update(in, &changed);
        ui::bar(0, ribbons_.bar_y(), true, ribbons_.pitch);
        if (in.down & KEY_B)
        {
            ribbon_list_ = false;
            ui::clear(bottom());
            preview();
            draw();
            return true;
        }
        if (a >= 0)
        {
            int b, n;
            ribbon_pos(a, &b, &n);
            e_.ribbons[b] ^= (uint8_t)(1 << n);
            e_.mask |= 1024;
            changed = true;
        }
        if (changed)
            draw_ribbons();
        return true;
    }

    // The highlighted ribbon in the details panel (rows row..row+5): its image from the ROM, name
    // and whether this Pokémon has it.
    void ribbon_details(int row, int i)
    {
        ui::panel(top(), 1, row, 30, 6);
        int y = row * 8;
        ui::clear_px(top(), y, y + 48);
        uint8_t tiles[512];
        uint16_t colors[16];
        bool icon = core::ribbon_icon(i, tiles, colors);
        ui::item_icon(true, 14, y + 8, icon ? tiles : nullptr, colors);
        int x = icon ? 56 : kX;
        ui::text(top(), x, y + 6, "%s", ribbon_name(i));
        ui::badge(top(), x / 8, row + 3, ribbon_on(i) ? ui::Badge::Legal : ui::Badge::Fishy,
                  ribbon_on(i) ? "Has it" : "Not set");
        if (!icon)
            ui::text(top(), kX, y + 38, UI_MUTED "A ROM on the SD card adds images.");
    }

    void draw_ribbons()
    {
        ui::top_art_hide();
        ui::frame_clear_rows(top(), 1, kEdit - 1);  // the taller key-hint panel
        ribbon_details(kDetail, ribbons_.selected);
        ui::frame_clear(bottom());
        ui::title(bottom(), "Ribbons");
        char n[16];
        snprintf(n, sizeof(n), "%d set", ribbon_count());
        ui::title_right(bottom(), n);
        ui::panel(bottom(), 0, 2, 32, 20);
        ui::hints(bottom(), "A:Toggle|L/R:Page|B:Done");
        ui::clear_px(bottom(), 16, 176);
        for (int r = 0; r < ribbons_.rows; r++)
        {
            int i = ribbons_.scroll + r;
            if (i >= kRibbons)
                break;
            int y = ribbons_.text_y(i);
            bool on = ribbon_on(i);
            ui::text(bottom(), kListX, y, on ? "[x]" : UI_MUTED "[ ]");
            ui::text(bottom(), kListX + 24, y, on ? "%s" : UI_MUTED "%s", ribbon_name(i));
        }
    }

    const char* ball_name(uint8_t b) const
    {
        static const char* kApricorn[] = {"Fast", "Level", "Lure", "Heavy", "Love", "Friend", "Moon", "Sport"};
        static char buf[20];
        if (b >= 17 && b <= 24)
        {
            snprintf(buf, sizeof(buf), "%s Ball", kApricorn[b - 17]);
            return buf;
        }
        if (b == 25)
            return "Dream Ball";
        return ui::name(VERITY_NAME_ITEMS4, b);
    }

    void find_closest()
    {
        ui::clear_px(top(), (kEdit + 2) * 8 + 14, (kEdit + 2) * 8 + 24);
        ui::text(top(), kX, (kEdit + 2) * 8 + 14, "Searching for legal IVs...");
        ui::present();
        uint8_t code = verity_closest_ivs(core::doc().buf, core::doc().len, verity_loc{box_, slot_},
                                          e_.ivs, &fix_);
        ui::clear_px(top(), (kEdit + 2) * 8 + 14, (kEdit + 2) * 8 + 24);
        if (code != 0)
        {
            snprintf(status_, sizeof(status_), "%s",
                     code == 12 ? "No legal IVs found nearby" : verity_gen_status_text(code));
            draw();
            return;
        }
        showing_fix_ = true;
        ui::bar(0, 0, false);
        draw_fix();
    }

    void draw_fix()
    {
        static const char* st[6] = {"HP", "Atk", "Def", "Spe", "SpA", "SpD"};
        ui::clear(bottom());
        ui::frame_clear(bottom());
        ui::title(bottom(), "Closest legal IVs");
        char d[24];
        snprintf(d, sizeof(d), "%u point%s away", fix_.distance, fix_.distance == 1 ? "" : "s");
        ui::title_right(bottom(), d);
        // Columns: stat, yours, legal (right-aligned numbers), and a note on changed rows.
        ui::panel(bottom(), 1, 2, 30, 11);
        ui::Lines l(2);
        const int x = ui::kInsetTop, yours = 96, legal = 150;
        int y = l.line();
        ui::text_right(bottom(), yours, y, UI_MUTED "Yours");
        ui::text_right(bottom(), legal, y, UI_MUTED "Legal");
        for (int i = 0; i < 6; i++)
        {
            y = l.line();
            bool same = e_.ivs[i] == fix_.ivs[i];
            ui::text(bottom(), x, y, UI_MUTED "%s", st[i]);
            ui::text_right(bottom(), yours, y, same ? UI_MUTED "%u" : "%u", e_.ivs[i]);
            ui::text_right(bottom(), legal, y, same ? UI_MUTED "%u" : "%u", fix_.ivs[i]);
            if (!same)
                ui::text(bottom(), legal + 16, y, UI_MUTED "changes");
        }
        ui::panel(bottom(), 1, 16, 30, 3);
        ui::Lines n(16);
        ui::text(bottom(), x, n.line(), "Its PID changes too; nature,");
        ui::text(bottom(), x, n.line(), "gender and ability stay.");
        ui::hints(bottom(), "A:Use these|B:Keep mine");
    }

    bool update_fix(const ui::Input& in)
    {
        if (in.down & (KEY_A | KEY_B))
        {
            if (in.down & KEY_A)
            {
                memcpy(e_.ivs, fix_.ivs, 6);
                e_.pid = fix_.pid;
                e_.mask |= 4 | 64;
            }
            showing_fix_ = false;
            ui::clear(bottom());
            preview();
            draw();
        }
        return true;
    }

    bool open_picker()
    {
        if (tab_ == 4 && row_ == 0)
        {
            if (!open_keyboard())
                draw();
            return true;
        }
        if (tab_ == 4 && row_ == 7)
        {
            ribbon_list_ = true;
            ribbons_ = spaced_list();
            ribbons_.count = kRibbons;
            ui::clear(bottom());
            draw_ribbons();
            return true;
        }
        if (tab_ == 0 && row_ == 0)
        {
            picking_ = 1;
            pick_ = spaced_list();
            pick_.count = items_;
            pick_.selected = e_.held_item;
        }
        else if (tab_ == 2)
        {
            picking_ = 2 + row_;
            pick_ = spaced_list();
            pick_.count = moves_;
            pick_.selected = e_.moves[row_];
        }
        else
            return false;
        pick_.clamp();
        draw_picker();
        return true;
    }

    bool update_picker(const ui::Input& in)
    {
        bool changed;
        int a = pick_.update(in, &changed);
        ui::bar(0, pick_.bar_y(), true, 12);
        if (in.down & KEY_B)
        {
            picking_ = 0;
            ui::clear(bottom());
            draw();
            return true;
        }
        if (a >= 0)
        {
            if (picking_ == 1)
            {
                e_.held_item = (uint16_t)a;
                e_.mask |= 1;
            }
            else
            {
                e_.moves[picking_ - 2] = (uint16_t)a;
                e_.mask |= 2;
            }
            picking_ = 0;
            ui::clear(bottom());
            preview();
            draw();
            return true;
        }
        if (changed)
            draw_picker();
        return true;
    }

    // A move in the details panel (rows row..row+9): name, PP and type pill; category pill,
    // power and accuracy 16 px below; the description in the label shade on a 12 px pitch.
    void move_details(int row, uint16_t mv)
    {
        ui::panel(top(), 1, row, 30, 10);
        int y = row * 8;
        ui::clear_px(top(), y, y + 80);
        if (mv == 0)
        {
            ui::text(top(), kX, y, UI_MUTED "No move in this slot.");
            return;
        }
        verity_move_stats m;
        uint8_t gen = gen4_ ? 4 : 5;
        // The name, then PP and the type pill (or PP alone, beside the type icon) at the right.
        ui::Row head(top(), row, kX);
        head.text("%s", ui::name(VERITY_NAME_MOVES, mv));
        if (!core::move_info(gen, mv, &m))
        {
            head.draw();
            ui::text(top(), kX, y + 16, UI_MUTED "Not in Gen %u games.", gen);
            return;
        }
        // The game's type and category icons when the sprite cache has them, else pills.
        uint8_t tt[256], ct[256];
        uint16_t tc[16], cc[16];
        bool icons = m.type_ != 0xFF && m.from_rom && core::type_icon(gen, 0, m.type_, tt, tc) &&
                     core::type_icon(gen, 1, m.category % 3, ct, cc);
        if (m.type_ != 0xFF)
        {
            head.push_right().text("%u " UI_MUTED "PP", m.pp);
            if (!icons)
                head.pill(ui::type_color(m.type_), ui::name(VERITY_NAME_TYPES, m.type_));
        }
        head.draw();
        if (!m.from_rom)
        {
            ui::text(top(), kX, y + 20, UI_MUTED "A Gen %u ROM on the SD card adds", gen);
            ui::text(top(), kX, y + 32, UI_MUTED "power, accuracy and description.");
            return;
        }
        static const char* kCat[3] = {"Physical", "Special", "Status"};
        static const uint16_t kCatColor[3] = {RGB15(27, 9, 5), RGB15(7, 12, 26), RGB15(11, 11, 13)};
        char power[12], acc[16];
        if (m.power == 0)
            snprintf(power, sizeof(power), "-");
        else if (m.power == 1)
            snprintf(power, sizeof(power), "varies");
        else
            snprintf(power, sizeof(power), "%u", m.power);
        if (m.accuracy == 0 || m.accuracy == 101)
            snprintf(acc, sizeof(acc), "sure hit");
        else
            snprintf(acc, sizeof(acc), "%u%%", m.accuracy);
        int col;
        if (icons)
        {
            // Centred on the stats line (y + 16).
            ui::small_icon(0, kX, y + 12, tt, tc);
            ui::small_icon(1, kX + 36, y + 12, ct, cc);
            col = (kX + 36 + 32) / 8;
        }
        else
            col = ui::pill(top(), 2, row + 2, kCatColor[m.category % 3], kCat[m.category % 3]);
        int x = col * 8 + 8;
        char buf[24];
        snprintf(buf, sizeof(buf), UI_MUTED "Power" UI_MUTED " %s", power);
        ui::text(top(), x, y + 16, "%s", buf);
        // Accuracy at the panel's right edge ("sure hit" is wide).
        ui::text_right(top(), 244, y + 16, UI_MUTED "Acc" UI_MUTED " %s", acc);
        char desc[192];
        if (core::move_description(gen, mv, desc, sizeof(desc)))
            // 12 px below the stats row (1.5x the line pitch); four lines end at the panel's edge.
            ui::wrap_px(top(), kX, y + 36, 12, 30 * 8 - kX, 4, desc, true);
    }

    // The held item's icon from a ROM of the save's generation, if there is one; false otherwise
    // (the sprite is hidden).
    bool show_item_icon(bool on_top, int x, int y, uint16_t item)
    {
        uint8_t tiles[512];
        uint16_t colors[16];
        bool ok = item && core::item_icon(gen4_ ? 4 : 5, item, tiles, colors);
        ui::item_icon(on_top, x, y, ok ? tiles : nullptr, colors);
        return ok;
    }

    // An item in the details panel (rows row..row+5): its icon, name and number.
    void item_details(int row, uint16_t item)
    {
        ui::panel(top(), 1, row, 30, 6);
        int y = row * 8;
        ui::clear_px(top(), y, y + 48);
        // The art is the top-left 24x24 of the frame: centred in the panel's 48 px.
        bool icon = show_item_icon(true, 16, y + 12, item);
        if (item == 0)
        {
            ui::text(top(), kX, y + 4, UI_MUTED "No held item.");
            return;
        }
        int x = icon ? 52 : kX, ty = icon ? y + 10 : y + 4;
        ui::text(top(), x, ty, "%s", ui::name(gen4_ ? VERITY_NAME_ITEMS4 : VERITY_NAME_ITEMS5, item));
        ui::text(top(), x, ty + 14, UI_MUTED "Item %u", item);
        if (!icon)
            ui::text(top(), kX, ty + 28, UI_MUTED "A Gen %u ROM on SD adds icons.", gen4_ ? 4 : 5);
    }

    void draw_picker()
    {
        ui::top_art_hide();
        ui::frame_clear_rows(top(), 1, kEdit - 1);  // the taller key-hint panel
        uint32_t cat = picking_ == 1 ? (gen4_ ? VERITY_NAME_ITEMS4 : VERITY_NAME_ITEMS5)
                                     : VERITY_NAME_MOVES;
        ui::item_icon(false, 0, 0, nullptr, nullptr);
        ui::frame_clear(bottom());
        if (picking_ == 1)
            ui::title(bottom(), "Held item");
        else
            ui::title(bottom(), "Move %d", picking_ - 1);
        ui::panel(bottom(), 0, 2, 32, 20);
        ui::hints(bottom(), "A:Choose|L/R:Page|B:Cancel");
        ui::clear_px(bottom(), 16, 176);
        for (int r = 0; r < pick_.rows; r++)
        {
            int i = pick_.scroll + r;
            if (i >= pick_.count)
                break;
            int y = pick_.text_y(i);
            verity_move_stats m;
            if (picking_ >= 2 && i && core::move_info(gen4_ ? 4 : 5, (uint16_t)i, &m) && m.type_ != 0xFF)
            {
                ui::text(bottom(), 12, y, "%s", ui::name(cat, i));
                ui::text(bottom(), 136, y, UI_MUTED "%s", ui::name(VERITY_NAME_TYPES, m.type_));
                char pp[12];
                snprintf(pp, sizeof(pp), "%u " UI_MUTED "PP", m.pp);
                ui::text(bottom(), 244 - ui::text_width(pp), y, "%s", pp);
            }
            else
            {
                ui::text(bottom(), 12, y, UI_MUTED "%d", i);
                ui::text(bottom(), 40, y, "%s", i ? ui::name(cat, i) : "(none)");
            }
        }
        if (picking_ >= 2)
            move_details(kDetail, (uint16_t)pick_.selected);
        else
            item_details(kDetail, (uint16_t)pick_.selected);
    }

    void apply()
    {
        uint8_t code = verity_edit_apply(core::doc().buf, core::doc().len, verity_loc{box_, slot_},
                                         &e_, legal_mode_ ? 0 : 1);
        if (code == 0)
        {
            core::doc().dirty = true;
            orig_ = core::slot(box_, slot_);
            e_.mask = 0;
            load_fields();
            snprintf(status_, sizeof(status_), "applied (save from the menu)");
        }
        else
            snprintf(status_, sizeof(status_), "refused: %s", verity_issue_text(code));
        draw();
        preview();
    }

    // Recomputes the verdict for the edit as it stands, then shows it.
    void preview()
    {
        memset(&report_, 0, sizeof(report_));
        issue_ = 0;
        preview_ok_ = verity_edit_preview(core::doc().buf, core::doc().len, verity_loc{box_, slot_},
                                          &e_, &report_, &issue_) == VERITY_OK;
        show_preview();
    }

    // Spacing prototype (top screen): content starts at column 2 (8 px inside the panel), lines
    // that carry a badge or pill sit on the 8 px grid 16 px apart, plain text uses a 12 px pitch.
    static constexpr int kX = 16;      // text x on top-screen panels
    static constexpr int kDetail = 2;  // first content row of the details panel
    static constexpr int kEdit = 15;   // first content row of the edit panel (rows 15-21)

    // The edit panel: mode, verdict if applied now, status, and what is wrong.
    void show_preview()
    {
        // Seven rows: the badges, then status and what is wrong on two 12 px lines.
        ui::panel(top(), 1, kEdit, 30, 7);
        ui::clear_px(top(), kEdit * 8, (kEdit + 7) * 8);
        int c = ui::badge(top(), 2, kEdit, legal_mode_ ? ui::Badge::Legal : ui::Badge::Safe,
                          legal_mode_ ? "Legal mode" : "Game-safe mode");
        ui::text(top(), c * 8 + 8, kEdit * 8, UI_MUTED "SEL switches");
        int y = (kEdit + 2) * 8;
        const char* status = status_[0] ? status_ : e_.mask ? "* Edited, not applied yet." : "No changes yet.";
        if (!preview_ok_)
        {
            ui::text(top(), kX, y, "Preview unavailable");
            ui::text(top(), kX, y + 14, UI_MUTED "%s", status);
            return;
        }
        const verity_legality_report& r = report_;
        // The verdict (no label, as on the summary), then Game-safe at the panel's right edge.
        ui::Row verdict(top(), kEdit + 2, kX);
        if (r.status == 1)
            verdict.badge(ui::Badge::Legal, "LEGAL");
        else if (r.status == 2)
            verdict.badge(ui::Badge::Invalid, "ILLEGAL");
        else
            verdict.badge(ui::Badge::Fishy, "UNCHECKED");
        verdict.push_right().text(UI_MUTED "Game-safe");
        verdict.badge(issue_ ? ui::Badge::Invalid : ui::Badge::Legal, issue_ ? "NO" : "OK");
        verdict.draw();
        ui::text(top(), kX, y + 14, UI_MUTED "%s", status);
        char why[48] = "";
        if (issue_)
            snprintf(why, sizeof(why), "%s", verity_issue_text(issue_));
        else if (r.status == 2)
        {
            for (int i = 0; i < r.check_count && strlen(why) < 30; i++)
            {
                strcat(why, i ? " " : "");
                strcat(why, core::check_name(r.checks[i]));
            }
            if (!(r.move_ok[0] && r.move_ok[1] && r.move_ok[2] && r.move_ok[3]))
                strcat(why, " Moves");
        }
        if (why[0])
            ui::text(top(), kX, y + 26, "%s", why);
    }

    void draw()
    {
        char nick[32];
        ui::utf16_to_text(orig_.nickname, nick, sizeof(nick));
        ui::clear(top());
        ui::frame_clear(top());
        ui::title(top(), "Edit  %s", nick);
        ui::title_right(top(), ui::name(VERITY_NAME_SPECIES, orig_.species));
        ui::item_icon(true, 0, 0, nullptr, nullptr);
        ui::top_art_hide();
        if (tab_ == 2)
            move_details(kDetail, e_.moves[row_ < 4 ? row_ : 0]);
        else
        {
            // What the keys do, beside the Pokémon (as saved: edits show after Apply).
            ui::panel(top(), 1, kDetail, 30, 11);
            show_pokemon(orig_, 148, 12, false);
            int y = kDetail * 8 + 4;
            ui::text(top(), kX, y, "Left/Right " UI_MUTED "changes it");
            ui::text(top(), kX, y + 12, "Y+Left/Right " UI_MUTED "by tens");
            int next = y + 36;
            if (tab_ == 1)
            {
                int total = 0;
                for (int i = 0; i < 6; i++)
                    total += e_.evs[i];
                ui::text(top(), kX, y + 24, UI_MUTED "EV total " UI_MUTED "%d / 510", total);
                if (total > 510)
                {
                    ui::text(top(), kX, next, UI_RED "Over the limit" UI_RED);
                    next += 12;
                }
            }
            else
                ui::text(top(), kX, y + 24, "A " UI_MUTED "picks from a list");
            ui::text(top(), kX, next, "%s", tab_ == 1 && gen4_ ? "X " UI_MUTED "closest legal IVs"
                                                              : "Tap twice " UI_MUTED "steps a row");
        }
        show_preview();

        static const char* tabs[kTabs] = {"Main", "Stats", "Moves", "Met", "Misc"};
        ui::clear(bottom());
        ui::frame_clear(bottom());
        ui::tabs(bottom(), tabs, kTabs, tab_);
        int rows = tab_rows();
        ui::panel(bottom(), 0, 3, 32, (rows * row_pitch() + 8) / 8 + (tab_ == 3 ? 2 : 0));
        ui::hints(bottom(), tab_ == 1 && gen4_ ? "L/R:Tab|X:Closest IVs|START:Apply"
                                               : "L/R:Tab|A:List|START:Apply");
        uint32_t items = gen4_ ? VERITY_NAME_ITEMS4 : VERITY_NAME_ITEMS5;
        // Beside the Main tab's three rows, at the panel's right edge.
        show_item_icon(false, 220, 34, tab_ == 0 ? e_.held_item : 0);
        if (tab_ == 0)
        {
            row(0, "Item", "< %s >", e_.held_item ? ui::name(items, e_.held_item) : "-");
            row(1, "Level", "< %u >", e_.level);
            row(2, "Friendship", "< %u >", e_.friendship);
        }
        else if (tab_ == 1)
        {
            static const char* st[6] = {"HP", "Atk", "Def", "Spe", "SpA", "SpD"};
            char label[12];
            for (int i = 0; i < 6; i++)
            {
                snprintf(label, sizeof(label), "IV  %s", st[i]);
                row(i, label, "< %2u >  " UI_MUTED "%s", e_.ivs[i], e_.ivs[i] == 31 ? "max" : "");
            }
            for (int i = 0; i < 6; i++)
            {
                snprintf(label, sizeof(label), "EV  %s", st[i]);
                row(6 + i, label, "< %3u >", e_.evs[i]);
            }
        }
        else if (tab_ == 2)
        {
            char label[8];
            for (int m = 0; m < 4; m++)
            {
                snprintf(label, sizeof(label), "Move %d", m + 1);
                row(m, label, "< %s >", e_.moves[m] ? ui::name(VERITY_NAME_MOVES, e_.moves[m]) : "-");
            }
        }
        else if (tab_ == 3)
        {
            row(0, "Ball", "< %s >", ball_name(e_.ball));
            row(1, "Location", "< %s >", ui::location(gen4_ ? 4 : 5, e_.met_location));
            row(2, "Met level", "< %u >", e_.met_level);
            row(3, "Year", "< %u >", 2000 + e_.met_date[0]);
            row(4, "Month", "< %u >", e_.met_date[1]);
            row(5, "Day", "< %u >", e_.met_date[2]);
            ui::text(bottom(), kListX, row_text_y(6) + 4, UI_MUTED "OT ID  %05u / %05u", orig_.tid, orig_.sid);
        }
        else
        {
            static const char* cs[6] = {"Cool", "Beauty", "Cute", "Smart", "Tough", "Sheen"};
            char nick2[32];
            if (e_.mask & 2048)
                ui::utf16_to_text(e_.nickname, nick2, sizeof(nick2));
            else if (e_.mask & 4096)
                snprintf(nick2, sizeof(nick2), "%s", ui::name(VERITY_NAME_SPECIES, orig_.species));
            else
                ui::utf16_to_text(orig_.nickname, nick2, sizeof(nick2));
            row(0, "Nickname", "%s   " UI_MUTED "A to type", nick2);
            for (int i = 0; i < 6; i++)
                row(1 + i, cs[i], "< %3u >", e_.contest[i]);
            row(7, "Ribbons", "%d set   " UI_MUTED "A to list", ribbon_count());
        }
    }
};

}  // namespace

ui::Scene* file_picker() { return new FilePicker(); }
ui::Scene* overview() { return new Overview(); }
ui::Scene* box_view() { return new BoxView(); }
ui::Scene* add_wizard(uint8_t b, uint8_t s) { return new AddWizard(b, s); }
ui::Scene* editor(uint8_t b, uint8_t s) { return new Editor(b, s); }
ui::Scene* gifts() { return new Gifts(); }
ui::Scene* bag() { return new Bag(); }
ui::Scene* unova_link() { return new UnovaLink(); }

}  // namespace scenes
