//! Port of `checker/structure.py` (lines 5252-5848).

#![allow(dead_code, unused_variables, unused_imports)]

use std::collections::BTreeSet;
use std::rc::Rc;

use crate::diagnostics::{DiagRef, Diagnostic, Diagnostics};
use crate::profiles::Profile;
use crate::syntax::{
    Assignment, Call, Case, ConstDecl, Declaration, Expr, ExprRef, Index, Instruction, Juxtaposed,
    Member, Number, OrderedMap, Program, ProgramRef, Stmt, StmtRef, SubDef, Unparsed, VarDecl,
};

/// Port of `checker/structure.py::_group_end`.
pub fn _group_end(_text: &str, _open_at: i64) -> Option<i64> {
    todo!("checker/structure.py::_group_end")
}

/// Port of `checker/structure.py::_group_items`.
pub fn _group_items(_text: &str, _open_at: i64) -> Option<Vec<String>> {
    todo!("checker/structure.py::_group_items")
}

/// Port of `checker/structure.py::_report_decl_expr`.
pub fn _report_decl_expr(
    _echo: &str,
    _known: &BTreeSet<String>,
    _variables: &BTreeSet<String>,
    _stmt: &Stmt,
    _diags: &mut Diagnostics,
    _allow_variable: bool,
    _indexed_constants: &BTreeSet<String>,
) -> bool {
    todo!(
        "checker/structure.py::_report_decl_expr (omitted: expr: untyped, profile: untyped, tag: object)"
    )
}

/// Port of `checker/structure.py::_report_juxtaposed_dim`.
pub fn _report_juxtaposed_dim(
    _expr: &Juxtaposed,
    _echo: &str,
    _known: &BTreeSet<String>,
    _variables: &BTreeSet<String>,
    _stmt: &Stmt,
    _diags: &mut Diagnostics,
) -> bool {
    todo!("checker/structure.py::_report_juxtaposed_dim (omitted: profile: untyped)")
}

/// Port of `checker/structure.py::_check_member_types`.
pub fn _check_member_types(
    _stmt: &Declaration,
    _structs_seen: &BTreeSet<String>,
    _diags: &mut Diagnostics,
) {
    todo!("checker/structure.py::_check_member_types (omitted: profile: untyped)")
}

/// Port of `checker/structure.py::_check_header_types`.
pub fn _check_header_types(
    _text: &str,
    _stmt: &SubDef,
    _structs_seen: &BTreeSet<String>,
    _diags: &mut Diagnostics,
) -> bool {
    todo!("checker/structure.py::_check_header_types (omitted: profile: untyped)")
}

/// Port of `checker/structure.py::_check_init_bounds`.
pub fn _check_init_bounds(_stmt: &Declaration, _diags: &mut Diagnostics) {
    todo!("checker/structure.py::_check_init_bounds (omitted: env: consteval.Env)")
}

/// Port of `checker/structure.py::_adjacent_literals`.
pub fn _adjacent_literals(_text: &str) -> bool {
    todo!("checker/structure.py::_adjacent_literals")
}

/// Port of `checker/structure.py::_check_decl_exprs`.
pub fn _check_decl_exprs(
    _stmt: &Declaration,
    _known: &BTreeSet<String>,
    _variables: &BTreeSet<String>,
    _diags: &mut Diagnostics,
    _struct_types: &BTreeSet<String>,
    _stop_at: Option<i64>,
    _indexed_constants: &BTreeSet<String>,
) -> Option<i64> {
    todo!(
        "checker/structure.py::_check_decl_exprs (omitted: profile: untyped, length_scope: tuple[set[str], set[str]] | None)"
    )
}

/// Port of `checker/structure.py::_check_alias_index_names`.
pub fn _check_alias_index_names(
    _stmt: &Instruction,
    _known: &BTreeSet<String>,
    _variables: &BTreeSet<String>,
    _diags: &mut Diagnostics,
    _check_variables: bool,
    _indexed_constants: &BTreeSet<String>,
) -> bool {
    todo!("checker/structure.py::_check_alias_index_names (omitted: profile: untyped)")
}

/// Port of `checker/structure.py::_alias_flat_offset`.
pub fn _alias_flat_offset(_index: Vec<i64>, _dims: Vec<i64>) -> i64 {
    todo!("checker/structure.py::_alias_flat_offset")
}

/// Port of `checker/structure.py::_alias_string_slots`.
pub fn _alias_string_slots(_program: &Program) {
    todo!("checker/structure.py::_alias_string_slots (omitted: env: consteval.Env)")
}

/// Port of `checker/structure.py::_alias_parameter_declaration`.
pub fn _alias_parameter_declaration(
    _routine: &SubDef,
    _declared: &OrderedMap<String, String>,
    _known: &BTreeSet<String>,
    _forward: &BTreeSet<String>,
    _diags: Option<&mut Diagnostics>,
    _structs_seen: &BTreeSet<String>,
) {
    todo!(
        "checker/structure.py::_alias_parameter_declaration (omitted: env: consteval.Env, profile: untyped)"
    )
}

/// Port of `checker/structure.py::_alias_name_collisions`.
pub fn _alias_name_collisions(
    _stmt: &Instruction,
    _declared: &OrderedMap<String, String>,
    _in_routine: bool,
) -> OrderedMap<usize, String> {
    todo!("checker/structure.py::_alias_name_collisions (omitted: profile: untyped)")
}
