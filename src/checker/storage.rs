//! Port of `checker/storage.py`.

#![allow(dead_code, unused_variables, unused_imports)]

use std::collections::BTreeSet;
use std::rc::Rc;

use crate::diagnostics::{DiagRef, Diagnostic, Diagnostics};
use crate::profiles::Profile;
use crate::syntax::{
    Assignment, Call, Case, ConstDecl, Declaration, Expr, ExprRef, Index, Instruction, Juxtaposed,
    Member, Number, OrderedMap, Program, ProgramRef, Stmt, StmtRef, SubDef, Unparsed, VarDecl,
};

/// Port of `checker/storage.py::_f32`.
pub fn _f32(_x: f64) -> f64 {
    todo!("checker/storage.py::_f32")
}

/// Port of `checker/storage.py::_cvt_int`.
pub fn _cvt_int(_x: f64) -> i64 {
    todo!("checker/storage.py::_cvt_int")
}

/// Port of `checker/storage.py::_wrap32`.
pub fn _wrap32(_v: i64) -> i64 {
    todo!("checker/storage.py::_wrap32")
}

/// Port of `checker/storage.py::_i32`.
pub fn _i32(_v: i64) -> i64 {
    todo!("checker/storage.py::_i32")
}

/// Port of `checker/storage.py::_min_recs`.
pub fn _min_recs(_k: i64, _cc: i64) -> i64 {
    todo!("checker/storage.py::_min_recs (omitted: sp: dict)")
}

/// Port of `checker/storage.py::preserve_charge`.
pub fn preserve_charge(
    _a: i64,
    _d: i64,
    _q_arg: i64,
    _min_tick: i64,
    _tick: i64,
    _public_bytes: i64,
) -> i64 {
    todo!("checker/storage.py::preserve_charge")
}

/// Port of `checker/storage.py::_round_page`.
pub fn _round_page(_v: i64) -> i64 {
    todo!("checker/storage.py::_round_page")
}

/// Port of `checker/storage.py::allocate`.
pub fn allocate(_pool: i64) {
    todo!("checker/storage.py::allocate (omitted: specs: list[dict])")
}

/// Port of `checker/storage.py::_lap`.
pub fn _lap() -> i64 {
    todo!("checker/storage.py::_lap (omitted: t: dict)")
}

/// Port of `checker/storage.py::_tdiv`.
pub fn _tdiv(_a: i64, _b: i64) -> i64 {
    todo!("checker/storage.py::_tdiv")
}

/// Port of `checker/storage.py::_alloc`.
pub fn _alloc(_ptr: i64, _u: i64) -> (i64, i64) {
    todo!("checker/storage.py::_alloc")
}

/// Port of `checker/storage.py::_blockfix`.
pub fn _blockfix(_arg: i64) {
    todo!("checker/storage.py::_blockfix (omitted: t: dict)")
}

/// Port of `checker/storage.py::pool_bytes`.
pub fn pool_bytes(
    _profile_name: &str,
    _n_declared: i64,
    _has_public: bool,
    _n_named_const: i64,
) -> i64 {
    todo!("checker/storage.py::pool_bytes")
}

/// Port of `checker/storage.py::spec_of`.
pub fn spec_of() {
    todo!("checker/storage.py::spec_of (omitted: m: dict)")
}

/// Port of `checker/storage.py::run`.
pub fn run(_pool: i64) {
    todo!("checker/storage.py::run (omitted: metas: list[dict])")
}

/// Port of `checker/storage.py::cr6_table_oom`.
pub fn cr6_table_oom(_auto_n: Option<i64>, _name: &str) -> Option<String> {
    todo!("checker/storage.py::cr6_table_oom (omitted: m: dict)")
}

/// Port of `checker/storage.py::_var_words`.
pub fn _var_words(_program: &Program) {
    todo!("checker/storage.py::_var_words (omitted: env: untyped, kinds: tuple)")
}

/// Port of `checker/storage.py::_segment_oom`.
pub fn _segment_oom() -> bool {
    todo!("checker/storage.py::_segment_oom (omitted: words: list[tuple[int, int]])")
}

/// Port of `checker/storage.py::variables_oom`.
pub fn variables_oom(_program: &Program) -> bool {
    todo!("checker/storage.py::variables_oom (omitted: profile: untyped, env: untyped)")
}

/// Port of `checker/storage.py::string_length_oom_line`.
pub fn string_length_oom_line(_program: &Program) -> Option<i64> {
    todo!("checker/storage.py::string_length_oom_line (omitted: diags: untyped)")
}

/// Port of `checker/storage.py::check_pass1`.
pub fn check_pass1(_program: &Program) {
    todo!(
        "checker/storage.py::check_pass1 (omitted: profile: untyped, diags: untyped, env: untyped)"
    )
}

/// Port of `checker/storage.py::public_table_oom`.
pub fn public_table_oom(_program: &Program) -> bool {
    todo!("checker/storage.py::public_table_oom (omitted: env: untyped)")
}

/// Port of `checker/storage.py::check`.
pub fn check(_program: &Program) {
    todo!("checker/storage.py::check (omitted: profile: untyped, result: untyped, diags: untyped)")
}
