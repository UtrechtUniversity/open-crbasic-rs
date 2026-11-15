//! Port of `checker/cr3xx.py` (lines 1328-2123).

#![allow(dead_code, unused_variables, unused_imports)]

use std::collections::BTreeSet;
use std::rc::Rc;

use crate::diagnostics::{DiagRef, Diagnostic, Diagnostics};
use crate::profiles::Profile;
use crate::syntax::{
    Assignment, Call, Case, ConstDecl, Declaration, Expr, ExprRef, Index, Instruction, Juxtaposed,
    Member, Number, OrderedMap, Program, ProgramRef, Stmt, StmtRef, SubDef, Unparsed, VarDecl,
};

/// Port of `checker/cr3xx.py::check_unknown_line_words`.
pub fn check_unknown_line_words(_program: &Program, _diags: &mut Diagnostics) {
    todo!("checker/cr3xx.py::check_unknown_line_words (omitted: profile: untyped)")
}

/// Port of `checker/cr3xx.py::_ArityState`.
#[derive(Debug, Default)]
pub struct ArityState {}

/// Port of `checker/cr3xx.py::_arity_walk`.
pub fn _arity_walk(_expr: Option<&Expr>, _known: &BTreeSet<String>, _state: &mut ArityState) {
    todo!("checker/cr3xx.py::_arity_walk (omitted: profile: untyped)")
}

/// Port of `checker/cr3xx.py::order_names_around_arity`.
pub fn order_names_around_arity(_program: &Program, _diags: &mut Diagnostics) {
    todo!("checker/cr3xx.py::order_names_around_arity (omitted: profile: untyped)")
}

/// Port of `checker/cr3xx.py::order_names_before_call_arity`.
pub fn order_names_before_call_arity(_program: &Program, _diags: &mut Diagnostics) {
    todo!("checker/cr3xx.py::order_names_before_call_arity (omitted: profile: untyped)")
}

/// Port of `checker/cr3xx.py::_hides_nothing`.
pub fn _hides_nothing() -> bool {
    todo!("checker/cr3xx.py::_hides_nothing (omitted: d: untyped)")
}

/// Port of `checker/cr3xx.py::drop_block_closer_errors_after_errors`.
pub fn drop_block_closer_errors_after_errors(_program: &Program, _diags: &mut Diagnostics) {
    todo!("checker/cr3xx.py::drop_block_closer_errors_after_errors (omitted: profile: untyped)")
}

/// Port of `checker/cr3xx.py::drop_calltable_error_with_other_errors`.
pub fn drop_calltable_error_with_other_errors(_program: &Program, _diags: &mut Diagnostics) {
    todo!("checker/cr3xx.py::drop_calltable_error_with_other_errors (omitted: profile: untyped)")
}

/// Port of `checker/cr3xx.py::order_block_keyword_calls`.
pub fn order_block_keyword_calls(_program: &Program, _diags: &mut Diagnostics) {
    todo!("checker/cr3xx.py::order_block_keyword_calls (omitted: profile: untyped)")
}

/// Port of `checker/cr3xx.py::fix_then_operand`.
pub fn fix_then_operand(_program: &Program, _diags: &mut Diagnostics) {
    todo!("checker/cr3xx.py::fix_then_operand (omitted: profile: untyped)")
}

/// Port of `checker/cr3xx.py::order_invalid_assignments`.
pub fn order_invalid_assignments(_program: &Program, _diags: &mut Diagnostics) {
    todo!("checker/cr3xx.py::order_invalid_assignments (omitted: profile: untyped)")
}

/// Port of `checker/cr3xx.py::_string_reached`.
pub fn _string_reached(_text: &str) -> bool {
    todo!("checker/cr3xx.py::_string_reached")
}
