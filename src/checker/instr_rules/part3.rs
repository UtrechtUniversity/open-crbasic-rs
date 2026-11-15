//! Port of `checker/instr_rules.py` (lines 1539-2311).

#![allow(dead_code, unused_variables, unused_imports)]

use std::collections::BTreeSet;
use std::rc::Rc;

use crate::diagnostics::{DiagRef, Diagnostic, Diagnostics};
use crate::profiles::Profile;
use crate::syntax::{
    Assignment, Call, Case, ConstDecl, Declaration, Expr, ExprRef, Index, Instruction, Juxtaposed,
    Member, Number, OrderedMap, Program, ProgramRef, Stmt, StmtRef, SubDef, Unparsed, VarDecl,
};

/// Port of `checker/instr_rules.py::check_precision_variable`.
pub fn check_precision_variable(
    _stmt: &Instruction,
    _program: &Program,
    _profile: &Profile,
    _diags: &mut Diagnostics,
) {
    todo!("checker/instr_rules.py::check_precision_variable (omitted: env: consteval.Env)")
}

/// Port of `checker/instr_rules.py::check_vibrating_wire_diagnostics`.
pub fn check_vibrating_wire_diagnostics(
    _stmt: &Instruction,
    _program: &Program,
    _profile: &Profile,
    _diags: &mut Diagnostics,
) {
    todo!("checker/instr_rules.py::check_vibrating_wire_diagnostics (omitted: env: consteval.Env)")
}

/// Port of `checker/instr_rules.py::check_sdm_address`.
pub fn check_sdm_address(
    _stmt: &Instruction,
    _program: &Program,
    _profile: &Profile,
    _diags: &mut Diagnostics,
) {
    todo!("checker/instr_rules.py::check_sdm_address (omitted: env: consteval.Env)")
}

/// Port of `checker/instr_rules.py::_operator_tree`.
pub fn _operator_tree(_expr: Option<&Expr>) -> bool {
    todo!("checker/instr_rules.py::_operator_tree")
}

/// Port of `checker/instr_rules.py::check_avgrun_options`.
pub fn check_avgrun_options(
    _stmt: &Instruction,
    _program: &Program,
    _profile: &Profile,
    _diags: &mut Diagnostics,
) {
    todo!("checker/instr_rules.py::check_avgrun_options (omitted: env: consteval.Env)")
}

/// Port of `checker/instr_rules.py::check_option_values`.
pub fn check_option_values(_program: &Program, _profile: &Profile, _diags: &mut Diagnostics) {
    todo!("checker/instr_rules.py::check_option_values (omitted: env: consteval.Env)")
}

/// Port of `checker/instr_rules.py::_cdm_delay_overlong`.
pub fn _cdm_delay_overlong(_profile: &Profile) -> i64 {
    todo!(
        "checker/instr_rules.py::_cdm_delay_overlong (omitted: args: list, env: consteval.Env, names: untyped, literals: untyped)"
    )
}

/// Port of `checker/instr_rules.py::_matrix_shapes`.
pub fn _matrix_shapes(_program: &Program) -> OrderedMap<String, (i64, i64)> {
    todo!("checker/instr_rules.py::_matrix_shapes (omitted: env: consteval.Env)")
}

/// Port of `checker/instr_rules.py::matrix_undefined_operands`.
pub fn matrix_undefined_operands(_program: &Program, _profile: &Profile) -> Option<Vec<String>> {
    todo!(
        "checker/instr_rules.py::matrix_undefined_operands (omitted: stmt: untyped, env: untyped, known: untyped)"
    )
}

/// Port of `checker/instr_rules.py::_matrix_dimension_errors`.
pub fn _matrix_dimension_errors(_option: i64) -> Vec<String> {
    todo!(
        "checker/instr_rules.py::_matrix_dimension_errors (omitted: args: untyped, shapes: untyped)"
    )
}

/// Port of `checker/instr_rules.py::check_const_int_args`.
pub fn check_const_int_args(_program: &Program, _profile: &Profile, _diags: &mut Diagnostics) {
    todo!("checker/instr_rules.py::check_const_int_args (omitted: env: consteval.Env)")
}

/// Port of `checker/instr_rules.py::_strip_parens`.
pub fn _strip_parens(_text: &str) -> String {
    todo!("checker/instr_rules.py::_strip_parens")
}

/// Port of `checker/instr_rules.py::_paren_depth_ok`.
pub fn _paren_depth_ok(_text: &str) -> bool {
    todo!("checker/instr_rules.py::_paren_depth_ok")
}

/// Port of `checker/instr_rules.py::_is_integer_token`.
pub fn _is_integer_token(_text: &str, _arg: &Expr) -> bool {
    todo!("checker/instr_rules.py::_is_integer_token (omitted: env: consteval.Env)")
}

/// Port of `checker/instr_rules.py::check_serialbrk_comport`.
pub fn check_serialbrk_comport(
    _stmt: &Instruction,
    _program: &Program,
    _profile: &Profile,
    _diags: &mut Diagnostics,
) {
    todo!("checker/instr_rules.py::check_serialbrk_comport (omitted: env: consteval.Env)")
}

/// Port of `checker/instr_rules.py::_serialin_bad_destination`.
pub fn _serialin_bad_destination(_arg: &Expr, _types: &OrderedMap<String, String>) -> Option<bool> {
    todo!("checker/instr_rules.py::_serialin_bad_destination")
}

/// Port of `checker/instr_rules.py::check_serialin_destination`.
pub fn check_serialin_destination(
    _stmt: &Instruction,
    _program: &Program,
    _profile: &Profile,
    _diags: &mut Diagnostics,
) {
    todo!("checker/instr_rules.py::check_serialin_destination (omitted: env: consteval.Env)")
}

/// Port of `checker/instr_rules.py::check_serialin_comport`.
pub fn check_serialin_comport(
    _stmt: &Instruction,
    _program: &Program,
    _profile: &Profile,
    _diags: &mut Diagnostics,
) {
    todo!("checker/instr_rules.py::check_serialin_comport (omitted: env: consteval.Env)")
}

/// Port of `checker/instr_rules.py::check_serialinrecord`.
pub fn check_serialinrecord(_program: &Program, _profile: &Profile, _diags: &mut Diagnostics) {
    todo!("checker/instr_rules.py::check_serialinrecord (omitted: env: consteval.Env)")
}

/// Port of `checker/instr_rules.py::debug_history_oom`.
pub fn debug_history_oom(_stmt: &Instruction, _profile: &Profile, _program: &Program) -> bool {
    todo!("checker/instr_rules.py::debug_history_oom (omitted: env: untyped)")
}

/// Port of `checker/instr_rules.py::check_debug_args`.
pub fn check_debug_args(
    _stmt: &Instruction,
    _program: &Program,
    _profile: &Profile,
    _diags: &mut Diagnostics,
) {
    todo!("checker/instr_rules.py::check_debug_args (omitted: env: consteval.Env)")
}
