//! Port of `checker/expressions.py` (lines 1-797).

#![allow(dead_code, unused_variables, unused_imports)]

use std::collections::BTreeSet;
use std::rc::Rc;

use crate::diagnostics::{DiagRef, Diagnostic, Diagnostics};
use crate::profiles::Profile;
use crate::syntax::{
    Assignment, Call, Case, ConstDecl, Declaration, Expr, ExprRef, Index, Instruction, Juxtaposed,
    Member, Number, OrderedMap, Program, ProgramRef, Stmt, StmtRef, SubDef, Unparsed, VarDecl,
};

/// Port of `checker/expressions.py::_has_dotted_field`.
pub fn _has_dotted_field(_expr: Option<&Expr>) -> bool {
    todo!("checker/expressions.py::_has_dotted_field")
}

/// Port of `checker/expressions.py::_variable_refs`.
pub fn _variable_refs(_expr: Option<&Expr>, _variable_names: &BTreeSet<String>) -> Vec<String> {
    todo!("checker/expressions.py::_variable_refs")
}

/// Port of `checker/expressions.py::set_arity_context`.
pub fn set_arity_context(_program: &Program, _declared: Option<BTreeSet<String>>) {
    todo!("checker/expressions.py::set_arity_context (omitted: profile: untyped)")
}

/// Port of `checker/expressions.py::_timer_arg`.
pub fn _timer_arg(_declared: &BTreeSet<String>) {
    todo!("checker/expressions.py::_timer_arg (omitted: arg: untyped, env: untyped)")
}

/// Port of `checker/expressions.py::_timeout_wide`.
pub fn _timeout_wide() -> bool {
    todo!("checker/expressions.py::_timeout_wide (omitted: arg: untyped, profile: untyped)")
}

/// Port of `checker/expressions.py::_network_integer_problem`.
pub fn _network_integer_problem() {
    todo!(
        "checker/expressions.py::_network_integer_problem (omitted: name: untyped, arg: untyped, profile: untyped)"
    )
}

/// Port of `checker/expressions.py::_not_integer_type`.
pub fn _not_integer_type() -> bool {
    todo!("checker/expressions.py::_not_integer_type (omitted: arg: untyped)")
}

/// Port of `checker/expressions.py::_not_constant_integer`.
pub fn _not_constant_integer() -> bool {
    todo!("checker/expressions.py::_not_constant_integer (omitted: arg: untyped)")
}

/// Port of `checker/expressions.py::_type_code_kinds`.
pub fn _type_code_kinds() -> OrderedMap<usize, String> {
    todo!("checker/expressions.py::_type_code_kinds")
}

/// Port of `checker/expressions.py::_position_kinds`.
pub fn _position_kinds(_params: Vec<i64>, _keyword: &str) -> OrderedMap<usize, String> {
    todo!("checker/expressions.py::_position_kinds (omitted: profile: untyped)")
}

/// Port of `checker/expressions.py::_undefined_names`.
pub fn _undefined_names(_expr: Option<&Expr>, _known: &BTreeSet<String>) -> Vec<String> {
    todo!("checker/expressions.py::_undefined_names")
}

/// Port of `checker/expressions.py::_undefined_names_walk`.
pub fn _undefined_names_walk(_expr: Option<&Expr>, _known: &BTreeSet<String>) -> Vec<String> {
    todo!("checker/expressions.py::_undefined_names_walk")
}
