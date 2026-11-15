//! Port of `checker/instr_rules.py` (lines 766-1538).

#![allow(dead_code, unused_variables, unused_imports)]

use std::collections::BTreeSet;
use std::rc::Rc;

use crate::diagnostics::{DiagRef, Diagnostic, Diagnostics};
use crate::profiles::Profile;
use crate::syntax::{
    Assignment, Call, Case, ConstDecl, Declaration, Expr, ExprRef, Index, Instruction, Juxtaposed,
    Member, Number, OrderedMap, Program, ProgramRef, Stmt, StmtRef, SubDef, Unparsed, VarDecl,
};

/// Port of `checker/instr_rules.py::check_snmp_oid`.
pub fn check_snmp_oid(_program: &Program, _profile: &Profile, _diags: &mut Diagnostics) {
    todo!("checker/instr_rules.py::check_snmp_oid (omitted: env: untyped)")
}

/// Port of `checker/instr_rules.py::check_modemcallback_baud`.
pub fn check_modemcallback_baud(
    _stmt: &Instruction,
    _program: &Program,
    _profile: &Profile,
    _diags: &mut Diagnostics,
) {
    todo!("checker/instr_rules.py::check_modemcallback_baud (omitted: env: consteval.Env)")
}

/// Port of `checker/instr_rules.py::check_snmpvariable_too_many`.
pub fn check_snmpvariable_too_many(
    _program: &Program,
    _profile: &Profile,
    _diags: &mut Diagnostics,
) {
    todo!("checker/instr_rules.py::check_snmpvariable_too_many")
}

/// Port of `checker/instr_rules.py::_is_negative_or_invalid`.
pub fn _is_negative_or_invalid() -> bool {
    todo!("checker/instr_rules.py::_is_negative_or_invalid (omitted: value: object)")
}

/// Port of `checker/instr_rules.py::check_scan_params`.
pub fn check_scan_params(_program: &Program, _profile: &Profile, _diags: &mut Diagnostics) {
    todo!("checker/instr_rules.py::check_scan_params (omitted: env: consteval.Env)")
}

/// Port of `checker/instr_rules.py::_check_slow_scan_interval`.
pub fn _check_slow_scan_interval(
    _stmt: &Instruction,
    _split: (i64, i64),
    _diags: &mut Diagnostics,
) {
    todo!("checker/instr_rules.py::_check_slow_scan_interval")
}

/// Port of `checker/instr_rules.py::_scan_min_interval`.
pub fn _scan_min_interval(_stmt: &Instruction, _diags: &mut Diagnostics) -> Option<(i64, i64)> {
    todo!("checker/instr_rules.py::_scan_min_interval (omitted: env: consteval.Env)")
}

/// Port of `checker/instr_rules.py::_crem`.
pub fn _crem(_a: i64, _b: i64) -> i64 {
    todo!("checker/instr_rules.py::_crem")
}

/// Port of `checker/instr_rules.py::scan_split`.
pub fn scan_split(_stmt: &Instruction) -> Option<(i64, i64)> {
    todo!("checker/instr_rules.py::scan_split (omitted: env: consteval.Env)")
}

/// Port of `checker/instr_rules.py::_f32_is_2_31`.
pub fn _f32_is_2_31() -> bool {
    todo!("checker/instr_rules.py::_f32_is_2_31 (omitted: value: untyped)")
}

/// Port of `checker/instr_rules.py::_cdiv`.
pub fn _cdiv(_a: i64, _b: i64) -> i64 {
    todo!("checker/instr_rules.py::_cdiv")
}

/// Port of `checker/instr_rules.py::check_scan_max_interval`.
pub fn check_scan_max_interval(_program: &Program, _profile: &Profile, _diags: &mut Diagnostics) {
    todo!("checker/instr_rules.py::check_scan_max_interval (omitted: env: consteval.Env)")
}

/// Port of `checker/instr_rules.py::_scan_raw`.
pub fn _scan_raw(_stmt: &Instruction) -> Option<(i64, i64)> {
    todo!("checker/instr_rules.py::_scan_raw (omitted: env: consteval.Env)")
}

/// Port of `checker/instr_rules.py::_units_key`.
pub fn _units_key(_units_arg: &Expr, _truncate: bool) {
    todo!("checker/instr_rules.py::_units_key (omitted: env: consteval.Env)")
}

/// Port of `checker/instr_rules.py::_interval_us`.
pub fn _interval_us(
    _interval_arg: Option<&Expr>,
    _units_arg: Option<&Expr>,
    _truncate_units: bool,
) -> Option<f64> {
    todo!("checker/instr_rules.py::_interval_us (omitted: env: consteval.Env)")
}

/// Port of `checker/instr_rules.py::_divisible_interval_us`.
pub fn _divisible_interval_us(
    _interval_arg: Option<&Expr>,
    _units_arg: Option<&Expr>,
    _truncate_units: bool,
) -> Option<f64> {
    todo!("checker/instr_rules.py::_divisible_interval_us (omitted: env: consteval.Env)")
}

/// Port of `checker/instr_rules.py::check_data_interval`.
pub fn check_data_interval(_program: &Program, _profile: &Profile, _diags: &mut Diagnostics) {
    todo!("checker/instr_rules.py::check_data_interval (omitted: env: consteval.Env)")
}

/// Port of `checker/instr_rules.py::_divisibility_message`.
pub fn _divisibility_message(_dt_us: i64, _scan_us: i64) -> Option<String> {
    todo!("checker/instr_rules.py::_divisibility_message")
}

/// Port of `checker/instr_rules.py::_has_call`.
pub fn _has_call(_expr: Option<&Expr>, _variable_names: &BTreeSet<String>) -> bool {
    todo!("checker/instr_rules.py::_has_call")
}

/// Port of `checker/instr_rules.py::_stringish`.
pub fn _stringish(_expr: Option<&Expr>, _str_consts: &BTreeSet<String>) -> bool {
    todo!("checker/instr_rules.py::_stringish")
}

/// Port of `checker/instr_rules.py::_string_typed`.
pub fn _string_typed(
    _expr: Option<&Expr>,
    _str_consts: &BTreeSet<String>,
    _profile: &Profile,
) -> Option<bool> {
    todo!("checker/instr_rules.py::_string_typed (omitted: typer: untyped)")
}

/// Port of `checker/instr_rules.py::check_iif_types`.
pub fn check_iif_types(_program: &Program, _profile: &Profile, _diags: &mut Diagnostics) {
    todo!("checker/instr_rules.py::check_iif_types")
}
