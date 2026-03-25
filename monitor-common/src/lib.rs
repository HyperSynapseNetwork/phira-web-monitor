//! Phira Web Monitor - Common Types & Logic

pub mod core;
pub mod live;

// Re-exports for phira_mp_macros::BinaryData derive (generates `crate::X` references)
pub use anyhow::{bail, Result};
pub use phira_mp_common::{BinaryData, BinaryReader, BinaryWriter};
