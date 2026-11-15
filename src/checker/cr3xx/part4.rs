//! Port of `checker/cr3xx.py` (lines 2124-2364).

#![allow(dead_code, unused_variables, unused_imports)]

use std::collections::BTreeSet;
use std::rc::Rc;

use crate::diagnostics::{DiagRef, Diagnostic, Diagnostics};
use crate::profiles::Profile;
use crate::syntax::{
    Assignment, Call, Case, ConstDecl, Declaration, Expr, ExprRef, Index, Instruction, Juxtaposed,
    Member, Number, OrderedMap, Program, ProgramRef, Stmt, StmtRef, SubDef, Unparsed, VarDecl,
};

/// Port of `checker/cr3xx.py::order_unterminated_string`.
pub fn order_unterminated_string(_program: &Program, _diags: &mut Diagnostics) {
    todo!("checker/cr3xx.py::order_unterminated_string (omitted: profile: untyped)")
}

/// Port of `checker/cr3xx.py::semicolon_headers`.
pub fn semicolon_headers(_program: &Program, _diags: &mut Diagnostics) {
    todo!("checker/cr3xx.py::semicolon_headers (omitted: profile: untyped)")
}

/// Port of `checker/cr3xx.py::add_string_illegal`.
pub fn add_string_illegal(_program: &Program, _diags: &mut Diagnostics) {
    todo!("checker/cr3xx.py::add_string_illegal (omitted: profile: untyped)")
}

/// Port of `checker/cr3xx.py::_order_arith_string_lines`.
pub fn _order_arith_string_lines(_diags: &mut Diagnostics) {
    todo!("checker/cr3xx.py::_order_arith_string_lines")
}

/// Port of `checker/cr3xx.py::check_tcpopen_handle`.
pub fn check_tcpopen_handle(_program: &Program, _diags: &mut Diagnostics) {
    todo!("checker/cr3xx.py::check_tcpopen_handle (omitted: profile: untyped)")
}
