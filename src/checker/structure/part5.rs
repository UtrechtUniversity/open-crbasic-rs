//! Port of `checker/structure.py` (lines 2869-3659).

#![allow(dead_code, unused_variables, unused_imports)]

use std::collections::BTreeSet;
use std::rc::Rc;

use crate::diagnostics::{DiagRef, Diagnostic, Diagnostics};
use crate::profiles::Profile;
use crate::syntax::{
    Assignment, Call, Case, ConstDecl, Declaration, Expr, ExprRef, Index, Instruction, Juxtaposed,
    Member, Number, OrderedMap, Program, ProgramRef, Stmt, StmtRef, SubDef, Unparsed, VarDecl,
};

/// Port of `checker/structure.py::_unclosed_paren_kind`.
pub fn _unclosed_paren_kind(_text: &str, _known: &BTreeSet<String>) -> String {
    todo!("checker/structure.py::_unclosed_paren_kind")
}

/// Port of `checker/structure.py::is_hash_statement`.
pub fn is_hash_statement(_stmt: &Unparsed) -> bool {
    todo!("checker/structure.py::is_hash_statement")
}

/// Port of `checker/structure.py::_is_generic_unparsed`.
pub fn _is_generic_unparsed(_stmt: &Unparsed) -> bool {
    todo!("checker/structure.py::_is_generic_unparsed")
}

/// Port of `checker/structure.py::pass1_operands`.
pub fn pass1_operands(_program: &Program) -> BTreeSet<String> {
    todo!("checker/structure.py::pass1_operands (omitted: profile: untyped)")
}

/// Port of `checker/structure.py::inline_if_condition`.
pub fn inline_if_condition(_text: &str) -> Option<String> {
    todo!("checker/structure.py::inline_if_condition")
}

/// Port of `checker/structure.py::_inline_condition_stmt`.
pub fn _inline_condition_stmt(_stmt: &Stmt) -> StmtRef {
    todo!("checker/structure.py::_inline_condition_stmt (omitted: profile: untyped)")
}

/// Port of `checker/structure.py::condition_part`.
pub fn condition_part(_text: &str) -> Option<String> {
    todo!("checker/structure.py::condition_part")
}

/// Port of `checker/structure.py::_invalid_assignment`.
pub fn _invalid_assignment() -> bool {
    todo!("checker/structure.py::_invalid_assignment (omitted: toks: untyped)")
}

/// Port of `checker/structure.py::_semicolons_dropped`.
pub fn _semicolons_dropped(_text: &str) -> bool {
    todo!("checker/structure.py::_semicolons_dropped (omitted: profile: untyped)")
}

/// Port of `checker/structure.py::_pass1_scan`.
pub fn _pass1_scan(_text: &str, _operands: &BTreeSet<String>) -> Option<(String, String)> {
    todo!("checker/structure.py::_pass1_scan (omitted: profile: untyped)")
}

/// Port of `checker/structure.py::_assigns_user_const`.
pub fn _assigns_user_const(_stmt: &Assignment, _program: &Program) -> bool {
    todo!("checker/structure.py::_assigns_user_const")
}

/// Port of `checker/structure.py::_signed_target`.
pub fn _signed_target() -> Option<String> {
    todo!(
        "checker/structure.py::_signed_target (omitted: toks: untyped, operands: untyped, functions: untyped, profile: untyped)"
    )
}

/// Port of `checker/structure.py::_logical_bad_target`.
pub fn _logical_bad_target() -> bool {
    todo!(
        "checker/structure.py::_logical_bad_target (omitted: toks: untyped, operands: untyped, functions: untyped)"
    )
}

/// Port of `checker/structure.py::_logical_assign_eq`.
pub fn _logical_assign_eq(_i: i64) -> Option<i64> {
    todo!("checker/structure.py::_logical_assign_eq (omitted: toks: untyped)")
}

/// Port of `checker/structure.py::_known_first_token`.
pub fn _known_first_token(_text: &str, _operands: &BTreeSet<String>) -> bool {
    todo!("checker/structure.py::_known_first_token")
}

/// Port of `checker/structure.py::_logical_led_string`.
pub fn _logical_led_string(_text: &str) -> bool {
    todo!("checker/structure.py::_logical_led_string")
}

/// Port of `checker/structure.py::_logical_assign_state`.
pub fn _logical_assign_state(_stmt: &Unparsed, _program: &Program) {
    todo!("checker/structure.py::_logical_assign_state (omitted: profile: untyped)")
}

/// Port of `checker/structure.py::_logical_assign_accepted`.
pub fn _logical_assign_accepted(_stmt: &Unparsed, _program: &Program) -> bool {
    todo!("checker/structure.py::_logical_assign_accepted (omitted: profile: untyped)")
}

/// Port of `checker/structure.py::_logical_assign_undefined_messages`.
pub fn _logical_assign_undefined_messages(
    _stmt: &Unparsed,
    _program: &Program,
) -> Option<Vec<String>> {
    todo!("checker/structure.py::_logical_assign_undefined_messages (omitted: profile: untyped)")
}

/// Port of `checker/structure.py::_unparsed_string_messages`.
pub fn _unparsed_string_messages(_stmt: &Unparsed, _program: &Program) -> Vec<String> {
    todo!("checker/structure.py::_unparsed_string_messages (omitted: profile: untyped)")
}

/// Port of `checker/structure.py::string_reached`.
pub fn string_reached(_text: &str, _operands: &BTreeSet<String>) -> Option<String> {
    todo!("checker/structure.py::string_reached (omitted: profile: untyped)")
}

/// Port of `checker/structure.py::_Everything`.
#[derive(Debug, Default)]
pub struct Everything {}

impl Everything {
    /// Port of `checker/structure.py::_Everything::__contains__`.
    pub fn __contains__(&self) -> bool {
        todo!("checker/structure.py::_Everything::__contains__ (omitted: item: untyped)")
    }
}
