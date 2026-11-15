//! Port of `checker/structure.py` (lines 780-1293).

#![allow(dead_code, unused_variables, unused_imports)]

use std::collections::BTreeSet;
use std::rc::Rc;

use crate::diagnostics::{DiagRef, Diagnostic, Diagnostics};
use crate::profiles::Profile;
use crate::syntax::{
    Assignment, Call, Case, ConstDecl, Declaration, Expr, ExprRef, Index, Instruction, Juxtaposed,
    Member, Number, OrderedMap, Program, ProgramRef, Stmt, StmtRef, SubDef, Unparsed, VarDecl,
};

/// Port of `checker/structure.py::_scan_placement_walk`.
pub fn _scan_placement_walk(_program: &Program, _diags: &mut Diagnostics) {
    todo!("checker/structure.py::_scan_placement_walk (omitted: profile: untyped)")
}

/// Port of `checker/structure.py::_is_case_else`.
pub fn _is_case_else(_stmt: &Case) -> bool {
    todo!("checker/structure.py::_is_case_else")
}

/// Port of `checker/structure.py::stray_controls`.
pub fn stray_controls(_program: &Program) {
    todo!("checker/structure.py::stray_controls")
}

/// Port of `checker/structure.py::scan_only_outside`.
pub fn scan_only_outside(_program: &Program) -> OrderedMap<usize, String> {
    todo!("checker/structure.py::scan_only_outside")
}

/// Port of `checker/structure.py::_subscan_body_time`.
pub fn _subscan_body_time(_stmt: &Stmt) -> Option<bool> {
    todo!("checker/structure.py::_subscan_body_time (omitted: profile: untyped)")
}

/// Port of `checker/structure.py::subscan_controls`.
pub fn subscan_controls(_program: &Program) {
    todo!("checker/structure.py::subscan_controls (omitted: profile: untyped)")
}

/// Port of `checker/structure.py::check_stray_controls`.
pub fn check_stray_controls(_program: &Program, _diags: &mut Diagnostics) -> Option<(String, i64)> {
    todo!("checker/structure.py::check_stray_controls (omitted: profile: untyped)")
}

/// Port of `checker/structure.py::_has_error_on`.
pub fn _has_error_on(_stmt: &Stmt, _diags: &mut Diagnostics) -> bool {
    todo!("checker/structure.py::_has_error_on")
}

/// Port of `checker/structure.py::_before_beginprog_name`.
pub fn _before_beginprog_name(_stmt: &Stmt, _kw: &str, _subs: &BTreeSet<String>) -> Option<String> {
    todo!("checker/structure.py::_before_beginprog_name (omitted: profile: untyped)")
}

/// Port of `checker/structure.py::_quote_in_first_arg`.
pub fn _quote_in_first_arg(_text: &str) -> bool {
    todo!("checker/structure.py::_quote_in_first_arg")
}

/// Port of `checker/structure.py::_control_keyword`.
pub fn _control_keyword(_stmt: &Stmt) -> Option<String> {
    todo!("checker/structure.py::_control_keyword")
}

/// Port of `checker/structure.py::_table_names`.
pub fn _table_names(_program: &Program) -> BTreeSet<String> {
    todo!("checker/structure.py::_table_names")
}

/// Port of `checker/structure.py::_leftover_message`.
pub fn _leftover_message(_stmt: &Stmt) -> String {
    todo!("checker/structure.py::_leftover_message (omitted: profile: untyped)")
}

/// Port of `checker/structure.py::_constant_assignment`.
pub fn _constant_assignment(_text: &str) -> bool {
    todo!("checker/structure.py::_constant_assignment (omitted: profile: untyped)")
}

/// Port of `checker/structure.py::_no_matching`.
pub fn _no_matching(_text: &str, _at_eol: bool) -> String {
    todo!("checker/structure.py::_no_matching (omitted: profile: untyped)")
}

/// Port of `checker/structure.py::_out_of_place`.
pub fn _out_of_place(_text: &str, _at_eol: bool) -> String {
    todo!("checker/structure.py::_out_of_place (omitted: profile: untyped)")
}

/// Port of `checker/structure.py::_function_statement`.
pub fn _function_statement(_text: &str) -> bool {
    todo!("checker/structure.py::_function_statement (omitted: profile: untyped)")
}

/// Port of `checker/structure.py::_semicolon_assignment`.
pub fn _semicolon_assignment(_text: &str) -> bool {
    todo!("checker/structure.py::_semicolon_assignment")
}

/// Port of `checker/structure.py::_keyword`.
pub fn _keyword(_stmt: &Stmt) -> String {
    todo!("checker/structure.py::_keyword")
}

/// Port of `checker/structure.py::_block_word`.
pub fn _block_word() -> Option<String> {
    todo!("checker/structure.py::_block_word (omitted: args: list)")
}
