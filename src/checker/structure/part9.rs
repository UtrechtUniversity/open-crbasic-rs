//! Port of `checker/structure.py` (lines 5849-6309).

#![allow(dead_code, unused_variables, unused_imports)]

use std::collections::BTreeSet;
use std::sync::LazyLock;

use crate::checker::context;
use crate::checker::expressions::part1::_WHOLE_EXPR_ECHO_LOGGERS;
use crate::checker::structure::part7::_split_top;
use crate::checker::structure::part8::{_adjacent_literals, _alias_flat_offset};
use crate::checker::symbols::is_reserved_predefined_name_with_profile;
use crate::consteval;
use crate::diagnostics::{Diagnostics, ShowFile};
use crate::lexer;
use crate::parser::helpers::{CONST_INCOMPLETE, CONST_NOMATCH, UNCLOSED_DIM_PREFIX};
use crate::profiles::Profile;
use crate::pyre;
use crate::pystr;
use crate::syntax::{ConstValue, Expr, Instruction, OrderedMap, Program, ProgramRef, Unparsed};

const _KNOWN_PARSER_GAP_REASONS: &[&str] = &[
    "unexpected trailing input starting at '+='",
    "dotted SNMP OID literal",
];
const _BENIGN_REASONS: &[&str] = &["empty statement"];
const _UNITS_WITHOUT_EQ: &str = "Units without '='";
const _CONST_WITHOUT_EQ: &str = "Const without '='";
const _CONST_BAD_VALUE: &str = "Const bad value: ";
const _CONST_BAD_NAME: &str = "Const bad name: ";
const _CONST_LEAD: &str = "Const lead: ";
const _CONST_LEAD_NAME: &str = "Const lead: name ";
const _DO_LOOP_PARAM: &str = "Do/Loop parameter: ";
const _ALIAS_NOTHING_AFTER_EQ: &str = "Alias nothing after '='";
const _FOR_MISSING: &str = "For missing parameter";
const _ALIAS_WITHOUT_EQ: &str = "Alias without '='";
const _ALIAS_TARGET_NO_EQ: &str = "Alias target with no '='";
const _UNITS_TARGET_NO_EQ: &str = "Units target with no '='";
const _INVALID_AS_CLAUSE: &str = "Invalid As clause: ";

static _JUNK_INIT_HEAD: LazyLock<pyre::Regex> =
    LazyLock::new(|| pyre::Regex::new(r"(?is)\s*(CONST|PUBLIC|DIM)\s+(.*)$", 0));
static _JUNK_INIT_NAME: LazyLock<pyre::Regex> =
    LazyLock::new(|| pyre::Regex::new(r"\s*([A-Za-z_]\w*)", 0));
static _CONST_SURPLUS_NAME: LazyLock<pyre::Regex> =
    LazyLock::new(|| pyre::Regex::new(r#"\s*("[^"]*"|[A-Za-z_]\w*(?=\()|[^\s,]+)"#, 0));

fn err(diags: &mut Diagnostics, line: i64, msg: &str, file: &str) {
    diags.error(line, msg, file, ShowFile::Bool(true), 0, "", "");
}

#[allow(clippy::cast_possible_truncation)]
fn const_to_i64(v: &ConstValue) -> Option<i64> {
    match v {
        ConstValue::Int(i) => Some(*i),
        ConstValue::Float(f) | ConstValue::CFloat(f) => Some(*f as i64),
        ConstValue::BigInt(_) | ConstValue::Str(_) => None,
    }
}

/// Port of checker/structure.py::_check_alias_dims
#[allow(clippy::too_many_arguments)]
pub fn _check_alias_dims(
    stmt: &Instruction,
    diags: &mut Diagnostics,
    env: &consteval::Env,
    dim0_sizes: &OrderedMap<String, i64>,
    all_dims: &OrderedMap<String, Vec<i64>>,
    seen_alias_names: &mut BTreeSet<String>,
    seen_alias_sources: &mut OrderedMap<String, Vec<Vec<i64>>>,
    collisions: Option<&OrderedMap<usize, String>>,
    profile: Option<&Profile>,
    string_slots: Option<&OrderedMap<String, (Option<i64>, usize)>>,
    register_collisions: bool,
) {
    let line = stmt.base.node.line;
    let file = stmt.base.node.file.as_str();
    let target = stmt.args.first().and_then(|a| a.as_deref());
    let (target_name, target_args) = match target {
        Some(Expr::Index(idx)) => (idx.name.as_str(), Some(&idx.args)),
        Some(Expr::Name(n)) => (n.name.as_str(), None),
        _ => return,
    };
    let name_upper = pystr::upper(target_name);
    if !dim0_sizes.contains_key(&name_upper) {
        err(
            diags,
            line,
            &format!("{target_name} not yet declared so cannot be aliased."),
            file,
        );
        return;
    }

    if let Some(args) = target_args
        && args.is_empty()
    {
        err(diags, line, "Need closing )", file);
        err(diags, line, "Alias is illegally dimensioned.", file);
        return;
    }

    let (words, rank) = string_slots
        .and_then(|m| m.get(&name_upper).copied())
        .unwrap_or((Some(1), 0));
    let has_rank = string_slots.is_some_and(|m| m.contains_key(&name_upper));
    if has_rank
        && let Some(args) = target_args
        && args.len() > rank
    {
        err(diags, line, "Alias is illegally dimensioned.", file);
        return;
    }

    let mut index_key: Option<Vec<i64>> = Some(Vec::new());
    let mut base: i64 = 1;
    let mut total: Option<i64> = None;

    if let Some(args) = target_args {
        let mut key = Vec::new();
        for a in args {
            if let Ok(v) = consteval::value_of(a.as_deref(), env)
                && let Some(iv) = const_to_i64(&v)
            {
                key.push(iv);
            } else {
                index_key = None;
                break;
            }
        }
        if index_key.is_some() {
            index_key = Some(key);
        }

        if let (Some(key), Some(raw_dims)) = (&index_key, all_dims.get(&name_upper))
            && !key.is_empty()
            && key.len() <= raw_dims.len()
        {
            let dims: Vec<i64> = raw_dims.iter().map(|&d| d.max(1)).collect();
            let mut tot: i64 = 1;
            for &d in &dims {
                tot *= d;
            }
            total = Some(tot);
            let mut padded = key.clone();
            padded.resize(dims.len(), 1);
            base = _alias_flat_offset(padded, dims);
            if !(1 <= base && base <= tot) {
                err(diags, line, "Illegally dimensioned aliased name", file);
            }
        }
    } else if index_key.as_ref().is_some_and(Vec::is_empty)
        && dim0_sizes.contains_key(&name_upper)
        && let Some(raw_dims) = all_dims.get(&name_upper)
        && !raw_dims.is_empty()
    {
        let mut tot: i64 = 1;
        for &d in raw_dims {
            tot *= d.max(1);
        }
        base = 1;
        total = Some(tot);
    }

    if let Some(w) = words {
        if let Some(tot) = total {
            base = (base - 1) * w + 1;
            total = Some(tot * w);
        }
    } else {
        total = None;
    }

    let first = stmt.args.get(1).and_then(|a| a.as_deref());
    let first_junk = stmt
        .alias_item_junk
        .as_ref()
        .and_then(|v| v.first().copied())
        .unwrap_or(true);
    if total.is_some()
        && let Some(Expr::Index(first_idx)) = first
        && first_idx.args.len() != 1
        && !first_junk
    {
        err(diags, line, "Need closing )", file);
        err(diags, line, "Alias is illegally dimensioned.", file);
        return;
    }

    let sizes = if total.is_some() {
        _alias_name_sizes(stmt, env)
    } else {
        None
    };
    let mut cur = base;
    if let (Some(sz), Some(tot), Some(_w)) = (&sizes, total, words)
        && !_alias_dim_ok(
            stmt,
            diags,
            cur,
            sz[0],
            tot,
            false,
            has_rank && matches!(first, Some(Expr::Index(_))),
        )
    {
        return;
    }

    let empty_collisions = OrderedMap::default();
    let collisions = collisions.unwrap_or(&empty_collisions);
    let name_collision = collisions.get(&0);
    let predefined = name_collision.is_some_and(|s| s.ends_with("predefined CONST."));
    if predefined && let Some(msg) = name_collision {
        err(diags, line, msg, file);
    }

    let mut dedup_key = index_key.clone();
    if let (Some(key), Some(raw_dims)) = (&index_key, all_dims.get(&name_upper))
        && key.len() <= raw_dims.len()
    {
        let dims: Vec<i64> = raw_dims.iter().map(|&d| d.max(1)).collect();
        let mut padded = key.clone();
        padded.resize(dims.len(), 1);
        let flat = _alias_flat_offset(padded, dims);
        if flat >= 1 {
            dedup_key = Some(vec![flat]);
        }
    }

    if let Some(dk) = dedup_key {
        let mut bucket = seen_alias_sources
            .get(&name_upper)
            .cloned()
            .unwrap_or_default();
        if bucket.contains(&dk) {
            err(diags, line, "Duplicate Alias", file);
        } else {
            bucket.push(dk);
            seen_alias_sources.insert(name_upper.clone(), bucket);
        }
    }

    if let Some(msg) = name_collision
        && !predefined
    {
        err(diags, line, msg, file);
    }

    let lead = _alias_name_lead(stmt);
    for (k, rhs_opt) in stmt.args.iter().skip(1).enumerate() {
        let Some(rhs) = rhs_opt.as_deref() else {
            continue;
        };
        if let (Some(sz), Some(tot), Some(w)) = (&sizes, total, words)
            && k > 0
        {
            cur += sz[k - 1] * w;
            if !_alias_dim_ok(
                stmt,
                diags,
                cur,
                sz[k],
                tot,
                true,
                has_rank && matches!(rhs, Expr::Index(_)),
            ) {
                return;
            }
            if let Expr::Name(n) = rhs
                && let Some(prof) = profile
                && is_reserved_predefined_name_with_profile(&n.name, prof)
            {
                err(
                    diags,
                    line,
                    &format!("{} is a predefined CONST.", n.name),
                    file,
                );
            }
        }
        if k > 0
            && let Some(msg) = collisions.get(&k)
        {
            err(diags, line, msg, file);
        }
        let rhs_name = match rhs {
            Expr::Name(n) => n.name.as_str(),
            Expr::Index(idx) if sizes.is_some() => idx.name.as_str(),
            _ => continue,
        };
        let rhs_upper = pystr::upper(rhs_name);
        if let Some(msg) = collisions.get(&k)
            && !msg.ends_with("predefined CONST.")
        {
            if register_collisions {
                seen_alias_names.insert(rhs_upper);
            }
            continue;
        }
        if seen_alias_names.contains(&rhs_upper) {
            let (blanks, written_name) = lead
                .get(k)
                .map_or(("", rhs_name), |(b, w)| (b.as_str(), w.as_str()));
            err(
                diags,
                line,
                &format!("Duplicate Alias Name: {blanks}{written_name}."),
                file,
            );
        } else {
            seen_alias_names.insert(rhs_upper);
        }
    }

    if let (Some(sz), Some(tot), Some(w)) = (&sizes, total, words)
        && pystr::rstrip(&stmt.base.text).ends_with(',')
    {
        let next_cur = cur + sz[sz.len() - 1] * w;
        if !(1 <= next_cur && next_cur <= tot) {
            err(diags, line, "Illegally dimensioned aliased name", file);
        }
    }
}

/// Port of checker/structure.py::_alias_name_sizes
pub fn _alias_name_sizes(stmt: &Instruction, env: &consteval::Env) -> Option<Vec<i64>> {
    let names = &stmt.args[1..];
    let junk = stmt.alias_item_junk.as_ref()?;
    if names.is_empty() || junk.iter().any(|&j| j) || junk.len() != names.len() {
        return None;
    }
    let mut out = Vec::with_capacity(names.len());
    for a in names {
        match a.as_deref() {
            Some(Expr::Name(_)) => out.push(1),
            Some(Expr::Index(idx)) if idx.args.len() == 1 && idx.args[0].is_some() => {
                let v = consteval::value_of(idx.args[0].as_deref(), env).ok()?;
                let iv = const_to_i64(&v)?;
                out.push(iv);
            }
            _ => return None,
        }
    }
    Some(out)
}

/// Port of checker/structure.py::_alias_dim_ok
pub fn _alias_dim_ok(
    stmt: &Instruction,
    diags: &mut Diagnostics,
    cur: i64,
    n: i64,
    total: i64,
    check_start: bool,
    any_start: bool,
) -> bool {
    let line = stmt.base.node.line;
    let file = stmt.base.node.file.as_str();
    if check_start && !(1 <= cur && cur <= total) {
        err(diags, line, "Illegally dimensioned aliased name", file);
    }
    if n < 1 || ((cur >= 1 || any_start) && n > total + cur - 1) {
        err(diags, line, "Alias is illegally dimensioned.", file);
        return false;
    }
    true
}

/// Port of checker/structure.py::_alias_name_lead
pub fn _alias_name_lead(stmt: &Instruction) -> Vec<(String, String)> {
    let text = stmt.base.text.as_str();
    let Some(eq) = text.find('=') else {
        return Vec::new();
    };
    let mut parts = Vec::new();
    let mut depth: i32 = 0;
    let mut cur = String::new();
    for ch in text[eq + 1..].chars() {
        if ch == '(' {
            depth += 1;
        } else if ch == ')' {
            depth -= 1;
        }
        if ch == ',' && depth == 0 {
            parts.push(cur);
            cur = String::new();
        } else {
            cur.push(ch);
        }
    }
    parts.push(cur);
    parts
        .into_iter()
        .enumerate()
        .map(|(k, p)| {
            let trimmed_start = pystr::lstrip(&p);
            let blanks = if k == 0 {
                String::new()
            } else {
                p[..p.len() - trimmed_start.len()].to_string()
            };
            (blanks, pystr::strip(&p).to_string())
        })
        .collect()
}

/// Port of checker/structure.py::_after_keyword
pub fn _after_keyword(stmt: &Unparsed) -> String {
    let stripped = pystr::lstrip(&stmt.base.text);
    pystr::strip(pystr::slice(stripped, Some(5), None)).to_string()
}

/// Port of checker/structure.py::_junk_init_findings
pub fn _junk_init_findings(stmt: &Unparsed, profile: &Profile) -> Vec<String> {
    let head = _JUNK_INIT_HEAD.match_(&stmt.base.text);
    let mut out = Vec::new();
    if stmt.reason.starts_with(_CONST_BAD_VALUE) {
        return out;
    }
    if pystr::lstrip(&stmt.base.text).starts_with('#') {
        let nl = if stmt.base.at_eol.get() { "\n" } else { "" };
        return vec![format!("Unknown instruction: {}{nl}.", stmt.base.text)];
    }
    let Some(head) = head else {
        return out;
    };
    let kw = head.group(1).unwrap_or("");
    let body = head.group(2).unwrap_or("");
    let is_const = pystr::upper(kw) == "CONST";
    for seg in _split_top(body, ",") {
        let name = _JUNK_INIT_NAME.match_(&seg);
        let Some(name_m) = name else {
            continue;
        };
        let Some((_, rhs)) = seg.split_once('=') else {
            continue;
        };
        let value = pystr::strip(rhs);
        let items = if value.starts_with('{') && value.ends_with('}') {
            _split_top(&value[1..value.len() - 1], ",")
        } else {
            vec![value.to_string()]
        };
        let bad = items
            .into_iter()
            .find(|i| _adjacent_literals(i))
            .map(|i| pystr::strip(&i).to_string());
        let Some(bad) = bad else {
            continue;
        };
        out.push(format!("Incomplete expression: {bad}."));
        if !is_const {
            let ident = name_m.group(1).unwrap_or("");
            out.push(format!(
                "Variable initialization of {ident} has bad syntax."
            ));
        }
        if is_const || profile.single_pass {
            break;
        }
    }
    out
}

/// Port of checker/structure.py::_const_undefined_names
pub fn _const_undefined_names(
    text: &str,
    program: &ProgramRef,
    profile: &'static Profile,
) -> Vec<String> {
    let Ok(toks) = lexer::tokenize(text, 0) else {
        return Vec::new();
    };
    let ctx = context::of(program, Some(profile), None);
    let known = ctx.known_names();
    let mut out = Vec::new();
    for (i, t) in toks.iter().enumerate() {
        if t.kind != "IDENT" || known.names.contains(&t.upper) {
            continue;
        }
        let start = i.saturating_sub(1);
        let end = (i + 2).min(toks.len());
        if toks[start..end]
            .iter()
            .any(|o| o.kind == "OP" && o.text == ".")
        {
            return Vec::new();
        }
        out.push(t.text.clone());
    }
    out
}

/// Port of checker/structure.py::_check_unparsed_pass1
pub fn _check_unparsed_pass1(
    stmt: &Unparsed,
    diags: &mut Diagnostics,
    program: &ProgramRef,
    profile: &'static Profile,
) {
    let line = stmt.base.node.line;
    let file = stmt.base.node.file.as_str();
    if _KNOWN_PARSER_GAP_REASONS
        .iter()
        .any(|r| stmt.reason.starts_with(r))
        || _BENIGN_REASONS.contains(&stmt.reason.as_str())
    {
        return;
    }
    if stmt.reason == _UNITS_WITHOUT_EQ {
        err(
            diags,
            line,
            &format!("Need '=' in units definition: {}.", _after_keyword(stmt)),
            file,
        );
        return;
    }
    if stmt.reason == _UNITS_TARGET_NO_EQ {
        err(
            diags,
            line,
            &format!("Need '=' in units definition: {}\n.", stmt.base.text),
            file,
        );
        return;
    }
    if stmt.reason == _CONST_WITHOUT_EQ {
        let rest = _after_keyword(stmt);
        err(diags, line, "Const with no assignment.", file);
        if !rest.contains('=')
            && let Some(named) = _CONST_SURPLUS_NAME.match_(&rest)
        {
            let g1 = named.group(1).unwrap_or("");
            err(diags, line, &format!("Too many parameters:  {g1}."), file);
        }
        return;
    }
    if stmt.reason == _ALIAS_NOTHING_AFTER_EQ {
        let stripped = pystr::lstrip(&stmt.base.text);
        let body = pystr::slice(stripped, Some(5), None);
        let eq = body.find('=').unwrap_or(body.len());
        let mut target = pystr::lstrip(&body[..eq]);
        if !target.contains('(') {
            target = pystr::strip(target);
        }
        err(
            diags,
            line,
            &format!("Nothing after '=' in alias definition: {target}."),
            file,
        );
        return;
    }
    if let Some(msg) = stmt.reason.strip_prefix(_DO_LOOP_PARAM) {
        err(diags, line, msg, file);
        return;
    }
    if stmt.reason.starts_with(_CONST_LEAD_NAME) {
        return;
    }
    if let Some(rest) = stmt.reason.strip_prefix(_CONST_LEAD) {
        for message in rest.split('\n') {
            err(diags, line, message, file);
        }
        return;
    }
    if let Some(bad_name) = stmt.reason.strip_prefix(_CONST_BAD_NAME) {
        err(
            diags,
            line,
            &format!("Const name {bad_name} illegal, starts with a number."),
            file,
        );
        return;
    }
    if let Some(mut text) = stmt.reason.strip_prefix(_CONST_BAD_VALUE) {
        let mut generic_prefix = "Invalid, or out of place expression: ";
        if let Some(rest) = text.strip_prefix(CONST_NOMATCH) {
            text = rest;
            generic_prefix = "No Matching, or out of place ) in expression: ";
        } else if let Some(rest) = text.strip_prefix(CONST_INCOMPLETE) {
            text = rest;
            generic_prefix = "Incomplete expression: ";
        }
        let names = _const_undefined_names(text, program, profile);
        if !names.is_empty() && _WHOLE_EXPR_ECHO_LOGGERS.contains(&profile.name) {
            let msg = profile.unresolved_symbol_message.format(text);
            err(diags, line, &msg, file);
            return;
        }
        for name in names {
            let msg = profile.unresolved_symbol_message.format(&name);
            err(diags, line, &msg, file);
        }
        err(diags, line, &format!("{generic_prefix}{text}."), file);
        return;
    }
    if stmt.reason == _FOR_MISSING {
        err(diags, line, "Invalid or missing parameter.", file);
        return;
    }
    if stmt.reason == _ALIAS_WITHOUT_EQ {
        err(
            diags,
            line,
            &format!("Need '=' in alias definition: {}.", _after_keyword(stmt)),
            file,
        );
        return;
    }
    if stmt.reason == _ALIAS_TARGET_NO_EQ {
        err(
            diags,
            line,
            &format!("Need '=' in alias definition: {}\n.", _after_keyword(stmt)),
            file,
        );
        return;
    }
    if let Some(bad_token) = stmt.reason.strip_prefix(_INVALID_AS_CLAUSE) {
        err(
            diags,
            line,
            &format!("Invalid Variable declaration {bad_token}."),
            file,
        );
        return;
    }
    if let Some(text) = stmt.reason.strip_prefix(UNCLOSED_DIM_PREFIX) {
        err(
            diags,
            line,
            &format!("{text} is missing the ) character."),
            file,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profiles;
    use crate::syntax::{Expr, ExprRef, Index, Name, Node, Number, NumberValue, StmtBase};
    use std::rc::Rc;

    fn make_unparsed(text: &str, reason: &str, at_eol: bool) -> Unparsed {
        Unparsed {
            base: StmtBase {
                node: Node {
                    line: 1,
                    file: String::new(),
                },
                text: text.to_string(),
                at_eol: std::cell::Cell::new(at_eol),
                ..Default::default()
            },
            reason: reason.to_string(),
            ..Default::default()
        }
    }

    fn make_alias_instr(text: &str, args: Vec<Option<ExprRef>>, junk: Vec<bool>) -> Instruction {
        Instruction {
            base: StmtBase {
                node: Node {
                    line: 1,
                    file: String::new(),
                },
                text: text.to_string(),
                ..Default::default()
            },
            keyword: "Alias".to_string(),
            args,
            alias_item_junk: Some(junk),
            ..Default::default()
        }
    }

    #[test]
    fn test_after_keyword() {
        let u = make_unparsed("  Units   foo ", _UNITS_WITHOUT_EQ, false);
        assert_eq!(_after_keyword(&u), "foo");
    }

    #[test]
    fn test_alias_dim_ok() {
        let stmt = make_alias_instr("Alias a = b", vec![], vec![]);
        let mut diags = Diagnostics::new();
        assert!(_alias_dim_ok(&stmt, &mut diags, 1, 2, 4, true, false));
        assert_eq!(diags.items.len(), 0);

        let mut diags2 = Diagnostics::new();
        assert!(!_alias_dim_ok(&stmt, &mut diags2, 5, 0, 4, true, false));
        assert_eq!(diags2.items.len(), 2);
    }

    #[test]
    fn test_alias_name_lead() {
        let stmt = make_alias_instr("Alias a =  b,  c(2)", vec![], vec![]);
        let lead = _alias_name_lead(&stmt);
        assert_eq!(
            lead,
            vec![
                (String::new(), "b".to_string()),
                ("  ".to_string(), "c(2)".to_string()),
            ]
        );
    }

    #[test]
    fn test_alias_name_sizes() {
        let cr6 = profiles::get("cr6").unwrap();
        let env = consteval::Env::new(cr6);
        let stmt = make_alias_instr(
            "Alias a = b, c(3)",
            vec![
                Some(Rc::new(Expr::Name(Name {
                    node: Node {
                        line: 1,
                        file: String::new(),
                    },
                    name: "a".to_string(),
                }))),
                Some(Rc::new(Expr::Name(Name {
                    node: Node {
                        line: 1,
                        file: String::new(),
                    },
                    name: "b".to_string(),
                }))),
                Some(Rc::new(Expr::Index(Index {
                    node: Node {
                        line: 1,
                        file: String::new(),
                    },
                    name: "c".to_string(),
                    args: vec![Some(Rc::new(Expr::Number(Number {
                        node: Node {
                            line: 1,
                            file: String::new(),
                        },
                        value: NumberValue::Int(3),
                        text: "3".to_string(),
                    })))],
                }))),
            ],
            vec![false, false],
        );
        assert_eq!(_alias_name_sizes(&stmt, &env), Some(vec![1, 3]));
    }

    #[test]
    fn test_junk_init_findings_hash() {
        let cr6 = profiles::get("cr6").unwrap();
        let u = make_unparsed("# 1", "unknown", true);
        assert_eq!(
            _junk_init_findings(&u, cr6),
            vec!["Unknown instruction: # 1\n.".to_string()]
        );
    }

    #[test]
    fn test_check_unparsed_pass1() {
        let cr6 = profiles::get("cr6").unwrap();
        let prog = Rc::new(Program::default());
        let mut diags = Diagnostics::new();
        let u = make_unparsed("Units foo", _UNITS_WITHOUT_EQ, false);
        _check_unparsed_pass1(&u, &mut diags, &prog, cr6);
        assert_eq!(diags.items.len(), 1);
        assert_eq!(diags.items[0].message, "Need '=' in units definition: foo.");
    }
}
