//! Port of `tables.py` (lines 1527-2177).

#![allow(dead_code, unused_variables, unused_imports)]

use std::collections::BTreeSet;
use std::rc::Rc;

use super::TdfTable;
use super::{DataTableDecl, OutField};
use crate::diagnostics::{DiagRef, Diagnostic, Diagnostics};
use crate::profiles::Profile;
use crate::syntax::{
    Assignment, Call, Case, ConstDecl, Declaration, Expr, ExprRef, Index, Instruction, Juxtaposed,
    Member, Number, OrderedMap, Program, ProgramRef, Stmt, StmtRef, SubDef, Unparsed, VarDecl,
};

/// Port of `tables.py::_apply_fieldnames`.
pub fn _apply_fieldnames(_fields: Vec<OutField>, _arg: &str, _profile_name: &str) -> Vec<OutField> {
    todo!("tables.py::_apply_fieldnames")
}

/// Port of `tables.py::_interval_from_args`.
pub fn _interval_from_args(_into_idx: i64, _val_idx: i64, _unit_idx: i64) {
    todo!("tables.py::_interval_from_args (omitted: args: list, env: consteval.Env)")
}

/// Port of `tables.py::_build_data_table`.
pub fn _build_data_table(_dt: &DataTableDecl, _called: &BTreeSet<String>, _anon: Option<Vec<i64>>) {
    todo!(
        "tables.py::_build_data_table (omitted: env: consteval.Env, var_info: dict, by_display: dict, by_base: dict | None)"
    )
}

/// Port of `tables.py::_field_record_bytes`.
pub fn _field_record_bytes(_of: OutField) -> i64 {
    todo!("tables.py::_field_record_bytes")
}

/// Port of `tables.py::_table_default_min`.
pub fn _table_default_min(_bytes_per_record: i64) -> i64 {
    todo!("tables.py::_table_default_min")
}

/// Port of `tables.py::_resolve_auto_sizes`.
pub fn _resolve_auto_sizes(_scan_weights: &OrderedMap<String, f64>) {
    todo!("tables.py::_resolve_auto_sizes (omitted: tables_meta: list[tuple[TdfTable, dict]])")
}

/// Port of `tables.py::_apply_sizes`.
pub fn _apply_sizes(_scan_weights: &OrderedMap<String, f64>, _resolve_auto: bool) {
    todo!("tables.py::_apply_sizes (omitted: tables_meta: list[tuple[TdfTable, dict]])")
}

/// Port of `tables.py::_register_table_sources`.
pub fn _register_table_sources(_table: &TdfTable) {
    todo!("tables.py::_register_table_sources (omitted: var_info: dict[str, VarInfo])")
}

/// Port of `tables.py::build_tables`.
pub fn build_tables(_program: &Program) {
    todo!("tables.py::build_tables (omitted: profile: untyped, consts: dict)")
}

/// Port of `tables.py::_preserve_deduction`.
pub fn _preserve_deduction(_program: &Program) -> i64 {
    todo!(
        "tables.py::_preserve_deduction (omitted: env: untyped, scanned: untyped, var_info: untyped)"
    )
}

/// Port of `tables.py::_explode_indices`.
pub fn _explode_indices(_subdims: Vec<i64>, _drop_last: bool, _begin: i64, _dim: i64) {
    todo!("tables.py::_explode_indices")
}

/// Port of `tables.py::render_table_list`.
pub fn render_table_list(_tables: Vec<TdfTable>) -> String {
    todo!("tables.py::render_table_list")
}

/// Port of `tables.py::_seconds_to_display`.
pub fn _seconds_to_display(_secs: i64) {
    todo!("tables.py::_seconds_to_display")
}
