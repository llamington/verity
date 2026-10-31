#![no_std]

#[cfg(feature = "std")]
extern crate std;

#[cfg(any(feature = "alloc", feature = "std"))]
extern crate alloc;

pub mod data;
pub mod gen;
pub mod gift;
pub mod legality;
pub mod memlink;
pub mod pkm;
pub mod rng;
pub mod safety;
pub mod save;
pub mod text;

pub use save::crc16::crc16_ccitt;
