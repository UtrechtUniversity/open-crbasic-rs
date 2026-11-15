//! Port of `checker/symbols.py`.

#![allow(dead_code, unused_variables, unused_imports)]

use std::collections::BTreeSet;
use std::rc::Rc;

use crate::diagnostics::{DiagRef, Diagnostic, Diagnostics};
use crate::profiles::Profile;
use crate::syntax::{
    Assignment, Call, Case, ConstDecl, Declaration, Expr, ExprRef, Index, Instruction, Juxtaposed,
    Member, Number, OrderedMap, Program, ProgramRef, Stmt, StmtRef, SubDef, Unparsed, VarDecl,
};

/// Port of `checker/symbols.py::_walk_names`.
pub fn _walk_names(_expr: Option<&Expr>) {
    todo!("checker/symbols.py::_walk_names")
}

/// Port of `checker/symbols.py::_stmt_exprs`.
pub fn _stmt_exprs(_stmt: &Stmt) {
    todo!("checker/symbols.py::_stmt_exprs")
}

/// Port of `checker/symbols.py::_name_nodes`.
pub fn _name_nodes(_expr: Option<&Expr>) {
    todo!("checker/symbols.py::_name_nodes")
}

/// Port of `checker/symbols.py::is_reserved_predefined_name`.
pub fn is_reserved_predefined_name(_name: &str) -> bool {
    todo!("checker/symbols.py::is_reserved_predefined_name (omitted: profile: untyped)")
}

/// Port of `checker/symbols.py::report_predefined_name_collision`.
pub fn report_predefined_name_collision(
    _stmt: &Stmt,
    _name: &str,
    _diags: &mut Diagnostics,
) -> bool {
    todo!("checker/symbols.py::report_predefined_name_collision (omitted: profile: untyped)")
}

/// Port of `checker/symbols.py::report_function_name_collision`.
pub fn report_function_name_collision(_stmt: &Stmt, _name: &str, _diags: &mut Diagnostics) -> bool {
    todo!("checker/symbols.py::report_function_name_collision (omitted: profile: untyped)")
}

/// Port of `checker/symbols.py::check_local_name_collision`.
pub fn check_local_name_collision(_stmt: &Stmt, _diags: &mut Diagnostics) {
    todo!("checker/symbols.py::check_local_name_collision (omitted: profile: untyped)")
}
