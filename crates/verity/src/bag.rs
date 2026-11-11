//! The bag screen's entry points (listing pouches, adding, changing counts and removing items),
//! shared by the DS (`verity-ffi`) and the browser (`verity-wasm`).
//!
//! Pouches hold only the items the game puts in them (the game blob's `pouch_items`, from PKHeX
//! `IItemStorage.GetItems` without unreleased items). Every write leaves the pouch packed from
//! slot 0, as the games keep it, and the TM and Berry pouches in the game's order.
//!
//! Event key items are read-only: the Member Card, Oak's Letter, Azure Flute and Secret Key
//! (PKHeX `GiftType4` 8, 9, 10 and 12), the Enigma Stone (a Gen 4 item card) and the Liberty
//! Pass (a Gen 5 item card). The game unlocks their events when its deliveryman hands them over
//! (the Liberty Pass also has its own activation value, PKHeX `Misc5.IsLibertyTicketActivated`),
//! so an item placed in the bag would not unlock anything. They come from the Mystery Gift
//! screen instead.

use crate::data::GameData;
use crate::save::bag::{PouchDef, PouchKind};
use crate::save::{Save, SaveError};

/// Largest pouch (Gen 5 Items).
const MAX_SLOTS: usize = 261;

const EVENT_ITEMS_4: [u16; 5] = [452, 454, 455, 467, 536];
const EVENT_ITEMS_5: [u16; 1] = [574];

/// Why a bag change was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    NoSuchPouch,
    NoSuchSlot,
    /// The item can't go in this pouch (or this game's blob has no pouch lists).
    NotInPouch,
    /// An event key item; see the module docs.
    EventItem,
    /// Already in the pouch; change its count instead.
    AlreadyInPouch,
    /// Outside 1 to the pouch's maximum.
    Count,
    /// No free slot.
    Full,
}

impl Refusal {
    pub fn text(self) -> &'static str {
        match self {
            Refusal::NoSuchPouch => "There is no such pouch.",
            Refusal::NoSuchSlot => "There is no item in that slot.",
            Refusal::NotInPouch => "That item doesn't go in this pouch.",
            Refusal::EventItem => {
                "This is an event item. It comes from Mystery Gift, which also unlocks its event."
            }
            Refusal::AlreadyInPouch => {
                "That item is already in the pouch. Change its count instead."
            }
            Refusal::Count => "That count is outside what the pouch allows.",
            Refusal::Full => "The pouch is full.",
        }
    }
}

impl From<SaveError> for Refusal {
    fn from(_: SaveError) -> Self {
        Refusal::NoSuchSlot
    }
}

/// True for the event key items, which can't be added, changed or removed here.
pub fn is_event_item(generation: u8, item: u16) -> bool {
    match generation {
        4 => EVENT_ITEMS_4.contains(&item),
        5 => EVENT_ITEMS_5.contains(&item),
        _ => false,
    }
}

/// The pouch's name as the games show it.
pub fn pouch_name(kind: PouchKind) -> &'static str {
    match kind {
        PouchKind::Items => "Items",
        PouchKind::KeyItems => "Key Items",
        PouchKind::TmHms => "TMs & HMs",
        PouchKind::Mail => "Mail",
        PouchKind::Medicine => "Medicine",
        PouchKind::Berries => "Berries",
        PouchKind::Balls => "Poké Balls",
        PouchKind::BattleItems => "Battle Items",
    }
}

/// One item in a pouch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Entry {
    pub item: u16,
    pub count: u16,
    /// An event item (see [`is_event_item`]).
    pub locked: bool,
}

fn def(save: &Save, pouch: usize) -> Result<PouchDef, Refusal> {
    save.pouches()
        .get(pouch)
        .copied()
        .ok_or(Refusal::NoSuchPouch)
}

/// The pouch's items in slot order (empty slots skipped).
pub fn entries<'s>(
    save: &'s Save<'_>,
    pouch: usize,
) -> Result<impl Iterator<Item = Entry> + 's, Refusal> {
    let d = def(save, pouch)?;
    let generation = save.generation();
    Ok((0..d.slots as usize).filter_map(move |s| {
        let (item, count) = save.bag_item(pouch, s).ok()?;
        (item != 0).then(|| Entry {
            item,
            count,
            locked: is_event_item(generation, item),
        })
    }))
}

/// The most of one item the pouch holds.
pub fn max_count(save: &Save, pouch: usize) -> Result<u16, Refusal> {
    Ok(def(save, pouch)?.max_count)
}

/// Items that can be added to the pouch: those it holds, minus event items and items already in
/// it, in the game's order.
pub fn addable<'s>(
    save: &'s Save<'_>,
    game: &'s GameData<'_>,
    pouch: usize,
) -> Result<impl Iterator<Item = u16> + 's, Refusal> {
    def(save, pouch)?;
    let list = game.pouch_items(pouch);
    let generation = save.generation();
    Ok((0..list.len()).filter_map(move |i| {
        let item = list.get(i)?;
        let present = entries(save, pouch).ok()?.any(|e| e.item == item);
        (!present && !is_event_item(generation, item)).then_some(item)
    }))
}

/// Position of `item` in the pouch's game order, if the pouch holds it.
fn order(game: &GameData, pouch: usize, item: u16) -> Option<usize> {
    let list = game.pouch_items(pouch);
    (0..list.len()).find(|&i| list.get(i) == Some(item))
}

/// Reads the pouch's items packed, applies `change`, sorts if the game keeps the pouch sorted,
/// and writes it back packed.
fn rewrite(
    save: &mut Save,
    game: &GameData,
    pouch: usize,
    change: impl FnOnce(&mut [(u16, u16); MAX_SLOTS], &mut usize) -> Result<(), Refusal>,
) -> Result<(), Refusal> {
    let d = def(save, pouch)?;
    let slots = d.slots as usize;
    let mut items = [(0u16, 0u16); MAX_SLOTS];
    let mut n = 0;
    for s in 0..slots {
        let e = save.bag_item(pouch, s)?;
        if e.0 != 0 {
            items[n] = e;
            n += 1;
        }
    }
    change(&mut items, &mut n)?;
    if n > slots {
        return Err(Refusal::Full);
    }
    if matches!(d.kind, PouchKind::TmHms | PouchKind::Berries) {
        items[..n].sort_by_key(|&(item, _)| order(game, pouch, item).unwrap_or(usize::MAX));
    }
    // Slots from `n` on are (0, 0): `items` starts zeroed and `remove` clears what it frees.
    for (s, &(item, count)) in items.iter().enumerate().take(slots) {
        save.set_bag_item(pouch, s, item, count)?;
    }
    Ok(())
}

/// Index among the pouch's items (as listed by [`entries`]) to a packed position, checking the
/// item isn't locked.
fn unlocked(save: &Save, pouch: usize, index: usize) -> Result<Entry, Refusal> {
    let e = entries(save, pouch)?
        .nth(index)
        .ok_or(Refusal::NoSuchSlot)?;
    if e.locked {
        return Err(Refusal::EventItem);
    }
    Ok(e)
}

/// Adds `count` of `item` to the pouch.
pub fn add(
    save: &mut Save,
    game: &GameData,
    pouch: usize,
    item: u16,
    count: u16,
) -> Result<(), Refusal> {
    let max = max_count(save, pouch)?;
    if is_event_item(save.generation(), item) {
        return Err(Refusal::EventItem);
    }
    if order(game, pouch, item).is_none() {
        return Err(Refusal::NotInPouch);
    }
    if entries(save, pouch)?.any(|e| e.item == item) {
        return Err(Refusal::AlreadyInPouch);
    }
    if count == 0 || count > max {
        return Err(Refusal::Count);
    }
    rewrite(save, game, pouch, |items, n| {
        let slot = items.get_mut(*n).ok_or(Refusal::Full)?;
        *slot = (item, count);
        *n += 1;
        Ok(())
    })
}

/// Sets the count of the `index`th item in the pouch (as listed by [`entries`]).
pub fn set_count(
    save: &mut Save,
    game: &GameData,
    pouch: usize,
    index: usize,
    count: u16,
) -> Result<(), Refusal> {
    let max = max_count(save, pouch)?;
    let e = unlocked(save, pouch, index)?;
    if count == 0 || count > max {
        return Err(Refusal::Count);
    }
    rewrite(save, game, pouch, |items, n| {
        let slot = items[..*n]
            .iter_mut()
            .find(|s| s.0 == e.item)
            .ok_or(Refusal::NoSuchSlot)?;
        slot.1 = count;
        Ok(())
    })
}

/// Removes the `index`th item in the pouch (as listed by [`entries`]).
pub fn remove(save: &mut Save, game: &GameData, pouch: usize, index: usize) -> Result<(), Refusal> {
    let e = unlocked(save, pouch, index)?;
    rewrite(save, game, pouch, |items, n| {
        let at = items[..*n]
            .iter()
            .position(|s| s.0 == e.item)
            .ok_or(Refusal::NoSuchSlot)?;
        items.copy_within(at + 1..*n, at);
        *n -= 1;
        items[*n] = (0, 0);
        Ok(())
    })
}
