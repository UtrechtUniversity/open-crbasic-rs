//! Port of `checker/scan_time.py`.

#![allow(dead_code, unused_variables, unused_imports)]

use std::collections::BTreeSet;
use std::rc::Rc;

use crate::diagnostics::{DiagRef, Diagnostic, Diagnostics};
use crate::profiles::Profile;
use crate::syntax::{
    Assignment, Call, Case, ConstDecl, Declaration, Expr, ExprRef, Index, Instruction, Juxtaposed,
    Member, Number, OrderedMap, Program, ProgramRef, Stmt, StmtRef, SubDef, Unparsed, VarDecl,
};

/// Port of `checker/scan_time.py::delay_us`.
pub fn delay_us(_value: f64, _unit: i64) -> i64 {
    todo!("checker/scan_time.py::delay_us")
}

/// Port of `checker/scan_time.py::excite_delay_us`.
pub fn excite_delay_us(_value: f64) -> i64 {
    todo!("checker/scan_time.py::excite_delay_us")
}

/// Port of `checker/scan_time.py::task_memory_overflow`.
pub fn task_memory_overflow(_us: i64) -> bool {
    todo!("checker/scan_time.py::task_memory_overflow")
}

/// Port of `checker/scan_time.py::amount`.
pub fn amount(_excess: i64, _unit: i64) -> i64 {
    todo!("checker/scan_time.py::amount")
}

/// Port of `checker/scan_time.py::_f32`.
pub fn _f32(_x: f64) -> f64 {
    todo!("checker/scan_time.py::_f32")
}

/// Port of `checker/scan_time.py::i32`.
pub fn i32(_x: i64) -> i64 {
    todo!("checker/scan_time.py::i32")
}

/// Port of `checker/scan_time.py::_cint`.
pub fn _cint(_x: f64) -> i64 {
    todo!("checker/scan_time.py::_cint")
}

/// Port of `checker/scan_time.py::_cdiv`.
pub fn _cdiv(_a: i64, _b: i64) -> i64 {
    todo!("checker/scan_time.py::_cdiv")
}

/// Port of `checker/scan_time.py::Model`.
#[derive(Debug, Default)]
pub struct Model {}

impl Model {
    /// Port of `checker/scan_time.py::Model::integ_code`.
    pub fn integ_code(&self, _fnotch: f64) -> Option<i64> {
        todo!("checker/scan_time.py::Model::integ_code")
    }

    /// Port of `checker/scan_time.py::Model::integ_time`.
    pub fn integ_time(&self, _k: i64) -> i64 {
        todo!("checker/scan_time.py::Model::integ_time")
    }

    /// Port of `checker/scan_time.py::Model::settle_code`.
    pub fn settle_code(&self, _settling: i64) -> i64 {
        todo!("checker/scan_time.py::Model::settle_code")
    }

    /// Port of `checker/scan_time.py::Model::settle_units`.
    pub fn settle_units(&self, _code: i64) -> i64 {
        todo!("checker/scan_time.py::Model::settle_units")
    }

    /// Port of `checker/scan_time.py::Model::open_detect_code`.
    pub fn open_detect_code(&self) -> i64 {
        todo!("checker/scan_time.py::Model::open_detect_code")
    }

    /// Port of `checker/scan_time.py::Model::fixed_code`.
    pub fn fixed_code(&self, _constant: i64) -> i64 {
        todo!("checker/scan_time.py::Model::fixed_code")
    }
}

/// Port of `checker/scan_time.py::_CR6`.
#[derive(Debug, Default)]
pub struct CR6 {}

impl CR6 {
    /// Port of `checker/scan_time.py::_CR6::integ_time`.
    pub fn integ_time(&self) {
        todo!("checker/scan_time.py::_CR6::integ_time (omitted: k: untyped)")
    }

    /// Port of `checker/scan_time.py::_CR6::settle_code`.
    pub fn settle_code(&self) {
        todo!("checker/scan_time.py::_CR6::settle_code (omitted: settling: untyped)")
    }

    /// Port of `checker/scan_time.py::_CR6::settle_units`.
    pub fn settle_units(&self) {
        todo!("checker/scan_time.py::_CR6::settle_units (omitted: code: untyped)")
    }

    /// Port of `checker/scan_time.py::_CR6::open_detect_code`.
    pub fn open_detect_code(&self) {
        todo!("checker/scan_time.py::_CR6::open_detect_code")
    }

    /// Port of `checker/scan_time.py::_CR6::fixed_code`.
    pub fn fixed_code(&self) {
        todo!("checker/scan_time.py::_CR6::fixed_code (omitted: constant: untyped)")
    }
}

/// Port of `checker/scan_time.py::_CR1000X`.
#[derive(Debug, Default)]
pub struct CR1000X {}

impl CR1000X {
    /// Port of `checker/scan_time.py::_CR1000X::integ_time`.
    pub fn integ_time(&self) {
        todo!("checker/scan_time.py::_CR1000X::integ_time (omitted: k: untyped)")
    }

    /// Port of `checker/scan_time.py::_CR1000X::settle_code`.
    pub fn settle_code(&self) {
        todo!("checker/scan_time.py::_CR1000X::settle_code (omitted: settling: untyped)")
    }

    /// Port of `checker/scan_time.py::_CR1000X::settle_units`.
    pub fn settle_units(&self) {
        todo!("checker/scan_time.py::_CR1000X::settle_units (omitted: code: untyped)")
    }

    /// Port of `checker/scan_time.py::_CR1000X::open_detect_code`.
    pub fn open_detect_code(&self) {
        todo!("checker/scan_time.py::_CR1000X::open_detect_code")
    }

    /// Port of `checker/scan_time.py::_CR1000X::fixed_code`.
    pub fn fixed_code(&self) {
        todo!("checker/scan_time.py::_CR1000X::fixed_code (omitted: constant: untyped)")
    }
}

/// Port of `checker/scan_time.py::analog_units`.
pub fn analog_units(
    _m: &Model,
    _reps: i64,
    _gain: i64,
    _fnotch: f64,
    _settling: i64,
    _single_ended: bool,
    _meas_off: bool,
    _reverse: bool,
    _excite: bool,
    _open_detect: bool,
    _groups: i64,
) -> Option<i64> {
    todo!("checker/scan_time.py::analog_units")
}

/// Port of `checker/scan_time.py::panel_units`.
pub fn panel_units(_m: &Model, _fnotch: f64) -> Option<i64> {
    todo!("checker/scan_time.py::panel_units")
}

/// Port of `checker/scan_time.py::battery_units`.
pub fn battery_units(_m: &Model) -> i64 {
    todo!("checker/scan_time.py::battery_units")
}

/// Port of `checker/scan_time.py::Budget`.
#[derive(Debug, Default)]
pub struct Budget {}

/// Port of `checker/scan_time.py::_value`.
pub fn _value(_expr: Option<&Expr>) {
    todo!("checker/scan_time.py::_value (omitted: env: consteval.Env)")
}

/// Port of `checker/scan_time.py::_num`.
pub fn _num(_expr: Option<&Expr>) -> Option<f64> {
    todo!("checker/scan_time.py::_num (omitted: env: consteval.Env)")
}

/// Port of `checker/scan_time.py::_long`.
pub fn _long(_expr: Option<&Expr>) -> Option<i64> {
    todo!("checker/scan_time.py::_long (omitted: env: consteval.Env)")
}

/// Port of `checker/scan_time.py::_settling`.
pub fn _settling(_expr: Option<&Expr>) -> Option<i64> {
    todo!("checker/scan_time.py::_settling (omitted: env: consteval.Env)")
}

/// Port of `checker/scan_time.py::_bridge_units`.
pub fn _bridge_units(_m: &Model) -> Option<(i64, bool, i64)> {
    todo!(
        "checker/scan_time.py::_bridge_units (omitted: args: list, env: consteval.Env, spec: tuple)"
    )
}

/// Port of `checker/scan_time.py::instruction_units`.
pub fn instruction_units(_stmt: &Instruction, _profile: &Profile) -> Option<(i64, bool, i64)> {
    todo!("checker/scan_time.py::instruction_units (omitted: env: consteval.Env)")
}

/// Port of `checker/scan_time.py::budgets`.
pub fn budgets(_program: &Program, _profile: &Profile, _include_zero: bool) {
    todo!("checker/scan_time.py::budgets (omitted: env: consteval.Env)")
}
