//! Port of `checker/longconv.py`.

#![allow(dead_code, unused_variables, unused_imports)]

use std::collections::BTreeSet;
use std::rc::Rc;

use crate::diagnostics::{DiagRef, Diagnostic, Diagnostics};
use crate::profiles::Profile;
use crate::syntax::{
    Assignment, Call, Case, ConstDecl, Declaration, Expr, ExprRef, Index, Instruction, Juxtaposed,
    Member, Number, OrderedMap, Program, ProgramRef, Stmt, StmtRef, SubDef, Unparsed, VarDecl,
};

/// Port of `checker/longconv.py::float_names`.
pub fn float_names() -> BTreeSet<String> {
    todo!("checker/longconv.py::float_names (omitted: statements: untyped)")
}

/// Port of `checker/longconv.py::literal_names`.
pub fn literal_names() -> BTreeSet<String> {
    todo!("checker/longconv.py::literal_names (omitted: statements: untyped)")
}

/// Port of `checker/longconv.py::_float_typed`.
pub fn _float_typed(_expr: Option<&Expr>) -> bool {
    todo!("checker/longconv.py::_float_typed (omitted: names: untyped)")
}

/// Port of `checker/longconv.py::_plain`.
pub fn _plain(_expr: Option<&Expr>) -> bool {
    todo!("checker/longconv.py::_plain")
}

/// Port of `checker/longconv.py::long_value`.
pub fn long_value(_arg: Option<&Expr>) -> Option<i64> {
    todo!(
        "checker/longconv.py::long_value (omitted: env: consteval.Env, profile: untyped, names: untyped, literals: untyped)"
    )
}

/// Port of `checker/longconv.py::_integer_literal`.
pub fn _integer_literal(_expr: Option<&Expr>) -> bool {
    todo!("checker/longconv.py::_integer_literal")
}

/// Port of `checker/longconv.py::_hex_inside`.
pub fn _hex_inside(_expr: Option<&Expr>) -> bool {
    todo!("checker/longconv.py::_hex_inside")
}

/// Port of `checker/longconv.py::_wrap`.
pub fn _wrap(_value: i64) -> i64 {
    todo!("checker/longconv.py::_wrap")
}

/// Port of `checker/longconv.py::_float32_long`.
pub fn _float32_long(_value: f64) -> i64 {
    todo!("checker/longconv.py::_float32_long")
}
