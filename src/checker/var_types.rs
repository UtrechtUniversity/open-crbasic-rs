//! Port of `checker/var_types.py`.

#![allow(dead_code, unused_variables, unused_imports)]

use std::collections::BTreeSet;
use std::rc::Rc;

use crate::diagnostics::{DiagRef, Diagnostic, Diagnostics};
use crate::profiles::Profile;
use crate::syntax::{
    Assignment, Call, Case, ConstDecl, Declaration, Expr, ExprRef, Index, Instruction, Juxtaposed,
    Member, Number, OrderedMap, Program, ProgramRef, Stmt, StmtRef, SubDef, Unparsed, VarDecl,
};

/// Port of `checker/var_types.py::_types`.
pub fn _types(_program: &Program, _scoped: bool) {
    todo!("checker/var_types.py::_types")
}

/// Port of `checker/var_types.py::_Typer`.
#[derive(Debug, Default)]
pub struct Typer {}

impl Typer {
    /// Port of `checker/var_types.py::_Typer::__init__`.
    pub fn __init__(
        &self,
        _program: &Program,
        _scope: Option<&Program>,
        _profile: Option<&Profile>,
    ) {
        todo!("checker/var_types.py::_Typer::__init__")
    }

    /// Port of `checker/var_types.py::_Typer::base`.
    pub fn base(&self, _name: &str) -> String {
        todo!("checker/var_types.py::_Typer::base")
    }

    /// Port of `checker/var_types.py::_Typer::variable`.
    pub fn variable(&self, _expr: Option<&Expr>) -> Option<String> {
        todo!("checker/var_types.py::_Typer::variable")
    }

    /// Port of `checker/var_types.py::_Typer::of`.
    pub fn of(&self, _expr: Option<&Expr>, _depth: i64) -> Option<String> {
        todo!("checker/var_types.py::_Typer::of")
    }
}

/// Port of `checker/var_types.py::_subexpressions`.
pub fn _subexpressions(_expr: Option<&Expr>) {
    todo!("checker/var_types.py::_subexpressions")
}

/// Port of `checker/var_types.py::_statement_expressions`.
pub fn _statement_expressions(_stmt: &Stmt) {
    todo!("checker/var_types.py::_statement_expressions")
}

/// Port of `checker/var_types.py::_string_typed`.
pub fn _string_typed(_typer: &mut Typer, _node: Option<&Expr>) -> bool {
    todo!("checker/var_types.py::_string_typed")
}

/// Port of `checker/var_types.py::_in_source_order`.
pub fn _in_source_order(_expr: Option<&Expr>) {
    todo!("checker/var_types.py::_in_source_order")
}

/// Port of `checker/var_types.py::_string_operand_messages`.
pub fn _string_operand_messages(_typer: &mut Typer, _stmt: &Stmt) -> Vec<String> {
    todo!("checker/var_types.py::_string_operand_messages")
}

/// Port of `checker/var_types.py::_snmp_mismatch`.
pub fn _snmp_mismatch(_typer: &mut Typer, _name: Option<&Expr>, _snmp_type: &str) -> bool {
    todo!("checker/var_types.py::_snmp_mismatch")
}

/// Port of `checker/var_types.py::_same_type`.
pub fn _same_type(_dest: &str, _source: &str) -> bool {
    todo!("checker/var_types.py::_same_type")
}

/// Port of `checker/var_types.py::_indexed_past_first`.
pub fn _indexed_past_first(_typer: &mut Typer, _dest: Option<&Expr>) -> bool {
    todo!("checker/var_types.py::_indexed_past_first (omitted: env: consteval.Env)")
}

/// Port of `checker/var_types.py::check_string_operands`.
pub fn check_string_operands(_program: &Program, _diags: &mut Diagnostics) {
    todo!("checker/var_types.py::check_string_operands (omitted: profile: untyped)")
}

/// Port of `checker/var_types.py::check`.
pub fn check(_program: &Program, _diags: &mut Diagnostics) {
    todo!("checker/var_types.py::check (omitted: profile: untyped, env: consteval.Env)")
}

/// Port of `checker/var_types.py::_array_size`.
pub fn _array_size(
    _typer: &mut Typer,
    _dims: &OrderedMap<String, Vec<i64>>,
    _expr: Option<&Expr>,
) -> Option<i64> {
    todo!("checker/var_types.py::_array_size")
}

/// Port of `checker/var_types.py::_ctype_illegal`.
pub fn _ctype_illegal(_node: &Index) -> bool {
    todo!(
        "checker/var_types.py::_ctype_illegal (omitted: env: untyped, legal: frozenset, known: set | None)"
    )
}

/// Port of `checker/var_types.py::check_ctype`.
pub fn check_ctype(_program: &Program, _diags: &mut Diagnostics) {
    todo!("checker/var_types.py::check_ctype (omitted: profile: untyped)")
}

/// Port of `checker/var_types.py::_symbol_value`.
pub fn _symbol_value(_name: &str) {
    todo!("checker/var_types.py::_symbol_value (omitted: env: untyped)")
}

/// Port of `checker/var_types.py::_dereference_names`.
pub fn _dereference_names(_top: Option<&Expr>) {
    todo!("checker/var_types.py::_dereference_names")
}

/// Port of `checker/var_types.py::check_pointer_deref`.
pub fn check_pointer_deref(_program: &Program, _diags: &mut Diagnostics) {
    todo!("checker/var_types.py::check_pointer_deref (omitted: profile: untyped)")
}
