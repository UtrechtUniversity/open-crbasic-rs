//! Port of `checker/structure.py` (lines 3660-4451).

#![allow(dead_code, unused_variables, unused_imports, clippy::doc_markdown)]

use std::collections::BTreeSet;
use std::rc::Rc;

use super::CallLinks;
use crate::checker::{expressions, instr_rules};
use crate::lexer::Token;
use crate::profiles::Function;
use crate::{lexer, pyre, pystr};
use std::sync::LazyLock;

pub type ScanVerdict = Option<(String, String)>;
pub type ScanFunctions = OrderedMap<String, Function>;
pub type FunctionCallScan = (Vec<Token>, BTreeSet<String>, ScanFunctions, ScanVerdict);

use super::part5::{_BINARY_OPS, _BINARY_WORDS, _START_ERROR_WORDS};
use crate::checker::instr_rules::ZERO_PARAM_INSTRUCTIONS;

pub(crate) fn functions_for(profile: &Profile) -> ScanFunctions {
    let mut out = ScanFunctions::new();
    for (k, v) in profile.functions().iter() {
        out.insert(k.to_owned(), v.clone());
    }
    out
}
#[allow(clippy::unnecessary_wraps)]
fn verdict(stage: &str, kind: &str) -> ScanVerdict {
    Some((stage.to_owned(), kind.to_owned()))
}
fn is_verdict(v: &ScanVerdict, stage: &str, kind: &str) -> bool {
    v.as_ref().is_some_and(|(s, k)| s == stage && k == kind)
}
fn contains(operands: Option<&BTreeSet<String>>, name: &str) -> bool {
    operands.is_none_or(|s| s.contains(name))
}
fn unknown_name(t: &Token, operands: &BTreeSet<String>, functions: &ScanFunctions) -> bool {
    t.kind == "IDENT"
        && !operands.contains(&t.upper)
        && !functions.contains_key(&t.upper)
        && !_BINARY_WORDS.contains(&t.upper.as_str())
        && t.upper != "NOT"
}
fn adjacent(prev: &Token, tok: &Token) -> bool {
    prev.upper != "NOT"
        && (matches!(prev.kind.as_str(), "IDENT" | "NUMBER" | "STRING") || prev.text == ")")
        && (matches!(tok.kind.as_str(), "NUMBER" | "STRING")
            || tok.kind == "IDENT" && !_BINARY_WORDS.contains(&tok.upper.as_str()))
}
fn tokens(text: &str) -> Option<Vec<Token>> {
    Some(
        lexer::tokenize(text, 0)
            .ok()?
            .into_iter()
            .filter(|t| t.kind != "EOF")
            .collect(),
    )
}
fn scan(
    toks: &[Token],
    operands: Option<&BTreeSet<String>>,
    functions: &ScanFunctions,
) -> ScanVerdict {
    _scan_expr(
        toks, 0, operands, functions, true, false, None, false, true, false, false,
    )
}
fn col(n: usize) -> i64 {
    i64::try_from(n).expect("source column fits i64")
}

use crate::diagnostics::{DiagRef, Diagnostic, Diagnostics};
use crate::profiles::Profile;
use crate::syntax::{
    Assignment, Call, Case, ConstDecl, Declaration, Expr, ExprRef, Index, Instruction, Juxtaposed,
    Member, Number, OrderedMap, Program, ProgramRef, Stmt, StmtRef, SubDef, Unparsed, VarDecl,
};

/// Port of checker/structure.py::_function_call_scan
pub fn _function_call_scan(
    stmt: &StmtRef,
    program: &crate::syntax::ProgramRef,
    profile: &'static Profile,
) -> Option<FunctionCallScan> {
    let mut functions = ScanFunctions::new();
    for st in &program.statements {
        if profile.single_pass && Rc::ptr_eq(st, stmt) {
            break;
        }
        if let Stmt::SubDef(st) = st.as_ref()
            && pystr::upper(&st.keyword) == "FUNCTION"
        {
            let name = pystr::upper(&st.name);
            functions.insert(
                name.clone(),
                Function {
                    name,
                    code: 0,
                    kind: 0,
                    argspec: 0,
                    nargs: Some(i64::try_from(st.params.len()).expect("parameter count fits i64")),
                    argtypes: None,
                },
            );
        }
    }
    if functions.is_empty() {
        return None;
    }
    let toks = tokens(stmt.text())?;
    let start = if toks.len() > 2 && toks[0].kind == "IDENT" && toks[1].text == "=" {
        2
    } else {
        0
    };
    if toks.len() <= start + 1
        || toks[start].kind != "IDENT"
        || !functions.contains_key(&toks[start].upper)
        || toks[start + 1].text != "("
    {
        return None;
    }
    let operands = super::pass1_operands(program, profile);
    let mut merged = functions_for(profile);
    for (k, v) in functions.iter() {
        merged.insert(k.clone(), v.clone());
    }
    let v = scan(&toks, Some(&operands), &merged);
    Some((toks, operands, merged, v))
}

/// Port of checker/structure.py::_function_symbol_finding
pub fn _function_symbol_finding(
    stmt: &StmtRef,
    program: &crate::syntax::ProgramRef,
    profile: &'static Profile,
) -> Option<String> {
    if profile.single_pass {
        return None;
    }
    let (toks, operands, functions, v) = _function_call_scan(stmt, program, profile)?;
    if !is_verdict(&v, "pass2", "symbol") {
        return None;
    }
    let unknown = toks
        .iter()
        .position(|t| unknown_name(t, &operands, &functions))?;
    if unknown == 0 {
        return None;
    }
    Some(format!("Invalid symbol in expression: {}.", stmt.text()))
}

/// Port of checker/structure.py::_function_syntax_findings
pub fn _function_syntax_findings(
    stmt: &StmtRef,
    program: &crate::syntax::ProgramRef,
    profile: &'static Profile,
) -> Vec<String> {
    let Some((toks, operands, functions, v)) = _function_call_scan(stmt, program, profile) else {
        return vec![];
    };
    if is_verdict(&v, "pass1", "expects_comma") && !profile.single_pass {
        let mut depth = 0;
        let mut own = None;
        for pair in toks.windows(2) {
            let (prev, tok) = (&pair[0], &pair[1]);
            if prev.kind == "OP" && matches!(prev.text.as_str(), "(" | ")") {
                depth += if prev.text == "(" { 1 } else { -1 };
            }
            if adjacent(prev, tok) {
                own = Some(depth == 1);
                break;
            }
        }
        match own {
            None => return vec![],
            Some(false) => return vec![format!("Expression expects , : {}.", stmt.text())],
            Some(true) => {}
        }
    } else if is_verdict(&v, "pass1", "incomplete") && !profile.single_pass {
        let (mut depth, mut _before, mut after, mut found) = (0, 0, 0, false);
        for pair in toks.windows(2) {
            let (prev, tok) = (&pair[0], &pair[1]);
            if prev.kind == "OP" && matches!(prev.text.as_str(), "(" | ")") {
                depth += if prev.text == "(" { 1 } else { -1 };
            } else if prev.kind == "OP" && prev.text == "," && depth == 1 {
                after += i64::from(found);
                _before += i64::from(!found);
            }
            if !found && adjacent(prev, tok) {
                found = true;
            }
        }
        if !found {
            return vec![];
        }
        if after != 0 {
            return vec![
                "Call argument required.".to_owned(),
                super::_out_of_place(profile, stmt.text(), stmt.at_eol()),
            ];
        }
        return vec![format!("Incomplete expression: {}.", stmt.text())];
    } else if !is_verdict(&v, "pass1", "no_matching") {
        return vec![];
    }
    let mut messages = vec![];
    if profile.single_pass {
        let unknown: Vec<_> = toks
            .iter()
            .enumerate()
            .filter(|(_, t)| unknown_name(t, &operands, &functions))
            .collect();
        if let Some((idx, _)) = unknown.first()
            && (unknown.len() > 1 || toks[idx + 1..].iter().any(|t| t.text == "("))
        {
            return vec![];
        }
        messages.extend(
            unknown
                .into_iter()
                .map(|(_, t)| profile.unresolved_symbol_message.format(&t.text)),
        );
    }
    messages.push(super::_no_matching(profile, stmt.text(), stmt.at_eol()));
    messages
}

/// Port of checker/structure.py::_routine_arg_findings
pub fn _routine_arg_findings(
    stmt: &Unparsed,
    program: &crate::syntax::ProgramRef,
    profile: &'static Profile,
) -> Vec<String> {
    let Some(mut toks) = tokens(&stmt.base.text) else {
        return vec![];
    };
    let explicit = toks.first().is_some_and(|t| t.upper == "CALL");
    if explicit {
        toks.remove(0);
    }
    let routines: BTreeSet<_> = program
        .statements
        .iter()
        .filter_map(|st| {
            if let Stmt::SubDef(s) = st.as_ref()
                && (pystr::upper(&s.keyword) == "SUB" || explicit && !profile.single_pass)
            {
                Some(pystr::upper(&s.name))
            } else {
                None
            }
        })
        .collect();
    if toks.len() < 4
        || toks[0].kind != "IDENT"
        || !routines.contains(&toks[0].upper)
        || toks[1].text != "("
    {
        return vec![];
    }
    let mut args = vec![];
    let (mut depth, mut start, mut closed) = (0, 2, false);
    for (i, tok) in toks.iter().enumerate().skip(2) {
        if tok.kind == "OP" && tok.text == "(" {
            depth += 1;
        } else if tok.kind == "OP" && tok.text == ")" {
            if depth == 0 {
                if i != toks.len() - 1 {
                    return vec![];
                }
                args.push((
                    &toks[start..i],
                    pystr::lstrip(pystr::slice(
                        &stmt.base.text,
                        Some(col(toks[start - 1].col) + 1),
                        Some(col(tok.col)),
                    )),
                ));
                closed = true;
                break;
            }
            depth -= 1;
        } else if tok.kind == "OP" && tok.text == "," && depth == 0 {
            args.push((
                &toks[start..i],
                pystr::lstrip(pystr::slice(
                    &stmt.base.text,
                    Some(col(toks[start - 1].col) + 1),
                    Some(col(tok.col)),
                )),
            ));
            start = i + 1;
        }
    }
    if !closed {
        return vec![];
    }
    let mut messages = vec![];
    let operands = super::pass1_operands(program, profile);
    let functions = functions_for(profile);
    for (ts, text) in args {
        if ts.is_empty() {
            continue;
        }
        if ts.iter().any(|t| unknown_name(t, &operands, &functions)) {
            return vec![];
        }
        let v = scan(ts, Some(&operands), &functions);
        if is_verdict(&v, "pass1", "incomplete") {
            messages.push(format!("Incomplete expression: {text}."));
        } else if v.is_some() {
            return vec![];
        }
    }
    messages
}

/// Port of checker/structure.py::_instruction_arg_findings
pub fn _instruction_arg_findings(
    text: &str,
    operands: &BTreeSet<String>,
    profile: &Profile,
    variables: &BTreeSet<String>,
) -> Vec<(i64, String, String)> {
    let Some(toks) = tokens(text) else {
        return vec![];
    };
    if toks.len() < 2
        || toks[0].kind != "IDENT"
        || !profile.instructions().contains(&toks[0].upper)
        || crate::parser::helpers::_RAW_NAME_FIRST.contains(&toks[0].upper.as_str())
    {
        return vec![];
    }
    if ZERO_PARAM_INSTRUCTIONS.contains(&toks[0].upper.as_str())
        && !(toks[1].kind == "OP" && matches!(toks[1].text.as_str(), "=" | "."))
    {
        let token = instr_rules::_next_simple_token(pystr::slice(
            text,
            Some(col(toks[0].col) + col(pystr::len(&toks[0].text))),
            None,
        ));
        return if token.is_empty() {
            vec![]
        } else {
            vec![(3, format!("Too many parameters:  {token}."), String::new())]
        };
    }
    if toks[0].upper == "CALLTABLE"
        && toks.len() > 2
        && toks[1].kind == "IDENT"
        && toks[2].kind != "OP"
    {
        let cut = col(toks[1].col) + col(pystr::len(&toks[1].text));
        return _instruction_arg_findings(
            &format!(
                "{},{}",
                pystr::slice(text, None, Some(cut)),
                pystr::slice(text, Some(cut), None)
            ),
            operands,
            profile,
            variables,
        );
    }
    let paren = toks[1].kind == "OP" && toks[1].text == "(";
    if paren && (toks.len() < 3 || !toks.last().is_some_and(|t| t.kind == "OP" && t.text == ")")) {
        return vec![];
    }
    let record = profile
        .instructions()
        .get(&toks[0].upper)
        .expect("instruction checked above");
    if !paren
        && (toks[1].kind == "OP"
            || _STATEMENT_FORM_EXCLUDED.contains(&toks[0].upper.as_str())
            || (toks[0].upper != "CALLTABLE"
                && !instr_rules::plain_expression_params(record.params.clone())))
    {
        return vec![];
    }
    let mut args: Vec<(Vec<Token>, String)> = vec![];
    let (mut depth, mut cur) = (0, vec![]);
    let mut start = col(toks[1].col) + i64::from(paren);
    let mut closed = false;
    for t in toks.iter().skip(if paren { 2 } else { 1 }) {
        if t.kind == "OP" && t.text == "(" {
            depth += 1;
        } else if t.kind == "OP" && t.text == ")" {
            if depth == 0 {
                args.push((
                    std::mem::take(&mut cur),
                    pystr::lstrip(pystr::slice(text, Some(start), Some(col(t.col)))).to_owned(),
                ));
                closed = true;
                break;
            }
            depth -= 1;
        } else if t.kind == "OP" && t.text == "," && depth == 0 {
            args.push((
                std::mem::take(&mut cur),
                pystr::lstrip(pystr::slice(text, Some(start), Some(col(t.col)))).to_owned(),
            ));
            start = col(t.col) + 1;
            continue;
        }
        cur.push(t.clone());
    }
    if !closed {
        if paren {
            return vec![];
        }
        args.push((
            cur,
            pystr::lstrip(pystr::slice(text, Some(start), None)).to_owned(),
        ));
    }
    if toks[0].upper == "CALLTABLE" && args.len() > 2 {
        let token = instr_rules::_next_simple_token(&args[2].1);
        if !token.is_empty() {
            return vec![(3, format!("Too many parameters:  {token}."), String::new())];
        }
    }
    let functions = functions_for(profile);
    let syntax: Vec<_> = args
        .iter()
        .map(|(t, txt)| {
            let mut v = if t.is_empty() {
                None
            } else {
                scan(t, None, &functions)
            };
            if t.first().is_some_and(|t| {
                t.kind == "IDENT" && _START_ERROR_WORDS.contains(&t.upper.as_str())
            }) {
                v = verdict("pass1", "fallback");
            }
            (v, t, txt)
        })
        .collect();
    if syntax.iter().any(|(v, _, _)| {
        v.as_ref().is_some_and(|(s, k)| {
            s != "pass1" || !["incomplete", "no_matching", "fallback"].contains(&k.as_str())
        })
    }) || !(syntax.iter().any(|(v, _, _)| v.is_some()) || toks[0].upper == "CALLTABLE")
    {
        return vec![];
    }
    let mut out = vec![];
    let short = instr_rules::shortfall_message(record, col(args.len()));
    let params = instr_rules::_real_params(record.params.clone());
    for (i, (v, arg_toks, arg_text)) in syntax.iter().enumerate() {
        if arg_toks.is_empty() || i == 0 && toks[0].upper == "CALLTABLE" {
            continue;
        }
        let code = params
            .get(i)
            .map(|p| pystr::pymod_i64(*p, 100).expect("nonzero divisor"));
        if is_verdict(
            &scan(arg_toks, Some(operands), &functions),
            "pass2",
            "symbol",
        ) {
            if let Some(short) = &short
                && out.is_empty()
                && (code.is_some_and(|c| _DEFERRED_DEST_CODES.contains(&c))
                    || _symbol_stage(code) != 4)
            {
                return vec![(3, short.clone(), String::new())];
            }
            return if out.is_empty() {
                vec![(
                    _symbol_stage(code),
                    "symbol".to_owned(),
                    (*arg_text).clone(),
                )]
            } else {
                out
            };
        }
        if v.as_ref().is_some_and(|(_, k)| k != "fallback")
            && out.is_empty()
            && code.is_some_and(|c| _CONST_ONLY_CODES.contains(&c))
            && arg_toks
                .iter()
                .any(|t| unknown_name(t, operands, &functions))
        {
            let mut result = vec![(
                _symbol_stage(code),
                "symbol".to_owned(),
                (*arg_text).clone(),
            )];
            if arg_toks
                .iter()
                .any(|t| t.kind == "IDENT" && variables.contains(&t.upper))
            {
                result.push((
                    5,
                    format!("Variable illegal in Parameter: {arg_text}."),
                    String::new(),
                ));
            }
            return result;
        }
        if let Some((_, kind)) = v {
            out.push((1, kind.clone(), (*arg_text).clone()));
            if code.is_some_and(|c| _CONST_ONLY_CODES.contains(&c))
                && arg_toks
                    .iter()
                    .any(|t| t.kind == "IDENT" && variables.contains(&t.upper))
            {
                out.push((
                    3,
                    format!("Variable illegal in Parameter: {arg_text}."),
                    String::new(),
                ));
            }
        }
    }
    if let Some(short) = short
        && !out.is_empty()
        && args.len() >= 2
        && out.iter().all(|o| o.0 == 1)
    {
        out.push((3, short, String::new()));
    }
    out
}

const _STATEMENT_FORM_EXCLUDED: &[&str] = &[
    "IF", "ELSEIF", "WHILE", "DO", "LOOP", "SELECT", "CASE", "FOR", "NEXT", "CALL", "RETURN",
    "EXIT",
];
const _DEFERRED_DEST_CODES: &[i64] = &[8, 27, 29];

/// Port of checker/structure.py::_symbol_stage
pub fn _symbol_stage(code: Option<i64>) -> i64 {
    let kinds = expressions::_type_code_kinds();
    let kind = code
        .and_then(|c| usize::try_from(if c >= 100 { c - 100 } else { c }).ok())
        .and_then(|c| kinds.get(&c))
        .map(String::as_str);
    if kind == Some("deferred_text") {
        return 6;
    }
    if kind.is_some_and(|k| expressions::part1::_CONST_FAMILY_KINDS.contains(&k)) {
        4
    } else {
        2
    }
}

const _CONST_ONLY_CODES: &[i64] = &[1, 5, 20, 38];

/// Port of checker/structure.py::_later_arg_unknown
pub fn _later_arg_unknown(
    args: &[Token],
    operands: Option<&BTreeSet<String>>,
    functions: &ScanFunctions,
) -> bool {
    let (mut depth, mut start) = (0, None);
    for (k, t) in args.iter().enumerate() {
        if t.kind == "OP" && matches!(t.text.as_str(), "(" | ")") {
            depth += if t.text == "(" { 1 } else { -1 };
        } else if t.kind == "OP" && t.text == "," && depth == 0 {
            if let Some(start) = start
                && is_verdict(
                    &scan(&args[start..k], operands, functions),
                    "pass2",
                    "symbol",
                )
            {
                return true;
            }
            start = Some(k + 1);
        }
    }
    start.is_some_and(|start| {
        is_verdict(
            &scan(&args[start..], operands, functions),
            "pass2",
            "symbol",
        )
    })
}

const _SELF_CLOSING_FUNCTIONS: &[&str] = &[
    "ABS",
    "ABSLONG",
    "ARRAYLENGTH",
    "TYPEOF",
    "LINENUM",
    "SIGNATURE",
];

/// Port of checker/structure.py::_scan_expr
#[allow(clippy::if_not_else)]
pub fn _scan_expr(
    toks: &[Token],
    i: i64,
    operands: Option<&BTreeSet<String>>,
    functions: &ScanFunctions,
    assign: bool,
    start_operand: bool,
    unclosed: Option<&str>,
    cond: bool,
    to_ok: bool,
    open_fallback: bool,
    symbol_unknown: bool,
) -> ScanVerdict {
    let (mut to_levels, mut rel_levels) = (BTreeSet::new(), BTreeSet::new());
    let unknown = verdict(
        "pass2",
        if assign || symbol_unknown {
            "symbol"
        } else {
            "fallback"
        },
    );
    let unclosed = unclosed.filter(|s| !s.is_empty()).unwrap_or(if assign {
        "no_matching"
    } else {
        "fallback"
    });
    let mut parens: Vec<&str> = vec![];
    let mut pnames: Vec<Option<&str>> = vec![];
    let mut argn: Vec<i64> = vec![];
    let mut expect_operand = !start_operand;
    let mut i = i;
    while i < col(toks.len()) {
        let idx = usize::try_from(if i < 0 { col(toks.len()) + i } else { i }).ok()?;
        let t = toks.get(idx)?;
        let nxt = if i + 1 < col(toks.len()) {
            let next = if i + 1 < 0 {
                col(toks.len()) + i + 1
            } else {
                i + 1
            };
            usize::try_from(next).ok().and_then(|j| toks.get(j))
        } else {
            None
        };
        let opens = nxt.is_some_and(|t| t.kind == "OP" && t.text == "(");
        if expect_operand {
            if matches!(t.kind.as_str(), "NUMBER" | "STRING") {
                expect_operand = false;
            } else if t.kind == "IDENT" {
                let u = t.upper.as_str();
                if u == "NOT" {
                } else if _START_ERROR_WORDS.contains(&u) {
                    return None;
                } else if functions.contains_key(u) && !contains(operands, u) {
                    if !opens {
                        if _bare_function_needs_paren(u, functions) {
                            return verdict("pass1", "expects_paren");
                        }
                        expect_operand = false;
                    } else {
                        parens.push("call");
                        pnames.push(Some(u));
                        argn.push(0);
                        to_levels.remove(&parens.len());
                        rel_levels.remove(&parens.len());
                        i += 1;
                    }
                } else if contains(operands, u) {
                    if opens {
                        parens.push("call");
                        pnames.push(if functions.contains_key(u) && operands.is_none() {
                            Some(u)
                        } else {
                            None
                        });
                        argn.push(0);
                        to_levels.remove(&parens.len());
                        rel_levels.remove(&parens.len());
                        i += 1;
                    } else {
                        expect_operand = false;
                    }
                } else {
                    return unknown;
                }
            } else if t.kind == "OP" && t.text == "(" {
                parens.push("group");
                pnames.push(None);
                argn.push(0);
                to_levels.remove(&parens.len());
                rel_levels.remove(&parens.len());
            } else if t.kind == "OP" && matches!(t.text.as_str(), "-" | "+") {
            } else if t.kind == "OP"
                && t.text == "&"
                && !(nxt.is_some_and(|n| n.kind == "IDENT" && n.upper == "H" && n.col == t.col + 1)
                    && i + 2 < col(toks.len()))
            {
                return unknown;
            } else if t.kind == "OP"
                && (_BINARY_OPS.contains(&t.text.as_str()) || cond && t.text == ",")
            {
                return verdict("pass1", "fallback");
            } else {
                return None;
            }
        } else if t.kind == "OP" && t.text == ")" {
            if parens.is_empty() {
                return verdict(
                    "pass1",
                    if assign || cond {
                        "incomplete"
                    } else {
                        "fallback"
                    },
                );
            }
            parens.pop();
            pnames.pop();
            argn.pop();
        } else if t.kind == "OP" && t.text == "," {
            to_levels.remove(&parens.len());
            rel_levels.remove(&parens.len());
            if parens.is_empty() {
                if cond {
                    return verdict("pass1", "incomplete");
                }
                return if assign {
                    None
                } else {
                    verdict("pass1", "fallback")
                };
            }
            if parens.last() != Some(&"call") {
                return verdict("pass1", if assign { "no_matching" } else { "fallback" });
            }
            *argn.last_mut().expect("parallel stack") += 1;
            expect_operand = true;
        } else if t.kind == "OP" && _BINARY_OPS.contains(&t.text.as_str()) {
            if to_ok && ["=", "<>", "<", ">", "<=", ">="].contains(&t.text.as_str()) {
                if to_levels.contains(&parens.len()) {
                    return verdict(
                        "pass1",
                        if parens.is_empty() {
                            "incomplete"
                        } else {
                            "no_matching"
                        },
                    );
                }
                rel_levels.insert(parens.len());
            }
            expect_operand = true;
        } else if t.kind == "IDENT" && _BINARY_WORDS.contains(&t.upper.as_str()) {
            if t.upper != "MOD" {
                to_levels.remove(&parens.len());
                rel_levels.remove(&parens.len());
            }
            expect_operand = true;
        } else if to_ok
            && t.kind == "IDENT"
            && t.upper == "TO"
            && !to_levels.contains(&parens.len())
            && !rel_levels.contains(&parens.len())
        {
            to_levels.insert(parens.len());
            expect_operand = true;
        } else if parens.is_empty() && t.kind == "OP" && t.text == "." {
            return verdict("pass1", if assign { "incomplete" } else { "fallback" });
        } else if matches!(t.kind.as_str(), "IDENT" | "NUMBER" | "STRING") {
            if parens.last() == Some(&"call")
                && let Some(Some(name)) = pnames.last()
                && !_SELF_CLOSING_FUNCTIONS.contains(name)
            {
                let nargs = functions.get(*name).and_then(|f| f.nargs).unwrap_or(0);
                return verdict(
                    "pass1",
                    if argn.last().expect("parallel stack") + 1 < nargs {
                        "expects_comma"
                    } else {
                        "no_matching"
                    },
                );
            }
            return verdict(
                "pass1",
                if !parens.is_empty()
                    && (parens.last() == Some(&"group") || to_ok && t.upper == "TO")
                {
                    "no_matching"
                } else {
                    "incomplete"
                },
            );
        } else {
            return None;
        }
        i += 1;
    }
    if expect_operand {
        return if parens.is_empty() || open_fallback {
            verdict("pass1", "fallback")
        } else {
            None
        };
    }
    if !parens.is_empty() {
        return verdict("pass1", unclosed);
    }
    if assign {
        None
    } else {
        verdict("pass1", "fallback")
    }
}

/// Port of checker/structure.py::_bare_function_needs_paren
pub fn _bare_function_needs_paren(name: &str, functions: &ScanFunctions) -> bool {
    functions
        .get(name)
        .is_some_and(|record| record.nargs.is_some_and(|n| n > 0 || record.argspec != 0))
}

/// Port of checker/structure.py::_mentions_then
pub fn _mentions_then(node: Option<&Expr>) -> bool {
    let Some(node) = node else {
        return false;
    };
    if let Expr::Name(n) = node {
        return pystr::upper(&n.name) == "THEN";
    }
    node.children().iter().any(|n| _mentions_then(Some(n)))
}

/// Port of checker/structure.py::_check_if_pass1
pub fn _check_if_pass1(
    stmt: &Stmt,
    operands: &BTreeSet<String>,
    profile: &'static Profile,
    diags: &mut Diagnostics,
    program: Option<&crate::syntax::ProgramRef>,
) {
    let cond = match stmt {
        Stmt::If(s) => s.cond.as_deref(),
        Stmt::ElseIf(s) => s.cond.as_deref(),
        _ => None,
    };
    if _mentions_then(cond) && super::condition_part(stmt.text()).is_some() {
        let v = super::_pass1_scan(stmt.text(), Some(operands), profile);
        if let Some((stage, kind)) = v
            && stage == "pass1"
        {
            diags.error_basic(
                stmt.line(),
                &_pass1_message(profile, stmt, &kind, None),
                stmt.file(),
            );
        }
    }
    let Stmt::If(if_stmt) = stmt else {
        return;
    };
    for sub in if_stmt
        .inline
        .iter()
        .flatten()
        .chain(if_stmt.inline_else.iter().flatten())
    {
        if let Stmt::Call(s) = sub.as_ref()
            && !s.explicit
            && operands.contains(&pystr::upper(&s.name))
            && !profile.instructions().contains(&pystr::upper(&s.name))
            && !profile.functions().contains(&pystr::upper(&s.name))
        {
            diags.error_basic(
                sub.line(),
                &super::_out_of_place(profile, sub.text(), sub.at_eol()),
                sub.file(),
            );
        }
        if profile.known_first_word_pass1 && _block_keyword_call(sub) {
            diags.error_basic(
                sub.line(),
                &format!("Incomplete expression: {}.", sub.text()),
                sub.file(),
            );
        }
        if let Stmt::Unparsed(s) = sub.as_ref()
            && super::_is_generic_unparsed(s)
        {
            if let Some(program) = program {
                let messages = _routine_arg_findings(s, program, profile);
                if !messages.is_empty() {
                    for message in messages {
                        diags.error_basic(sub.line(), &message, sub.file());
                    }
                    continue;
                }
                if super::_logical_assign_accepted(sub, program, profile) {
                    continue;
                }
            }
            if let Some((stage, kind)) = super::_pass1_scan(sub.text(), Some(operands), profile)
                && stage == "pass1"
            {
                diags.error_basic(
                    sub.line(),
                    &_pass1_message(profile, sub, &kind, None),
                    sub.file(),
                );
            }
        }
    }
}

const _CALLED_BLOCK_KEYWORDS: &[&str] = &[
    "CASE", "DO", "FOR", "FUNCTION", "SCAN", "SELECT", "SUB", "SUBSCAN", "WHILE",
];
pub(crate) static _FOR_TO_RE: LazyLock<pyre::Regex> =
    LazyLock::new(|| pyre::Regex::new(r"^\s*For\s(?:.*?)=(?:.*?)\sTo\s(.*)$", pyre::I | pyre::S));
static _FOR_STEP_RE: LazyLock<pyre::Regex> =
    LazyLock::new(|| pyre::Regex::new(r"\sStep\s", pyre::I));

/// Port of checker/structure.py::_for_header_incomplete
pub fn _for_header_incomplete(text: &str, profile: &Profile) -> Option<(String, bool)> {
    let m = _FOR_TO_RE.match_(text)?;
    let init = _FOR_INIT_RE.match_(text);
    let mut parts = vec![];
    if let Some(init) = &init {
        parts.push((init.group(1).expect("capture"), true));
    }
    parts.extend(
        _FOR_STEP_RE
            .split(m.group(1).expect("capture"), 1)
            .into_iter()
            .map(|h| (h, false)),
    );
    let functions = functions_for(profile);
    for (part, is_init) in parts {
        let toks = tokens(part)?;
        if !toks.is_empty()
            && is_verdict(
                &_scan_expr(
                    &_merge_members(&toks),
                    0,
                    None,
                    &functions,
                    false,
                    false,
                    None,
                    false,
                    true,
                    false,
                    false,
                ),
                "pass1",
                "incomplete",
            )
        {
            return Some((part.to_owned(), is_init));
        }
    }
    None
}

pub(crate) static _FOR_INIT_RE: LazyLock<pyre::Regex> =
    LazyLock::new(|| pyre::Regex::new(r"^\s*For\s(?:.*?)=(.*?)\sTo\s", pyre::I | pyre::S));

/// Port of checker/structure.py::_merge_members
pub fn _merge_members(toks: &[Token]) -> Vec<Token> {
    let mut out = vec![];
    let mut i = 0;
    while i < toks.len() {
        let mut t = toks[i].clone();
        if t.kind == "IDENT" {
            let mut j = i + 1;
            let mut text = t.text.clone();
            while j < toks.len()
                && toks[j].kind == "OP"
                && toks[j].text == "."
                && (j + 1 >= toks.len() || toks[j + 1].kind == "IDENT")
            {
                text.push('.');
                j += 1;
                if j < toks.len() && toks[j].kind == "IDENT" {
                    text.push_str(&toks[j].text);
                    j += 1;
                }
            }
            if j > i + 1 {
                t = Token {
                    kind: "IDENT".to_owned(),
                    upper: pystr::upper(&text),
                    text,
                    col: t.col,
                    line: t.line,
                    value: lexer::TokenValue::None,
                };
            }
            out.push(t);
            i = j;
            continue;
        }
        out.push(t);
        i += 1;
    }
    out
}

/// Port of checker/structure.py::_for_header_parts
pub fn _for_header_parts(text: &str, profile: &Profile) -> Vec<(String, String)> {
    let mut out = vec![];
    let Some(m) = _FOR_TO_RE.match_(text) else {
        return out;
    };
    let split = _FOR_STEP_RE.split(m.group(1).expect("capture"), 1);
    let mut halves: Vec<Rc<str>> = vec![];
    for half in split {
        // CPython re.split reuses the empty and Latin-1 single-character string singletons.
        let half = if pystr::len(half) <= 1 {
            halves
                .first()
                .filter(|first| first.as_ref() == half)
                .cloned()
                .unwrap_or_else(|| Rc::from(half))
        } else {
            Rc::from(half)
        };
        halves.push(half);
    }
    let init = _FOR_INIT_RE.match_(text);
    let mut parts = vec![];
    if let Some(init) = &init {
        parts.push((Rc::<str>::from(init.group(1).expect("capture")), true));
    }
    parts.extend(halves.iter().map(|h| (Rc::clone(h), false)));
    let functions = functions_for(profile);
    for (part, is_init) in parts {
        let Some(toks) = tokens(&part) else {
            return vec![];
        };
        if toks.is_empty() {
            if is_init || Rc::ptr_eq(&part, &halves[0]) {
                out.push(("fallback".to_owned(), part.to_string()));
            }
            continue;
        }
        let v = _scan_expr(
            &_merge_members(&toks),
            0,
            None,
            &functions,
            true,
            false,
            Some("no_matching"),
            false,
            true,
            true,
            false,
        );
        if let Some((stage, kind)) = v
            && stage == "pass1"
            && ["incomplete", "fallback", "no_matching"].contains(&kind.as_str())
        {
            out.push((kind, part.to_string()));
        }
    }
    out
}

/// Port of checker/structure.py::_for_header_findings
pub fn _for_header_findings(text: &str, profile: &Profile) -> Vec<String> {
    _for_header_parts(text, profile)
        .iter()
        .map(|(kind, part)| _for_header_message(kind, part))
        .collect()
}

/// Port of checker/structure.py::_for_header_message
pub fn _for_header_message(kind: &str, part: &str) -> String {
    match kind {
        "incomplete" => format!("Incomplete expression: {part}."),
        "fallback" => format!("Invalid, or out of place expression: {part}."),
        "no_matching" => format!("No Matching, or out of place ) in expression: {part}."),
        _ => panic!("unknown For header finding: {kind}"),
    }
}

/// Port of checker/structure.py::_block_keyword_call
pub fn _block_keyword_call(stmt: &Stmt) -> bool {
    let expr = match stmt {
        Stmt::Assignment(s) => s.value.as_deref(),
        Stmt::If(s) => s.cond.as_deref(),
        Stmt::ElseIf(s) => s.cond.as_deref(),
        _ => return false,
    };
    expressions::_index_nodes_with(expr)
        .iter()
        .any(|i| _CALLED_BLOCK_KEYWORDS.contains(&pystr::upper(&i.name).as_str()))
}

/// Port of checker/structure.py::_unclosed_call_message
pub fn _unclosed_call_message(
    profile: &Profile,
    text: &str,
    at_eol: bool,
    operands: &BTreeSet<String>,
) -> Option<String> {
    if !(profile.known_first_word_pass1 || profile.single_pass) {
        return None;
    }
    let toks = tokens(text)?;
    if toks.is_empty() || toks.last()?.text == "(" {
        return None;
    }
    let mut stack: Vec<(Option<&Function>, i64)> = vec![];
    for (j, t) in toks.iter().enumerate() {
        if t.kind != "OP" {
            continue;
        }
        if t.text == "(" {
            let prev = j.checked_sub(1).and_then(|j| toks.get(j));
            let rec = prev
                .filter(|p| p.kind == "IDENT" && !operands.contains(&p.upper) && p.upper != "ABS")
                .and_then(|p| profile.functions().get(&p.upper));
            stack.push((rec, 1));
        } else if t.text == ")" && !stack.is_empty() {
            stack.pop();
        } else if t.text == ","
            && let Some((_, count)) = stack.last_mut()
        {
            *count += 1;
        }
    }
    let (record, count) = stack.last()?;
    let nargs = record.as_ref()?.nargs.filter(|n| *n != 0)?;
    if toks.last()?.text == "," {
        return if *count <= nargs {
            Some(super::_out_of_place(profile, text, at_eol))
        } else {
            None
        };
    }
    if *count >= nargs {
        return None;
    }
    let sep = if profile.fallback_echo_newline && at_eol {
        "\n"
    } else {
        ""
    };
    Some(format!("Expression expects , : {text}{sep}."))
}

/// Port of checker/structure.py::_assigns_const_index
pub fn _assigns_const_index(
    stmt: &Stmt,
    consts: &BTreeSet<String>,
    predefined: &BTreeSet<String>,
) -> bool {
    let Stmt::Assignment(stmt) = stmt else {
        return false;
    };
    let Some(Expr::Index(target)) = stmt.target.as_deref() else {
        return false;
    };
    let name = pystr::upper(&target.name);
    consts.contains(&name) || predefined.contains(&name)
}

/// Port of checker/structure.py::_pass1_message
pub fn _pass1_message(
    profile: &Profile,
    stmt: &Stmt,
    kind: &str,
    operands: Option<&BTreeSet<String>>,
) -> String {
    let mut text = super::condition_part(stmt.text());
    if kind == "no_matching"
        && let Some(operands) = operands
        && let Some(refined) = _unclosed_call_message(
            profile,
            text.as_deref().unwrap_or(stmt.text()),
            stmt.at_eol(),
            operands,
        )
    {
        return refined;
    }
    if text.is_none()
        && ["fallback", "no_matching"].contains(&kind)
        && let Some(operands) = operands
        && let Some(inline) = super::inline_if_condition(stmt.text())
        && !inline.is_empty()
        && let Some(refined) = _unclosed_call_message(profile, &inline, false, operands)
    {
        return refined;
    }
    if text.is_none()
        && kind == "fallback"
        && !profile.single_pass
        && matches!(stmt, Stmt::Unparsed(s) if s.reason.starts_with("expected an expression"))
        && let Some(inline) = super::inline_if_condition(stmt.text())
    {
        return format!("Invalid, or out of place expression: {inline}.");
    }
    if text.is_none() {
        text = Some(stmt.text().to_owned());
    }
    let text = text.expect("text assigned above");
    match kind {
        "fallback" => super::_out_of_place(profile, &text, stmt.at_eol()),
        "incomplete" => format!("Incomplete expression: {text}."),
        "no_matching" => format!("No Matching, or out of place ) in expression: {text}."),
        "expects_comma" => format!("Expression expects , : {text}."),
        "symbol" => format!("Invalid symbol in expression: {text}."),
        "expects_paren" => format!("Expression expects ( : {text}."),
        "missing_expr" => "Invalid or missing expression.".to_owned(),
        "invalid_assignment" => format!(
            "Invalid assignment in expression: {}{}.",
            stmt.text(),
            if profile.fallback_echo_newline && stmt.at_eol() {
                "\n"
            } else {
                ""
            }
        ),
        _ => panic!("unknown pass1 finding: {kind}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Results observed from all four read-only Python profiles during struct-6.
    #[test]
    fn observed_python_expression_scanner() {
        type Config = (
            bool,
            bool,
            Option<&'static str>,
            bool,
            bool,
            bool,
            bool,
            bool,
        );
        let configs: [Config; 4] = [
            (true, false, None, false, true, false, false, true),
            (false, false, None, false, true, false, false, true),
            (true, false, None, true, false, true, false, false),
            (true, true, None, false, true, false, true, false),
        ];
        let cases: Vec<(String,Vec<ScanVerdict>)> = serde_json::from_str(r#"[
["X Y",[["pass1","incomplete"],["pass1","incomplete"],["pass1","incomplete"],["pass1","incomplete"]]],
["Abs(X Y)",[["pass1","incomplete"],["pass1","incomplete"],["pass1","incomplete"],["pass1","incomplete"]]],
["Round(X Y,1)",[["pass1","expects_comma"],["pass1","expects_comma"],["pass1","expects_comma"],["pass1","incomplete"]]],
["Round(X,X Y)",[["pass1","no_matching"],["pass1","no_matching"],["pass1","no_matching"],["pass1","incomplete"]]],
["X To Y To Z",[["pass1","incomplete"],["pass1","incomplete"],["pass1","incomplete"],["pass1","incomplete"]]],
["(X To Y) + Z",[null,["pass1","fallback"],["pass1","no_matching"],null]],
["X = Y To Z",[["pass1","incomplete"],["pass1","incomplete"],["pass1","incomplete"],["pass1","incomplete"]]],
["X +",[["pass1","fallback"],["pass1","fallback"],["pass1","fallback"],["pass1","incomplete"]]],
["(X +",[null,null,["pass1","fallback"],null]],
["X .",[["pass1","incomplete"],["pass1","fallback"],["pass1","incomplete"],["pass1","incomplete"]]],
["& X",[["pass2","symbol"],["pass2","fallback"],["pass2","symbol"],null]],
["F(X)",[null,["pass1","fallback"],["pass2","symbol"],["pass1","incomplete"]]],
["X, Y",[null,["pass1","fallback"],["pass1","incomplete"],["pass1","incomplete"]]],
["(X,Y)",[["pass1","no_matching"],["pass1","fallback"],["pass1","no_matching"],null]],
["X)",[["pass1","incomplete"],["pass1","fallback"],["pass1","incomplete"],["pass1","incomplete"]]],
["X AND Y",[null,["pass1","fallback"],null,["pass1","incomplete"]]],
["X MOD Y To Z",[null,["pass1","fallback"],["pass1","incomplete"],["pass1","incomplete"]]],
["X To Y MOD Z",[null,["pass1","fallback"],["pass1","incomplete"],["pass1","incomplete"]]],
["X To Y = Z",[["pass1","incomplete"],["pass1","incomplete"],["pass1","incomplete"],["pass1","incomplete"]]],
["NOT X + Y",[null,["pass1","fallback"],null,["pass1","incomplete"]]],
["X + & H 3",[["pass2","symbol"],["pass2","fallback"],["pass2","symbol"],["pass1","incomplete"]]],
["X + & H",[["pass2","symbol"],["pass2","fallback"],["pass2","symbol"],["pass1","incomplete"]]],
["X + &HFF",[null,["pass1","fallback"],null,["pass1","incomplete"]]],
["Round",[null,["pass1","fallback"],["pass1","expects_paren"],["pass1","incomplete"]]],
["Abs",[null,["pass1","fallback"],["pass1","expects_paren"],["pass1","incomplete"]]],
["PI",[null,["pass1","fallback"],["pass2","symbol"],["pass1","incomplete"]]],
["(X",[["pass1","no_matching"],["pass1","fallback"],["pass1","no_matching"],null]],
["Round(X,",[null,null,["pass1","fallback"],["pass1","incomplete"]]],
["Round(X",[["pass1","no_matching"],["pass1","fallback"],["pass1","no_matching"],["pass1","incomplete"]]],
["Round(X,Y",[["pass1","no_matching"],["pass1","fallback"],["pass1","no_matching"],["pass1","incomplete"]]],
["((X Y",[["pass1","no_matching"],["pass1","no_matching"],["pass1","no_matching"],null]],
["X + TO Y",[null,null,null,["pass1","incomplete"]]],
["X + AND Y",[null,null,null,["pass1","incomplete"]]],
["X =",[["pass1","fallback"],["pass1","fallback"],["pass1","fallback"],["pass1","incomplete"]]],
["X(1, Z)",[null,["pass1","fallback"],["pass2","symbol"],["pass1","incomplete"]]],
["X OR (Y To Z)",[null,["pass1","fallback"],["pass1","no_matching"],["pass1","incomplete"]]],
["X . Y",[["pass1","incomplete"],["pass1","fallback"],["pass1","incomplete"],["pass1","incomplete"]]],
["X + * Y",[["pass1","fallback"],["pass1","fallback"],["pass1","fallback"],["pass1","incomplete"]]],
[", X",[null,null,["pass1","fallback"],null]],
["(X + Y))",[["pass1","incomplete"],["pass1","fallback"],["pass1","incomplete"],null]],
["X + NOT NOT Y",[null,["pass1","fallback"],null,["pass1","incomplete"]]],
["(X) Y",[["pass1","incomplete"],["pass1","incomplete"],["pass1","incomplete"],null]]
]"#).unwrap();
        let operands = BTreeSet::from(["X".to_owned(), "Y".to_owned()]);
        for name in ["cr6", "cr1000x", "cr300", "cr350"] {
            let profile = crate::profiles::get(name).unwrap();
            let functions = functions_for(profile);
            for (text, results) in &cases {
                for (config, expected) in configs.iter().zip(results) {
                    let &(
                        assign,
                        start,
                        unclosed,
                        cond,
                        to_ok,
                        open_fallback,
                        symbol_unknown,
                        everything,
                    ) = config;
                    let got = _scan_expr(
                        &tokens(text).unwrap(),
                        0,
                        if everything { None } else { Some(&operands) },
                        &functions,
                        assign,
                        start,
                        unclosed,
                        cond,
                        to_ok,
                        open_fallback,
                        symbol_unknown,
                    );
                    assert_eq!(&got, expected, "{name}: {text:?}, config={config:?}");
                }
            }
        }
    }

    #[test]
    fn observed_python_for_headers() {
        type Row = (
            String,
            Vec<(String, String)>,
            Option<(String, bool)>,
            Vec<String>,
        );
        let cases: Vec<Row> = serde_json::from_str(r#"[
["For I = 1 To 5 Step -1 X",[["incomplete","-1 X"]],["-1 X",false],["Incomplete expression: -1 X."]],
["For I = To Step ",[["fallback",""]],null,["Invalid, or out of place expression: ."]],
["For I = 1 4 To 3",[["incomplete"," 1 4"]],[" 1 4",true],["Incomplete expression:  1 4."]],
["For I = 1 To T.X 3",[["incomplete","T.X 3"]],["T.X 3",false],["Incomplete expression: T.X 3."]],
["For I = 1 To (X +",[["fallback","(X +"]],null,["Invalid, or out of place expression: (X +."]],
["For I = 1 To 5 Step",[["incomplete","5 Step"]],["5 Step",false],["Incomplete expression: 5 Step."]],
["For I = 1 To  Step ",[["fallback",""],["fallback",""]],null,["Invalid, or out of place expression: .","Invalid, or out of place expression: ."]],
["For I = 1 To   Step  ",[["fallback"," "],["fallback"," "]],null,["Invalid, or out of place expression:  .","Invalid, or out of place expression:  ."]],
["For I = 1 To    Step   ",[["fallback","  "]],null,["Invalid, or out of place expression:   ."]],
["For I = 1 To T. ",[],null,[]],
["For I = 1 To Status.SerialNumber(1,1)",[],null,[]],
["For I = (X Y To (Z 2 Step (W +",[["no_matching"," (X Y"],["no_matching","(Z 2"],["fallback","(W +"]],null,["No Matching, or out of place ) in expression:  (X Y.","No Matching, or out of place ) in expression: (Z 2.","Invalid, or out of place expression: (W +."]],
["For I = 1 To 10 Step -1",[],null,[]],
["bad For I = 1 To 3",[],null,[]]
]"#).unwrap();
        for name in ["cr6", "cr1000x", "cr300", "cr350"] {
            let profile = crate::profiles::get(name).unwrap();
            for (text, parts, incomplete, findings) in &cases {
                assert_eq!(&_for_header_parts(text, profile), parts, "{name}: {text:?}");
                assert_eq!(
                    &_for_header_incomplete(text, profile),
                    incomplete,
                    "{name}: {text:?}"
                );
                assert_eq!(
                    &_for_header_findings(text, profile),
                    findings,
                    "{name}: {text:?}"
                );
            }
        }
    }
}
