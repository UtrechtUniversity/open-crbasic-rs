//! Port of `checker/instr_measure.py` (lines 754-1540).

#![allow(dead_code, unused_variables, unused_imports)]

use std::collections::BTreeSet;
use std::rc::Rc;

use crate::diagnostics::{DiagRef, Diagnostic, Diagnostics};
use crate::profiles::Profile;
use crate::syntax::{
    Assignment, Call, Case, ConstDecl, Declaration, Expr, ExprRef, Index, Instruction, Juxtaposed,
    Member, Number, OrderedMap, Program, ProgramRef, Stmt, StmtRef, SubDef, Unparsed, VarDecl,
};

/// Port of `checker/instr_measure.py::_check_delay`.
pub fn _check_delay(
    _stmt: &Instruction,
    _diags: &mut Diagnostics,
    _sequential: bool,
    _slow: bool,
    _typer: &mut crate::checker::var_types::Typer,
) {
    todo!("checker/instr_measure.py::_check_delay (omitted: cfg: dict, env: consteval.Env)")
}

/// Port of `checker/instr_measure.py::_delay_interval_us`.
pub fn _delay_interval_us() -> Option<f64> {
    todo!(
        "checker/instr_measure.py::_delay_interval_us (omitted: interval_arg: untyped, units_arg: untyped, env: consteval.Env)"
    )
}

/// Port of `checker/instr_measure.py::_check_cs616_reps`.
pub fn _check_cs616_reps(_stmt: &Instruction, _diags: &mut Diagnostics) {
    todo!(
        "checker/instr_measure.py::_check_cs616_reps (omitted: measure: dict, env: consteval.Env)"
    )
}

/// Port of `checker/instr_measure.py::_check_excite_magnitude`.
pub fn _check_excite_magnitude(_stmt: &Instruction, _diags: &mut Diagnostics) {
    todo!(
        "checker/instr_measure.py::_check_excite_magnitude (omitted: measure: dict, env: consteval.Env)"
    )
}

/// Port of `checker/instr_measure.py::is_sequential`.
pub fn is_sequential(_program: &Program, _profile: &Profile) -> bool {
    todo!("checker/instr_measure.py::is_sequential (omitted: env: consteval.Env)")
}

/// Port of `checker/instr_measure.py::check_scan_delay_budget`.
pub fn check_scan_delay_budget(
    _program: &Program,
    _profile: &Profile,
    _diags: &mut Diagnostics,
    _zero: bool,
) {
    todo!("checker/instr_measure.py::check_scan_delay_budget (omitted: env: consteval.Env)")
}

/// Port of `checker/instr_measure.py::check_cpi_time`.
pub fn check_cpi_time(_program: &Program, _profile: &Profile, _diags: &mut Diagnostics) {
    todo!("checker/instr_measure.py::check_cpi_time (omitted: env: consteval.Env)")
}

/// Port of `checker/instr_measure.py::_value`.
pub fn _value(_expr: Option<&Expr>) {
    todo!("checker/instr_measure.py::_value (omitted: env: consteval.Env)")
}

/// Port of `checker/instr_measure.py::_measurement_output_count`.
pub fn _measurement_output_count(_stmt: &Instruction, _reps: i64) -> i64 {
    todo!("checker/instr_measure.py::_measurement_output_count (omitted: env: consteval.Env)")
}

/// Port of `checker/instr_measure.py::_measurement_aliases`.
pub fn _measurement_aliases(_program: &Program) {
    todo!(
        "checker/instr_measure.py::_measurement_aliases (omitted: tables: untyped, env: consteval.Env)"
    )
}

/// Port of `checker/instr_measure.py::_build_measurement_aliases`.
pub fn _build_measurement_aliases(_program: &Program) {
    todo!(
        "checker/instr_measure.py::_build_measurement_aliases (omitted: tables: untyped, env: consteval.Env)"
    )
}

/// Port of `checker/instr_measure.py::_alias_overrun`.
pub fn _alias_overrun(_stmt: &Instruction, _count: i64, _profile: &Profile) -> bool {
    todo!(
        "checker/instr_measure.py::_alias_overrun (omitted: dest: untyped, alias: untyped, env: consteval.Env, signed_index: untyped)"
    )
}

/// Port of `checker/instr_measure.py::check_reps`.
pub fn check_reps(_program: &Program, _profile: &Profile, _diags: &mut Diagnostics) {
    todo!("checker/instr_measure.py::check_reps (omitted: env: consteval.Env)")
}

/// Port of `checker/instr_measure.py::_check_bridge_span`.
pub fn _check_bridge_span(_program: &Program, _profile: &Profile, _diags: &mut Diagnostics) {
    todo!("checker/instr_measure.py::_check_bridge_span (omitted: env: consteval.Env)")
}

/// Port of `checker/instr_measure.py::_insert_in_line_order`.
pub fn _insert_in_line_order(_diags: &mut Diagnostics, _stmt: &Instruction, _texts: Vec<String>) {
    todo!("checker/instr_measure.py::_insert_in_line_order")
}

/// Port of `checker/instr_measure.py::_pulse_index`.
pub fn _pulse_index(_code: i64) -> Option<(i64, i64)> {
    todo!("checker/instr_measure.py::_pulse_index")
}

/// Port of `checker/instr_measure.py::_check_pulse_span`.
pub fn _check_pulse_span(_program: &Program, _profile: &Profile, _diags: &mut Diagnostics) {
    todo!("checker/instr_measure.py::_check_pulse_span (omitted: env: consteval.Env)")
}

/// Port of `checker/instr_measure.py::_dest_overrun`.
pub fn _dest_overrun() -> Option<String> {
    todo!(
        "checker/instr_measure.py::_dest_overrun (omitted: stmt: untyped, dest: untyped, reps: untyped, profile: untyped, env: untyped, alias_storage: untyped, scope: untyped)"
    )
}

/// Port of `checker/instr_measure.py::_check_bridge_burst`.
pub fn _check_bridge_burst(_program: &Program, _profile: &Profile, _diags: &mut Diagnostics) {
    todo!("checker/instr_measure.py::_check_bridge_burst (omitted: env: consteval.Env)")
}

/// Port of `checker/instr_measure.py::_byte`.
pub fn _byte(_value: i64) -> i64 {
    todo!("checker/instr_measure.py::_byte")
}
