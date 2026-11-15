//! Port of `checker/structure.py` (lines 1294-2081).

#![allow(dead_code, unused_variables, unused_imports)]

use std::collections::BTreeSet;
use std::rc::Rc;

use crate::diagnostics::{DiagRef, Diagnostic, Diagnostics};
use crate::profiles::Profile;
use crate::syntax::{
    Assignment, Call, Case, ConstDecl, Declaration, Expr, ExprRef, Index, Instruction, Juxtaposed,
    Member, Number, OrderedMap, Program, ProgramRef, Stmt, StmtRef, SubDef, Unparsed, VarDecl,
};

/// Port of `checker/structure.py::check_pass1`.
pub fn check_pass1(_program: &Program, _diags: &mut Diagnostics) {
    todo!(
        "checker/structure.py::check_pass1 (omitted: profile: untyped, env: consteval.Env | None)"
    )
}

/// Port of `checker/structure.py::_expr_resolves`.
pub fn _expr_resolves(_e: Option<&Expr>, _declared: &BTreeSet<String>) -> bool {
    todo!("checker/structure.py::_expr_resolves (omitted: functions: untyped)")
}

/// Port of `checker/structure.py::_slowsequence_resolves`.
pub fn _slowsequence_resolves(_stmt: &Stmt, _declared: &BTreeSet<String>) -> bool {
    todo!("checker/structure.py::_slowsequence_resolves (omitted: functions: untyped)")
}

/// Port of `checker/structure.py::open_program_at_slowsequence`.
pub fn open_program_at_slowsequence(_program: &Program, _profile: &Profile) -> ProgramRef {
    todo!("checker/structure.py::open_program_at_slowsequence")
}
