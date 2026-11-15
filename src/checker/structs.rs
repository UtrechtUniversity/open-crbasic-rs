//! Port of `checker/structs.py`.

#![allow(dead_code, unused_variables, unused_imports)]

use std::collections::BTreeSet;
use std::rc::Rc;

use crate::diagnostics::{DiagRef, Diagnostic, Diagnostics};
use crate::profiles::Profile;
use crate::syntax::{
    Assignment, Call, Case, ConstDecl, Declaration, Expr, ExprRef, Index, Instruction, Juxtaposed,
    Member, Number, OrderedMap, Program, ProgramRef, Stmt, StmtRef, SubDef, Unparsed, VarDecl,
};

/// Port of `checker/structs.py::_structure_members`.
pub fn _structure_members(_program: &Program) {
    todo!("checker/structs.py::_structure_members")
}

/// Port of `checker/structs.py::_variable_kinds`.
pub fn _variable_kinds(_program: &Program) -> OrderedMap<String, String> {
    todo!("checker/structs.py::_variable_kinds (omitted: structs: dict)")
}

/// Port of `checker/structs.py::_names`.
pub fn _names(_expr: Option<&Expr>) -> Vec<String> {
    todo!("checker/structs.py::_names")
}

/// Port of `checker/structs.py::_member_chain`.
pub fn _member_chain(_expr: &Member) {
    todo!("checker/structs.py::_member_chain")
}

/// Port of `checker/structs.py::_member_errors`.
pub fn _member_errors(_expr: Option<&Expr>, _kinds: &OrderedMap<String, String>) {
    todo!("checker/structs.py::_member_errors (omitted: structs: dict)")
}

/// Port of `checker/structs.py::check_members`.
pub fn check_members(_program: &Program, _diags: &mut Diagnostics, _flat: &Program) {
    todo!("checker/structs.py::check_members (omitted: profile: untyped)")
}

/// Port of `checker/structs.py::_operand_kind`.
pub fn _operand_kind(_expr: Option<&Expr>, _kinds: &OrderedMap<String, String>) -> Option<String> {
    todo!("checker/structs.py::_operand_kind (omitted: structs: dict)")
}

/// Port of `checker/structs.py::check_struct_assignment`.
pub fn check_struct_assignment(_program: &Program, _diags: &mut Diagnostics, _flat: &Program) {
    todo!("checker/structs.py::check_struct_assignment (omitted: profile: untyped)")
}
