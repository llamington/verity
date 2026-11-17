//! State the front end keeps in statics. The DS runs it on one thread, and no interrupt handler
//! touches it, so a `RefCell` (panicking on overlapping borrows rather than aliasing) is enough.

use core::cell::{Cell, RefCell, RefMut};

pub struct Global<T>(RefCell<T>);

// SAFETY: the front end has one thread and its interrupt handlers (libnds's) never reach these,
// so no two threads ever see one.
unsafe impl<T> Sync for Global<T> {}

impl<T> Global<T> {
    pub const fn new(value: T) -> Self {
        Self(RefCell::new(value))
    }

    pub fn borrow_mut(&self) -> RefMut<'_, T> {
        self.0.borrow_mut()
    }
}

/// A `Copy` value in a static.
pub struct Var<T: Copy>(Cell<T>);

// SAFETY: as for `Global`: one thread, never reached from an interrupt.
unsafe impl<T: Copy> Sync for Var<T> {}

impl<T: Copy> Var<T> {
    pub const fn new(value: T) -> Self {
        Self(Cell::new(value))
    }

    pub fn get(&self) -> T {
        self.0.get()
    }

    pub fn set(&self, value: T) {
        self.0.set(value)
    }
}
