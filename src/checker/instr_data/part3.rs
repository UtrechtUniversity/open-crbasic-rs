//! Port of `checker/instr_data.py` (lines 1474-1963).

#![allow(dead_code, unused_variables, unused_imports)]

use std::collections::BTreeSet;
use std::rc::Rc;

use crate::diagnostics::{DiagRef, Diagnostic, Diagnostics};
use crate::profiles::Profile;
use crate::syntax::{
    Assignment, Call, Case, ConstDecl, Declaration, Expr, ExprRef, Index, Instruction, Juxtaposed,
    Member, Number, OrderedMap, Program, ProgramRef, Stmt, StmtRef, SubDef, Unparsed, VarDecl,
};

/// Port of `checker/instr_data.py::_check_stage2a`.
pub fn _check_stage2a(
    _program: &Program,
    _profile: &Profile,
    _diags: &mut Diagnostics,
    _destinations: bool,
) -> bool {
    todo!("checker/instr_data.py::_check_stage2a (omitted: env: untyped)")
}

/// Port of `checker/instr_data.py::check_eventcount_tables`.
pub fn check_eventcount_tables(_program: &Program, _profile: &Profile, _diags: &mut Diagnostics) {
    todo!("checker/instr_data.py::check_eventcount_tables")
}

/// Port of `checker/instr_data.py::_statement_exprs`.
pub fn _statement_exprs(_st: &Stmt) -> Vec<ExprRef> {
    todo!("checker/instr_data.py::_statement_exprs")
}

/// Port of `checker/instr_data.py::_members`.
pub fn _members(_expr: Option<&Expr>) {
    todo!("checker/instr_data.py::_members")
}

/// Port of `checker/instr_data.py::_declared_table_name`.
pub fn _declared_table_name(_st: &Instruction) -> Option<String> {
    todo!("checker/instr_data.py::_declared_table_name")
}

/// Port of `checker/instr_data.py::_illegal_table_name`.
pub fn _illegal_table_name(_st: &Instruction) -> Vec<DiagRef> {
    todo!("checker/instr_data.py::_illegal_table_name")
}

/// Port of `checker/instr_data.py::is_table_name_diag`.
pub fn is_table_name_diag(_d: &Diagnostic) -> bool {
    todo!("checker/instr_data.py::is_table_name_diag")
}

/// Port of `checker/instr_data.py::finalize_illegal_table_names`.
pub fn finalize_illegal_table_names(
    _program: &Program,
    _profile: &Profile,
    _diags: &mut Diagnostics,
) {
    todo!("checker/instr_data.py::finalize_illegal_table_names")
}

/// Port of `checker/instr_data.py::_constant_data_type`.
pub fn _constant_data_type(_stmt: &Instruction, _pos: i64) -> Option<i64> {
    todo!("checker/instr_data.py::_constant_data_type (omitted: env: consteval.Env)")
}

/// Port of `checker/instr_data.py::_fieldless`.
pub fn _fieldless(_stmt: &Instruction, _profile: &Profile) -> bool {
    todo!("checker/instr_data.py::_fieldless (omitted: env: consteval.Env)")
}

/// Port of `checker/instr_data.py::check_output_datatype`.
pub fn check_output_datatype(_program: &Program, _profile: &Profile, _diags: &mut Diagnostics) {
    todo!("checker/instr_data.py::check_output_datatype (omitted: env: consteval.Env)")
}

/// Port of `checker/instr_data.py::_odd_pair_reps`.
pub fn _odd_pair_reps(_stmt: &Instruction) -> Option<String> {
    todo!("checker/instr_data.py::_odd_pair_reps (omitted: env: consteval.Env)")
}

/// Port of `checker/instr_data.py::check_odd_pair_reps`.
pub fn check_odd_pair_reps(_program: &Program, _profile: &Profile, _diags: &mut Diagnostics) {
    todo!("checker/instr_data.py::check_odd_pair_reps (omitted: env: consteval.Env)")
}
