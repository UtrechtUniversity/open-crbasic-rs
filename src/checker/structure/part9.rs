//! Port of `checker/structure.py` (lines 5849-6309).

#![allow(dead_code, unused_variables, unused_imports)]

use std::collections::BTreeSet;
use std::rc::Rc;

use crate::diagnostics::{DiagRef, Diagnostic, Diagnostics};
use crate::profiles::Profile;
use crate::syntax::{
    Assignment, Call, Case, ConstDecl, Declaration, Expr, ExprRef, Index, Instruction, Juxtaposed,
    Member, Number, OrderedMap, Program, ProgramRef, Stmt, StmtRef, SubDef, Unparsed, VarDecl,
};

/// Port of `checker/structure.py::_check_alias_dims`.
pub fn _check_alias_dims(
    _stmt: &Instruction,
    _diags: &mut Diagnostics,
    _dim0_sizes: &OrderedMap<String, i64>,
    _all_dims: &OrderedMap<String, Vec<i64>>,
    _seen_alias_names: &BTreeSet<String>,
    _collisions: Option<OrderedMap<usize, String>>,
    _register_collisions: bool,
) {
    todo!(
        "checker/structure.py::_check_alias_dims (omitted: env: consteval.Env, seen_alias_sources: dict[str, set[tuple]], profile: untyped, string_slots: dict[str, tuple[int | None, int]] | None)"
    )
}

/// Port of `checker/structure.py::_alias_name_sizes`.
pub fn _alias_name_sizes(_stmt: &Instruction) -> Option<Vec<i64>> {
    todo!("checker/structure.py::_alias_name_sizes (omitted: env: consteval.Env)")
}

/// Port of `checker/structure.py::_alias_dim_ok`.
pub fn _alias_dim_ok(
    _stmt: &Instruction,
    _diags: &mut Diagnostics,
    _cur: i64,
    _n: i64,
    _total: i64,
    _check_start: bool,
    _any_start: bool,
) -> bool {
    todo!("checker/structure.py::_alias_dim_ok")
}

/// Port of `checker/structure.py::_alias_name_lead`.
pub fn _alias_name_lead(_stmt: &Instruction) {
    todo!("checker/structure.py::_alias_name_lead")
}

/// Port of `checker/structure.py::_after_keyword`.
pub fn _after_keyword(_stmt: &Unparsed) -> String {
    todo!("checker/structure.py::_after_keyword")
}

/// Port of `checker/structure.py::_junk_init_findings`.
pub fn _junk_init_findings(_stmt: &Unparsed) -> Vec<String> {
    todo!("checker/structure.py::_junk_init_findings (omitted: profile: untyped)")
}

/// Port of `checker/structure.py::_const_undefined_names`.
pub fn _const_undefined_names(_text: &str) -> Vec<String> {
    todo!(
        "checker/structure.py::_const_undefined_names (omitted: program: untyped, profile: untyped)"
    )
}

/// Port of `checker/structure.py::_check_unparsed_pass1`.
pub fn _check_unparsed_pass1(
    _stmt: &Unparsed,
    _diags: &mut Diagnostics,
    _program: &Program,
    _profile: &Profile,
) {
    todo!("checker/structure.py::_check_unparsed_pass1")
}
