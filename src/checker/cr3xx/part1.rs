//! Port of `checker/cr3xx.py` (lines 1-768).

#![allow(dead_code, unused_variables, unused_imports)]

use std::collections::BTreeSet;
use std::rc::Rc;

use crate::diagnostics::{DiagRef, Diagnostic, Diagnostics};
use crate::profiles::Profile;
use crate::syntax::{
    Assignment, Call, Case, ConstDecl, Declaration, Expr, ExprRef, Index, Instruction, Juxtaposed,
    Member, Number, OrderedMap, Program, ProgramRef, Stmt, StmtRef, SubDef, Unparsed, VarDecl,
};

/// Port of `checker/cr3xx.py::_check_cr350_comms_mode`.
pub fn _check_cr350_comms_mode(
    _stmt: &Call,
    _port: i64,
    _diags: &mut Diagnostics,
    _invalid_format: bool,
    _shifted: bool,
) {
    todo!("checker/cr3xx.py::_check_cr350_comms_mode (omitted: profile: untyped, consts: dict)")
}

/// Port of `checker/cr3xx.py::_commsmode_problem`.
pub fn _commsmode_problem(_stmt: &Call, _index: i64) -> Option<(bool, String)> {
    todo!(
        "checker/cr3xx.py::_commsmode_problem (omitted: profile: untyped, consts: dict | None, mode: object)"
    )
}

/// Port of `checker/cr3xx.py::bare_unknown_delay`.
pub fn bare_unknown_delay(_stmt: &Call, _known: &BTreeSet<String>) -> bool {
    todo!("checker/cr3xx.py::bare_unknown_delay")
}

/// Port of `checker/cr3xx.py::_format_needs_com1_4`.
pub fn _format_needs_com1_4(_stmt: &Call) -> bool {
    todo!("checker/cr3xx.py::_format_needs_com1_4 (omitted: profile: untyped, consts: dict | None)")
}

/// Port of `checker/cr3xx.py::_non_finite`.
pub fn _non_finite(_arg: Option<&Expr>) -> Option<String> {
    todo!("checker/cr3xx.py::_non_finite (omitted: profile: untyped, consts: dict | None)")
}

/// Port of `checker/cr3xx.py::_invalid_cr3xx_serial_format`.
pub fn _invalid_cr3xx_serial_format(_arg: &Expr) -> bool {
    todo!("checker/cr3xx.py::_invalid_cr3xx_serial_format (omitted: env: consteval.Env)")
}

/// Port of `checker/cr3xx.py::_non_numeric`.
pub fn _non_numeric() -> bool {
    todo!(
        "checker/cr3xx.py::_non_numeric (omitted: expr: untyped, profile: untyped, consts: dict | None, variable_names: untyped)"
    )
}

/// Port of `checker/cr3xx.py::_check_baud`.
pub fn _check_baud(_stmt: &Call, _diags: &mut Diagnostics, _port11: bool) {
    todo!("checker/cr3xx.py::_check_baud (omitted: profile: untyped, consts: dict | None)")
}

/// Port of `checker/cr3xx.py::check_dropped_tail`.
pub fn check_dropped_tail(_program: &Program, _diags: &mut Diagnostics) {
    todo!("checker/cr3xx.py::check_dropped_tail (omitted: profile: untyped)")
}

/// Port of `checker/cr3xx.py::check`.
pub fn check(_program: &Program, _diags: &mut Diagnostics) {
    todo!("checker/cr3xx.py::check (omitted: profile: untyped)")
}

/// Port of `checker/cr3xx.py::_destination_types`.
pub fn _destination_types(_program: &Program) {
    todo!("checker/cr3xx.py::_destination_types")
}

/// Port of `checker/cr3xx.py::_is_bad_destination`.
pub fn _is_bad_destination(_expr: &Expr, _types: &OrderedMap<String, String>) -> Option<bool> {
    todo!("checker/cr3xx.py::_is_bad_destination")
}

/// Port of `checker/cr3xx.py::check_float_destinations`.
pub fn check_float_destinations(_program: &Program, _diags: &mut Diagnostics) {
    todo!("checker/cr3xx.py::check_float_destinations (omitted: profile: untyped)")
}

/// Port of `checker/cr3xx.py::drop_too_many_after_variable_illegal`.
pub fn drop_too_many_after_variable_illegal(_program: &Program, _diags: &mut Diagnostics) {
    todo!("checker/cr3xx.py::drop_too_many_after_variable_illegal (omitted: profile: untyped)")
}

/// Port of `checker/cr3xx.py::_expr_names`.
pub fn _expr_names(_expr: Option<&Expr>) -> Vec<String> {
    todo!("checker/cr3xx.py::_expr_names")
}
