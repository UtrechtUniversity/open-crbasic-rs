//! Port of `tables.py` (lines 1-2177).

#![allow(dead_code, unused_variables, unused_imports)]

pub use crate::tdf::{TdfField, TdfTable};

pub mod part1;
pub use part1::*;
pub mod part2;
pub use part2::*;
pub mod part3;
pub use part3::*;

/// Port of `tables_hook.py::attach`.
pub fn attach(
    program: &crate::syntax::Program,
    profile: &'static crate::profiles::Profile,
    result: &mut crate::compiler::CompileResult,
) {
    let (tables, table_list_text, extras) = build_tables(program, profile, &program.consts);
    result.tables = tables;
    result.table_list = Some(table_list_text);
    result.table_extras = extras;
}
