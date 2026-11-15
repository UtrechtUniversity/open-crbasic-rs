//! Port of `checker/intervals.py`.

#![allow(dead_code, unused_variables, unused_imports)]

use std::collections::BTreeSet;
use std::rc::Rc;

use crate::diagnostics::{DiagRef, Diagnostic, Diagnostics};
use crate::profiles::Profile;
use crate::syntax::{
    Assignment, Call, Case, ConstDecl, Declaration, Expr, ExprRef, Index, Instruction, Juxtaposed,
    Member, Number, OrderedMap, Program, ProgramRef, Stmt, StmtRef, SubDef, Unparsed, VarDecl,
};

/// Port of `checker/intervals.py::gate_text`.
pub fn gate_text() -> Option<String> {
    todo!("checker/intervals.py::gate_text (omitted: stmt: untyped)")
}

/// Port of `checker/intervals.py::timeout_gate`.
pub fn timeout_gate(_seen: bool, _text: Option<&str>) {
    todo!("checker/intervals.py::timeout_gate")
}

/// Port of `checker/intervals.py::check`.
pub fn check(_program: &Program, _diags: &mut Diagnostics) {
    todo!("checker/intervals.py::check (omitted: profile: untyped, env: consteval.Env)")
}

/// Port of `checker/intervals.py::_put_get_option`.
pub fn _put_get_option() -> Option<i64> {
    todo!("checker/intervals.py::_put_get_option (omitted: call: untyped, env: consteval.Env)")
}

/// Port of `checker/intervals.py::_timer_context`.
pub fn _timer_context() {
    todo!(
        "checker/intervals.py::_timer_context (omitted: stmt: untyped, call: untyped, known: untyped, single_pass: untyped)"
    )
}

/// Port of `checker/intervals.py::_timer_shadowed`.
pub fn _timer_shadowed() {
    todo!(
        "checker/intervals.py::_timer_shadowed (omitted: program: untyped, stmt: untyped, positions: untyped, diags: untyped)"
    )
}

/// Port of `checker/intervals.py::_nonfinite_literal`.
pub fn _nonfinite_literal(_expr: Option<&Expr>) -> bool {
    todo!("checker/intervals.py::_nonfinite_literal (omitted: env: untyped)")
}

/// Port of `checker/intervals.py::_check_timeout_value`.
pub fn _check_timeout_value(_stmt: &Stmt, _diags: &mut Diagnostics) {
    todo!(
        "checker/intervals.py::_check_timeout_value (omitted: env: untyped, profile: untyped, wide: untyped, program: untyped)"
    )
}

/// Port of `checker/intervals.py::_negative_timeout`.
pub fn _negative_timeout() -> bool {
    todo!(
        "checker/intervals.py::_negative_timeout (omitted: call: untyped, env: untyped, profile: untyped, wide: untyped, program: untyped)"
    )
}

/// Port of `checker/intervals.py::_undefined_sibling`.
pub fn _undefined_sibling(_call: &Index, _i: i64, _program: &Program) -> bool {
    todo!("checker/intervals.py::_undefined_sibling (omitted: profile: untyped)")
}

/// Port of `checker/intervals.py::_needs_integer_fold`.
pub fn _needs_integer_fold() -> bool {
    todo!("checker/intervals.py::_needs_integer_fold (omitted: arg: untyped)")
}

/// Port of `checker/intervals.py::fold_timeout_expressions`.
pub fn fold_timeout_expressions(_program: &Program, _diags: &mut Diagnostics) {
    todo!(
        "checker/intervals.py::fold_timeout_expressions (omitted: profile: untyped, env: untyped, reported: untyped)"
    )
}

/// Port of `checker/intervals.py::order_timeout_negative`.
pub fn order_timeout_negative(_diags: &mut Diagnostics, _program: &Program) {
    todo!("checker/intervals.py::order_timeout_negative (omitted: profile: untyped)")
}

/// Port of `checker/intervals.py::_check_timeout`.
pub fn _check_timeout(_stmt: &Stmt, _diags: &mut Diagnostics) {
    todo!(
        "checker/intervals.py::_check_timeout (omitted: env: untyped, profile: untyped, wide: untyped)"
    )
}

/// Port of `checker/intervals.py::wide_constants`.
pub fn wide_constants(_program: &Program) -> BTreeSet<String> {
    todo!("checker/intervals.py::wide_constants (omitted: profile: untyped)")
}

/// Port of `checker/intervals.py::_out_of_range`.
pub fn _out_of_range() -> bool {
    todo!("checker/intervals.py::_out_of_range (omitted: value: untyped, profile: untyped)")
}

/// Port of `checker/intervals.py::not_integral`.
pub fn not_integral(_expr: Option<&Expr>) -> bool {
    todo!(
        "checker/intervals.py::not_integral (omitted: env: consteval.Env, profile: untyped, wide: untyped)"
    )
}

/// Port of `checker/intervals.py::_plain`.
pub fn _plain(_expr: Option<&Expr>) -> bool {
    todo!("checker/intervals.py::_plain")
}

/// Port of `checker/intervals.py::_float_typed`.
pub fn _float_typed(_expr: Option<&Expr>) -> bool {
    todo!("checker/intervals.py::_float_typed (omitted: env: consteval.Env, wide: untyped)")
}

/// Port of `checker/intervals.py::_signed32`.
pub fn _signed32(_x: i64) -> i64 {
    todo!("checker/intervals.py::_signed32")
}

/// Port of `checker/intervals.py::timeout_value`.
pub fn timeout_value() {
    todo!(
        "checker/intervals.py::timeout_value (omitted: arg: untyped, env: untyped, profile: untyped, wide: untyped)"
    )
}

/// Port of `checker/intervals.py::_timeout_literal`.
pub fn _timeout_literal(_lit: &Number, _sign: i64) {
    todo!("checker/intervals.py::_timeout_literal (omitted: profile: untyped)")
}

/// Port of `checker/intervals.py::_decimal`.
pub fn _decimal(_magnitude: i64, _sign: i64, _literal: bool) {
    todo!("checker/intervals.py::_decimal (omitted: profile: untyped)")
}

/// Port of `checker/intervals.py::_check_call`.
pub fn _check_call(_call: &Stmt, _stmt: &Stmt, _diags: &mut Diagnostics, _after_error: bool) {
    todo!("checker/intervals.py::_check_call (omitted: scan: untyped, env: untyped)")
}

/// Port of `checker/intervals.py::_scan_step`.
pub fn _scan_step(_stmt: &Instruction) -> Option<(i64, i64)> {
    todo!("checker/intervals.py::_scan_step (omitted: env: consteval.Env)")
}

/// Port of `checker/intervals.py::_wrap32`.
pub fn _wrap32(_n: i64) -> i64 {
    todo!("checker/intervals.py::_wrap32")
}

/// Port of `checker/intervals.py::_split`.
pub fn _split(_total_us: i64) -> (i64, i64) {
    todo!("checker/intervals.py::_split")
}

/// Port of `checker/intervals.py::_cdiv`.
pub fn _cdiv(_a: i64, _b: i64) -> i64 {
    todo!("checker/intervals.py::_cdiv")
}

/// Port of `checker/intervals.py::_cmod`.
pub fn _cmod(_a: i64, _b: i64) -> i64 {
    todo!("checker/intervals.py::_cmod")
}

/// Port of `checker/intervals.py::_int`.
pub fn _int(_expr: Option<&Expr>) -> Option<i64> {
    todo!("checker/intervals.py::_int (omitted: env: consteval.Env)")
}

/// Port of `checker/intervals.py::_calls`.
pub fn _calls(_stmt: &Stmt) {
    todo!("checker/intervals.py::_calls (omitted: names: untyped)")
}
