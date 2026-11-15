//! Port of `checker/structure.py` (lines 2082-2868).

#![allow(dead_code, unused_variables, unused_imports)]

use std::collections::BTreeSet;
use std::rc::Rc;
use std::sync::LazyLock;

use crate::diagnostics::{DiagRef, Diagnostic, Diagnostics};
use crate::lexer;
use crate::profiles::Profile;
use crate::pyre::{self, Regex};
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
pub fn _unparsed_opener(text: &str) -> Option<String> {
    static IF: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s*IF\b", pyre::I));
    static THEN_BODY: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\bTHEN\s*\S", pyre::I));
    static SELECT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s*SELECT\s+CASE\b", pyre::I));
    static DO: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"\s*(?:DO\s+(?:WHILE|UNTIL)|WHILE)\b", pyre::I));

    if IF.match_(text).is_some() && THEN_BODY.search(text).is_none() {
        Some("IF".to_owned())
    } else if SELECT.match_(text).is_some() {
        Some("SELECT".to_owned())
    } else {
        DO.match_(text).map(|_| "DO".to_owned())
    }
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
pub fn _leftover_first_name_known(text: &str, known: &BTreeSet<String>) -> bool {
    let Ok(tokens) = lexer::tokenize(text, 0) else {
        return false;
    };
    tokens
        .iter()
        .find(|token| token.kind == "IDENT")
        .is_none_or(|token| known.contains(&token.upper))
}
