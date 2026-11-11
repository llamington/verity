//! The "Memory Link and keys" screen's entry points, shared by the DS (`verity-ffi`) and the
//! browser (`verity-wasm`): what linking another save into the open Black 2 or White 2 save would
//! do, linking it, and enabling every key without a partner.
//!
//! The open save's checksums are recomputed first, as saving does: unsaved edits leave them
//! stale, and the link code would otherwise take the save for a corrupt one.

use super::keys::{self, KEY_COUNT};
use super::{MemLinkError, Seeds, apply, build, preview};
use crate::save::{Save, SaveKind, detect};

const RAW_SAVE_LEN: usize = 0x80000;

/// The key names, in `keys` order.
pub const KEY_NAMES: [&str; KEY_COUNT] = [
    "Easy Mode",
    "Challenge Mode",
    "City swap",
    "Iron Chamber",
    "Iceberg Chamber",
];

/// What the other save is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Mode {
    /// Not a Black, White, Black 2 or White 2 save.
    Unusable = 0,
    /// A Black or White save: Memory Link.
    MemoryLink = 1,
    /// A Black 2 or White 2 save: receive its keys.
    Keys = 2,
    /// No partner: enable every key.
    EnableAll = 3,
}

/// Whether linking can go ahead (C codes: the discriminants).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Status {
    Ready = 0,
    /// Ready, but the link replaces another player's (the game asks first).
    ReplacesOther = 1,
    NoNewKeys = 2,
    WrongFile = 3,
    GiftsCorrupt = 4,
    KeysCorrupt = 5,
}

impl Status {
    pub fn from_code(code: u8) -> Self {
        use Status::*;
        [
            Ready,
            ReplacesOther,
            NoNewKeys,
            WrongFile,
            GiftsCorrupt,
            KeysCorrupt,
        ]
        .get(usize::from(code))
        .copied()
        .unwrap_or(WrongFile)
    }

    /// Whether a link can go ahead.
    pub fn ready(self) -> bool {
        matches!(self, Status::Ready | Status::ReplacesOther)
    }

    pub fn text(self) -> &'static str {
        let t = self.c_text();
        &t[..t.len() - 1]
    }

    /// [`Status::text`], NUL-terminated for C.
    pub fn c_text(self) -> &'static str {
        match self {
            Status::Ready => "Ready\0",
            Status::ReplacesOther => "Replaces another player's link\0",
            Status::NoNewKeys => "No new keys to enable\0",
            Status::WrongFile => "Needs a valid B/W or B2/W2 save\0",
            Status::GiftsCorrupt => "This save's gift data is corrupt\0",
            Status::KeysCorrupt => "This save's link data is corrupt\0",
        }
    }
}

fn status_of(error: MemLinkError) -> Status {
    match error {
        MemLinkError::DifferentPlayer => Status::ReplacesOther,
        MemLinkError::GiftsCorrupt => Status::GiftsCorrupt,
        MemLinkError::KeysCorrupt => Status::KeysCorrupt,
        MemLinkError::NotBlackWhite | MemLinkError::NotBlack2White2 => Status::WrongFile,
    }
}

/// What a link would do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Plan {
    pub mode: Mode,
    pub status: Status,
    /// Memory Link: unlocks a flashback this save doesn't have.
    pub flashback: bool,
    /// Keys: the ones this link enables, by [`KEY_NAMES`].
    pub keys: [bool; KEY_COUNT],
}

impl Plan {
    fn new(mode: Mode) -> Self {
        Plan {
            mode,
            status: Status::Ready,
            flashback: false,
            keys: [false; KEY_COUNT],
        }
    }
}

/// Recomputes the open save's checksums (as saving does).
fn settle(save: &mut [u8]) {
    if let Ok(mut s) = Save::open(save) {
        s.finalize();
    }
}

fn mode_of(other: &[u8]) -> Mode {
    match other.get(..RAW_SAVE_LEN).and_then(detect) {
        Some(SaveKind::BW) => Mode::MemoryLink,
        Some(SaveKind::B2W2) => Mode::Keys,
        _ => Mode::Unusable,
    }
}

/// What linking `other` into `save` would do. With `replace`, a Memory Link replaces another
/// player's; without, that shows as [`Status::ReplacesOther`] (and the plan is the replacing
/// one's). Changes only the save's checksums.
pub fn plan(save: &mut [u8], other: &[u8]) -> Plan {
    settle(save);
    let mut out = Plan::new(mode_of(other));
    match out.mode {
        Mode::MemoryLink => {
            let first = build(other).and_then(|data| {
                preview(save, &data, false).or_else(|e| {
                    if e == MemLinkError::DifferentPlayer {
                        out.status = Status::ReplacesOther;
                        preview(save, &data, true)
                    } else {
                        Err(e)
                    }
                })
            });
            match first {
                Ok(applied) => out.flashback = applied.unlocked_flashback,
                Err(e) => out.status = status_of(e),
            }
        }
        Mode::Keys => match keys::read(save).and_then(|s| Ok((s, keys::read(other)?))) {
            Ok((target, partner)) => {
                out.keys = keys::receivable(&target, &partner);
                if !out.keys.contains(&true) {
                    out.status = Status::NoNewKeys;
                }
            }
            Err(e) => out.status = status_of(e),
        },
        Mode::Unusable | Mode::EnableAll => out.status = Status::WrongFile,
    }
    out
}

/// Links `other` into `save`, replacing another player's Memory Link (as [`plan`] said). `seed`
/// stands in for the game's frame counter. The save is not finalised.
pub fn link(save: &mut [u8], other: &[u8], seed: u32) -> Result<(), Status> {
    let p = plan(save, other);
    if !p.status.ready() {
        return Err(p.status);
    }
    let result = match p.mode {
        Mode::MemoryLink => build(other).and_then(|data| {
            let seeds = Seeds {
                gifts: (seed >> 24) as u8,
                key_info: seed,
            };
            apply(save, &data, seeds, p.status == Status::ReplacesOther).map(|_| ())
        }),
        Mode::Keys => keys::receive(save, other).map(|_| ()),
        Mode::Unusable | Mode::EnableAll => return Err(Status::WrongFile),
    };
    result.map_err(status_of)
}

/// What enabling every key without a partner would do. Changes only the save's checksums.
pub fn enable_plan(save: &mut [u8]) -> Plan {
    settle(save);
    let mut out = Plan::new(Mode::EnableAll);
    match keys::read(save) {
        Ok(k) => {
            out.keys = keys::missing(&k);
            if !out.keys.contains(&true) {
                out.status = Status::NoNewKeys;
            }
        }
        Err(e) => out.status = status_of(e),
    }
    out
}

/// Enables every key without a partner (enabled only, never earned). The save is not finalised.
pub fn enable_all(save: &mut [u8]) -> Result<(), Status> {
    settle(save);
    match keys::enable_all(save) {
        Ok(new) if !new.contains(&true) => Err(Status::NoNewKeys),
        Ok(_) => Ok(()),
        Err(e) => Err(status_of(e)),
    }
}
