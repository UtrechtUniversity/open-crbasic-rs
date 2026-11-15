//! Port of `checker/instr_data.py` (lines 1-759).

#![allow(dead_code, unused_variables, unused_imports)]

use std::collections::BTreeSet;
use std::rc::Rc;

use crate::diagnostics::{DiagRef, Diagnostic, Diagnostics};
use crate::profiles::Profile;
use crate::syntax::{
    Assignment, Call, Case, ConstDecl, Declaration, Expr, ExprRef, Index, Instruction, Juxtaposed,
    Member, Number, OrderedMap, Program, ProgramRef, Stmt, StmtRef, SubDef, Unparsed, VarDecl,
};

/// Port of `checker/instr_data.py::_table_name`.
pub fn _table_name(_stmt: &Instruction) -> Option<String> {
    todo!("checker/instr_data.py::_table_name")
}

/// Port of `checker/instr_data.py::calltable_targets`.
pub fn calltable_targets(_program: &Program) -> BTreeSet<String> {
    todo!("checker/instr_data.py::calltable_targets")
}

/// Port of `checker/instr_data.py::unknown_calltables`.
pub fn unknown_calltables(_program: &Program) {
    todo!("checker/instr_data.py::unknown_calltables")
}

/// Port of `checker/instr_data.py::_with_inline`.
pub fn _with_inline() {
    todo!("checker/instr_data.py::_with_inline (omitted: stmts: untyped)")
}

/// Port of `checker/instr_data.py::_const_table_name`.
pub fn _const_table_name(_name: &str, _program: &Program) -> String {
    todo!("checker/instr_data.py::_const_table_name")
}

/// Port of `checker/instr_data.py::_calltable_names`.
pub fn _calltable_names(_program: &Program) {
    todo!("checker/instr_data.py::_calltable_names")
}

/// Port of `checker/instr_data.py::unknown_table_references`.
pub fn unknown_table_references(_program: &Program, _profile: Option<&Profile>) {
    todo!("checker/instr_data.py::unknown_table_references")
}

/// Port of `checker/instr_data.py::_unknown_table_references`.
pub fn _unknown_table_references(_program: &Program, _profile: Option<&Profile>) {
    todo!("checker/instr_data.py::_unknown_table_references")
}

/// Port of `checker/instr_data.py::_getrecord_short`.
pub fn _getrecord_short(
    _stmt: &Instruction,
    _table: &str,
    _program: &Program,
    _profile: &Profile,
) -> Option<i64> {
    todo!("checker/instr_data.py::_getrecord_short")
}

/// Port of `checker/instr_data.py::check_unclosed_calltable`.
pub fn check_unclosed_calltable(
    _program: &Program,
    _diags: &mut Diagnostics,
    _profile: Option<&Profile>,
) {
    todo!("checker/instr_data.py::check_unclosed_calltable")
}

/// Port of `checker/instr_data.py::fieldnames_owners`.
pub fn fieldnames_owners(_program: &Program, _keyword: &str) {
    todo!("checker/instr_data.py::fieldnames_owners")
}

/// Port of `checker/instr_data.py::check_fieldclassify`.
pub fn check_fieldclassify(_program: &Program, _profile: &Profile, _diags: &mut Diagnostics) {
    todo!("checker/instr_data.py::check_fieldclassify (omitted: env: consteval.Env)")
}

/// Port of `checker/instr_data.py::move_fieldnames_symbol_errors`.
pub fn move_fieldnames_symbol_errors(_program: &Program, _diags: &mut Diagnostics) {
    todo!("checker/instr_data.py::move_fieldnames_symbol_errors")
}

/// Port of `checker/instr_data.py::check_fieldnames`.
pub fn check_fieldnames(_program: &Program, _profile: &Profile, _diags: &mut Diagnostics) {
    todo!("checker/instr_data.py::check_fieldnames (omitted: env: untyped)")
}

/// Port of `checker/instr_data.py::check_table_structure`.
pub fn check_table_structure(_program: &Program, _profile: &Profile, _diags: &mut Diagnostics) {
    todo!("checker/instr_data.py::check_table_structure")
}

/// Port of `checker/instr_data.py::_dup_entries`.
pub fn _dup_entries() {
    todo!("checker/instr_data.py::_dup_entries (omitted: table: untyped)")
}

/// Port of `checker/instr_data.py::check_duplicate_fields`.
pub fn check_duplicate_fields(_program: &Program, _diags: &mut Diagnostics) {
    todo!("checker/instr_data.py::check_duplicate_fields (omitted: result: untyped)")
}
