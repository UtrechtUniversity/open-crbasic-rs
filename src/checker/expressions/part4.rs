//! Port of `checker/expressions.py` (lines 2353-3083).

#![allow(dead_code, unused_variables, unused_imports)]

use std::collections::BTreeSet;
use std::rc::Rc;

use crate::diagnostics::{DiagRef, Diagnostic, Diagnostics};
use crate::profiles::Profile;
use crate::syntax::{
    Assignment, Call, Case, ConstDecl, Declaration, Expr, ExprRef, Index, Instruction, Juxtaposed,
    Member, Number, OrderedMap, Program, ProgramRef, Stmt, StmtRef, SubDef, Unparsed, VarDecl,
};

/// Port of `checker/expressions.py::check_data_params`.
pub fn check_data_params(_program: &Program, _diags: &mut Diagnostics) {
    todo!("checker/expressions.py::check_data_params (omitted: profile: untyped)")
}

/// Port of `checker/expressions.py::_data_param_text`.
pub fn _data_param_text(_stmt: &Instruction, _i: i64) -> String {
    todo!("checker/expressions.py::_data_param_text")
}

/// Port of `checker/expressions.py::_check_data_instruction`.
pub fn _check_data_instruction(
    _stmt: &Instruction,
    _known: &BTreeSet<String>,
    _diags: &mut Diagnostics,
) {
    todo!("checker/expressions.py::_check_data_instruction (omitted: profile: untyped)")
}

/// Port of `checker/expressions.py::_check_unparsed_data`.
pub fn _check_unparsed_data(_stmt: &Unparsed, _diags: &mut Diagnostics) {
    todo!(
        "checker/expressions.py::_check_unparsed_data (omitted: profile: untyped, program: untyped)"
    )
}

/// Port of `checker/expressions.py::_call_arity_problem`.
pub fn _call_arity_problem(_name: &str, _args: &[Option<ExprRef>]) -> Option<String> {
    todo!("checker/expressions.py::_call_arity_problem (omitted: profile: untyped)")
}

/// Port of `checker/expressions.py::_find_arity_problem`.
pub fn _find_arity_problem(
    _expr: Option<&Expr>,
    _known: Option<BTreeSet<String>>,
    _constants: &BTreeSet<String>,
) -> Option<String> {
    todo!("checker/expressions.py::_find_arity_problem (omitted: profile: untyped)")
}

/// Port of `checker/expressions.py::_arity_message`.
pub fn _arity_message(_kind: &str, _text: &str, _at_eol: bool, _whole_statement: bool) -> String {
    todo!("checker/expressions.py::_arity_message (omitted: profile: untyped)")
}

/// Port of `checker/expressions.py::_is_bare_reference_problem`.
pub fn _is_bare_reference_problem(_name: &str, _declared_variables: &BTreeSet<String>) -> bool {
    todo!("checker/expressions.py::_is_bare_reference_problem (omitted: profile: untyped)")
}

/// Port of `checker/expressions.py::_find_bare_function_ref`.
pub fn _find_bare_function_ref(
    _expr: Option<&Expr>,
    _declared_variables: &BTreeSet<String>,
) -> bool {
    todo!("checker/expressions.py::_find_bare_function_ref (omitted: profile: untyped)")
}

/// Port of `checker/expressions.py::_bare_function_message`.
pub fn _bare_function_message(_text: &str, _whole_statement: bool, _at_eol: bool) -> String {
    todo!("checker/expressions.py::_bare_function_message (omitted: profile: untyped)")
}

/// Port of `checker/expressions.py::_struct_var_names`.
pub fn _struct_var_names(_program: &Program) -> BTreeSet<String> {
    todo!("checker/expressions.py::_struct_var_names")
}

/// Port of `checker/expressions.py::_multi_indexed_struct`.
pub fn _multi_indexed_struct(_expr: Option<&Expr>, _struct_vars: &BTreeSet<String>) -> bool {
    todo!("checker/expressions.py::_multi_indexed_struct")
}

/// Port of `checker/expressions.py::_hold`.
pub fn _hold(_diags: &mut Diagnostics, _problem: &str) {
    todo!("checker/expressions.py::_hold (omitted: held: list | None)")
}

/// Port of `checker/expressions.py::check_function_arity`.
pub fn check_function_arity(_program: &Program, _diags: &mut Diagnostics) {
    todo!(
        "checker/expressions.py::check_function_arity (omitted: profile: untyped, held: list | None)"
    )
}

/// Port of `checker/expressions.py::_function_target_message`.
pub fn _function_target_message(
    _stmt: &Assignment,
    _declared: &BTreeSet<String>,
) -> Option<String> {
    todo!("checker/expressions.py::_function_target_message (omitted: profile: untyped)")
}
