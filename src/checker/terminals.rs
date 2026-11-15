//! Port of `checker/terminals.py`.

#![allow(dead_code, unused_variables, unused_imports)]

use std::collections::BTreeSet;
use std::rc::Rc;

use crate::diagnostics::{DiagRef, Diagnostic, Diagnostics};
use crate::profiles::Profile;
use crate::syntax::{
    Assignment, Call, Case, ConstDecl, Declaration, Expr, ExprRef, Index, Instruction, Juxtaposed,
    Member, Number, OrderedMap, Program, ProgramRef, Stmt, StmtRef, SubDef, Unparsed, VarDecl,
};

/// Port of `checker/terminals.py::_ranges`.
pub fn _ranges() {
    todo!("checker/terminals.py::_ranges (omitted: kind: dict)")
}

/// Port of `checker/terminals.py::_in_range`.
pub fn _in_range(_v: f64) -> bool {
    todo!("checker/terminals.py::_in_range (omitted: kind: dict)")
}

/// Port of `checker/terminals.py::_cr6_burst_kind`.
pub fn _cr6_burst_kind() -> bool {
    todo!("checker/terminals.py::_cr6_burst_kind (omitted: kind: dict)")
}

/// Port of `checker/terminals.py::_evaluate`.
pub fn _evaluate(_value: f64) -> (bool, bool, i64) {
    todo!("checker/terminals.py::_evaluate (omitted: kind: dict)")
}

/// Port of `checker/terminals.py::_bus_port`.
pub fn _bus_port() {
    todo!(
        "checker/terminals.py::_bus_port (omitted: arg: untyped, env: untyped, profile: untyped, names: untyped)"
    )
}

/// Port of `checker/terminals.py::_check_bus_port`.
pub fn _check_bus_port() {
    todo!(
        "checker/terminals.py::_check_bus_port (omitted: stmt: untyped, kind: untyped, value: untyped, diags: untyped)"
    )
}

/// Port of `checker/terminals.py::check_i2c_open_order`.
pub fn check_i2c_open_order(_program: &Program, _profile: &Profile, _diags: &mut Diagnostics) {
    todo!("checker/terminals.py::check_i2c_open_order (omitted: env: consteval.Env)")
}

/// Port of `checker/terminals.py::check`.
pub fn check(_program: &Program, _profile: &Profile, _diags: &mut Diagnostics) {
    todo!("checker/terminals.py::check (omitted: env: consteval.Env)")
}

/// Port of `checker/terminals.py::_const_int`.
pub fn _const_int(_stmt: &Instruction, _pos: i64, _truncate: bool) -> Option<i64> {
    todo!("checker/terminals.py::_const_int (omitted: env: consteval.Env)")
}

/// Port of `checker/terminals.py::_check_thermistor_span`.
pub fn _check_thermistor_span(
    _stmt: &Instruction,
    _profile: &Profile,
    _diags: &mut Diagnostics,
    _reps: i64,
) {
    todo!("checker/terminals.py::_check_thermistor_span (omitted: env: consteval.Env)")
}

/// Port of `checker/terminals.py::_check_bridge_excitation`.
pub fn _check_bridge_excitation(
    _stmt: &Instruction,
    _profile: &Profile,
    _diags: &mut Diagnostics,
    _channel_pos: i64,
) {
    todo!("checker/terminals.py::_check_bridge_excitation (omitted: env: consteval.Env)")
}

/// Port of `checker/terminals.py::_pulse_config`.
pub fn _pulse_config(
    _stmt: &Instruction,
    _profile: &Profile,
    _diags: &mut Diagnostics,
    _reps: Option<i64>,
) -> Vec<String> {
    todo!("checker/terminals.py::_pulse_config (omitted: kind: dict, env: consteval.Env)")
}

/// Port of `checker/terminals.py::_check_acpower`.
pub fn _check_acpower(_stmt: &Instruction, _diags: &mut Diagnostics) -> bool {
    todo!("checker/terminals.py::_check_acpower (omitted: kind: dict, env: consteval.Env)")
}

/// Port of `checker/terminals.py::_check_arg`.
pub fn _check_arg(
    _stmt: &Instruction,
    _pos: i64,
    _diags: &mut Diagnostics,
    _span: i64,
    _reps: Option<i64>,
    _width: i64,
) {
    todo!("checker/terminals.py::_check_arg (omitted: kind: dict, env: consteval.Env)")
}
