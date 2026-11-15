//! Port of `compiler.py`.

#![allow(dead_code, unused_variables, unused_imports)]

use std::collections::BTreeSet;
use std::rc::Rc;

use crate::diagnostics::{DiagRef, Diagnostic, Diagnostics};
use crate::profiles::Profile;
use crate::syntax::{
    Assignment, Call, Case, ConstDecl, Declaration, Expr, ExprRef, Index, Instruction, Juxtaposed,
    Member, Number, OrderedMap, Program, ProgramRef, Stmt, StmtRef, SubDef, Unparsed, VarDecl,
};

/// Port of `compiler.py::_no_line_kind`.
pub fn _no_line_kind() -> i64 {
    todo!("compiler.py::_no_line_kind (omitted: d: untyped)")
}

/// Port of `compiler.py::_bounded_by_message_buffer`.
pub fn _bounded_by_message_buffer(_lines: Vec<String>) -> Vec<String> {
    todo!("compiler.py::_bounded_by_message_buffer")
}

/// Port of `compiler.py::CompileResult`.
#[derive(Debug, Default)]
pub struct CompileResult {}

impl CompileResult {
    /// Port of `compiler.py::CompileResult::record_implementation_failure`.
    pub fn record_implementation_failure(&self, _where_: &str) {
        todo!("compiler.py::CompileResult::record_implementation_failure (omitted: exc: Exception)")
    }

    /// Port of `compiler.py::CompileResult::_position`.
    pub fn _position(&self) {
        todo!("compiler.py::CompileResult::_position (omitted: d: untyped)")
    }

    /// Port of `compiler.py::CompileResult::ok`.
    pub fn ok(&self) -> bool {
        todo!("compiler.py::CompileResult::ok")
    }

    /// Port of `compiler.py::CompileResult::_sort_key`.
    pub fn _sort_key(&self) {
        todo!("compiler.py::CompileResult::_sort_key (omitted: d: untyped)")
    }

    /// Port of `compiler.py::CompileResult::result_lines`.
    pub fn result_lines(&self, _show_signature: bool) -> Vec<String> {
        todo!("compiler.py::CompileResult::result_lines")
    }
}

/// Port of `compiler.py::dropped_message`.
pub fn dropped_message(_result: &mut CompileResult) {
    todo!("compiler.py::dropped_message")
}

/// Port of `compiler.py::compile_file`.
pub fn compile_file(
    _path: &std::path::Path,
    _profile: Option<&Profile>,
    _display_name: Option<&str>,
    _warn_unused: bool,
    _provenance: bool,
) -> CompileResult {
    todo!("compiler.py::compile_file (omitted: source: SourceProvider | None)")
}

/// Port of `compiler.py::_compile_once`.
pub fn _compile_once(
    _path: &std::path::Path,
    _profile: &Profile,
    _name: &str,
    _pull_limit: Option<Vec<i64>>,
    _provenance: bool,
) {
    todo!("compiler.py::_compile_once (omitted: source: SourceProvider | None)")
}

/// Port of `compiler.py::_finish`.
pub fn _finish(
    _result: &mut CompileResult,
    _path: &std::path::Path,
    _warn_unused: bool,
) -> CompileResult {
    todo!("compiler.py::_finish (omitted: source: SourceProvider | None)")
}
