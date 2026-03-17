//! Execution runtime for Arky.
//!
//! This crate is the promoted home for in-process agent execution. During the
//! migration it re-exports the existing `arky-core` surface.

pub use arky_core as core;
pub use arky_core::*;
