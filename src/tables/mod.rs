//! Port of `tables.py` (lines 1-2177).

#![allow(dead_code, unused_variables, unused_imports)]

/// Placeholder for `tdf.py::TdfField` (to be moved to `tdf.rs` in P3-h).
#[derive(Debug, Default)]
pub struct TdfField {}

/// Placeholder for `tdf.py::TdfTable` (to be moved to `tdf.rs` in P3-h).
#[derive(Debug, Default)]
pub struct TdfTable {}

pub mod part1;
pub use part1::*;
pub mod part2;
pub use part2::*;
pub mod part3;
pub use part3::*;
