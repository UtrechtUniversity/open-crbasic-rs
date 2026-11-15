//! Port of `checker/expressions.py` (lines 1571-2352).

#![allow(dead_code, unused_variables, unused_imports)]

use std::collections::BTreeSet;
use std::rc::Rc;

use crate::diagnostics::{DiagRef, Diagnostic, Diagnostics};
use crate::profiles::Profile;
use crate::syntax::{
    Assignment, Call, Case, ConstDecl, Declaration, Expr, ExprRef, Index, Instruction, Juxtaposed,
    Member, Number, OrderedMap, Program, ProgramRef, Stmt, StmtRef, SubDef, Unparsed, VarDecl,
};

/// Port of `checker/expressions.py::check_table_members`.
pub fn check_table_members(_program: &Program, _diags: &mut Diagnostics) {
    todo!("checker/expressions.py::check_table_members (omitted: profile: untyped)")
}

/// Port of `checker/expressions.py::_dotted_members`.
pub fn _dotted_members(_expr: Option<&Expr>) {
    todo!("checker/expressions.py::_dotted_members")
}

/// Port of `checker/expressions.py::_unknown_status_member`.
pub fn _unknown_status_member() -> bool {
    todo!("checker/expressions.py::_unknown_status_member (omitted: m: untyped, profile: untyped)")
}

/// Port of `checker/expressions.py::_unknown_setting`.
pub fn _unknown_setting() -> bool {
    todo!("checker/expressions.py::_unknown_setting (omitted: m: untyped, profile: untyped)")
}

/// Port of `checker/expressions.py::_unknown_public_member`.
pub fn _unknown_public_member(_variables: &BTreeSet<String>) -> bool {
    todo!("checker/expressions.py::_unknown_public_member (omitted: m: untyped)")
}

/// Port of `checker/expressions.py::check_status_member_reads_late`.
pub fn check_status_member_reads_late(_program: &Program, _diags: &mut Diagnostics) {
    todo!("checker/expressions.py::check_status_member_reads_late (omitted: profile: untyped)")
}

/// Port of `checker/expressions.py::check_undefined_dotted_bases`.
pub fn check_undefined_dotted_bases(_program: &Program, _diags: &mut Diagnostics) {
    todo!("checker/expressions.py::check_undefined_dotted_bases (omitted: profile: untyped)")
}

/// Port of `checker/expressions.py::check_digit_members`.
pub fn check_digit_members(_program: &Program, _diags: &mut Diagnostics) {
    todo!("checker/expressions.py::check_digit_members (omitted: profile: untyped)")
}

/// Port of `checker/expressions.py::check_const_family_args`.
pub fn check_const_family_args(_program: &Program, _diags: &mut Diagnostics) {
    todo!("checker/expressions.py::check_const_family_args (omitted: profile: untyped)")
}

/// Port of `checker/expressions.py::_names_before_invalid_compound`.
pub fn _names_before_invalid_compound() -> Option<Vec<String>> {
    todo!(
        "checker/expressions.py::_names_before_invalid_compound (omitted: expr: untyped, known: untyped, read_only: untyped)"
    )
}

/// Port of `checker/expressions.py::_report_invalid_compound`.
pub fn _report_invalid_compound() -> bool {
    todo!(
        "checker/expressions.py::_report_invalid_compound (omitted: stmt: untyped, expr: untyped, text: untyped, known: untyped, read_only: untyped, profile: untyped, diags: untyped, whole_statement: untyped)"
    )
}

/// Port of `checker/expressions.py::check`.
pub fn check(_program: &Program, _diags: &mut Diagnostics) {
    todo!(
        "checker/expressions.py::check (omitted: profile: untyped, env: consteval.Env, skip: frozenset)"
    )
}

/// Port of `checker/expressions.py::check_consts`.
pub fn check_consts(_program: &Program, _diags: &mut Diagnostics) {
    todo!("checker/expressions.py::check_consts (omitted: profile: untyped)")
}

/// Port of `checker/expressions.py::_report_const_names`.
pub fn _report_const_names(_stmt: &ConstDecl, _names: Vec<String>, _diags: &mut Diagnostics) {
    todo!("checker/expressions.py::_report_const_names (omitted: profile: untyped)")
}
