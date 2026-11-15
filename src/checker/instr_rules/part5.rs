//! Port of `checker/instr_rules.py` (lines 3085-3737).

#![allow(dead_code, unused_variables, unused_imports)]

use std::collections::BTreeSet;
use std::rc::Rc;

use crate::diagnostics::{DiagRef, Diagnostic, Diagnostics};
use crate::profiles::Profile;
use crate::syntax::{
    Assignment, Call, Case, ConstDecl, Declaration, Expr, ExprRef, Index, Instruction, Juxtaposed,
    Member, Number, OrderedMap, Program, ProgramRef, Stmt, StmtRef, SubDef, Unparsed, VarDecl,
};

/// Port of `checker/instr_rules.py::check_fieldcal_variable_args`.
pub fn check_fieldcal_variable_args(
    _program: &Program,
    _profile: &Profile,
    _diags: &mut Diagnostics,
) {
    todo!("checker/instr_rules.py::check_fieldcal_variable_args (omitted: env: untyped)")
}

/// Port of `checker/instr_rules.py::_fieldcal_string_subscript`.
pub fn _fieldcal_string_subscript() -> bool {
    todo!("checker/instr_rules.py::_fieldcal_string_subscript (omitted: arg: untyped)")
}

/// Port of `checker/instr_rules.py::check_matrix_destination`.
pub fn check_matrix_destination(
    _stmt: &Instruction,
    _program: &Program,
    _profile: &Profile,
    _diags: &mut Diagnostics,
) {
    todo!("checker/instr_rules.py::check_matrix_destination (omitted: env: consteval.Env)")
}

/// Port of `checker/instr_rules.py::_fieldcal_bad_variable`.
pub fn _fieldcal_bad_variable() -> bool {
    todo!("checker/instr_rules.py::_fieldcal_bad_variable (omitted: arg: untyped)")
}

/// Port of `checker/instr_rules.py::check_samplefieldcal`.
pub fn check_samplefieldcal(_program: &Program, _profile: &Profile, _diags: &mut Diagnostics) {
    todo!("checker/instr_rules.py::check_samplefieldcal")
}

/// Port of `checker/instr_rules.py::_cdm_type_value`.
pub fn _cdm_type_value() -> Option<i64> {
    todo!("checker/instr_rules.py::_cdm_type_value (omitted: kind: untyped)")
}

/// Port of `checker/instr_rules.py::cdm_null_module`.
pub fn cdm_null_module(_stmt: &Instruction) -> bool {
    todo!("checker/instr_rules.py::cdm_null_module (omitted: env: untyped)")
}

/// Port of `checker/instr_rules.py::_check_cdm_bridge_options`.
pub fn _check_cdm_bridge_options(_profile: &Profile) {
    todo!(
        "checker/instr_rules.py::_check_cdm_bridge_options (omitted: stmt: untyped, positions: untyped, diags: untyped, env: untyped, names: untyped, literals: untyped)"
    )
}

/// Port of `checker/instr_rules.py::_check_cdm_mux_options`.
pub fn _check_cdm_mux_options(_profile: &Profile) {
    todo!(
        "checker/instr_rules.py::_check_cdm_mux_options (omitted: stmt: untyped, module: untyped, diags: untyped, env: untyped, names: untyped, literals: untyped)"
    )
}

/// Port of `checker/instr_rules.py::check_cdm_address_types`.
pub fn check_cdm_address_types(_program: &Program, _profile: &Profile, _diags: &mut Diagnostics) {
    todo!("checker/instr_rules.py::check_cdm_address_types (omitted: env: untyped)")
}

/// Port of `checker/instr_rules.py::check_epi_scan_buffers`.
pub fn check_epi_scan_buffers(_program: &Program, _profile: &Profile, _diags: &mut Diagnostics) {
    todo!("checker/instr_rules.py::check_epi_scan_buffers (omitted: env: untyped)")
}

/// Port of `checker/instr_rules.py::_check_cdm_fft`.
pub fn _check_cdm_fft(_st: &Instruction, _us: i64, _profile: &Profile, _diags: &mut Diagnostics) {
    todo!("checker/instr_rules.py::_check_cdm_fft (omitted: env: untyped, names: untyped)")
}

/// Port of `checker/instr_rules.py::_const_int`.
pub fn _const_int() -> Option<i64> {
    todo!("checker/instr_rules.py::_const_int (omitted: arg: untyped, env: untyped)")
}

/// Port of `checker/instr_rules.py::check_avw200_scan_time`.
pub fn check_avw200_scan_time(_program: &Program, _profile: &Profile, _diags: &mut Diagnostics) {
    todo!("checker/instr_rules.py::check_avw200_scan_time (omitted: env: untyped)")
}

/// Port of `checker/instr_rules.py::check_mqtt_publish_payload`.
pub fn check_mqtt_publish_payload(
    _program: &Program,
    _profile: &Profile,
    _diags: &mut Diagnostics,
) {
    todo!("checker/instr_rules.py::check_mqtt_publish_payload")
}

/// Port of `checker/instr_rules.py::_numeric_expression`.
pub fn _numeric_expression(_expr: Option<&Expr>) -> bool {
    todo!("checker/instr_rules.py::_numeric_expression (omitted: types: dict)")
}

/// Port of `checker/instr_rules.py::_constant_name_text`.
pub fn _constant_name_text(_arg: Option<&Expr>, _depth: i64) -> Option<String> {
    todo!("checker/instr_rules.py::_constant_name_text (omitted: env: consteval.Env)")
}

/// Port of `checker/instr_rules.py::_check_port_index`.
pub fn _check_port_index(
    _stmt: &Instruction,
    _key: &str,
    _index: Option<&str>,
    _profile: &Profile,
    _found: &mut Diagnostics,
) {
    todo!(
        "checker/instr_rules.py::_check_port_index (omitted: cfg: dict | None, env: consteval.Env)"
    )
}

/// Port of `checker/instr_rules.py::check_settings_fields`.
pub fn check_settings_fields(
    _program: &Program,
    _profile: &Profile,
    _diags: &mut Diagnostics,
    _found: Option<&mut Diagnostics>,
) {
    todo!("checker/instr_rules.py::check_settings_fields (omitted: env: consteval.Env | None)")
}
