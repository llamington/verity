//! Legality checks (PKHeX `LegalityAnalysis` port).

#[cfg(feature = "alloc")]
pub mod analysis3;
pub mod analysis4;
pub mod analysis5;
#[cfg(feature = "alloc")]
pub mod breed4;
pub mod contest;
pub mod enc4;
#[cfg(feature = "alloc")]
pub mod evo;
#[cfg(feature = "alloc")]
pub mod gift3;
pub mod gift4;
pub mod hatch4;
pub mod moves4;
pub mod ranch4;
pub mod ribbons4;
#[cfg(feature = "alloc")]
pub mod verdict;
