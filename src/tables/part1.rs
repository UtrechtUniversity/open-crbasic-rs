//! Port of `tables.py` (lines 1-745).

#![allow(dead_code, unused_variables, unused_imports)]

use std::collections::BTreeSet;
use std::rc::Rc;

use super::TdfTable;
use crate::diagnostics::{DiagRef, Diagnostic, Diagnostics};
use crate::profiles::Profile;
use crate::syntax::{
    Assignment, Call, Case, ConstDecl, Declaration, Expr, ExprRef, Index, Instruction, Juxtaposed,
    Member, Number, OrderedMap, Program, ProgramRef, Stmt, StmtRef, SubDef, Unparsed, VarDecl,
};

/// Port of `tables.py::_dtype_code`.
pub fn _dtype_code(_expr: Option<&Expr>) -> i64 {
    todo!("tables.py::_dtype_code (omitted: env: consteval.Env)")
}

/// Port of `tables.py::_f32`.
pub fn _f32(_x: f64) -> f64 {
    todo!("tables.py::_f32")
}

/// Port of `tables.py::_f32_safe`.
pub fn _f32_safe() -> f64 {
    todo!("tables.py::_f32_safe (omitted: x: untyped)")
}

/// Port of `tables.py::size_class`.
pub fn size_class() {
    todo!("tables.py::size_class (omitted: raw: untyped)")
}

/// Port of `tables.py::declared_size`.
pub fn declared_size(_cr3xx: bool, _bytes_per_record: i64) -> Option<i64> {
    todo!("tables.py::declared_size (omitted: raw: untyped)")
}

/// Port of `tables.py::_fold`.
pub fn _fold(_expr: Option<&Expr>) {
    todo!("tables.py::_fold (omitted: env: consteval.Env, default: untyped)")
}

/// Port of `tables.py::_unit_value`.
pub fn _unit_value(_expr: Option<&Expr>) -> Option<i64> {
    todo!("tables.py::_unit_value (omitted: env: consteval.Env)")
}

/// Port of `tables.py::_scan_unit_name`.
pub fn _scan_unit_name(_expr: Option<&Expr>) -> String {
    todo!("tables.py::_scan_unit_name (omitted: env: consteval.Env)")
}

/// Port of `tables.py::_as_int`.
pub fn _as_int() -> i64 {
    todo!("tables.py::_as_int (omitted: v: untyped, default: untyped)")
}

/// Port of `tables.py::_const_is_int`.
pub fn _const_is_int(_expr: Option<&Expr>) -> bool {
    todo!("tables.py::_const_is_int (omitted: env: consteval.Env, int_names: dict)")
}

/// Port of `tables.py::VarInfo`.
#[derive(Debug, Default)]
pub struct VarInfo {}

impl VarInfo {
    /// Port of `tables.py::VarInfo::full_shape`.
    pub fn full_shape(&self) -> Vec<i64> {
        todo!("tables.py::VarInfo::full_shape")
    }

    /// Port of `tables.py::VarInfo::collapsed`.
    pub fn collapsed(&self) {
        todo!("tables.py::VarInfo::collapsed")
    }
}

/// Port of `tables.py::_string_stored_len`.
pub fn _string_stored_len(_n: i64) -> i64 {
    todo!("tables.py::_string_stored_len")
}

/// Port of `tables.py::_var_shape`.
pub fn _var_shape(_v: &VarDecl) -> Vec<i64> {
    todo!("tables.py::_var_shape (omitted: env: consteval.Env)")
}

/// Port of `tables.py::_var_type_code`.
pub fn _var_type_code(_v: &VarDecl) -> i64 {
    todo!("tables.py::_var_type_code")
}

/// Port of `tables.py::_string_size_expr`.
pub fn _string_size_expr(_v: &VarDecl) -> Option<ExprRef> {
    todo!("tables.py::_string_size_expr")
}

/// Port of `tables.py::_make_var_info`.
pub fn _make_var_info(_v: &VarDecl) -> VarInfo {
    todo!("tables.py::_make_var_info (omitted: env: consteval.Env)")
}

/// Port of `tables.py::_kw`.
pub fn _kw(_s: &Stmt) -> String {
    todo!("tables.py::_kw")
}

/// Port of `tables.py::ConstTableDecl`.
#[derive(Debug, Default)]
pub struct ConstTableDecl {}

/// Port of `tables.py::DataTableDecl`.
#[derive(Debug, Default)]
pub struct DataTableDecl {}

/// Port of `tables.py::_Scanned`.
#[derive(Debug, Default)]
pub struct Scanned {}

/// Port of `tables.py::_scan_program`.
pub fn _scan_program(_program: &Program) -> Scanned {
    todo!("tables.py::_scan_program")
}

/// Port of `tables.py::_flat`.
pub fn _flat() {
    todo!("tables.py::_flat (omitted: ss: untyped)")
}

/// Port of `tables.py::_scan_weights`.
pub fn _scan_weights(_stmts: &[StmtRef]) -> OrderedMap<String, f64> {
    todo!("tables.py::_scan_weights (omitted: env: consteval.Env)")
}

/// Port of `tables.py::_scan_calls`.
pub fn _scan_calls(_stmts: &[StmtRef]) {
    todo!("tables.py::_scan_calls (omitted: env: consteval.Env)")
}

/// Port of `tables.py::_last_call_interval`.
pub fn _last_call_interval(_stmts: &[StmtRef]) {
    todo!("tables.py::_last_call_interval (omitted: env: consteval.Env)")
}

/// Port of `tables.py::_status_table`.
pub fn _status_table(_slow_seq_count: i64) -> TdfTable {
    todo!("tables.py::_status_table")
}

/// Port of `tables.py::_build_struct_tables`.
pub fn _build_struct_tables(_all_vars: &[Rc<VarDecl>]) {
    todo!("tables.py::_build_struct_tables (omitted: struct_types: dict, env: consteval.Env)")
}

/// Port of `tables.py::_ch400status_table`.
pub fn _ch400status_table() -> TdfTable {
    todo!("tables.py::_ch400status_table")
}

/// Port of `tables.py::_cpistatus_table`.
pub fn _cpistatus_table() -> TdfTable {
    todo!("tables.py::_cpistatus_table")
}

/// Port of `tables.py::_resolve_alias_target`.
pub fn _resolve_alias_target(_expr: Option<&Expr>) {
    todo!("tables.py::_resolve_alias_target (omitted: var_info: dict | None)")
}
