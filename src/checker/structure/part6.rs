//! Port of `checker/structure.py` (lines 3660-4451).

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

/// Port of `checker/structure.py::_function_call_scan`.
pub fn _function_call_scan(_stmt: &Unparsed, _program: &Program) {
    todo!("checker/structure.py::_function_call_scan (omitted: profile: untyped)")
}

/// Port of `checker/structure.py::_function_symbol_finding`.
pub fn _function_symbol_finding(_stmt: &Unparsed, _program: &Program) -> Option<String> {
    todo!("checker/structure.py::_function_symbol_finding (omitted: profile: untyped)")
}

/// Port of `checker/structure.py::_function_syntax_findings`.
pub fn _function_syntax_findings(_stmt: &Unparsed, _program: &Program) -> Vec<String> {
    todo!("checker/structure.py::_function_syntax_findings (omitted: profile: untyped)")
}

/// Port of `checker/structure.py::_routine_arg_findings`.
pub fn _routine_arg_findings(_stmt: &Unparsed, _program: &Program) -> Vec<String> {
    todo!("checker/structure.py::_routine_arg_findings (omitted: profile: untyped)")
}

/// Port of `checker/structure.py::_instruction_arg_findings`.
pub fn _instruction_arg_findings(_text: &str, _operands: &BTreeSet<String>) {
    todo!(
        "checker/structure.py::_instruction_arg_findings (omitted: profile: untyped, variables: untyped)"
    )
}

/// Port of `checker/structure.py::_symbol_stage`.
pub fn _symbol_stage(_code: Option<i64>) -> i64 {
    todo!("checker/structure.py::_symbol_stage")
}

/// Port of `checker/structure.py::_later_arg_unknown`.
pub fn _later_arg_unknown(_operands: &BTreeSet<String>) -> bool {
    todo!("checker/structure.py::_later_arg_unknown (omitted: args: untyped, functions: dict)")
}

/// Port of `checker/structure.py::_scan_expr`.
pub fn _scan_expr(
    _i: i64,
    _operands: &BTreeSet<String>,
    _assign: bool,
    _start_operand: bool,
    _unclosed: Option<&str>,
    _cond: bool,
    _to_ok: bool,
    _open_fallback: bool,
    _symbol_unknown: bool,
) -> Option<(String, String)> {
    todo!("checker/structure.py::_scan_expr (omitted: toks: untyped, functions: dict)")
}

/// Port of `checker/structure.py::_bare_function_needs_paren`.
pub fn _bare_function_needs_paren(_name: &str) -> bool {
    todo!("checker/structure.py::_bare_function_needs_paren (omitted: functions: dict)")
}

/// Port of `checker/structure.py::_mentions_then`.
pub fn _mentions_then() -> bool {
    todo!("checker/structure.py::_mentions_then (omitted: node: untyped)")
}

/// Port of `checker/structure.py::_check_if_pass1`.
pub fn _check_if_pass1(_operands: &BTreeSet<String>, _diags: &mut Diagnostics) {
    todo!(
        "checker/structure.py::_check_if_pass1 (omitted: stmt: untyped, profile: untyped, program: untyped)"
    )
}

/// Port of `checker/structure.py::_for_header_incomplete`.
pub fn _for_header_incomplete(_text: &str) -> Option<(String, bool)> {
    todo!("checker/structure.py::_for_header_incomplete (omitted: profile: untyped)")
}

/// Port of `checker/structure.py::_merge_members`.
pub fn _merge_members() {
    todo!("checker/structure.py::_merge_members (omitted: toks: list)")
}

/// Port of `checker/structure.py::_for_header_parts`.
pub fn _for_header_parts(_text: &str) {
    todo!("checker/structure.py::_for_header_parts (omitted: profile: untyped)")
}

/// Port of `checker/structure.py::_for_header_findings`.
pub fn _for_header_findings(_text: &str) -> Vec<String> {
    todo!("checker/structure.py::_for_header_findings (omitted: profile: untyped)")
}

/// Port of `checker/structure.py::_for_header_message`.
pub fn _for_header_message(_kind: &str, _part: &str) -> String {
    todo!("checker/structure.py::_for_header_message")
}

/// Port of `checker/structure.py::_block_keyword_call`.
pub fn _block_keyword_call(_stmt: &Stmt) -> bool {
    todo!("checker/structure.py::_block_keyword_call")
}

/// Port of `checker/structure.py::_unclosed_call_message`.
pub fn _unclosed_call_message(
    _text: &str,
    _at_eol: bool,
    _operands: &BTreeSet<String>,
) -> Option<String> {
    todo!("checker/structure.py::_unclosed_call_message (omitted: profile: untyped)")
}

/// Port of `checker/structure.py::_assigns_const_index`.
pub fn _assigns_const_index(
    _stmt: &Stmt,
    _consts: &BTreeSet<String>,
    _predefined: &BTreeSet<String>,
) -> bool {
    todo!("checker/structure.py::_assigns_const_index")
}

/// Port of `checker/structure.py::_pass1_message`.
pub fn _pass1_message(_stmt: &Stmt, _kind: &str, _operands: Option<BTreeSet<String>>) -> String {
    todo!("checker/structure.py::_pass1_message (omitted: profile: untyped)")
}
