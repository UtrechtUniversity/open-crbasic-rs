//! Port of `checker/structure.py` (lines 2082-2868).

#![allow(dead_code, unused_variables, unused_imports)]

use std::collections::BTreeSet;
use std::rc::Rc;

use crate::diagnostics::{DiagRef, Diagnostic, Diagnostics};
use crate::profiles::Profile;
use crate::syntax::{
    Assignment, Call, Case, ConstDecl, Declaration, Expr, ExprRef, Index, Instruction, Juxtaposed,
    Member, Number, OrderedMap, Program, ProgramRef, Stmt, StmtRef, SubDef, Unparsed, VarDecl,
};

/// Port of `checker/structure.py::check_end_blocks`.
pub fn check_end_blocks(_program: &Program, _diags: &mut Diagnostics) {
    todo!("checker/structure.py::check_end_blocks")
}

/// Port of `checker/structure.py::CallLinks`.
#[derive(Debug, Default)]
pub struct CallLinks {}

/// Port of `checker/structure.py::nested_table`.
pub fn nested_table(_program: &Program) -> Option<StmtRef> {
    todo!("checker/structure.py::nested_table (omitted: profile: untyped)")
}

/// Port of `checker/structure.py::check_pass2`.
pub fn check_pass2(_program: &Program, _diags: &mut Diagnostics, _links: Option<&CallLinks>) {
    todo!(
        "checker/structure.py::check_pass2 (omitted: profile: untyped, env: consteval.Env | None)"
    )
}

/// Port of `checker/structure.py::_names_unresolved`.
pub fn _names_unresolved(_known: &BTreeSet<String>, _stmt: &Instruction, _sources: bool) -> bool {
    todo!("checker/structure.py::_names_unresolved")
}

/// Port of `checker/structure.py::_unparsed_opener`.
pub fn _unparsed_opener(_text: &str) -> Option<String> {
    todo!("checker/structure.py::_unparsed_opener")
}

/// Port of `checker/structure.py::check_block_closers`.
pub fn check_block_closers(_program: &Program, _diags: &mut Diagnostics) {
    todo!("checker/structure.py::check_block_closers (omitted: profile: untyped)")
}

/// Port of `checker/structure.py::_undefined_table_source`.
pub fn _undefined_table_source(_stmt: &Instruction, _known: &BTreeSet<String>) -> Option<String> {
    todo!("checker/structure.py::_undefined_table_source (omitted: profile: untyped)")
}

/// Port of `checker/structure.py::_leftover_first_name_known`.
pub fn _leftover_first_name_known(_text: &str, _known: &BTreeSet<String>) -> bool {
    todo!("checker/structure.py::_leftover_first_name_known")
}
