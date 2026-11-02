//! Legality checks (PKHeX `LegalityAnalysis` port).

#[cfg(feature = "alloc")]
pub mod analysis4;
#[cfg(feature = "alloc")]
pub mod enc4;
#[cfg(feature = "alloc")]
pub mod evo;
#[cfg(feature = "alloc")]
pub mod gift4;
pub mod hatch4;
pub mod moves4;
