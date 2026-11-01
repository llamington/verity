//! Legality checks (PKHeX `LegalityAnalysis` port).

#[cfg(feature = "alloc")]
pub mod analysis4;
#[cfg(feature = "alloc")]
pub mod enc4;
#[cfg(feature = "alloc")]
pub mod evo;
pub mod moves4;
