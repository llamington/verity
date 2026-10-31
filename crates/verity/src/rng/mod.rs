pub mod lcrng;

pub use lcrng::{advance_seed, crypt_array_16, next_seed, prev_seed};
