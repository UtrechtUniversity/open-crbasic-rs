//! Port of `checker/expressions.py` (lines 798-1570).

#![allow(dead_code, unused_variables, unused_imports)]

use std::collections::BTreeSet;
use std::rc::Rc;
use std::sync::LazyLock;

use crate::diagnostics::{DiagRef, Diagnostic, Diagnostics};
use crate::profiles::Profile;
use crate::syntax::{
    Assignment, Call, Case, ConstDecl, Declaration, Expr, ExprRef, Index, Instruction, Juxtaposed,
    Member, Number, OrderedMap, Program, ProgramRef, Stmt, StmtRef, SubDef, Unparsed, VarDecl,
};
use crate::{pyre, pystr};

/// Port of `checker/expressions.py::_names_through_over_index`.
pub fn _names_through_over_index(_expr: Option<&Expr>, _known: &BTreeSet<String>) -> Vec<String> {
    todo!("checker/expressions.py::_names_through_over_index (omitted: dim_counts: dict)")
}

/// Port of `checker/expressions.py::_dim_limit`.
pub fn _dim_limit() -> Option<i64> {
    todo!("checker/expressions.py::_dim_limit (omitted: profile: untyped)")
}

/// Port of `checker/expressions.py::_over_indexed`.
pub fn _over_indexed(
    _expr: Option<&Expr>,
    _dim_counts: &OrderedMap<String, i64>,
    _limit: Option<i64>,
) -> bool {
    todo!("checker/expressions.py::_over_indexed")
}

/// Port of `checker/expressions.py::DimsState`.
#[derive(Debug, Default)]
pub struct DimsState {}

impl DimsState {
    /// Port of `checker/expressions.py::DimsState::__init__`.
    pub fn __init__(&self) {
        todo!(
            "checker/expressions.py::DimsState::__init__ (omitted: var_dims: dict, before: dict | None, dim_counts: dict | None)"
        )
    }
}

/// Port of `checker/expressions.py::_stmt_exprs`.
pub fn _stmt_exprs(_stmt: &Stmt) {
    todo!("checker/expressions.py::_stmt_exprs")
}

/// Port of `checker/expressions.py::_build_dims_state`.
pub fn _build_dims_state(_ctx: &crate::checker::context::Context) -> DimsState {
    todo!("checker/expressions.py::_build_dims_state")
}

/// Port of `checker/expressions.py::_dims_walk`.
pub fn _dims_walk(_expr: Option<&Expr>) -> bool {
    todo!(
        "checker/expressions.py::_dims_walk (omitted: known: untyped, var_dims: dict, state: list)"
    )
}

/// Port of `checker/expressions.py::_has_undefined_index`.
pub fn _has_undefined_index(
    _expr: Option<&Expr>,
    _known: &BTreeSet<String>,
    _ds: Option<&mut DimsState>,
    _stmt: Option<&Stmt>,
) -> bool {
    todo!("checker/expressions.py::_has_undefined_index (omitted: prior: tuple)")
}

/// Port of `checker/expressions.py::_names_and_dims`.
pub fn _names_and_dims(
    _known: &BTreeSet<String>,
    _ds: Option<&mut DimsState>,
    _stmt: Option<&Stmt>,
) {
    todo!("checker/expressions.py::_names_and_dims (omitted: prior: tuple)")
}

/// Port of `checker/expressions.py::dimensions_message`.
pub fn dimensions_message(_text: &str, _whole_statement: bool, _at_eol: bool) -> String {
    todo!("checker/expressions.py::dimensions_message (omitted: profile: untyped)")
}

/// Port of `checker/expressions.py::_arg_text`.
pub fn _arg_text(stmt: &Instruction, i: i64) -> String {
    static SELECT_CASE: LazyLock<pyre::Regex> =
        LazyLock::new(|| pyre::Regex::new(r"(?i)\s*Select\s+Case(.*)$", 0));

    let text = &stmt.base.text;
    if i == 0
        && stmt.keyword.eq_ignore_ascii_case("SELECT")
        && let Some(tail) = SELECT_CASE.match_(text).and_then(|m| m.group(1))
    {
        return tail.to_owned();
    }
    let index = |len: usize| {
        if i < 0 {
            len.checked_sub(usize::try_from(i.unsigned_abs()).ok()?)
        } else {
            usize::try_from(i).ok()
        }
    };
    if let Some((start, end)) = index(stmt.arg_spans.len())
        .and_then(|i| stmt.arg_spans.get(i))
        .and_then(|span| *span)
    {
        return pystr::slice(text, Some(start), Some(end)).to_owned();
    }
    match index(stmt.args.len())
        .and_then(|i| stmt.args.get(i))
        .and_then(Option::as_deref)
    {
        Some(Expr::Name(name)) => name.name.clone(),
        _ => String::new(),
    }
}

/// Port of `checker/expressions.py::_call_arg_text`.
pub fn _call_arg_text(_text: &str, _index: i64) -> String {
    todo!("checker/expressions.py::_call_arg_text")
}

/// Port of `checker/expressions.py::_if_condition_text`.
pub fn _if_condition_text(_stmt: &Stmt) -> Option<String> {
    todo!("checker/expressions.py::_if_condition_text")
}

/// Port of `checker/expressions.py::_if_condition_text_no_then`.
pub fn _if_condition_text_no_then(_stmt: &Stmt) -> Option<String> {
    todo!("checker/expressions.py::_if_condition_text_no_then")
}

/// Port of `checker/expressions.py::_build_known_names`.
pub fn _build_known_names(_ctx: &crate::checker::context::Context) -> KnownNames {
    todo!("checker/expressions.py::_build_known_names")
}

/// Port of `checker/expressions.py::_unclosed_structure_variables`.
pub fn _unclosed_structure_variables(_program: &Program) -> BTreeSet<String> {
    todo!("checker/expressions.py::_unclosed_structure_variables")
}

/// Port of `checker/expressions.py::KnownNames`.
#[derive(Debug, Default)]
pub struct KnownNames {}

impl KnownNames {
    /// Port of `checker/expressions.py::KnownNames::__init__`.
    pub fn __init__(&self, _dotted_tables: bool) {
        todo!(
            "checker/expressions.py::KnownNames::__init__ (omitted: names: untyped, tables: frozenset, out_of_scope: frozenset)"
        )
    }

    /// Port of `checker/expressions.py::KnownNames::without`.
    #[must_use]
    pub fn without(&self) -> KnownNames {
        todo!("checker/expressions.py::KnownNames::without (omitted: names: untyped)")
    }
}

/// Port of `checker/expressions.py::_alias_names`.
pub fn _alias_names(_program: &Program) -> BTreeSet<String> {
    todo!("checker/expressions.py::_alias_names")
}

/// Port of `checker/expressions.py::_value_less_tables`.
pub fn _value_less_tables(_ctx: &crate::checker::context::Context) {
    todo!("checker/expressions.py::_value_less_tables")
}

/// Port of `checker/expressions.py::_failed_consts`.
pub fn _failed_consts(
    _ctx: &crate::checker::context::Context,
    _known: &BTreeSet<String>,
) -> BTreeSet<String> {
    todo!("checker/expressions.py::_failed_consts")
}

/// Port of `checker/expressions.py::_report_statement`.
pub fn _report_statement(
    _stmt: &Instruction,
    _positions: &OrderedMap<usize, String>,
    _known: &BTreeSet<String>,
    _variable_names: &BTreeSet<String>,
    _diags: &mut Diagnostics,
    _families: &str,
    _dim_counts: Option<OrderedMap<String, i64>>,
    _ds: Option<&mut DimsState>,
) -> bool {
    todo!("checker/expressions.py::_report_statement (omitted: profile: untyped)")
}

/// Port of `checker/expressions.py::_member_index_text`.
pub fn _member_index_text() -> String {
    todo!("checker/expressions.py::_member_index_text (omitted: args: list | None)")
}

/// Port of `checker/expressions.py::_table_member_problem`.
pub fn _table_member_problem(
    _expr: Option<&Expr>,
    _data_tables: &BTreeSet<String>,
    _absent: &BTreeSet<String>,
) {
    todo!(
        "checker/expressions.py::_table_member_problem (omitted: const_tables: dict[str, frozenset[str]] | dict[str, set[str]])"
    )
}

/// Port of `checker/expressions.py::_TablesWithFields`.
#[derive(Debug, Default)]
pub struct TablesWithFields {}

/// Port of `checker/expressions.py::_data_table_fields`.
pub fn _data_table_fields(_program: &Program) {
    todo!("checker/expressions.py::_data_table_fields (omitted: profile: untyped)")
}

#[cfg(test)]
mod tests {
    use super::_arg_text;
    use crate::syntax::{Instruction, StmtBase};

    #[test]
    fn arg_text_uses_character_spans_and_preserves_select_spacing() {
        let mut stmt = Instruction {
            base: StmtBase {
                text: "Scan(é + 1,Sec)".to_owned(),
                ..StmtBase::default()
            },
            keyword: "SCAN".to_owned(),
            arg_spans: vec![Some((5, 10)), Some((11, 14))],
            ..Instruction::default()
        };
        assert_eq!(_arg_text(&stmt, 0), "é + 1");
        assert_eq!(_arg_text(&stmt, -1), "Sec");

        stmt.keyword = "SELECT".to_owned();
        stmt.base.text = " Select Case  X + Nope".to_owned();
        assert_eq!(_arg_text(&stmt, 0), "  X + Nope");
    }
}
