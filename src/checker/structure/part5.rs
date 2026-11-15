//! Port of `checker/structure.py` (lines 2869-3659).

#![allow(dead_code, unused_variables, unused_imports)]
// Keep the required one-line Python function references verbatim.
#![allow(clippy::doc_markdown)]

use crate::checker::expressions::part1::{KnownView, UndefinedName};
use crate::checker::{context, expressions, var_types};
use crate::profiles::{Function, Table};
use crate::syntax::Token;
use crate::{lexer, parser, pyre, pystr};
use std::collections::BTreeSet;
use std::rc::Rc;
use std::sync::LazyLock;

use crate::diagnostics::{DiagRef, Diagnostic, Diagnostics};
use crate::profiles::Profile;
use crate::syntax::{
    Assignment, Call, Case, ConstDecl, Declaration, Expr, ExprRef, Index, Instruction, Juxtaposed,
    Member, Number, OrderedMap, Program, ProgramRef, Stmt, StmtRef, SubDef, Unparsed, VarDecl,
};

/// Port of checker/structure.py::_unclosed_paren_kind
pub fn _unclosed_paren_kind(text: &str, known: &BTreeSet<String>) -> String {
    let Ok(tokens) = parser::tokenize_for_parser(text, 0) else {
        return "fallback".into();
    };
    let tokens: Vec<_> = tokens.iter().filter(|t| t.kind != "EOF").collect();
    if tokens
        .first()
        .is_none_or(|t| t.kind != "IDENT" || !known.contains(&t.upper))
    {
        return "fallback".into();
    }
    if tokens
        .iter()
        .any(|t| t.kind == "IDENT" && !known.contains(&t.upper))
    {
        return "symbol".into();
    }
    "pass1".into()
}

/// Port of checker/structure.py::is_hash_statement
pub fn is_hash_statement(stmt: &Unparsed) -> bool {
    pystr::lstrip(&stmt.base.text).starts_with('#')
}

/// Port of checker/structure.py::_is_generic_unparsed
pub fn _is_generic_unparsed(stmt: &Unparsed) -> bool {
    // These shared Python constants are private in part9.rs.
    !(is_hash_statement(stmt)
        || [
            "unexpected trailing input starting at '+='",
            "dotted SNMP OID literal",
        ]
        .iter()
        .any(|r| stmt.reason.starts_with(r))
        || [
            "empty statement",
            "Units without '='",
            "Units target with no '='",
            "Const without '='",
            "Alias without '='",
            "Alias target with no '='",
            "For missing parameter",
        ]
        .contains(&stmt.reason.as_str())
        || [
            "Invalid As clause: ",
            "Const bad value: ",
            "Const bad name: ",
            "Do/Loop parameter: ",
            "Const lead: ",
            "Alias nothing after '='",
        ]
        .iter()
        .any(|r| stmt.reason.starts_with(r))
        || stmt
            .reason
            .starts_with(parser::helpers::UNCLOSED_DIM_PREFIX))
}

pub const _BINARY_WORDS: &[&str] = &["MOD", "OR", "XOR", "AND", "IMP", "EQV"];

pub const _START_ERROR_WORDS: &[&str] = &["MOD", "OR", "XOR", "AND", "IMP", "EQV", "IS", "TO"];

pub const _BINARY_OPS: &[&str] = &[
    "=", "<>", "<", ">", "<=", ">=", "+", "-", "*", "/", "^", "&", "\\", ">>", "<<",
];

/// Port of checker/structure.py::pass1_operands
pub fn pass1_operands(_program: &Program) -> BTreeSet<String> {
    todo!("checker/structure.py::pass1_operands (omitted: profile: untyped)")
}

/// Port of checker/structure.py::pass1_operands
pub fn pass1_operands_with(program: &ProgramRef, profile: &'static Profile) -> BTreeSet<String> {
    let ctx = context::of(program, Some(profile), None);
    let mut names = ctx.declared_variable_names().clone();
    for (k, _) in program.consts.iter() {
        names.insert(k.clone());
        names.insert(pystr::upper(k));
    }
    for stmt in &program.statements {
        match stmt.as_ref() {
            Stmt::ConstDecl(c) => {
                names.insert(pystr::upper(&c.name));
            }
            Stmt::For(f) => {
                if let Some(Expr::Name(n)) = f.var.as_deref() {
                    names.insert(pystr::upper(&n.name));
                }
            }
            Stmt::Instruction(i) if pystr::upper(&i.keyword) == "ALIAS" => {
                for a in i.args.iter().skip(1).flatten() {
                    if let Expr::Name(n) = a.as_ref() {
                        names.insert(pystr::upper(&n.name));
                    }
                }
            }
            _ => {}
        }
    }
    names.extend(profile.symbols().iter().map(|(k, _)| k.to_owned()));
    names.extend(
        expressions::part1::_EXTRA_PREDEFINED_SYMBOLS
            .iter()
            .map(|s| (*s).to_owned()),
    );
    names
}

static _COND_HEAD: LazyLock<pyre::Regex> = LazyLock::new(|| {
    pyre::Regex::new(
        r"\s*(?:IF|ELSEIF|WHILE|DO\s+WHILE|DO\s+UNTIL|LOOP\s+WHILE|LOOP\s+UNTIL|SELECT\s+CASE|CASE)\b(.*)$",
        pyre::I | pyre::S,
    )
});
static _COND_THEN: LazyLock<pyre::Regex> =
    LazyLock::new(|| pyre::Regex::new(r"(.*?)\bTHEN\s*$", pyre::I | pyre::S));
static _INLINE_IF: LazyLock<pyre::Regex> =
    LazyLock::new(|| pyre::Regex::new(r"\s*(?:IF|ELSEIF)\b(.*?)\bTHEN\b\s*\S", pyre::I | pyre::S));
static IF_HEAD: LazyLock<pyre::Regex> = LazyLock::new(|| pyre::Regex::new(r"\s*IF\b", pyre::I));
static SELECT_HEAD: LazyLock<pyre::Regex> =
    LazyLock::new(|| pyre::Regex::new(r"\s*SELECT", pyre::I));
static CASE_HEAD: LazyLock<pyre::Regex> = LazyLock::new(|| pyre::Regex::new(r"\s*CASE\b", pyre::I));
static TO_IS: LazyLock<pyre::Regex> = LazyLock::new(|| pyre::Regex::new(r"\b(?:TO|IS)\b", pyre::I));
static IF_ELSEIF: LazyLock<pyre::Regex> =
    LazyLock::new(|| pyre::Regex::new(r"\s*(?:IF|ELSEIF)\b", pyre::I));
static THEN: LazyLock<pyre::Regex> = LazyLock::new(|| pyre::Regex::new(r"\bTHEN\b", pyre::I));

/// Port of checker/structure.py::inline_if_condition
pub fn inline_if_condition(text: &str) -> Option<String> {
    Some(pystr::lstrip(_INLINE_IF.match_(text)?.group(1)?).to_owned())
}

/// Port of checker/structure.py::_inline_condition_stmt
pub fn _inline_condition_stmt(_stmt: &Stmt) -> StmtRef {
    todo!("checker/structure.py::_inline_condition_stmt (omitted: profile: untyped)")
}

/// Port of checker/structure.py::_inline_condition_stmt
pub fn _inline_condition_stmt_with(stmt: &StmtRef, profile: &Profile) -> StmtRef {
    if !profile.single_pass
        && let Stmt::Unparsed(u) = stmt.as_ref()
        && (u.reason.starts_with("expected THEN") || u.reason.starts_with("expected ')'"))
        && IF_HEAD.match_(&u.base.text).is_some()
        && let Some(cond) = inline_if_condition(&u.base.text)
    {
        return crate::syntax::replace_stmt(stmt, |replaced| {
            replaced.base_mut().text = "If ".to_owned() + &cond;
            replaced.base().at_eol.set(false);
        });
    }
    stmt.clone()
}

/// Port of checker/structure.py::condition_part
pub fn condition_part(text: &str) -> Option<String> {
    let m = _COND_HEAD.match_(text)?;
    let mut cond = m.group(1)?;
    if SELECT_HEAD.match_(text).is_none() {
        cond = pystr::lstrip(cond);
    }
    if CASE_HEAD.match_(text).is_some() && TO_IS.search(cond).is_some() {
        return None;
    }
    if IF_ELSEIF.match_(text).is_some() {
        if let Some(t) = _COND_THEN.match_(cond) {
            return t.group(1).map(str::to_owned);
        }
        if THEN.search(cond).is_some() {
            return None;
        }
    }
    Some(cond.to_owned())
}

pub const _LEADING_OPS: &[&str] = &[
    "=", "*", "/", "<", "<>", ">", ">=", "<=", "^", "\\", ",", ")", ".", ";",
];

/// Port of checker/structure.py::_invalid_assignment
pub fn _invalid_assignment() -> bool {
    todo!("checker/structure.py::_invalid_assignment (omitted: toks: untyped)")
}

/// Port of checker/structure.py::_invalid_assignment
pub fn _invalid_assignment_with(toks: &[Token]) -> bool {
    let mut depth = 0;
    let mut eq = None;
    for (i, t) in toks.iter().enumerate() {
        if t.kind == "OP" {
            match t.text.as_str() {
                "(" | "{" => depth += 1,
                ")" | "}" => depth -= 1,
                "=" if depth == 0 => {
                    eq = Some(i);
                    break;
                }
                _ => {}
            }
        }
    }
    let Some(eq) = eq else {
        return false;
    };
    let lhs = &toks[..eq];
    if lhs.is_empty() || (lhs[0].kind == "OP" && lhs[0].text != "-") {
        return false;
    }
    if lhs.len() == 1 || (lhs[0].text == "-" && lhs.len() == 2) {
        return matches!(lhs[lhs.len() - 1].kind.as_str(), "NUMBER" | "STRING");
    }
    depth = 0;
    for t in lhs {
        if t.kind == "OP" && matches!(t.text.as_str(), "(" | ")") {
            depth += if t.text == "(" { 1 } else { -1 };
        } else if depth == 0
            && t.kind == "OP"
            && ["<", ">", "<>", "<=", ">="].contains(&t.text.as_str())
        {
            return false;
        }
    }
    let known = lhs
        .iter()
        .filter(|t| t.kind == "IDENT")
        .map(|t| t.upper.clone())
        .collect();
    if scan_expr_dependency(
        lhs, 0, &known, None, true, false, None, false, true, false, false,
    )
    .is_some()
    {
        return false;
    }
    depth = 0;
    for (i, t) in lhs.iter().enumerate() {
        if t.kind == "OP" && t.text == "(" {
            depth += 1;
        } else if t.kind == "OP" && t.text == ")" {
            depth -= 1;
        } else if depth == 0 && t.kind == "OP" && _BINARY_OPS.contains(&t.text.as_str()) && i != 0 {
            return true;
        }
    }
    false
}

/// Port of checker/structure.py::_semicolons_dropped
pub fn _semicolons_dropped(_text: &str) -> bool {
    todo!("checker/structure.py::_semicolons_dropped (omitted: profile: untyped)")
}

/// Port of checker/structure.py::_semicolons_dropped
pub fn _semicolons_dropped_with(text: &str, profile: &Profile) -> bool {
    if !text.contains(';') {
        return false;
    }
    let Ok(toks) = parser::tokenize_for_parser(text, 0) else {
        return false;
    };
    let n = toks.len();
    parser::helpers::_drop_semicolons(toks, profile).len() < n
}

/// Port of checker/structure.py::_pass1_scan
pub fn _pass1_scan(_text: &str, _operands: &BTreeSet<String>) -> Option<(String, String)> {
    todo!("checker/structure.py::_pass1_scan (omitted: profile: untyped)")
}

/// Port of checker/structure.py::_pass1_scan
pub fn _pass1_scan_with(
    text: &str,
    operands: &BTreeSet<String>,
    profile: &Profile,
) -> Option<(String, String)> {
    static EMPTY_COND: LazyLock<pyre::Regex> =
        LazyLock::new(|| pyre::Regex::new(r"\bTHEN\b|^\s*SELECT", pyre::I));
    static TO_OK: LazyLock<pyre::Regex> =
        LazyLock::new(|| pyre::Regex::new(r"\s*(?:SELECT|IF|ELSEIF)\b", pyre::I));
    let verdict = |kind: &str| Some(("pass1".to_owned(), kind.to_owned()));
    let cond = condition_part(text);
    let drop = _semicolons_dropped_with(text, profile);
    let functions = profile.functions();
    let scan = |ts: &[Token], i: usize, assign: bool, cond: bool| {
        scan_expr_dependency(
            ts,
            i,
            operands,
            Some(functions),
            assign,
            false,
            None,
            cond,
            true,
            false,
            false,
        )
    };
    if let Some(cond) = cond {
        let mut ctoks = parser::tokenize_for_parser(&cond, 0).ok()?;
        ctoks.retain(|t| t.kind != "EOF" && !(drop && t.kind == "OP" && t.text == ";"));
        if ctoks.is_empty() {
            return if text == pystr::rstrip(text) && EMPTY_COND.search(text).is_none() {
                verdict("missing_expr")
            } else {
                verdict("fallback")
            };
        }
        return scan_expr_dependency(
            &ctoks,
            0,
            operands,
            Some(functions),
            true,
            false,
            None,
            true,
            TO_OK.match_(text).is_some(),
            true,
            false,
        );
    }
    let mut toks = match parser::tokenize_for_parser(text, 0) {
        Ok(toks) => toks,
        Err(e) => {
            let head_text = pystr::slice(text, None, Some(i64::try_from(e.col).ok()?));
            let c = pystr::char_at(text, i64::try_from(e.col).ok()?);
            if e.message.starts_with("unrecognised character") && pystr::strip(head_text).is_empty()
            {
                return if matches!(c, Some('%' | '[' | ']')) {
                    verdict("fallback")
                } else {
                    None
                };
            }
            if e.message != "unterminated string" || c != Some('"') {
                return None;
            }
            let mut head = parser::tokenize_for_parser(head_text, 0).ok()?;
            head.retain(|t| t.kind != "EOF");
            if head.first().is_some_and(|t| {
                t.kind == "OP" && t.text != ";" && _LEADING_OPS.contains(&t.text.as_str())
            }) {
                return verdict("fallback");
            }
            if head.len() < 2
                || head[0].kind != "IDENT"
                || head[1].kind != "IDENT"
                || !operands.contains(&head[0].upper)
                || _BINARY_WORDS.contains(&head[1].upper.as_str())
                || functions.contains(&head[0].upper)
                || profile.instructions().contains(&head[0].upper)
                || ["IF", "ELSEIF", "WHILE", "UNTIL", "SELECT", "CASE", "FOR"]
                    .contains(&head[0].upper.as_str())
            {
                return None;
            }
            return verdict("fallback");
        }
    };
    toks.retain(|t| t.kind != "EOF" && !(drop && t.kind == "OP" && t.text == ";"));
    let first = toks.first()?;
    if _logical_bad_target_with(&toks, operands, functions) {
        return verdict("invalid_assignment");
    }
    let mut i = 1;
    if first.kind == "IDENT" {
        let u = &first.upper;
        if _START_ERROR_WORDS.contains(&u.as_str()) {
            return verdict("fallback");
        }
        if u == "NOT" {
            return scan(&toks, 1, false, false);
        }
        if functions.contains(u) && !operands.contains(u) {
            if toks.get(i).is_some_and(|t| t.kind == "OP" && t.text == "(") {
                return scan_expr_dependency(
                    &toks,
                    0,
                    operands,
                    Some(functions),
                    false,
                    false,
                    Some("no_matching"),
                    false,
                    true,
                    false,
                    true,
                );
            }
            // part6 still omits the functions argument.
            if super::part6::_bare_function_needs_paren(u) {
                return verdict("expects_paren");
            }
            return None;
        }
        if !operands.contains(u) {
            return None;
        }
        if toks.get(i).is_some_and(|t| t.kind == "OP" && t.text == "(") {
            let mut depth = 0;
            let mut j = i;
            for (k, t) in toks.iter().enumerate().skip(i) {
                j = k;
                if t.kind == "OP" {
                    depth += i32::from(t.text == "(") - i32::from(t.text == ")");
                }
                if depth == 0 {
                    break;
                }
            }
            if depth != 0
                || scan(&toks[i + 1..j], 0, true, false).is_some()
                || later_arg_unknown_dependency(&toks[i + 1..=j], operands, functions)
            {
                if profile.symbols().contains(u) && depth == 0 {
                    return verdict("fallback");
                }
                return None;
            }
            i = j + 1;
        }
    } else if matches!(first.kind.as_str(), "NUMBER" | "STRING") {
        if _invalid_assignment_with(&toks) {
            return verdict("invalid_assignment");
        }
    } else if first.kind == "OP" && first.text == "-" && toks.len() > 1 && toks[1].kind == "NUMBER"
    {
        if _invalid_assignment_with(&toks) {
            return verdict("invalid_assignment");
        }
        i = 2;
    } else if first.kind == "OP" && matches!(first.text.as_str(), "{" | "}") {
        return verdict("fallback");
    } else if first.kind == "OP" && first.text == "(" {
        let result = scan(&toks, 0, false, false);
        let eq = toks
            .iter()
            .position(|t| t.kind == "OP" && matches!(t.text.as_str(), "=" | "=="));
        let predefined = |u: &str| {
            profile.symbols().contains(u)
                || expressions::part1::_EXTRA_PREDEFINED_SYMBOLS.contains(&u)
        };
        if let Some(eq) = eq
            && toks[1..eq]
                .iter()
                .all(|t| !(t.kind == "OP" && matches!(t.text.as_str(), "(" | ")")))
        {
            let lhs = &toks[1..eq];
            if lhs.len() == 1
                && (matches!(lhs[0].kind.as_str(), "NUMBER" | "STRING")
                    || lhs[0].kind == "IDENT"
                        && !profile.instructions().contains(&lhs[0].upper)
                        && predefined(&lhs[0].upper))
                || lhs.len() == 2 && lhs[0].text == "-" && lhs[1].kind == "NUMBER"
            {
                return verdict("invalid_assignment");
            }
            if lhs.first().is_some_and(|t| {
                t.kind == "IDENT" && operands.contains(&t.upper) && !predefined(&t.upper)
            }) {
                return result;
            }
        }
        let mut depth = 0;
        let mut close = toks.len();
        for (j, t) in toks.iter().enumerate() {
            if t.kind == "OP" && matches!(t.text.as_str(), "(" | ")") {
                depth += if t.text == "(" { 1 } else { -1 };
                if depth == 0 {
                    close = j;
                    break;
                }
            }
        }
        if let Some(eq) = eq
            && eq < close
            && result == verdict("incomplete")
            && toks[1..eq].iter().any(|t| {
                t.kind == "OP" && ["<", ">", "<>", "<=", ">="].contains(&t.text.as_str())
                    || t.kind == "IDENT" && _BINARY_WORDS.contains(&t.upper.as_str())
            })
        {
            return result;
        }
        return if result == verdict("incomplete") {
            verdict("fallback")
        } else {
            result
        };
    } else if first.kind == "OP" && toks.len() > 1 && _LEADING_OPS.contains(&first.text.as_str()) {
        return verdict("fallback");
    } else if first.kind == "OP" && toks.len() > 1 && matches!(first.text.as_str(), "+" | "-") {
        let signed = _signed_target_with(&toks, operands, functions, profile);
        if signed.as_deref() == Some("bad") {
            return verdict("invalid_assignment");
        }
        let result = scan(&toks, 1, false, false);
        return if result == verdict("incomplete") && signed.is_none() {
            verdict("fallback")
        } else {
            result
        };
    } else {
        return None;
    }
    if i == toks.len() {
        return verdict("fallback");
    }
    let nxt = &toks[i];
    if nxt.kind == "OP" && nxt.text == "=" && first.kind == "IDENT" {
        return scan(&toks, i + 1, true, i == 1);
    }
    if matches!(nxt.kind.as_str(), "IDENT" | "NUMBER" | "STRING")
        && !(nxt.kind == "IDENT" && _BINARY_WORDS.contains(&nxt.upper.as_str()))
    {
        return verdict("fallback");
    }
    if nxt.kind == "OP" && matches!(nxt.text.as_str(), "," | ")" | "}") {
        return verdict("fallback");
    }
    if nxt.kind == "OP" && _BINARY_OPS.contains(&nxt.text.as_str()) || nxt.kind == "IDENT" {
        if first.kind == "IDENT" && nxt.kind == "OP" && _invalid_assignment_with(&toks) {
            return verdict("invalid_assignment");
        }
        let result = scan(&toks, i + 1, false, false);
        if result.as_ref().is_some_and(|(stage, kind)| {
            stage == "pass1" && ["incomplete", "no_matching", "fallback"].contains(&kind.as_str())
        }) {
            let eq = if nxt.kind == "IDENT" {
                _logical_assign_eq_with(&toks, i64::try_from(i).ok()?)
            } else {
                None
            };
            if let Some(eq) = eq {
                let tail = scan(&toks, usize::try_from(eq).ok()? + 1, true, false);
                if tail == verdict("incomplete") || tail == verdict("no_matching") {
                    return tail;
                }
            }
            return verdict("fallback");
        }
        return result;
    }
    None
}

/// Port of checker/structure.py::_assigns_user_const
pub fn _assigns_user_const(_stmt: &Assignment, _program: &Program) -> bool {
    // Context::enclosing_sub requires the original statement/program Rc identities.
    // Use _assigns_user_const_with rather than manufacturing replacement identities.
    todo!("checker/structure.py::_assigns_user_const")
}

/// Port of checker/structure.py::_assigns_user_const
pub fn _assigns_user_const_with(stmt: &StmtRef, program: &ProgramRef) -> bool {
    let Stmt::Assignment(a) = stmt.as_ref() else {
        return false;
    };
    let Some(Expr::Name(n)) = a.target.as_deref() else {
        return false;
    };
    let name = pystr::upper(&n.name);
    if !program
        .statements
        .iter()
        .any(|st| matches!(st.as_ref(), Stmt::ConstDecl(c) if pystr::upper(&c.name) == name))
    {
        return false;
    }
    let ctx = context::of(program, None, None);
    let sub = ctx.enclosing_sub(stmt);
    sub.as_ref().is_none_or(|st| match st.as_ref() {
        Stmt::SubDef(s) => !s.params.iter().any(|p| pystr::upper(&p.name) == name),
        _ => true,
    })
}

pub const _LOGICAL_WORDS: &[&str] = &["AND", "OR", "XOR", "IMP", "EQV"];

/// Port of checker/structure.py::_signed_target
pub fn _signed_target() -> Option<String> {
    todo!(
        "checker/structure.py::_signed_target (omitted: toks: untyped, operands: untyped, functions: untyped, profile: untyped)"
    )
}

/// Port of checker/structure.py::_signed_target
pub fn _signed_target_with(
    toks: &[Token],
    operands: &BTreeSet<String>,
    functions: &Table<Function>,
    profile: &Profile,
) -> Option<String> {
    let k = toks
        .iter()
        .take_while(|t| t.kind == "OP" && matches!(t.text.as_str(), "+" | "-"))
        .count();
    let mut depth = 0;
    let mut eq = None;
    for (j, t) in toks.iter().enumerate() {
        if t.kind == "OP" && matches!(t.text.as_str(), "(" | ")") {
            depth += if t.text == "(" { 1 } else { -1 };
        } else if t.kind == "OP" && t.text == "=" && depth == 0 {
            eq = Some(j);
            break;
        }
    }
    let eq = eq?;
    if eq == k
        || toks.iter().any(|t| {
            t.kind == "STRING"
                || (t.kind == "IDENT"
                    && !operands.contains(&t.upper)
                    && !functions.contains(&t.upper)
                    && !_BINARY_WORDS.contains(&t.upper.as_str()))
        })
    {
        return None;
    }
    let seg = toks.get(k..eq)?;
    let first = seg.first()?;
    let indexed = seg.len() > 2
        && first.kind == "IDENT"
        && seg[1].text == "("
        && seg[seg.len() - 1].text == ")";
    if first.kind == "IDENT" && functions.contains(&first.upper) && !operands.contains(&first.upper)
    {
        return indexed.then(|| "bad".into());
    }
    if seg.len() == 1 && first.kind == "NUMBER" {
        return Some("bad".into());
    }
    if first.kind == "IDENT" && (seg.len() == 1 || indexed && !profile.single_pass) {
        return Some(
            if toks[..k].iter().any(|t| t.text == "-") {
                "bad"
            } else {
                "ok"
            }
            .into(),
        );
    }
    None
}

/// Port of checker/structure.py::_logical_bad_target
pub fn _logical_bad_target() -> bool {
    todo!(
        "checker/structure.py::_logical_bad_target (omitted: toks: untyped, operands: untyped, functions: untyped)"
    )
}

/// Port of checker/structure.py::_logical_bad_target
pub fn _logical_bad_target_with(
    toks: &[Token],
    operands: &BTreeSet<String>,
    functions: &Table<Function>,
) -> bool {
    let (mut depth, mut last_logical, mut eq) = (0, None, None);
    for (k, t) in toks.iter().enumerate() {
        if t.kind == "OP" {
            match t.text.as_str() {
                "(" | "{" => depth += 1,
                ")" | "}" => depth -= 1,
                "=" if depth == 0 => {
                    eq = Some(k);
                    break;
                }
                "." => return false,
                _ => {}
            }
        } else if t.kind == "IDENT" && depth == 0 && _LOGICAL_WORDS.contains(&t.upper.as_str()) {
            last_logical = Some(k);
        }
    }
    let (Some(eq), Some(last)) = (eq, last_logical) else {
        return false;
    };
    if last + 1 >= eq || toks.iter().any(|t| t.kind == "STRING") {
        return false;
    }
    if toks.iter().any(|t| {
        t.kind == "IDENT"
            && !_BINARY_WORDS.contains(&t.upper.as_str())
            && t.upper != "NOT"
            && !operands.contains(&t.upper)
            && !functions.contains(&t.upper)
    }) {
        return false;
    }
    let mut seg = &toks[last + 1..eq];
    if seg
        .iter()
        .any(|t| t.kind == "OP" && ["<", ">", "<>", "<=", ">="].contains(&t.text.as_str()))
    {
        return false;
    }
    while seg.len() > 2 && seg[0].text == "(" && seg[seg.len() - 1].text == ")" {
        let mut d = 0;
        let mut closes_at_end = true;
        for (j, t) in seg.iter().enumerate() {
            if t.kind == "OP" {
                d += i32::from(t.text == "(") - i32::from(t.text == ")");
            }
            if d == 0 && j < seg.len() - 1 {
                closes_at_end = false;
                break;
            }
        }
        if !closes_at_end {
            break;
        }
        seg = &seg[1..seg.len() - 1];
    }
    let Some(first) = seg.first() else {
        return false;
    };
    if first.kind == "NUMBER" || first.kind == "OP" && matches!(first.text.as_str(), "-" | "+") {
        return true;
    }
    if first.kind != "IDENT" {
        return false;
    }
    if first.upper == "NOT" {
        return true;
    }
    if functions.contains(&first.upper) && !operands.contains(&first.upper) {
        return seg.len() > 1 && seg[1].text == "(";
    }
    let mut d = 0;
    for (j, t) in seg.iter().enumerate() {
        if t.kind == "OP" && t.text == "(" {
            d += 1;
        } else if t.kind == "OP" && t.text == ")" {
            d -= 1;
        } else if d == 0
            && j != 0
            && (t.kind == "OP" && _BINARY_OPS.contains(&t.text.as_str())
                || t.kind == "IDENT" && _BINARY_WORDS.contains(&t.upper.as_str()))
        {
            return true;
        }
    }
    false
}

/// Port of checker/structure.py::_logical_assign_eq
pub fn _logical_assign_eq(_i: i64) -> Option<i64> {
    todo!("checker/structure.py::_logical_assign_eq (omitted: toks: untyped)")
}

/// Port of checker/structure.py::_logical_assign_eq
pub fn _logical_assign_eq_with(toks: &[Token], i: i64) -> Option<i64> {
    let i = usize::try_from(i).ok()?;
    if !_LOGICAL_WORDS.contains(&toks.get(i)?.upper.as_str()) {
        return None;
    }
    let mut depth = 0;
    for k in i + 1..toks.len() {
        let t = &toks[k];
        if t.kind != "OP" {
            continue;
        }
        if t.text == "(" {
            depth += 1;
        } else if t.text == ")" {
            depth -= 1;
        } else if t.text == "=" && depth == 0 {
            let prev = &toks[k - 1];
            return if k > i + 1 && (prev.kind == "IDENT" || prev.text == ")") {
                i64::try_from(k).ok()
            } else {
                None
            };
        }
    }
    None
}

/// Port of checker/structure.py::_known_first_token
pub fn _known_first_token(text: &str, operands: &BTreeSet<String>) -> bool {
    let Ok(toks) = parser::tokenize_for_parser(text, 0) else {
        return false;
    };
    toks.iter().find(|t| t.kind != "EOF").is_some_and(|t| {
        matches!(t.kind.as_str(), "NUMBER" | "STRING")
            || (t.kind == "IDENT" && operands.contains(&t.upper))
    })
}

/// Port of checker/structure.py::_logical_led_string
pub fn _logical_led_string(text: &str) -> bool {
    static RE: LazyLock<pyre::Regex> =
        LazyLock::new(|| pyre::Regex::new(r"\s*[A-Za-z_]\w*\s+AND\b", pyre::I));
    RE.match_(text).is_some() && text.contains('"')
}

/// Port of checker/structure.py::_logical_assign_state
pub fn _logical_assign_state(_stmt: &Unparsed, _program: &Program) {
    todo!("checker/structure.py::_logical_assign_state (omitted: profile: untyped)")
}

/// Port of checker/structure.py::_logical_assign_state
#[allow(clippy::items_after_statements)]
pub fn _logical_assign_state_with(
    stmt: &StmtRef,
    program: &ProgramRef,
    profile: &'static Profile,
) -> (String, Vec<String>, bool) {
    let rejected = || ("rejected".to_owned(), Vec::new(), false);
    let Stmt::Unparsed(u) = stmt.as_ref() else {
        return rejected();
    };
    if u.reason != parser::helpers::LOGICAL_ASSIGN {
        return rejected();
    }
    let ctx = context::of(program, Some(profile), None);
    let sub_stmt = ctx.enclosing_sub(stmt);
    let sub = sub_stmt.as_ref().and_then(|s| {
        if let Stmt::SubDef(s) = s.as_ref() {
            Some(s)
        } else {
            None
        }
    });
    let local: BTreeSet<String> = sub
        .map(|s| s.params.iter().map(|p| pystr::upper(&p.name)).collect())
        .unwrap_or_default();
    let mut known = ctx.known_names().clone();
    known.names.extend(local.iter().cloned());
    let undefined = expressions::part1::_undefined_names_with(
        u.expr.as_deref(),
        &KnownView {
            names: &known.names,
            tables: &known.tables,
            dotted_tables: known.dotted_tables,
            out_of_scope: &known.out_of_scope,
        },
    );
    let mut variables = ctx.declared_variable_names().clone();
    variables.extend(local.iter().cloned());
    if let Some(sub) = sub
        && pystr::upper(&sub.keyword) == "FUNCTION"
    {
        variables.insert(pystr::upper(&sub.name));
    }
    let mut dims: OrderedMap<String, usize> = OrderedMap::new();
    let mut tables = BTreeSet::from(["STATUS".to_owned()]);
    let mut struct_vars = BTreeSet::new();
    // _VARIABLE_TYPES is private/unported in part8.
    let variable_types = ["FLOAT", "LONG", "BOOLEAN", "STRING", "DOUBLE", "UINT1"];
    for st in &program.statements {
        match st.as_ref() {
            Stmt::Declaration(d) => {
                for v in &d.vars {
                    dims.insert(pystr::upper(&v.name), v.dims.len());
                    if v.type_.as_ref().is_some_and(|t| {
                        !t.is_empty() && !variable_types.contains(&pystr::upper(t).as_str())
                    }) {
                        struct_vars.insert(pystr::upper(&v.name));
                    }
                }
            }
            Stmt::Instruction(i) if pystr::upper(&i.keyword) == "DATATABLE" => {
                if let Some(Some(a)) = i.args.first()
                    && let Expr::Name(n) = a.as_ref()
                {
                    tables.insert(pystr::upper(&n.name));
                }
            }
            Stmt::SubDef(s) => {
                for p in &s.params {
                    if p.type_.as_ref().is_some_and(|t| {
                        !t.is_empty() && !variable_types.contains(&pystr::upper(t).as_str())
                    }) {
                        struct_vars.insert(pystr::upper(&p.name));
                    }
                }
            }
            _ => {}
        }
    }
    let mut base_undefined = false;
    if profile.single_pass && pystr::lstrip(&u.base.text).starts_with('(') {
        return rejected();
    }
    /// Port of checker/structure.py::_logical_assign_state.public_member
    fn public_member(m: &Member, variables: &BTreeSet<String>) -> bool {
        matches!(m.base.as_deref(), Some(Expr::Name(n)) if pystr::upper(&n.name) == "PUBLIC" && variables.contains(&pystr::upper(&m.member)))
    }
    /// Port of checker/structure.py::_logical_assign_state.members_ok
    fn members_ok(
        e: Option<&Expr>,
        variables: &BTreeSet<String>,
        tables: &BTreeSet<String>,
        struct_vars: &BTreeSet<String>,
        undefined: &[UndefinedName],
    ) -> bool {
        match e {
            Some(Expr::Member(m)) => {
                public_member(m, variables)
                    || matches!(m.base.as_deref(), Some(Expr::Name(n)) if tables.contains(&pystr::upper(&n.name)) || struct_vars.contains(&pystr::upper(&n.name)) || undefined.iter().any(|u| u.as_str() == n.name))
            }
            Some(Expr::Binary(b)) => {
                members_ok(b.left.as_deref(), variables, tables, struct_vars, undefined)
                    && members_ok(
                        b.right.as_deref(),
                        variables,
                        tables,
                        struct_vars,
                        undefined,
                    )
            }
            Some(Expr::Unary(u)) => members_ok(
                u.operand.as_deref(),
                variables,
                tables,
                struct_vars,
                undefined,
            ),
            Some(Expr::Index(i)) => i
                .args
                .iter()
                .flatten()
                .all(|a| members_ok(Some(a), variables, tables, struct_vars, undefined)),
            _ => true,
        }
    }
    /// Port of checker/structure.py::_logical_assign_state.target_ok
    #[allow(clippy::too_many_arguments)]
    fn target_ok(
        t: Option<&Expr>,
        variables: &BTreeSet<String>,
        undefined: &[UndefinedName],
        local: &BTreeSet<String>,
        dims: &OrderedMap<String, usize>,
        tables: &BTreeSet<String>,
        profile: &Profile,
        base_undefined: &mut bool,
    ) -> bool {
        let is_undefined = |s: &str| undefined.iter().any(|u| u.as_str() == s);
        match t {
            Some(Expr::Name(n)) => {
                variables.contains(&pystr::upper(&n.name)) || is_undefined(&n.name)
            }
            Some(Expr::Index(t)) => {
                let name = pystr::upper(&t.name);
                if !variables.contains(&name) {
                    if is_undefined(&t.name) {
                        *base_undefined = true;
                        return true;
                    }
                    return false;
                }
                if !profile.single_pass || local.contains(&name) || !dims.contains_key(&name) {
                    return true;
                }
                let count = dims.get(&name).copied().unwrap_or(0);
                t.args.len() <= count && (!t.args.is_empty() || count > 0)
            }
            Some(Expr::Member(t)) => {
                public_member(t, variables)
                    || matches!(t.base.as_deref(), Some(Expr::Name(n)) if tables.contains(&pystr::upper(&n.name)) && !is_undefined(&n.name))
            }
            _ => false,
        }
    }
    /// Port of checker/structure.py::_logical_assign_state.targets_ok
    fn targets_ok(expr: Option<&Expr>, target_ok: &mut impl FnMut(Option<&Expr>) -> bool) -> bool {
        let Some(Expr::Binary(b)) = expr else {
            return true;
        };
        if b.op == "ASSIGN" && !target_ok(b.left.as_deref()) {
            return false;
        }
        targets_ok(b.left.as_deref(), target_ok) && targets_ok(b.right.as_deref(), target_ok)
    }
    if !targets_ok(u.expr.as_deref(), &mut |t| {
        target_ok(
            t,
            &variables,
            &undefined,
            &local,
            &dims,
            &tables,
            profile,
            &mut base_undefined,
        )
    }) || !members_ok(
        u.expr.as_deref(),
        &variables,
        &tables,
        &struct_vars,
        &undefined,
    ) {
        return rejected();
    }
    if undefined.is_empty() {
        return ("ok".into(), Vec::new(), false);
    }
    if undefined
        .iter()
        .any(|n| matches!(n, UndefinedName::Dotted(_)))
    {
        return rejected();
    }
    (
        "undefined".into(),
        undefined
            .into_iter()
            .map(UndefinedName::into_string)
            .collect(),
        base_undefined,
    )
}

/// Port of checker/structure.py::_logical_assign_accepted
pub fn _logical_assign_accepted(_stmt: &Unparsed, _program: &Program) -> bool {
    todo!("checker/structure.py::_logical_assign_accepted (omitted: profile: untyped)")
}

/// Port of checker/structure.py::_logical_assign_accepted
pub fn _logical_assign_accepted_with(
    stmt: &StmtRef,
    program: &ProgramRef,
    profile: &'static Profile,
) -> bool {
    _logical_assign_state_with(stmt, program, profile).0 == "ok"
}

/// Port of checker/structure.py::_logical_assign_undefined_messages
pub fn _logical_assign_undefined_messages(
    _stmt: &Unparsed,
    _program: &Program,
) -> Option<Vec<String>> {
    todo!("checker/structure.py::_logical_assign_undefined_messages (omitted: profile: untyped)")
}

/// Port of checker/structure.py::_logical_assign_undefined_messages
pub fn _logical_assign_undefined_messages_with(
    stmt: &StmtRef,
    program: &ProgramRef,
    profile: &'static Profile,
) -> Option<Vec<String>> {
    let (state, undefined, base_undefined) = _logical_assign_state_with(stmt, program, profile);
    let Stmt::Unparsed(u) = stmt.as_ref() else {
        return None;
    };
    if state != "undefined" || base_undefined || u.base.text.contains('"') {
        return None;
    }
    if profile.single_pass {
        return Some(
            undefined
                .iter()
                .map(|n| profile.unresolved_symbol_message.format(n))
                .collect(),
        );
    }
    let toks = parser::tokenize_for_parser(&u.base.text, 0).ok()?;
    let names: BTreeSet<_> = undefined.iter().map(|n| pystr::upper(n)).collect();
    if toks
        .iter()
        .filter(|t| t.kind != "EOF")
        .take_while(|t| !(t.kind == "OP" && t.text == "="))
        .any(|t| t.kind == "IDENT" && names.contains(&t.upper))
    {
        return None;
    }
    Some(vec![format!(
        "Invalid symbol in expression: {}.",
        u.base.text
    )])
}

/// Port of checker/structure.py::_unparsed_string_messages
pub fn _unparsed_string_messages(_stmt: &Unparsed, _program: &Program) -> Vec<String> {
    todo!("checker/structure.py::_unparsed_string_messages (omitted: profile: untyped)")
}

/// Port of checker/structure.py::_unparsed_string_messages
pub fn _unparsed_string_messages_with(
    stmt: &Unparsed,
    program: &ProgramRef,
    profile: &'static Profile,
) -> Vec<String> {
    if condition_part(&stmt.base.text).is_some() {
        return Vec::new();
    }
    let Some(expr) = parser::partial_expression(&stmt.base.text, profile) else {
        return Vec::new();
    };
    let assignment = Stmt::Assignment(Assignment {
        base: crate::syntax::StmtBase::new(stmt.base.node.line, &stmt.base.node.file),
        target: Some(Rc::new(Expr::Name(crate::syntax::Name {
            node: crate::syntax::Node::new(stmt.base.node.line, ""),
            name: "_".into(),
        }))),
        value: Some(expr),
        ..Default::default()
    });
    var_types::_string_operand_messages(
        &mut var_types::Typer::new(program, None, None),
        &assignment,
    )
}

/// Port of checker/structure.py::string_reached
pub fn string_reached(_text: &str, _operands: &BTreeSet<String>) -> Option<String> {
    todo!("checker/structure.py::string_reached (omitted: profile: untyped)")
}

/// Port of checker/structure.py::string_reached
pub fn string_reached_with(
    text: &str,
    operands: &BTreeSet<String>,
    profile: &Profile,
) -> Option<String> {
    if profile.single_pass {
        return None;
    }
    let e = parser::tokenize_for_parser(text, 0).err()?;
    let col = i64::try_from(e.col).ok()?;
    if e.message != "unterminated string" || pystr::char_at(text, col) != Some('"') {
        return None;
    }
    let head_text = pystr::slice(text, None, Some(col));
    let mut head = parser::tokenize_for_parser(head_text, 0).ok()?;
    head.retain(|t| t.kind != "EOF");
    let first = head.first()?;
    if first.kind == "IDENT"
        && (profile.instructions().contains(&first.upper)
            || _START_ERROR_WORDS.contains(&first.upper.as_str())
            || [
                "IF", "ELSEIF", "WHILE", "UNTIL", "SELECT", "CASE", "FOR", "NOT",
            ]
            .contains(&first.upper.as_str()))
    {
        return None;
    }
    let functions = profile.functions();
    if head.iter().any(|t| {
        t.kind == "IDENT"
            && !operands.contains(&t.upper)
            && !functions.contains(&t.upper)
            && !_BINARY_WORDS.contains(&t.upper.as_str())
    }) {
        return None;
    }
    let mut stack: Vec<(Option<&Function>, i64)> = Vec::new();
    for (j, t) in head.iter().enumerate() {
        if t.kind != "OP" {
            continue;
        }
        if t.text == "(" {
            let prev = j.checked_sub(1).and_then(|i| head.get(i));
            let rec = prev
                .filter(|p| p.kind == "IDENT" && !operands.contains(&p.upper))
                .and_then(|p| functions.get(&p.upper));
            stack.push((rec, 1));
        } else if t.text == ")" && !stack.is_empty() {
            stack.pop();
        } else if t.text == ","
            && let Some(last) = stack.last_mut()
        {
            last.1 += 1;
        }
    }
    if stack.iter().any(|(rec, n)| {
        rec.and_then(|r| r.nargs)
            .is_some_and(|count| count != 0 && *n > count)
    }) {
        return Some("many".into());
    }
    let last = head.last()?;
    if matches!(last.kind.as_str(), "IDENT" | "NUMBER") || last.kind == "OP" && last.text == ")" {
        if first.kind == "NUMBER" && head.len() == 1 {
            return Some("reached".into());
        }
        if first.kind == "IDENT"
            && operands.contains(&first.upper)
            && !profile.symbols().contains(&first.upper)
        {
            let inner = if head.len() > 2 {
                &head[2..head.len() - 1]
            } else {
                &[]
            };
            return if head.len() == 1
                || head[1].text == "("
                    && last.text == ")"
                    && inner.iter().all(|t| t.text != "(" && t.text != ")")
            {
                Some("reached".into())
            } else {
                None
            };
        }
    }
    let probe = _pass1_scan_with(&(head_text.to_owned() + " __crbunk__"), operands, profile);
    if probe.is_some_and(|p| p.0 == "pass2") {
        Some("reached".into())
    } else {
        None
    }
}

/// Port of checker/structure.py::_Everything
#[derive(Debug, Default)]
pub struct Everything {}

impl Everything {
    /// Port of checker/structure.py::_Everything::__contains__
    pub fn __contains__(&self) -> bool {
        true
    }

    /// Port of checker/structure.py::_Everything.__contains__
    #[allow(non_snake_case)]
    pub fn __contains___with<T>(&self, _item: T) -> bool {
        true
    }
}

// Retains all Python arguments for the part6 integration. The existing dependency
// omits tokens/functions and still panics; do not duplicate its implementation here.
#[allow(clippy::too_many_arguments)]
fn scan_expr_dependency(
    _toks: &[Token],
    i: usize,
    operands: &BTreeSet<String>,
    _functions: Option<&Table<Function>>,
    assign: bool,
    start_operand: bool,
    unclosed: Option<&str>,
    cond: bool,
    to_ok: bool,
    open_fallback: bool,
    symbol_unknown: bool,
) -> Option<(String, String)> {
    super::part6::_scan_expr(
        i64::try_from(i).ok()?,
        operands,
        assign,
        start_operand,
        unclosed,
        cond,
        to_ok,
        open_fallback,
        symbol_unknown,
    )
}

fn later_arg_unknown_dependency(
    _args: &[Token],
    operands: &BTreeSet<String>,
    _functions: &Table<Function>,
) -> bool {
    super::part6::_later_arg_unknown(operands)
}
