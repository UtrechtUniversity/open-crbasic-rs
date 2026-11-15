//! Port of `checker/intervals.py`.

#![allow(
    dead_code,
    unused_variables,
    unused_imports,
    clippy::used_underscore_items
)]

use std::collections::{BTreeSet, HashMap, VecDeque};
use std::rc::Rc;
use std::sync::LazyLock;

use crate::consteval::{self, Env};
use crate::diagnostics::{DiagRef, Diagnostics, ShowFile};
use crate::profiles::Profile;
use crate::pyre;
use crate::pystr;
use crate::syntax::{
    ConstValue, Expr, ExprRef, Index, Instruction, Number, NumberValue, Program, ProgramRef, Stmt,
    StmtRef, expr_id, stmt_id,
};

use super::context;
use super::expressions::{self, KnownNames};
use super::symbols;

const _FUNCTIONS: &[&str] = &["TIMEINTOINTERVAL", "IFTIME"];
const _ALL_INTEGRAL_NAMES: &[&str] = &["TIMEINTOINTERVAL", "IFTIME", "SERIALOPEN", "TIMER"];
const _TIMEOUT_NAMES: &[&str] = &["HTTPPUT", "FTPCLIENT"];
const _MONTH: i64 = 6;
const NOT_INTEGRAL: &str = "Parameter must be integral.";
const _TIMEOUT_NEGATIVE: &str = "Parameter TimeOut cannot be negative";
const _INT_MIN: i64 = -(1_i64 << 31);
const _INT_MAX: i64 = (1_i64 << 31) - 1;
const _INT_MIN_F64: f64 = -2_147_483_648.0;
const _INT_MAX_F64: f64 = 2_147_483_647.0;
const _F32_MAX: f64 = 3.402_823_466_385_288_6e38;

// TODO(P4): move to `instr_data` when `checker/instr_data.py` is ported.
const _TABLE_ONLY_INSTRUCTIONS: &[&str] = &[
    "CARDOUT",
    "MQTTPUBLISHTABLE",
    "GOESTABLE",
    "GOESFIELD",
    "TABLEFILE",
    "TOTALIZE",
    "AVERAGE",
    "MAXIMUM",
    "MINIMUM",
    "SAMPLEMAXMIN",
    "STDDEV",
    "MOMENT",
    "SAMPLE",
    "HISTOGRAM",
    "HISTOGRAM4D",
    "COVARIANCE",
    "RAINFLOW",
    "LEVELCROSSING",
    "WINDVECTOR",
    "MEDIAN",
    "SAMPLEFIELDCAL",
    "FILLSTOP",
    "DATATIME",
    "OPENINTERVAL",
    "DATAINTERVAL",
    "FFT",
    "DATAEVENT",
];

static _MODE_RE: LazyLock<pyre::Regex> = LazyLock::new(|| {
    pyre::Regex::new(
        r"(?i)\b(IfTime|TimeIntoInterval|Timer|SerialOpen|HTTPPut|FTPClient)\b\s*\(",
        0,
    )
});
static _LITERAL_RE: LazyLock<pyre::Regex> =
    LazyLock::new(|| pyre::Regex::new(r#""[^"]*"|'.*$"#, 0));
static _THEN_RE: LazyLock<pyre::Regex> = LazyLock::new(|| pyre::Regex::new(r"(?i)\bThen\b", 0));

/// Classified constant `TimeOut` argument (`("wide", 0)` or `("int", v)`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeoutValue {
    Wide,
    Int(i64),
}

/// A call site yielded by `_calls` (`S.Call` statement or `S.Index` expression).
#[derive(Debug, Clone)]
pub enum CallSite {
    Stmt(StmtRef),
    Expr(ExprRef),
}

impl CallSite {
    fn id(&self) -> usize {
        match self {
            Self::Stmt(s) => stmt_id(s),
            Self::Expr(e) => expr_id(e),
        }
    }

    fn name(&self) -> &str {
        match self {
            Self::Stmt(s) => match &**s {
                Stmt::Call(c) => &c.name,
                _ => "",
            },
            Self::Expr(e) => match &**e {
                Expr::Index(ix) => &ix.name,
                _ => "",
            },
        }
    }

    fn args(&self) -> &[Option<ExprRef>] {
        match self {
            Self::Stmt(s) => match &**s {
                Stmt::Call(c) => &c.args,
                _ => &[],
            },
            Self::Expr(e) => match &**e {
                Expr::Index(ix) => &ix.args,
                _ => &[],
            },
        }
    }

    fn is_explicit_call(&self) -> bool {
        match self {
            Self::Stmt(s) => matches!(&**s, Stmt::Call(c) if c.explicit),
            Self::Expr(_) => false,
        }
    }
}

fn integral_args_for(name: &str) -> &'static [usize] {
    match name {
        "TIMEINTOINTERVAL" | "IFTIME" => &[0, 1, 2],
        "SERIALOPEN" => &[1],
        "TIMER" => &[0],
        _ => &[],
    }
}

fn timeout_arg_for(name: &str) -> Option<usize> {
    match name {
        "HTTPPUT" => Some(8),
        "FTPCLIENT" => Some(10),
        _ => None,
    }
}

fn us_per_unit(unit: i64) -> i128 {
    match unit {
        0 => 1,
        1 => 1_000,
        2 => 1_000_000,
        3 => 60_000_000,
        4 => 3_600_000_000,
        5 => 86_400_000_000,
        _ => 0,
    }
}

// TODO(P4): use `expressions::_stmt_exprs` once its return type is fixed.
fn stmt_exprs_compat(stmt: &Stmt) -> Vec<Option<ExprRef>> {
    match stmt {
        Stmt::Assignment(a) => vec![a.target.clone(), a.value.clone()],
        Stmt::If(i) => vec![i.cond.clone()],
        Stmt::ElseIf(e) => vec![e.cond.clone()],
        Stmt::DoLoop(d) => vec![d.cond.clone()],
        Stmt::For(f) => vec![
            f.var.clone(),
            f.start.clone(),
            f.end.clone(),
            f.step.clone(),
        ],
        Stmt::Call(c) if c.tail_expr.is_some() => vec![c.tail_expr.clone()],
        Stmt::Instruction(i) => i.args.clone(),
        Stmt::Call(c) => c.args.clone(),
        _ => Vec::new(),
    }
}

/// Port of `checker/intervals.py::gate_text`.
pub fn gate_text() -> Option<String> {
    todo!("checker/intervals.py::gate_text (omitted: stmt: untyped)")
}

/// Port of `checker/intervals.py::gate_text`.
pub fn gate_text_with(stmt: &Stmt) -> Option<String> {
    let text = stmt.text();
    if let Stmt::If(i) = stmt
        && i.inline.is_some()
        && !text.is_empty()
    {
        let stripped = _LITERAL_RE.sub("\"\"", text);
        if let Some(m) = _THEN_RE.search(&stripped) {
            return Some(pystr::slice(&stripped, None, i64::try_from(m.start0()).ok()).to_string());
        }
        return Some(text.to_string());
    }
    Some(text.to_string())
}

/// Port of `checker/intervals.py::timeout_gate`.
pub fn timeout_gate(_seen: bool, _text: Option<&str>) {
    todo!(
        "checker/intervals.py::timeout_gate (signature returns () instead of (Option<bool>, bool))"
    )
}

/// Port of `checker/intervals.py::timeout_gate`.
pub fn timeout_gate_with(seen: bool, text: Option<&str>) -> (Option<bool>, bool) {
    let code = _LITERAL_RE.sub("\"\"", text.unwrap_or(""));
    let mut mode = seen;
    let mut gate: Option<bool> = Some(false);
    let mut queried = false;
    for m in _MODE_RE.finditer(&code) {
        let word = pystr::upper(m.group(1).unwrap_or(""));
        if word == "HTTPPUT" || word == "FTPCLIENT" {
            if !queried {
                gate = Some(mode);
                queried = true;
            }
        } else if word == "IFTIME" || word == "TIMEINTOINTERVAL" {
            if queried && !mode {
                gate = None;
            }
            mode = true;
        } else {
            mode = false;
        }
    }
    (gate, mode)
}

/// Port of `checker/intervals.py::check`.
pub fn check(_program: &Program, _diags: &mut Diagnostics) {
    todo!("checker/intervals.py::check (omitted: profile: untyped, env: consteval.Env)")
}

/// Port of `checker/intervals.py::check`.
pub fn check_with(
    program: &ProgramRef,
    profile: &'static Profile,
    diags: &mut Diagnostics,
    env: &Env,
) {
    let wide = wide_constants_with(program, profile);
    let mut known_cache: Option<&KnownNames> = None;
    let ctx = context::of(program, Some(profile), Some(env.clone()));
    let positions: HashMap<usize, usize> = program
        .statements
        .iter()
        .enumerate()
        .map(|(i, st)| (stmt_id(st), i))
        .collect();
    let mut in_table = false;
    let mut blocked_table = false;
    let mut scan: Option<(i64, i64)> = None;
    let mut seen = false;
    let empty_known = BTreeSet::new();

    for stmt in &program.statements {
        let (gate, next_seen) = timeout_gate_with(seen, Some(stmt.text()));
        seen = next_seen;
        _check_timeout_value_with(stmt, diags, env, profile, &wide, program);
        if gate == Some(true) {
            _check_timeout_with(stmt, diags, env, profile, &wide);
        }
        let kw = match &**stmt {
            Stmt::Instruction(i) => pystr::upper(&i.keyword),
            _ => String::new(),
        };
        if let Stmt::Instruction(i) = &**stmt
            && kw == "DATATABLE"
        {
            in_table = true;
            let _ = *known_cache.get_or_insert_with(|| ctx.known_names());
            blocked_table = i
                .args
                .iter()
                .skip(1)
                .flatten()
                .any(|a| !expressions::_undefined_names(Some(a), &empty_known).is_empty());
        } else if kw == "ENDTABLE" {
            in_table = false;
            blocked_table = false;
        }
        if let Stmt::Instruction(i) = &**stmt
            && kw == "SCAN"
        {
            scan = _scan_step_with(i, env);
            continue;
        }
        if matches!(kw.as_str(), "NEXTSCAN" | "SUBSCAN" | "NEXTSUBSCAN")
            || matches!(&**stmt, Stmt::SubDef(_))
        {
            scan = None;
        }
        if !profile.single_pass {
            for call in _calls_with(stmt, &["FTPCLIENT"]) {
                if let Some(option) = _put_get_option_with(call.args(), env) {
                    diags.error_basic(
                        stmt.line(),
                        &format!("Invalid PutGetOption: {option}"),
                        stmt.file(),
                    );
                }
            }
        }
        for call in _calls_with(stmt, _ALL_INTEGRAL_NAMES) {
            if call.is_explicit_call() {
                continue;
            }
            let cname = pystr::upper(call.name());
            if cname == "TIMER" {
                let known = *known_cache.get_or_insert_with(|| ctx.known_names());
                let (owner, before) = _timer_context_with(stmt, &call, known, profile.single_pass);
                let table_only = matches!(
                    &*owner,
                    Stmt::Instruction(i)
                        if _TABLE_ONLY_INSTRUCTIONS.contains(&pystr::upper(&i.keyword).as_str())
                );
                if before
                    || blocked_table
                    || (table_only && !in_table)
                    || (!profile.single_pass
                        && _timer_shadowed_with(program, stmt, &positions, diags))
                {
                    continue;
                }
            }
            let mut bad = Vec::new();
            for &idx in integral_args_for(&cname) {
                if idx < call.args().len() {
                    let arg = call.args()[idx].as_deref();
                    if not_integral_with(arg, env, profile, &wide)
                        || _nonfinite_literal_with(arg, env)
                    {
                        bad.push(idx);
                    }
                }
            }
            for _ in &bad {
                diags.error_basic(stmt.line(), NOT_INTEGRAL, stmt.file());
            }
            if bad.is_empty() && _FUNCTIONS.contains(&cname.as_str()) {
                let after_error = diags.errors().iter().any(|d| d.line < stmt.line());
                _check_call_with(&call, scan, stmt, diags, env, after_error);
            }
        }
    }
}

/// Port of `checker/intervals.py::_put_get_option`.
pub fn _put_get_option() -> Option<i64> {
    todo!("checker/intervals.py::_put_get_option (omitted: call: untyped, env: consteval.Env)")
}

/// Port of `checker/intervals.py::_put_get_option`.
pub fn _put_get_option_with(args: &[Option<ExprRef>], env: &Env) -> Option<i64> {
    if args.len() < 6 {
        return None;
    }
    let value = consteval::value_of(args[5].as_deref(), env).ok()?;
    let option = match value {
        ConstValue::Str(_) => return None,
        ConstValue::BigInt(_) => _INT_MIN,
        ConstValue::Int(v) => {
            if v < _INT_MIN {
                return None;
            }
            if v <= _INT_MAX { v } else { _INT_MIN }
        }
        ConstValue::Float(f) | ConstValue::CFloat(f) => {
            if !f.is_finite() || f < _INT_MIN_F64 {
                return None;
            }
            if f > _F32_MAX {
                _INT_MAX
            } else if f <= _INT_MAX_F64 {
                pystr::py_int_from_f64(f)?
            } else {
                _INT_MIN
            }
        }
    };
    if option.abs() >= 30 {
        Some(option)
    } else {
        None
    }
}

/// Port of `checker/intervals.py::_timer_context`.
pub fn _timer_context() {
    todo!(
        "checker/intervals.py::_timer_context (omitted: stmt: untyped, call: untyped, known: untyped, single_pass: untyped)"
    )
}

/// Port of `checker/intervals.py::_timer_context`.
pub fn _timer_context_with(
    stmt: &StmtRef,
    call: &CallSite,
    known: &KnownNames,
    single_pass: bool,
) -> (StmtRef, bool) {
    let _ = known;
    if call.id() == stmt_id(stmt) {
        return (Rc::clone(stmt), false);
    }
    let mut before = false;
    for expr in stmt_exprs_compat(stmt) {
        if !(single_pass && matches!(&**stmt, Stmt::Call(_))) {
            before = false;
        }
        for node in symbols::name_nodes(expr.as_ref()) {
            if expr_id(&node) == call.id() {
                return (Rc::clone(stmt), before);
            }
            // TODO(P4): check `node.name.upper() not in known or id(node) in known.out_of_scope`
            // once `KnownNames` fields are ported.
            let _ = &node;
        }
    }
    if let Stmt::If(i) = &**stmt {
        for inner in i
            .inline
            .iter()
            .flatten()
            .chain(i.inline_else.iter().flatten())
        {
            if _calls_with(inner, &["TIMER"])
                .iter()
                .any(|c| c.id() == call.id())
            {
                return _timer_context_with(inner, call, known, single_pass);
            }
        }
    }
    (Rc::clone(stmt), false)
}

/// Port of `checker/intervals.py::_timer_shadowed`.
pub fn _timer_shadowed() {
    todo!(
        "checker/intervals.py::_timer_shadowed (omitted: program: untyped, stmt: untyped, positions: untyped, diags: untyped)"
    )
}

/// Port of `checker/intervals.py::_timer_shadowed`.
pub fn _timer_shadowed_with<S: std::hash::BuildHasher>(
    program: &ProgramRef,
    stmt: &StmtRef,
    positions: &HashMap<usize, usize, S>,
    diags: &Diagnostics,
) -> bool {
    let stmt_pos = positions.get(&stmt_id(stmt)).copied().unwrap_or(0);
    let view = context::of(program, None, None).scoped_view(stmt, true);
    let errors = diags.errors();
    for decl in &view.statements {
        if positions.get(&stmt_id(decl)).copied().unwrap_or(0) > stmt_pos {
            continue;
        }
        match &**decl {
            Stmt::SubDef(sd) if sd.params.iter().any(|p| pystr::upper(&p.name) == "TIMER") => {
                return true;
            }
            Stmt::Declaration(d)
                if d.kind == "DIM"
                    && d.vars
                        .borrow()
                        .iter()
                        .any(|v| pystr::upper(&v.name) == "TIMER") =>
            {
                return true;
            }
            Stmt::Instruction(i)
                if pystr::upper(&i.keyword) == "ALIAS"
                    && !errors
                        .iter()
                        .any(|d| d.line == decl.line() && d.file == decl.file())
                    && i.args.iter().skip(1).flatten().any(|a| {
                        matches!(&**a, Expr::Name(n) if pystr::upper(&n.name) == "TIMER")
                            || matches!(&**a, Expr::Index(ix) if pystr::upper(&ix.name) == "TIMER")
                    }) =>
            {
                return true;
            }
            _ => {}
        }
    }
    false
}

/// Port of `checker/intervals.py::_nonfinite_literal`.
pub fn _nonfinite_literal(_expr: Option<&Expr>) -> bool {
    todo!("checker/intervals.py::_nonfinite_literal (omitted: env: untyped)")
}

/// Port of `checker/intervals.py::_nonfinite_literal`.
pub fn _nonfinite_literal_with(mut expr: Option<&Expr>, env: &Env) -> bool {
    while let Some(Expr::Unary(u)) = expr
        && (u.op == "+" || u.op == "-")
    {
        expr = u.operand.as_deref();
    }
    if let Some(Expr::Name(n)) = expr {
        let up = pystr::upper(&n.name);
        (up == "NAN" || up == "INF") && !env.consts.borrow().contains_key(&up)
    } else {
        false
    }
}

/// Port of `checker/intervals.py::_check_timeout_value`.
pub fn _check_timeout_value(_stmt: &Stmt, _diags: &mut Diagnostics) {
    todo!(
        "checker/intervals.py::_check_timeout_value (omitted: env: untyped, profile: untyped, wide: untyped, program: untyped)"
    )
}

/// Port of `checker/intervals.py::_check_timeout_value`.
pub fn _check_timeout_value_with(
    stmt: &StmtRef,
    diags: &mut Diagnostics,
    env: &Env,
    profile: &'static Profile,
    wide: &BTreeSet<String>,
    program: &ProgramRef,
) {
    for call in _calls_with(stmt, _TIMEOUT_NAMES) {
        if _negative_timeout_with(&call, env, profile, wide, program) {
            diags.error(
                stmt.line(),
                _TIMEOUT_NEGATIVE,
                stmt.file(),
                ShowFile::Bool(true),
                stmt_id(stmt),
                "",
                "",
            );
        }
    }
}

/// Port of `checker/intervals.py::_negative_timeout`.
pub fn _negative_timeout() -> bool {
    todo!(
        "checker/intervals.py::_negative_timeout (omitted: call: untyped, env: untyped, profile: untyped, wide: untyped, program: untyped)"
    )
}

/// Port of `checker/intervals.py::_negative_timeout`.
pub fn _negative_timeout_with(
    call: &CallSite,
    env: &Env,
    profile: &'static Profile,
    wide: &BTreeSet<String>,
    program: &ProgramRef,
) -> bool {
    if call.is_explicit_call() {
        return false;
    }
    let Some(i) = timeout_arg_for(&pystr::upper(call.name())) else {
        return false;
    };
    let args = call.args();
    if i >= args.len() {
        return false;
    }
    let Some(arg) = args[i].as_deref() else {
        return false;
    };
    let Some(TimeoutValue::Int(v)) = timeout_value_with(Some(arg), Some(env), profile, wide) else {
        return false;
    };
    if _signed32(10 * v) >= 0 {
        return false;
    }
    profile.single_pass || !_undefined_sibling_with(args, i, program, profile)
}

/// Port of `checker/intervals.py::_undefined_sibling`.
pub fn _undefined_sibling(_call: &Index, _i: i64, _program: &Program) -> bool {
    todo!("checker/intervals.py::_undefined_sibling (omitted: profile: untyped)")
}

/// Port of `checker/intervals.py::_undefined_sibling`.
pub fn _undefined_sibling_with(
    args: &[Option<ExprRef>],
    i: usize,
    program: &ProgramRef,
    profile: &'static Profile,
) -> bool {
    let _known = context::of(program, Some(profile), None).known_names();
    let empty_known = BTreeSet::new();
    args.iter().enumerate().any(|(j, a)| {
        j != i
            && a.as_deref()
                .is_some_and(|e| !expressions::_undefined_names(Some(e), &empty_known).is_empty())
    })
}

/// Port of `checker/intervals.py::_needs_integer_fold`.
pub fn _needs_integer_fold() -> bool {
    todo!("checker/intervals.py::_needs_integer_fold (omitted: arg: untyped)")
}

/// Port of `checker/intervals.py::_needs_integer_fold`.
pub fn _needs_integer_fold_with(mut arg: Option<&Expr>) -> bool {
    while let Some(Expr::Unary(u)) = arg
        && (u.op == "-" || u.op == "+")
    {
        if u.op == "-" {
            return true;
        }
        arg = u.operand.as_deref();
    }
    matches!(arg, Some(Expr::Binary(b)) if matches!(b.op.as_str(), "+" | "-" | "*"))
}

/// Port of `checker/intervals.py::fold_timeout_expressions`.
pub fn fold_timeout_expressions(_program: &Program, _diags: &mut Diagnostics) {
    todo!(
        "checker/intervals.py::fold_timeout_expressions (omitted: profile: untyped, env: untyped, reported: untyped)"
    )
}

/// Port of `checker/intervals.py::fold_timeout_expressions`.
pub fn fold_timeout_expressions_with(
    program: &ProgramRef,
    profile: &'static Profile,
    diags: &mut Diagnostics,
    env: &Env,
    reported: &[DiagRef],
) {
    let stmts = &program.statements;
    let mut neg_lines: HashMap<(String, i64), VecDeque<DiagRef>> = HashMap::new();
    for d in &diags.items {
        if d.severity == "error" && d.message == _TIMEOUT_NEGATIVE {
            neg_lines
                .entry((d.file.clone(), d.line))
                .or_default()
                .push_back(Rc::clone(d));
        }
    }
    let wide = wide_constants_with(program, profile);
    let mut lines: HashMap<String, Vec<(i64, usize)>> = HashMap::new();
    let mut first: HashMap<(String, i64), usize> = HashMap::new();
    for (i, st) in stmts.iter().enumerate() {
        lines
            .entry(st.file().to_string())
            .or_default()
            .push((st.line(), i));
        first.entry((st.file().to_string(), st.line())).or_insert(i);
    }
    for v in lines.values_mut() {
        v.sort_unstable();
    }

    let position = |d: &DiagRef| -> usize {
        let Some(entries) = lines.get(&d.file) else {
            return 0;
        };
        let k = entries.partition_point(|&(line, _)| line < d.line);
        if k < entries.len() {
            entries[k].1
        } else {
            stmts.len()
        }
    };

    let mut known_loaded = false;
    let empty_known = BTreeSet::new();
    let mut seen = false;
    let mut family_error = false;
    let mut added: Vec<DiagRef> = Vec::new();

    for (k, stmt) in stmts.iter().enumerate() {
        let (gate, next_seen) = timeout_gate_with(seen, gate_text_with(stmt).as_deref());
        seen = next_seen;
        let mut inline_ids: BTreeSet<usize> = BTreeSet::new();
        if let Stmt::If(i) = &**stmt {
            for inner in i
                .inline
                .iter()
                .flatten()
                .chain(i.inline_else.iter().flatten())
            {
                for c in _calls_with(inner, _TIMEOUT_NAMES) {
                    inline_ids.insert(c.id());
                }
            }
        }
        for call in _calls_with(stmt, _TIMEOUT_NAMES) {
            if inline_ids.contains(&call.id()) || call.is_explicit_call() {
                continue;
            }
            let Some(i) = timeout_arg_for(&pystr::upper(call.name())) else {
                continue;
            };
            let args = call.args();
            if i >= args.len() {
                continue;
            }
            let Some(arg) = args[i].as_deref() else {
                continue;
            };
            let key = (stmt.file().to_string(), stmt.line());
            let mut own: Option<DiagRef> = None;
            if _negative_timeout_with(&call, env, profile, &wide, program) {
                own = neg_lines.get_mut(&key).and_then(VecDeque::pop_front);
            }
            let value = timeout_value_with(Some(arg), Some(env), profile, &wide);
            if _needs_integer_fold_with(Some(arg)) && matches!(value, Some(TimeoutValue::Int(_))) {
                let line_first = first[&(stmt.file().to_string(), stmt.line())];
                let counted: Vec<&DiagRef> = diags
                    .items
                    .iter()
                    .filter(|d| {
                        d.severity == "error"
                            && d.line != 0
                            && d.message != _TIMEOUT_NEGATIVE
                            && !added.iter().any(|a| Rc::ptr_eq(a, d))
                            && (profile.single_pass || reported.iter().any(|r| Rc::ptr_eq(r, d)))
                    })
                    .collect();
                let mut before = family_error
                    || counted.iter().any(|d| position(d) < line_first)
                    || (k > line_first && counted.iter().any(|d| position(d) == line_first));
                if !before && profile.single_pass {
                    if !known_loaded {
                        let _ = context::of(program, Some(profile), None).known_names();
                        known_loaded = true;
                    }
                    before = args[..i].iter().any(|a| {
                        a.as_deref().is_some_and(|e| {
                            !expressions::_undefined_names(Some(e), &empty_known).is_empty()
                        })
                    });
                }
                if before && let Some(g) = gate {
                    if let Some(ref own_d) = own
                        && let Some(idx) = diags.items.iter().position(|d| Rc::ptr_eq(d, own_d))
                    {
                        diags.items.remove(idx);
                    }
                    if !g {
                        let msg = expressions::_arity_message(
                            "constint",
                            stmt.text(),
                            stmt.at_eol(),
                            true,
                        );
                        diags.error_basic(stmt.line(), &msg, stmt.file());
                        if let Some(last) = diags.items.last() {
                            added.push(Rc::clone(last));
                        }
                        family_error = true;
                    }
                    continue;
                }
            }
            if own.is_some() {
                family_error = true;
            }
        }
    }
}

/// Port of `checker/intervals.py::order_timeout_negative`.
pub fn order_timeout_negative(_diags: &mut Diagnostics, _program: &Program) {
    todo!("checker/intervals.py::order_timeout_negative (omitted: profile: untyped)")
}

/// Port of `checker/intervals.py::order_timeout_negative`.
pub fn order_timeout_negative_with(diags: &mut Diagnostics, profile: &Profile, program: &Program) {
    if !profile.single_pass {
        return;
    }
    let targets: Vec<DiagRef> = diags
        .items
        .iter()
        .filter(|d| d.severity == "error" && d.message == _TIMEOUT_NEGATIVE)
        .cloned()
        .collect();
    for d in targets {
        let Some(at) = diags.items.iter().position(|o| Rc::ptr_eq(o, &d)) else {
            continue;
        };
        let same: Vec<&StmtRef> = program
            .statements
            .iter()
            .filter(|st| st.file() == d.file && st.line() == d.line)
            .collect();
        let mine = same
            .iter()
            .position(|st| stmt_id(st) == d.stmt)
            .unwrap_or(same.len());
        let mut after = Vec::new();
        for (j, o) in diags.items.iter().enumerate() {
            if Rc::ptr_eq(o, &d)
                || o.severity != "error"
                || o.file != d.file
                || o.line != d.line
                || !o.message.ends_with(" is not defined.")
            {
                continue;
            }
            let sym = o.message.strip_suffix(" is not defined.").unwrap_or("");
            let pat = format!(r"(?i)\b{}\b", pyre::escape(sym));
            let name_re = pyre::Regex::new(&pat, 0);
            let owner = same
                .iter()
                .position(|st| name_re.search(st.text()).is_some())
                .unwrap_or(0);
            if owner <= mine {
                after.push(j);
            }
        }
        if let Some(&max_after) = after.iter().max()
            && max_after > at
        {
            let removed = diags.items.remove(at);
            diags.items.insert(max_after, removed);
        }
    }
}

/// Port of `checker/intervals.py::_check_timeout`.
pub fn _check_timeout(_stmt: &Stmt, _diags: &mut Diagnostics) {
    todo!(
        "checker/intervals.py::_check_timeout (omitted: env: untyped, profile: untyped, wide: untyped)"
    )
}

/// Port of `checker/intervals.py::_check_timeout`.
pub fn _check_timeout_with(
    stmt: &StmtRef,
    diags: &mut Diagnostics,
    env: &Env,
    profile: &Profile,
    wide: &BTreeSet<String>,
) {
    for call in _calls_with(stmt, _TIMEOUT_NAMES) {
        if call.is_explicit_call() {
            continue;
        }
        let Some(i) = timeout_arg_for(&pystr::upper(call.name())) else {
            continue;
        };
        let args = call.args();
        if i >= args.len() {
            continue;
        }
        let Some(arg) = args[i].as_deref() else {
            continue;
        };
        if matches!(arg, Expr::String(_)) {
            diags.warning_basic(
                stmt.line(),
                "Parameter will truncate to the nearest integer.",
                stmt.file(),
            );
        } else if matches!(arg, Expr::Name(n) if pystr::upper(&n.name) == "NAN")
            || (not_integral_with(Some(arg), env, profile, wide)
                && (!profile.single_pass || _float_typed_with(Some(arg), env, wide)))
            || timeout_value_with(Some(arg), Some(env), profile, wide) == Some(TimeoutValue::Wide)
        {
            diags.error_basic(stmt.line(), NOT_INTEGRAL, stmt.file());
        }
    }
}

/// Port of `checker/intervals.py::wide_constants`.
pub fn wide_constants(_program: &Program) -> BTreeSet<String> {
    todo!("checker/intervals.py::wide_constants (omitted: profile: untyped)")
}

/// Port of `checker/intervals.py::wide_constants`.
pub fn wide_constants_with(program: &Program, profile: &Profile) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for st in &program.statements {
        if let Stmt::ConstDecl(c) = &**st
            && let Some(val_expr) = &c.value
            && let Expr::Number(n) = &**val_expr
            && !n.text.starts_with('&')
            && _out_of_range_with(n.value.clone(), profile)
        {
            out.insert(pystr::upper(&c.name));
        }
    }
    out
}

/// Port of `checker/intervals.py::_out_of_range`.
pub fn _out_of_range() -> bool {
    todo!("checker/intervals.py::_out_of_range (omitted: value: untyped, profile: untyped)")
}

/// Port of `checker/intervals.py::_out_of_range`.
#[allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    reason = "matches Python float() and struct.pack('<f') rounding"
)]
pub fn _out_of_range_with(value: NumberValue, profile: &Profile) -> bool {
    if matches!(profile.name, "cr300" | "cr350") {
        let f = match value {
            NumberValue::Int(i) => i as f64,
            NumberValue::Float(f) => f,
            NumberValue::BigInt(s) => s.parse::<f64>().unwrap_or(f64::INFINITY),
        };
        let rounded = if f.abs() < 3.4e38 {
            f64::from(f as f32)
        } else {
            f64::INFINITY
        };
        return !(_INT_MIN_F64..=2_147_483_648.0).contains(&rounded);
    }
    match value {
        NumberValue::Int(i) => !(_INT_MIN..=_INT_MAX).contains(&i),
        NumberValue::Float(f) => !(_INT_MIN_F64..=_INT_MAX_F64).contains(&f),
        NumberValue::BigInt(_) => true,
    }
}

/// Port of `checker/intervals.py::not_integral`.
pub fn not_integral(_expr: Option<&Expr>) -> bool {
    todo!(
        "checker/intervals.py::not_integral (omitted: env: consteval.Env, profile: untyped, wide: untyped)"
    )
}

/// Port of `checker/intervals.py::not_integral`.
pub fn not_integral_with(
    expr: Option<&Expr>,
    env: &Env,
    profile: &Profile,
    wide: &BTreeSet<String>,
) -> bool {
    if expr.is_none() || !_plain(expr) {
        return false;
    }
    let Ok(value) = consteval::value_of(expr, env) else {
        return false;
    };
    let (num, is_whole) = match value {
        ConstValue::Str(_) => return false,
        ConstValue::Int(i) => (NumberValue::Int(i), true),
        ConstValue::BigInt(s) => (NumberValue::BigInt(s), true),
        ConstValue::Float(f) | ConstValue::CFloat(f) => {
            (NumberValue::Float(f), f.is_finite() && pystr::is_integer(f))
        }
    };
    if _float_typed_with(expr, env, wide) || !is_whole {
        return true;
    }
    let literal = match expr {
        Some(Expr::Unary(u)) if u.op == "-" => u.operand.as_deref(),
        _ => expr,
    };
    let Some(Expr::Number(n)) = literal else {
        return false;
    };
    if n.text.starts_with('&') {
        return false;
    }
    _out_of_range_with(num, profile)
}

/// Port of `checker/intervals.py::_plain`.
pub fn _plain(expr: Option<&Expr>) -> bool {
    match expr {
        Some(Expr::Number(_) | Expr::Name(_)) => true,
        Some(Expr::Unary(u)) => matches!(u.op.as_str(), "-" | "+") && _plain(u.operand.as_deref()),
        Some(Expr::Binary(b)) => {
            matches!(b.op.as_str(), "+" | "-" | "*" | "/" | "^" | "\\" | "MOD")
                && _plain(b.left.as_deref())
                && _plain(b.right.as_deref())
        }
        _ => false,
    }
}

/// Port of `checker/intervals.py::_float_typed`.
pub fn _float_typed(_expr: Option<&Expr>) -> bool {
    todo!("checker/intervals.py::_float_typed (omitted: env: consteval.Env, wide: untyped)")
}

/// Port of `checker/intervals.py::_float_typed`.
pub fn _float_typed_with(expr: Option<&Expr>, env: &Env, wide: &BTreeSet<String>) -> bool {
    match expr {
        Some(Expr::Number(n)) => {
            let text = pystr::upper(&n.text);
            !text.starts_with('&') && (text.contains('.') || text.contains('E'))
        }
        Some(Expr::Name(n)) => {
            if wide.contains(&pystr::upper(&n.name)) {
                return true;
            }
            let Ok(value) = consteval::value_of(expr, env) else {
                return false;
            };
            match value {
                ConstValue::Float(f) | ConstValue::CFloat(f) => {
                    !f.is_finite() || !pystr::is_integer(f)
                }
                _ => false,
            }
        }
        Some(Expr::Unary(u)) => _float_typed_with(u.operand.as_deref(), env, wide),
        Some(Expr::Binary(b)) => {
            matches!(b.op.as_str(), "/" | "^")
                || _float_typed_with(b.left.as_deref(), env, wide)
                || _float_typed_with(b.right.as_deref(), env, wide)
        }
        _ => false,
    }
}

/// Port of `checker/intervals.py::_signed32`.
pub fn _signed32(x: i64) -> i64 {
    let x = x & 0xFFFF_FFFF;
    if x >= (1_i64 << 31) {
        x - (1_i64 << 32)
    } else {
        x
    }
}

/// Port of `checker/intervals.py::timeout_value`.
pub fn timeout_value() {
    todo!(
        "checker/intervals.py::timeout_value (omitted: arg: untyped, env: untyped, profile: untyped, wide: untyped)"
    )
}

/// Port of `checker/intervals.py::timeout_value`.
pub fn timeout_value_with(
    arg: Option<&Expr>,
    env: Option<&Env>,
    profile: &Profile,
    wide: &BTreeSet<String>,
) -> Option<TimeoutValue> {
    let arg = arg?;
    if let Expr::Unary(u) = arg
        && (u.op == "-" || u.op == "+")
    {
        let mut neg_count = 0_usize;
        let mut lit: Option<&Expr> = Some(arg);
        while let Some(Expr::Unary(inner)) = lit
            && (inner.op == "-" || inner.op == "+")
        {
            if inner.op == "-" {
                neg_count += 1;
            }
            lit = inner.operand.as_deref();
        }
        let sign = if neg_count % 2 == 1 { -1 } else { 1 };
        if let Some(Expr::Number(n)) = lit {
            return _timeout_literal_with(n, sign, profile);
        }
        let val = timeout_value_with(lit, env, profile, wide)?;
        return match val {
            TimeoutValue::Int(v) => Some(TimeoutValue::Int(_signed32(sign * v))),
            TimeoutValue::Wide => Some(TimeoutValue::Wide),
        };
    }
    if let Expr::Binary(b) = arg
        && matches!(b.op.as_str(), "+" | "-" | "*")
    {
        let Some(TimeoutValue::Int(a)) = timeout_value_with(b.left.as_deref(), env, profile, wide)
        else {
            return None;
        };
        let Some(TimeoutValue::Int(right_b)) =
            timeout_value_with(b.right.as_deref(), env, profile, wide)
        else {
            return None;
        };
        let value = match b.op.as_str() {
            "+" => a + right_b,
            "-" => a - right_b,
            _ => a * right_b,
        };
        return Some(TimeoutValue::Int(_signed32(value)));
    }
    if let Expr::Number(n) = arg {
        return _timeout_literal_with(n, 1, profile);
    }
    if let Expr::Name(n) = arg {
        let name = pystr::upper(&n.name);
        if name == "TRUE" {
            return Some(TimeoutValue::Int(-1));
        }
        if name == "FALSE" {
            return Some(TimeoutValue::Int(0));
        }
        if wide.contains(&name) {
            return Some(TimeoutValue::Wide);
        }
        let env = env?;
        let Ok(ConstValue::Int(v)) = consteval::value_of(Some(arg), env) else {
            return None;
        };
        if (_INT_MIN..=_INT_MAX).contains(&v) {
            return Some(TimeoutValue::Int(v));
        }
        return _decimal_with(v.unsigned_abs(), if v < 0 { -1 } else { 1 }, profile, false);
    }
    None
}

/// Port of `checker/intervals.py::_timeout_literal`.
pub fn _timeout_literal(_lit: &Number, _sign: i64) {
    todo!("checker/intervals.py::_timeout_literal (omitted: profile: untyped)")
}

/// Port of `checker/intervals.py::_timeout_literal`.
pub fn _timeout_literal_with(lit: &Number, sign: i64, profile: &Profile) -> Option<TimeoutValue> {
    let text = pystr::upper(&lit.text);
    if text.starts_with("&H") || text.starts_with("&B") {
        let digits = pystr::slice(&text, Some(2), None);
        let (base, limit) = if pystr::char_at(&text, 1) == Some('H') {
            (16_u32, 8_usize)
        } else {
            (2_u32, 32_usize)
        };
        if digits.is_empty()
            || pystr::len(digits) > limit
            || !digits.chars().all(|c| c.is_digit(base))
        {
            return None;
        }
        let parsed = i64::from_str_radix(digits, base).ok()?;
        return Some(TimeoutValue::Int(_signed32(sign * _signed32(parsed))));
    }
    if text.starts_with('&') {
        return None;
    }
    if text.contains('.') || text.contains('E') {
        return Some(TimeoutValue::Wide);
    }
    if text.is_empty() || !text.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    match text.parse::<u64>() {
        Ok(mag) => _decimal_with(mag, sign, profile, true),
        Err(_) => Some(TimeoutValue::Wide),
    }
}

/// Port of `checker/intervals.py::_decimal`.
pub fn _decimal(_magnitude: i64, _sign: i64, _literal: bool) {
    todo!("checker/intervals.py::_decimal (omitted: profile: untyped)")
}

/// Port of `checker/intervals.py::_decimal`.
#[allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    reason = "magnitude is clamped or checked against 2^31/2^32"
)]
pub fn _decimal_with(
    magnitude: u64,
    sign: i64,
    profile: &Profile,
    literal: bool,
) -> Option<TimeoutValue> {
    if profile.single_pass {
        if magnitude >= (1_u64 << 32) {
            return Some(TimeoutValue::Wide);
        }
        let rounded = (magnitude as f64) as f32;
        if f64::from(rounded) > 2_147_483_648.0 {
            return Some(TimeoutValue::Wide);
        }
        let clamped = magnitude.min(_INT_MAX as u64) as i64;
        return Some(TimeoutValue::Int(sign * clamped));
    }
    if magnitude > (_INT_MAX as u64) {
        if !literal {
            return None;
        }
        if sign < 0 && magnitude == (1_u64 << 31) {
            return Some(TimeoutValue::Int(_INT_MIN));
        }
        return Some(TimeoutValue::Wide);
    }
    Some(TimeoutValue::Int(sign * (magnitude as i64)))
}

/// Port of `checker/intervals.py::_check_call`.
pub fn _check_call(_call: &Stmt, _stmt: &Stmt, _diags: &mut Diagnostics, _after_error: bool) {
    todo!("checker/intervals.py::_check_call (omitted: scan: untyped, env: untyped)")
}

/// Port of `checker/intervals.py::_check_call`.
pub fn _check_call_with(
    call: &CallSite,
    scan: Option<(i64, i64)>,
    stmt: &Stmt,
    diags: &mut Diagnostics,
    env: &Env,
    after_error: bool,
) {
    let args = call.args();
    if args.len() != 3 {
        return;
    }
    let Some(tinto_raw) = _int_with(args[0].as_deref(), env) else {
        return;
    };
    let Some(interval_raw) = _int_with(args[1].as_deref(), env) else {
        return;
    };
    let Some(unit) = _int_with(args[2].as_deref(), env) else {
        return;
    };
    if unit == _MONTH || tinto_raw == _INT_MIN || interval_raw == _INT_MIN {
        return;
    }
    if after_error && tinto_raw.min(interval_raw) < 0 {
        return;
    }
    let per_unit = us_per_unit(unit);
    let (mut t_sec, mut t_us) = split_i128(i128::from(tinto_raw) * per_unit);
    let (i_sec, i_us) = split_i128(i128::from(interval_raw) * per_unit);
    if t_us < 0 {
        t_sec += i_sec - 1;
        t_us += 1_000_000;
    } else if t_sec < 0 {
        t_sec += i_sec;
    }
    if i_sec < 0 || i_us < 0 || t_sec < 0 || i_sec < t_sec || (t_sec == i_sec && i_us <= t_us) {
        diags.error_basic(
            stmt.line(),
            &format!(
                "Interval ({interval_raw}) must be > 0 and TimeInto Interval ({tinto_raw}) must be < Interval."
            ),
            stmt.file(),
        );
    }
    let Some((scan_sec, scan_us)) = scan else {
        return;
    };
    let (interval, tinto, step) = if i_sec < 3601 {
        (
            i_sec * 10000 + _cdiv(i_us, 100),
            t_sec * 10000 + _cdiv(t_us, 100),
            _wrap32(scan_sec * 10000 + _cdiv(scan_us, 100)),
        )
    } else {
        if i_us != 0 || t_us != 0 {
            diags.error_basic(
                stmt.line(),
                "Illegal sub seconds in time into and/or interval when interval > 3600 secs.",
                stmt.file(),
            );
        }
        (i_sec, t_sec, scan_sec)
    };
    if step != 0 && (_cmod(interval, step) != 0 || _cmod(tinto, step) != 0) {
        diags.error_basic(
            stmt.line(),
            "Scan interval must be integrally divisible into interval.",
            stmt.file(),
        );
    }
}

fn split_i128(total_us: i128) -> (i64, i64) {
    let sec = total_us / 1_000_000;
    let us = total_us - sec * 1_000_000;
    (
        i64::try_from(sec).expect("seconds fit in i64"),
        i64::try_from(us).expect("microseconds fit in i64"),
    )
}

/// Port of `checker/intervals.py::_scan_step`.
pub fn _scan_step(_stmt: &Instruction) -> Option<(i64, i64)> {
    todo!("checker/intervals.py::_scan_step (omitted: env: consteval.Env)")
}

/// Port of `checker/intervals.py::_scan_step`.
pub fn _scan_step_with(stmt: &Instruction, env: &Env) -> Option<(i64, i64)> {
    super::instr_rules::scan_split(stmt, env)
}

/// Port of `checker/intervals.py::_wrap32`.
pub fn _wrap32(n: i64) -> i64 {
    let n = n & 0xFFFF_FFFF;
    if n >= (1_i64 << 31) {
        n - (1_i64 << 32)
    } else {
        n
    }
}

/// Port of `checker/intervals.py::_split`.
pub fn _split(total_us: i64) -> (i64, i64) {
    let sec = _cdiv(total_us, 1_000_000);
    (sec, total_us - sec * 1_000_000)
}

/// Port of `checker/intervals.py::_cdiv`.
pub fn _cdiv(a: i64, b: i64) -> i64 {
    let q = a.unsigned_abs() / b.unsigned_abs();
    let q = i64::try_from(q).unwrap_or(i64::MAX);
    if (a >= 0) == (b > 0) { q } else { -q }
}

/// Port of `checker/intervals.py::_cmod`.
pub fn _cmod(a: i64, b: i64) -> i64 {
    a - b * _cdiv(a, b)
}

/// Port of `checker/intervals.py::_int`.
pub fn _int(_expr: Option<&Expr>) -> Option<i64> {
    todo!("checker/intervals.py::_int (omitted: env: consteval.Env)")
}

/// Port of `checker/intervals.py::_int`.
pub fn _int_with(expr: Option<&Expr>, env: &Env) -> Option<i64> {
    let e = expr?;
    if !consteval::is_constant(Some(e), env) {
        return None;
    }
    let value = consteval::value_of(Some(e), env).ok()?;
    let v = match value {
        ConstValue::Str(_) | ConstValue::BigInt(_) => return None,
        ConstValue::Int(i) => i,
        ConstValue::Float(f) | ConstValue::CFloat(f) => {
            if !f.is_finite() || !pystr::is_integer(f) {
                return None;
            }
            pystr::py_int_from_f64(f)?
        }
    };
    if let Expr::Number(n) = e
        && n.text.starts_with('&')
    {
        if v > 0xFFFF_FFFF {
            return None;
        }
        let v = v & 0xFFFF_FFFF;
        return Some(if v >= (1_i64 << 31) {
            v - (1_i64 << 32)
        } else {
            v
        });
    }
    let inner = match e {
        Expr::Unary(u) if u.op == "-" => u.operand.as_deref(),
        _ => Some(e),
    };
    if matches!(env.profile.name, "cr300" | "cr350") && matches!(inner, Some(Expr::Number(_))) {
        return Some(v.clamp(-_INT_MAX, _INT_MAX));
    }
    (_INT_MIN..=_INT_MAX).contains(&v).then_some(v)
}

/// Port of `checker/intervals.py::_calls`.
pub fn _calls(_stmt: &Stmt) {
    todo!("checker/intervals.py::_calls (omitted: names: untyped)")
}

/// Port of `checker/intervals.py::_calls`.
pub fn _calls_with(stmt: &StmtRef, names: &[&str]) -> Vec<CallSite> {
    let mut out = Vec::new();
    let mut exprs: Vec<Option<ExprRef>> = Vec::new();
    match &**stmt {
        Stmt::If(i) => {
            exprs.push(i.cond.clone());
            for inner in i
                .inline
                .iter()
                .flatten()
                .chain(i.inline_else.iter().flatten())
            {
                out.extend(_calls_with(inner, names));
            }
        }
        Stmt::ElseIf(e) => exprs.push(e.cond.clone()),
        Stmt::DoLoop(d) => exprs.push(d.cond.clone()),
        Stmt::Call(c) => {
            if names.contains(&pystr::upper(&c.name).as_str()) {
                out.push(CallSite::Stmt(Rc::clone(stmt)));
            }
            exprs.extend(c.args.iter().cloned());
        }
        Stmt::Assignment(a) => {
            exprs.push(a.target.clone());
            exprs.push(a.value.clone());
        }
        Stmt::Instruction(i) => exprs.extend(i.args.iter().cloned()),
        Stmt::For(f) => {
            exprs.push(f.start.clone());
            exprs.push(f.end.clone());
            exprs.push(f.step.clone());
        }
        _ => {}
    }
    let mut stack: VecDeque<ExprRef> = exprs.into_iter().flatten().collect();
    while let Some(expr) = stack.pop_front() {
        match &*expr {
            Expr::Index(ix) => {
                if names.contains(&pystr::upper(&ix.name).as_str()) {
                    out.push(CallSite::Expr(Rc::clone(&expr)));
                }
                stack.extend(ix.args.iter().flatten().cloned());
            }
            Expr::Unary(u) => {
                if let Some(op) = &u.operand {
                    stack.push_back(Rc::clone(op));
                }
            }
            Expr::Binary(b) => {
                if let Some(l) = &b.left {
                    stack.push_back(Rc::clone(l));
                }
                if let Some(r) = &b.right {
                    stack.push_back(Rc::clone(r));
                }
            }
            Expr::Member(m) => {
                if let Some(args) = &m.args {
                    stack.extend(args.iter().flatten().cloned());
                }
            }
            _ => {}
        }
    }
    out
}

#[cfg(test)]
#[allow(
    clippy::unreadable_literal,
    reason = "expected values match Python outputs"
)]
mod tests {
    use super::*;
    use crate::profiles;
    use crate::syntax::{Binary, Call, ConstDecl, If, Name, Node, StmtBase, StringLit, Unary};

    fn node(line: i64) -> Node {
        Node::new(line, "test.cr6")
    }

    fn num_i(line: i64, v: i64, text: &str) -> ExprRef {
        Rc::new(Expr::Number(Number {
            node: node(line),
            value: NumberValue::Int(v),
            text: text.to_string(),
        }))
    }

    fn num_f(line: i64, v: f64, text: &str) -> ExprRef {
        Rc::new(Expr::Number(Number {
            node: node(line),
            value: NumberValue::Float(v),
            text: text.to_string(),
        }))
    }

    fn name_e(line: i64, s: &str) -> ExprRef {
        Rc::new(Expr::Name(Name {
            node: node(line),
            name: s.to_string(),
        }))
    }

    #[test]
    fn timeout_gate_matches_python() {
        assert_eq!(
            timeout_gate_with(false, Some("HTTPPut(1,2,3,4,5,6,7,8,9)")),
            (Some(false), false)
        );
        assert_eq!(
            timeout_gate_with(false, Some("IfTime(0,1,Sec) : HTTPPut(1,2,3,4,5,6,7,8,9)")),
            (Some(true), true)
        );
        assert_eq!(
            timeout_gate_with(true, Some("HTTPPut(1,2,3,4,5,6,7,8,9)")),
            (Some(true), true)
        );
        assert_eq!(
            timeout_gate_with(true, Some("Timer(0,Sec,0) : HTTPPut(1,2,3,4,5,6,7,8,9)")),
            (Some(false), false)
        );
        assert_eq!(
            timeout_gate_with(false, Some("HTTPPut(1,2,3,4,5,6,7,8,9) : IfTime(0,1,Sec)")),
            (None, true)
        );
        assert_eq!(
            timeout_gate_with(false, Some("\"IfTime(\" : HTTPPut(1,2,3,4,5,6,7,8,9)")),
            (Some(false), false)
        );
    }

    #[test]
    fn gate_text_matches_python() {
        let mut base = StmtBase::new(1, "test.cr6");
        base.text = "If IfTime(0,1,Sec) Then HTTPPut(1)".to_string();
        let stmt = Stmt::If(If {
            base,
            cond: None,
            inline: Some(Vec::new()),
            inline_else: None,
        });
        assert_eq!(
            gate_text_with(&stmt),
            Some("If IfTime(0,1,Sec) ".to_string())
        );
    }

    #[test]
    fn integer_math_helpers_match_python() {
        assert_eq!(_signed32(0), 0);
        assert_eq!(_signed32((1 << 31) - 1), 2147483647);
        assert_eq!(_signed32(1 << 31), -2147483648);
        assert_eq!(_signed32((1 << 32) - 1), -1);
        assert_eq!(_signed32(-1), -1);
        assert_eq!(_signed32(-(1 << 31)), -2147483648);

        assert_eq!(_cdiv(7, 3), 2);
        assert_eq!(_cdiv(-7, 3), -2);
        assert_eq!(_cdiv(7, -3), -2);
        assert_eq!(_cdiv(-7, -3), 2);
        assert_eq!(_cdiv(0, 5), 0);

        assert_eq!(_cmod(7, 3), 1);
        assert_eq!(_cmod(-7, 3), -1);
        assert_eq!(_cmod(7, -3), 1);
        assert_eq!(_cmod(-7, -3), -1);
        assert_eq!(_cmod(0, 5), 0);

        assert_eq!(_split(1_500_000), (1, 500_000));
        assert_eq!(_split(-1_500_000), (-1, -500_000));
        assert_eq!(_split(0), (0, 0));
        assert_eq!(_split(-999_999), (0, -999_999));
    }

    #[test]
    fn decimal_and_timeout_literal_match_python() {
        let cr6 = profiles::get("cr6").unwrap();
        let cr300 = profiles::get("cr300").unwrap();

        assert_eq!(_decimal_with(10, 1, cr6, true), Some(TimeoutValue::Int(10)));
        assert_eq!(
            _decimal_with(1 << 31, -1, cr6, true),
            Some(TimeoutValue::Int(-2147483648))
        );
        assert_eq!(
            _decimal_with(1 << 31, 1, cr6, true),
            Some(TimeoutValue::Wide)
        );
        assert_eq!(_decimal_with(1 << 31, -1, cr6, false), None);

        assert_eq!(
            _decimal_with((1 << 31) - 1, 1, cr300, true),
            Some(TimeoutValue::Int(2147483647))
        );
        assert_eq!(
            _decimal_with(1 << 31, -1, cr300, true),
            Some(TimeoutValue::Int(-2147483647))
        );
        assert_eq!(
            _decimal_with((1 << 31) + 1000, 1, cr300, true),
            Some(TimeoutValue::Wide)
        );
        assert_eq!(
            _decimal_with(1 << 32, 1, cr300, true),
            Some(TimeoutValue::Wide)
        );

        assert_eq!(
            _timeout_literal_with(
                &Number {
                    node: node(1),
                    value: NumberValue::Int(0xFFFFFFFF),
                    text: "&HFFFFFFFF".to_string()
                },
                1,
                cr6
            ),
            Some(TimeoutValue::Int(-1))
        );
        assert_eq!(
            _timeout_literal_with(
                &Number {
                    node: node(1),
                    value: NumberValue::Int(0xFFFFFFFF),
                    text: "&HFFFFFFFF".to_string()
                },
                -1,
                cr6
            ),
            Some(TimeoutValue::Int(1))
        );
        assert_eq!(
            _timeout_literal_with(
                &Number {
                    node: node(1),
                    value: NumberValue::Int(5),
                    text: "&B101".to_string()
                },
                1,
                cr6
            ),
            Some(TimeoutValue::Int(5))
        );
        assert_eq!(
            _timeout_literal_with(
                &Number {
                    node: node(1),
                    value: NumberValue::Float(1.5),
                    text: "1.5".to_string()
                },
                1,
                cr6
            ),
            Some(TimeoutValue::Wide)
        );
        assert_eq!(
            _timeout_literal_with(
                &Number {
                    node: node(1),
                    value: NumberValue::Int(2147483648),
                    text: "2147483648".to_string()
                },
                -1,
                cr6
            ),
            Some(TimeoutValue::Int(-2147483648))
        );
        assert_eq!(
            _timeout_literal_with(
                &Number {
                    node: node(1),
                    value: NumberValue::Int(2147483648),
                    text: "2147483648".to_string()
                },
                1,
                cr6
            ),
            Some(TimeoutValue::Wide)
        );
    }

    #[test]
    fn out_of_range_and_put_get_option_match_python() {
        let cr6 = profiles::get("cr6").unwrap();
        let cr300 = profiles::get("cr300").unwrap();
        let env6 = Env::new(cr6);

        assert!(!_out_of_range_with(NumberValue::Int(2147483647), cr6));
        assert!(_out_of_range_with(NumberValue::Int(2147483648), cr6));
        assert!(!_out_of_range_with(NumberValue::Int(-2147483648), cr6));
        assert!(_out_of_range_with(NumberValue::Int(-2147483649), cr6));

        assert!(!_out_of_range_with(NumberValue::Int(2147483648), cr300));
        assert!(!_out_of_range_with(NumberValue::Int(2147483649), cr300));
        assert!(!_out_of_range_with(NumberValue::Int(2147483700), cr300));
        assert!(_out_of_range_with(NumberValue::Float(3.5e38), cr300));

        let mk_args = |e: ExprRef| vec![None, None, None, None, None, Some(e)];
        assert_eq!(
            _put_get_option_with(&mk_args(num_i(1, 29, "29")), &env6),
            None
        );
        assert_eq!(
            _put_get_option_with(&mk_args(num_i(1, 30, "30")), &env6),
            Some(30)
        );
        assert_eq!(
            _put_get_option_with(&mk_args(num_i(1, -30, "-30")), &env6),
            Some(-30)
        );
        assert_eq!(
            _put_get_option_with(&mk_args(num_i(1, 2147483648, "2147483648")), &env6),
            Some(-2147483648)
        );
        assert_eq!(
            _put_get_option_with(&mk_args(num_i(1, -2147483649, "-2147483649")), &env6),
            None
        );
        assert_eq!(
            _put_get_option_with(&mk_args(num_f(1, 1e39, "1e+39")), &env6),
            Some(2147483647)
        );
    }

    #[test]
    fn check_call_matches_python() {
        let cr6 = profiles::get("cr6").unwrap();
        let env6 = Env::new(cr6);

        let run = |tinto: i64, interval: i64, unit: i64, scan: Option<(i64, i64)>, after: bool| {
            let mut diags = Diagnostics::new();
            let call_expr = Rc::new(Expr::Index(Index {
                node: node(10),
                name: "IfTime".to_string(),
                args: vec![
                    Some(num_i(10, tinto, &tinto.to_string())),
                    Some(num_i(10, interval, &interval.to_string())),
                    Some(num_i(10, unit, &unit.to_string())),
                ],
            }));
            let st = Rc::new(Stmt::If(If {
                base: StmtBase::new(10, "t"),
                cond: Some(call_expr.clone()),
                inline: None,
                inline_else: None,
            }));
            let call = CallSite::Expr(call_expr);
            _check_call_with(&call, scan, &st, &mut diags, &env6, after);
            diags
                .items
                .into_iter()
                .map(|d| (d.line, d.message.clone()))
                .collect::<Vec<_>>()
        };

        assert_eq!(run(0, 5, 2, Some((1, 0)), false), vec![]);
        assert_eq!(
            run(5, 5, 2, Some((1, 0)), false),
            vec![(
                10,
                "Interval (5) must be > 0 and TimeInto Interval (5) must be < Interval."
                    .to_string()
            )]
        );
        assert_eq!(run(-1, 5, 2, Some((1, 0)), false), vec![]);
        assert_eq!(run(-6, 5, 2, Some((1, 0)), true), vec![]);
        assert_eq!(
            run(0, 5, 2, Some((2, 0)), false),
            vec![(
                10,
                "Scan interval must be integrally divisible into interval.".to_string()
            )]
        );
        assert_eq!(run(1, 3602, 2, Some((1, 0)), false), vec![]);
        assert_eq!(
            run(1500, 3601500, 1, Some((1, 0)), false),
            vec![(
                10,
                "Illegal sub seconds in time into and/or interval when interval > 3600 secs."
                    .to_string()
            )]
        );
    }

    #[test]
    fn not_integral_and_wide_constants_match_python() {
        let cr6 = profiles::get("cr6").unwrap();
        let env6 = Env::new(cr6);
        env6.define("KB", ConstValue::Int(999999999999), None);
        let wide_stmt = Rc::new(Stmt::ConstDecl(ConstDecl {
            base: StmtBase::new(1, "test.cr6"),
            name: "kb".to_string(),
            type_: None,
            value: Some(num_i(1, 999999999999, "999999999999")),
            ..Default::default()
        }));
        let prog = Program::new("test.cr6", vec![wide_stmt]);
        let wide = wide_constants_with(&prog, cr6);
        assert!(wide.contains("KB"));

        assert!(not_integral_with(
            Some(&num_f(1, 1.5, "1.5")),
            &env6,
            cr6,
            &wide
        ));
        assert!(not_integral_with(
            Some(&num_f(1, 60.0, "60.0")),
            &env6,
            cr6,
            &wide
        ));
        assert!(!not_integral_with(
            Some(&num_i(1, 60, "60")),
            &env6,
            cr6,
            &wide
        ));
        assert!(not_integral_with(Some(&name_e(1, "KB")), &env6, cr6, &wide));
        assert!(_nonfinite_literal_with(Some(&name_e(1, "NAN")), &env6));
        assert!(_nonfinite_literal_with(
            Some(&Expr::Unary(Unary {
                node: node(1),
                op: "-".to_string(),
                operand: Some(name_e(1, "inf")),
            })),
            &env6
        ));
        assert!(_needs_integer_fold_with(Some(&Expr::Binary(Binary {
            node: node(1),
            op: "+".to_string(),
            left: Some(num_i(1, 1, "1")),
            right: Some(num_i(1, 2, "2")),
        }))));
        assert!(!_needs_integer_fold_with(Some(&Expr::Unary(Unary {
            node: node(1),
            op: "+".to_string(),
            operand: Some(num_i(1, 1, "1")),
        }))));
        assert!(!_plain(Some(&Expr::String(StringLit {
            node: node(1),
            value: "hi".to_string(),
        }))));
        assert_eq!(
            _calls_with(
                &Rc::new(Stmt::Call(Call {
                    base: StmtBase::new(1, "t"),
                    name: "Timer".to_string(),
                    args: vec![Some(num_i(1, 0, "0"))],
                    ..Default::default()
                })),
                _ALL_INTEGRAL_NAMES
            )
            .len(),
            1
        );
    }
}
