//! Port of `checker/instr_rules.py` (lines 2312-3084).

#![allow(dead_code, unused_variables, unused_imports)]

use std::collections::BTreeSet;
use std::rc::Rc;

use crate::diagnostics::{DiagRef, Diagnostic, Diagnostics};
use crate::profiles::Profile;
use crate::syntax::{
    Assignment, Call, Case, ConstDecl, Declaration, Expr, ExprRef, Index, Instruction, Juxtaposed,
    Member, Number, OrderedMap, Program, ProgramRef, Stmt, StmtRef, SubDef, Unparsed, VarDecl,
};

/// Port of `checker/instr_rules.py::check_emailrecv_destination`.
pub fn check_emailrecv_destination(
    _program: &Program,
    _profile: &Profile,
    _diags: &mut Diagnostics,
) {
    todo!("checker/instr_rules.py::check_emailrecv_destination")
}

/// Port of `checker/instr_rules.py::check_tcpopen_handle_type`.
pub fn check_tcpopen_handle_type(_program: &Program, _profile: &Profile, _diags: &mut Diagnostics) {
    todo!("checker/instr_rules.py::check_tcpopen_handle_type")
}

/// Port of `checker/instr_rules.py::check_tcpopen_connections`.
pub fn check_tcpopen_connections(_program: &Program, _profile: &Profile, _diags: &mut Diagnostics) {
    todo!("checker/instr_rules.py::check_tcpopen_connections (omitted: env: consteval.Env)")
}

/// Port of `checker/instr_rules.py::check_pulsecount_channels`.
pub fn check_pulsecount_channels(_program: &Program, _profile: &Profile, _diags: &mut Diagnostics) {
    todo!("checker/instr_rules.py::check_pulsecount_channels")
}

/// Port of `checker/instr_rules.py::check_sdi12_nan_fill`.
pub fn check_sdi12_nan_fill(
    _st: &Instruction,
    _program: &Program,
    _profile: &Profile,
    _diags: &mut Diagnostics,
) {
    todo!("checker/instr_rules.py::check_sdi12_nan_fill (omitted: env: consteval.Env)")
}

/// Port of `checker/instr_rules.py::check_terminal_conflicts`.
pub fn check_terminal_conflicts(_program: &Program, _profile: &Profile, _diags: &mut Diagnostics) {
    todo!("checker/instr_rules.py::check_terminal_conflicts (omitted: env: untyped)")
}

/// Port of `checker/instr_rules.py::_port_number`.
pub fn _port_number(_arg: Option<&Expr>) -> Option<i64> {
    todo!("checker/instr_rules.py::_port_number")
}

/// Port of `checker/instr_rules.py::check_spi_open_order`.
pub fn check_spi_open_order(_program: &Program, _profile: &Profile, _diags: &mut Diagnostics) {
    todo!("checker/instr_rules.py::check_spi_open_order")
}

/// Port of `checker/instr_rules.py::_dnp_long`.
pub fn _dnp_long(_arg: Option<&Expr>, _wrap16: bool) -> Option<i64> {
    todo!("checker/instr_rules.py::_dnp_long (omitted: conv: untyped)")
}

/// Port of `checker/instr_rules.py::_dnp_source`.
pub fn _dnp_source(_stmt: &Instruction) {
    todo!("checker/instr_rules.py::_dnp_source (omitted: conv: untyped, dims: dict)")
}

/// Port of `checker/instr_rules.py::_dnp_long_flag`.
pub fn _dnp_long_flag(_profile: &Profile) -> Option<bool> {
    todo!(
        "checker/instr_rules.py::_dnp_long_flag (omitted: arg: untyped, typer: untyped, env: untyped, routine_names: untyped)"
    )
}

/// Port of `checker/instr_rules.py::_dnp_statement`.
pub fn _dnp_statement(_stmt: &Instruction, _kind: Option<&str>, _long_flag: Option<bool>) {
    todo!("checker/instr_rules.py::_dnp_statement (omitted: conv: untyped, dims: untyped)")
}

/// Port of `checker/instr_rules.py::check_dnp_update`.
pub fn check_dnp_update(_program: &Program, _profile: &Profile, _diags: &mut Diagnostics) {
    todo!("checker/instr_rules.py::check_dnp_update (omitted: env: untyped)")
}

/// Port of `checker/instr_rules.py::check_portbridge_array`.
pub fn check_portbridge_array(
    _stmt: &Instruction,
    _program: &Program,
    _profile: &Profile,
    _diags: &mut Diagnostics,
) {
    todo!("checker/instr_rules.py::check_portbridge_array (omitted: env: consteval.Env)")
}

/// Port of `checker/instr_rules.py::cdm_unsupported_module`.
pub fn cdm_unsupported_module(_stmt: &Instruction) -> Option<String> {
    todo!("checker/instr_rules.py::cdm_unsupported_module (omitted: env: untyped)")
}

/// Port of `checker/instr_rules.py::_delay_option`.
pub fn _delay_option(_stmt: &Instruction) {
    todo!("checker/instr_rules.py::_delay_option (omitted: env: untyped)")
}
