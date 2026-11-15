//! Port of `checker/instr_rules.py` (lines 1-765).

#![allow(dead_code, unused_variables, unused_imports)]

use std::collections::BTreeSet;
use std::rc::Rc;

use crate::diagnostics::{DiagRef, Diagnostic, Diagnostics};
use crate::profiles::Profile;
use crate::syntax::{
    Assignment, Call, Case, ConstDecl, Declaration, Expr, ExprRef, Index, Instruction, Juxtaposed,
    Member, Number, OrderedMap, Program, ProgramRef, Stmt, StmtRef, SubDef, Unparsed, VarDecl,
};

/// Port of `checker/instr_rules.py::rule`.
pub fn rule(_phase: &str, _single_pass: Option<bool>) {
    todo!("checker/instr_rules.py::rule")
}

/// Port of `checker/instr_rules.py::run_rules`.
pub fn run_rules(_phase: &str, _program: &Program, _profile: &Profile, _diags: &mut Diagnostics) {
    todo!("checker/instr_rules.py::run_rules (omitted: env: consteval.Env)")
}

/// Port of `checker/instr_rules.py::unterminated_string_shape`.
pub fn unterminated_string_shape(
    _stmt: &Stmt,
    _profile: &Profile,
    _subs: &BTreeSet<String>,
    _operands: Option<BTreeSet<String>>,
    _functions: &BTreeSet<String>,
) -> Option<String> {
    todo!("checker/instr_rules.py::unterminated_string_shape")
}

/// Port of `checker/instr_rules.py::_function_call_string`.
pub fn _function_call_string(_text: &str, _functions: &BTreeSet<String>) -> Option<String> {
    todo!("checker/instr_rules.py::_function_call_string")
}

/// Port of `checker/instr_rules.py::user_functions`.
pub fn user_functions(_program: &Program) -> BTreeSet<String> {
    todo!("checker/instr_rules.py::user_functions")
}

/// Port of `checker/instr_rules.py::_past_last_param`.
pub fn _past_last_param(_text: &str, _count: i64) -> Option<String> {
    todo!("checker/instr_rules.py::_past_last_param")
}

/// Port of `checker/instr_rules.py::_check_unterminated_string_param`.
pub fn _check_unterminated_string_param(
    _program: &Program,
    _profile: &Profile,
    _diags: &mut Diagnostics,
) {
    todo!("checker/instr_rules.py::_check_unterminated_string_param")
}

/// Port of `checker/instr_rules.py::_real_params`.
pub fn _real_params(_params: Vec<i64>) -> Vec<i64> {
    todo!("checker/instr_rules.py::_real_params")
}

/// Port of `checker/instr_rules.py::_required_count`.
pub fn _required_count(_params: Vec<i64>) -> i64 {
    todo!("checker/instr_rules.py::_required_count")
}

/// Port of `checker/instr_rules.py::misplaced_returns`.
pub fn misplaced_returns(_program: &Program) {
    todo!("checker/instr_rules.py::misplaced_returns")
}

/// Port of `checker/instr_rules.py::check_return_placement`.
pub fn check_return_placement(_program: &Program, _profile: &Profile, _diags: &mut Diagnostics) {
    todo!("checker/instr_rules.py::check_return_placement")
}

/// Port of `checker/instr_rules.py::_return_statements`.
pub fn _return_statements(_program: &Program) {
    todo!("checker/instr_rules.py::_return_statements")
}

/// Port of `checker/instr_rules.py::check`.
pub fn check(_program: &Program, _profile: &Profile, _diags: &mut Diagnostics) {
    todo!("checker/instr_rules.py::check")
}

/// Port of `checker/instr_rules.py::given_count`.
pub fn given_count(_stmt: &Instruction, _profile: &Profile) -> i64 {
    todo!("checker/instr_rules.py::given_count")
}

/// Port of `checker/instr_rules.py::given_count_of`.
pub fn given_count_of(_args: Vec<String>) -> i64 {
    todo!("checker/instr_rules.py::given_count_of")
}

/// Port of `checker/instr_rules.py::shortfall_message`.
pub fn shortfall_message(_given: i64) -> Option<String> {
    todo!("checker/instr_rules.py::shortfall_message (omitted: record: dict)")
}

/// Port of `checker/instr_rules.py::_report_constant_destinations`.
pub fn _report_constant_destinations(
    _stmt: &Instruction,
    _profile: &Profile,
    _diags: &mut Diagnostics,
) {
    todo!(
        "checker/instr_rules.py::_report_constant_destinations (omitted: record: dict, params: list, program: untyped)"
    )
}

/// Port of `checker/instr_rules.py::_next_simple_token`.
pub fn _next_simple_token(_text: &str) -> String {
    todo!("checker/instr_rules.py::_next_simple_token")
}

/// Port of `checker/instr_rules.py::_too_many_eligible`.
pub fn _too_many_eligible(_params: Vec<i64>) -> bool {
    todo!("checker/instr_rules.py::_too_many_eligible")
}

/// Port of `checker/instr_rules.py::plain_expression_params`.
pub fn plain_expression_params(_params: Vec<i64>) -> bool {
    todo!("checker/instr_rules.py::plain_expression_params")
}

/// Port of `checker/instr_rules.py::check_too_many`.
pub fn check_too_many(_program: &Program, _profile: &Profile, _diags: &mut Diagnostics) {
    todo!("checker/instr_rules.py::check_too_many")
}

/// Port of `checker/instr_rules.py::_earlier_error_in_stmt`.
pub fn _earlier_error_in_stmt(_program: &Program, _diags: &mut Diagnostics) -> bool {
    todo!(
        "checker/instr_rules.py::_earlier_error_in_stmt (omitted: stmt: untyped, prefixes: tuple)"
    )
}

/// Port of `checker/instr_rules.py::check_essinitialize_too_many`.
pub fn check_essinitialize_too_many(
    _program: &Program,
    _profile: &Profile,
    _diags: &mut Diagnostics,
) {
    todo!("checker/instr_rules.py::check_essinitialize_too_many")
}

/// Port of `checker/instr_rules.py::_snmp_components`.
pub fn _snmp_components(_text: &str) {
    todo!("checker/instr_rules.py::_snmp_components")
}
