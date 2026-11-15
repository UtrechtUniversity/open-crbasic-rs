//! Port of `checker/__init__.py`.

#![allow(dead_code, unused_variables, unused_imports)]

use std::collections::BTreeSet;
use std::rc::Rc;

use crate::diagnostics::{DiagRef, Diagnostic, Diagnostics};
use crate::profiles::Profile;
use crate::syntax::{
    Assignment, Call, Case, ConstDecl, Declaration, Expr, ExprRef, Index, Instruction, Juxtaposed,
    Member, Number, OrderedMap, Program, ProgramRef, Stmt, StmtRef, SubDef, Unparsed, VarDecl,
};

pub mod context;
pub mod cr3xx;
pub mod ess;
pub mod expressions;
pub mod instr_data;
pub mod instr_measure;
pub mod instr_rules;
pub mod intervals;
pub mod longconv;
pub mod scan_time;
pub mod storage;
pub mod structs;
pub mod structure;
pub mod symbols;
pub mod terminals;
pub mod var_types;
pub mod warnings;

/// Port of `checker/__init__.py::_is_unreached`.
pub fn _is_unreached(_d: &Diagnostic) -> bool {
    todo!("checker/__init__.py::_is_unreached")
}

/// Port of `checker/__init__.py::_pos_or_line`.
pub fn _pos_or_line(_d: &Diagnostic) -> i64 {
    todo!("checker/__init__.py::_pos_or_line")
}

/// Port of `checker/__init__.py::_before_cutoff`.
pub fn _before_cutoff(_d: &Diagnostic, _stop_at: Option<i64>) -> bool {
    todo!("checker/__init__.py::_before_cutoff")
}

/// Port of `checker/__init__.py::_blocks_tier_c`.
pub fn _blocks_tier_c(_d: &Diagnostic, _single_pass: bool) -> bool {
    todo!("checker/__init__.py::_blocks_tier_c")
}

/// Port of `checker/__init__.py::_is_tier_b_generic`.
pub fn _is_tier_b_generic(_d: &Diagnostic) -> bool {
    todo!("checker/__init__.py::_is_tier_b_generic")
}

/// Port of `checker/__init__.py::_run_pass`.
pub fn _run_pass(_phase: &str) {
    todo!("checker/__init__.py::_run_pass (omitted: fn: untyped)")
}

/// Port of `checker/__init__.py::_run_aside`.
pub fn _run_aside(_phase: &str, _diags: &mut Diagnostics) {
    todo!("checker/__init__.py::_run_aside (omitted: fn: untyped)")
}

/// Port of `checker/__init__.py::_placement_rejected`.
pub fn _placement_rejected(_program: &Program, _diags: &mut Diagnostics) {
    todo!("checker/__init__.py::_placement_rejected (omitted: profile: untyped)")
}

/// Port of `checker/__init__.py::_run_pass_capture`.
pub fn _run_pass_capture(_phase: &str) {
    todo!("checker/__init__.py::_run_pass_capture (omitted: fn: untyped)")
}

/// Port of `checker/__init__.py::_record_exception`.
pub fn _record_exception() {
    todo!("checker/__init__.py::_record_exception (omitted: fn: untyped, exc: Exception)")
}

/// Port of `checker/__init__.py::check`.
pub fn check(_program: &Program, _profile: &Profile, _diags: &mut Diagnostics) {
    todo!("checker/__init__.py::check (omitted: result: untyped)")
}

/// Port of `checker/__init__.py::_reorder_scan_placement_after_symbols`.
pub fn _reorder_scan_placement_after_symbols(_items: Vec<DiagRef>) -> Vec<DiagRef> {
    todo!("checker/__init__.py::_reorder_scan_placement_after_symbols")
}

/// Port of `checker/__init__.py::_place_index_bounds`.
pub fn _place_index_bounds(
    _bounds_items: Vec<DiagRef>,
    _resolved: Vec<DiagRef>,
    _program: &Program,
    _single_pass: bool,
) {
    todo!("checker/__init__.py::_place_index_bounds")
}

/// Port of `checker/__init__.py::_resolve`.
pub fn _resolve(
    _diags: &mut Diagnostics,
    _program: &Program,
    _flat: &Program,
    _profile: &Profile,
    _stop_at: Option<i64>,
    _table_stop_pos: Option<i64>,
) {
    todo!("checker/__init__.py::_resolve")
}

/// Port of `checker/__init__.py::_check`.
pub fn _check(_program: &Program, _profile: &Profile, _diags: &mut Diagnostics) {
    todo!("checker/__init__.py::_check (omitted: result: untyped)")
}

/// Port of `checker/__init__.py::_check_with_context`.
pub fn _check_with_context(
    _program: &Program,
    _profile: &Profile,
    _diags: &mut Diagnostics,
    _ctx: &crate::checker::context::Context,
) {
    todo!("checker/__init__.py::_check_with_context (omitted: result: untyped, env: consteval.Env)")
}
