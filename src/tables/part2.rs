//! Port of `tables.py` (lines 746-1526).

#![allow(dead_code, unused_variables, unused_imports)]

use std::collections::BTreeSet;
use std::rc::Rc;

use super::TdfTable;
use super::{ConstTableDecl, VarInfo};
use crate::diagnostics::{DiagRef, Diagnostic, Diagnostics};
use crate::profiles::Profile;
use crate::syntax::{
    Assignment, Call, Case, ConstDecl, Declaration, Expr, ExprRef, Index, Instruction, Juxtaposed,
    Member, Number, OrderedMap, Program, ProgramRef, Stmt, StmtRef, SubDef, Unparsed, VarDecl,
};

/// Port of `tables.py::_build_public_table`.
pub fn _build_public_table(
    _publics: &[Rc<VarDecl>],
    _alias_stmts: &[StmtRef],
    _units_stmts: &[StmtRef],
    _readonly_names: &BTreeSet<String>,
) {
    todo!("tables.py::_build_public_table (omitted: var_info: dict[str, VarInfo])")
}

/// Port of `tables.py::_build_const_tables`.
pub fn _build_const_tables(_const_tables: Vec<ConstTableDecl>) -> Vec<TdfTable> {
    todo!("tables.py::_build_const_tables (omitted: env: consteval.Env)")
}

/// Port of `tables.py::_datatableinfo_table`.
pub fn _datatableinfo_table(_n: i64) -> TdfTable {
    todo!("tables.py::_datatableinfo_table")
}

/// Port of `tables.py::_source_info`.
pub fn _source_info(_expr: Option<&Expr>) -> Option<VarInfo> {
    todo!("tables.py::_source_info (omitted: var_info: dict[str, VarInfo])")
}

/// Port of `tables.py::_source_name`.
pub fn _source_name(_expr: Option<&Expr>) -> String {
    todo!("tables.py::_source_name (omitted: var_info: dict[str, VarInfo] | None)")
}

/// Port of `tables.py::_is_anonymous_source`.
pub fn _is_anonymous_source(_expr: Option<&Expr>) -> bool {
    todo!("tables.py::_is_anonymous_source (omitted: var_info: dict, env: consteval.Env)")
}

/// Port of `tables.py::_anonymous_string_len`.
pub fn _anonymous_string_len(_expr: Option<&Expr>) -> i64 {
    todo!("tables.py::_anonymous_string_len (omitted: var_info: dict, env: consteval.Env)")
}

/// Port of `tables.py::_number_anonymous`.
pub fn _number_anonymous(_src: Option<&Expr>, _anon: Option<Vec<i64>>) {
    todo!(
        "tables.py::_number_anonymous (omitted: fields: list, var_info: dict, env: consteval.Env)"
    )
}

/// Port of `tables.py::_source_shape`.
pub fn _source_shape(_expr: Option<&Expr>, _reps: i64) {
    todo!("tables.py::_source_shape (omitted: var_info: dict[str, VarInfo], env: consteval.Env)")
}

/// Port of `tables.py::_units_for_source`.
pub fn _units_for_source(_expr: Option<&Expr>) -> String {
    todo!(
        "tables.py::_units_for_source (omitted: var_info: dict[str, VarInfo], by_display: dict[str, TdfField])"
    )
}

/// Port of `tables.py::_OutField`.
#[derive(Debug, Default)]
pub struct OutField {}

/// Port of `tables.py::_element_range`.
pub fn _element_range(_src: Option<&Expr>, _begin: i64, _dim: i64) {
    todo!("tables.py::_element_range (omitted: env: consteval.Env)")
}

/// Port of `tables.py::_type_bytes`.
pub fn _type_bytes(_type_code: i64, _dim_for_string: i64) -> i64 {
    todo!("tables.py::_type_bytes")
}

/// Port of `tables.py::_elem_size`.
pub fn _elem_size(_f: OutField, _info: &VarInfo) -> i64 {
    todo!("tables.py::_elem_size")
}

/// Port of `tables.py::_split_by_alias`.
pub fn _split_by_alias(_fields: Vec<OutField>) -> Vec<OutField> {
    todo!(
        "tables.py::_split_by_alias (omitted: src: untyped, var_info: dict, by_base: dict, env: consteval.Env)"
    )
}

/// Port of `tables.py::_fft_field`.
pub fn _fft_field(_s: &Instruction, _anon: Option<Vec<i64>>) -> OutField {
    todo!("tables.py::_fft_field (omitted: var_info: dict, by_display: dict, env: consteval.Env)")
}

/// Port of `tables.py::_idx_suffix`.
pub fn _idx_suffix(_src: Option<&Expr>) -> String {
    todo!("tables.py::_idx_suffix (omitted: var_info: dict, env: consteval.Env)")
}

/// Port of `tables.py::_alias_source`.
pub fn _alias_source(_instr: &Instruction) {
    todo!(
        "tables.py::_alias_source (omitted: var_info: dict, by_display: dict, by_base: dict | None)"
    )
}

/// Port of `tables.py::_emit_output_fields`.
pub fn _emit_output_fields(_instr: &Instruction, _anon: Option<Vec<i64>>) -> Vec<OutField> {
    todo!(
        "tables.py::_emit_output_fields (omitted: var_info: dict, by_display: dict, env: consteval.Env, by_base: dict | None)"
    )
}

/// Port of `tables.py::_emit_struct_fields`.
pub fn _emit_struct_fields(_instr: &Instruction) {
    todo!("tables.py::_emit_struct_fields (omitted: var_info: dict, env: consteval.Env)")
}

/// Port of `tables.py::_emit_output_fields_raw`.
pub fn _emit_output_fields_raw(_instr: &Instruction) -> Vec<OutField> {
    todo!(
        "tables.py::_emit_output_fields_raw (omitted: var_info: dict, by_display: dict, env: consteval.Env)"
    )
}
