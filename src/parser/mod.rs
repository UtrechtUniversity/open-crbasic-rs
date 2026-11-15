//! Port of open-crbasic `parser.py`: CRBasic parser (logical lines -> `syntax::Program`).

#![allow(dead_code, unused_imports, clippy::used_underscore_items)]

pub mod decls;
pub mod driver;
pub mod exprs;
pub mod helpers;
pub mod stmts;

use std::cell::RefCell;
use std::collections::HashSet;
use std::path::Path;
use std::rc::Rc;
use std::sync::LazyLock;

use crate::consteval;
use crate::diagnostics::Diagnostics;
use crate::lexer;
use crate::profiles::Profile;
use crate::pyre::{self, Regex};
use crate::pystr;
use crate::source;
use crate::sources::{DiskSource, SourceProvider};
use crate::syntax::{
    Expr, ExprRef, LineSpan, LogicalLine, OrderedMap, Program, ProgramRef, Stmt, Token, TokenValue,
};

type ParserLineMap = OrderedMap<String, Rc<RefCell<OrderedMap<i64, Rc<RefCell<Vec<LineSpan>>>>>>>;

/// Port of `parser.py::_Parser`
pub struct Parser {
    pub provenance: bool,
    pub profile: &'static Profile,
    pub diags: Diagnostics,
    pub env: consteval::Env,
    pub cond_stack: Vec<helpers::CondFrame>,
    pub program: Program,
    pub line_map: ParserLineMap,
    pub done: bool,
    pub inline_body: bool,
    pub in_struct: bool,
    pub in_table: bool,
    pub function_names: HashSet<String>,
    pub collect: bool,
    pub to_ok: bool,
    pub logical_assign: bool,
}

impl Parser {
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

    pub fn take_line_map(&self) -> OrderedMap<String, OrderedMap<i64, Vec<LineSpan>>> {
        let mut out = OrderedMap::new();
        for (file, inner_rc) in self.line_map.iter() {
            let mut inner_out = OrderedMap::new();
            for (&line, spans_rc) in inner_rc.borrow().iter() {
                inner_out.insert(line, spans_rc.borrow().clone());
            }
            out.insert(file.clone(), inner_out);
        }
        out
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
                lexer::TokenValue::BigInt(s) => TokenValue::BigInt(s),
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
pub fn partial_expression(text: &str, profile: &'static Profile) -> Option<ExprRef> {
    let toks: Vec<Token> = tokenize_for_parser(text, 0)
        .ok()?
        .into_iter()
        .filter(|t| t.kind != "EOF")
        .collect();
    let mut start: usize = 0;
    if toks.len() > 2 && toks[0].kind == "IDENT" {
        if toks[1].text == "=" {
            start = 2;
        } else if toks[1].text == "(" {
            let mut depth: i64 = 0;
            for j in 1..toks.len() {
                if toks[j].kind == "OP" {
                    depth += i64::from(toks[j].text == "(") - i64::from(toks[j].text == ")");
                }
                if depth == 0 {
                    start = if j + 1 < toks.len() && toks[j + 1].text == "=" {
                        j + 2
                    } else {
                        0
                    };
                    break;
                }
            }
        }
    }
    let mut parser_ = Parser::new(profile, Diagnostics::new(), true);
    parser_.logical_assign = true;
    let mut body: Vec<Token> = toks[start..].to_vec();
    let depth: i64 = body
        .iter()
        .filter(|t| t.kind == "OP")
        .map(|t| i64::from(t.text == "(") - i64::from(t.text == ")"))
        .sum();
    if let Some(last) = body.last()
        && ((last.kind == "IDENT" && exprs::_LOGICAL_OPS.contains(&last.upper.as_str()))
            || (last.kind == "OP" && last.text == "="))
    {
        body.push(Token {
            kind: "NUMBER".to_string(),
            text: "0".to_string(),
            upper: "0".to_string(),
            col: 0,
            line: 0,
            value: TokenValue::Int(0),
        });
    }
    let close_count = usize::try_from(depth.max(0)).unwrap_or(0);
    for _ in 0..close_count {
        body.push(Token {
            kind: "OP".to_string(),
            text: ")".to_string(),
            upper: ")".to_string(),
            col: 0,
            line: 0,
            value: TokenValue::None,
        });
    }
    body.push(helpers::_eof_like(0));
    if let Ok(expr) = parser_._parse_expr(&mut helpers::TokStream::new(body, text), 0, "") {
        return Some(expr);
    }
    for cut in (start + 1..=toks.len()).rev() {
        let mut sub = toks[start..cut].to_vec();
        sub.push(helpers::_eof_like(0));
        if let Ok(expr) = parser_._parse_expr(&mut helpers::TokStream::new(sub, text), 0, "") {
            return Some(expr);
        }
    }
    None
}

/// Port of `parser.py::parse_file`
pub fn parse_file(
    path: &Path,
    profile: Option<&'static Profile>,
    pull_limit: Option<&[i64]>,
    source_provider: Option<&dyn SourceProvider>,
    provenance: bool,
    macros: bool,
) -> Result<(ProgramRef, Diagnostics), std::io::Error> {
    let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    let prof = profile.unwrap_or_else(|| _profile_for(file_name));
    let (lines, mut diags) = source::load_program(
        path,
        _include_own_ext(prof.name),
        _pulls_init(prof),
        pull_limit,
        _cr3xx_include_exts(prof.name).as_deref(),
        source_provider,
        provenance,
        macros,
    );
    let pull_log = diags.provenance.pull_log.take();
    let mut parser = Parser::new(prof, diags, provenance);
    let (mut program, mut diags) = parser.run(&lines, file_name);
    program.provenance.pull_log = pull_log;
    program
        .provenance
        .pull_site
        .clone_from(&diags.provenance.pull_site);
    program
        .provenance
        .include_paths
        .clone_from(&diags.provenance.include_paths);
    program.provenance.line_map = Some(parser.take_line_map());
    program
        .provenance
        .occurrences
        .clone_from(&diags.provenance.occurrences);
    _include_warning_drives(&mut diags, prof);
    let disk = DiskSource;
    let src = source_provider.unwrap_or(&disk);
    let main_bytes = src.read(path).ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::NotFound, format!("{}", path.display()))
    })?;
    let main_text = pystr::decode_latin1(&main_bytes);
    _set_last_line(&mut program, &parser, &main_text);
    Ok((Rc::new(program), diags))
}

/// Port of `parser.py::_pulls_init`
pub fn _pulls_init(profile: &Profile) -> bool {
    matches!(profile.name, "cr6" | "cr1000x" | "cr300" | "cr350")
}

static _MISSING_INCLUDE_DRIVE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^(Cannot open include file  )(?:CPU|USR|CRD|USB):",
        pyre::I,
    )
});

fn _include_own_ext(profile_name: &str) -> Option<&'static str> {
    match profile_name {
        "cr6" => Some("CR6"),
        "cr1000x" => Some("CR1X"),
        _ => None,
    }
}

fn _cr3xx_include_exts(profile_name: &str) -> Option<Vec<&'static str>> {
    match profile_name {
        "cr300" => Some(vec!["CR300", "CR310"]),
        "cr350" => Some(vec!["CR350"]),
        _ => None,
    }
}

/// Port of `parser.py::_include_warning_drives`
pub fn _include_warning_drives(diags: &mut Diagnostics, profile: &Profile) {
    if profile.include_drive_prefix {
        return;
    }
    for d in &mut diags.items {
        if d.severity == "warning" {
            let new_msg = _MISSING_INCLUDE_DRIVE_RE.sub(r"\1", &d.message);
            *d = Rc::new(d.replace(|x| x.message = new_msg));
        }
    }
}

/// Port of `parser.py::_set_last_line`
pub fn _set_last_line(program: &mut Program, parser: &Parser, main_text: &str) {
    if parser.done && !program.statements.is_empty() {
        if let Some(last) = program.statements.last() {
            program.last_line = last.base().node.line;
        }
    } else {
        program.last_line = i64::try_from(pystr::splitlines(main_text).len())
            .unwrap_or(1)
            .max(1);
    }
}

/// Port of `parser.py::parse_text`
pub fn parse_text(
    text: &str,
    name: &str,
    directory: Option<&Path>,
    profile: Option<&'static Profile>,
) -> (ProgramRef, Diagnostics) {
    let prof = profile.unwrap_or_else(|| _profile_for(name));
    let (lines, diags) = source::load_text(
        text,
        name,
        directory,
        _include_own_ext(prof.name),
        _pulls_init(prof),
        _cr3xx_include_exts(prof.name).as_deref(),
    );
    let mut parser = Parser::new(prof, diags, true);
    let (mut program, mut diags) = parser.run(&lines, name);
    program.provenance.line_map = Some(parser.take_line_map());
    program
        .provenance
        .occurrences
        .clone_from(&diags.provenance.occurrences);
    _include_warning_drives(&mut diags, prof);
    _set_last_line(&mut program, &parser, text);
    (Rc::new(program), diags)
}
