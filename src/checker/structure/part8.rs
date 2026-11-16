//! Port of `checker/structure.py` (lines 5252-5848).

#![allow(
    dead_code,
    unused_variables,
    unused_imports,
    clippy::used_underscore_items,
    clippy::too_many_arguments,
    clippy::doc_markdown,
    clippy::similar_names
)]

use super::part7::{_illegal_dim_message, _split_top};
use super::part9::_alias_name_lead;
use crate::checker::expressions::part1::{
    _WHOLE_EXPR_ECHO_LOGGERS, _undefined_names, _variable_refs, KnownView,
};
use crate::checker::expressions::part4::_find_arity_problem;
use crate::checker::symbols::is_reserved_predefined_name;
use crate::consteval::{self, Env};
use crate::diagnostics::ShowFile;
use crate::syntax::{ConstValue, Node, NumberValue, StmtBase, rc_id};
use crate::{lexer, pyre, pystr};
use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;
use std::sync::LazyLock;

use crate::diagnostics::{DiagRef, Diagnostic, Diagnostics};
use crate::profiles::Profile;
use crate::syntax::{
    Assignment, Call, Case, ConstDecl, Declaration, Expr, ExprRef, Index, Instruction, Juxtaposed,
    Member, Number, OrderedMap, Program, ProgramRef, Stmt, StmtRef, SubDef, Unparsed, VarDecl,
};

/// Port of checker/structure.py::_group_end
pub fn _group_end(text: &str, open_at: i64) -> Option<i64> {
    let mut depth = 0;
    let mut in_str = false;
    for i in open_at..i64::try_from(pystr::len(text)).unwrap() {
        match pystr::char_at(text, i) {
            Some('"') => in_str = !in_str,
            Some('(') if !in_str => depth += 1,
            Some(')') if !in_str => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

/// Port of checker/structure.py::_group_items
#[allow(clippy::needless_continue, reason = "retain Python branch order")]
pub fn _group_items(text: &str, open_at: i64) -> Option<Vec<String>> {
    let close = match pystr::char_at(text, open_at)? {
        '(' => ')',
        '{' => '}',
        _ => return None,
    };
    let mut depth = 0;
    let mut in_str = false;
    for i in open_at..i64::try_from(pystr::len(text)).unwrap() {
        let ch = pystr::char_at(text, i)?;
        if ch == '"' {
            in_str = !in_str;
        } else if in_str {
            continue;
        } else if "({".contains(ch) {
            depth += 1;
        } else if ")}".contains(ch) {
            depth -= 1;
            if depth == 0 {
                return (ch == close)
                    .then(|| _split_top(pystr::slice(text, Some(open_at + 1), Some(i)), ","));
            }
        }
    }
    None
}

/// Port of checker/structure.py::_report_decl_expr
pub fn _report_decl_expr(
    expr: Option<&Expr>,
    echo: &str,
    known: &KnownView<'_>,
    variables: &BTreeSet<String>,
    profile: &Profile,
    stmt: &Stmt,
    diags: &mut Diagnostics,
    allow_variable: bool,
    tag: usize,
    indexed_constants: &BTreeSet<String>,
) -> bool {
    let indexed = !indexed_constants.is_empty()
        && _find_arity_problem(expr, profile, None, indexed_constants).as_deref()
            == Some("indexed_const");
    if indexed
        && _find_arity_problem(expr, profile, Some(known.names), indexed_constants).as_deref()
            == Some("indexed_const")
    {
        diags.error_basic(
            stmt.line(),
            &format!("Incomplete expression: {echo}."),
            stmt.file(),
        );
        return true;
    }
    let names = _undefined_names(expr, known);
    if !names.is_empty() {
        if _WHOLE_EXPR_ECHO_LOGGERS.contains(&profile.name) {
            diags.error_basic(
                stmt.line(),
                &format!("Invalid symbol in expression: {echo}."),
                stmt.file(),
            );
        } else {
            for name in names {
                diags.error_basic(
                    stmt.line(),
                    &match &name {
                        crate::checker::expressions::part1::UndefinedName::Plain(name) => {
                            profile.unresolved_symbol_message.format(name)
                        }
                        crate::checker::expressions::part1::UndefinedName::Dotted(name) => {
                            profile.unresolved_symbol_message.format_dotted(name)
                        }
                    },
                    stmt.file(),
                );
            }
            if indexed {
                diags.error_basic(
                    stmt.line(),
                    &format!("Incomplete expression: {echo}."),
                    stmt.file(),
                );
            }
            diags.error(
                stmt.line(),
                &format!("Variable illegal in Parameter: {echo}."),
                stmt.file(),
                ShowFile::Bool(true),
                tag,
                "",
                "",
            );
        }
        return true;
    }
    if !allow_variable && !_variable_refs(expr, variables).is_empty() {
        diags.error(
            stmt.line(),
            &format!("Variable illegal in Parameter: {echo}."),
            stmt.file(),
            ShowFile::Bool(true),
            tag,
            "",
            "",
        );
        return true;
    }
    false
}

/// Port of checker/structure.py::_report_juxtaposed_dim
pub fn _report_juxtaposed_dim(
    expr: &Juxtaposed,
    echo: &str,
    known: &KnownView<'_>,
    variables: &BTreeSet<String>,
    profile: &Profile,
    stmt: &Stmt,
    diags: &mut Diagnostics,
) -> bool {
    let expr = Expr::Juxtaposed(expr.clone());
    if _WHOLE_EXPR_ECHO_LOGGERS.contains(&profile.name) {
        if _undefined_names(Some(&expr), known).is_empty() {
            diags.error_basic(
                stmt.line(),
                &format!("Incomplete expression: {echo}."),
                stmt.file(),
            );
        }
        _report_decl_expr(
            Some(&expr),
            echo,
            known,
            variables,
            profile,
            stmt,
            diags,
            false,
            0,
            &BTreeSet::new(),
        );
        return true;
    }
    for name in _undefined_names(Some(&expr), known) {
        diags.error_basic(
            stmt.line(),
            &match &name {
                crate::checker::expressions::part1::UndefinedName::Plain(name) => {
                    profile.unresolved_symbol_message.format(name)
                }
                crate::checker::expressions::part1::UndefinedName::Dotted(name) => {
                    profile.unresolved_symbol_message.format_dotted(name)
                }
            },
            stmt.file(),
        );
    }
    diags.error_basic(
        stmt.line(),
        &format!("Incomplete expression: {echo}."),
        stmt.file(),
    );
    if !_undefined_names(Some(&expr), known).is_empty()
        || !_variable_refs(Some(&expr), variables).is_empty()
    {
        diags.error_basic(
            stmt.line(),
            &format!("Variable illegal in Parameter: {echo}."),
            stmt.file(),
        );
    }
    true
}

const _VARIABLE_TYPES: [&str; 6] = ["FLOAT", "LONG", "BOOLEAN", "STRING", "DOUBLE", "UINT1"];
static _EMPTY_DIMS_AS_RE: LazyLock<pyre::Regex> =
    LazyLock::new(|| pyre::Regex::new(r"(?i)\s*\(\s*\)(\s*As)\b", 0));
static _AS_TYPE_RE: LazyLock<pyre::Regex> =
    LazyLock::new(|| pyre::Regex::new(r"(?i)\bAs\s+([A-Za-z_][A-Za-z0-9_]*)", 0));
static _STRING_SIZE_RE: LazyLock<pyre::Regex> =
    LazyLock::new(|| pyre::Regex::new(r"(?i)\bAs\s+String\s*\* ?([^=]*)", pyre::S));
static _EMPTY_DIMS_INIT_RE: LazyLock<pyre::Regex> =
    LazyLock::new(|| pyre::Regex::new(r"\s*\(\s*\)(\s*)=", 0));
static _TYPED_DIMS_RE: LazyLock<pyre::Regex> =
    LazyLock::new(|| pyre::Regex::new(r"(?i)\s*As\s+[A-Za-z_][A-Za-z0-9_]*!?\s*(?=\()", 0));
static JUNK_RE: LazyLock<pyre::Regex> =
    LazyLock::new(|| pyre::Regex::new(r#"\s+(?:[\w.\"]+\s*)+?(?=\()"#, 0));
static AS_RE: LazyLock<pyre::Regex> = LazyLock::new(|| pyre::Regex::new(r"(?i)\s*as\b", 0));
static BLANK_RE: LazyLock<pyre::Regex> = LazyLock::new(|| pyre::Regex::new(r"\s*\(\s*\)(\s+)", 0));

/// Port of checker/structure.py::_check_member_types
pub fn _check_member_types(
    stmt: &Declaration,
    structs_seen: &BTreeSet<String>,
    profile: &Profile,
    diags: &mut Diagnostics,
) {
    let segments = _split_top(&stmt.base.text, ",");
    if segments.len() != stmt.vars.borrow().len() {
        return;
    }
    for (v, seg) in stmt.vars.borrow().iter().zip(segments) {
        if let Some(ty) = &v.type_
            && !ty.is_empty()
            && !_VARIABLE_TYPES.contains(&ty.as_str())
            && !structs_seen.contains(ty)
            && let Some(m) = _AS_TYPE_RE.search(&seg)
            && pystr::upper(m.group(1).unwrap()) == *ty
        {
            diags.error_basic(
                stmt.base.node.line,
                &format!("Variable Type {} not supported", m.group(1).unwrap()),
                &stmt.base.node.file,
            );
            if profile.single_pass {
                return;
            }
        }
    }
}

/// Port of checker/structure.py::_check_header_types
pub fn _check_header_types(
    text: &str,
    stmt: &SubDef,
    structs_seen: &BTreeSet<String>,
    profile: &Profile,
    diags: &mut Diagnostics,
) -> bool {
    let mut reported = false;
    for m in _AS_TYPE_RE.finditer(text) {
        let name = m.group(1).unwrap();
        if !_VARIABLE_TYPES.contains(&pystr::upper(name).as_str())
            && !structs_seen.contains(&pystr::upper(name))
        {
            diags.error_basic(
                stmt.base.node.line,
                &format!("Variable Type {name} not supported"),
                &stmt.base.node.file,
            );
            reported = true;
            if profile.single_pass {
                break;
            }
        }
    }
    reported
}

/// Port of checker/structure.py::_check_init_bounds
pub fn _check_init_bounds(stmt: &Declaration, env: &Env, diags: &mut Diagnostics) {
    for v in stmt.vars.borrow().iter() {
        let Some(init) = &v.init else {
            continue;
        };
        if init.is_empty() {
            continue;
        }
        let mut size = 1_i64;
        for d in &v.dims {
            let Ok(n) = consteval::value_of(Some(d), env) else {
                continue;
            };
            let n = match n {
                ConstValue::Int(n) => Some(n),
                ConstValue::BigInt(n) => Some(pystr::parse_int(&n).unwrap_or_else(|| {
                    if n.starts_with('-') {
                        i64::MIN
                    } else {
                        i64::MAX
                    }
                })),
                ConstValue::Float(f) | ConstValue::CFloat(f) => {
                    assert!(!f.is_nan(), "cannot convert float NaN to integer");
                    assert!(!f.is_infinite(), "cannot convert float infinity to integer");
                    pystr::py_int_from_f64(f).or_else(|| {
                        f.is_finite().then_some(if f.is_sign_negative() {
                            i64::MIN
                        } else {
                            i64::MAX
                        })
                    })
                }
                ConstValue::Str(_) => None,
            };
            if let Some(n) = n
                && n >= 1
            {
                size = size.saturating_mul(n);
            }
        }
        if i64::try_from(init.len()).unwrap() > size {
            diags.warning_basic(
                stmt.base.node.line,
                "Attempt to initialize variable out of bounds.",
                &stmt.base.node.file,
            );
        }
    }
}

/// Port of checker/structure.py::_adjacent_literals
pub fn _adjacent_literals(text: &str) -> bool {
    let Ok(tokens) = lexer::tokenize(text, 0) else {
        return false;
    };
    let kinds: Vec<_> = tokens
        .iter()
        .filter(|t| t.kind != "EOF")
        .map(|t| t.kind.as_str())
        .collect();
    kinds
        .windows(2)
        .any(|pair| pair[0] == "NUMBER" && matches!(pair[1], "NUMBER" | "STRING"))
}

/// Port of checker/structure.py::_check_decl_exprs
pub fn _check_decl_exprs(
    stmt: &Declaration,
    known: &KnownView<'_>,
    variables: &BTreeSet<String>,
    profile: &Profile,
    diags: &mut Diagnostics,
    struct_types: &BTreeSet<String>,
    stop_at: Option<i64>,
    length_scope: &Option<(BTreeSet<String>, BTreeSet<String>)>,
    indexed_constants: &BTreeSet<String>,
) -> Option<i64> {
    let mut body = pystr::lstrip(&stmt.base.text);
    if stmt.kind != "STRUCTUREMEMBER" {
        let first = body
            .chars()
            .take_while(|&c| !pystr::is_space_char(c))
            .count();
        body = pystr::slice(body, Some(i64::try_from(first).unwrap()), None);
    }
    let mut segments = _split_top(body, ",");
    while segments.len() > stmt.vars.borrow().len()
        && segments.last().is_some_and(|s| pystr::strip(s).is_empty())
    {
        segments.pop();
    }
    if segments.len() != stmt.vars.borrow().len() {
        return None;
    }
    let mut unsupported = false;
    let mut empty_eq = false;
    let mut unsupported_at: Option<usize> = None;
    let statement = Stmt::Declaration(stmt.clone());
    for (index, (v, seg)) in stmt.vars.borrow().iter().zip(&segments).enumerate() {
        if unsupported_at.is_some() {
            break;
        }
        let mut length_bad = false;
        let Some(name_at) = pystr::find(&pystr::upper(seg), &pystr::upper(&v.name), None, None)
        else {
            continue;
        };
        let name_end = name_at + pystr::len(&v.name);
        let mut after_name = pystr::slice(seg, Some(i64::try_from(name_end).unwrap()), None);
        if let Some(junk) = JUNK_RE.match_(after_name)
            && AS_RE.match_(after_name).is_none()
        {
            after_name = pystr::slice(after_name, Some(i64::try_from(junk.end0()).unwrap()), None);
        }
        if index == stmt.vars.borrow().len() - 1
            && !profile.single_pass
            && let Some(blank) = BLANK_RE.fullmatch(after_name)
        {
            diags.error_basic(
                stmt.base.node.line,
                &format!(
                    "Invalid, or out of place expression: {}.",
                    blank.group(1).unwrap()
                ),
                &stmt.base.node.file,
            );
            continue;
        }
        if let Some(empty_as) = _EMPTY_DIMS_AS_RE.match_(after_name) {
            let echo = empty_as.group(1).unwrap();
            if _WHOLE_EXPR_ECHO_LOGGERS.contains(&profile.name) {
                diags.error_basic(
                    stmt.base.node.line,
                    &format!("Invalid symbol in expression: {echo}."),
                    &stmt.base.node.file,
                );
            } else {
                diags.error_basic(
                    stmt.base.node.line,
                    &profile.unresolved_symbol_message.format(pystr::strip(echo)),
                    &stmt.base.node.file,
                );
                diags.error_basic(
                    stmt.base.node.line,
                    &format!("Variable illegal in Parameter: {echo}."),
                    &stmt.base.node.file,
                );
                if let Some(next) = stmt.vars.borrow().get(index + 2) {
                    diags.error_basic(
                        stmt.base.node.line,
                        &format!("Too many parameters:  {}.", next.name),
                        &stmt.base.node.file,
                    );
                }
                return Some(i64::try_from(index).unwrap());
            }
            continue;
        }
        if matches!(stmt.kind.as_str(), "PUBLIC" | "DIM")
            && let Some(ty) = &v.type_
            && !ty.is_empty()
            && !_VARIABLE_TYPES.contains(&ty.as_str())
            && !struct_types.contains(ty)
            && let Some(m) = _AS_TYPE_RE.search(seg)
            && pystr::upper(m.group(1).unwrap()) == *ty
            && !(profile.single_pass && unsupported)
        {
            diags.error_basic(
                stmt.base.node.line,
                &format!("Variable Type {} not supported", m.group(1).unwrap()),
                &stmt.base.node.file,
            );
            unsupported = true;
            if profile.single_pass {
                unsupported_at = Some(index);
            }
        }
        if v.empty_type_init {
            diags.error_basic(
                stmt.base.node.line,
                "Invalid, or out of place expression:  =.",
                &stmt.base.node.file,
            );
        }
        let size_match = _STRING_SIZE_RE.search(seg);
        if v.type_.as_deref() == Some("STRING")
            && v.string_size.is_some()
            && size_match
                .as_ref()
                .is_some_and(|m| _adjacent_literals(m.group(1).unwrap()))
        {
            diags.error_basic(
                stmt.base.node.line,
                &format!(
                    "Incomplete expression: {}.",
                    size_match.as_ref().unwrap().group(1).unwrap()
                ),
                &stmt.base.node.file,
            );
            if profile.single_pass {
                if let Some(next) = stmt.vars.borrow().get(index + 2) {
                    diags.error_basic(
                        stmt.base.node.line,
                        &format!("Too many parameters:  {}.", next.name),
                        &stmt.base.node.file,
                    );
                }
                return Some(i64::try_from(index).unwrap());
            }
        } else if v.type_.as_deref() == Some("STRING")
            && v.string_size.is_some()
            && let Some((length_known, length_variables)) = length_scope
            && stop_at.is_none_or(|stop| i64::try_from(index).unwrap() < stop)
            && let Some(m) = size_match
        {
            length_bad = _report_decl_expr(
                v.string_size.as_deref(),
                m.group(1).unwrap(),
                &KnownView::from_set(length_known),
                length_variables,
                profile,
                &statement,
                diags,
                false,
                rc_id(v),
                &BTreeSet::new(),
            );
        }
        let mut rest_at = name_end;
        let mut rest = pystr::slice(seg, Some(i64::try_from(rest_at).unwrap()), None);
        if let Some(junk) = JUNK_RE.match_(rest)
            && AS_RE.match_(rest).is_none()
        {
            rest_at += junk.end0();
            rest = pystr::slice(seg, Some(i64::try_from(rest_at).unwrap()), None);
        }
        if let Some(m) = _EMPTY_DIMS_INIT_RE.match_(rest)
            && !(profile.single_pass && empty_eq)
        {
            diags.error_basic(
                stmt.base.node.line,
                &format!(
                    "Invalid, or out of place expression: {}=.",
                    m.group(1).unwrap()
                ),
                &stmt.base.node.file,
            );
            empty_eq = true;
        }
        let typed_dims = _TYPED_DIMS_RE.match_(rest);
        let dims = if v.junk_dims.is_empty() {
            &v.dims
        } else {
            &v.junk_dims
        };
        if !dims.is_empty() && (pystr::lstrip(rest).starts_with('(') || typed_dims.is_some()) {
            let open_at = rest_at
                + typed_dims.as_ref().map_or_else(
                    || pystr::len(rest) - pystr::len(pystr::lstrip(rest)),
                    pyre::Match::end0,
                );
            let items = _group_items(seg, i64::try_from(open_at).unwrap());
            if let Some(items) = items
                && items.len() == dims.len()
                && stop_at.is_none_or(|stop| i64::try_from(index).unwrap() < stop)
            {
                let mut bad_dim = false;
                for (d, text) in dims.iter().zip(items) {
                    bad_dim |= if let Expr::Juxtaposed(d) = d.as_ref() {
                        _report_juxtaposed_dim(
                            d,
                            pystr::lstrip(&text),
                            known,
                            variables,
                            profile,
                            &statement,
                            diags,
                        )
                    } else {
                        _report_decl_expr(
                            Some(d),
                            pystr::lstrip(&text),
                            known,
                            variables,
                            profile,
                            &statement,
                            diags,
                            false,
                            0,
                            indexed_constants,
                        )
                    };
                }
                if bad_dim && profile.single_pass {
                    if let Some(next) = stmt.vars.borrow().get(index + 2) {
                        diags.error_basic(
                            stmt.base.node.line,
                            &format!("Too many parameters:  {}.", next.name),
                            &stmt.base.node.file,
                        );
                    }
                    return Some(i64::try_from(index).unwrap());
                }
            }
        }
        if length_bad && profile.single_pass {
            if let Some(next) = stmt.vars.borrow().get(index + 2) {
                diags.error_basic(
                    stmt.base.node.line,
                    &format!("Too many parameters:  {}.", next.name),
                    &stmt.base.node.file,
                );
            }
            return Some(i64::try_from(index).unwrap());
        }
        if let Some(init) = v.init.as_ref().filter(|init| !init.is_empty())
            && stop_at.is_none_or(|stop| i64::try_from(index).unwrap() < stop)
        {
            let eq = _split_top(seg, "=");
            if eq.len() != 2 {
                continue;
            }
            let init_text = &eq[1];
            let stripped = pystr::lstrip(init_text);
            if _adjacent_literals(init_text) && !stripped.starts_with('{') {
                diags.error_basic(
                    stmt.base.node.line,
                    &format!("Incomplete expression: {}.", pystr::strip(init_text)),
                    &stmt.base.node.file,
                );
                diags.error_basic(
                    stmt.base.node.line,
                    &format!("Variable initialization of {} has bad syntax.", v.name),
                    &stmt.base.node.file,
                );
                if profile.single_pass {
                    return Some(i64::try_from(index).unwrap());
                }
                continue;
            }
            let items = if stripped.starts_with('{') {
                _group_items(
                    init_text,
                    i64::try_from(pystr::len(init_text) - pystr::len(stripped)).unwrap(),
                )
            } else {
                Some(vec![init_text.clone()])
            };
            let Some(items) = items.filter(|items| items.len() == init.len()) else {
                continue;
            };
            let mut bad = false;
            let mut live = variables.clone();
            live.extend(["STATUS".to_owned(), "SETTINGS".to_owned()]);
            for (e, text) in init.iter().zip(items) {
                bad |= _report_decl_expr(
                    e.as_deref(),
                    pystr::lstrip(&text),
                    known,
                    &live,
                    profile,
                    &statement,
                    diags,
                    profile.single_pass,
                    0,
                    indexed_constants,
                );
            }
            if bad {
                diags.error_basic(
                    stmt.base.node.line,
                    &format!("Variable initialization of {} has bad syntax.", v.name),
                    &stmt.base.node.file,
                );
                if profile.single_pass {
                    if let Some(next) = stmt.vars.borrow().get(index + 2) {
                        diags.error_basic(
                            stmt.base.node.line,
                            &format!("Too many parameters:  {}.", next.name),
                            &stmt.base.node.file,
                        );
                    }
                    return Some(i64::try_from(index).unwrap());
                }
            }
        }
    }
    if let Some(index) = unsupported_at {
        if let Some(next) = stmt.vars.borrow().get(index + 2) {
            diags.error_basic(
                stmt.base.node.line,
                &format!("Too many parameters:  {}.", next.name),
                &stmt.base.node.file,
            );
        }
        return Some(i64::try_from(index).unwrap());
    }
    None
}

/// Port of checker/structure.py::_check_alias_index_names
pub fn _check_alias_index_names(
    stmt: &Instruction,
    known: &KnownView<'_>,
    variables: &BTreeSet<String>,
    profile: &Profile,
    diags: &mut Diagnostics,
    check_variables: bool,
    indexed_constants: &BTreeSet<String>,
) -> bool {
    let Some(Expr::Index(target)) = stmt.args.first().and_then(Option::as_deref) else {
        return false;
    };
    if !variables.contains(&pystr::upper(&target.name)) {
        return false;
    }
    if target
        .args
        .iter()
        .any(|a| matches!(a.as_deref(), Some(Expr::String(_))))
    {
        diags.error_basic(
            stmt.base.node.line,
            "Illegally dimensioned aliased name",
            &stmt.base.node.file,
        );
        return true;
    }
    let items = pystr::find(&stmt.base.text, "(", None, None)
        .and_then(|open| _group_items(&stmt.base.text, i64::try_from(open).unwrap()));
    let Some(items) = items.filter(|items| items.len() == target.args.len()) else {
        return false;
    };
    for text in &items {
        let item = pystr::strip(text);
        if item.starts_with('(') && item.ends_with(')') {
            diags.error_basic(
                stmt.base.node.line,
                &format!(
                    "No Matching, or out of place ) in expression: {}.",
                    pystr::slice(item, None, Some(-1))
                ),
                &stmt.base.node.file,
            );
            diags.error_basic(
                stmt.base.node.line,
                "Alias is illegally dimensioned.",
                &stmt.base.node.file,
            );
            return true;
        }
    }
    let mut bad = false;
    let statement = Stmt::Instruction(stmt.clone());
    for (a, text) in target.args.iter().zip(items) {
        if !indexed_constants.is_empty()
            && _find_arity_problem(a.as_deref(), profile, Some(known.names), indexed_constants)
                .as_deref()
                == Some("indexed_const")
        {
            diags.error_basic(
                stmt.base.node.line,
                &format!(
                    "Incomplete expression: {}.",
                    text.split(')').next().unwrap()
                ),
                &stmt.base.node.file,
            );
            diags.error_basic(
                stmt.base.node.line,
                "Alias is illegally dimensioned.",
                &stmt.base.node.file,
            );
            return true;
        }
        if a.is_some()
            && (!_undefined_names(a.as_deref(), known).is_empty()
                || (check_variables && !_variable_refs(a.as_deref(), variables).is_empty()))
        {
            bad |= _report_decl_expr(
                a.as_deref(),
                &text,
                known,
                variables,
                profile,
                &statement,
                diags,
                false,
                0,
                &BTreeSet::new(),
            );
        }
    }
    if bad {
        diags.error_basic(
            stmt.base.node.line,
            "Alias is illegally dimensioned.",
            &stmt.base.node.file,
        );
    }
    bad
}

/// Port of checker/structure.py::_alias_flat_offset
#[allow(
    clippy::needless_pass_by_value,
    reason = "preserve existing public signature"
)]
pub fn _alias_flat_offset(index: Vec<i64>, dims: Vec<i64>) -> i64 {
    let mut flat = *index.last().expect("nonempty Alias index");
    let mut mult = 1;
    for k in (0..dims.len().saturating_sub(1)).rev() {
        mult *= dims[k + 1];
        flat += (index[k] - 1) * mult;
    }
    flat
}

/// Port of checker/structure.py::_alias_string_slots
pub fn _alias_string_slots(
    program: &Program,
    env: &Env,
) -> OrderedMap<String, (Option<i64>, usize)> {
    let mut out = OrderedMap::new();
    for stmt in &program.statements {
        let Stmt::Declaration(stmt) = stmt.as_ref() else {
            continue;
        };
        if !matches!(stmt.kind.as_str(), "PUBLIC" | "DIM" | "STATIC") {
            continue;
        }
        for v in stmt.vars.borrow().iter() {
            out.remove(&pystr::upper(&v.name));
            if pystr::upper(v.type_.as_deref().unwrap_or("")) != "STRING"
                || v.paren_dim
                || v.dims.len() > 2
            {
                continue;
            }
            let size = if v.string_size.is_none() {
                Some(ConstValue::Int(23))
            } else {
                consteval::value_of(v.string_size.as_deref(), env).ok()
            };
            let words = size
                .as_ref()
                .filter(|size| !matches!(size, ConstValue::Str(_)))
                .map(|size| -pystr::floordiv_i64(-4.max(consteval::to_long(size) + 1), 4).unwrap());
            out.insert(pystr::upper(&v.name), (words, v.dims.len()));
        }
    }
    out
}

/// Port of checker/structure.py::_alias_parameter_declaration
pub fn _alias_parameter_declaration(
    routine: &SubDef,
    env: &Env,
    declared: &OrderedMap<String, String>,
    known: &BTreeSet<String>,
    forward: &BTreeSet<String>,
    profile: &Profile,
    mut diags: Option<&mut Diagnostics>,
    structs_seen: &BTreeSet<String>,
) -> (Declaration, bool) {
    let mut consts = OrderedMap::new();
    for (name, value) in env.consts.borrow().iter() {
        if declared
            .get(&pystr::upper(name))
            .is_some_and(|kind| kind == "CONST")
        {
            consts.insert(name.clone(), value.clone());
        }
    }
    let mut visible = Env::with_consts(env.profile, Rc::new(RefCell::new(consts)));
    visible.section.clone_from(&env.section);
    let mut header_known: BTreeSet<String> = known.difference(forward).cloned().collect();
    let mut variables: BTreeSet<String> = declared
        .iter()
        .filter(|(_, kind)| *kind == "VAR")
        .map(|(name, _)| name.clone())
        .collect();
    let segments = pystr::find(&routine.base.text, "(", None, None)
        .and_then(|open| _group_items(&routine.base.text, i64::try_from(open).unwrap()));
    let split = segments
        .as_ref()
        .is_some_and(|segments| segments.len() == routine.params.len());
    if !split && let Some(diags) = diags.as_deref_mut() {
        _check_header_types(&routine.base.text, routine, structs_seen, profile, diags);
    }
    let one = Rc::new(Expr::Number(Number {
        node: Node::new(routine.base.node.line, ""),
        value: NumberValue::Int(1),
        text: "1".to_owned(),
    }));
    let mut view = Vec::new();
    let mut failed = false;
    let mut broke = false;
    let statement = Stmt::SubDef(routine.clone());
    for (index, param) in routine.params.iter().enumerate() {
        let segment = if split {
            segments
                .as_ref()
                .and_then(|segments| segments.get(index))
                .map(String::as_str)
        } else {
            None
        };
        let texts = segment
            .and_then(|segment| {
                pystr::find(segment, "(", None, None)
                    .and_then(|open| _group_items(segment, i64::try_from(open).unwrap()))
            })
            .filter(|texts| texts.len() == param.dim_exprs.len());
        let mut stop =
            if let (Some(diags), true, Some(segment)) = (diags.as_deref_mut(), split, segment) {
                _check_header_types(segment, routine, structs_seen, profile, diags)
            } else {
                false
            };
        let mut dims = Some(Vec::new());
        let mut bad = false;
        if param.dim_exprs.len() > 3
            && let (Some(diags), Some(texts)) = (diags.as_deref_mut(), texts.as_ref())
        {
            stop = true;
            diags.error_basic(
                routine.base.node.line,
                &format!(
                    "Variable {}({} dimension exceeds max of 3.",
                    param.name,
                    pystr::lstrip(&texts[0])
                ),
                &routine.base.node.file,
            );
        }
        if param.dim_exprs.len() <= 3 {
            for (pos, d) in param.dim_exprs.iter().enumerate() {
                if let (Expr::Juxtaposed(d), Some(diags)) = (d.as_ref(), diags.as_deref_mut()) {
                    let echo = if let Some(texts) = &texts {
                        pystr::lstrip(&texts[pos]).to_owned()
                    } else {
                        d.items
                            .iter()
                            .map(|i| {
                                i.as_deref().map_or("", |i| {
                                    if i.text().is_empty() {
                                        i.name()
                                    } else {
                                        i.text()
                                    }
                                })
                            })
                            .collect::<Vec<_>>()
                            .join(" ")
                    };
                    _report_juxtaposed_dim(
                        d,
                        &echo,
                        &KnownView::from_set(&header_known),
                        &variables,
                        profile,
                        &statement,
                        diags,
                    );
                    dims.as_mut().unwrap().push(Rc::clone(&one));
                    bad = true;
                    continue;
                }
                let size =
                    consteval::value_of(Some(d), &visible)
                        .ok()
                        .and_then(|value| match value {
                            ConstValue::Int(n) => Some(n),
                            ConstValue::BigInt(n) => {
                                Some(pystr::parse_int(&n).unwrap_or_else(|| {
                                    if n.starts_with('-') {
                                        i64::MIN
                                    } else {
                                        i64::MAX
                                    }
                                }))
                            }
                            ConstValue::Float(f) | ConstValue::CFloat(f) => {
                                pystr::py_int_from_f64(f).or_else(|| {
                                    f.is_finite().then_some(if f.is_sign_negative() {
                                        i64::MIN
                                    } else {
                                        i64::MAX
                                    })
                                })
                            }
                            ConstValue::Str(_) => None,
                        });
                if size.is_some_and(|size| size < 1) {
                    if let Some(diags) = diags.as_deref_mut() {
                        diags.error_basic(
                            routine.base.node.line,
                            &_illegal_dim_message(&statement, None),
                            &routine.base.node.file,
                        );
                    }
                    dims.as_mut().unwrap().push(Rc::clone(&one));
                    bad = true;
                } else if size.is_some_and(|size| size <= 2_147_483_647) {
                    dims.as_mut().unwrap().push(Rc::clone(d));
                } else {
                    let reported = if let (true, Some(diags), Some(texts)) =
                        (size.is_none(), diags.as_deref_mut(), texts.as_ref())
                    {
                        _report_decl_expr(
                            Some(d),
                            pystr::lstrip(&texts[pos]),
                            &KnownView::from_set(&header_known),
                            &variables,
                            profile,
                            &statement,
                            diags,
                            false,
                            0,
                            &BTreeSet::new(),
                        )
                    } else {
                        false
                    };
                    if reported {
                        dims.as_mut().unwrap().push(Rc::clone(&one));
                        bad = true;
                    } else {
                        dims = None;
                        break;
                    }
                }
            }
        }
        if dims.is_some()
            && param.string_size.is_some()
            && consteval::value_of(param.string_size.as_deref(), &visible).is_err()
            && let Some(size_text) = _STRING_SIZE_RE.search(segment.unwrap_or(""))
            && let Some(diags) = diags.as_deref_mut()
        {
            _report_decl_expr(
                param.string_size.as_deref(),
                pystr::strip(size_text.group(1).unwrap()),
                &KnownView::from_set(&header_known),
                &variables,
                profile,
                &statement,
                diags,
                false,
                rc_id(param),
                &BTreeSet::new(),
            );
        }
        if let Some(dims) = dims
            && ["FLOAT", "LONG", "BOOLEAN", "DOUBLE", "STRING"].contains(
                &pystr::upper(
                    param
                        .type_
                        .as_deref()
                        .filter(|s| !s.is_empty())
                        .unwrap_or("FLOAT"),
                )
                .as_str(),
            )
        {
            view.push(Rc::new(VarDecl {
                name: param.name.clone(),
                dims,
                type_: param.type_.clone(),
                string_size: param.string_size.clone(),
                ..VarDecl::default()
            }));
        }
        header_known.insert(pystr::upper(&param.name));
        variables.insert(pystr::upper(&param.name));
        failed |= bad;
        if (bad || stop) && profile.single_pass {
            if let Some(diags) = diags.as_deref_mut()
                && let Some(next) = routine.params.get(index + 2)
            {
                diags.error_basic(
                    routine.base.node.line,
                    &format!("Too many parameters:  {}.", next.name),
                    &routine.base.node.file,
                );
            }
            broke = true;
            break;
        }
    }
    if !broke
        && split
        && let Some(diags) = diags
    {
        let after = routine
            .base
            .text
            .chars()
            .collect::<Vec<_>>()
            .iter()
            .rposition(|&c| c == ')');
        let after = after.map_or(0, |at| at + 1);
        _check_header_types(
            pystr::slice(
                &routine.base.text,
                Some(i64::try_from(after).unwrap()),
                None,
            ),
            routine,
            structs_seen,
            profile,
            diags,
        );
    }
    (
        Declaration {
            base: StmtBase::new(routine.base.node.line, &routine.base.node.file),
            kind: "DIM".to_owned(),
            vars: view.into(),
        },
        failed,
    )
}

/// Port of checker/structure.py::_alias_name_collisions
pub fn _alias_name_collisions(
    stmt: &Instruction,
    declared: &OrderedMap<String, String>,
    profile: Option<&Profile>,
    in_routine: bool,
) -> OrderedMap<usize, String> {
    let mut out = OrderedMap::new();
    let lead = _alias_name_lead(stmt);
    let junk = stmt.alias_item_junk.as_deref().unwrap_or(&[]);
    for (k, a) in stmt.args.iter().skip(1).enumerate() {
        let Some(Expr::Name(a)) = a.as_deref() else {
            continue;
        };
        if (k == 0 && stmt.alias_junk)
            || (k != 0
                && (junk.get(k).copied().unwrap_or(false)
                    || k >= lead.len()
                    || !lead[k].0.is_empty()))
        {
            continue;
        }
        let prior = declared.get(&pystr::upper(&a.name)).map(String::as_str);
        if prior == Some("VAR") || (in_routine && prior == Some("ALIAS")) {
            out.insert(k, format!("{} already declared as a variable.", a.name));
        } else if prior == Some("CONST") {
            out.insert(k, format!("{} is already declared as a CONST.", a.name));
        } else if k == 0
            && profile.is_some_and(|profile| is_reserved_predefined_name(&a.name, profile))
        {
            out.insert(k, format!("{} is a predefined CONST.", a.name));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grouping_literals_and_offsets_match_python() {
        assert_eq!(_group_end("é(\"x)\",(2))z", 1), Some(10));
        assert_eq!(_group_end("(1", 0), None);
        assert_eq!(_group_end("((2))", 0), Some(4));
        assert_eq!(_group_items("{1)", 0), None);
        assert_eq!(_group_items("(1", 0), None);
        let inputs = ["1  0", "1 \"a\"", "\"a\" 1", "1 + 2", "@\""];
        assert_eq!(
            inputs.map(_adjacent_literals),
            [true, true, false, false, false]
        );
        assert_eq!(_alias_flat_offset(vec![2, 3, 4], vec![2, 3, 4]), 24);
        assert_eq!(_alias_flat_offset(vec![1, 0], vec![2, 3]), 0);
    }

    #[test]
    fn declaration_diagnostics_match_python() {
        for logger in ["cr6", "cr300"] {
            let profile = crate::profiles::get(logger).unwrap();
            let mut diags = Diagnostics::new();
            let stmt = Stmt::Declaration(Declaration {
                base: StmtBase::new(7, "é.crb"),
                kind: "DIM".to_owned(),
                vars: Vec::new().into(),
            });
            let expr = Expr::Name(crate::syntax::Name {
                node: Node::new(7, ""),
                name: "Nope".to_owned(),
            });
            assert!(_report_decl_expr(
                Some(&expr),
                " Nope ",
                &KnownView::from_set(&BTreeSet::new()),
                &BTreeSet::new(),
                profile,
                &stmt,
                &mut diags,
                false,
                0,
                &BTreeSet::new()
            ));
            let expected = if logger == "cr6" {
                vec!["Invalid symbol in expression:  Nope ."]
            } else {
                vec![
                    "Nope is not defined.",
                    "Variable illegal in Parameter:  Nope .",
                ]
            };
            assert_eq!(
                diags
                    .items
                    .iter()
                    .map(|d| d.message.as_str())
                    .collect::<Vec<_>>(),
                expected
            );
            assert!(diags.items.iter().all(|d| d.line == 7 && d.file == "é.crb"));
            let header = SubDef {
                base: StmtBase::new(8, ""),
                ..SubDef::default()
            };
            let mut diags = Diagnostics::new();
            assert!(_check_header_types(
                "Sub F As UInt4 As Floatx",
                &header,
                &BTreeSet::new(),
                profile,
                &mut diags
            ));
            let expected = if logger == "cr6" {
                vec![
                    "Variable Type UInt4 not supported",
                    "Variable Type Floatx not supported",
                ]
            } else {
                vec!["Variable Type UInt4 not supported"]
            };
            assert_eq!(
                diags
                    .items
                    .iter()
                    .map(|d| d.message.as_str())
                    .collect::<Vec<_>>(),
                expected
            );
            let expr = crate::parser::partial_expression("K(1)", profile).unwrap();
            let constants = BTreeSet::from(["K".to_owned()]);
            let mut diags = Diagnostics::new();
            assert!(_report_decl_expr(
                Some(&expr),
                "K(1)",
                &KnownView::from_set(&constants),
                &BTreeSet::new(),
                profile,
                &stmt,
                &mut diags,
                false,
                0,
                &constants
            ));
            assert_eq!(diags.items.len(), 1);
            assert_eq!(diags.items[0].message, "Incomplete expression: K(1).");
        }
    }

    #[test]
    fn nonfinite_initializer_dimensions_raise_like_python() {
        let env = Env::new(crate::profiles::get("cr6").unwrap());
        for value in [f64::INFINITY, f64::NAN] {
            let number = Rc::new(Expr::Number(Number {
                value: NumberValue::Float(value),
                ..Number::default()
            }));
            let stmt = Declaration {
                vars: vec![Rc::new(VarDecl {
                    dims: vec![Rc::clone(&number)],
                    init: Some(vec![Some(number)]),
                    ..VarDecl::default()
                })]
                .into(),
                ..Declaration::default()
            };
            let raised = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                _check_init_bounds(&stmt, &env, &mut Diagnostics::new());
            }));
            assert!(raised.is_err());
        }
    }

    #[test]
    fn bounds_and_string_slots_match_python() {
        let profile = crate::profiles::get("cr6").unwrap();
        let env = Env::new(profile);
        for (value, warnings) in [
            (NumberValue::BigInt("100000000000000000000".to_owned()), 0),
            (NumberValue::Float(1e20), 0),
            (NumberValue::BigInt("-100000000000000000000".to_owned()), 1),
            (NumberValue::Float(-1e20), 1),
        ] {
            let number = Rc::new(Expr::Number(Number {
                value,
                ..Number::default()
            }));
            let stmt = Declaration {
                vars: vec![Rc::new(VarDecl {
                    name: "X".to_owned(),
                    dims: vec![Rc::clone(&number)],
                    init: Some(vec![Some(Rc::clone(&number)), Some(number)]),
                    ..VarDecl::default()
                })]
                .into(),
                ..Declaration::default()
            };
            let mut diags = Diagnostics::new();
            _check_init_bounds(&stmt, &env, &mut diags);
            assert_eq!(diags.items.len(), warnings);
        }
        let (program, _) = crate::parser::parse_text(
            "Public A(2) = {1,2,3}\nPublic B(3,-1) = {1,2,3,4}\n",
            "x.cr6",
            None,
            Some(profile),
        );
        let mut diags = Diagnostics::new();
        for stmt in &program.statements {
            if let Stmt::Declaration(stmt) = stmt.as_ref() {
                _check_init_bounds(stmt, &env, &mut diags);
            }
        }
        assert_eq!(diags.items.len(), 2);
        assert!(
            diags
                .items
                .iter()
                .all(|d| d.message == "Attempt to initialize variable out of bounds.")
        );
        let (program, _) = crate::parser::parse_text(
            "Public A As String\nPublic B As String * 0\nPublic C As String * 1e10\nPublic D As String * Nope\n",
            "x.cr6",
            None,
            Some(profile),
        );
        let slots = _alias_string_slots(&program, &env);
        assert_eq!(
            slots
                .iter()
                .map(|(name, value)| (name.as_str(), *value))
                .collect::<Vec<_>>(),
            vec![
                ("A", (Some(6), 0)),
                ("B", (Some(1), 0)),
                ("C", (Some(1), 0)),
                ("D", (None, 0))
            ]
        );
    }
}
