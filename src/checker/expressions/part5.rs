//! Port of `checker/expressions.py` (lines 3084-3758).

#![allow(dead_code, unused_variables, unused_imports)]

use std::collections::BTreeSet;
use std::rc::Rc;

use crate::diagnostics::{DiagRef, Diagnostic, Diagnostics};
use crate::profiles::Profile;
use crate::syntax::{
    Assignment, Call, Case, ConstDecl, Declaration, Expr, ExprRef, Index, Instruction, Juxtaposed,
    Member, Number, OrderedMap, Program, ProgramRef, Stmt, StmtRef, SubDef, Unparsed, VarDecl,
};

/// Port of `checker/expressions.py::check_function_call_counts`.
pub fn check_function_call_counts(_program: &Program, _diags: &mut Diagnostics) {
    todo!("checker/expressions.py::check_function_call_counts (omitted: profile: untyped)")
}

/// Port of `checker/expressions.py::_index_nodes`.
pub fn _index_nodes(_expr: Option<&Expr>) {
    todo!("checker/expressions.py::_index_nodes")
}

/// Port of `checker/expressions.py::_check_for_headers`.
pub fn _check_for_headers(_program: &Program, _diags: &mut Diagnostics) {
    todo!("checker/expressions.py::_check_for_headers (omitted: profile: untyped, known: untyped)")
}

/// Port of `checker/expressions.py::_check_elseif_and_case`.
pub fn _check_elseif_and_case(_program: &Program, _diags: &mut Diagnostics) {
    todo!(
        "checker/expressions.py::_check_elseif_and_case (omitted: profile: untyped, known: untyped)"
    )
}

/// Port of `checker/expressions.py::_check_function_statements`.
pub fn _check_function_statements(_program: &Program, _diags: &mut Diagnostics) {
    todo!(
        "checker/expressions.py::_check_function_statements (omitted: profile: untyped, known: untyped)"
    )
}

/// Port of `checker/expressions.py::check_instruction_index_bounds`.
pub fn check_instruction_index_bounds(_program: &Program, _diags: &mut Diagnostics) {
    todo!(
        "checker/expressions.py::check_instruction_index_bounds (omitted: profile: untyped, env: consteval.Env)"
    )
}

/// Port of `checker/expressions.py::check_splitstr_element_reps`.
pub fn check_splitstr_element_reps(_program: &Program, _diags: &mut Diagnostics) {
    todo!(
        "checker/expressions.py::check_splitstr_element_reps (omitted: profile: untyped, env: untyped)"
    )
}

/// Port of `checker/expressions.py::_sdmsio4_count`.
pub fn _sdmsio4_count(_stmt: &Instruction) -> Option<i64> {
    todo!("checker/expressions.py::_sdmsio4_count (omitted: env: untyped)")
}

/// Port of `checker/expressions.py::check_sdmsio4_count`.
pub fn check_sdmsio4_count(_program: &Program, _diags: &mut Diagnostics) {
    todo!("checker/expressions.py::check_sdmsio4_count (omitted: profile: untyped, env: untyped)")
}

/// Port of `checker/expressions.py::check_timedcontrol_constants`.
pub fn check_timedcontrol_constants(_program: &Program, _diags: &mut Diagnostics) {
    todo!("checker/expressions.py::check_timedcontrol_constants (omitted: profile: untyped)")
}

/// Port of `checker/expressions.py::_names_in`.
pub fn _names_in() -> BTreeSet<String> {
    todo!("checker/expressions.py::_names_in (omitted: expr: untyped)")
}

/// Port of `checker/expressions.py::check_calibrate_size`.
pub fn check_calibrate_size(_program: &Program, _diags: &mut Diagnostics) {
    todo!("checker/expressions.py::check_calibrate_size (omitted: profile: untyped, env: untyped)")
}

/// Port of `checker/expressions.py::check_splitstr_whole_array`.
pub fn check_splitstr_whole_array(_program: &Program, _diags: &mut Diagnostics) {
    todo!(
        "checker/expressions.py::check_splitstr_whole_array (omitted: profile: untyped, env: untyped)"
    )
}

/// Port of `checker/expressions.py::check_subcall_element_bounds`.
pub fn check_subcall_element_bounds(_program: &Program, _diags: &mut Diagnostics) {
    todo!(
        "checker/expressions.py::check_subcall_element_bounds (omitted: profile: untyped, env: untyped)"
    )
}

/// Port of `checker/expressions.py::check_recursive_functions`.
pub fn check_recursive_functions(_program: &Program, _diags: &mut Diagnostics) {
    todo!("checker/expressions.py::check_recursive_functions (omitted: profile: untyped)")
}
