//! Port of `checker/instr_measure.py` (lines 1-753).

#![allow(dead_code, unused_variables, unused_imports)]

use std::collections::BTreeSet;
use std::rc::Rc;

use crate::diagnostics::{DiagRef, Diagnostic, Diagnostics};
use crate::profiles::Profile;
use crate::syntax::{
    Assignment, Call, Case, ConstDecl, Declaration, Expr, ExprRef, Index, Instruction, Juxtaposed,
    Member, Number, OrderedMap, Program, ProgramRef, Stmt, StmtRef, SubDef, Unparsed, VarDecl,
};

/// Port of `checker/instr_measure.py::check`.
pub fn check(_program: &Program, _profile: &Profile, _diags: &mut Diagnostics) {
    todo!("checker/instr_measure.py::check (omitted: env: consteval.Env)")
}

/// Port of `checker/instr_measure.py::_check_timed_sdm`.
pub fn _check_timed_sdm(_program: &Program, _profile: &Profile, _diags: &mut Diagnostics) {
    todo!(
        "checker/instr_measure.py::_check_timed_sdm (omitted: statements: list, env: consteval.Env)"
    )
}

/// Port of `checker/instr_measure.py::_check_periodavg_channels`.
pub fn _check_periodavg_channels(
    _stmt: &Instruction,
    _profile: &Profile,
    _diags: &mut Diagnostics,
) {
    todo!("checker/instr_measure.py::_check_periodavg_channels (omitted: env: consteval.Env)")
}

/// Port of `checker/instr_measure.py::check_const_ranges`.
pub fn check_const_ranges(_profile: &Profile, _diags: &mut Diagnostics) {
    todo!("checker/instr_measure.py::check_const_ranges (omitted: flat: list, env: consteval.Env)")
}

/// Port of `checker/instr_measure.py::_tccomp_before_channel`.
pub fn _tccomp_before_channel() {
    todo!(
        "checker/instr_measure.py::_tccomp_before_channel (omitted: stmt: untyped, env: untyped, profile: untyped, names: untyped, diags: untyped)"
    )
}

/// Port of `checker/instr_measure.py::_tccomp_after_channel`.
pub fn _tccomp_after_channel() {
    todo!(
        "checker/instr_measure.py::_tccomp_after_channel (omitted: stmt: untyped, env: untyped, profile: untyped, names: untyped, diags: untyped)"
    )
}

/// Port of `checker/instr_measure.py::_apply_cdm_channel`.
pub fn _apply_cdm_channel() {
    todo!(
        "checker/instr_measure.py::_apply_cdm_channel (omitted: stmt: untyped, rule: untyped, env: untyped, profile: untyped, names: untyped, diags: untyped)"
    )
}

/// Port of `checker/instr_measure.py::_apply_const_rule`.
pub fn _apply_const_rule() {
    todo!(
        "checker/instr_measure.py::_apply_const_rule (omitted: stmt: untyped, rule: untyped, env: untyped, profile: untyped, names: untyped, diags: untyped)"
    )
}

/// Port of `checker/instr_measure.py::_matches_invalid_raw`.
pub fn _matches_invalid_raw() -> bool {
    todo!("checker/instr_measure.py::_matches_invalid_raw (omitted: value: object, invalid: dict)")
}

/// Port of `checker/instr_measure.py::_long_arg`.
pub fn _long_arg(_stmt: &Instruction, _pos: i64, _profile: &Profile) -> Option<i64> {
    todo!(
        "checker/instr_measure.py::_long_arg (omitted: rule: dict, env: consteval.Env, names: untyped)"
    )
}

/// Port of `checker/instr_measure.py::_raw_arg`.
pub fn _raw_arg(_stmt: &Instruction, _pos: i64) {
    todo!("checker/instr_measure.py::_raw_arg (omitted: env: consteval.Env)")
}

/// Port of `checker/instr_measure.py::_check_string_prefix`.
pub fn _check_string_prefix(_stmt: &Instruction, _diags: &mut Diagnostics) -> bool {
    todo!(
        "checker/instr_measure.py::_check_string_prefix (omitted: rule: dict, env: consteval.Env)"
    )
}

/// Port of `checker/instr_measure.py::_check_ch400status`.
pub fn _check_ch400status(_stmt: &Instruction, _diags: &mut Diagnostics) {
    todo!("checker/instr_measure.py::_check_ch400status (omitted: env: consteval.Env)")
}

/// Port of `checker/instr_measure.py::_check_fft_source`.
pub fn _check_fft_source(_stmt: &Instruction, _diags: &mut Diagnostics) {
    todo!("checker/instr_measure.py::_check_fft_source (omitted: dims: dict, env: consteval.Env)")
}

/// Port of `checker/instr_measure.py::_check_sdi12_structure_dest`.
pub fn _check_sdi12_structure_dest(
    _stmt: &Instruction,
    _program: &Program,
    _diags: &mut Diagnostics,
) {
    todo!("checker/instr_measure.py::_check_sdi12_structure_dest")
}

/// Port of `checker/instr_measure.py::_check_tablefile`.
pub fn _check_tablefile(
    _stmt: &Instruction,
    _diags: &mut Diagnostics,
    _profile: &Profile,
    _typer: &mut crate::checker::var_types::Typer,
) {
    todo!(
        "checker/instr_measure.py::_check_tablefile (omitted: cfg: dict, env: consteval.Env, names: tuple[frozenset[str], frozenset[str]])"
    )
}

/// Port of `checker/instr_measure.py::_name_text`.
pub fn _name_text(_arg: Option<&Expr>) -> Option<String> {
    todo!("checker/instr_measure.py::_name_text (omitted: env: consteval.Env)")
}

/// Port of `checker/instr_measure.py::_check_structure_members`.
pub fn _check_structure_members(_stmt: &Instruction, _program: &Program, _diags: &mut Diagnostics) {
    todo!(
        "checker/instr_measure.py::_check_structure_members (omitted: cfg: dict, env: consteval.Env)"
    )
}

/// Port of `checker/instr_measure.py::_structure_members`.
pub fn _structure_members(_program: &Program, _type_name: &str) {
    todo!("checker/instr_measure.py::_structure_members")
}

/// Port of `checker/instr_measure.py::check_pass1_values`.
pub fn check_pass1_values(_program: &Program, _profile: &Profile, _diags: &mut Diagnostics) {
    todo!("checker/instr_measure.py::check_pass1_values (omitted: env: consteval.Env)")
}

/// Port of `checker/instr_measure.py::_fnotch_value`.
pub fn _fnotch_value(_arg: Option<&Expr>) {
    todo!("checker/instr_measure.py::_fnotch_value (omitted: env: consteval.Env)")
}

/// Port of `checker/instr_measure.py::_check_fnotch_min`.
pub fn _check_fnotch_min(
    _stmt: &Instruction,
    _pos: i64,
    _diags: &mut Diagnostics,
    _integ_num: f64,
) {
    todo!("checker/instr_measure.py::_check_fnotch_min (omitted: cfg: dict, env: consteval.Env)")
}

/// Port of `checker/instr_measure.py::_const_value`.
pub fn _const_value(_arg: Option<&Expr>) {
    todo!("checker/instr_measure.py::_const_value (omitted: env: consteval.Env)")
}

/// Port of `checker/instr_measure.py::_int_value`.
pub fn _int_value(_arg: Option<&Expr>) -> Option<i64> {
    todo!("checker/instr_measure.py::_int_value (omitted: env: consteval.Env)")
}

/// Port of `checker/instr_measure.py::_check_gps_nmea`.
pub fn _check_gps_nmea(_stmt: &Instruction, _program: &Program, _diags: &mut Diagnostics) {
    todo!("checker/instr_measure.py::_check_gps_nmea (omitted: env: consteval.Env)")
}

/// Port of `checker/instr_measure.py::_check_gps_port`.
pub fn _check_gps_port(_stmt: &Instruction, _diags: &mut Diagnostics) {
    todo!("checker/instr_measure.py::_check_gps_port (omitted: cfg: dict, env: consteval.Env)")
}

/// Port of `checker/instr_measure.py::_check_range_code`.
pub fn _check_range_code(_stmt: &Instruction, _pos: i64, _diags: &mut Diagnostics) {
    todo!(
        "checker/instr_measure.py::_check_range_code (omitted: measure: dict, env: consteval.Env)"
    )
}

/// Port of `checker/instr_measure.py::_check_sw12`.
pub fn _check_sw12(_stmt: &Instruction, _diags: &mut Diagnostics) {
    todo!("checker/instr_measure.py::_check_sw12 (omitted: measure: dict, env: consteval.Env)")
}

/// Port of `checker/instr_measure.py::_delay_option_byte`.
pub fn _delay_option_byte(_value: i64) -> i64 {
    todo!("checker/instr_measure.py::_delay_option_byte")
}

/// Port of `checker/instr_measure.py::_task_memory_full`.
pub fn _task_memory_full(_stmt: &Instruction, _diags: &mut Diagnostics) {
    todo!("checker/instr_measure.py::_task_memory_full")
}
