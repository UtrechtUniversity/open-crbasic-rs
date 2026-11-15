//! Port of `checker/instr_measure.py` (lines 1541-1803).

#![allow(dead_code, unused_variables, unused_imports)]

use std::collections::BTreeSet;
use std::rc::Rc;

use crate::diagnostics::{DiagRef, Diagnostic, Diagnostics};
use crate::profiles::Profile;
use crate::syntax::{
    Assignment, Call, Case, ConstDecl, Declaration, Expr, ExprRef, Index, Instruction, Juxtaposed,
    Member, Number, OrderedMap, Program, ProgramRef, Stmt, StmtRef, SubDef, Unparsed, VarDecl,
};

/// Port of `checker/instr_measure.py::_builder_counts`.
pub fn _builder_counts(_kw: &str) {
    todo!("checker/instr_measure.py::_builder_counts (omitted: args: list, env: consteval.Env)")
}

/// Port of `checker/instr_measure.py::check_dest_bounds`.
pub fn check_dest_bounds(_program: &Program, _profile: &Profile, _diags: &mut Diagnostics) {
    todo!("checker/instr_measure.py::check_dest_bounds (omitted: env: consteval.Env)")
}

/// Port of `checker/instr_measure.py::_index_value`.
pub fn _index_value(_arg: Option<&Expr>) -> Option<i64> {
    todo!("checker/instr_measure.py::_index_value (omitted: env: consteval.Env | None)")
}

/// Port of `checker/instr_measure.py::_flat_offset`.
pub fn _flat_offset(_dest: &Expr, _shape: Vec<i64>) -> Option<i64> {
    todo!("checker/instr_measure.py::_flat_offset (omitted: env: consteval.Env | None)")
}
