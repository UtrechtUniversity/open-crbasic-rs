//! Port of `checker/warnings.py`.

#![allow(dead_code, unused_variables, unused_imports)]

use std::collections::BTreeSet;
use std::rc::Rc;

use crate::diagnostics::{DiagRef, Diagnostic, Diagnostics};
use crate::profiles::Profile;
use crate::syntax::{
    Assignment, Call, Case, ConstDecl, Declaration, Expr, ExprRef, Index, Instruction, Juxtaposed,
    Member, Number, OrderedMap, Program, ProgramRef, Stmt, StmtRef, SubDef, Unparsed, VarDecl,
};

/// Port of `checker/warnings.py::_table_arg_name`.
pub fn _table_arg_name(_stmt: &Instruction) -> Option<String> {
    todo!("checker/warnings.py::_table_arg_name")
}

/// Port of `checker/warnings.py::_with_inline_branches`.
pub fn _with_inline_branches() {
    todo!("checker/warnings.py::_with_inline_branches (omitted: statements: untyped)")
}

/// Port of `checker/warnings.py::check`.
pub fn check(_program: &Program, _diags: &mut Diagnostics) {
    todo!("checker/warnings.py::check (omitted: result: untyped)")
}

/// Port of `checker/warnings.py::_check_uint1_dims`.
pub fn _check_uint1_dims(_program: &Program, _profile: &Profile, _diags: &mut Diagnostics) {
    todo!("checker/warnings.py::_check_uint1_dims (omitted: env: consteval.Env)")
}

/// Port of `checker/warnings.py::check_early`.
pub fn check_early(_program: &Program, _profile: &Profile, _diags: &mut Diagnostics) {
    todo!("checker/warnings.py::check_early (omitted: env: consteval.Env)")
}

/// Port of `checker/warnings.py::_kw`.
pub fn _kw(_stmt: &Stmt) -> String {
    todo!("checker/warnings.py::_kw")
}

/// Port of `checker/warnings.py::_is_program_end`.
pub fn _is_program_end(_stmt: &Stmt) -> bool {
    todo!("checker/warnings.py::_is_program_end")
}

/// Port of `checker/warnings.py::_display_name`.
pub fn _display_name(_stmt: &Stmt, _profile: &Profile) -> String {
    todo!("checker/warnings.py::_display_name")
}

/// Port of `checker/warnings.py::_open_conditional`.
pub fn _open_conditional(_statements: &[StmtRef], _start: i64, _end: i64) -> i64 {
    todo!("checker/warnings.py::_open_conditional")
}

/// Port of `checker/warnings.py::_scan_span`.
pub fn _scan_span(_statements: &[StmtRef], _start: i64) -> (i64, bool) {
    todo!("checker/warnings.py::_scan_span")
}

/// Port of `checker/warnings.py::_scan_count_is_zero`.
pub fn _scan_count_is_zero(_stmt: &Instruction) -> Option<bool> {
    todo!("checker/warnings.py::_scan_count_is_zero (omitted: env: consteval.Env)")
}

/// Port of `checker/warnings.py::_check_reachability`.
pub fn _check_reachability(_program: &Program, _profile: &Profile, _diags: &mut Diagnostics) {
    todo!("checker/warnings.py::_check_reachability (omitted: env: consteval.Env)")
}

/// Port of `checker/warnings.py::check_background_calibration`.
pub fn check_background_calibration(
    _program: &Program,
    _profile: &Profile,
    _diags: &mut Diagnostics,
) {
    todo!("checker/warnings.py::check_background_calibration (omitted: env: consteval.Env)")
}

/// Port of `checker/warnings.py::_value_in`.
pub fn _value_in(_position: i64) -> bool {
    todo!("checker/warnings.py::_value_in (omitted: args: list, values: list, env: untyped)")
}

/// Port of `checker/warnings.py::check_slow_scan_skips`.
pub fn check_slow_scan_skips(_program: &Program, _profile: &Profile, _diags: &mut Diagnostics) {
    todo!("checker/warnings.py::check_slow_scan_skips (omitted: env: consteval.Env)")
}

/// Port of `checker/warnings.py::_scan_interval_ms`.
pub fn _scan_interval_ms(_stmt: &Instruction) -> Option<f64> {
    todo!("checker/warnings.py::_scan_interval_ms (omitted: env: consteval.Env)")
}

/// Port of `checker/warnings.py::_check_fnotch`.
pub fn _check_fnotch(_program: &Program, _profile: &Profile, _diags: &mut Diagnostics) {
    todo!("checker/warnings.py::_check_fnotch (omitted: env: consteval.Env)")
}

/// Port of `checker/warnings.py::_call_args`.
pub fn _call_args(_text: &str, _start: i64) -> Option<Vec<String>> {
    todo!("checker/warnings.py::_call_args")
}

/// Port of `checker/warnings.py::add_truncate_warnings`.
pub fn add_truncate_warnings(_program: &Program, _diags: &mut Diagnostics) {
    todo!("checker/warnings.py::add_truncate_warnings (omitted: profile: untyped)")
}
