//! Port of `mode.py`.

#![allow(dead_code, unused_variables, unused_imports)]

use std::collections::BTreeSet;
use std::rc::Rc;

use crate::diagnostics::{DiagRef, Diagnostic, Diagnostics};
use crate::profiles::Profile;
use crate::syntax::{
    Assignment, Call, Case, ConstDecl, Declaration, Expr, ExprRef, Index, Instruction, Juxtaposed,
    Member, Number, OrderedMap, Program, ProgramRef, Stmt, StmtRef, SubDef, Unparsed, VarDecl,
};

/// Port of `mode.py::_const_value`.
pub fn _const_value() -> Option<f64> {
    todo!("mode.py::_const_value (omitted: expr: untyped, value_of: Callable)")
}

/// Port of `mode.py::_numeric_constant`.
pub fn _numeric_constant() -> bool {
    todo!(
        "mode.py::_numeric_constant (omitted: expr: untyped, is_constant: Callable, value_of: Callable)"
    )
}

/// Port of `mode.py::decide`.
pub fn decide(_statements: &[StmtRef], _profile: &Profile) -> String {
    todo!("mode.py::decide (omitted: is_constant: Callable, value_of: Callable)")
}

/// Port of `mode.py::_keyword`.
pub fn _keyword(_stmt: &Stmt) -> String {
    todo!("mode.py::_keyword")
}

/// Port of `mode.py::_arg`.
pub fn _arg(_i: i64) {
    todo!("mode.py::_arg (omitted: args: list)")
}

/// Port of `mode.py::_block_word`.
pub fn _block_word() -> Option<String> {
    todo!("mode.py::_block_word (omitted: args: list)")
}
