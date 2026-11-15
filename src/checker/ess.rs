//! Port of `checker/ess.py`.

#![allow(dead_code, unused_variables, unused_imports)]

use std::collections::BTreeSet;
use std::rc::Rc;

use crate::diagnostics::{DiagRef, Diagnostic, Diagnostics};
use crate::profiles::Profile;
use crate::syntax::{
    Assignment, Call, Case, ConstDecl, Declaration, Expr, ExprRef, Index, Instruction, Juxtaposed,
    Member, Number, OrderedMap, Program, ProgramRef, Stmt, StmtRef, SubDef, Unparsed, VarDecl,
};

/// Port of `checker/ess.py::_table`.
pub fn _table(_data_dir: &str) {
    todo!("checker/ess.py::_table")
}

/// Port of `checker/ess.py::_const_int`.
pub fn _const_int(_expr: Option<&Expr>) -> Option<i64> {
    todo!("checker/ess.py::_const_int (omitted: env: consteval.Env)")
}

/// Port of `checker/ess.py::check`.
pub fn check(_program: &Program, _diags: &mut Diagnostics) {
    todo!("checker/ess.py::check (omitted: profile: untyped, env: consteval.Env)")
}
