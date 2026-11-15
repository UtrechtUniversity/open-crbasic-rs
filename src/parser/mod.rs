//! Port of open-crbasic `parser.py`: CRBasic parser (logical lines -> `syntax::Program`).

#![allow(dead_code, unused_imports, clippy::used_underscore_items)]

pub mod exprs;
pub mod helpers;
pub mod stmts;

use std::collections::HashSet;
use std::path::Path;
use std::rc::Rc;

use crate::consteval;
use crate::diagnostics::Diagnostics;
use crate::profiles::Profile;
use crate::syntax::{
    ExprRef, LineSpan, LogicalLine, OrderedMap, Program, ProgramRef, Stmt, Token, VarDecl,
};

/// Port of `parser.py::_Parser`
pub struct Parser<'a> {
    pub provenance: bool,
    pub profile: &'a Profile,
    pub diags: Diagnostics,
    pub env: consteval::Env,
    pub cond_stack: Vec<helpers::CondFrame>,
    pub program: Program,
    pub line_map: OrderedMap<String, OrderedMap<i64, Vec<LineSpan>>>,
    pub done: bool,
    pub inline_body: bool,
    pub in_struct: bool,
    pub in_table: bool,
    pub function_names: HashSet<String>,
    pub collect: bool,
    pub to_ok: bool,
    pub logical_assign: bool,
}

impl Parser<'_> {
    /// Port of `parser.py::_Parser.__init__`
    pub fn new(profile: &'static Profile, diags: Diagnostics, provenance: bool) -> Self {
        Self {
            provenance,
            profile,
            diags,
            env: consteval::Env::new(profile),
            cond_stack: Vec::new(),
            program: Program::new("", Vec::new()),
            line_map: OrderedMap::new(),
            done: false,
            inline_body: false,
            in_struct: false,
            in_table: false,
            function_names: HashSet::new(),
            collect: false,
            to_ok: false,
            logical_assign: false,
        }
    }

    // ---- Chunk 3 stubs (decls.rs) ----
    pub fn _parse_const(
        &mut self,
        _p: &mut helpers::TokStream,
        _line: i64,
        _file: &str,
    ) -> Result<Stmt, helpers::ParseError> {
        todo!("Chunk 3: _parse_const")
    }

    pub fn _parse_declaration(
        &mut self,
        _p: &mut helpers::TokStream,
        _line: i64,
        _file: &str,
        _kind: &str,
    ) -> Result<Stmt, helpers::ParseError> {
        todo!("Chunk 3: _parse_declaration")
    }

    pub fn _numeric_name(_p: &mut helpers::TokStream) -> Option<String> {
        todo!("Chunk 3: _numeric_name")
    }

    pub fn _parse_dim(
        &mut self,
        _p: &mut helpers::TokStream,
        _line: i64,
        _file: &str,
    ) -> Result<ExprRef, helpers::ParseError> {
        todo!("Chunk 3: _parse_dim")
    }

    pub fn _parse_vardecl(
        &mut self,
        _p: &mut helpers::TokStream,
        _line: i64,
        _file: &str,
    ) -> Result<Rc<VarDecl>, helpers::ParseError> {
        todo!("Chunk 3: _parse_vardecl")
    }

    pub fn _parse_alias(
        &mut self,
        _p: &mut helpers::TokStream,
        _line: i64,
        _file: &str,
    ) -> Result<Stmt, helpers::ParseError> {
        todo!("Chunk 3: _parse_alias")
    }

    // ---- Chunk 4 stubs (driver.rs) ----
    pub fn _parse_segment(
        &mut self,
        _seg: Vec<Token>,
        _line: i64,
        _file: &str,
        _seg_text: &str,
    ) -> Stmt {
        todo!("Chunk 4: _parse_segment")
    }
}

/// Port of `parser.py::_profile_for`
pub fn _profile_for(name: &str) -> &'static Profile {
    crate::profiles::for_file(Path::new(name))
}

/// Port of `parser.py::partial_expression`
pub fn partial_expression(_text: &str, _profile: &Profile) -> Option<ExprRef> {
    todo!("Chunk 4: partial_expression")
}

/// Port of `parser.py::parse_file`
pub fn parse_file(
    _path: &Path,
    _profile: Option<&Profile>,
    _pull_limit: Option<&[i64]>,
    _provenance: bool,
) -> Result<(ProgramRef, Diagnostics), std::io::Error> {
    todo!("Chunk 4: parse_file")
}

/// Port of `parser.py::_pulls_init`
pub fn _pulls_init(profile: &Profile) -> bool {
    matches!(profile.name, "cr6" | "cr1000x" | "cr300" | "cr350")
}

/// Port of `parser.py::parse_text`
pub fn parse_text(
    _text: &str,
    _name: &str,
    _directory: Option<&Path>,
    _profile: Option<&Profile>,
) -> (ProgramRef, Diagnostics) {
    todo!("Chunk 4: parse_text")
}
