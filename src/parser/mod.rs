//! Port of open-crbasic `parser.py`: CRBasic parser (logical lines -> `syntax::Program`).

#![allow(dead_code, unused_imports, clippy::used_underscore_items)]

pub mod decls;
pub mod exprs;
pub mod helpers;
pub mod stmts;

use std::collections::HashSet;
use std::path::Path;
use std::rc::Rc;

use crate::consteval;
use crate::diagnostics::Diagnostics;
use crate::lexer;
use crate::profiles::Profile;
use crate::syntax::{
    ExprRef, LineSpan, LogicalLine, OrderedMap, Program, ProgramRef, Stmt, Token, TokenValue,
    VarDecl,
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

    // ---- Chunk 4 stubs (driver.rs) ----
    pub fn _parse_text_segment(&mut self, _seg_text_raw: &str, _line: i64, _file: &str) -> Stmt {
        todo!("Chunk 4: _parse_text_segment")
    }

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

pub(crate) fn tokenize_for_parser(text: &str, line: i64) -> Result<Vec<Token>, lexer::LexError> {
    let line_u = usize::try_from(line.max(0)).unwrap_or(0);
    let toks = lexer::tokenize(text, line_u)?;
    Ok(toks
        .into_iter()
        .map(|t| Token {
            kind: t.kind,
            text: t.text,
            upper: t.upper,
            col: i64::try_from(t.col).unwrap_or(0),
            line: i64::try_from(t.line).unwrap_or(0),
            value: match t.value {
                lexer::TokenValue::None => TokenValue::None,
                lexer::TokenValue::Int(n) => TokenValue::Int(n),
                lexer::TokenValue::Float(f) => TokenValue::Float(f),
                lexer::TokenValue::Str(s) => TokenValue::Str(s),
            },
        })
        .collect())
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
