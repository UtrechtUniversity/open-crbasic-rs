//! Port of `checker/structure.py` (lines 4452-5251).

#![allow(dead_code, unused_variables, unused_imports)]

use std::collections::BTreeSet;
use std::rc::Rc;

use super::CallLinks;
use crate::diagnostics::{DiagRef, Diagnostic, Diagnostics};
use crate::profiles::Profile;
use crate::syntax::{
    Assignment, Call, Case, ConstDecl, Declaration, Expr, ExprRef, Index, Instruction, Juxtaposed,
    Member, Number, OrderedMap, Program, ProgramRef, Stmt, StmtRef, SubDef, Unparsed, VarDecl,
};

/// Port of `checker/structure.py::check_missing_param_followup`.
pub fn check_missing_param_followup(_program: &Program, _diags: &mut Diagnostics) {
    todo!("checker/structure.py::check_missing_param_followup (omitted: profile: untyped)")
}

/// Port of `checker/structure.py::_optional_header_error`.
pub fn _optional_header_error(_message: &str) -> bool {
    todo!("checker/structure.py::_optional_header_error")
}

/// Port of `checker/structure.py::_check_call_args`.
pub fn _check_call_args(_stmt: &Call, _sub: &SubDef, _diags: &mut Diagnostics) {
    todo!("checker/structure.py::_check_call_args")
}

/// Port of `checker/structure.py::_report_call_arg_names`.
pub fn _report_call_arg_names(
    _stmt: &Call,
    _known: &BTreeSet<String>,
    _diags: &mut Diagnostics,
    _is_function: bool,
) {
    todo!("checker/structure.py::_report_call_arg_names (omitted: profile: untyped, ds: untyped)")
}

/// Port of `checker/structure.py::_LinkContext`.
#[derive(Debug, Default)]
pub struct LinkContext {}

/// Port of `checker/structure.py::_call_link_name`.
pub fn _call_link_name(_text: &str) -> String {
    todo!("checker/structure.py::_call_link_name")
}

/// Port of `checker/structure.py::_bare_sub_link_name`.
pub fn _bare_sub_link_name(_text: &str) -> Option<String> {
    todo!("checker/structure.py::_bare_sub_link_name (omitted: subs: set)")
}

/// Port of `checker/structure.py::_unlinked_call`.
pub fn _unlinked_call(
    _link: &str,
    _diags: &mut Diagnostics,
    _before_beginprog: bool,
) -> Option<i64> {
    todo!(
        "checker/structure.py::_unlinked_call (omitted: stmt: untyped, profile: untyped, ctx: _LinkContext)"
    )
}

/// Port of `checker/structure.py::_link_step_runs`.
pub fn _link_step_runs(_diags: &mut Diagnostics, _pass1_errors: bool) -> bool {
    todo!("checker/structure.py::_link_step_runs (omitted: profile: untyped)")
}

/// Port of `checker/structure.py::commit_deferred_sub_error`.
pub fn commit_deferred_sub_error(_links: CallLinks, _diags: &mut Diagnostics) {
    todo!("checker/structure.py::commit_deferred_sub_error (omitted: profile: untyped)")
}

/// Port of `checker/structure.py::finalize_call_links`.
pub fn finalize_call_links(_links: CallLinks, _diags: &mut Diagnostics, _pass1_errors: bool) {
    todo!("checker/structure.py::finalize_call_links (omitted: profile: untyped)")
}

/// Port of `checker/structure.py::_check_call_pass2`.
pub fn _check_call_pass2(
    _stmt: &Call,
    _subs: &BTreeSet<String>,
    _diags: &mut Diagnostics,
    _before_bp: bool,
    _in_body_block: bool,
    _known: Option<BTreeSet<String>>,
    _link_ctx: Option<&mut LinkContext>,
) -> Option<i64> {
    todo!(
        "checker/structure.py::_check_call_pass2 (omitted: profile: untyped, sub_params: dict | None, ds: untyped)"
    )
}

/// Port of `checker/structure.py::_over_indexed_target`.
pub fn _over_indexed_target(_target: &Index) -> bool {
    todo!("checker/structure.py::_over_indexed_target (omitted: ds: untyped)")
}

/// Port of `checker/structure.py::_check_assignment_pass2`.
pub fn _check_assignment_pass2(
    _stmt: &Assignment,
    _known: &BTreeSet<String>,
    _diags: &mut Diagnostics,
    _before_bp: bool,
    _in_body_block: bool,
) -> Option<i64> {
    todo!("checker/structure.py::_check_assignment_pass2 (omitted: profile: untyped, ds: untyped)")
}

/// Port of `checker/structure.py::_check_declaration`.
pub fn _check_declaration(_stmt: &Declaration, _diags: &mut Diagnostics) -> i64 {
    todo!("checker/structure.py::_check_declaration")
}

/// Port of `checker/structure.py::_first_dim_text`.
pub fn _first_dim_text(_stmt_text: &str, _v: &VarDecl) -> String {
    todo!("checker/structure.py::_first_dim_text")
}

/// Port of `checker/structure.py::_report_numeric_sub_name`.
pub fn _report_numeric_sub_name(_stmt: &SubDef, _diags: &mut Diagnostics) {
    todo!("checker/structure.py::_report_numeric_sub_name (omitted: value: object)")
}

/// Port of `checker/structure.py::_bad_const_type`.
pub fn _bad_const_type(_stmt: &ConstDecl) -> Option<String> {
    todo!("checker/structure.py::_bad_const_type")
}

/// Port of `checker/structure.py::_local_const_conflicts`.
pub fn _local_const_conflicts(_stmt: &Stmt, _declared: &OrderedMap<String, String>) -> Vec<String> {
    todo!("checker/structure.py::_local_const_conflicts")
}

/// Port of `checker/structure.py::_check_duplicate_declaration`.
pub fn _check_duplicate_declaration(
    _stmt: &Stmt,
    _declared: &OrderedMap<String, String>,
    _diags: &mut Diagnostics,
) -> Option<i64> {
    todo!("checker/structure.py::_check_duplicate_declaration (omitted: profile: untyped)")
}

/// Port of `checker/structure.py::_illegal_dim_message`.
pub fn _illegal_dim_message(_stmt: &Stmt) -> String {
    todo!("checker/structure.py::_illegal_dim_message (omitted: v: untyped)")
}

/// Port of `checker/structure.py::_check_array_dims`.
pub fn _check_array_dims(_stmt: &Declaration, _diags: &mut Diagnostics) -> Option<i64> {
    todo!("checker/structure.py::_check_array_dims (omitted: env: consteval.Env, profile: untyped)")
}

/// Port of `checker/structure.py::_check_var_dims`.
pub fn _check_var_dims(_stmt: &Declaration, _diags: &mut Diagnostics) {
    todo!("checker/structure.py::_check_var_dims (omitted: v: untyped, env: consteval.Env)")
}

/// Port of `checker/structure.py::_split_top`.
pub fn _split_top(_text: &str, _sep: &str) -> Vec<String> {
    todo!("checker/structure.py::_split_top")
}
