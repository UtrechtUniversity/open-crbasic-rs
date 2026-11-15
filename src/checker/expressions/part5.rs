//! Port of `checker/expressions.py` (lines 3084-3758).

#![allow(
    dead_code,
    unused_variables,
    unused_imports,
    clippy::used_underscore_items,
    clippy::too_many_lines
)]

use std::collections::{BTreeSet, HashMap};
use std::rc::Rc;
use std::sync::LazyLock;

use crate::checker::context;
use crate::consteval::{self, Env, NotConstant};
use crate::diagnostics::{DiagRef, Diagnostic, Diagnostics};
use crate::profiles::Profile;
use crate::syntax::{
    Assignment, Call, Case, CaseItem, ConstDecl, ConstValue, Declaration, Expr, ExprRef, Index,
    Instruction, Juxtaposed, Member, Number, NumberValue, OrderedMap, Program, ProgramRef, Stmt,
    StmtRef, SubDef, Unparsed, VarDecl, stmt_id,
};
use crate::{pyre, pystr};

fn is_whole_expr_echo_logger(profile: &Profile) -> bool {
    matches!(profile.name, "cr6" | "cr1000x")
}

fn const_as_exact_int(val: &ConstValue) -> Option<i64> {
    match val {
        ConstValue::Int(i) => Some(*i),
        ConstValue::Float(f) | ConstValue::CFloat(f) => {
            if pystr::is_integer(*f) {
                pystr::py_int_from_f64(*f)
            } else {
                None
            }
        }
        _ => None,
    }
}

fn const_is_positive(val: &ConstValue) -> bool {
    match val {
        ConstValue::Int(i) => *i > 0,
        ConstValue::Float(f) | ConstValue::CFloat(f) => *f > 0.0,
        _ => false,
    }
}

/// Port of `checker/expressions.py::check_function_call_counts`.
pub fn check_function_call_counts(_program: &Program, _diags: &mut Diagnostics) {
    todo!("checker/expressions.py::check_function_call_counts (omitted: profile: untyped)")
}

#[allow(clippy::too_many_arguments)]
fn visit_call_count_stmt(
    stmt: &Stmt,
    functions: &HashMap<String, &SubDef>,
    known: &BTreeSet<String>,
    profile: &'static Profile,
    missing: &mut usize,
    diags: &mut Diagnostics,
    problem: &dyn Fn(Option<&Expr>, &mut usize) -> Option<&'static str>,
    report: &dyn Fn(&Stmt, &str, &str, bool, usize, &mut Diagnostics),
) {
    match stmt {
        Stmt::Assignment(a) => {
            let mut kind = problem(a.target.as_deref(), missing)
                .or_else(|| problem(a.value.as_deref(), missing));
            let target_single_pass_known = profile.single_pass
                && match a.target.as_deref() {
                    Some(Expr::Name(_)) => true,
                    Some(Expr::Index(ix)) => known.contains(&pystr::upper(&ix.name)),
                    _ => false,
                };
            if kind.is_some()
                && !super::_undefined_names(a.target.as_deref(), known).is_empty()
                && !target_single_pass_known
            {
                kind = None;
            }
            if let Some(k) = kind {
                report(stmt, k, stmt.text(), true, *missing, diags);
            }
        }
        Stmt::If(if_stmt) => {
            let kind = problem(if_stmt.cond.as_deref(), missing);
            let echo = super::_if_condition_text(stmt);
            if let (Some(k), Some(echo)) = (kind, echo) {
                report(stmt, k, &echo, false, *missing, diags);
            }
            for inner in if_stmt
                .inline
                .iter()
                .flatten()
                .chain(if_stmt.inline_else.iter().flatten())
            {
                visit_call_count_stmt(
                    inner, functions, known, profile, missing, diags, problem, report,
                );
            }
        }
        Stmt::Instruction(inst) => {
            for (i, arg) in inst.args.iter().enumerate() {
                if let Some(k) = problem(arg.as_deref(), missing) {
                    let arg_i = i64::try_from(i).unwrap_or(i64::MAX);
                    let echo_owned = super::_arg_text(inst, arg_i);
                    report(stmt, k, pystr::strip(&echo_owned), false, *missing, diags);
                    break;
                }
            }
        }
        Stmt::Call(call) => {
            if !call.explicit
                && call.tail_expr.is_some()
                && let Some(k) = problem(call.tail_expr.as_deref(), missing)
            {
                report(stmt, k, stmt.text(), true, *missing, diags);
                return;
            }
            let fn_def = functions.get(&pystr::upper(&call.name)).copied();
            if fn_def.is_none() && call.tail_expr.is_none() && call.juxtaposed.is_none() {
                for (i, arg) in call.args.iter().enumerate() {
                    if let Some(k) = problem(arg.as_deref(), missing) {
                        let arg_i = i64::try_from(i).unwrap_or(i64::MAX);
                        let echo_owned = super::_call_arg_text(stmt.text(), arg_i);
                        report(stmt, k, pystr::strip(&echo_owned), false, *missing, diags);
                        return;
                    }
                }
            }
            let name_len = i64::try_from(pystr::len(&call.name)).unwrap_or(i64::MAX);
            let after_name = pystr::slice(pystr::lstrip(stmt.text()), Some(name_len), None);
            let bare = call.args.is_empty() && !pystr::lstrip(after_name).starts_with('(');
            if !call.explicit
                && let Some(fn_def) = fn_def
                && !bare
            {
                *missing = fn_def.missing_required_params(&call.args);
                let kind = if *missing > 0 {
                    Some("few")
                } else if call.args.len() > fn_def.params.len() {
                    Some("many")
                } else {
                    None
                };
                if let Some(k) = kind {
                    report(stmt, k, stmt.text(), true, *missing, diags);
                } else {
                    let pat = format!(r"\s*{}\s*(?=\()", pyre::escape(&call.name));
                    let re = pyre::Regex::new(&pat, pyre::I);
                    let unclosed = if let Some(m) = re.match_(stmt.text())
                        && let Some(end_idx) = m.end(0)
                    {
                        let end = i64::try_from(end_idx).unwrap_or(i64::MAX);
                        super::super::structure::_group_end(stmt.text(), end).is_none()
                    } else {
                        false
                    };
                    if unclosed {
                        report(stmt, "many", stmt.text(), true, *missing, diags);
                    } else {
                        for arg in &call.args {
                            if let Some(k) = problem(arg.as_deref(), missing) {
                                report(stmt, k, stmt.text(), true, *missing, diags);
                                return;
                            }
                        }
                    }
                }
            }
        }
        _ => {}
    }
}

/// Port of `checker/expressions.py::check_function_call_counts` (full signature).
pub fn check_function_call_counts_with(
    program: &ProgramRef,
    profile: &'static Profile,
    diags: &mut Diagnostics,
) {
    let mut functions: HashMap<String, &SubDef> = HashMap::new();
    for stmt in &program.statements {
        if let Stmt::SubDef(st) = &**stmt
            && st.keyword == "FUNCTION"
        {
            functions.insert(pystr::upper(&st.name), st);
        }
    }
    if functions.is_empty() {
        return;
    }
    let ctx = context::of(program, Some(profile), None);
    let _known_names = ctx.known_names();
    // TODO(integrator-2): use ctx.known_names() once KnownNames is ported in part2.rs.
    let known_set = BTreeSet::new();
    let known = &known_set;
    let mut missing: usize = 1;

    let problem = |expr: Option<&Expr>, missing: &mut usize| -> Option<&'static str> {
        for node in _index_nodes_with(expr) {
            let Some(fn_def) = functions.get(&pystr::upper(&node.name)) else {
                continue;
            };
            if fn_def.missing_required_params(&node.args) > 0 {
                *missing = 0;
                for inner in _index_nodes_from_index(node) {
                    if let Some(f2) = functions.get(&pystr::upper(&inner.name)) {
                        *missing += f2.missing_required_params(&inner.args);
                    }
                }
                return Some("few");
            }
            if node.args.len() > fn_def.params.len() {
                return Some("many");
            }
        }
        None
    };

    let report = |stmt: &Stmt,
                  kind: &str,
                  echo: &str,
                  whole: bool,
                  missing: usize,
                  diags: &mut Diagnostics| {
        let text = if kind == "few" {
            for _ in 0..missing {
                diags.error_basic(stmt.line(), "Call argument required.", stmt.file());
            }
            if whole {
                // TODO(integrator-2): pass profile to structure::_out_of_place
                super::super::structure::_out_of_place(echo, stmt.at_eol())
            } else {
                format!("Invalid, or out of place expression: {echo}.")
            }
        } else {
            let sep = if whole && profile.fallback_echo_newline && stmt.at_eol() {
                "\n"
            } else {
                ""
            };
            format!("No Matching, or out of place ) in expression: {echo}{sep}.")
        };
        diags.error_basic(stmt.line(), &text, stmt.file());
    };

    for stmt in &program.statements {
        visit_call_count_stmt(
            stmt,
            &functions,
            known,
            profile,
            &mut missing,
            diags,
            &problem,
            &report,
        );
    }
}

/// Port of `checker/expressions.py::_index_nodes`.
pub fn _index_nodes(_expr: Option<&Expr>) {
    todo!("checker/expressions.py::_index_nodes")
}

/// Port of `checker/expressions.py::_index_nodes` (returning collected `Index` nodes).
pub fn _index_nodes_with(expr: Option<&Expr>) -> Vec<&Index> {
    let mut out = Vec::new();
    collect_index_nodes(expr, &mut out);
    out
}

fn _index_nodes_from_index(ix: &Index) -> Vec<&Index> {
    let mut out = vec![ix];
    for a in &ix.args {
        collect_index_nodes(a.as_deref(), &mut out);
    }
    out
}

fn collect_index_nodes<'a>(expr: Option<&'a Expr>, out: &mut Vec<&'a Index>) {
    let Some(expr) = expr else {
        return;
    };
    match expr {
        Expr::Index(ix) => {
            out.push(ix);
            for a in &ix.args {
                collect_index_nodes(a.as_deref(), out);
            }
        }
        Expr::Unary(u) => collect_index_nodes(u.operand.as_deref(), out),
        Expr::Binary(b) => {
            collect_index_nodes(b.left.as_deref(), out);
            collect_index_nodes(b.right.as_deref(), out);
        }
        Expr::Member(m) => {
            for a in m.args.iter().flatten() {
                collect_index_nodes(a.as_deref(), out);
            }
        }
        _ => {}
    }
}

static _FOR_HEADER_RE: LazyLock<pyre::Regex> =
    LazyLock::new(|| pyre::Regex::new(r"(?i)^\s*For\s(.*?)=(.*?)\sTo\s(.*?)(?:\sStep\s(.*))?$", 0));

static NEXT_SCAN_RE: LazyLock<pyre::Regex> =
    LazyLock::new(|| pyre::Regex::new(r"(?i)\s*Next\s+(Sub)?Scan\b", 0));

/// Port of `checker/expressions.py::_check_for_headers`.
pub fn _check_for_headers(_program: &Program, _diags: &mut Diagnostics) {
    todo!("checker/expressions.py::_check_for_headers (omitted: profile: untyped, known: untyped)")
}

/// Port of `checker/expressions.py::_check_for_headers` (full signature).
pub fn _check_for_headers_with(
    program: &ProgramRef,
    profile: &'static Profile,
    diags: &mut Diagnostics,
    known: &BTreeSet<String>,
) {
    let ctx = context::of(program, Some(profile), None);
    let _ds = ctx.dims_state();
    let dim_counts = &ctx.dim_counts(true).counts;
    let mut pending: Vec<(&StmtRef, String)> = Vec::new();
    let whole_echo = is_whole_expr_echo_logger(profile);
    for stmt in &program.statements {
        if let Stmt::Instruction(inst) = &**stmt
            && pystr::upper(&inst.keyword) == "NEXT"
            && !pending.is_empty()
            && NEXT_SCAN_RE.match_(stmt.text()).is_none()
        {
            let (_loop_stmt, var_text) = pending.pop().expect("non-empty pending");
            if !var_text.is_empty() {
                diags.error_basic(
                    stmt.line(),
                    &format!("Invalid symbol in expression: {var_text}."),
                    stmt.file(),
                );
            }
            continue;
        }
        let Stmt::For(for_stmt) = &**stmt else {
            continue;
        };
        let m = _FOR_HEADER_RE.match_(stmt.text());
        let parts = [
            (for_stmt.var.as_deref(), m.as_ref().and_then(|m| m.group(1))),
            (
                for_stmt.start.as_deref(),
                m.as_ref().and_then(|m| m.group(2)),
            ),
            (for_stmt.end.as_deref(), m.as_ref().and_then(|m| m.group(3))),
            (
                for_stmt.step.as_deref(),
                m.as_ref().and_then(|m| m.group(4)),
            ),
        ];
        let mut var_text = String::new();
        for (i, (expr, text)) in parts.into_iter().enumerate() {
            let names = if expr.is_some() {
                super::_undefined_names(expr, known)
            } else {
                Vec::new()
            };
            if names.is_empty() {
                // TODO(integrator-2): pass profile to _dim_limit and dimensions_message.
                if i > 0
                    && let Some(text) = text
                    && super::_over_indexed(expr, dim_counts, super::_dim_limit())
                {
                    diags.error_basic(
                        stmt.line(),
                        &super::dimensions_message(text, false, stmt.at_eol()),
                        stmt.file(),
                    );
                }
                continue;
            }
            if whole_echo {
                let Some(text) = text else {
                    continue;
                };
                if i == 0 {
                    var_text = text.to_string();
                } else {
                    diags.error_basic(
                        stmt.line(),
                        &format!("Invalid symbol in expression: {text}."),
                        stmt.file(),
                    );
                }
            } else {
                for name in &names {
                    diags.error_basic(
                        stmt.line(),
                        &profile.unresolved_symbol_message.format(name),
                        stmt.file(),
                    );
                }
                // TODO(integrator-2): pass ds and profile to _has_undefined_index / dimensions_message.
                if i == 0
                    && let Some(text) = text
                    && super::_has_undefined_index(expr, known, None, Some(stmt))
                {
                    diags.error_basic(
                        stmt.line(),
                        &super::dimensions_message(text, false, stmt.at_eol()),
                        stmt.file(),
                    );
                }
            }
        }
        pending.push((stmt, var_text));
    }
}

static END_SELECT_RE: LazyLock<pyre::Regex> =
    LazyLock::new(|| pyre::Regex::new(r"(?i)\s*End\s+Select\b", 0));
static END_IF_RE: LazyLock<pyre::Regex> =
    LazyLock::new(|| pyre::Regex::new(r"(?i)\s*End\s+If\b", 0));
static CASE_ELSE_RE: LazyLock<pyre::Regex> =
    LazyLock::new(|| pyre::Regex::new(r"(?i)\s*Case\s+Else", 0));
static CASE_TAIL_RE: LazyLock<pyre::Regex> =
    LazyLock::new(|| pyre::Regex::new(r"(?i)\s*Case\s(.*)$", 0));

/// Port of `checker/expressions.py::_check_elseif_and_case`.
pub fn _check_elseif_and_case(_program: &Program, _diags: &mut Diagnostics) {
    todo!(
        "checker/expressions.py::_check_elseif_and_case (omitted: profile: untyped, known: untyped)"
    )
}

/// Port of `checker/expressions.py::_check_elseif_and_case` (full signature).
pub fn _check_elseif_and_case_with(
    program: &ProgramRef,
    profile: &'static Profile,
    diags: &mut Diagnostics,
    known: &BTreeSet<String>,
) {
    let ctx = context::of(program, Some(profile), None);
    let _ds = ctx.dims_state();
    let dim_counts = &ctx.dim_counts(true).counts;
    let whole_echo = is_whole_expr_echo_logger(profile);
    let mut orphans: Vec<bool> = Vec::new();
    let mut open_ifs: usize = 0;

    for stmt in &program.statements {
        let kw = if let Stmt::Instruction(inst) = &**stmt {
            pystr::upper(&inst.keyword)
        } else {
            String::new()
        };
        if let Stmt::Instruction(inst) = &**stmt
            && kw == "SELECT"
        {
            let is_orphan = inst
                .args
                .first()
                .and_then(Option::as_deref)
                .is_some_and(|first| {
                    let undef = super::_undefined_names(Some(first), known);
                    // TODO(integrator-2): pass profile to _dim_limit.
                    (!whole_echo && !undef.is_empty())
                        || (undef.is_empty()
                            && super::_over_indexed(Some(first), dim_counts, super::_dim_limit()))
                });
            orphans.push(is_orphan);
            continue;
        }
        if kw == "ENDSELECT" || (kw == "END" && END_SELECT_RE.match_(stmt.text()).is_some()) {
            if orphans.pop() == Some(true) {
                diags.error_basic(stmt.line(), "EndSelect w/o Select", stmt.file());
            }
            continue;
        }
        if let Stmt::If(if_stmt) = &**stmt
            && if_stmt.inline.is_none()
        {
            open_ifs += 1;
        } else if kw == "ENDIF" || (kw == "END" && END_IF_RE.match_(stmt.text()).is_some()) {
            open_ifs = open_ifs.saturating_sub(1);
        }
        if let Stmt::ElseIf(elseif_stmt) = &**stmt {
            if whole_echo && open_ifs == 0 {
                continue;
            }
            let names = super::_undefined_names(elseif_stmt.cond.as_deref(), known);
            let text = super::_if_condition_text(stmt);
            // TODO(integrator-2): pass profile to _dim_limit and dimensions_message.
            if let Some(ref text) = text
                && names.is_empty()
                && super::_over_indexed(
                    elseif_stmt.cond.as_deref(),
                    dim_counts,
                    super::_dim_limit(),
                )
            {
                diags.error_basic(
                    stmt.line(),
                    &super::dimensions_message(text, false, stmt.at_eol()),
                    stmt.file(),
                );
                continue;
            }
            let Some(text) = text else {
                continue;
            };
            if names.is_empty() {
                continue;
            }
            if whole_echo {
                diags.error_basic(
                    stmt.line(),
                    &format!("Invalid symbol in expression: {text}."),
                    stmt.file(),
                );
            } else {
                for name in &names {
                    diags.error_basic(
                        stmt.line(),
                        &profile.unresolved_symbol_message.format(name),
                        stmt.file(),
                    );
                }
                // TODO(integrator-2): pass ds and profile to _has_undefined_index / dimensions_message.
                if super::_has_undefined_index(elseif_stmt.cond.as_deref(), known, None, Some(stmt))
                {
                    diags.error_basic(
                        stmt.line(),
                        &super::dimensions_message(&text, false, stmt.at_eol()),
                        stmt.file(),
                    );
                }
            }
            continue;
        }
        let Stmt::Case(case_stmt) = &**stmt else {
            continue;
        };
        if case_stmt.is_else || CASE_ELSE_RE.match_(stmt.text()).is_some() {
            continue;
        }
        if orphans.last().copied() == Some(true) {
            diags.error_basic(stmt.line(), "Case outside Select.", stmt.file());
        }
        let mut names: Vec<String> = Vec::new();
        let mut outside: usize = 0;
        for (index, item) in case_stmt.items.iter().enumerate() {
            let (lower, upper) = match item {
                CaseItem::Value(e) => (Some(e.as_ref()), None),
                CaseItem::Range(lo, hi) => (Some(lo.as_ref()), Some(hi.as_ref())),
                CaseItem::Is(_, e) => (None, Some(e.as_ref())),
            };
            let low = super::_undefined_names(lower, known);
            let high = super::_undefined_names(upper, known);
            if !low.is_empty() && outside == 0 && names.is_empty() {
                outside = index + 1;
            }
            names.extend(low);
            names.extend(high);
        }
        if names.is_empty() {
            if let Some(cm) = CASE_TAIL_RE.match_(stmt.text())
                && let Some(tail) = cm.group(1)
            {
                let limit = super::_dim_limit(); // TODO(integrator-2): pass profile
                let has_over = case_stmt.items.iter().any(|item| match item {
                    CaseItem::Value(e) | CaseItem::Is(_, e) => {
                        super::_over_indexed(Some(e), dim_counts, limit)
                    }
                    CaseItem::Range(lo, hi) => {
                        super::_over_indexed(Some(lo), dim_counts, limit)
                            || super::_over_indexed(Some(hi), dim_counts, limit)
                    }
                });
                if has_over {
                    diags.error_basic(
                        stmt.line(),
                        &super::dimensions_message(tail, false, stmt.at_eol()),
                        stmt.file(),
                    );
                }
            }
            continue;
        }
        if whole_echo {
            let Some(m) = CASE_TAIL_RE.match_(stmt.text()) else {
                continue;
            };
            let Some(tail) = m.group(1) else {
                continue;
            };
            for _ in 0..outside {
                diags.error_basic(stmt.line(), "Case outside Select.", stmt.file());
            }
            diags.error_basic(
                stmt.line(),
                &format!("Invalid symbol in expression: {tail}."),
                stmt.file(),
            );
        } else {
            for name in &names {
                diags.error_basic(
                    stmt.line(),
                    &profile.unresolved_symbol_message.format(name),
                    stmt.file(),
                );
            }
        }
    }
}

/// Port of `checker/expressions.py::_check_function_statements`.
pub fn _check_function_statements(_program: &Program, _diags: &mut Diagnostics) {
    todo!(
        "checker/expressions.py::_check_function_statements (omitted: profile: untyped, known: untyped)"
    )
}

/// Port of `checker/expressions.py::_check_function_statements` (full signature).
pub fn _check_function_statements_with(
    program: &ProgramRef,
    profile: &'static Profile,
    diags: &mut Diagnostics,
    known: &BTreeSet<String>,
) {
    let _ds = context::of(program, Some(profile), None).dims_state();
    let mut first_on_line: HashMap<(i64, String), usize> = HashMap::new();
    for stmt in &program.statements {
        first_on_line
            .entry((stmt.line(), stmt.file().to_string()))
            .or_insert_with(|| stmt_id(stmt));
    }
    for stmt in &program.statements {
        let Stmt::Call(call) = &**stmt else {
            continue;
        };
        if call.explicit || !profile.functions().contains(&pystr::upper(&call.name)) {
            continue;
        }
        let mut args: Vec<Option<ExprRef>> = call.args.clone();
        if pystr::upper(&call.name) == "SERIALOPEN"
            && is_whole_expr_echo_logger(profile)
            && super::super::cr3xx::bare_unknown_delay(call, known)
            && args.len() > 3
        {
            args.remove(3);
        }
        let names: Vec<String> = args
            .iter()
            .flatten()
            .flat_map(|a| super::_undefined_names(Some(a), known))
            .collect();
        let stmt_ptr = stmt_id(stmt);
        let is_first = first_on_line
            .get(&(stmt.line(), stmt.file().to_string()))
            .copied()
            == Some(stmt_ptr);
        let has_other_error = diags.errors().iter().any(|d| {
            d.line == stmt.line()
                && d.file == stmt.file()
                && !(d.message == "Expr illegally occurs before BeginProg." && is_first)
        });
        if names.is_empty() || has_other_error {
            continue;
        }
        if is_whole_expr_echo_logger(profile) {
            diags.error_basic(
                stmt.line(),
                &format!("Invalid symbol in expression: {}.", stmt.text()),
                stmt.file(),
            );
            continue;
        }
        // TODO(integrator-2): wire _names_and_dims(known, ds, stmt, (), *stmt.args) -> (Vec<String>, bool).
        let (names, dims): (Vec<String>, bool) = {
            super::_names_and_dims(known, None, Some(stmt));
            (Vec::new(), false)
        };
        for name in &names {
            diags.error_basic(
                stmt.line(),
                &profile.unresolved_symbol_message.format(name),
                stmt.file(),
            );
        }
        if dims {
            // TODO(integrator-2): pass profile to dimensions_message.
            diags.error_basic(
                stmt.line(),
                &super::dimensions_message(stmt.text(), true, stmt.at_eol()),
                stmt.file(),
            );
        }
    }
}

const _BOUNDS_CHECKED_CODES: &[i64] = &[2, 8, 24, 27, 28, 29, 41];
const _BOUNDS_UNCHECKED: &[&str] = &["ADDPRECISE", "MOVEPRECISE"];

fn bounds_probed(code: i64) -> Option<&'static [&'static str]> {
    match code {
        24 => Some(&["SDMIO16", "PORTPAIRCONFIG"]),
        27 => Some(&[
            "MOVE",
            "ERASE",
            "GETRECORD",
            "SPLITSTR",
            "MOVEBYTES",
            "SDI12RECORDER",
        ]),
        29 => Some(&[
            "REALTIME",
            "READIO",
            "PORTGET",
            "AVGSPA",
            "MAXSPA",
            "MINSPA",
            "RMSSPA",
            "STDDEVSPA",
            "SORTSPA",
            "SORTSPAINDEXED",
        ]),
        _ => None,
    }
}

/// Port of `checker/expressions.py::check_instruction_index_bounds`.
pub fn check_instruction_index_bounds(_program: &Program, _diags: &mut Diagnostics) {
    todo!(
        "checker/expressions.py::check_instruction_index_bounds (omitted: profile: untyped, env: consteval.Env)"
    )
}

/// Port of `checker/expressions.py::check_instruction_index_bounds` (full signature).
pub fn check_instruction_index_bounds_with(
    program: &ProgramRef,
    profile: &'static Profile,
    diags: &mut Diagnostics,
    env: &Env,
) {
    let padded = is_whole_expr_echo_logger(profile);
    let tables = context::of(program, Some(profile), Some(env.clone())).scope_tables(padded, false);
    for stmt in &program.statements {
        let candidates: Vec<(Option<i64>, Option<&Expr>)> = match &**stmt {
            Stmt::Instruction(inst)
                if pystr::upper(&inst.keyword) == "WHILE"
                    && inst.args.len() == 1
                    && matches!(inst.args[0].as_deref(), Some(Expr::Index(_))) =>
            {
                vec![(Some(2), inst.args[0].as_deref())]
            }
            Stmt::If(s) if matches!(s.cond.as_deref(), Some(Expr::Index(_))) => {
                vec![(Some(2), s.cond.as_deref())]
            }
            Stmt::ElseIf(s) if matches!(s.cond.as_deref(), Some(Expr::Index(_))) => {
                vec![(Some(2), s.cond.as_deref())]
            }
            Stmt::DoLoop(s) if matches!(s.cond.as_deref(), Some(Expr::Index(_))) => {
                vec![(Some(2), s.cond.as_deref())]
            }
            Stmt::Instruction(inst) => {
                let kw = pystr::upper(&inst.keyword);
                if _BOUNDS_UNCHECKED.contains(&kw.as_str()) {
                    continue;
                }
                let Some(record) = profile.instructions().get(&kw) else {
                    continue;
                };
                #[allow(clippy::float_cmp)]
                let first_is_0_or_4 = matches!(
                    inst.args.first().and_then(Option::as_deref),
                    Some(Expr::Number(n)) if matches!(n.value, NumberValue::Int(0 | 4))
                        || matches!(n.value, NumberValue::Float(f) if f == 0.0 || f == 4.0)
                );
                inst.args
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| !(kw == "FIELDCAL" && *i == 6 && first_is_0_or_4))
                    .map(|(i, arg)| (record.params.get(i).copied(), arg.as_deref()))
                    .collect()
            }
            _ => continue,
        };
        let scope = tables.for_statement(stmt);
        for (code, arg) in candidates {
            let Some(code) = code else {
                continue;
            };
            if !_BOUNDS_CHECKED_CODES.contains(&code) {
                continue;
            }
            let Some(Expr::Index(ix)) = arg else {
                continue;
            };
            if ix.args.is_empty() {
                continue;
            }
            if let Some(probed) = bounds_probed(code)
                && let Stmt::Instruction(inst) = &**stmt
                && !probed.contains(&pystr::upper(&inst.keyword).as_str())
            {
                continue;
            }
            let name = pystr::upper(&ix.name);
            let Some(shape) = scope.dims.get(&name) else {
                continue;
            };
            if shape.is_empty() {
                continue;
            }
            let ty = scope.types.get(&name).map(String::as_str);
            let type_ok = ty == Some("FLOAT")
                || (code == 27 && scope.arrays.contains(&name))
                || ((code == 24 || code == 29) && ty == Some("LONG"));
            if !type_ok {
                continue;
            }
            let mut index = Vec::with_capacity(ix.args.len());
            let mut all_int = true;
            for a in &ix.args {
                if let Some(Expr::Number(n)) = a.as_deref()
                    && let NumberValue::Int(v) = n.value
                {
                    index.push(v);
                } else {
                    all_int = false;
                    break;
                }
            }
            if !all_int {
                continue;
            }
            let bad = if index.len() == 1 && shape.len() > 1 {
                !(1..=shape[0]).contains(&index[0])
            } else if index.len() == shape.len() || (padded && index.len() > shape.len()) {
                let mut padded_shape = shape.clone();
                padded_shape.resize(index.len(), 1);
                index
                    .iter()
                    .zip(padded_shape.iter())
                    .any(|(&k, &n)| !(1..=n).contains(&k))
            } else {
                continue;
            };
            if bad {
                diags.error_basic(
                    stmt.line(),
                    &format!("Variable {} out of bounds.", ix.name),
                    stmt.file(),
                );
            }
        }
    }
}

/// Port of `checker/expressions.py::check_splitstr_element_reps`.
pub fn check_splitstr_element_reps(_program: &Program, _diags: &mut Diagnostics) {
    todo!(
        "checker/expressions.py::check_splitstr_element_reps (omitted: profile: untyped, env: untyped)"
    )
}

/// Port of `checker/expressions.py::check_splitstr_element_reps` (full signature).
pub fn check_splitstr_element_reps_with(
    program: &ProgramRef,
    profile: &'static Profile,
    diags: &mut Diagnostics,
    env: &Env,
) {
    let mut strings = BTreeSet::new();
    for st in &program.statements {
        if let Stmt::Declaration(decl) = &**st {
            for v in &decl.vars {
                if v.type_.as_deref().map(pystr::upper).as_deref() == Some("STRING") && !v.paren_dim
                {
                    strings.insert(pystr::upper(&v.name));
                }
            }
        }
    }
    let tables = context::of(program, Some(profile), Some(env.clone())).scope_tables(false, false);
    for stmt in &program.statements {
        let Stmt::Instruction(inst) = &**stmt else {
            continue;
        };
        if pystr::upper(&inst.keyword) != "SPLITSTR" || inst.args.len() < 4 {
            continue;
        }
        let Some(Expr::Index(dest)) = inst.args[0].as_deref() else {
            continue;
        };
        if dest.args.len() != 1 {
            continue;
        }
        let Some(Expr::Number(num)) = dest.args[0].as_deref() else {
            continue;
        };
        let NumberValue::Int(start) = num.value else {
            continue;
        };
        let name = pystr::upper(&dest.name);
        let scope = tables.for_statement(stmt);
        let Some(shape) = scope.dims.get(&name) else {
            continue;
        };
        if shape.len() != 1 || strings.contains(&name) || !scope.arrays.contains(&name) {
            continue;
        }
        let Ok(reps_val) = consteval::value_of(inst.args[3].as_deref(), env) else {
            continue;
        };
        let Some(reps) = const_as_exact_int(&reps_val) else {
            continue;
        };
        if reps < 1 {
            continue;
        }
        if start.saturating_add(reps).saturating_sub(1) > shape[0] {
            diags.error_basic(
                stmt.line(),
                &format!("Variable {} out of bounds.", dest.name),
                stmt.file(),
            );
        }
    }
}

/// Port of `checker/expressions.py::_sdmsio4_count`.
pub fn _sdmsio4_count(_stmt: &Instruction) -> Option<i64> {
    todo!("checker/expressions.py::_sdmsio4_count (omitted: env: untyped)")
}

/// Port of `checker/expressions.py::_sdmsio4_count` (full signature).
pub fn _sdmsio4_count_with(stmt: &Instruction, env: &Env) -> Option<i64> {
    if stmt.args.len() < 8 || stmt.args[7].is_none() {
        return None;
    }
    let Ok(n) = consteval::value_of(stmt.args[7].as_deref(), env) else {
        return None;
    };
    const_as_exact_int(&n)
}

/// Port of `checker/expressions.py::check_sdmsio4_count`.
pub fn check_sdmsio4_count(_program: &Program, _diags: &mut Diagnostics) {
    todo!("checker/expressions.py::check_sdmsio4_count (omitted: profile: untyped, env: untyped)")
}

/// Port of `checker/expressions.py::check_sdmsio4_count` (full signature).
pub fn check_sdmsio4_count_with(
    program: &ProgramRef,
    profile: &'static Profile,
    diags: &mut Diagnostics,
    env: &Env,
) {
    let tables = context::of(program, Some(profile), Some(env.clone())).scope_tables(true, false);
    for stmt in &program.statements {
        let Stmt::Instruction(inst) = &**stmt else {
            continue;
        };
        if pystr::upper(&inst.keyword) != "SDMSIO4" || inst.args.is_empty() {
            continue;
        }
        let Some(n) = _sdmsio4_count_with(inst, env) else {
            continue;
        };
        if n < 1 {
            continue;
        }
        let (dest_name, start) = match inst.args[0].as_deref() {
            Some(Expr::Name(n)) => (&n.name, 1),
            Some(Expr::Index(ix)) => {
                if ix.args.len() != 1 {
                    continue;
                }
                let Some(Expr::Number(num)) = ix.args[0].as_deref() else {
                    continue;
                };
                let NumberValue::Int(v) = num.value else {
                    continue;
                };
                (&ix.name, v)
            }
            _ => continue,
        };
        let scope = tables.for_statement(stmt);
        let Some(shape) = scope.dims.get(&pystr::upper(dest_name)) else {
            continue;
        };
        if shape.len() != 1 {
            continue;
        }
        if start.saturating_add(n).saturating_sub(1) > shape[0] {
            diags.error_basic(
                stmt.line(),
                &format!("Variable {dest_name} out of bounds."),
                stmt.file(),
            );
        }
    }
}

/// Port of `checker/expressions.py::check_timedcontrol_constants`.
pub fn check_timedcontrol_constants(_program: &Program, _diags: &mut Diagnostics) {
    todo!("checker/expressions.py::check_timedcontrol_constants (omitted: profile: untyped)")
}

/// Port of `checker/expressions.py::check_timedcontrol_constants` (full signature).
pub fn check_timedcontrol_constants_with(
    program: &Program,
    profile: &'static Profile,
    diags: &mut Diagnostics,
) {
    if !is_whole_expr_echo_logger(profile) {
        return;
    }
    let mut consts: BTreeSet<String> = program.consts.keys().map(|n| pystr::upper(n)).collect();
    for (sym, _) in profile.symbols().iter() {
        consts.insert(sym.to_string());
    }
    for stmt in &program.statements {
        let Stmt::Instruction(inst) = &**stmt else {
            continue;
        };
        if pystr::upper(&inst.keyword) != "TIMEDCONTROL" {
            continue;
        }
        for (i, what) in [(0, "Size"), (1, "Interval")] {
            if let Some(Some(arg)) = inst.args.get(i)
                && !_names_in_with(Some(arg.as_ref())).is_subset(&consts)
            {
                diags.error_basic(
                    stmt.line(),
                    &format!("{what} must be a constant."),
                    stmt.file(),
                );
            }
        }
    }
}

/// Port of `checker/expressions.py::_names_in`.
pub fn _names_in() -> BTreeSet<String> {
    todo!("checker/expressions.py::_names_in (omitted: expr: untyped)")
}

/// Port of `checker/expressions.py::_names_in` (full signature).
pub fn _names_in_with(expr: Option<&Expr>) -> BTreeSet<String> {
    let Some(expr) = expr else {
        return BTreeSet::new();
    };
    match expr {
        Expr::Name(n) => BTreeSet::from([pystr::upper(&n.name)]),
        Expr::Index(ix) => {
            let mut out = BTreeSet::from([pystr::upper(&ix.name)]);
            for a in ix.args.iter().flatten() {
                out.extend(_names_in_with(Some(a.as_ref())));
            }
            out
        }
        Expr::Unary(u) => _names_in_with(u.operand.as_deref()),
        Expr::Binary(b) => {
            let mut out = _names_in_with(b.left.as_deref());
            out.extend(_names_in_with(b.right.as_deref()));
            out
        }
        Expr::Member(m) => _names_in_with(m.base.as_deref()),
        _ => BTreeSet::new(),
    }
}

/// Port of `checker/expressions.py::check_calibrate_size`.
pub fn check_calibrate_size(_program: &Program, _diags: &mut Diagnostics) {
    todo!("checker/expressions.py::check_calibrate_size (omitted: profile: untyped, env: untyped)")
}

/// Port of `checker/expressions.py::check_calibrate_size` (full signature).
pub fn check_calibrate_size_with(
    program: &ProgramRef,
    profile: &'static Profile,
    diags: &mut Diagnostics,
    env: &Env,
) {
    let need = match profile.name {
        "cr6" | "cr1000x" => 6,
        "cr300" | "cr350" => 51,
        _ => return,
    };
    let tables = context::of(program, Some(profile), Some(env.clone())).scope_tables(true, false);
    for stmt in &program.statements {
        let Stmt::Instruction(inst) = &**stmt else {
            continue;
        };
        if pystr::upper(&inst.keyword) != "CALIBRATE" || inst.args.is_empty() {
            continue;
        }
        let (dest_name, start) = match inst.args[0].as_deref() {
            Some(Expr::Name(n)) => (&n.name, 1),
            Some(Expr::Index(ix)) if ix.args.is_empty() => (&ix.name, 1),
            Some(Expr::Index(ix)) if ix.args.len() == 1 => {
                let Some(Expr::Number(num)) = ix.args[0].as_deref() else {
                    continue;
                };
                let NumberValue::Int(v) = num.value else {
                    continue;
                };
                (&ix.name, v)
            }
            _ => continue,
        };
        let scope = tables.for_statement(stmt);
        let Some(shape) = scope.dims.get(&pystr::upper(dest_name)) else {
            continue;
        };
        if shape.len() != 1 {
            continue;
        }
        if shape[0].saturating_sub(start).saturating_add(1) < need {
            diags.error_basic(
                stmt.line(),
                &format!("Variable {dest_name} out of bounds."),
                stmt.file(),
            );
        }
    }
}

/// Port of `checker/expressions.py::check_splitstr_whole_array`.
pub fn check_splitstr_whole_array(_program: &Program, _diags: &mut Diagnostics) {
    todo!(
        "checker/expressions.py::check_splitstr_whole_array (omitted: profile: untyped, env: untyped)"
    )
}

/// Port of `checker/expressions.py::check_splitstr_whole_array` (full signature).
pub fn check_splitstr_whole_array_with(
    program: &ProgramRef,
    profile: &'static Profile,
    diags: &mut Diagnostics,
    env: &Env,
) {
    let tables = context::of(program, Some(profile), Some(env.clone())).scope_tables(false, false);
    for stmt in &program.statements {
        let Stmt::Instruction(inst) = &**stmt else {
            continue;
        };
        if pystr::upper(&inst.keyword) != "SPLITSTR" {
            continue;
        }
        let scope = tables.for_statement(stmt);
        if inst.args.len() < 4 {
            continue;
        }
        let Some(Expr::Index(dest)) = inst.args[0].as_deref() else {
            continue;
        };
        if !dest.args.is_empty() {
            continue;
        }
        let name = pystr::upper(&dest.name);
        let Some(shape) = scope.dims.get(&name) else {
            continue;
        };
        if shape.is_empty() || !scope.arrays.contains(&name) {
            continue;
        }
        let reps = match consteval::value_of(inst.args[3].as_deref(), env) {
            Ok(ConstValue::Int(v)) => v,
            Ok(_) | Err(NotConstant::NotConstant(_)) => continue,
            Err(NotConstant::DivideByZero { numerator, .. }) => {
                let arg_txt = super::_arg_text(inst, 3);
                let text = pystr::strip(&arg_txt);
                if !text.contains('.') && !profile.single_pass {
                    diags.error_basic(
                        stmt.line(),
                        &format!("Divide by zero in expression: {text}."),
                        stmt.file(),
                    );
                } else if let Some(Expr::Binary(b)) = inst.args[3].as_deref()
                    && b.op == "/"
                    && numerator.as_ref().is_some_and(const_is_positive)
                {
                    diags.error_basic(
                        stmt.line(),
                        &format!("Variable {} out of bounds.", dest.name),
                        stmt.file(),
                    );
                }
                continue;
            }
        };
        let total = shape.iter().fold(1i64, |acc, &n| acc.saturating_mul(n));
        if reps > total {
            diags.error_basic(
                stmt.line(),
                &format!("Variable {} out of bounds.", dest.name),
                stmt.file(),
            );
        }
    }
}

/// Port of `checker/expressions.py::check_subcall_element_bounds`.
pub fn check_subcall_element_bounds(_program: &Program, _diags: &mut Diagnostics) {
    todo!(
        "checker/expressions.py::check_subcall_element_bounds (omitted: profile: untyped, env: untyped)"
    )
}

/// Port of `checker/expressions.py::check_subcall_element_bounds` (full signature).
pub fn check_subcall_element_bounds_with(
    program: &ProgramRef,
    profile: &'static Profile,
    diags: &mut Diagnostics,
    env: &Env,
) {
    let ctx = context::of(program, Some(profile), Some(env.clone()));
    let subs = ctx.declared_subs();
    let tables = ctx.scope_tables(false, false);
    for stmt in &program.statements {
        let Stmt::Call(call) = &**stmt else {
            continue;
        };
        if !subs.contains(&pystr::upper(&call.name)) {
            continue;
        }
        let dims = &tables.for_statement(stmt).dims;
        for arg in &call.args {
            let Some(Expr::Index(ix)) = arg.as_deref() else {
                continue;
            };
            if ix.args.is_empty() {
                continue;
            }
            let Some(shape) = dims.get(&pystr::upper(&ix.name)) else {
                continue;
            };
            let mut index = Vec::new();
            for a in &ix.args {
                if let Some(Expr::Number(n)) = a.as_deref()
                    && let NumberValue::Int(v) = n.value
                {
                    index.push(v);
                }
            }
            if shape.is_empty()
                || index.len() != ix.args.len()
                || index.len() != shape.len()
                || index.iter().any(|&k| k < 0)
            {
                continue;
            }
            if index
                .iter()
                .zip(shape.iter())
                .any(|(&k, &n)| !(1..=n).contains(&k))
            {
                diags.error_basic(
                    stmt.line(),
                    &format!("Variable {} out of bounds.", ix.name),
                    stmt.file(),
                );
            }
        }
    }
}

static END_ROUTINE_RE: LazyLock<pyre::Regex> =
    LazyLock::new(|| pyre::Regex::new(r"(?i)\s*End\s+(Function|Sub)\b", 0));

/// Port of `checker/expressions.py::check_recursive_functions`.
pub fn check_recursive_functions(_program: &Program, _diags: &mut Diagnostics) {
    todo!("checker/expressions.py::check_recursive_functions (omitted: profile: untyped)")
}

/// Port of `checker/expressions.py::check_recursive_functions` (full signature).
pub fn check_recursive_functions_with(
    program: &Program,
    _profile: &'static Profile,
    diags: &mut Diagnostics,
) {
    let mut current: Option<String> = None;
    for stmt in &program.statements {
        if let Stmt::SubDef(sd) = &**stmt {
            current = if sd.keyword == "FUNCTION" {
                Some(pystr::upper(&sd.name))
            } else {
                None
            };
            continue;
        }
        if let Stmt::Instruction(inst) = &**stmt
            && (matches!(
                pystr::upper(&inst.keyword).as_str(),
                "ENDFUNCTION" | "ENDSUB"
            ) || END_ROUTINE_RE.match_(stmt.text()).is_some())
        {
            current = None;
            continue;
        }
        let Some(ref cur) = current else {
            continue;
        };
        let mut exprs: Vec<Option<&Expr>> = Vec::new();
        match &**stmt {
            Stmt::Assignment(a) => exprs.push(a.value.as_deref()),
            Stmt::If(s) => exprs.push(s.cond.as_deref()),
            Stmt::ElseIf(s) => exprs.push(s.cond.as_deref()),
            Stmt::DoLoop(s) => exprs.push(s.cond.as_deref()),
            Stmt::Instruction(s) => exprs.extend(s.args.iter().map(Option::as_deref)),
            Stmt::Call(s) => exprs.extend(s.args.iter().map(Option::as_deref)),
            Stmt::For(s) => {
                exprs.push(s.start.as_deref());
                exprs.push(s.end.as_deref());
                exprs.push(s.step.as_deref());
            }
            _ => {}
        }
        if exprs.into_iter().any(|e| {
            _index_nodes_with(e)
                .into_iter()
                .any(|node| &pystr::upper(&node.name) == cur)
        }) {
            diags.error_basic(
                stmt.line(),
                "Cannot recursively call functions.",
                stmt.file(),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser;
    use crate::profiles;
    use crate::syntax::{Binary, Name, Node, Unary};

    fn cr6() -> &'static Profile {
        profiles::get("cr6").expect("cr6 profile")
    }

    fn cr300() -> &'static Profile {
        profiles::get("cr300").expect("cr300 profile")
    }

    fn parse(src: &str, profile: &'static Profile) -> (ProgramRef, Diagnostics) {
        parser::parse_text(src, "<test>.CR6", None, Some(profile))
    }

    fn err_msgs(diags: &Diagnostics) -> Vec<String> {
        diags
            .errors()
            .into_iter()
            .map(|d| d.message.clone())
            .collect()
    }

    #[test]
    fn test_index_nodes_with() {
        let inner = Rc::new(Expr::Index(Index {
            node: Node::new(1, ""),
            name: "B".to_string(),
            args: vec![Some(Rc::new(Expr::Name(Name {
                node: Node::new(1, ""),
                name: "x".to_string(),
            })))],
        }));
        let outer = Expr::Binary(Binary {
            node: Node::new(1, ""),
            op: "+".to_string(),
            left: Some(Rc::new(Expr::Index(Index {
                node: Node::new(1, ""),
                name: "A".to_string(),
                args: vec![Some(inner)],
            }))),
            right: Some(Rc::new(Expr::Unary(Unary {
                node: Node::new(1, ""),
                op: "-".to_string(),
                operand: Some(Rc::new(Expr::Index(Index {
                    node: Node::new(1, ""),
                    name: "C".to_string(),
                    args: vec![],
                }))),
            }))),
        });
        let names: Vec<&str> = _index_nodes_with(Some(&outer))
            .into_iter()
            .map(|ix| ix.name.as_str())
            .collect();
        assert_eq!(names, vec!["A", "B", "C"]);
    }

    #[test]
    fn test_names_in_with() {
        let expr = Expr::Binary(Binary {
            node: Node::new(1, ""),
            op: "+".to_string(),
            left: Some(Rc::new(Expr::Index(Index {
                node: Node::new(1, ""),
                name: "arr".to_string(),
                args: vec![Some(Rc::new(Expr::Name(Name {
                    node: Node::new(1, ""),
                    name: "idx".to_string(),
                })))],
            }))),
            right: Some(Rc::new(Expr::Member(Member {
                node: Node::new(1, ""),
                base: Some(Rc::new(Expr::Name(Name {
                    node: Node::new(1, ""),
                    name: "tbl".to_string(),
                }))),
                member: "field".to_string(),
                args: None,
            }))),
        });
        let names = _names_in_with(Some(&expr));
        assert_eq!(
            names,
            BTreeSet::from(["ARR".to_string(), "IDX".to_string(), "TBL".to_string()])
        );
    }

    #[test]
    fn test_instruction_index_bounds_cr6_vs_cr300() {
        let (prog, mut diags) = parse(
            "Public A(5)\nBeginProg\n  Battery(A(9))\n  Battery(A(0))\nEndProg\n",
            cr6(),
        );
        let env = Env::new(cr6());
        check_instruction_index_bounds_with(&prog, cr6(), &mut diags, &env);
        assert_eq!(
            err_msgs(&diags),
            vec!["Variable A out of bounds.", "Variable A out of bounds."]
        );

        let (prog6, mut diags6) = parse(
            "Public X\nBeginProg\n  Battery(X(2))\n  Battery(X(1,1))\nEndProg\n",
            cr6(),
        );
        check_instruction_index_bounds_with(&prog6, cr6(), &mut diags6, &Env::new(cr6()));
        assert_eq!(err_msgs(&diags6), vec!["Variable X out of bounds."]);

        let (prog3, mut diags3) = parse("Public X\nBeginProg\n  Battery(X(2))\nEndProg\n", cr300());
        check_instruction_index_bounds_with(&prog3, cr300(), &mut diags3, &Env::new(cr300()));
        assert_eq!(err_msgs(&diags3), Vec::<String>::new());
    }

    #[test]
    fn test_splitstr_element_reps() {
        let (prog, mut diags) = parse(
            "Public F(3)\nPublic S(3) As String * 10\nBeginProg\n  SplitStr(F(2),\"a\",\"\",3,0)\n  SplitStr(S(2),\"a\",\"\",3,0)\nEndProg\n",
            cr6(),
        );
        check_splitstr_element_reps_with(&prog, cr6(), &mut diags, &Env::new(cr6()));
        assert_eq!(err_msgs(&diags), vec!["Variable F out of bounds."]);
    }

    #[test]
    fn test_sdmsio4_count() {
        let (prog, mut diags) = parse(
            "Public D(4)\nPublic X\nBeginProg\n  SDMSIO4(D(2),0,0,0,0,0,0,4)\n  SDMSIO4(X,0,0,0,0,0,0,2)\nEndProg\n",
            cr6(),
        );
        check_sdmsio4_count_with(&prog, cr6(), &mut diags, &Env::new(cr6()));
        assert_eq!(
            err_msgs(&diags),
            vec!["Variable D out of bounds.", "Variable X out of bounds."]
        );
    }

    #[test]
    fn test_timedcontrol_constants() {
        let (prog, mut diags) = parse(
            "Public N\nConst C = 5\nBeginProg\n  TimedControl(N, C, 1, 0, 0, 0)\nEndProg\n",
            cr6(),
        );
        check_timedcontrol_constants_with(&prog, cr6(), &mut diags);
        assert_eq!(err_msgs(&diags), vec!["Size must be a constant."]);

        let (prog3, mut diags3) = parse(
            "Public N\nConst C = 5\nBeginProg\n  TimedControl(N, C, 1, 0, 0, 0)\nEndProg\n",
            cr300(),
        );
        check_timedcontrol_constants_with(&prog3, cr300(), &mut diags3);
        assert_eq!(err_msgs(&diags3), Vec::<String>::new());
    }

    #[test]
    fn test_calibrate_size() {
        let (prog6, mut diags6) = parse(
            "Public A(5)\nPublic B(50)\nBeginProg\n  Calibrate(A)\n  Calibrate(B)\nEndProg\n",
            cr6(),
        );
        check_calibrate_size_with(&prog6, cr6(), &mut diags6, &Env::new(cr6()));
        assert_eq!(err_msgs(&diags6), vec!["Variable A out of bounds."]);

        let (prog3, mut diags3) = parse(
            "Public B(50)\nBeginProg\n  Calibrate(B)\nEndProg\n",
            cr300(),
        );
        check_calibrate_size_with(&prog3, cr300(), &mut diags3, &Env::new(cr300()));
        assert_eq!(err_msgs(&diags3), vec!["Variable B out of bounds."]);
    }

    #[test]
    fn test_splitstr_whole_array() {
        let (prog, mut diags) = parse(
            "Public S(5)\nBeginProg\n  SplitStr(S(),\"a\",\"\",6,0)\n  SplitStr(S(),\"a\",\"\",1/0,0)\nEndProg\n",
            cr300(),
        );
        check_splitstr_whole_array_with(&prog, cr300(), &mut diags, &Env::new(cr300()));
        assert_eq!(
            err_msgs(&diags),
            vec!["Variable S out of bounds.", "Variable S out of bounds."]
        );
    }

    #[test]
    fn test_subcall_element_bounds() {
        let (prog, mut diags) = parse(
            "Public P(9)\nSub S1(x)\nEndSub\nBeginProg\n  S1(P(10))\n  S1(P(0))\n  S1(P(-1))\nEndProg\n",
            cr6(),
        );
        check_subcall_element_bounds_with(&prog, cr6(), &mut diags, &Env::new(cr6()));
        assert_eq!(
            err_msgs(&diags),
            vec!["Variable P out of bounds.", "Variable P out of bounds."]
        );
    }

    #[test]
    fn test_recursive_functions() {
        let (prog, mut diags) = parse(
            "Function F(a)\n  F = F(a) + 1\n  F = 1\nEndFunction\nSub S()\n  S()\nEndSub\n",
            cr6(),
        );
        check_recursive_functions_with(&prog, cr6(), &mut diags);
        assert_eq!(err_msgs(&diags), vec!["Cannot recursively call functions."]);
    }
}
