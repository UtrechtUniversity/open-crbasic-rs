//! Port of `checker/cr3xx.py` (lines 769-1327).

#![allow(dead_code, unused_variables, unused_imports)]

use std::collections::BTreeSet;
use std::rc::Rc;

use crate::diagnostics::{DiagRef, Diagnostic, Diagnostics};
use crate::profiles::Profile;
use crate::syntax::{
    Assignment, Call, Case, ConstDecl, Declaration, Expr, ExprRef, Index, Instruction, Juxtaposed,
    Member, Number, OrderedMap, Program, ProgramRef, Stmt, StmtRef, SubDef, Unparsed, VarDecl,
};

/// Port of `checker/cr3xx.py::_split_top_commas`.
pub fn _split_top_commas(_text: &str) -> Vec<String> {
    todo!("checker/cr3xx.py::_split_top_commas")
}

/// Port of `checker/cr3xx.py::_inside_table`.
pub fn _inside_table(_program: &Program, _line: i64, _file: &str) -> bool {
    todo!("checker/cr3xx.py::_inside_table")
}

/// Port of `checker/cr3xx.py::_loose_before_beginprog`.
pub fn _loose_before_beginprog(_program: &Program, _line: i64, _file: &str) -> bool {
    todo!("checker/cr3xx.py::_loose_before_beginprog")
}

/// Port of `checker/cr3xx.py::prose_known`.
pub fn prose_known(_program: &Program) -> BTreeSet<String> {
    todo!("checker/cr3xx.py::prose_known (omitted: profile: untyped)")
}

/// Port of `checker/cr3xx.py::prose_known_tables`.
pub fn prose_known_tables(_program: &Program) -> BTreeSet<String> {
    todo!("checker/cr3xx.py::prose_known_tables (omitted: profile: untyped)")
}

/// Port of `checker/cr3xx.py::first_declarations`.
pub fn first_declarations(_program: &Program) {
    todo!("checker/cr3xx.py::first_declarations")
}

/// Port of `checker/cr3xx.py::declared_later`.
pub fn declared_later(_line: i64, _file: &str) -> BTreeSet<String> {
    todo!("checker/cr3xx.py::declared_later (omitted: first: dict[str, tuple[str, int]])")
}

/// Port of `checker/cr3xx.py::_paren_items_words`.
pub fn _paren_items_words(_inner: &str, _known: &BTreeSet<String>) -> Vec<String> {
    todo!("checker/cr3xx.py::_paren_items_words")
}

/// Port of `checker/cr3xx.py::_leading_unknown_words`.
pub fn _leading_unknown_words(_text: &str, _known: &BTreeSet<String>) -> Vec<String> {
    todo!(
        "checker/cr3xx.py::_leading_unknown_words (omitted: callables: frozenset, state: dict | None)"
    )
}

/// Port of `checker/cr3xx.py::_split_args_quoted`.
pub fn _split_args_quoted(_text: &str) -> Option<Vec<String>> {
    todo!("checker/cr3xx.py::_split_args_quoted")
}

/// Port of `checker/cr3xx.py::_adjacent_operands`.
pub fn _adjacent_operands(_arg: &str) -> bool {
    todo!("checker/cr3xx.py::_adjacent_operands")
}

/// Port of `checker/cr3xx.py::_call_adjacent_arg_messages`.
pub fn _call_adjacent_arg_messages(
    _inner: &str,
    _here: &BTreeSet<String>,
    _variables: &BTreeSet<String>,
    _in_table: bool,
) -> Option<Vec<String>> {
    todo!("checker/cr3xx.py::_call_adjacent_arg_messages (omitted: record: dict, profile: untyped)")
}

/// Port of `checker/cr3xx.py::order_email_destination`.
pub fn order_email_destination(_program: &Program, _diags: &mut Diagnostics) {
    todo!("checker/cr3xx.py::order_email_destination (omitted: profile: untyped)")
}

/// Port of `checker/cr3xx.py::drop_shortfall_with_names`.
pub fn drop_shortfall_with_names(_program: &Program, _diags: &mut Diagnostics) {
    todo!("checker/cr3xx.py::drop_shortfall_with_names (omitted: profile: untyped)")
}

/// Port of `checker/cr3xx.py::check_for_header`.
pub fn check_for_header(_program: &Program, _diags: &mut Diagnostics) {
    todo!("checker/cr3xx.py::check_for_header (omitted: profile: untyped)")
}

/// Port of `checker/cr3xx.py::_echo_key`.
pub fn _echo_key(_text: &str) -> String {
    todo!("checker/cr3xx.py::_echo_key")
}
