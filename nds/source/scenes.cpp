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
    bool framed_ = false;
    ListView list_;

    void draw_top(const char* message)
    {
        ui::clear(top());
        ui::frame_clear(top());
        ui::title(top(), "verity");
        ui::title_right(top(), "Gen 4/5 save editor");
        ui::panel(top(), 1, 2, 30, message[0] ? 6 : 4);
        at(top(), 1, 3, "%d save file%s found", count_, count_ == 1 ? "" : "s");
        if (core::doc().loaded())
            at(top(), 1, 4, UI_MUTED "Open" UI_MUTED "  %.26s", strrchr(core::doc().path, '/') + 1);
        if (message[0])
            at(top(), 1, 6, "! %s", message);
        ui::panel(top(), 1, 17, 30, 4);
        if (ui::has_rom_font())
        {
            at(top(), 1, 18, "Names and icons come from");
            at(top(), 1, 19, "your ROM " UI_MUTED "(%s)", core::font_game());
        }
        else
        {
            at(top(), 1, 18, "Put a Gen 4/5 ROM (.nds) on the SD");
            at(top(), 1, 19, "card for JP/KR names and icons.");
        }
        at(top(), 1, 22, UI_MUTED "Drag or L/R to scroll the list");
    }

    void draw_list()
    {
        if (!framed_)
        {
            framed_ = true;
            ui::frame_clear(bottom());
            ui::panel(bottom(), 0, list_.first_row, 32, list_.rows);
            ui::hints(bottom(), "A:Open|L/R:Page|Drag:Scroll");
        }
        ui::title(bottom(), "Choose a save");
        char n[16];
        snprintf(n, sizeof(n), "%d", count_);
        ui::title_right(bottom(), n);
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
                at(bottom(), 0, list_.first_row + r, " %-23.23s  " UI_MUTED "%.7s", name, dir);
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
        char ot[32];
        ui::utf16_to_text(i.ot_name, ot, sizeof(ot));
        ui::clear(top());
        ui::frame_clear(top());
        ui::title(top(), "%.30s", strrchr(d.path, '/') + 1);
        ui::panel(top(), 1, 2, 30, 8);
        at(top(), 1, 2, "%s", ot);
        at(top(), 1, 3, UI_MUTED "%s", core::game_name(i.kind));
        at(top(), 1, 5, UI_MUTED "ID" UI_MUTED "  %05u / %05u", i.tid, i.sid);
        at(top(), 1, 6, UI_MUTED "Played" UI_MUTED "  %u:%02u:%02u", i.played_hours, i.played_minutes,
           i.played_seconds);
        at(top(), 1, 7, UI_MUTED "Party" UI_MUTED "  %u   " UI_MUTED "Boxes" UI_MUTED "  %lu", i.party_count,
           (unsigned long)d.boxes);
        // Unsaved edits leave the checksums in memory stale; saving recomputes them, so show the
        // file's state and say they'll be updated.
        at(top(), 1, 9, UI_MUTED "Checksums");
        if (d.dirty && d.file_checksums_ok)
            ui::badge(top(), 12, 9, ui::Badge::Safe, "updated on save");
        else if (d.dirty)
            ui::badge(top(), 12, 9, ui::Badge::Invalid, "BAD in file, fixed on save");
        else
            ui::badge(top(), 12, 9, i.checksums_valid ? ui::Badge::Legal : ui::Badge::Invalid,
                      i.checksums_valid ? "OK" : "BAD");
        if (!core::data_available())
            at(top(), 1, 12, "(data missing: showing IDs)");
        if (!message_[0] && d.dirty)
            snprintf(message_, sizeof(message_), "Unsaved changes");
        if (message_[0])
        {
            ui::panel(top(), 1, 14, 30, 2);
            at(top(), 1, 14, "%s %s", d.dirty ? "*" : "-", message_);
        }
        ui::clear(bottom());
        ui::frame_clear(bottom());
        ui::title(bottom(), "Save");
        if (d.dirty)
            ui::title_right(bottom(), "* unsaved");
        count_ = 0;
        items_[count_++] = kBoxes;
        items_[count_++] = kGifts;
        if (i.kind == kBlack2White2)
            items_[count_++] = kUnova;
        items_[count_++] = kSave;
        items_[count_++] = kClose;
        ui::panel(bottom(), 0, list_.first_row, 32, count_);
        for (int r = 0; r < count_; r++)
            at(bottom(), 0, list_.first_row + r, " %s", kItems[items_[r]]);
        ui::hints(bottom(), "A:Open|B:Files");
        list_.count = count_;
        list_.rows = count_;
    }

    bool update(const ui::Input& in) override
    {
        bool changed;
        int a = list_.update(in, &changed);
        ui::bar(0, list_.bar_y(), true);
        if (in.down & KEY_B)
            return false;
        int item = a >= 0 ? items_[a] : -1;
        if (item == kBoxes)
        {
            message_[0] = 0;
            ui::push(box_view());
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
        if (item == kClose)
            return false;
        return true;
    }

private:
    enum { kBoxes, kGifts, kSave, kClose, kUnova };
    static constexpr uint8_t kBlack2White2 = 5;
    static constexpr const char* kItems[5] = {"Boxes and party", "Mystery gifts", "Save to SD card",
                                              "Close", "Memory Link and keys"};
    int items_[5];
    int count_ = 0;
    ListView list_;
    char message_[48] = "";
};

// ---- Mystery Gifts --------------------------------------------------------------------------

class Gifts : public ui::Scene
{
public:
    void enter() override
    {
        ui::clear(bottom());
        ui::title(bottom(), "Mystery gifts");
        ui::hints(bottom(), "A:Deliver|L/R:Page|B:Back");
        if (!core::ensure_events_data())
        {
            ui::clear(top());
            ui::title(top(), "Mystery gifts");
            ui::panel(top(), 1, 2, 30, 2);
            at(top(), 1, 2, "Gift data is missing.");
            at(top(), 1, 3, UI_MUTED "B goes back.");
            return;
        }
        ui::panel(bottom(), 0, list_.first_row, 32, list_.rows);
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
        ui::bar(0, list_.bar_y(), count_ > 0);
        if (a >= 0)
            deliver(a);
        return true;
    }

private:
    static constexpr int kMaxEvents = 500;
    verity_event_view events_[kMaxEvents];
    int count_ = 0;
    ListView list_;
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
        for (int r = 0; r < list_.rows; r++)
        {
            int i = list_.scroll + r;
            int row = list_.first_row + r;
            if (i >= count_)
            {
                at(bottom(), 0, row, "%s", "");
                continue;
            }
            const auto& e = events_[i];
            const char* m = e.status ? UI_MUTED : "";
            at(bottom(), 0, row, " %s%.16s", m, gives(e));
            char ot[24];
            ui::utf16_to_text(e.ot, ot, sizeof(ot));
            if (ot[0])
            {
                at(bottom(), 12, row, UI_MUTED "%s", ot);  // at most 7 characters
                at(bottom(), 22, row, UI_MUTED "%05u", e.tid);
            }
            if (region_tag(e.region)[0])
                at(bottom(), 28, row, UI_MUTED "%s", region_tag(e.region));
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
            ui::panel(top(), 1, 2, 30, 2);
            at(top(), 1, 2, "None for this language.");
            at(top(), 1, 3, UI_MUTED "B goes back.");
            return;
        }
        const auto& e = events_[list_.selected];
        char t[37];
        title_of(e, t, sizeof(t));
        ui::panel(top(), 1, 2, 30, 8);
        at(top(), 1, 2, "%.32s", t);
        at(top(), 1, 4, UI_MUTED "Type" UI_MUTED "       %s", kind_name(e.gift_type));
        if (e.species || e.item)
            at(top(), 1, 5, UI_MUTED "%s" UI_MUTED "   %s", e.species ? "Pokemon" : "Item   ", gives(e));
        char ot[24];
        ui::utf16_to_text(e.ot, ot, sizeof(ot));
        if (ot[0])
            at(top(), 1, 6, UI_MUTED "OT" UI_MUTED "          %s   " UI_MUTED "ID" UI_MUTED " %05u", ot, e.tid);
        static const char* regions[] = {"", "North America", "United Kingdom", "Australia",
                                        "Europe"};
        if (e.region >= 1 && e.region <= 4)
            at(top(), 1, 7, UI_MUTED "Region" UI_MUTED "     %s", regions[e.region]);
        if (e.card_id)
            at(top(), 1, 8, UI_MUTED "Card" UI_MUTED "        %u", e.card_id);
        at(top(), 1, 9, UI_MUTED "Status");
        ui::badge(top(), 7, 9, e.status ? ui::Badge::Fishy : ui::Badge::Legal,
                  verity_event_status_text(e.status));
        if (message_[0])
        {
            ui::panel(top(), 1, 12, 30, 2);
            at(top(), 1, 12, "%s", message_);
        }
        ui::panel(top(), 1, 19, 30, 2);
        at(top(), 1, 19, UI_MUTED "Collect it in-game from the");
        at(top(), 1, 20, UI_MUTED "%s", core::doc().info.generation == 5 ? "deliveryman in any Poke Center."
                                                                         : "deliveryman in any Poke Mart.");
    }
};

// ---- Memory Link and Key System ---------------------------------------------------------------

class UnovaLink : public ui::Scene
{
public:
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
            else if ((in.down & KEY_A) && preview_.status == 0)
                link();
            return true;
        }
        bool changed;
        int pick = list_.update(in, &changed);
        if (changed)
            draw_list();
        ui::bar(0, list_.bar_y(), true);
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
    ListView list_;
    uint8_t* other_ = nullptr;
    size_t other_len_ = 0;
    const char* other_path_ = "";
    bool replace_ = false;
    bool enabling_ = false;
    verity_unova_preview_t preview_{};
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
        verity_unova_preview(d.buf, d.len, other_, other_len_, false, &preview_);
        replace_ = preview_.status == 1;
        if (replace_)
            verity_unova_preview(d.buf, d.len, other_, other_len_, true, &preview_);
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
                              : verity_unova_apply(d.buf, d.len, other_, other_len_, replace_, seed);
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
            ui::panel(top(), 1, 2, 30, 4);
            at(top(), 1, 2, "Pick a Black or White save for");
            at(top(), 1, 3, "Memory Link, or a Black 2 or");
            at(top(), 1, 4, "White 2 save to receive its keys.");
            at(top(), 1, 5, UI_MUTED "Or enable every key, no partner.");
            if (message_[0])
            {
                ui::panel(top(), 1, 8, 30, 2);
                at(top(), 1, 8, "%s", message_);
            }
            draw_list();
        }
    }

    void draw_preview()
    {
        static constexpr const char* kKeys[5] = {"Easy Mode", "Challenge Mode", "City swap",
                                                 "Iron Chamber", "Iceberg Chamber"};
        if (other_)
        {
            const char* slash = strrchr(other_path_, '/');
            ui::panel(top(), 1, 2, 30, 2);
            at(top(), 1, 2, "%.30s", slash ? slash + 1 : other_path_);
            at(top(), 1, 3, UI_MUTED "Trainer" UI_MUTED "  %s", trainer_);
        }
        ui::bar(0, 0, false);
        ui::panel(top(), 1, 6, 30, 9);
        if (preview_.status != 0)
            ui::badge(top(), 1, 6, ui::Badge::Invalid, verity_unova_status_text(preview_.status));
        else if (preview_.mode == 1)
        {
            at(top(), 1, 6, "Memory Link");
            at(top(), 1, 7, UI_MUTED "%s", preview_.flashback ? "Unlocks a new flashback" : "No new flashbacks");
            if (replace_)
                at(top(), 1, 9, "%s", verity_unova_status_text(1));
        }
        else
        {
            at(top(), 1, 6, preview_.mode == 3 ? "Keys this will enable:" : "Keys this save will receive:");
            if (preview_.mode == 3)
                at(top(), 1, 14, UI_MUTED "These can't be sent to other games.");
            int row = 7;
            for (int i = 0; i < 5; i++)
                if (preview_.keys[i])
                    ui::badge(top(), 2, row++, ui::Badge::Legal, kKeys[i]);
        }
        ui::title(bottom(), "Link");
        ui::panel(bottom(), 1, 3, 30, 2);
        at(bottom(), 1, 3, "%s", preview_.status == 0 ? "Ready." : "Can't link these saves.");
        at(bottom(), 1, 4, UI_MUTED "Nothing changes until you link.");
        ui::hints(bottom(), preview_.status == 0 ? "A:Link|B:Back" : "B:Back");
    }

    void draw_list()
    {
        ui::title(bottom(), "Link with");
        ui::panel(bottom(), 0, list_.first_row, 32, list_.rows);
        ui::hints(bottom(), "A:Choose|L/R:Page|B:Back");
        for (int r = 0; r < list_.rows; r++)
        {
            int i = list_.scroll + r - 1;
            if (i < 0)
                at(bottom(), 0, list_.first_row + r, " Enable all keys   " UI_MUTED "no partner");
            else if (i < count_)
            {
                const char* slash = strrchr(files_[i], '/');
                at(bottom(), 0, list_.first_row + r, " %.30s", slash ? slash + 1 : files_[i]);
            }
            else
                at(bottom(), 0, list_.first_row + r, "%s", "");
        }
    }
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
            char n[12];
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

    void details(const verity_pkm_view& v)
    {
        ui::clear(top());
        ui::frame_clear(top());
        if (v.species == 0)
        {
            ui::title(top(), "Empty slot");
            ui::panel(top(), 1, 2, 30, 2);
            at(top(), 1, 2, "Nothing here. " UI_MUTED "X or A adds a");
            at(top(), 1, 3, "Pokémon to this slot.");
            return;
        }
        char nick[32], ot[32];
        ui::utf16_to_text(v.nickname, nick, sizeof(nick));
        ui::utf16_to_text(v.ot_name, ot, sizeof(ot));
        const char* sp = ui::name(VERITY_NAME_SPECIES, v.species);
        const char* g = v.gender == 0 ? "M" : v.gender == 1 ? "F" : "-";
        if (v.is_egg)
        {
            ui::title(top(), "Egg");
            ui::panel(top(), 1, 2, 30, 3);
            at(top(), 1, 2, "%s", sp);
            at(top(), 1, 4, UI_MUTED "OT" UI_MUTED "  %s  " UI_MUTED "%05u/%05u", ot, v.tid, v.sid);
            legality_line();
            return;
        }
        ui::title(top(), "%s%s", nick, v.is_shiny ? " *" : "");
        char lv[12];
        snprintf(lv, sizeof(lv), "Lv %u", v.level);
        ui::title_right(top(), lv);
        // Panels sit two rows apart so each keeps its padding.
        ui::panel(top(), 1, 2, 30, 4);
        if (v.form)
            at(top(), 1, 2, "#%03u %s " UI_MUTED "form %u" UI_MUTED "  %s", v.species, sp, v.form, g);
        else
            at(top(), 1, 2, "#%03u %s  %s", v.species, sp, g);
        at(top(), 1, 3, UI_MUTED "Nature" UI_MUTED "  %s", ui::name(VERITY_NAME_NATURES, v.nature));
        at(top(), 1, 4, UI_MUTED "Ability" UI_MUTED "  %s", ui::name(VERITY_NAME_ABILITIES, v.ability));
        uint32_t items = core::doc().info.generation == 4 ? VERITY_NAME_ITEMS4 : VERITY_NAME_ITEMS5;
        at(top(), 1, 5, UI_MUTED "Item" UI_MUTED "  %s", v.held_item ? ui::name(items, v.held_item) : "-");
        ui::panel(top(), 1, 8, 30, 6);
        for (int m = 0; m < 4; m++)
            at(top(), 1 + (m % 2) * 15, 8 + m / 2, "%.14s",
               v.moves[m] ? ui::name(VERITY_NAME_MOVES, v.moves[m]) : "-");
        at(top(), 1, 11, UI_MUTED "      HP  At  Df  Sp  SA  SD");
        at(top(), 1, 12, UI_MUTED "IVs" UI_MUTED " %4u%4u%4u%4u%4u%4u", v.ivs[0], v.ivs[1], v.ivs[2], v.ivs[3],
           v.ivs[4], v.ivs[5]);
        at(top(), 1, 13, UI_MUTED "EVs" UI_MUTED " %4u%4u%4u%4u%4u%4u", v.evs[0], v.evs[1], v.evs[2], v.evs[3],
           v.evs[4], v.evs[5]);
        ui::panel(top(), 1, 16, 30, 3);
        at(top(), 1, 16, UI_MUTED "OT" UI_MUTED "  %s  " UI_MUTED "%05u/%05u", ot, v.tid, v.sid);
        at(top(), 1, 17, UI_MUTED "Met" UI_MUTED "  %.20s Lv %u",
           ui::location(core::doc().info.generation, v.met_location), v.met_level);
        at(top(), 1, 18, UI_MUTED "PID" UI_MUTED "  %08lX%s", (unsigned long)v.pid,
           v.checksum_ok ? "" : "  ! checksum mismatch");
        legality_line();
    }

    void legality_line()
    {
        uint32_t ms = 0;
        verity_legality_report r = party_ ? core::legality(0xFF, cur_, &ms)
                                          : core::legality(box_, cur_, &ms);
        static const char* why[] = {"",        "egg",       "origin game",
                                    "data missing", "Gen 5 save", "hatched",
                                    "event/trade/walker"};
        ui::panel(top(), 1, 21, 30, 2);
        if (r.status == 1)
        {
            int c = ui::badge(top(), 1, 21, ui::Badge::Legal, "LEGAL");
            at(top(), c, 21, UI_MUTED "checked in %lu ms", (unsigned long)ms);
        }
        else if (r.status == 2)
        {
            char list[40] = "";
            for (int i = 0; i < r.check_count && strlen(list) < 24; i++)
            {
                strcat(list, i ? " " : "");
                strcat(list, core::check_name(r.checks[i]));
            }
            bool moves = !(r.move_ok[0] && r.move_ok[1] && r.move_ok[2] && r.move_ok[3]);
            int c = ui::badge(top(), 1, 21, ui::Badge::Invalid, "ILLEGAL");
            at(top(), c, 21, UI_MUTED "%lu ms", (unsigned long)ms);
            at(top(), 1, 22, "%s%s", list, moves ? " Moves" : "");
        }
        else
        {
            ui::badge(top(), 1, 21, ui::Badge::Fishy, "NOT CHECKED");
            at(top(), 1, 22, "%s", r.unsupported < 7 ? why[r.unsupported] : "?");
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
            ui::bar(0, list_.bar_y(), true);
            if (in.down & KEY_B)
                return false;
            if ((in.down & KEY_Y) && !gen5())
            {
                egg_ = !egg_;
                load_species();
                list_ = ListView();
                draw();
                return true;
            }
            if ((in.down & KEY_SELECT) && other_game())
            {
                version_ = other_game();
                load_species();
                list_ = ListView();
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
                    ui::bar(0, 2 * 8, true);
                    return true;
                }
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
                int row = in.tap_y / 8 - 2;
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
                       ((in.down & KEY_A) || (in.tapped && in.tap_y / 8 - 2 == opts - 1))) ||
                      (in.down & KEY_START);
            if (go && (egg_ ? generate_egg() : generate()))
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
    ListView list_, enc_list_;

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
            ui::panel(top(), 1, 2, 30, 2);
            at(top(), 1, 2, "Only Gen 4 saves for now");
            at(top(), 1, 3, UI_MUTED "(D/P/Pt wild encounters).");
            ui::hints(bottom(), "B:Back");
            return;
        }
        ui::title(top(), "Add %s", egg_ ? "Egg" : "Pokemon");
        char g[32];
        snprintf(g, sizeof(g), "legal, %s", game_name(version_));
        ui::title_right(top(), g);
        ui::panel(top(), 1, 2, 30, 2);
        at(top(), 1, 2, UI_MUTED "To" UI_MUTED "  %s %u", box_ == 0xFF ? "party slot" : "box slot",
           (box_ == 0xFF ? slot_ : box_ * 30 + slot_) + 1);
        at(top(), 1, 3, UI_MUTED "Step %d of 3", step_ + 1);
        ui::panel(top(), 1, 6, 30, 4);
        if (step_ == 0)
        {
            at(top(), 1, 6, egg_ ? "Egg species" : "Species");
            at(top(), 1, 7, UI_MUTED "%d %s", species_count_, egg_ ? "can be bred here" : "with wild slots");
            at(top(), 1, 8, UI_MUTED "%s", other_game() ? "SELECT switches game" : "");
            at(top(), 1, 9, UI_MUTED "%s", gen5() ? "" : egg_ ? "Y: wild encounter instead" : "Y: bred egg instead");
            draw_list();
        }
        else if (step_ == 1)
        {
            at(top(), 1, 6, "Encounter");
            at(top(), 1, 7, UI_MUTED "for %s", ui::name(VERITY_NAME_SPECIES, species_[species_ix_]));
            draw_list();
        }
        else
        {
            at(top(), 1, 6, "Options");
            at(top(), 1, 7, UI_MUTED "for %s", ui::name(VERITY_NAME_SPECIES, species_[species_ix_]));
            draw_options();
        }
        ui::hints(bottom(), step_ == 2 ? "A:Next|B:Back|START:Generate" : "A:Next|L/R:Page|B:Back");
    }

    void draw_list()
    {
        if (step_ == 0)
        {
            ui::title(bottom(), "Species");
            ui::panel(bottom(), 0, 2, 32, list_.rows);
            for (int r = 0; r < list_.rows; r++)
            {
                int i = list_.scroll + r;
                if (i < species_count_)
                    at(bottom(), 0, 2 + r, " " UI_MUTED "#%03u" UI_MUTED "  %s", species_[i],
                       ui::name(VERITY_NAME_SPECIES, species_[i]));
                else
                    at(bottom(), 0, 2 + r, "%s", "");
            }
        }
        else
        {
            ui::title(bottom(), "Encounter");
            ui::panel(bottom(), 0, 2, 32, enc_list_.rows);
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
                    at(bottom(), 0, 2 + r, " %-14.14s  " UI_MUTED "%-7.7s" UI_MUTED "  %s",
                       ui::location(gen5() ? 5 : 4, e.location),
                       gen5() ? slot_type5(e.slot_type) : slot_type(e.slot_type), lv);
                }
                else
                    at(bottom(), 0, 2 + r, "%s", "");
            }
        }
    }

    void draw_egg_options()
    {
        ui::title(bottom(), "Options");
        ui::panel(bottom(), 0, 2, 32, 5);
        at(bottom(), 0, 2, " " UI_MUTED "Nature" UI_MUTED "     < %s >", nature_ < 0 ? "Any" : ui::name(VERITY_NAME_NATURES, nature_));
        at(bottom(), 0, 3, " " UI_MUTED "Gender" UI_MUTED "     < %s >", gender_ < 0 ? "Any" : gender_ ? "Female" : "Male");
        at(bottom(), 0, 4, " " UI_MUTED "Shiny" UI_MUTED "       < %s >", shiny_ ? "Yes" : "No");
        at(bottom(), 0, 5, " " UI_MUTED "Hatched" UI_MUTED "    < %s >", hatched_ ? "Yes" : "No");
        at(bottom(), 0, 6, " Generate and place");
        if (status_[0])
        {
            ui::panel(bottom(), 1, 10, 30, 2);
            at(bottom(), 1, 10, "%s", status_);
        }
    }

    void draw_options()
    {
        if (egg_)
        {
            draw_egg_options();
            return;
        }
        ui::title(bottom(), "Options");
        ui::panel(bottom(), 0, 2, 32, 7);
        at(bottom(), 0, 2, " " UI_MUTED "Nature" UI_MUTED "    < %s >", nature_ < 0 ? "Any" : ui::name(VERITY_NAME_NATURES, nature_));
        at(bottom(), 0, 3, " " UI_MUTED "Shiny" UI_MUTED "      < %s >", shiny_ ? "Yes" : "No");
        if (level_)
            at(bottom(), 0, 4, " " UI_MUTED "Level" UI_MUTED "      < %d >", level_);
        else
            at(bottom(), 0, 4, " " UI_MUTED "Level" UI_MUTED "      < Any >");
        at(bottom(), 0, 5, " " UI_MUTED "Gender" UI_MUTED "    < %s >", gender_ < 0 ? "Any" : gender_ ? "Female" : "Male");
        if (gen5())
            at(bottom(), 0, 6, " " UI_MUTED "Lead        not used in Gen 5");
        else
            at(bottom(), 0, 6, " " UI_MUTED "Lead" UI_MUTED "       < %s >", lead_name(lead_));
        at(bottom(), 0, 7, " " UI_MUTED "Ball" UI_MUTED "        < %s >",
           ball_ ? ui::name(VERITY_NAME_ITEMS4, kBalls[ball_ - 1]) : "Default");
        at(bottom(), 0, 8, " Generate and place");
        if (status_[0])
        {
            ui::panel(bottom(), 1, 12, 30, 2);
            at(bottom(), 1, 12, "%s", status_);
        }
    }
};


// ---- editor --------------------------------------------------------------------------------

class Editor : public ui::Scene
{
public:
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
            else if (in.tap_y / 8 - 3 >= 0 && in.tap_y / 8 - 3 < rows)
            {
                int r = in.tap_y / 8 - 3;
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
        ui::bar(0, (3 + row_) * 8, true);
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

    static constexpr int kKeyRows = 4, kKeyCols = 10, kKeyTop = 5;
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
            int c = (in.tap_x - 16) / 24;
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
                at(bottom(), 0, 3, " A name can't be empty");
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
        ui::panel(bottom(), 1, 2, 30, 2);
        at(bottom(), 1, 2, "%s_", text);
        at(bottom(), 1, 3, UI_MUTED "%s", layout() == 1 ? "Japanese (Y: next page)"
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
                int x = 2 + c * 3;
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
        ui::bar(0, ribbons_.bar_y(), true);
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

    // The highlighted ribbon on the top screen (rows row..row+3): its image from the ROM, name and
    // whether this Pokémon has it.
    void ribbon_details(int row, int i)
    {
        ui::panel(top(), 1, row, 30, 4);
        for (int r = row; r < row + 4; r++)
            at(top(), 1, r, "%s", "");
        uint8_t tiles[512];
        uint16_t colors[16];
        bool icon = core::ribbon_icon(i, tiles, colors);
        ui::item_icon(true, 10, row * 8, icon ? tiles : nullptr, colors);
        int c = icon ? 6 : 1;
        at(top(), c, row + 1, "%s", ribbon_name(i));
        ui::badge(top(), c, row + 2, ribbon_on(i) ? ui::Badge::Legal : ui::Badge::Fishy,
                  ribbon_on(i) ? "Has it" : "Not set");
        if (!icon)
            at(top(), 1, row + 3, UI_MUTED "A ROM on the SD card adds images.");
    }

    void draw_ribbons()
    {
        ribbon_details(10, ribbons_.selected);
        ui::frame_clear(bottom());
        ui::title(bottom(), "Ribbons");
        char n[16];
        snprintf(n, sizeof(n), "%d set", ribbon_count());
        ui::title_right(bottom(), n);
        ui::panel(bottom(), 0, 2, 32, ribbons_.rows);
        ui::hints(bottom(), "A:Toggle|L/R:Page|B:Done");
        for (int r = 0; r < ribbons_.rows; r++)
        {
            int i = ribbons_.scroll + r;
            if (ribbon_on(i))
                at(bottom(), 0, 2 + r, " [x]  %s", ribbon_name(i));
            else
                at(bottom(), 0, 2 + r, " " UI_MUTED "[ ]  %s", ribbon_name(i));
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
        at(top(), 0, 7, "Searching for legal IVs...");
        uint8_t code = verity_closest_ivs(core::doc().buf, core::doc().len, verity_loc{box_, slot_},
                                          e_.ivs, &fix_);
        at(top(), 0, 7, "%s", "");
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
        ui::panel(bottom(), 1, 2, 30, 7);
        at(bottom(), 1, 2, UI_MUTED "        Yours   Legal");
        for (int i = 0; i < 6; i++)
        {
            if (e_.ivs[i] == fix_.ivs[i])
                at(bottom(), 1, 3 + i, UI_MUTED "%-3s      %2u      %2u", st[i], e_.ivs[i], fix_.ivs[i]);
            else
                at(bottom(), 1, 3 + i, UI_MUTED "%-3s" UI_MUTED "      %2u  ->  %2u   " UI_MUTED "changes",
                   st[i], e_.ivs[i], fix_.ivs[i]);
        }
        ui::panel(bottom(), 1, 12, 30, 2);
        at(bottom(), 1, 12, "Its PID changes too; nature,");
        at(bottom(), 1, 13, "gender and ability stay.");
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
            ribbons_ = ListView();
            ribbons_.count = kRibbons;
            ui::clear(bottom());
            draw_ribbons();
            return true;
        }
        if (tab_ == 0 && row_ == 0)
        {
            picking_ = 1;
            pick_ = ListView();
            pick_.count = items_;
            pick_.selected = e_.held_item;
        }
        else if (tab_ == 2)
        {
            picking_ = 2 + row_;
            pick_ = ListView();
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
        ui::bar(0, pick_.bar_y(), true);
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

    // A move in a panel on the top screen (rows row..row+5): the name with its type pill; the
    // category pill with power, accuracy and PP; the description in the label shade, wrapped to
    // four rows.
    void move_details(int row, uint16_t mv)
    {
        ui::panel(top(), 1, row, 30, 6);
        for (int r = row; r < row + 6; r++)
            at(top(), 1, r, "%s", "");
        if (mv == 0)
        {
            at(top(), 1, row, UI_MUTED "No move in this slot.");
            return;
        }
        verity_move_stats m;
        uint8_t gen = gen4_ ? 4 : 5;
        const char* name = ui::name(VERITY_NAME_MOVES, mv);
        if (!core::move_info(gen, mv, &m))
        {
            at(top(), 1, row, "%s", name);
            at(top(), 1, row + 1, UI_MUTED "Not in Gen %u games.", gen);
            return;
        }
        at(top(), 1, row, "%s", name);
        if (m.type_ != 0xFF)
        {
            // At the right edge: "n PP", then the type pill.
            const char* type = ui::name(VERITY_NAME_TYPES, m.type_);
            int start = 31 - (ui::text_width(type) + 15) / 8;
            char pp[16];
            snprintf(pp, sizeof(pp), "%u " UI_MUTED "PP", m.pp);
            // at() clears the rest of its row, so the pill goes last.
            at(top(), start - 1 - (ui::text_width(pp) + 7) / 8, row, "%s", pp);
            ui::pill(top(), start, row, ui::type_color(m.type_), type);
        }
        if (!m.from_rom)
        {
            at(top(), 1, row + 2, UI_MUTED "A Gen %u ROM on the SD card adds", gen);
            at(top(), 1, row + 3, UI_MUTED "power, accuracy and a description.");
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
        // Left to right by measured width (fonts differ), PP at the right edge.
        int col = ui::pill(top(), 1, row + 1, kCatColor[m.category % 3], kCat[m.category % 3]) + 1;
        char buf[24];
        snprintf(buf, sizeof(buf), UI_MUTED "Power" UI_MUTED " %s", power);
        at(top(), col, row + 1, "%s", buf);
        col += (ui::text_width(buf) + 7) / 8 + 1;
        snprintf(buf, sizeof(buf), UI_MUTED "Acc" UI_MUTED " %s", acc);
        at(top(), col, row + 1, "%s", buf);
        char desc[192];
        if (core::move_description(gen, mv, desc, sizeof(desc)))
            ui::wrap(top(), 1, row + 2, 236, 4, desc, true);
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

    // An item in a panel on the top screen (rows row..row+3): its icon, name and number.
    void item_details(int row, uint16_t item)
    {
        ui::panel(top(), 1, row, 30, 4);
        for (int r = row; r < row + 4; r++)
            at(top(), 1, r, "%s", "");
        // The art is the top-left 24x24 of the frame: centred in the panel's 32 px.
        bool icon = show_item_icon(true, 12, row * 8 + 4, item);
        if (item == 0)
        {
            at(top(), 1, row, UI_MUTED "No held item.");
            return;
        }
        // Beside the icon, or at the top with a note when there is none.
        int c = icon ? 5 : 1, r = icon ? row + 1 : row;
        at(top(), c, r, "%s", ui::name(gen4_ ? VERITY_NAME_ITEMS4 : VERITY_NAME_ITEMS5, item));
        at(top(), c, r + 1, UI_MUTED "Item %u", item);
        if (!icon)
        {
            at(top(), 1, row + 2, UI_MUTED "A Gen %u ROM on the SD card adds", gen4_ ? 4 : 5);
            at(top(), 1, row + 3, UI_MUTED "the item's icon.");
        }
    }

    void draw_picker()
    {
        uint32_t cat = picking_ == 1 ? (gen4_ ? VERITY_NAME_ITEMS4 : VERITY_NAME_ITEMS5)
                                     : VERITY_NAME_MOVES;
        ui::item_icon(false, 0, 0, nullptr, nullptr);
        ui::frame_clear(bottom());
        if (picking_ == 1)
            ui::title(bottom(), "Held item");
        else
            ui::title(bottom(), "Move %d", picking_ - 1);
        ui::panel(bottom(), 0, 2, 32, pick_.rows);
        ui::hints(bottom(), "A:Choose|L/R:Page|B:Cancel");
        for (int r = 0; r < pick_.rows; r++)
        {
            int i = pick_.scroll + r;
            if (i < pick_.count)
            {
                verity_move_stats m;
                if (picking_ >= 2 && i && core::move_info(gen4_ ? 4 : 5, (uint16_t)i, &m) && m.type_ != 0xFF)
                    at(bottom(), 0, 2 + r, " %-14.14s  " UI_MUTED "%-8.8s  %2u PP", ui::name(cat, i),
                       ui::name(VERITY_NAME_TYPES, m.type_), m.pp);
                else
                    at(bottom(), 0, 2 + r, " " UI_MUTED "%3d" UI_MUTED "  %s", i, i ? ui::name(cat, i) : "(none)");
            }
            else
                at(bottom(), 0, 2 + r, "%s", "");
        }
        if (picking_ >= 2)
            move_details(10, (uint16_t)pick_.selected);
        else
            item_details(10, (uint16_t)pick_.selected);
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

    void show_preview()
    {
        ui::panel(top(), 1, 18, 30, 4);
        for (int r = 18; r < 22; r++)
            at(top(), 1, r, "%s", "");
        at(top(), 1, 18, UI_MUTED "If applied now");
        if (!preview_ok_)
        {
            at(top(), 1, 19, "Preview unavailable");
            return;
        }
        at(top(), 1, 19, UI_MUTED "Game-safe");
        if (issue_)
        {
            int c = ui::badge(top(), 11, 19, ui::Badge::Invalid, "NO");
            at(top(), c, 19, "%.18s", verity_issue_text(issue_));
        }
        else
            ui::badge(top(), 11, 19, ui::Badge::Legal, "OK");
        const verity_legality_report& r = report_;
        at(top(), 1, 20, UI_MUTED "Legality");
        if (r.status == 1)
            ui::badge(top(), 11, 20, ui::Badge::Legal, "LEGAL");
        else if (r.status == 2)
        {
            char list[40] = "";
            for (int i = 0; i < r.check_count && strlen(list) < 26; i++)
            {
                strcat(list, i ? " " : "");
                strcat(list, core::check_name(r.checks[i]));
            }
            bool mv = !(r.move_ok[0] && r.move_ok[1] && r.move_ok[2] && r.move_ok[3]);
            ui::badge(top(), 11, 20, ui::Badge::Invalid, "ILLEGAL");
            at(top(), 1, 21, "%s%s", list, mv ? " Moves" : "");
        }
        else
            ui::badge(top(), 11, 20, ui::Badge::Fishy, "NOT CHECKED");
    }

    void draw()
    {
        char nick[32];
        ui::utf16_to_text(orig_.nickname, nick, sizeof(nick));
        ui::clear(top());
        ui::frame_clear(top());
        ui::title(top(), "Edit  %s", nick);
        ui::title_right(top(), ui::name(VERITY_NAME_SPECIES, orig_.species));
        ui::panel(top(), 1, 2, 30, 2);
        int c = ui::badge(top(), 1, 2, legal_mode_ ? ui::Badge::Legal : ui::Badge::Safe,
                          legal_mode_ ? "Legal mode" : "Game-safe mode");
        at(top(), c + 1, 2, UI_MUTED "SELECT switches");
        at(top(), 1, 3, UI_MUTED "%s", legal_mode_ ? "Refuses edits that make it illegal."
                                                  : "Allows anything the game accepts.");
        ui::panel(top(), 1, 6, 30, 2);
        at(top(), 1, 6, "%s", e_.mask ? "* Edited, not applied yet" : "No changes yet");
        at(top(), 1, 7, UI_MUTED "%s", status_[0] ? status_ : "START applies, B goes back.");
        ui::item_icon(true, 0, 0, nullptr, nullptr);
        if (tab_ == 2)
            move_details(10, e_.moves[row_ < 4 ? row_ : 0]);
        else
        {
        ui::panel(top(), 1, 10, 30, 4);
        at(top(), 1, 10, UI_MUTED "Left/Right" UI_MUTED "  change the value");
        at(top(), 1, 11, UI_MUTED "Y+Left/Right" UI_MUTED "  in tens");
        at(top(), 1, 12, UI_MUTED "A" UI_MUTED "  pick from a list (item, moves)");
        at(top(), 1, 13, UI_MUTED "%s", tab_ == 1 && gen4_ ? "X  closest legal IVs" : "Tap a row twice to step it");
        }
        show_preview();

        static const char* tabs[kTabs] = {"Main", "Stats", "Moves", "Met", "Misc"};
        ui::clear(bottom());
        ui::frame_clear(bottom());
        ui::tabs(bottom(), tabs, kTabs, tab_);
        int rows = tab_rows();
        ui::panel(bottom(), 0, 3, 32, tab_ == 1 ? 14 : rows < 2 ? 2 : (tab_ == 3 ? 8 : rows));
        ui::hints(bottom(), tab_ == 1 && gen4_ ? "L/R:Tab|X:Closest IVs|START:Apply"
                                               : "L/R:Tab|A:List|START:Apply");
        uint32_t items = gen4_ ? VERITY_NAME_ITEMS4 : VERITY_NAME_ITEMS5;
        // Beside the Main tab's three rows, at the panel's right edge.
        show_item_icon(false, 228, 24, tab_ == 0 ? e_.held_item : 0);
        if (tab_ == 0)
        {
            at(bottom(), 0, 3, " " UI_MUTED "Item" UI_MUTED "         < %s >",
               e_.held_item ? ui::name(items, e_.held_item) : "-");
            at(bottom(), 0, 4, " " UI_MUTED "Level" UI_MUTED "         < %u >", e_.level);
            at(bottom(), 0, 5, " " UI_MUTED "Friendship" UI_MUTED "   < %u >", e_.friendship);
        }
        else if (tab_ == 1)
        {
            static const char* st[6] = {"HP", "Atk", "Def", "Spe", "SpA", "SpD"};
            for (int i = 0; i < 6; i++)
                at(bottom(), 0, 3 + i, " " UI_MUTED "IV" UI_MUTED "  %-3s       < %2u >   " UI_MUTED "%s",
                   st[i], e_.ivs[i], e_.ivs[i] == 31 ? "max" : "");
            int total = 0;
            for (int i = 0; i < 6; i++)
            {
                at(bottom(), 0, 9 + i, " " UI_MUTED "EV" UI_MUTED "  %-3s       < %3u >", st[i], e_.evs[i]);
                total += e_.evs[i];
            }
            at(bottom(), 0, 16, " " UI_MUTED "EV total" UI_MUTED "  %d / 510%s", total,
               total > 510 ? "  (over the limit)" : "");
        }
        else if (tab_ == 2)
        {
            for (int m = 0; m < 4; m++)
                at(bottom(), 0, 3 + m, " " UI_MUTED "Move %d" UI_MUTED "      < %s >", m + 1,
                   e_.moves[m] ? ui::name(VERITY_NAME_MOVES, e_.moves[m]) : "-");
        }
        else if (tab_ == 3)
        {
            at(bottom(), 0, 3, " " UI_MUTED "Ball" UI_MUTED "         < %s >", ball_name(e_.ball));
            at(bottom(), 0, 4, " " UI_MUTED "Location" UI_MUTED "   < %s >",
               ui::location(gen4_ ? 4 : 5, e_.met_location));
            at(bottom(), 0, 5, " " UI_MUTED "Met level" UI_MUTED "   < %u >", e_.met_level);
            at(bottom(), 0, 6, " " UI_MUTED "Year" UI_MUTED "         < %u >", 2000 + e_.met_date[0]);
            at(bottom(), 0, 7, " " UI_MUTED "Month" UI_MUTED "        < %u >", e_.met_date[1]);
            at(bottom(), 0, 8, " " UI_MUTED "Day" UI_MUTED "           < %u >", e_.met_date[2]);
            at(bottom(), 0, 10, " " UI_MUTED "OT ID  %05u / %05u", orig_.tid, orig_.sid);
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
            at(bottom(), 0, 3, " " UI_MUTED "Nickname" UI_MUTED "    %s   " UI_MUTED "A to type", nick2);
            for (int i = 0; i < 6; i++)
                at(bottom(), 0, 4 + i, " " UI_MUTED "%-6s" UI_MUTED "       < %3u >", cs[i], e_.contest[i]);
            at(bottom(), 0, 10, " " UI_MUTED "Ribbons" UI_MUTED "     %d set   " UI_MUTED "A to list", ribbon_count());
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
ui::Scene* unova_link() { return new UnovaLink(); }

}  // namespace scenes
