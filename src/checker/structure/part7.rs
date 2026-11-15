//! Port of `checker/structure.py` (lines 4452-5251).

#![allow(
    dead_code,
    unused_variables,
    unused_imports,
    clippy::doc_markdown,
    clippy::too_many_arguments,
    clippy::too_many_lines,
    clippy::items_after_statements
)]

use std::collections::BTreeSet;
use std::rc::Rc;

use super::CallLinks;
use super::part1::{
    _ALIAS_NOTHING_AFTER_EQ, _ALIAS_WITHOUT_EQ, _BENIGN_REASONS, _CONST_BAD_NAME, _CONST_BAD_VALUE,
    _CONST_LEAD, _CONST_LEAD_NAME, _CONST_WITHOUT_EQ, _DO_LOOP_PARAM, _EXTRA_CLOSE_PAREN,
    _FOR_MISSING, _INVALID_AS_CLAUSE, _KNOWN_PARSER_GAP_REASONS, _MAX_ARRAY_DIMS, _MAX_DIM_VALUE,
    _UNCLOSED_DIM_PREFIX, _UNCLOSED_PAREN_EXPR, _UNITS_WITHOUT_EQ, _assignment_head,
};
use super::part2::{_leftover_message, _out_of_place};
use super::part4::_leftover_first_name_known;
use super::part8::{_group_end, _group_items};
use crate::checker::expressions::part1::{
    _WHOLE_EXPR_ECHO_LOGGERS, _undefined_names, KnownView, UndefinedName,
};
use crate::checker::expressions::part2::{
    _dim_limit, _has_undefined_index, _names_and_dims, _over_indexed, DimsState, KnownNames,
    dimensions_message,
};
use crate::checker::expressions::part4::{
    _bare_function_message, _call_arity_problem, _find_bare_function_ref,
};
use crate::checker::{context, ess, instr_rules, symbols};
use crate::consteval::{self, Env};
use crate::diagnostics::ShowFile;
use crate::diagnostics::{DiagRef, Diagnostic, Diagnostics};
use crate::profiles::Profile;
use crate::syntax::{
    Assignment, Call, Case, ConstDecl, Declaration, Expr, ExprRef, Index, Instruction, Juxtaposed,
    Member, Number, OrderedMap, Program, ProgramRef, Stmt, StmtRef, SubDef, Unparsed, VarDecl,
};
use crate::syntax::{ConstValue, stmt_id};
use crate::{pyre, pystr};
use std::cell::RefCell;
use std::sync::LazyLock;

/// Port of checker/structure.py::check_missing_param_followup
pub fn check_missing_param_followup(
    program: &ProgramRef,
    profile: &'static Profile,
    diags: &mut Diagnostics,
) {
    let mut known = None;
    for pair in program.statements.windows(2) {
        let Stmt::Instruction(stmt) = pair[0].as_ref() else {
            continue;
        };
        let Some(record) = profile.instructions().get(&pystr::upper(&stmt.keyword)) else {
            continue;
        };
        let params = instr_rules::_real_params(record.params.clone());
        let required = instr_rules::_required_count(params.clone());
        if i64::try_from(stmt.args.len()).expect("argument count fits i64") >= required {
            continue;
        }
        if params
            .get(stmt.args.len())
            .is_some_and(|code| instr_rules::_TOLERANT_CODES.contains(code))
        {
            continue;
        }
        let Stmt::Unparsed(nxt) = pair[1].as_ref() else {
            continue;
        };
        if _KNOWN_PARSER_GAP_REASONS
            .iter()
            .any(|p| nxt.reason.starts_with(p))
            || _BENIGN_REASONS.contains(&nxt.reason.as_str())
        {
            continue;
        }
        if [
            _UNITS_WITHOUT_EQ,
            _CONST_WITHOUT_EQ,
            _ALIAS_WITHOUT_EQ,
            _FOR_MISSING,
        ]
        .contains(&nxt.reason.as_str())
            || [
                _INVALID_AS_CLAUSE,
                _CONST_BAD_VALUE,
                _CONST_BAD_NAME,
                _DO_LOOP_PARAM,
                _CONST_LEAD,
                _ALIAS_NOTHING_AFTER_EQ,
                _UNCLOSED_DIM_PREFIX,
            ]
            .iter()
            .any(|p| nxt.reason.starts_with(p))
        {
            continue;
        }
        let known = known.get_or_insert_with(|| {
            context::of(program, Some(profile), None)
                .known_names()
                .names
                .clone()
        });
        if !_leftover_first_name_known(&nxt.base.text, known) {
            continue;
        }
        let node = &nxt.base.node;
        if diags
            .errors()
            .iter()
            .any(|d| d.line == node.line && d.file == node.file)
        {
            continue;
        }
        if nxt.reason == _UNCLOSED_PAREN_EXPR {
            diags.error(
                node.line,
                &format!(
                    "No Matching, or out of place ) in expression: {}.",
                    nxt.base.text
                ),
                &node.file,
                ShowFile::Bool(false),
                0,
                "",
                "",
            );
        } else if nxt.reason == _EXTRA_CLOSE_PAREN
            && !_assignment_head(&nxt.base.text, program, profile)
        {
            diags.error(
                node.line,
                &_out_of_place(profile, &nxt.base.text, nxt.base.at_eol.get()),
                &node.file,
                ShowFile::Bool(false),
                0,
                "",
                "",
            );
        } else if nxt.reason == _EXTRA_CLOSE_PAREN {
            diags.error(
                node.line,
                &format!("Incomplete expression: {}.", nxt.base.text),
                &node.file,
                ShowFile::Bool(false),
                0,
                "",
                "",
            );
        } else {
            diags.error_basic(node.line, &_leftover_message(profile, &pair[1]), &node.file);
        }
    }
}

/// Port of checker/structure.py::_optional_header_error
pub fn _optional_header_error(message: &str) -> bool {
    message.starts_with("All arguments after the first optional")
        || message.starts_with("Optional parameter ")
}

/// Port of checker/structure.py::_check_call_args
pub fn _check_call_args(stmt: &Call, sub: &SubDef, diags: &mut Diagnostics) {
    if pystr::upper(&sub.keyword) != "SUB"
        || sub.header_errors.iter().any(|m| _optional_header_error(m))
    {
        return;
    }
    let given = stmt.args.len();
    if given > sub.params.len() {
        diags.error_basic(
            stmt.base.node.line,
            &format!(
                "Call to {} with {given} parameters, requires {} arguments.",
                sub.name,
                sub.params.len()
            ),
            &stmt.base.node.file,
        );
    } else if sub.params.iter().skip(given).any(|p| p.default.is_none()) {
        for _ in 0..if stmt.tail_ignored { 2 } else { 1 } {
            diags.error_basic(
                stmt.base.node.line,
                "Call argument required.",
                &stmt.base.node.file,
            );
        }
    } else {
        for (arg, param) in stmt.args.iter().zip(&sub.params) {
            if arg.is_none() && param.default.is_none() {
                diags.error_basic(
                    stmt.base.node.line,
                    "Call argument required.",
                    &stmt.base.node.file,
                );
            }
        }
    }
}

/// Port of checker/structure.py::_report_call_arg_names
pub fn _report_call_arg_names(
    stmt_ref: &StmtRef,
    known: &KnownNames,
    profile: &Profile,
    diags: &mut Diagnostics,
    is_function: bool,
    ds: Option<&DimsState>,
) {
    let Stmt::Call(stmt) = stmt_ref.as_ref() else {
        panic!("expected Call");
    };
    let node = &stmt.base.node;
    let text = &stmt.base.text;
    let at_eol = stmt.base.at_eol.get();
    let view = KnownView {
        names: &known.names,
        tables: &known.tables,
        out_of_scope: &known.out_of_scope,
        dotted_tables: known.dotted_tables,
    };
    let undefined = |e: Option<&Expr>| {
        _undefined_names(e, &view)
            .into_iter()
            .map(UndefinedName::into_string)
            .collect::<Vec<_>>()
    };
    let report_unique = |names: Vec<String>, diags: &mut Diagnostics| {
        let mut seen = BTreeSet::new();
        for name in names {
            if seen.insert(name.clone()) {
                diags.error_basic(
                    node.line,
                    &profile.unresolved_symbol_message.format(&name),
                    &node.file,
                );
            }
        }
    };
    let whole = _WHOLE_EXPR_ECHO_LOGGERS.contains(&profile.name);
    if let Some(juxtaposed) = &stmt.juxtaposed {
        let mut arg_bad: Vec<_> = stmt
            .args
            .iter()
            .flat_map(|a| undefined(a.as_deref()))
            .collect();
        if whole {
            if !arg_bad.is_empty() {
                diags.error_basic(
                    node.line,
                    &format!("Invalid symbol in expression: {text}."),
                    &node.file,
                );
                return;
            }
        } else {
            let builtin = profile.functions().contains(&pystr::upper(&stmt.name));
            if builtin && _call_arity_problem(&stmt.name, &stmt.args, profile).is_some() {
                return;
            }
            let tail = if builtin || stmt.tail_expr.is_some() {
                let exprs = if stmt.tail_expr.is_some() {
                    vec![stmt.tail_expr.clone()]
                } else {
                    stmt.args.clone()
                };
                let (names, dims) =
                    _names_and_dims(known, ds, Some(stmt_id(stmt_ref)), &[], &exprs);
                report_unique(names, diags);
                if dims {
                    diags.error_basic(
                        node.line,
                        &dimensions_message(text, profile, true, at_eol),
                        &node.file,
                    );
                    return;
                }
                arg_bad.clear();
                juxtaposed.first().filter(|a| {
                    !stmt.juxt_opaque && matches!(a.as_ref(), Expr::Name(_) | Expr::Index(_))
                })
            } else {
                juxtaposed
                    .first()
                    .filter(|a| !stmt.juxt_opaque && matches!(a.as_ref(), Expr::Name(_)))
            };
            if let Some(tail) = tail {
                arg_bad.extend(undefined(Some(tail)));
            }
            report_unique(arg_bad, diags);
        }
        let sep = if profile.fallback_echo_newline && at_eol {
            "\n"
        } else {
            ""
        };
        diags.error_basic(
            node.line,
            &format!("Incomplete expression: {text}{sep}."),
            &node.file,
        );
        return;
    }
    if stmt.tail_expr.is_some() {
        let bad = stmt
            .args
            .iter()
            .chain(std::iter::once(&stmt.tail_expr))
            .any(|a| !undefined(a.as_deref()).is_empty());
        if bad && whole {
            diags.error_basic(
                node.line,
                &format!("Invalid symbol in expression: {text}."),
                &node.file,
            );
        } else {
            let (names, dims) = _names_and_dims(
                known,
                None,
                Some(stmt_id(stmt_ref)),
                &[],
                std::slice::from_ref(&stmt.tail_expr),
            );
            report_unique(names, diags);
            if dims {
                diags.error_basic(
                    node.line,
                    &dimensions_message(text, profile, true, at_eol),
                    &node.file,
                );
            }
        }
        return;
    }
    static CALL_HEADS: LazyLock<std::sync::Mutex<OrderedMap<String, pyre::Regex>>> =
        LazyLock::new(|| std::sync::Mutex::new(OrderedMap::new()));
    let end = {
        let pattern = format!(r"\s*(?:call\s+)?{}\s*", pyre::escape(&stmt.name));
        let mut patterns = CALL_HEADS.lock().expect("regex cache lock");
        if !patterns.contains_key(&pattern) {
            patterns.insert(pattern.clone(), pyre::Regex::new(&pattern, pyre::I));
        }
        patterns
            .get(&pattern)
            .and_then(|re| re.match_(text))
            .map(|m| i64::try_from(m.end0()).expect("source length fits i64"))
    };
    let Some(end) = end else {
        return;
    };
    if !pystr::slice(text, Some(end), None).starts_with('(') {
        return;
    }
    if is_function && !whole {
        let (names, dims) = _names_and_dims(known, None, Some(stmt_id(stmt_ref)), &[], &stmt.args);
        for name in names {
            diags.error_basic(
                node.line,
                &profile.unresolved_symbol_message.format(&name),
                &node.file,
            );
        }
        if dims {
            diags.error_basic(
                node.line,
                &dimensions_message(text, profile, true, at_eol),
                &node.file,
            );
        }
        return;
    }
    let mut last_tail = None;
    let mut items = _group_items(text, end);
    let mut quirk = vec![false; items.as_ref().map_or(0, Vec::len)];
    if let Some(items) = items.as_mut()
        && items.len() < stmt.args.len()
        && !stmt.tail_ignored
    {
        let mut tail = _group_end(text, end).map_or_else(Vec::new, |close| {
            _split_top(pystr::slice(text, Some(close + 1), None), ",")
        });
        if tail.last().is_some_and(|t| pystr::strip(t).is_empty()) {
            tail.pop();
        }
        if items.len() + tail.len() == stmt.args.len() {
            quirk.extend(std::iter::repeat_n(false, tail.len().saturating_sub(1)));
            quirk.push(at_eol);
            items.extend(tail);
            last_tail = items.len().checked_sub(1);
        }
    }
    if items.is_none() && _group_end(text, end).is_none() && pystr::char_at(text, end) == Some('(')
    {
        let raw = _split_top(pystr::slice(text, Some(end + 1), None), ",");
        quirk = vec![false; raw.len().saturating_sub(1)];
        quirk.push(at_eol);
        last_tail = raw.len().checked_sub(1);
        items = Some(raw);
    }
    let Some(items) = items.filter(|i| i.len() == stmt.args.len()) else {
        return;
    };
    for (i, ((arg, raw), nl)) in stmt.args.iter().zip(items).zip(quirk).enumerate() {
        let names = undefined(arg.as_deref());
        if names.is_empty() {
            if !is_function
                && let Some(counts) = ds
                    .and_then(|d| d.dim_counts.as_ref())
                    .filter(|c| !c.counts.is_empty())
                && _over_indexed(arg.as_deref(), counts, _dim_limit(profile))
            {
                let echo = if Some(i) == last_tail {
                    pystr::lstrip(&raw)
                } else {
                    pystr::strip(&raw)
                };
                diags.error_basic(
                    node.line,
                    &dimensions_message(echo, profile, nl, nl),
                    &node.file,
                );
            }
            continue;
        }
        let echo = if Some(i) == last_tail {
            pystr::lstrip(&raw)
        } else {
            pystr::strip(&raw)
        };
        if whole && is_function {
            diags.error_basic(
                node.line,
                &format!("Invalid symbol in expression: {text}."),
                &node.file,
            );
            return;
        }
        if whole {
            diags.error_basic(
                node.line,
                &format!(
                    "Invalid symbol in expression: {echo}{}.",
                    if nl { "\n" } else { "" }
                ),
                &node.file,
            );
            continue;
        }
        for name in names {
            diags.error_basic(
                node.line,
                &profile.unresolved_symbol_message.format(&name),
                &node.file,
            );
        }
        if _has_undefined_index(arg.as_ref(), known, None, None, &[]) {
            diags.error_basic(
                node.line,
                &dimensions_message(echo, profile, nl, nl),
                &node.file,
            );
        }
    }
}

/// Port of checker/structure.py::_LinkContext
#[derive(Debug, Default)]
pub struct LinkContext {
    pub subs: BTreeSet<String>,
    pub has_routines: bool,
    pub links: Rc<RefCell<CallLinks>>,
}

/// Port of checker/structure.py::_call_link_name
pub fn _call_link_name(text: &str) -> String {
    static CALL: LazyLock<pyre::Regex> =
        LazyLock::new(|| pyre::Regex::new(r"^\s*call(?![A-Za-z0-9_])", pyre::I));
    static SPACE: LazyLock<pyre::Regex> = LazyLock::new(|| pyre::Regex::new(r"\s+", 0));
    SPACE
        .sub("", &CALL.sub("", text))
        .split('(')
        .next()
        .unwrap_or("")
        .to_owned()
}

/// Port of checker/structure.py::_bare_sub_link_name
pub fn _bare_sub_link_name(text: &str, subs: &BTreeSet<String>) -> Option<String> {
    static HEAD: LazyLock<pyre::Regex> =
        LazyLock::new(|| pyre::Regex::new(r"\s*([A-Za-z_][A-Za-z0-9_]*)\s*(?=[^\s(=])", 0));
    static SPACE: LazyLock<pyre::Regex> = LazyLock::new(|| pyre::Regex::new(r"\s+", 0));
    let m = HEAD.match_(text)?;
    if !subs.contains(&pystr::upper(m.group(1)?)) {
        return None;
    }
    Some(
        SPACE
            .sub("", text)
            .split('(')
            .next()
            .unwrap_or("")
            .to_owned(),
    )
}

/// Port of checker/structure.py::_unlinked_call
pub fn _unlinked_call(
    stmt: &Stmt,
    link: &str,
    profile: &Profile,
    diags: &mut Diagnostics,
    before_beginprog: bool,
    ctx: &LinkContext,
) -> Option<i64> {
    let node = &stmt.base().node;
    if ctx.has_routines {
        if before_beginprog {
            diags.error_basic(
                node.line,
                "Call illegally occurs before BeginProg.",
                &node.file,
            );
            return Some(node.line);
        }
        return None;
    }
    if profile.single_pass {
        let mut links = ctx.links.borrow_mut();
        if links.first.is_none() {
            links.first = Some((
                node.line,
                node.file.clone(),
                format!("Sub {link} not defined."),
            ));
        }
        return None;
    }
    diags.error(
        node.line,
        &format!("Sub {link} not defined."),
        &node.file,
        ShowFile::Bool(false),
        0,
        "",
        "",
    );
    Some(node.line)
}

/// Port of checker/structure.py::_LINK_TOLERATED
const _LINK_TOLERATED: &[&str] = &[
    "Call argument required",
    "Call to ",
    "Call illegally occurs",
    "Expr illegally occurs",
    "Could not find DataTable",
    "Sample must be used inside",
    "EndTable without",
    "DataTable is declared in the middle",
    "NextScan with no Scan",
    "EndSub without",
    "IF or SELECT without",
    "FOR without NEXT",
    "Excitation exceeds",
    "Channel ",
    "Invalid port",
];

/// Port of checker/structure.py::_link_step_runs
pub fn _link_step_runs(profile: &Profile, diags: &Diagnostics, pass1_errors: bool) -> bool {
    if profile.single_pass {
        return !diags.items.iter().any(|d| {
            d.severity == "error" && !_LINK_TOLERATED.iter().any(|p| d.message.starts_with(p))
        });
    }
    !pass1_errors
}

/// Port of checker/structure.py::commit_deferred_sub_error
pub fn commit_deferred_sub_error(links: &CallLinks, profile: &Profile, diags: &mut Diagnostics) {
    let Some((line, file, message)) = &links.first else {
        return;
    };
    if !_link_step_runs(profile, diags, false) {
        return;
    }
    let cutoff = links.positions.get(&(file.clone(), *line));
    diags.items.retain(|d| {
        d.severity != "error"
            || d.line == 0
            || cutoff
                .is_none_or(|c| links.positions.get(&(d.file.clone(), d.line)).unwrap_or(c) <= c)
    });
    diags.error(*line, message, file, ShowFile::Bool(false), 0, "", "");
}

/// Port of checker/structure.py::finalize_call_links
pub fn finalize_call_links(
    links: &CallLinks,
    profile: &Profile,
    diags: &mut Diagnostics,
    pass1_errors: bool,
) {
    let unresolved: Vec<_> = links
        .calls
        .iter()
        .filter(|n| !links.registered.contains(&pystr::upper(n)))
        .collect();
    if unresolved.is_empty() || !_link_step_runs(profile, diags, pass1_errors) {
        return;
    }
    for name in unresolved.into_iter().rev() {
        diags.error_basic(0, &format!("{name} is not declared as a Subroutine."), "");
    }
}

/// Port of checker/structure.py::_check_call_pass2
pub fn _check_call_pass2(
    stmt_ref: &StmtRef,
    subs: &BTreeSet<String>,
    profile: &Profile,
    diags: &mut Diagnostics,
    before_bp: bool,
    in_body_block: bool,
    sub_params: Option<&OrderedMap<String, SubDef>>,
    known: Option<&KnownNames>,
    link_ctx: Option<&LinkContext>,
    ds: Option<&DimsState>,
) -> Option<i64> {
    let Stmt::Call(stmt) = stmt_ref.as_ref() else {
        panic!("expected Call");
    };
    let name = pystr::upper(&stmt.name);
    let before_beginprog = !in_body_block && before_bp;
    let node = &stmt.base.node;
    let text = &stmt.base.text;
    let at_eol = stmt.base.at_eol.get();
    if stmt.explicit
        && let Some(ctx) = link_ctx
    {
        let link = _call_link_name(text);
        if ctx.has_routines {
            ctx.links.borrow_mut().calls.push(link.clone());
        }
        if !ctx.subs.contains(&pystr::upper(&link)) {
            if let Some(known) = known {
                _report_call_arg_names(stmt_ref, known, profile, diags, false, ds);
            }
            return _unlinked_call(stmt_ref, &link, profile, diags, before_beginprog, ctx);
        }
    }
    if let Some(sub) = sub_params.and_then(|p| p.get(&name)) {
        let before = diags.items.len();
        if let Some(known) = known {
            _report_call_arg_names(
                stmt_ref,
                known,
                profile,
                diags,
                pystr::upper(&sub.keyword) == "FUNCTION" && !stmt.explicit,
                ds,
            );
        }
        if !(profile.single_pass && diags.items.len() > before) {
            _check_call_args(stmt, sub, diags);
        }
    }
    if subs.contains(&name) {
        if before_beginprog {
            diags.error_basic(
                node.line,
                "Call illegally occurs before BeginProg.",
                &node.file,
            );
            return Some(node.line);
        }
        return None;
    }
    if profile.functions().contains(&name) {
        if profile.single_pass
            && let Some(known) = known
            && (stmt.tail_expr.is_some() || stmt.juxtaposed.is_some())
        {
            _report_call_arg_names(stmt_ref, known, profile, diags, true, ds);
        }
        if before_beginprog {
            diags.error_basic(
                node.line,
                "Expr illegally occurs before BeginProg.",
                &node.file,
            );
            return Some(node.line);
        }
        return None;
    }
    if profile.undefined_call_message {
        if known.is_none_or(|k| !k.names.contains(&name) || k.tables.contains(&name)) {
            diags.error_basic(
                node.line,
                &profile.unresolved_symbol_message.format(&stmt.name),
                &node.file,
            );
        }
        diags.error_basic(node.line, &_out_of_place(profile, text, at_eol), &node.file);
        return None;
    }
    static DOTTED: LazyLock<pyre::Regex> = LazyLock::new(|| pyre::Regex::new(r"[A-Za-z_]\w*\.", 0));
    static DOTTED_HEAD: LazyLock<pyre::Regex> = LazyLock::new(|| pyre::Regex::new(r"\s*\w+\.", 0));
    if DOTTED.fullmatch(&stmt.name).is_some()
        && DOTTED_HEAD.match_(text).is_some()
        && !stmt.explicit
        && let Some(known) = known
    {
        let base = pystr::upper(pystr::slice(&stmt.name, None, Some(-1)));
        if !base.is_empty()
            && !known.tables.contains(&base)
            && !symbols::_ALWAYS_KNOWN_SYSTEM_TABLES.contains(&base.as_str())
        {
            diags.error_basic(
                node.line,
                &format!(
                    "Could not find {}.",
                    pystr::slice(&stmt.name, None, Some(-1))
                ),
                &node.file,
            );
        }
    }
    diags.error(
        node.line,
        &_out_of_place(profile, text, at_eol),
        &node.file,
        ShowFile::Cr3xx,
        0,
        "",
        "",
    );
    if before_beginprog {
        diags.error_basic(
            node.line,
            "Expr illegally occurs before BeginProg.",
            &node.file,
        );
        return Some(node.line);
    }
    None
}

/// Port of checker/structure.py::_over_indexed_target
pub fn _over_indexed_target(target: &Index, ds: Option<&DimsState>) -> bool {
    let Some(counts) = ds.and_then(|d| d.dim_counts.as_ref()) else {
        return false;
    };
    let name = pystr::upper(&target.name);
    counts.counts.get(&name).is_some_and(|count| {
        i64::try_from(target.args.len()).expect("argument count fits i64") > *count
            && !(target.args.len() == 3 && counts.strings.contains(&name))
    })
}

/// Port of checker/structure.py::_check_assignment_pass2
pub fn _check_assignment_pass2(
    stmt_ref: &StmtRef,
    known: &KnownNames,
    diags: &mut Diagnostics,
    before_bp: bool,
    in_body_block: bool,
    profile: Option<&Profile>,
    ds: Option<&DimsState>,
) -> Option<i64> {
    let Stmt::Assignment(stmt) = stmt_ref.as_ref() else {
        panic!("expected Assignment");
    };
    if let Some(Expr::Member(member)) = stmt.target.as_deref()
        && let Some(Expr::Name(base)) = member.base.as_deref()
        && pystr::upper(&base.name) == "DIM"
    {
        return None;
    }
    let before_beginprog = !in_body_block && before_bp;
    let view = KnownView {
        names: &known.names,
        tables: &known.tables,
        out_of_scope: &known.out_of_scope,
        dotted_tables: known.dotted_tables,
    };
    let target_names = _undefined_names(stmt.target.as_deref(), &view);
    let node = &stmt.base.node;
    let text = &stmt.base.text;
    let at_eol = stmt.base.at_eol.get();
    if !target_names.is_empty()
        && let Some(profile) = profile.filter(|p| p.single_pass)
    {
        for name in &target_names {
            diags.error_basic(
                node.line,
                &profile.unresolved_symbol_message.format(name.as_str()),
                &node.file,
            );
        }
        if let Some(Expr::Index(target)) = stmt.target.as_deref()
            && ((!known.names.contains(&pystr::upper(&target.name))
                && _has_undefined_index(
                    stmt.target.as_ref(),
                    known,
                    ds,
                    Some(stmt_id(stmt_ref)),
                    &[],
                ))
                || _over_indexed_target(target, ds))
        {
            diags.error_basic(node.line, &_out_of_place(profile, text, at_eol), &node.file);
        } else {
            let plain_target = target_names
                .iter()
                .any(|t| !matches!(t, UndefinedName::Dotted(_)));
            for name in _undefined_names(stmt.value.as_deref(), &view) {
                if plain_target && matches!(name, UndefinedName::Dotted(_)) {
                    continue;
                }
                diags.error_basic(
                    node.line,
                    &profile.unresolved_symbol_message.format(name.as_str()),
                    &node.file,
                );
            }
            if _has_undefined_index(
                stmt.value.as_ref(),
                known,
                ds,
                Some(stmt_id(stmt_ref)),
                std::slice::from_ref(&stmt.target),
            ) {
                diags.error_basic(
                    node.line,
                    &dimensions_message(text, profile, true, at_eol),
                    &node.file,
                );
            } else if _find_bare_function_ref(stmt.value.as_deref(), profile, &BTreeSet::new()) {
                diags.error_basic(
                    node.line,
                    &_bare_function_message(text, profile, true, at_eol),
                    &node.file,
                );
            }
        }
        return None;
    }
    if !target_names.is_empty() {
        diags.error(
            node.line,
            &format!("Invalid, or out of place expression: {text}."),
            &node.file,
            ShowFile::Cr3xx,
            0,
            "",
            "",
        );
        if before_beginprog {
            diags.error_basic(
                node.line,
                "Expr illegally occurs before BeginProg.",
                &node.file,
            );
            return Some(node.line);
        }
        return None;
    }
    if !_undefined_names(stmt.value.as_deref(), &view).is_empty() {
        if before_beginprog && !profile.is_some_and(|p| p.single_pass) {
            diags.error_basic(
                node.line,
                "Expr illegally occurs before BeginProg.",
                &node.file,
            );
            return Some(node.line);
        }
        return None;
    }
    if before_beginprog {
        diags.error_basic(
            node.line,
            "Expr illegally occurs before BeginProg.",
            &node.file,
        );
        return Some(node.line);
    }
    None
}

/// Port of checker/structure.py::_check_declaration
pub fn _check_declaration(stmt: &Declaration, diags: &mut Diagnostics) -> i64 {
    diags.error_basic(
        stmt.base.node.line,
        "Variable not named.",
        &stmt.base.node.file,
    );
    stmt.base.node.line + 1
}

/// Port of checker/structure.py::_first_dim_text
pub fn _first_dim_text(stmt_text: &str, v: &VarDecl) -> String {
    v.dim0_span.map_or_else(String::new, |(start, end)| {
        pystr::slice(stmt_text, Some(start), Some(end)).to_owned()
    })
}

/// Port of checker/structure.py::_report_numeric_sub_name
pub fn _report_numeric_sub_name(
    stmt: &SubDef,
    value: Option<&ConstValue>,
    diags: &mut Diagnostics,
) {
    let numeric = match value {
        Some(ConstValue::Int(i)) => {
            if *i >= 3_000_000_000 {
                pystr::parse_float(&i.to_string())
            } else {
                None
            }
        }
        Some(ConstValue::Float(f) | ConstValue::CFloat(f)) => {
            if !pystr::is_integer(*f) || *f >= 3_000_000_000.0 {
                Some(*f)
            } else {
                None
            }
        }
        Some(ConstValue::BigInt(s)) => pystr::parse_float(s).filter(|f| *f >= 3_000_000_000.0),
        _ => None,
    };
    if let Some(value) = numeric {
        diags.error_basic(
            stmt.base.node.line,
            &format!("Parameter {} is integer type.", pystr::format_g(value, 6)),
            &stmt.base.node.file,
        );
    }
    diags.error_basic(
        stmt.base.node.line,
        &format!("Subroutine name {} illegal; cannot be numeric.", stmt.name),
        &stmt.base.node.file,
    );
}

use crate::checker::expressions::_CONST_TYPES;

/// Port of checker/structure.py::_bad_const_type
pub fn _bad_const_type(stmt: &ConstDecl) -> Option<String> {
    static AS: LazyLock<pyre::Regex> =
        LazyLock::new(|| pyre::Regex::new(r"\bAs\s+([^\s,]+)", pyre::IGNORECASE));
    let ty = stmt.type_.as_ref()?;
    if ty.is_empty() || _CONST_TYPES.contains(&pystr::upper(ty).as_str()) {
        return None;
    }
    Some(
        AS.search(&stmt.base.text)
            .and_then(|m| m.group(1).map(str::to_owned))
            .unwrap_or_else(|| ty.clone()),
    )
}

/// Port of checker/structure.py::_local_const_conflicts
pub fn _local_const_conflicts(stmt: &Stmt, declared: &OrderedMap<String, String>) -> Vec<String> {
    let declaration_vars = match stmt {
        Stmt::Declaration(d) => Some(d.vars.borrow()),
        _ => None,
    };
    let names: Vec<&str> = match stmt {
        Stmt::SubDef(s) => s.params.iter().map(|p| p.name.as_str()).collect(),
        Stmt::Declaration(s) if s.kind == "DIM" => declaration_vars
            .as_ref()
            .unwrap()
            .iter()
            .map(|v| v.name.as_str())
            .collect(),
        Stmt::ConstDecl(s) => vec![&s.name],
        _ => return Vec::new(),
    };
    names
        .into_iter()
        .filter(|name| {
            declared
                .get(&pystr::upper(name))
                .is_some_and(|v| v == "CONST")
        })
        .map(str::to_owned)
        .collect()
}

/// Port of checker/structure.py::_check_duplicate_declaration
pub fn _check_duplicate_declaration(
    stmt: &Stmt,
    declared: &mut OrderedMap<String, String>,
    diags: &mut Diagnostics,
    profile: Option<&Profile>,
) -> Option<i64> {
    let node = &stmt.base().node;
    match stmt {
        Stmt::Declaration(decl) if ["PUBLIC", "DIM"].contains(&decl.kind.as_str()) => {
            for (index, v) in decl.vars.borrow().iter().enumerate() {
                let name = pystr::upper(&v.name);
                if let Some(profile) = profile
                    && profile.functions().contains(&name)
                {
                    symbols::report_function_name_collision_with_profile(
                        stmt, &v.name, profile, diags,
                    );
                    if profile.single_pass {
                        if let Some(v) = decl.vars.borrow().get(index + 2) {
                            diags.error_basic(
                                node.line,
                                &format!("Too many parameters:  {}.", v.name),
                                &node.file,
                            );
                        }
                        return Some(i64::try_from(index).expect("variable index fits i64"));
                    }
                    continue;
                }
                let prior = declared.get(&name).map(String::as_str);
                if matches!(prior, Some("VAR" | "ALIAS" | "CONST")) {
                    let message = if prior == Some("CONST") {
                        format!("{} is already declared as a CONST.", v.name)
                    } else {
                        format!("{} already declared as a variable.", v.name)
                    };
                    diags.error_basic(node.line, &message, &node.file);
                    if profile.is_some_and(|p| p.single_pass) {
                        if let Some(v) = decl.vars.borrow().get(index + 2) {
                            diags.error_basic(
                                node.line,
                                &format!("Too many parameters:  {}.", v.name),
                                &node.file,
                            );
                        }
                        return Some(i64::try_from(index).expect("variable index fits i64"));
                    }
                } else {
                    declared.insert(name, "VAR".to_owned());
                }
                if let Some(profile) = profile
                    && symbols::report_predefined_name_collision_with_profile(
                        stmt, &v.name, profile, diags,
                    )
                    && profile.single_pass
                {
                    if let Some(v) = decl.vars.borrow().get(index + 2) {
                        diags.error_basic(
                            node.line,
                            &format!("Too many parameters:  {}.", v.name),
                            &node.file,
                        );
                    }
                    return Some(i64::try_from(index).expect("variable index fits i64"));
                }
            }
        }
        Stmt::Unparsed(u) if u.reason.starts_with(_CONST_LEAD_NAME) => {
            let name = pystr::slice(
                &u.reason,
                Some(i64::try_from(_CONST_LEAD_NAME.chars().count()).expect("prefix fits i64")),
                None,
            );
            let prior = declared.get(&pystr::upper(name)).map(String::as_str);
            if let Some(profile) = profile
                && profile.functions().contains(&pystr::upper(name))
            {
                symbols::report_function_name_collision_with_profile(stmt, name, profile, diags);
            } else if !profile.is_some_and(|p| {
                symbols::report_predefined_name_collision_with_profile(stmt, name, p, diags)
            }) {
                if matches!(prior, Some("VAR" | "ALIAS")) {
                    diags.error_basic(
                        node.line,
                        &format!("{name} already declared as a variable."),
                        &node.file,
                    );
                } else if prior == Some("CONST") {
                    diags.error_basic(
                        node.line,
                        &format!("{name} is already declared as a CONST."),
                        &node.file,
                    );
                }
            }
            diags.error_basic(node.line, "CONST needs assignment.", &node.file);
        }
        Stmt::ConstDecl(c) => {
            let name = pystr::upper(&c.name);
            if let Some(profile) = profile
                && profile.functions().contains(&name)
            {
                symbols::report_function_name_collision_with_profile(stmt, &c.name, profile, diags);
                return None;
            }
            if profile.is_some_and(|p| {
                symbols::report_predefined_name_collision_with_profile(stmt, &c.name, p, diags)
            }) {
                return None;
            }
            match declared.get(&name).map(String::as_str) {
                Some("VAR" | "ALIAS") => diags.error_basic(
                    node.line,
                    &format!("{} already declared as a variable.", c.name),
                    &node.file,
                ),
                Some("CONST") => diags.error_basic(
                    node.line,
                    &format!("{} is already declared as a CONST.", c.name),
                    &node.file,
                ),
                _ => {
                    declared.insert(name, "CONST".to_owned());
                }
            }
            if let Some(bad) = _bad_const_type(c) {
                diags.error_basic(node.line, &format!("{bad} unknown Const type"), &node.file);
            }
        }
        Stmt::Instruction(i) if pystr::upper(&i.keyword) == "READONLY" => {
            if i.args.is_empty() {
                diags.error_basic(node.line, "Missing variable name.", &node.file);
            }
            for a in &i.args {
                if let Some(Expr::Name(n)) = a.as_deref()
                    && !matches!(
                        declared.get(&pystr::upper(&n.name)).map(String::as_str),
                        Some("VAR" | "ALIAS")
                    )
                {
                    diags.error_basic(
                        node.line,
                        &format!("{} not yet declared.", n.name),
                        &node.file,
                    );
                }
            }
        }
        Stmt::Instruction(i) if pystr::upper(&i.keyword) == "ALIAS" => {
            for a in i.args.iter().skip(1) {
                if let Some(Expr::Name(n)) = a.as_deref()
                    && !declared.contains_key(&pystr::upper(&n.name))
                {
                    declared.insert(pystr::upper(&n.name), "ALIAS".to_owned());
                }
            }
        }
        Stmt::Instruction(i) if pystr::upper(&i.keyword) == "ESSVARIABLES" => {
            if let Some(profile) = profile {
                for row in ess::_table_for_profile(profile) {
                    if row.count != 0 && !declared.contains_key(&pystr::upper(&row.name)) {
                        declared.insert(pystr::upper(&row.name), "VAR".to_owned());
                    }
                }
            }
        }
        _ => {}
    }
    None
}

/// Port of checker/structure.py::_illegal_dim_message
pub fn _illegal_dim_message(stmt: &Stmt, v: Option<&VarDecl>) -> String {
    let text = stmt.text();
    if let (Stmt::Declaration(decl), Some(v)) = (stmt, v)
        && decl
            .vars
            .borrow()
            .iter()
            .take_while(|u| !std::ptr::eq(u.as_ref(), v))
            .any(|u| u.init.is_some())
        && let Some(eq) = pystr::find(text, "=", None, None)
        && let Some(comma) = pystr::find(text, ",", i64::try_from(eq).ok(), None)
    {
        return format!(
            "Illegally dimension: {}",
            pystr::rstrip(pystr::slice(text, None, i64::try_from(comma).ok()))
        );
    }
    format!(
        "Illegally dimension: {text}{}",
        if stmt.at_eol() { "\n" } else { "" }
    )
}

/// Port of checker/structure.py::_check_array_dims
pub fn _check_array_dims(
    stmt: &Declaration,
    diags: &mut Diagnostics,
    env: &Env,
    profile: Option<&Profile>,
) -> Option<i64> {
    for (index, v) in stmt.vars.borrow().iter().enumerate() {
        let before = diags.items.len();
        _check_var_dims(stmt, v, diags, env);
        if diags.items.len() > before && profile.is_some_and(|p| p.single_pass) {
            if let Some(v) = stmt.vars.borrow().get(index + 2) {
                diags.error_basic(
                    stmt.base.node.line,
                    &format!("Too many parameters:  {}.", v.name),
                    &stmt.base.node.file,
                );
            }
            return Some(i64::try_from(index).expect("variable index fits i64"));
        }
    }
    None
}

/// Port of checker/structure.py::_check_var_dims
pub fn _check_var_dims(stmt: &Declaration, v: &VarDecl, diags: &mut Diagnostics, env: &Env) {
    let node = &stmt.base.node;
    if i64::try_from(v.dims.len()).expect("dimension count fits i64") > _MAX_ARRAY_DIMS {
        diags.error_basic(
            node.line,
            &format!(
                "Variable {}({} dimension exceeds max of {_MAX_ARRAY_DIMS}.",
                v.name,
                _first_dim_text(&stmt.base.text, v)
            ),
            &node.file,
        );
        return;
    }
    if v.dims.len() == 3
        && pystr::upper(v.type_.as_deref().unwrap_or("")) == "STRING"
        && !v.paren_dim
        && let Some((start, _)) = v.dim0_span
    {
        let close = pystr::find(&stmt.base.text, ")", Some(start), None)
            .and_then(|i| i64::try_from(i).ok())
            .unwrap_or(-1);
        diags.error_basic(
            node.line,
            &format!(
                "Variable {}({}) of type String illegally dimensioned.",
                v.name,
                pystr::slice(&stmt.base.text, Some(start), Some(close))
            ),
            &node.file,
        );
        return;
    }
    for d in &v.dims {
        if matches!(d.as_ref(), Expr::String(_)) {
            diags.error_basic(
                node.line,
                &_illegal_dim_message(&Stmt::Declaration(stmt.clone()), Some(v)),
                &node.file,
            );
            continue;
        }
        let Ok(val) = consteval::value_of(Some(d), env) else {
            continue;
        };
        let illegal = match val {
            ConstValue::Int(i) => !(1..=_MAX_DIM_VALUE).contains(&i),
            ConstValue::Float(f) | ConstValue::CFloat(f) => !(1.0..=2_147_483_647.0).contains(&f),
            ConstValue::BigInt(s) => {
                pystr::parse_float(&s).is_none_or(|f| !(1.0..=2_147_483_647.0).contains(&f))
            }
            ConstValue::Str(_) => false,
        };
        if illegal {
            diags.error_basic(
                node.line,
                &_illegal_dim_message(&Stmt::Declaration(stmt.clone()), Some(v)),
                &node.file,
            );
        }
    }
}

/// Port of checker/structure.py::_split_top
#[allow(clippy::needless_continue, reason = "preserve Python branch order")]
pub fn _split_top(text: &str, sep: &str) -> Vec<String> {
    let (mut items, mut depth, mut in_str, mut start) = (Vec::new(), 0_i64, false, 0_i64);
    for (i, ch) in text.chars().enumerate() {
        let i = i64::try_from(i).expect("source length fits i64");
        if ch == '"' {
            in_str = !in_str;
        } else if in_str {
            continue;
        } else if "({".contains(ch) {
            depth += 1;
        } else if ")}".contains(ch) {
            depth -= 1;
        } else if sep.chars().count() == 1 && sep.starts_with(ch) && depth == 0 {
            items.push(pystr::slice(text, Some(start), Some(i)).to_owned());
            start = i + 1;
        }
    }
    items.push(pystr::slice(text, Some(start), None).to_owned());
    items
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn python_observed_checks() {
        let cases: serde_json::Value = serde_json::from_str(r#"[
["dims", "cr6", "Public R(0)", null, ["Illegally dimension: Public R(0)\n"]],
["dims", "cr6", "Public A = 10, R(0)", null, ["Illegally dimension: Public A = 10"]],
["dims", "cr6", "Public A = {1,2}, R(0)", null, ["Illegally dimension: Public A = {1"]],
["dims", "cr6", "Public R(2,2,2,2)", null, ["Variable R(2 dimension exceeds max of 3."]],
["dims", "cr6", "Public R(2,3,4) As String * 5", null, ["Variable R(2,3,4) of type String illegally dimensioned."]],
["dims", "cr6", "Public R(\"é\")", null, ["Illegally dimension: Public R(\"é\")\n"]],
["dims", "cr6", "Public R(1/0)", null, []],
["dims", "cr6", "Public R(0), Q(-1), Z(2)", null, ["Illegally dimension: Public R(0), Q(-1), Z(2)\n", "Illegally dimension: Public R(0), Q(-1), Z(2)\n"]],
["assign", "cr6", "Nope = Q", 1, ["Invalid, or out of place expression: Nope = Q.", "Expr illegally occurs before BeginProg."]],
["assign", "cr6", "X = Q", 1, ["Expr illegally occurs before BeginProg."]],
["assign", "cr6", "X = 1", 1, ["Expr illegally occurs before BeginProg."]],
["assign", "cr6", "Nope(2) = X", 1, ["Invalid, or out of place expression: Nope(2) = X.", "Expr illegally occurs before BeginProg."]],
["assign", "cr6", "Nope = Tmp.dat", 1, ["Invalid, or out of place expression: Nope = Tmp.dat.", "Expr illegally occurs before BeginProg."]],
["assign", "cr6", "Dim.X = 1", null, []],
["assign", "cr6", "X(BadIdx) = 2", 1, ["Invalid, or out of place expression: X(BadIdx) = 2.", "Expr illegally occurs before BeginProg."]],
["duplicates", "cr6", "Public P,Q,R\nPublic P,Q,R", null, ["P already declared as a variable.", "Q already declared as a variable.", "R already declared as a variable."]],
["duplicates", "cr6", "Const N=1\nPublic N\nConst N=2", null, ["N is already declared as a CONST.", "N is already declared as a CONST."]],
["duplicates", "cr6", "ReadOnly Nope", null, ["Nope not yet declared."]],
["duplicates", "cr6", "Public Sin, Q, R", null, ["Sin already defined as an internal function."]],
["counts", "cr6", "Sub Foo(A, Optional B=1)\nEndSub\nCall Foo()", null, ["Call argument required."]],
["counts", "cr6", "Sub Foo(A, Optional B=1)\nEndSub\nCall Foo(1,2,3)", null, ["Call to Foo with 3 parameters, requires 2 arguments."]],
["counts", "cr6", "Sub Foo(A,B)\nEndSub\nCall Foo(,2)", null, ["Call argument required."]],
["counts", "cr6", "Sub Foo(A)\nEndSub\nCall Foo() tail", null, ["Call argument required.", "Call argument required."]],
["dims", "cr1000x", "Public R(0)", null, ["Illegally dimension: Public R(0)\n"]],
["dims", "cr1000x", "Public A = 10, R(0)", null, ["Illegally dimension: Public A = 10"]],
["dims", "cr1000x", "Public A = {1,2}, R(0)", null, ["Illegally dimension: Public A = {1"]],
["dims", "cr1000x", "Public R(2,2,2,2)", null, ["Variable R(2 dimension exceeds max of 3."]],
["dims", "cr1000x", "Public R(2,3,4) As String * 5", null, ["Variable R(2,3,4) of type String illegally dimensioned."]],
["dims", "cr1000x", "Public R(\"é\")", null, ["Illegally dimension: Public R(\"é\")\n"]],
["dims", "cr1000x", "Public R(1/0)", null, []],
["dims", "cr1000x", "Public R(0), Q(-1), Z(2)", null, ["Illegally dimension: Public R(0), Q(-1), Z(2)\n", "Illegally dimension: Public R(0), Q(-1), Z(2)\n"]],
["assign", "cr1000x", "Nope = Q", 1, ["Invalid, or out of place expression: Nope = Q.", "Expr illegally occurs before BeginProg."]],
["assign", "cr1000x", "X = Q", 1, ["Expr illegally occurs before BeginProg."]],
["assign", "cr1000x", "X = 1", 1, ["Expr illegally occurs before BeginProg."]],
["assign", "cr1000x", "Nope(2) = X", 1, ["Invalid, or out of place expression: Nope(2) = X.", "Expr illegally occurs before BeginProg."]],
["assign", "cr1000x", "Nope = Tmp.dat", 1, ["Invalid, or out of place expression: Nope = Tmp.dat.", "Expr illegally occurs before BeginProg."]],
["assign", "cr1000x", "Dim.X = 1", null, []],
["assign", "cr1000x", "X(BadIdx) = 2", 1, ["Invalid, or out of place expression: X(BadIdx) = 2.", "Expr illegally occurs before BeginProg."]],
["duplicates", "cr1000x", "Public P,Q,R\nPublic P,Q,R", null, ["P already declared as a variable.", "Q already declared as a variable.", "R already declared as a variable."]],
["duplicates", "cr1000x", "Const N=1\nPublic N\nConst N=2", null, ["N is already declared as a CONST.", "N is already declared as a CONST."]],
["duplicates", "cr1000x", "ReadOnly Nope", null, ["Nope not yet declared."]],
["duplicates", "cr1000x", "Public Sin, Q, R", null, ["Sin already defined as an internal function."]],
["counts", "cr1000x", "Sub Foo(A, Optional B=1)\nEndSub\nCall Foo()", null, ["Call argument required."]],
["counts", "cr1000x", "Sub Foo(A, Optional B=1)\nEndSub\nCall Foo(1,2,3)", null, ["Call to Foo with 3 parameters, requires 2 arguments."]],
["counts", "cr1000x", "Sub Foo(A,B)\nEndSub\nCall Foo(,2)", null, ["Call argument required."]],
["counts", "cr1000x", "Sub Foo(A)\nEndSub\nCall Foo() tail", null, ["Call argument required.", "Call argument required."]],
["dims", "cr300", "Public R(0)", 0, ["Illegally dimension: Public R(0)\n"]],
["dims", "cr300", "Public A = 10, R(0)", 1, ["Illegally dimension: Public A = 10"]],
["dims", "cr300", "Public A = {1,2}, R(0)", 1, ["Illegally dimension: Public A = {1"]],
["dims", "cr300", "Public R(2,2,2,2)", 0, ["Variable R(2 dimension exceeds max of 3."]],
["dims", "cr300", "Public R(2,3,4) As String * 5", 0, ["Variable R(2,3,4) of type String illegally dimensioned."]],
["dims", "cr300", "Public R(\"é\")", 0, ["Illegally dimension: Public R(\"é\")\n"]],
["dims", "cr300", "Public R(1/0)", null, []],
["dims", "cr300", "Public R(0), Q(-1), Z(2)", 0, ["Illegally dimension: Public R(0), Q(-1), Z(2)\n", "Too many parameters:  Z."]],
["assign", "cr300", "Nope = Q", null, ["Nope is not defined.", "Q is not defined."]],
["assign", "cr300", "X = Q", null, []],
["assign", "cr300", "X = 1", 1, ["Expr illegally occurs before BeginProg."]],
["assign", "cr300", "Nope(2) = X", null, ["Nope is not defined.", "Invalid, or out of place expression: Nope(2) = X\n."]],
["assign", "cr300", "Nope = Tmp.dat", null, ["Nope is not defined.", "Tmp is not defined."]],
["assign", "cr300", "Dim.X = 1", null, []],
["assign", "cr300", "X(BadIdx) = 2", null, ["BadIdx is not defined."]],
["args", "cr300", "Fn(Q) Zork", null, ["Q is not defined.", "Zork is not defined."]],
["args", "cr300", "Fn(1)+Zork", null, ["Zork is not defined."]],
["args", "cr300", "Fn(Q)+Zork", null, ["Q is not defined.", "Zork is not defined."]],
["args", "cr300", "Fn(1) 2", null, []],
["args", "cr300", "Fn(Q) 2", null, ["Q is not defined."]],
["args", "cr300", "Fn(1) Zork(2)", null, ["Zork is not defined.", "Dimensions not declared: Fn(1) Zork(2)\n."]],
["duplicates", "cr300", "Public P,Q,R\nPublic P,Q,R", 0, ["P already declared as a variable.", "Too many parameters:  R."]],
["duplicates", "cr300", "Const N=1\nPublic N\nConst N=2", null, ["N is already declared as a CONST.", "N is already declared as a CONST."]],
["duplicates", "cr300", "ReadOnly Nope", null, ["Nope not yet declared."]],
["duplicates", "cr300", "Public Sin, Q, R", 0, ["Sin already defined as an internal function.", "Too many parameters:  R."]],
["counts", "cr300", "Sub Foo(A, Optional B=1)\nEndSub\nCall Foo()", null, ["Call argument required."]],
["counts", "cr300", "Sub Foo(A, Optional B=1)\nEndSub\nCall Foo(1,2,3)", null, ["Call to Foo with 3 parameters, requires 2 arguments."]],
["counts", "cr300", "Sub Foo(A,B)\nEndSub\nCall Foo(,2)", null, ["Call argument required."]],
["counts", "cr300", "Sub Foo(A)\nEndSub\nCall Foo() tail", null, ["Call argument required.", "Call argument required."]],
["dims", "cr350", "Public R(0)", 0, ["Illegally dimension: Public R(0)\n"]],
["dims", "cr350", "Public A = 10, R(0)", 1, ["Illegally dimension: Public A = 10"]],
["dims", "cr350", "Public A = {1,2}, R(0)", 1, ["Illegally dimension: Public A = {1"]],
["dims", "cr350", "Public R(2,2,2,2)", 0, ["Variable R(2 dimension exceeds max of 3."]],
["dims", "cr350", "Public R(2,3,4) As String * 5", 0, ["Variable R(2,3,4) of type String illegally dimensioned."]],
["dims", "cr350", "Public R(\"é\")", 0, ["Illegally dimension: Public R(\"é\")\n"]],
["dims", "cr350", "Public R(1/0)", null, []],
["dims", "cr350", "Public R(0), Q(-1), Z(2)", 0, ["Illegally dimension: Public R(0), Q(-1), Z(2)\n", "Too many parameters:  Z."]],
["assign", "cr350", "Nope = Q", null, ["Nope is not defined.", "Q is not defined."]],
["assign", "cr350", "X = Q", null, []],
["assign", "cr350", "X = 1", 1, ["Expr illegally occurs before BeginProg."]],
["assign", "cr350", "Nope(2) = X", null, ["Nope is not defined.", "Invalid, or out of place expression: Nope(2) = X\n."]],
["assign", "cr350", "Nope = Tmp.dat", null, ["Nope is not defined.", "Tmp is not defined."]],
["assign", "cr350", "Dim.X = 1", null, []],
["assign", "cr350", "X(BadIdx) = 2", null, ["BadIdx is not defined."]],
["args", "cr350", "Fn(Q) Zork", null, ["Q is not defined.", "Zork is not defined."]],
["args", "cr350", "Fn(1)+Zork", null, ["Zork is not defined."]],
["args", "cr350", "Fn(Q)+Zork", null, ["Q is not defined.", "Zork is not defined."]],
["args", "cr350", "Fn(1) 2", null, []],
["args", "cr350", "Fn(Q) 2", null, ["Q is not defined."]],
["args", "cr350", "Fn(1) Zork(2)", null, ["Zork is not defined.", "Dimensions not declared: Fn(1) Zork(2)\n."]],
["duplicates", "cr350", "Public P,Q,R\nPublic P,Q,R", 0, ["P already declared as a variable.", "Too many parameters:  R."]],
["duplicates", "cr350", "Const N=1\nPublic N\nConst N=2", null, ["N is already declared as a CONST.", "N is already declared as a CONST."]],
["duplicates", "cr350", "ReadOnly Nope", null, ["Nope not yet declared."]],
["duplicates", "cr350", "Public Sin, Q, R", 0, ["Sin already defined as an internal function.", "Too many parameters:  R."]],
["counts", "cr350", "Sub Foo(A, Optional B=1)\nEndSub\nCall Foo()", null, ["Call argument required."]],
["counts", "cr350", "Sub Foo(A, Optional B=1)\nEndSub\nCall Foo(1,2,3)", null, ["Call to Foo with 3 parameters, requires 2 arguments."]],
["counts", "cr350", "Sub Foo(A,B)\nEndSub\nCall Foo(,2)", null, ["Call argument required."]],
["counts", "cr350", "Sub Foo(A)\nEndSub\nCall Foo() tail", null, ["Call argument required.", "Call argument required."]],
["declared_args", "cr6", "Function Fn(A)\nFn=A\nEndFunction\nFn(Q) Zork", null, ["Invalid symbol in expression: Fn(Q) Zork."]],
["declared_args", "cr6", "Function Fn(A)\nFn=A\nEndFunction\nFn(1)+Zork", null, ["Invalid symbol in expression: Fn(1)+Zork."]],
["declared_args", "cr6", "Function Fn(A)\nFn=A\nEndFunction\nFn(Q)+Zork", null, ["Invalid symbol in expression: Fn(Q)+Zork."]],
["declared_args", "cr6", "Function Fn(A)\nFn=A\nEndFunction\nFn(1) 2", null, ["Incomplete expression: Fn(1) 2."]],
["declared_args", "cr6", "Function Fn(A)\nFn=A\nEndFunction\nFn(Q) 2", null, ["Invalid symbol in expression: Fn(Q) 2."]],
["declared_args", "cr6", "Function Fn(A)\nFn=A\nEndFunction\nFn(1) Zork(2)", null, ["Incomplete expression: Fn(1) Zork(2)."]],
["calls_after", "cr6", "Call Nope", 1, ["Sub Nope not defined."]],
["calls_before", "cr6", "Call Nope", 1, ["Sub Nope not defined."]],
["calls_after", "cr6", "Sub Foo(A)\nEndSub\nCall Foo()", null, ["Call argument required."]],
["calls_before", "cr6", "Sub Foo(A)\nEndSub\nCall Foo()", 3, ["Call argument required.", "Call illegally occurs before BeginProg."]],
["calls_after", "cr6", "Nope(1)", null, ["Invalid, or out of place expression: Nope(1)."]],
["calls_before", "cr6", "Nope(1)", 1, ["Invalid, or out of place expression: Nope(1).", "Expr illegally occurs before BeginProg."]],
["calls_after", "cr6", "Timer(1)", null, []],
["calls_before", "cr6", "Timer(1)", 1, ["Expr illegally occurs before BeginProg."]],
["calls_after", "cr6", "times.", null, ["Invalid, or out of place expression: times.."]],
["calls_before", "cr6", "times.", 1, ["Invalid, or out of place expression: times..", "Expr illegally occurs before BeginProg."]],
["declared_args", "cr1000x", "Function Fn(A)\nFn=A\nEndFunction\nFn(Q) Zork", null, ["Invalid symbol in expression: Fn(Q) Zork."]],
["declared_args", "cr1000x", "Function Fn(A)\nFn=A\nEndFunction\nFn(1)+Zork", null, ["Invalid symbol in expression: Fn(1)+Zork."]],
["declared_args", "cr1000x", "Function Fn(A)\nFn=A\nEndFunction\nFn(Q)+Zork", null, ["Invalid symbol in expression: Fn(Q)+Zork."]],
["declared_args", "cr1000x", "Function Fn(A)\nFn=A\nEndFunction\nFn(1) 2", null, ["Incomplete expression: Fn(1) 2."]],
["declared_args", "cr1000x", "Function Fn(A)\nFn=A\nEndFunction\nFn(Q) 2", null, ["Invalid symbol in expression: Fn(Q) 2."]],
["declared_args", "cr1000x", "Function Fn(A)\nFn=A\nEndFunction\nFn(1) Zork(2)", null, ["Incomplete expression: Fn(1) Zork(2)."]],
["calls_after", "cr1000x", "Call Nope", 1, ["Sub Nope not defined."]],
["calls_before", "cr1000x", "Call Nope", 1, ["Sub Nope not defined."]],
["calls_after", "cr1000x", "Sub Foo(A)\nEndSub\nCall Foo()", null, ["Call argument required."]],
["calls_before", "cr1000x", "Sub Foo(A)\nEndSub\nCall Foo()", 3, ["Call argument required.", "Call illegally occurs before BeginProg."]],
["calls_after", "cr1000x", "Nope(1)", null, ["Invalid, or out of place expression: Nope(1)."]],
["calls_before", "cr1000x", "Nope(1)", 1, ["Invalid, or out of place expression: Nope(1).", "Expr illegally occurs before BeginProg."]],
["calls_after", "cr1000x", "Timer(1)", null, []],
["calls_before", "cr1000x", "Timer(1)", 1, ["Expr illegally occurs before BeginProg."]],
["calls_after", "cr1000x", "times.", null, ["Invalid, or out of place expression: times.."]],
["calls_before", "cr1000x", "times.", 1, ["Invalid, or out of place expression: times..", "Expr illegally occurs before BeginProg."]],
["declared_args", "cr300", "Function Fn(A)\nFn=A\nEndFunction\nFn(Q) Zork", null, ["Q is not defined.", "Zork is not defined.", "Incomplete expression: Fn(Q) Zork\n."]],
["declared_args", "cr300", "Function Fn(A)\nFn=A\nEndFunction\nFn(1)+Zork", null, ["Zork is not defined."]],
["declared_args", "cr300", "Function Fn(A)\nFn=A\nEndFunction\nFn(Q)+Zork", null, ["Q is not defined.", "Zork is not defined."]],
["declared_args", "cr300", "Function Fn(A)\nFn=A\nEndFunction\nFn(1) 2", null, ["Incomplete expression: Fn(1) 2\n."]],
["declared_args", "cr300", "Function Fn(A)\nFn=A\nEndFunction\nFn(Q) 2", null, ["Q is not defined.", "Incomplete expression: Fn(Q) 2\n."]],
["declared_args", "cr300", "Function Fn(A)\nFn=A\nEndFunction\nFn(1) Zork(2)", null, ["Incomplete expression: Fn(1) Zork(2)\n."]],
["calls_after", "cr300", "Call Nope", null, []],
["calls_before", "cr300", "Call Nope", null, []],
["calls_after", "cr300", "Sub Foo(A)\nEndSub\nCall Foo()", null, ["Call argument required."]],
["calls_before", "cr300", "Sub Foo(A)\nEndSub\nCall Foo()", 3, ["Call argument required.", "Call illegally occurs before BeginProg."]],
["calls_after", "cr300", "Nope(1)", null, ["Nope is not defined.", "Invalid, or out of place expression: Nope(1)\n."]],
["calls_before", "cr300", "Nope(1)", null, ["Nope is not defined.", "Invalid, or out of place expression: Nope(1)\n."]],
["calls_after", "cr300", "Timer(1)", null, []],
["calls_before", "cr300", "Timer(1)", 1, ["Expr illegally occurs before BeginProg."]],
["declared_args", "cr350", "Function Fn(A)\nFn=A\nEndFunction\nFn(Q) Zork", null, ["Q is not defined.", "Zork is not defined.", "Incomplete expression: Fn(Q) Zork\n."]],
["declared_args", "cr350", "Function Fn(A)\nFn=A\nEndFunction\nFn(1)+Zork", null, ["Zork is not defined."]],
["declared_args", "cr350", "Function Fn(A)\nFn=A\nEndFunction\nFn(Q)+Zork", null, ["Q is not defined.", "Zork is not defined."]],
["declared_args", "cr350", "Function Fn(A)\nFn=A\nEndFunction\nFn(1) 2", null, ["Incomplete expression: Fn(1) 2\n."]],
["declared_args", "cr350", "Function Fn(A)\nFn=A\nEndFunction\nFn(Q) 2", null, ["Q is not defined.", "Incomplete expression: Fn(Q) 2\n."]],
["declared_args", "cr350", "Function Fn(A)\nFn=A\nEndFunction\nFn(1) Zork(2)", null, ["Incomplete expression: Fn(1) Zork(2)\n."]],
["calls_after", "cr350", "Call Nope", null, []],
["calls_before", "cr350", "Call Nope", null, []],
["calls_after", "cr350", "Sub Foo(A)\nEndSub\nCall Foo()", null, ["Call argument required."]],
["calls_before", "cr350", "Sub Foo(A)\nEndSub\nCall Foo()", 3, ["Call argument required.", "Call illegally occurs before BeginProg."]],
["calls_after", "cr350", "Nope(1)", null, ["Nope is not defined.", "Invalid, or out of place expression: Nope(1)\n."]],
["calls_before", "cr350", "Nope(1)", null, ["Nope is not defined.", "Invalid, or out of place expression: Nope(1)\n."]],
["calls_after", "cr350", "Timer(1)", null, []],
["calls_before", "cr350", "Timer(1)", 1, ["Expr illegally occurs before BeginProg."]]
]"#).unwrap();
        for row in cases.as_array().unwrap() {
            let kind = row[0].as_str().unwrap();
            let profile = crate::profiles::get(row[1].as_str().unwrap()).unwrap();
            let text = row[2].as_str().unwrap();
            let (program, _) = crate::parser::parse_text(text, "t.cr6", None, Some(profile));
            let stmt = &program.statements[0];
            let mut diags = Diagnostics::new();
            let result = match kind {
                "dims" => {
                    let Stmt::Declaration(s) = stmt.as_ref() else {
                        panic!("{row}");
                    };
                    _check_array_dims(s, &mut diags, &Env::new(profile), Some(profile))
                }
                "assign" => {
                    let known = KnownNames::new(
                        BTreeSet::from(["X".to_owned()]),
                        BTreeSet::new(),
                        false,
                        BTreeSet::new(),
                    );
                    _check_assignment_pass2(
                        stmt,
                        &known,
                        &mut diags,
                        true,
                        false,
                        Some(profile),
                        None,
                    )
                }
                "args" | "declared_args" => {
                    let known = KnownNames::new(
                        BTreeSet::from(["FN".to_owned()]),
                        BTreeSet::new(),
                        false,
                        BTreeSet::new(),
                    );
                    _report_call_arg_names(
                        if kind == "declared_args" {
                            program.statements.last().unwrap()
                        } else {
                            stmt
                        },
                        &known,
                        profile,
                        &mut diags,
                        true,
                        None,
                    );
                    None
                }
                "duplicates" => {
                    let mut declared = OrderedMap::new();
                    let mut result = None;
                    for s in &program.statements {
                        result = _check_duplicate_declaration(
                            s,
                            &mut declared,
                            &mut diags,
                            Some(profile),
                        );
                    }
                    result
                }
                "calls_before" | "calls_after" => {
                    let mut sub_params = OrderedMap::new();
                    if let Stmt::SubDef(sub) = stmt.as_ref() {
                        sub_params.insert(sub.name.to_uppercase(), sub.clone());
                    }
                    let ctx = LinkContext {
                        subs: BTreeSet::from(["FOO".to_owned()]),
                        has_routines: !sub_params.is_empty(),
                        ..LinkContext::default()
                    };
                    _check_call_pass2(
                        program.statements.last().unwrap(),
                        &ctx.subs,
                        profile,
                        &mut diags,
                        kind == "calls_before",
                        false,
                        Some(&sub_params),
                        None,
                        Some(&ctx),
                        None,
                    )
                }
                "counts" => {
                    let Stmt::SubDef(sub) = stmt.as_ref() else {
                        panic!("{row}");
                    };
                    let Stmt::Call(call) = program.statements.last().unwrap().as_ref() else {
                        panic!("{row}");
                    };
                    _check_call_args(call, sub, &mut diags);
                    None
                }
                _ => unreachable!(),
            };
            assert_eq!(serde_json::to_value(result).unwrap(), row[3], "{row}");
            let messages: Vec<_> = diags.items.iter().map(|d| d.message.as_str()).collect();
            assert_eq!(serde_json::to_value(messages).unwrap(), row[4], "{row}");
        }
    }

    #[test]
    fn python_observed_helpers() {
        let cases: serde_json::Value = serde_json::from_str(r#"[
["link", "Call S1 1", "S11", null],
["link", "Call  S2 (1)", "S2", null],
["link", "Callé(2)", "é", null],
["link", "call_1 (a)", "call_1", null],
["link", " S1 1,2", "S11,2", "S11,2"],
["link", "S1=2", "S1=2", null],
["link", "S1(2)", "S1", null],
["link", "S1", "S1", null],
["link", "S1 (2)", "S1", null],
["link", "S12", "S12", "S12"],
["split", "a,(b,c),\"d,e\",{f,g},", ["a", "(b,c)", "\"d,e\"", "{f,g}", ""]],
["split", "é,(2,3),4", ["é", "(2,3)", "4"]],
["split", "a),b,c", ["a),b,c"]],
["split", "a,\"b\"\"c,d\",e", ["a", "\"b\"\"c,d\"", "e"]],
["consttype", null, "", null],
["consttype", "LONG", "Const N As LONG =1", null],
["consttype", "uint1", "Const N As UiNt1=1", "UiNt1=1"],
["consttype", "Bogus", "", "Bogus"],
["consttype", "", "", null],
["numeric", "int", 1, ["Subroutine name N illegal; cannot be numeric."]],
["numeric", "int", 3000000000, ["Parameter 3e+09 is integer type.", "Subroutine name N illegal; cannot be numeric."]],
["numeric", "float", 1.5, ["Parameter 1.5 is integer type.", "Subroutine name N illegal; cannot be numeric."]],
["numeric", "float", -1.5, ["Parameter -1.5 is integer type.", "Subroutine name N illegal; cannot be numeric."]],
["numeric", "float", 1.0, ["Subroutine name N illegal; cannot be numeric."]],
["numeric", "float", 1e+20, ["Parameter 1e+20 is integer type.", "Subroutine name N illegal; cannot be numeric."]],
["numeric", "str", "abc", ["Subroutine name N illegal; cannot be numeric."]],
["numeric", "none", null, ["Subroutine name N illegal; cannot be numeric."]],
["local", "Sub Foo(N,N)", ["N", "N"]],
["local", "Dim N,N", ["N", "N"]],
["local", "Public N", []],
["local", "Const N=1", ["N"]],
["local", "Sub Foo(N,Q)", ["N"]]
]"#).unwrap();
        for row in cases.as_array().unwrap() {
            match row[0].as_str().unwrap() {
                "link" => {
                    let text = row[1].as_str().unwrap();
                    assert_eq!(_call_link_name(text), row[2].as_str().unwrap());
                    let subs = BTreeSet::from(["S1".to_owned(), "S12".to_owned()]);
                    assert_eq!(
                        serde_json::to_value(_bare_sub_link_name(text, &subs)).unwrap(),
                        row[3],
                        "{row}"
                    );
                }
                "split" => assert_eq!(
                    serde_json::to_value(_split_top(row[1].as_str().unwrap(), ",")).unwrap(),
                    row[2],
                    "{row}"
                ),
                "consttype" => {
                    let mut stmt = ConstDecl {
                        type_: row[1].as_str().map(str::to_owned),
                        ..ConstDecl::default()
                    };
                    stmt.base.text = row[2].as_str().unwrap().to_owned();
                    assert_eq!(
                        serde_json::to_value(_bad_const_type(&stmt)).unwrap(),
                        row[3],
                        "{row}"
                    );
                }
                "numeric" => {
                    let value = match row[1].as_str().unwrap() {
                        "int" => Some(ConstValue::Int(row[2].as_i64().unwrap())),
                        "float" => Some(ConstValue::Float(row[2].as_f64().unwrap())),
                        "str" => Some(ConstValue::Str(row[2].as_str().unwrap().to_owned())),
                        "none" => None,
                        _ => unreachable!(),
                    };
                    let mut diags = Diagnostics::new();
                    _report_numeric_sub_name(
                        &SubDef {
                            name: "N".to_owned(),
                            ..SubDef::default()
                        },
                        value.as_ref(),
                        &mut diags,
                    );
                    let messages: Vec<_> = diags.items.iter().map(|d| d.message.as_str()).collect();
                    assert_eq!(serde_json::to_value(messages).unwrap(), row[3], "{row}");
                }
                "local" => {
                    let (program, _) =
                        crate::parser::parse_text(row[1].as_str().unwrap(), "t.cr6", None, None);
                    let mut declared = OrderedMap::new();
                    declared.insert("N".to_owned(), "CONST".to_owned());
                    declared.insert("Q".to_owned(), "VAR".to_owned());
                    assert_eq!(
                        serde_json::to_value(_local_const_conflicts(
                            &program.statements[0],
                            &declared
                        ))
                        .unwrap(),
                        row[2],
                        "{row}"
                    );
                }
                _ => unreachable!(),
            }
        }
    }
}
