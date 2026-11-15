//! Port of `checker/structure.py` (lines 1-779).

#![allow(dead_code, unused_variables, unused_imports)]

use std::collections::BTreeSet;
use std::rc::Rc;

use crate::diagnostics::{DiagRef, Diagnostic, Diagnostics};
use crate::profiles::Profile;
use crate::syntax::{
    Assignment, Call, Case, ConstDecl, Declaration, Expr, ExprRef, Index, Instruction, Juxtaposed,
    Member, Number, OrderedMap, Program, ProgramRef, Stmt, StmtRef, SubDef, Unparsed, VarDecl,
};

/// Port of `checker/structure.py::_assignment_head`.
pub fn _assignment_head(_text: &str) -> bool {
    todo!("checker/structure.py::_assignment_head (omitted: program: untyped, profile: untyped)")
}

/// Port of `checker/structure.py::_track_tables`.
pub fn _track_tables(_stmt: &Stmt, _kw: &str) {
    todo!("checker/structure.py::_track_tables (omitted: state: dict)")
}

/// Port of `checker/structure.py::_calltable_known`.
pub fn _calltable_known(_stmt: &Stmt) {
    todo!("checker/structure.py::_calltable_known (omitted: state: dict)")
}

/// Port of `checker/structure.py::_scan_counts_for_table_placement`.
pub fn _scan_counts_for_table_placement(_stmt: &Stmt) -> bool {
    todo!(
        "checker/structure.py::_scan_counts_for_table_placement (omitted: env: consteval.Env | None)"
    )
}

/// Port of `checker/structure.py::_check_calltable_outside_scan`.
pub fn _check_calltable_outside_scan(
    _stmt: &Instruction,
    _program: &Program,
    _diags: &mut Diagnostics,
    _in_sub: bool,
    _zero_scan: bool,
) {
    todo!("checker/structure.py::_check_calltable_outside_scan (omitted: env: untyped)")
}

/// Port of `checker/structure.py::_accept_known`.
pub fn _accept_known(_stmt: &Stmt) {
    todo!("checker/structure.py::_accept_known (omitted: state: dict)")
}

/// Port of `checker/structure.py::_check_accept_table`.
pub fn _check_accept_table(
    _stmt: &Instruction,
    _program: &Program,
    _diags: &mut Diagnostics,
    _outside: bool,
) {
    todo!("checker/structure.py::_check_accept_table (omitted: env: untyped)")
}

/// Port of `checker/structure.py::_ends_sub`.
pub fn _ends_sub(_stmt: &Stmt, _kw: &str) -> bool {
    todo!("checker/structure.py::_ends_sub")
}

/// Port of `checker/structure.py::first_misplaced_sub`.
pub fn first_misplaced_sub(_program: &Program) {
    todo!("checker/structure.py::first_misplaced_sub")
}

/// Port of `checker/structure.py::check_apply_restart_sequence`.
pub fn check_apply_restart_sequence(_program: &Program, _diags: &mut Diagnostics) {
    todo!("checker/structure.py::check_apply_restart_sequence (omitted: profile: untyped)")
}

/// Port of `checker/structure.py::first_misplaced_data_table`.
pub fn first_misplaced_data_table(_program: &Program) -> Option<StmtRef> {
    todo!("checker/structure.py::first_misplaced_data_table (omitted: profile: untyped)")
}

/// Port of `checker/structure.py::check_data_table_placement`.
pub fn check_data_table_placement(_program: &Program, _diags: &mut Diagnostics) {
    todo!("checker/structure.py::check_data_table_placement (omitted: profile: untyped)")
}

/// Port of `checker/structure.py::pipeline_task_errors`.
pub fn pipeline_task_errors(_program: &Program) {
    todo!("checker/structure.py::pipeline_task_errors (omitted: profile: untyped)")
}

/// Port of `checker/structure.py::check_pipeline_task_placement`.
pub fn check_pipeline_task_placement(_program: &Program, _diags: &mut Diagnostics) {
    todo!("checker/structure.py::check_pipeline_task_placement (omitted: profile: untyped)")
}

/// Port of `checker/structure.py::pipeline_argument_error_first_locations`.
pub fn pipeline_argument_error_first_locations(_program: &Program) {
    todo!(
        "checker/structure.py::pipeline_argument_error_first_locations (omitted: profile: untyped)"
    )
}

/// Port of `checker/structure.py::check_slow_sequence_blocks`.
pub fn check_slow_sequence_blocks(_program: &Program, _diags: &mut Diagnostics) {
    todo!("checker/structure.py::check_slow_sequence_blocks (omitted: profile: untyped)")
}

/// Port of `checker/structure.py::check_scan_placement_late`.
pub fn check_scan_placement_late(_program: &Program, _diags: &mut Diagnostics) {
    todo!("checker/structure.py::check_scan_placement_late (omitted: profile: untyped)")
}
