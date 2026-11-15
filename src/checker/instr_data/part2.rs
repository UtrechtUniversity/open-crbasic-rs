//! Port of `checker/instr_data.py` (lines 760-1473).

#![allow(dead_code, unused_variables, unused_imports)]

use std::collections::BTreeSet;
use std::rc::Rc;

use crate::diagnostics::{DiagRef, Diagnostic, Diagnostics};
use crate::profiles::Profile;
use crate::syntax::{
    Assignment, Call, Case, ConstDecl, Declaration, Expr, ExprRef, Index, Instruction, Juxtaposed,
    Member, Number, OrderedMap, Program, ProgramRef, Stmt, StmtRef, SubDef, Unparsed, VarDecl,
};

/// Port of `checker/instr_data.py::check_units_targets`.
pub fn check_units_targets(_program: &Program, _profile: &Profile, _diags: &mut Diagnostics) {
    todo!("checker/instr_data.py::check_units_targets")
}

/// Port of `checker/instr_data.py::check_table_and_menu_placement`.
pub fn check_table_and_menu_placement(
    _program: &Program,
    _profile: &Profile,
    _diags: &mut Diagnostics,
) -> Option<(String, i64)> {
    todo!("checker/instr_data.py::check_table_and_menu_placement (omitted: env: untyped)")
}

/// Port of `checker/instr_data.py::_boolean_pick_ok`.
pub fn _boolean_pick_ok(_token: &str) -> bool {
    todo!("checker/instr_data.py::_boolean_pick_ok (omitted: env: untyped)")
}

/// Port of `checker/instr_data.py::_pick_tokens`.
pub fn _pick_tokens(_stmt: &Instruction) -> Option<Vec<String>> {
    todo!("checker/instr_data.py::_pick_tokens")
}

/// Port of `checker/instr_data.py::_check_boolean_picklist`.
pub fn _check_boolean_picklist(_stmt: &Instruction, _diags: &mut Diagnostics) {
    todo!("checker/instr_data.py::_check_boolean_picklist (omitted: env: untyped)")
}

/// Port of `checker/instr_data.py::_pick_value_bad`.
pub fn _pick_value_bad(_token: &str, _kind: &str) -> Option<String> {
    todo!(
        "checker/instr_data.py::_pick_value_bad (omitted: env: untyped, program: untyped, profile: untyped, variables: untyped)"
    )
}

/// Port of `checker/instr_data.py::_check_pick_consts`.
pub fn _check_pick_consts() {
    todo!(
        "checker/instr_data.py::_check_pick_consts (omitted: stmt: untyped, kind: untyped, env: untyped, program: untyped, profile: untyped, variables: untyped, diags: untyped, skip_first: untyped)"
    )
}

/// Port of `checker/instr_data.py::_destination_expression`.
pub fn _destination_expression(_expr: Option<&Expr>) -> bool {
    todo!("checker/instr_data.py::_destination_expression")
}

/// Port of `checker/instr_data.py::constant_destination`.
pub fn constant_destination() -> bool {
    todo!(
        "checker/instr_data.py::constant_destination (omitted: expr: untyped, env: untyped, variables: untyped)"
    )
}

/// Port of `checker/instr_data.py::_destination_env`.
pub fn _destination_env() {
    todo!("checker/instr_data.py::_destination_env (omitted: env: consteval.Env)")
}

/// Port of `checker/instr_data.py::_reps_value`.
pub fn _reps_value() -> Option<i64> {
    todo!("checker/instr_data.py::_reps_value (omitted: expr: untyped, env: untyped)")
}

/// Port of `checker/instr_data.py::check_stage2a`.
pub fn check_stage2a(_program: &Program, _profile: &Profile, _diags: &mut Diagnostics) -> bool {
    todo!("checker/instr_data.py::check_stage2a (omitted: env: untyped)")
}

/// Port of `checker/instr_data.py::check_table_reps_late`.
pub fn check_table_reps_late(_program: &Program, _profile: &Profile, _diags: &mut Diagnostics) {
    todo!("checker/instr_data.py::check_table_reps_late (omitted: env: untyped)")
}

/// Port of `checker/instr_data.py::_report_undefined_source`.
pub fn _report_undefined_source(
    _st: &Instruction,
    _known: &BTreeSet<String>,
    _profile: &Profile,
    _diags: &mut Diagnostics,
) -> bool {
    todo!("checker/instr_data.py::_report_undefined_source (omitted: record: dict)")
}

/// Port of `checker/instr_data.py::_is_struct_typed`.
pub fn _is_struct_typed(_program: &Program, _name: &str) -> bool {
    todo!("checker/instr_data.py::_is_struct_typed")
}

/// Port of `checker/instr_data.py::_short_array`.
pub fn _short_array(_reps: i64) -> bool {
    todo!("checker/instr_data.py::_short_array (omitted: arg: untyped, dims: dict, env: untyped)")
}

/// Port of `checker/instr_data.py::_alias_sources`.
pub fn _alias_sources(_program: &Program) {
    todo!("checker/instr_data.py::_alias_sources (omitted: tables: untyped, env: untyped)")
}

/// Port of `checker/instr_data.py::_wind_options`.
pub fn _wind_options(
    _st: &Instruction,
    _reps: Option<i64>,
    _diags: &mut Diagnostics,
    _short_speed: bool,
) -> bool {
    todo!("checker/instr_data.py::_wind_options (omitted: env: untyped)")
}

/// Port of `checker/instr_data.py::check_constant_destinations`.
pub fn check_constant_destinations(
    _program: &Program,
    _profile: &Profile,
    _diags: &mut Diagnostics,
) {
    todo!("checker/instr_data.py::check_constant_destinations (omitted: env: untyped)")
}

/// Port of `checker/instr_data.py::drop_unreached_destinations`.
pub fn drop_unreached_destinations(
    _program: &Program,
    _profile: &Profile,
    _diags: &mut Diagnostics,
) {
    todo!("checker/instr_data.py::drop_unreached_destinations (omitted: env: untyped)")
}
