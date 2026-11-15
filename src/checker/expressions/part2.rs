//! Port of `checker/expressions.py` (lines 798-1570).

#![allow(dead_code, unused_variables, unused_imports)]

use std::collections::{BTreeSet, HashMap};
use std::rc::Rc;
use std::sync::LazyLock;

use crate::checker::context::{self, Context, DimCounts};
use crate::diagnostics::{DiagRef, Diagnostic, Diagnostics, ShowFile};
use crate::profiles::Profile;
use crate::syntax::{
    Assignment, Call, Case, ConstDecl, Declaration, Expr, ExprRef, Index, Instruction, Juxtaposed,
    Member, Number, NumberValue, OrderedMap, Program, ProgramRef, Stmt, StmtRef, SubDef, Unparsed,
    VarDecl, expr_id, stmt_id,
};
use crate::{pyre, pystr};

// TODO(integrator-2): import these constants from `part1` / `instr_rules` once ported.
const _WHOLE_EXPR_ECHO_LOGGERS: [&str; 2] = ["cr6", "cr1000x"];
const _CONST_FAMILY_KINDS: [&str; 3] = ["const_expr", "const_expr_only", "datatype_kw"];
const _CONST_ONLY_VARIABLE_REJECT_KINDS: [&str; 2] = ["const_expr", "const_expr_only"];
const _EXTRA_PREDEFINED_SYMBOLS: [&str; 2] = ["NAN", "INF"];
const _CONTROL_KEYWORDS: [&str; 10] = [
    "CASE", "DO", "FOR", "FUNCTION", "IF", "SCAN", "SELECT", "SUB", "SUBSCAN", "WHILE",
];
const _DNP_FLAG_TYPE: &str = "Flag paramter must be of type LONG";

fn is_whole_expr_echo_logger(profile: &Profile) -> bool {
    _WHOLE_EXPR_ECHO_LOGGERS.contains(&profile.name)
}

fn multi_report_order(keyword: &str) -> Option<&'static [usize]> {
    match keyword {
        "MOVE" => Some(&[1, 0, 3, 2]),
        "RMSSPA" | "STDDEVSPA" | "MAXSPA" | "MINSPA" | "AVGSPA" => Some(&[1, 2, 0]),
        "CPIFILESEND" => Some(&[0, 1, 2, 3, 4]),
        "GOESSETUP" => Some(&[0, 1, 2, 3, 4, 5, 6, 9, 7, 8]),
        "GOESGPS"
        | "CH400SENDOS"
        | "HYDRAPROBE"
        | "SATVP"
        | "SDMCD16MASK"
        | "SDMIO16"
        | "TCPACTIVECONNECTIONS"
        | "SDMAO4A" => Some(&[0, 1]),
        "MOVEBYTES" => Some(&[0, 1, 3, 4, 2]),
        "READIO" | "SPLITSTR" | "PORTBRIDGE" => Some(&[1, 0]),
        "IPROUTE" | "DEWPOINT" | "SORTSPA" | "SORTSPAINDEXED" | "VAPORPRESSURE" => {
            if keyword == "SORTSPA" || keyword == "SORTSPAINDEXED" {
                Some(&[0, 2])
            } else {
                Some(&[0, 1, 2])
            }
        }
        "MQTTPUBLISHMETA" => Some(&[0, 1, 2, 3, 4, 5]),
        "ARGOSDATAREPEAT" | "EC100CONFIGURE" => Some(&[0, 3]),
        "AVGRUN" | "STDDEVRUN" => Some(&[0, 2, 4, 5]),
        "AVW200" | "GPS" | "SDMGENERIC" => Some(&[4, 0]),
        "PRT" => Some(&[0, 2]),
        "STRAINCALC" => Some(&[0, 2, 3]),
        "CDM_TCSE" => Some(&[7, 2]),
        "DEBUG" => Some(&[2, 3, 4]),
        "SENDVARIABLES" => Some(&[0, 1, 8]),
        "GETVARIABLES" => Some(&[0, 8]),
        "MATRIX" => Some(&[1, 2, 3]),
        "PEAKVALLEY" => Some(&[0, 1, 3]),
        "TIMEDCONTROL" => Some(&[4, 5]),
        "WETDRYBULB" => Some(&[0, 1, 2, 3]),
        "SERIALINRECORD" => Some(&[1, 5]),
        "TOTALRUN" => Some(&[0, 2, 4]),
        "MODBUSCLIENT" | "TCDIFF" => {
            if keyword == "MODBUSCLIENT" {
                Some(&[0, 5])
            } else {
                Some(&[5, 0])
            }
        }
        "FFTSPA" => Some(&[2, 0]),
        _ => None,
    }
}

/// Port of `checker/expressions.py::_names_through_over_index`.
pub fn _names_through_over_index(expr: Option<&Expr>, known: &BTreeSet<String>) -> Vec<String> {
    _names_through_over_index_with(expr, known, &DimCounts::default())
}

/// Full implementation of `checker/expressions.py::_names_through_over_index`.
pub fn _names_through_over_index_with(
    expr: Option<&Expr>,
    known: &BTreeSet<String>,
    dim_counts: &DimCounts,
) -> Vec<String> {
    fn walk(
        e: Option<&Expr>,
        known: &BTreeSet<String>,
        dim_counts: &DimCounts,
        out: &mut Vec<String>,
    ) -> bool {
        let Some(e) = e else {
            return false;
        };
        match e {
            Expr::Unary(u) => walk(u.operand.as_deref(), known, dim_counts, out),
            Expr::Binary(b) => {
                walk(b.left.as_deref(), known, dim_counts, out)
                    || walk(b.right.as_deref(), known, dim_counts, out)
            }
            Expr::Index(ix) => {
                let upper = pystr::upper(&ix.name);
                if !known.contains(&upper) {
                    out.push(ix.name.clone());
                }
                for a in &ix.args {
                    if walk(a.as_deref(), known, dim_counts, out) {
                        return true;
                    }
                }
                let count = dim_counts.counts.get(&upper).copied();
                let nargs = i64::try_from(ix.args.len()).unwrap_or(i64::MAX);
                count.is_some_and(|c| {
                    nargs > c && !(nargs == 3 && dim_counts.strings.contains(&upper))
                })
            }
            _ => {
                out.extend(super::part1::_undefined_names_walk(Some(e), known));
                false
            }
        }
    }

    let mut out = Vec::new();
    walk(expr, known, dim_counts, &mut out);
    out
}

/// Port of `checker/expressions.py::_dim_limit`.
pub fn _dim_limit() -> Option<i64> {
    None
}

/// Full implementation of `checker/expressions.py::_dim_limit`.
pub fn _dim_limit_with(profile: &Profile) -> Option<i64> {
    if is_whole_expr_echo_logger(profile) {
        Some(3)
    } else {
        None
    }
}

/// Port of `checker/expressions.py::_over_indexed`.
pub fn _over_indexed(
    expr: Option<&Expr>,
    dim_counts: &OrderedMap<String, i64>,
    limit: Option<i64>,
) -> bool {
    let empty_strings = BTreeSet::new();
    _over_indexed_inner(expr, dim_counts, &empty_strings, limit)
}

/// Full implementation of `checker/expressions.py::_over_indexed`.
pub fn _over_indexed_with(expr: Option<&Expr>, dim_counts: &DimCounts, limit: Option<i64>) -> bool {
    _over_indexed_inner(expr, &dim_counts.counts, &dim_counts.strings, limit)
}

fn _over_indexed_inner(
    expr: Option<&Expr>,
    counts: &OrderedMap<String, i64>,
    strings: &BTreeSet<String>,
    limit: Option<i64>,
) -> bool {
    let Some(expr) = expr else {
        return false;
    };
    match expr {
        Expr::Index(ix) => {
            let upper = pystr::upper(&ix.name);
            let count = counts.get(&upper).copied();
            let nargs = i64::try_from(ix.args.len()).unwrap_or(i64::MAX);
            if let Some(c) = count
                && nargs > limit.unwrap_or(c)
                && !(nargs == 3 && strings.contains(&upper))
            {
                return true;
            }
            ix.args
                .iter()
                .any(|a| _over_indexed_inner(a.as_deref(), counts, strings, limit))
        }
        Expr::Unary(u) => _over_indexed_inner(u.operand.as_deref(), counts, strings, limit),
        Expr::Binary(b) => {
            _over_indexed_inner(b.left.as_deref(), counts, strings, limit)
                || _over_indexed_inner(b.right.as_deref(), counts, strings, limit)
        }
        Expr::Member(m) => m
            .args
            .as_deref()
            .unwrap_or(&[])
            .iter()
            .any(|a| _over_indexed_inner(a.as_deref(), counts, strings, limit)),
        _ => false,
    }
}

/// Port of `checker/expressions.py::DimsState`.
#[derive(Debug, Clone, Default)]
pub struct DimsState {
    pub var_dims: OrderedMap<String, Option<i64>>,
    pub before: HashMap<usize, Option<i64>>,
    pub dim_counts: Option<DimCounts>,
}

impl DimsState {
    /// Port of `checker/expressions.py::DimsState::__init__`.
    pub fn __init__(&self) {}

    /// Full constructor for `DimsState`.
    pub fn new(
        var_dims: OrderedMap<String, Option<i64>>,
        before: Option<HashMap<usize, Option<i64>>>,
        dim_counts: Option<DimCounts>,
    ) -> Self {
        Self {
            var_dims,
            before: before.unwrap_or_default(),
            dim_counts,
        }
    }
}

/// Port of `checker/expressions.py::_stmt_exprs`.
pub fn _stmt_exprs(stmt: &Stmt) {
    let _ = _stmt_exprs_with(stmt);
}

/// Full implementation of `checker/expressions.py::_stmt_exprs`.
pub fn _stmt_exprs_with(stmt: &Stmt) -> Vec<Option<ExprRef>> {
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

/// Port of `checker/expressions.py::_build_dims_state`.
pub fn _build_dims_state(ctx: &crate::checker::context::Context) -> DimsState {
    let profile = ctx.profile.expect("_build_dims_state requires profile");
    let known = if profile.single_pass {
        Some(ctx.known_names())
    } else {
        None
    };
    let mut var_dims: OrderedMap<String, Option<i64>> = OrderedMap::new();
    let declare = |var_dims: &mut OrderedMap<String, Option<i64>>, name: &str, count: i64| {
        let upper = pystr::upper(name);
        let next = match var_dims.get(&upper) {
            Some(&prev) if prev != Some(count) => None,
            _ => Some(count),
        };
        var_dims.insert(upper, next);
    };

    for stmt in &ctx.program.statements {
        match &**stmt {
            Stmt::Declaration(d) => {
                for v in d.vars.borrow().iter() {
                    let count = i64::try_from(v.dims.len()).unwrap_or(i64::MAX)
                        + i64::from(pystr::upper(v.type_.as_deref().unwrap_or("")) == "STRING");
                    declare(&mut var_dims, &v.name, count);
                }
            }
            Stmt::SubDef(sd) => {
                for prm in &sd.params {
                    let count = prm.dims
                        + i64::from(pystr::upper(prm.type_.as_deref().unwrap_or("")) == "STRING");
                    declare(&mut var_dims, &prm.name, count);
                }
            }
            _ => {}
        }
    }

    let mut ordered: Vec<StmtRef> = Vec::new();
    let present: BTreeSet<usize> = ctx.program.statements.iter().map(stmt_id).collect();
    for stmt in &ctx.program.statements {
        ordered.push(stmt.clone());
        if let Stmt::If(i) = &**stmt {
            for st in i
                .inline
                .iter()
                .flatten()
                .chain(i.inline_else.iter().flatten())
            {
                if !present.contains(&stmt_id(st)) {
                    ordered.push(st.clone());
                }
            }
        }
    }

    let mut before: HashMap<usize, Option<i64>> = HashMap::new();
    let mut state: [Option<i64>; 1] = [Some(0)];
    for stmt in &ordered {
        before.insert(stmt_id(stmt), state[0]);
        for expr in _stmt_exprs_with(stmt) {
            if _dims_walk_with(expr.as_ref(), known, &var_dims, &mut state) {
                break;
            }
        }
    }

    DimsState::new(var_dims, Some(before), Some(ctx.dim_counts(true).clone()))
}

/// Port of `checker/expressions.py::_dims_walk`.
pub fn _dims_walk(_expr: Option<&Expr>) -> bool {
    false
}

/// Full implementation of `checker/expressions.py::_dims_walk`.
pub fn _dims_walk_with(
    expr: Option<&ExprRef>,
    known: Option<&KnownNames>,
    var_dims: &OrderedMap<String, Option<i64>>,
    state: &mut [Option<i64>],
) -> bool {
    let Some(expr) = expr else {
        return false;
    };
    let oos = known.is_some_and(|k| k.out_of_scope.contains(&expr_id(expr)));
    match &**expr {
        Expr::Name(n) => {
            let upper = pystr::upper(&n.name);
            if let Some(&d) = var_dims.get(&upper)
                && !oos
            {
                state[0] = d;
            }
            false
        }
        Expr::Index(ix) => {
            let upper = pystr::upper(&ix.name);
            if let Some(&d) = var_dims.get(&upper)
                && !oos
            {
                state[0] = d;
            } else if let Some(k) = known
                && (!k.names.contains(&upper) || oos)
            {
                let count = i64::try_from(ix.args.iter().filter(|a| a.is_some()).count())
                    .unwrap_or(i64::MAX);
                if state[0].is_some_and(|s| count > s) {
                    return true;
                }
            }
            ix.args
                .iter()
                .any(|a| _dims_walk_with(a.as_ref(), known, var_dims, state))
        }
        Expr::Unary(u) => _dims_walk_with(u.operand.as_ref(), known, var_dims, state),
        Expr::Binary(b) => {
            _dims_walk_with(b.left.as_ref(), known, var_dims, state)
                || _dims_walk_with(b.right.as_ref(), known, var_dims, state)
        }
        Expr::Member(m) => {
            _dims_walk_with(m.base.as_ref(), known, var_dims, state)
                || m.args
                    .as_deref()
                    .unwrap_or(&[])
                    .iter()
                    .any(|a| _dims_walk_with(a.as_ref(), known, var_dims, state))
        }
        Expr::ArrayLiteral(a) => a
            .items
            .iter()
            .any(|item| _dims_walk_with(item.as_ref(), known, var_dims, state)),
        Expr::Juxtaposed(j) => j
            .items
            .iter()
            .any(|item| _dims_walk_with(item.as_ref(), known, var_dims, state)),
        Expr::Number(_) | Expr::String(_) => false,
    }
}

/// Port of `checker/expressions.py::_has_undefined_index`.
pub fn _has_undefined_index(
    _expr: Option<&Expr>,
    _known: &BTreeSet<String>,
    _ds: Option<&mut DimsState>,
    _stmt: Option<&Stmt>,
) -> bool {
    false
}

/// Full implementation of `checker/expressions.py::_has_undefined_index`.
pub fn _has_undefined_index_with(
    expr: Option<&ExprRef>,
    known: &KnownNames,
    ds: Option<&DimsState>,
    stmt_id_opt: Option<usize>,
    prior: &[Option<ExprRef>],
) -> bool {
    let initial = ds
        .and_then(|d| stmt_id_opt.and_then(|sid| d.before.get(&sid).copied()))
        .unwrap_or(Some(0));
    let mut state = [initial];
    let empty_var_dims = OrderedMap::new();
    let var_dims = ds.map_or(&empty_var_dims, |d| &d.var_dims);
    prior
        .iter()
        .any(|e| _dims_walk_with(e.as_ref(), Some(known), var_dims, &mut state))
        || _dims_walk_with(expr, Some(known), var_dims, &mut state)
}

/// Port of `checker/expressions.py::_names_and_dims`.
pub fn _names_and_dims(
    _known: &BTreeSet<String>,
    _ds: Option<&mut DimsState>,
    _stmt: Option<&Stmt>,
) {
}

/// Full implementation of `checker/expressions.py::_names_and_dims`.
pub fn _names_and_dims_with(
    known: &KnownNames,
    ds: Option<&DimsState>,
    stmt_id_opt: Option<usize>,
    prior: &[Option<ExprRef>],
    exprs: &[Option<ExprRef>],
) -> (Vec<String>, bool) {
    fn walk(
        e: Option<&ExprRef>,
        known: &KnownNames,
        var_dims: &OrderedMap<String, Option<i64>>,
        state: &mut [Option<i64>],
        out: &mut Vec<String>,
    ) -> bool {
        let Some(e) = e else {
            return false;
        };
        let oos = known.out_of_scope.contains(&expr_id(e));
        match &**e {
            Expr::Name(n) => {
                let upper = pystr::upper(&n.name);
                if let Some(&d) = var_dims.get(&upper)
                    && !oos
                {
                    state[0] = d;
                }
                if !known.names.contains(&upper) || oos || known.tables.contains(&upper) {
                    out.push(n.name.clone());
                }
                false
            }
            Expr::Index(ix) => {
                let upper = pystr::upper(&ix.name);
                if let Some(&d) = var_dims.get(&upper)
                    && !oos
                {
                    state[0] = d;
                }
                if !known.names.contains(&upper) || oos {
                    out.push(ix.name.clone());
                    let count = i64::try_from(ix.args.iter().filter(|a| a.is_some()).count())
                        .unwrap_or(i64::MAX);
                    if state[0].is_some_and(|s| count > s) {
                        for a in &ix.args {
                            walk(a.as_ref(), known, var_dims, state, out);
                        }
                        return true;
                    }
                }
                for a in &ix.args {
                    if walk(a.as_ref(), known, var_dims, state, out) {
                        return true;
                    }
                }
                false
            }
            Expr::Unary(u) => walk(u.operand.as_ref(), known, var_dims, state, out),
            Expr::Binary(b) => {
                walk(b.left.as_ref(), known, var_dims, state, out)
                    || walk(b.right.as_ref(), known, var_dims, state, out)
            }
            Expr::Member(m) => {
                let skip_base = matches!(
                    m.base.as_deref(),
                    Some(Expr::Name(bn)) if known.names.contains(&pystr::upper(&bn.name))
                );
                if !skip_base && walk(m.base.as_ref(), known, var_dims, state, out) {
                    return true;
                }
                for a in m.args.as_deref().unwrap_or(&[]) {
                    if walk(a.as_ref(), known, var_dims, state, out) {
                        return true;
                    }
                }
                false
            }
            Expr::ArrayLiteral(a) => {
                for item in &a.items {
                    if walk(item.as_ref(), known, var_dims, state, out) {
                        return true;
                    }
                }
                false
            }
            Expr::Juxtaposed(j) => {
                for item in &j.items {
                    if walk(item.as_ref(), known, var_dims, state, out) {
                        return true;
                    }
                }
                false
            }
            Expr::Number(_) | Expr::String(_) => false,
        }
    }

    let mut combined = Vec::with_capacity(prior.len() + exprs.len());
    combined.extend(prior.iter().cloned());
    combined.extend(exprs.iter().cloned());
    let dims = _has_undefined_index_with(None, known, ds, stmt_id_opt, &combined);
    if !dims {
        let out = exprs
            .iter()
            .flat_map(|e| super::part1::_undefined_names(e.as_deref(), &known.names))
            .collect();
        return (out, false);
    }

    let initial = ds
        .and_then(|d| stmt_id_opt.and_then(|sid| d.before.get(&sid).copied()))
        .unwrap_or(Some(0));
    let mut state = [initial];
    let empty_var_dims = OrderedMap::new();
    let var_dims = ds.map_or(&empty_var_dims, |d| &d.var_dims);

    for e in prior {
        _dims_walk_with(e.as_ref(), Some(known), var_dims, &mut state);
    }

    let mut out = Vec::new();
    for e in exprs {
        if walk(e.as_ref(), known, var_dims, &mut state, &mut out) {
            break;
        }
    }
    (out, true)
}

/// Port of `checker/expressions.py::dimensions_message`.
pub fn dimensions_message(text: &str, _whole_statement: bool, _at_eol: bool) -> String {
    format!("Dimensions not declared: {text}.")
}

/// Full implementation of `checker/expressions.py::dimensions_message`.
pub fn dimensions_message_with(
    text: &str,
    profile: &Profile,
    whole_statement: bool,
    at_eol: bool,
) -> String {
    let sep = if whole_statement && profile.fallback_echo_newline && at_eol {
        "\n"
    } else {
        ""
    };
    format!("Dimensions not declared: {text}{sep}.")
}

/// Port of `checker/expressions.py::_arg_text`.
pub fn _arg_text(stmt: &Instruction, i: i64) -> String {
    static SELECT_CASE: LazyLock<pyre::Regex> =
        LazyLock::new(|| pyre::Regex::new(r"(?i)\s*Select\s+Case(.*)$", 0));

    let text = &stmt.base.text;
    if i == 0
        && stmt.keyword.eq_ignore_ascii_case("SELECT")
        && let Some(tail) = SELECT_CASE.match_(text).and_then(|m| m.group(1))
    {
        return tail.to_owned();
    }
    let index = |len: usize| {
        if i < 0 {
            len.checked_sub(usize::try_from(i.unsigned_abs()).ok()?)
        } else {
            usize::try_from(i).ok()
        }
    };
    if let Some((start, end)) = index(stmt.arg_spans.len())
        .and_then(|i| stmt.arg_spans.get(i))
        .and_then(|span| *span)
    {
        return pystr::slice(text, Some(start), Some(end)).to_owned();
    }
    match index(stmt.args.len())
        .and_then(|i| stmt.args.get(i))
        .and_then(Option::as_deref)
    {
        Some(Expr::Name(name)) => name.name.clone(),
        _ => String::new(),
    }
}

/// Port of `checker/expressions.py::_call_arg_text`.
pub fn _call_arg_text(text: &str, index: i64) -> String {
    let mut depth: i64 = 0;
    let mut quote = false;
    let mut start: Option<i64> = None;
    let mut n: i64 = 0;
    for (pos_u, ch) in text.chars().enumerate() {
        let pos = i64::try_from(pos_u).unwrap_or(i64::MAX);
        if ch == '"' {
            quote = !quote;
        } else if quote {
        } else if ch == '(' {
            depth += 1;
            if depth == 1 && start.is_none() {
                start = Some(pos + 1);
            }
        } else if ch == ')' || ch == ',' {
            if ch == ')' {
                depth -= 1;
            }
            if (ch == ',' && depth == 1) || depth == 0 {
                if n == index {
                    return pystr::lstrip(pystr::slice(text, start, Some(pos))).to_owned();
                }
                n += 1;
                start = Some(pos + 1);
            }
        }
    }
    String::new()
}

/// Port of `checker/expressions.py::_if_condition_text`.
pub fn _if_condition_text(stmt: &Stmt) -> Option<String> {
    static IF_COND_RE: LazyLock<pyre::Regex> =
        LazyLock::new(|| pyre::Regex::new(r"(?is)^\s*(?:Else)?If(?:\s|(?=\())(.*?)\bThen\b", 0));
    static IF_FALLBACK_RE: LazyLock<pyre::Regex> =
        LazyLock::new(|| pyre::Regex::new(r"(?is)^\s*If(?:\s|(?=\())(.*)$", 0));

    let text = stmt.text();
    let mut m = IF_COND_RE.match_(text);
    if m.is_none() && matches!(stmt, Stmt::If(i) if i.inline.is_none()) {
        m = IF_FALLBACK_RE.match_(text);
    }
    m.and_then(|mat| mat.group(1)).map(str::to_owned)
}

/// Port of `checker/expressions.py::_if_condition_text_no_then`.
pub fn _if_condition_text_no_then(stmt: &Stmt) -> Option<String> {
    static THEN_RE: LazyLock<pyre::Regex> = LazyLock::new(|| pyre::Regex::new(r"(?i)\bThen\b", 0));
    static IF_NO_THEN_RE: LazyLock<pyre::Regex> =
        LazyLock::new(|| pyre::Regex::new(r"(?is)^\s*If(?:\s|(?=\())([^'_]*)$", 0));

    let text = stmt.text();
    if THEN_RE.search(text).is_some() {
        return None;
    }
    IF_NO_THEN_RE
        .match_(text)
        .and_then(|m| m.group(1))
        .map(str::to_owned)
}

/// Port of `checker/expressions.py::_build_known_names`.
pub fn _build_known_names(ctx: &crate::checker::context::Context) -> KnownNames {
    let profile = ctx.profile.expect("_build_known_names requires profile");
    let program = &ctx.program;

    let mut resolvable: BTreeSet<String> = BTreeSet::new();
    for (k, _) in profile.symbols().iter() {
        resolvable.insert(pystr::upper(k));
    }
    for (k, _) in profile.functions().iter() {
        resolvable.insert(pystr::upper(k));
    }
    for (k, _) in profile.instructions().iter() {
        let up = pystr::upper(k);
        if _CONTROL_KEYWORDS.contains(&up.as_str()) {
            resolvable.insert(up);
        }
    }
    resolvable.extend(ctx.system_table_names().iter().cloned());
    resolvable.extend(ctx.ess_variable_names().iter().cloned());
    for sym in _EXTRA_PREDEFINED_SYMBOLS {
        resolvable.insert((*sym).to_owned());
    }

    let mut known: BTreeSet<String> = ctx.declared_names().iter().cloned().collect();
    known.extend(resolvable.iter().cloned());

    if program.statements.iter().any(
        |st| matches!(&**st, Stmt::Instruction(i) if pystr::upper(&i.keyword) == "CH400STATUS"),
    ) {
        known.insert("CH400STATUS".to_owned());
    }

    for name in _failed_consts(ctx, &known) {
        known.remove(&name);
    }

    if profile.single_pass {
        for name in _unclosed_structure_variables(program) {
            known.remove(&name);
        }
        let declared = ctx.declared_variable_names();
        let mut failed: BTreeSet<String> = BTreeSet::new();
        let mut valid: BTreeSet<String> = declared.clone();

        for st in &program.statements {
            let Stmt::Instruction(inst) = &**st else {
                continue;
            };
            if pystr::upper(&inst.keyword) != "ALIAS" || inst.args.len() != 2 {
                continue;
            }
            let Some(Some(dst_expr)) = inst.args.get(1) else {
                continue;
            };
            let Expr::Name(dst) = &**dst_expr else {
                continue;
            };
            let Some(Some(src_expr)) = inst.args.first() else {
                continue;
            };
            let (src_name, src_args) = match &**src_expr {
                Expr::Name(n) => (pystr::upper(&n.name), None),
                Expr::Index(ix) => (pystr::upper(&ix.name), Some(&ix.args)),
                _ => continue,
            };
            let mut bad = !declared.contains(&src_name)
                || src_args.is_some_and(|args| {
                    args.iter()
                        .flatten()
                        .any(|a| !super::part1::_undefined_names(Some(&**a), &known).is_empty())
                });
            if !bad
                && declared.contains(&src_name)
                && let Some(args) = src_args
            {
                let open_at = inst
                    .base
                    .text
                    .chars()
                    .position(|c| c == '(')
                    .map(|p| i64::try_from(p).unwrap_or(i64::MAX));
                let items = open_at.and_then(|p| {
                    crate::checker::structure::part8::_group_items(&inst.base.text, p)
                });
                bad = args.is_empty()
                    || items.is_some_and(|its| {
                        its.iter().any(|item| {
                            let s = pystr::strip(item);
                            s.starts_with('(') && s.ends_with(')')
                        })
                    });
            }
            if bad {
                failed.insert(pystr::upper(&dst.name));
            } else {
                valid.insert(pystr::upper(&dst.name));
            }
        }

        let env = ctx.env();
        for (pos, st) in program.statements.iter().enumerate() {
            let Stmt::Instruction(inst) = st.as_ref() else {
                continue;
            };
            if pystr::upper(&inst.keyword) != "ALIAS"
                || !inst
                    .args
                    .iter()
                    .skip(1)
                    .flatten()
                    .any(|a| matches!(a.as_ref(), Expr::Index(_)))
            {
                continue;
            }
            let prefix = crate::syntax::Program::new("", program.statements[..=pos].to_vec());
            let prefix = Rc::new(prefix);
            let view = crate::checker::context::of(&prefix, Some(profile), Some(env.clone()))
                .scoped_view(st, false);
            let view_ctx = crate::checker::context::of(&view, Some(profile), Some(env.clone()));
            let mut scratch = Diagnostics::default();
            let mut registered = BTreeSet::new();
            crate::checker::structure::_check_alias_dims(
                inst,
                &mut scratch,
                env,
                view_ctx.declared_dim0_sizes(),
                view_ctx.declared_all_dims(),
                &mut registered,
                &mut OrderedMap::new(),
                None,
                Some(profile),
                Some(&crate::checker::structure::_alias_string_slots(&view, env)),
                false,
            );
            if scratch
                .items
                .iter()
                .any(|d| d.message == "Alias is illegally dimensioned.")
            {
                failed.extend(
                    inst.args
                        .iter()
                        .skip(1)
                        .flatten()
                        .filter_map(|a| match a.as_ref() {
                            Expr::Index(a) => Some(pystr::upper(&a.name)),
                            _ => None,
                        })
                        .filter(|n| !registered.contains(n)),
                );
            }
        }
        for name in &failed {
            if !valid.contains(name) && !program.consts.contains_key(name) {
                known.remove(name);
            }
        }
    }

    let declared_vars = ctx.declared_variable_names();
    let const_keys: BTreeSet<String> = program.consts.keys().map(|k| pystr::upper(k)).collect();
    for st in &program.statements {
        if let Stmt::SubDef(sd) = &**st
            && pystr::upper(&sd.keyword) == "SUB"
        {
            let name = pystr::upper(&sd.name);
            if !declared_vars.contains(&name)
                && !const_keys.contains(&name)
                && !profile.symbols().contains(&name)
            {
                known.remove(&name);
            }
        }
    }

    KnownNames::new(
        known,
        _value_less_tables_with(ctx),
        profile.single_pass,
        ctx.out_of_scope_nodes(&resolvable),
    )
}

/// Port of `checker/expressions.py::_unclosed_structure_variables`.
pub fn _unclosed_structure_variables(program: &Program) -> BTreeSet<String> {
    let mut open_type: Option<String> = None;
    let mut unclosed = BTreeSet::new();
    for st in &program.statements {
        if let Stmt::Instruction(i) = &**st {
            let kw = pystr::upper(&i.keyword);
            if kw == "STRUCTURETYPE"
                && let Some(Some(arg0)) = i.args.first()
                && let Expr::Name(n) = &**arg0
            {
                open_type = Some(pystr::upper(&n.name));
            } else if kw == "ENDSTRUCTURETYPE" {
                open_type = None;
            }
        }
    }
    if let Some(ref ot) = open_type {
        for st in &program.statements {
            if let Stmt::Declaration(d) = &**st
                && matches!(d.kind.as_str(), "PUBLIC" | "DIM" | "STATIC")
            {
                for v in d.vars.borrow().iter() {
                    if v.type_.as_deref().unwrap_or("") == ot.as_str() {
                        unclosed.insert(pystr::upper(&v.name));
                    }
                }
            }
        }
    }
    unclosed
}

/// Port of `checker/expressions.py::KnownNames`.
#[derive(Debug, Clone, Default)]
pub struct KnownNames {
    pub names: BTreeSet<String>,
    pub tables: BTreeSet<String>,
    pub dotted_tables: bool,
    pub out_of_scope: BTreeSet<usize>,
}

impl KnownNames {
    /// Port of `checker/expressions.py::KnownNames::__init__`.
    pub fn __init__(&self, _dotted_tables: bool) {}

    /// Full constructor for `KnownNames`.
    pub fn new(
        names: BTreeSet<String>,
        tables: BTreeSet<String>,
        dotted_tables: bool,
        out_of_scope: BTreeSet<usize>,
    ) -> Self {
        Self {
            names,
            tables,
            dotted_tables,
            out_of_scope,
        }
    }

    /// Port of `checker/expressions.py::KnownNames::without`.
    #[must_use]
    pub fn without(&self) -> KnownNames {
        self.clone()
    }

    /// Full implementation of `checker/expressions.py::KnownNames::without`.
    #[must_use]
    pub fn without_names(&self, names: &BTreeSet<String>) -> KnownNames {
        if names.is_empty() {
            return self.clone();
        }
        KnownNames {
            names: self.names.difference(names).cloned().collect(),
            tables: self.tables.clone(),
            dotted_tables: self.dotted_tables,
            out_of_scope: self.out_of_scope.clone(),
        }
    }
}

/// Port of `checker/expressions.py::_alias_names`.
pub fn _alias_names(program: &Program) -> BTreeSet<String> {
    context::of(&Rc::new(program.clone()), None, None)
        .alias_names()
        .clone()
}

/// Port of `checker/expressions.py::_value_less_tables`.
pub fn _value_less_tables(ctx: &crate::checker::context::Context) {
    let _ = _value_less_tables_with(ctx);
}

/// Full implementation of `checker/expressions.py::_value_less_tables`.
pub fn _value_less_tables_with(ctx: &crate::checker::context::Context) -> BTreeSet<String> {
    let program = &ctx.program;
    let profile = ctx.profile;
    let mut tables: BTreeSet<String> = BTreeSet::new();
    for st in &program.statements {
        if let Stmt::Instruction(i) = &**st
            && pystr::upper(&i.keyword) == "DATATABLE"
            && let Some(Some(arg0)) = i.args.first()
            && let Expr::Name(n) = &**arg0
        {
            tables.insert(pystr::upper(&n.name));
        }
    }
    if let Some(p) = profile {
        for (sym, _) in p.symbols().iter() {
            tables.remove(&pystr::upper(sym));
        }
    }
    tables.insert("STATUS".to_owned());
    tables.insert("PUBLIC".to_owned());
    if profile.is_some_and(|p| p.single_pass) {
        tables.insert("SETTINGS".to_owned());
    }

    let aliases = _alias_names(program);
    let mut others: BTreeSet<String> = ctx.declared_variable_names().clone();
    for k in program.consts.keys() {
        others.insert(pystr::upper(k));
    }
    for st in &program.statements {
        if let Stmt::SubDef(sd) = &**st {
            others.insert(pystr::upper(&sd.name));
        }
    }
    others.extend(aliases);

    tables.difference(&others).cloned().collect()
}

/// Port of `checker/expressions.py::_failed_consts`.
pub fn _failed_consts(
    ctx: &crate::checker::context::Context,
    known: &BTreeSet<String>,
) -> BTreeSet<String> {
    let program = &ctx.program;
    let profile = ctx.profile;
    let consts: Vec<(usize, &ConstDecl)> = program
        .statements
        .iter()
        .filter_map(|st| {
            if let Stmt::ConstDecl(c) = &**st
                && c.value.is_some()
            {
                Some((stmt_id(st), c))
            } else {
                None
            }
        })
        .collect();

    let mut failed: BTreeSet<String> = BTreeSet::new();
    if profile.is_some() {
        let empty = BTreeSet::new();
        for (_, c) in &consts {
            // TODO(integrator-2): pass `profile` to `_find_arity_problem` and
            // `_find_bare_function_ref` once `p4-expr-4` adds `*_with` / profile params.
            if super::part4::_find_arity_problem(c.value.as_deref(), None, &empty).is_some()
                || super::part4::_find_bare_function_ref(c.value.as_deref(), &empty)
            {
                failed.insert(pystr::upper(&c.name));
            }
        }
    }

    let empty_unready: HashMap<usize, BTreeSet<String>> = HashMap::new();
    let unready = if profile.is_some() {
        ctx.not_yet_declared()
    } else {
        &empty_unready
    };

    let mut changed = true;
    while changed {
        changed = false;
        for &(cid, c) in &consts {
            let name = pystr::upper(&c.name);
            if failed.contains(&name) {
                continue;
            }
            let unready_for_c = unready.get(&cid);
            let effective: BTreeSet<String> = known
                .iter()
                .filter(|n| !failed.contains(*n) && !unready_for_c.is_some_and(|u| u.contains(*n)))
                .cloned()
                .collect();
            if !super::part1::_undefined_names(c.value.as_deref(), &effective).is_empty() {
                failed.insert(name);
                changed = true;
            }
        }
    }

    failed
}

/// Port of `checker/expressions.py::_report_statement`.
pub fn _report_statement(
    _stmt: &Instruction,
    _positions: &OrderedMap<usize, String>,
    _known: &BTreeSet<String>,
    _variable_names: &BTreeSet<String>,
    _diags: &mut Diagnostics,
    _families: &str,
    _dim_counts: Option<OrderedMap<String, i64>>,
    _ds: Option<&mut DimsState>,
) -> bool {
    false
}

/// Full implementation of `checker/expressions.py::_report_statement`.
#[allow(clippy::too_many_arguments)]
pub fn _report_statement_with(
    stmt: &Instruction,
    stmt_id: usize,
    positions: &OrderedMap<usize, String>,
    known: &KnownNames,
    variable_names: &BTreeSet<String>,
    profile: &Profile,
    diags: &mut Diagnostics,
    families: &str,
    dim_counts: Option<&DimCounts>,
    ds: Option<&DimsState>,
) -> bool {
    let kw = pystr::upper(&stmt.keyword);
    let whole_echo = is_whole_expr_echo_logger(profile);

    let mut check_position = |i: usize| -> bool {
        if i >= stmt.args.len() || stmt.args[i].is_none() {
            return false;
        }
        let arg = stmt.args[i].as_ref();
        let arg_expr = arg.map(|e| &**e);
        // TODO(integrator-2): pass `known: &KnownNames` once `part1::_undefined_names` takes it.
        let mut names = super::part1::_undefined_names(arg_expr, &known.names);
        let mut dims = false;
        if !names.is_empty() && !whole_echo {
            let (n, d) =
                _names_and_dims_with(known, ds, Some(stmt_id), &stmt.args[..i], &[arg.cloned()]);
            names = n;
            dims = d;
        }

        let pos_kind = positions.get(&i).map_or("", String::as_str);
        let fnotch_match = profile
            .fnotch()
            .get("instructions")
            .and_then(|ins| ins.get(&kw))
            .and_then(crate::profiles::Json::as_i64)
            == Some(i64::try_from(i).unwrap_or(-1) + 1);
        let is_const_family = _CONST_ONLY_VARIABLE_REJECT_KINDS.contains(&pos_kind)
            || (!names.is_empty() && !whole_echo && pos_kind == "datatype_kw")
            || (profile.single_pass && kw == "PANELTEMP" && i == 1)
            || (whole_echo && pos_kind == "datatype_kw" && fnotch_match);

        let i_i64 = i64::try_from(i).unwrap_or(i64::MAX);
        if !names.is_empty() {
            if whole_echo {
                diags.error_basic(
                    stmt.base.node.line,
                    &profile
                        .unresolved_symbol_message
                        .format(&_arg_text(stmt, i_i64)),
                    &stmt.base.node.file,
                );
                if i == 5 && kw == "DNPVARIABLE" {
                    diags.error_basic(stmt.base.node.line, _DNP_FLAG_TYPE, &stmt.base.node.file);
                }
                if i == 1 && kw == "MENUITEM" && matches!(arg_expr, Some(Expr::Name(_))) {
                    diags.error_basic(
                        stmt.base.node.line,
                        "Cannot use DataTable value as a MenuItem, use DisplayValue.",
                        &stmt.base.node.file,
                    );
                }
                return true;
            }
            for name in &names {
                diags.error_basic(
                    stmt.base.node.line,
                    &profile.unresolved_symbol_message.format(name),
                    &stmt.base.node.file,
                );
            }
            if dims {
                let arg_t = _arg_text(stmt, i_i64);
                diags.error_basic(
                    stmt.base.node.line,
                    &dimensions_message_with(pystr::strip(&arg_t), profile, false, true),
                    &stmt.base.node.file,
                );
                if is_const_family {
                    diags.error(
                        stmt.base.node.line,
                        &format!("Variable illegal in Parameter: {arg_t}."),
                        &stmt.base.node.file,
                        ShowFile::Bool(true),
                        stmt_id,
                        "",
                        "",
                    );
                    return true;
                }
                if i + 1 < stmt.args.len() && stmt.args[i + 1].is_some() {
                    let token = crate::checker::instr_rules::part1::_next_simple_token(&_arg_text(
                        stmt,
                        i_i64 + 1,
                    ));
                    diags.error_basic(
                        stmt.base.node.line,
                        &format!("Too many parameters:  {token}."),
                        &stmt.base.node.file,
                    );
                }
                return true;
            }
            if !is_const_family {
                return true;
            }
        }

        if let Some(dc) = dim_counts
            && _over_indexed_with(arg_expr, dc, _dim_limit_with(profile))
        {
            let text = _arg_text(stmt, i_i64);
            let shown = if kw == "SELECT" {
                text.as_str()
            } else {
                pystr::strip(&text)
            };
            diags.error_basic(
                stmt.base.node.line,
                &dimensions_message_with(shown, profile, false, true),
                &stmt.base.node.file,
            );
            if !whole_echo && i + 1 < stmt.args.len() && stmt.args[i + 1].is_some() {
                let next_t = _arg_text(stmt, i_i64 + 1);
                let tok = crate::checker::instr_rules::part1::_next_simple_token(&next_t);
                let token = if tok.is_empty() { next_t } else { tok };
                diags.error_basic(
                    stmt.base.node.line,
                    &format!("Too many parameters:  {token}."),
                    &stmt.base.node.file,
                );
            }
            return true;
        }

        if is_const_family {
            let var_refs = super::part1::_variable_refs(arg_expr, variable_names);
            if !names.is_empty()
                || !var_refs.is_empty()
                || super::part1::_has_dotted_field(arg_expr)
            {
                diags.error(
                    stmt.base.node.line,
                    &format!("Variable illegal in Parameter: {}.", _arg_text(stmt, i_i64)),
                    &stmt.base.node.file,
                    ShowFile::Bool(true),
                    stmt_id,
                    "",
                    "",
                );
                return true;
            }
        }

        !names.is_empty()
    };

    if let Some(multi) = multi_report_order(&kw)
        && whole_echo
        && families == "general"
    {
        let mut reported = false;
        for &i in multi {
            if positions.contains_key(&i) && check_position(i) {
                reported = true;
            }
        }
        if reported {
            return true;
        }
        let mut sorted_pos: Vec<usize> = positions.keys().copied().collect();
        sorted_pos.sort_unstable();
        for i in sorted_pos {
            if !multi.contains(&i)
                && !_CONST_FAMILY_KINDS.contains(&positions.get(&i).map_or("", String::as_str))
                && check_position(i)
            {
                return true;
            }
        }
        return false;
    }

    let mut order: Vec<usize> = Vec::new();
    if whole_echo {
        if families == "all" || families == "const" {
            let mut c: Vec<usize> = positions
                .iter()
                .filter(|(_, k)| _CONST_FAMILY_KINDS.contains(&k.as_str()))
                .map(|(&i, _)| i)
                .collect();
            c.sort_unstable();
            order.extend(c);
        }
        if families == "all" || families == "general" {
            let mut g: Vec<usize> = positions
                .iter()
                .filter(|(_, k)| !_CONST_FAMILY_KINDS.contains(&k.as_str()))
                .map(|(&i, _)| i)
                .collect();
            g.sort_unstable();
            order.extend(g);
        }
    } else {
        order = positions.keys().copied().collect();
        order.sort_unstable();
    }

    for i in order {
        if check_position(i) {
            return true;
        }
    }
    false
}

/// Port of `checker/expressions.py::_member_index_text`.
pub fn _member_index_text() -> String {
    "(1)".to_owned()
}

/// Full implementation of `checker/expressions.py::_member_index_text`.
pub fn _member_index_text_with(args: Option<&[Option<ExprRef>]>) -> String {
    let Some(args) = args else {
        return "(1)".to_owned();
    };
    let Some(Some(index)) = args.first() else {
        return "(1)".to_owned();
    };
    match &**index {
        Expr::Number(n) => match &n.value {
            NumberValue::Int(v) if *v >= 0 => format!("({v})"),
            NumberValue::Float(f) if *f >= 0.0 => {
                pystr::py_int_from_f64(*f).map_or_else(String::new, |v| format!("({v})"))
            }
            NumberValue::BigInt(s) if !s.starts_with('-') => format!("({s})"),
            _ => String::new(),
        },
        _ => String::new(),
    }
}

/// Port of `checker/expressions.py::_table_member_problem`.
pub fn _table_member_problem(
    _expr: Option<&Expr>,
    _data_tables: &BTreeSet<String>,
    _absent: &BTreeSet<String>,
) {
}

/// Full implementation of `checker/expressions.py::_table_member_problem`.
pub fn _table_member_problem_with(
    expr: Option<&Expr>,
    const_tables: &OrderedMap<String, BTreeSet<String>>,
    data_tables: &TablesWithFields,
    absent: &BTreeSet<String>,
) -> Option<(String, String, String)> {
    let expr = expr?;
    match expr {
        Expr::Member(m) => {
            if let Some(base) = m.base.as_deref()
                && let Expr::Name(bn) = base
            {
                let table = pystr::upper(&bn.name);
                let member = pystr::upper(&m.member);
                if let Some(cfields) = const_tables.get(&table)
                    && !cfields.contains(&member)
                {
                    return Some((bn.name.clone(), m.member.clone(), "(1)".to_owned()));
                }
                if data_tables.tables.contains(&table) && member == "TABLEBYTESX" {
                    return Some((bn.name.clone(), m.member.clone(), "(1)".to_owned()));
                }
                if let Some(fields) = data_tables.fields.get(&table)
                    && !fields.contains(&member)
                    && !pystr::isdigit(pystr::slice(&member, Some(0), Some(1)))
                {
                    return Some((
                        bn.name.clone(),
                        m.member.clone(),
                        _member_index_text_with(m.args.as_deref()),
                    ));
                }
                if data_tables.tables.contains(&table)
                    && absent.contains(&format!("{table}.{member}"))
                {
                    return Some((
                        bn.name.clone(),
                        m.member.clone(),
                        _member_index_text_with(m.args.as_deref()),
                    ));
                }
            }
            for arg in m.args.as_deref().unwrap_or(&[]).iter().flatten() {
                if let Some(found) =
                    _table_member_problem_with(Some(arg), const_tables, data_tables, absent)
                {
                    return Some(found);
                }
            }
            _table_member_problem_with(m.base.as_deref(), const_tables, data_tables, absent)
        }
        Expr::Index(ix) => {
            for arg in ix.args.iter().flatten() {
                if let Some(found) =
                    _table_member_problem_with(Some(arg), const_tables, data_tables, absent)
                {
                    return Some(found);
                }
            }
            None
        }
        Expr::Unary(u) => {
            _table_member_problem_with(u.operand.as_deref(), const_tables, data_tables, absent)
        }
        Expr::Binary(b) => {
            _table_member_problem_with(b.left.as_deref(), const_tables, data_tables, absent)
                .or_else(|| {
                    _table_member_problem_with(
                        b.right.as_deref(),
                        const_tables,
                        data_tables,
                        absent,
                    )
                })
        }
        Expr::ArrayLiteral(a) => {
            for item in a.items.iter().flatten() {
                if let Some(found) =
                    _table_member_problem_with(Some(item), const_tables, data_tables, absent)
                {
                    return Some(found);
                }
            }
            None
        }
        Expr::Juxtaposed(j) => {
            for item in j.items.iter().flatten() {
                if let Some(found) =
                    _table_member_problem_with(Some(item), const_tables, data_tables, absent)
                {
                    return Some(found);
                }
            }
            None
        }
        Expr::Number(_) | Expr::String(_) | Expr::Name(_) => None,
    }
}

/// Port of `checker/expressions.py::_TablesWithFields`.
#[derive(Debug, Clone, Default)]
pub struct TablesWithFields {
    pub tables: BTreeSet<String>,
    pub fields: OrderedMap<String, BTreeSet<String>>,
}

/// Port of `checker/expressions.py::_data_table_fields`.
pub fn _data_table_fields(_program: &Program) {}

/// Full implementation of `checker/expressions.py::_data_table_fields`.
pub fn _data_table_fields_with(
    _program: &Program,
    _profile: &Profile,
) -> OrderedMap<String, BTreeSet<String>> {
    // TODO(integrator-2): wire `tables::build_tables` once `tables` part3 is ported.
    OrderedMap::new()
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeSet, HashMap};
    use std::rc::Rc;

    use super::{
        _alias_names, _arg_text, _call_arg_text, _dim_limit_with, _dims_walk_with,
        _has_undefined_index_with, _if_condition_text, _if_condition_text_no_then,
        _member_index_text_with, _names_and_dims_with, _over_indexed_with, _stmt_exprs_with,
        _table_member_problem_with, _unclosed_structure_variables, _value_less_tables_with,
        DimsState, KnownNames, TablesWithFields, dimensions_message_with,
    };
    use crate::checker::context::{self, DimCounts};
    use crate::profiles;
    use crate::syntax::{
        Assignment, Declaration, Expr, If, Index, Instruction, Member, Name, Node, Number,
        NumberValue, OrderedMap, Program, Stmt, StmtBase, VarDecl,
    };

    fn name_expr(n: &str) -> Rc<Expr> {
        Rc::new(Expr::Name(Name {
            node: Node::new(1, ""),
            name: n.to_owned(),
        }))
    }

    fn num_i(v: i64) -> Rc<Expr> {
        Rc::new(Expr::Number(Number {
            node: Node::new(1, ""),
            value: NumberValue::Int(v),
            text: v.to_string(),
        }))
    }

    fn num_f(v: f64) -> Rc<Expr> {
        Rc::new(Expr::Number(Number {
            node: Node::new(1, ""),
            value: NumberValue::Float(v),
            text: v.to_string(),
        }))
    }

    fn index_expr(n: &str, args: Vec<Option<Rc<Expr>>>) -> Rc<Expr> {
        Rc::new(Expr::Index(Index {
            node: Node::new(1, ""),
            name: n.to_owned(),
            args,
        }))
    }

    #[test]
    fn arg_text_uses_character_spans_and_preserves_select_spacing() {
        let mut stmt = Instruction {
            base: StmtBase {
                text: "Scan(é + 1,Sec)".to_owned(),
                ..StmtBase::default()
            },
            keyword: "SCAN".to_owned(),
            arg_spans: vec![Some((5, 10)), Some((11, 14))],
            ..Instruction::default()
        };
        assert_eq!(_arg_text(&stmt, 0), "é + 1");
        assert_eq!(_arg_text(&stmt, -1), "Sec");

        stmt.keyword = "SELECT".to_owned();
        stmt.base.text = " Select Case  X + Nope".to_owned();
        assert_eq!(_arg_text(&stmt, 0), "  X + Nope");
    }

    #[test]
    fn dim_limit_and_dimensions_message_match_python() {
        let cr6 = profiles::get("cr6").unwrap();
        let cr300 = profiles::get("cr300").unwrap();
        assert_eq!(_dim_limit_with(cr6), Some(3));
        assert_eq!(_dim_limit_with(cr300), None);
        assert_eq!(
            dimensions_message_with("X", cr6, true, true),
            "Dimensions not declared: X."
        );
        assert_eq!(
            dimensions_message_with("X", cr300, true, true),
            "Dimensions not declared: X\n."
        );
        assert_eq!(
            dimensions_message_with("X", cr300, true, false),
            "Dimensions not declared: X."
        );
    }

    #[test]
    fn over_indexed_handles_strings_and_limits() {
        let cr6 = profiles::get("cr6").unwrap();
        let cr300 = profiles::get("cr300").unwrap();
        let mut counts = OrderedMap::new();
        counts.insert("R".to_owned(), 1);
        counts.insert("S".to_owned(), 1);
        let mut strings = BTreeSet::new();
        strings.insert("S".to_owned());
        let dc = DimCounts { counts, strings };

        let r_2 = index_expr("R", vec![Some(num_i(2)), Some(num_i(1))]);
        let s_3 = index_expr("S", vec![Some(num_i(1)), Some(num_i(2)), Some(num_i(3))]);
        let s_4 = index_expr(
            "S",
            vec![
                Some(num_i(1)),
                Some(num_i(2)),
                Some(num_i(3)),
                Some(num_i(4)),
            ],
        );

        assert!(_over_indexed_with(Some(&*r_2), &dc, _dim_limit_with(cr300)));
        assert!(!_over_indexed_with(
            Some(&*s_3),
            &dc,
            _dim_limit_with(cr300)
        ));
        assert!(!_over_indexed_with(Some(&*s_3), &dc, _dim_limit_with(cr6)));
        assert!(_over_indexed_with(Some(&*s_4), &dc, _dim_limit_with(cr6)));
    }

    #[test]
    fn call_arg_text_and_if_condition_text_match_python() {
        let call = "Foo(a, \"b,c\", d(1,2))";
        assert_eq!(_call_arg_text(call, 0), "a");
        assert_eq!(_call_arg_text(call, 1), "\"b,c\"");
        assert_eq!(_call_arg_text(call, 2), "d(1,2)");
        assert_eq!(_call_arg_text(call, 3), "");

        let if_then = Stmt::If(If {
            base: StmtBase {
                text: "If X > 1 And Nope Then".to_owned(),
                ..StmtBase::default()
            },
            ..If::default()
        });
        assert_eq!(
            _if_condition_text(&if_then).as_deref(),
            Some("X > 1 And Nope ")
        );

        let if_paren = Stmt::If(If {
            base: StmtBase {
                text: "If(X > 1) Then Y = 1".to_owned(),
                ..StmtBase::default()
            },
            inline: Some(vec![]),
            ..If::default()
        });
        assert_eq!(_if_condition_text(&if_paren).as_deref(), Some("(X > 1) "));

        let if_no_then = Stmt::If(If {
            base: StmtBase {
                text: "If Nope = True".to_owned(),
                ..StmtBase::default()
            },
            inline: None,
            ..If::default()
        });
        assert_eq!(
            _if_condition_text(&if_no_then).as_deref(),
            Some("Nope = True")
        );
        assert_eq!(
            _if_condition_text_no_then(&if_no_then).as_deref(),
            Some("Nope = True")
        );

        let if_comment = Stmt::If(If {
            base: StmtBase {
                text: "If Nope = True ' c".to_owned(),
                ..StmtBase::default()
            },
            ..If::default()
        });
        assert_eq!(_if_condition_text_no_then(&if_comment), None);
    }

    #[test]
    fn member_index_text_and_table_member_problem_match_python() {
        assert_eq!(_member_index_text_with(None), "(1)");
        assert_eq!(_member_index_text_with(Some(&[])), "(1)");
        assert_eq!(_member_index_text_with(Some(&[None])), "(1)");
        assert_eq!(_member_index_text_with(Some(&[Some(num_f(2.9))])), "(2)");
        assert_eq!(_member_index_text_with(Some(&[Some(num_i(-1))])), "");
        assert_eq!(_member_index_text_with(Some(&[Some(name_expr("I"))])), "");

        let mut const_tables = OrderedMap::new();
        let mut ct_fields = BTreeSet::new();
        ct_fields.insert("GOOD".to_owned());
        const_tables.insert("CT".to_owned(), ct_fields);

        let mut dt_tables = BTreeSet::new();
        dt_tables.insert("TBL".to_owned());
        let mut dt_fields = OrderedMap::new();
        let mut tbl_fields = BTreeSet::new();
        tbl_fields.insert("RECORD".to_owned());
        dt_fields.insert("TBL".to_owned(), tbl_fields);
        let dtf = TablesWithFields {
            tables: dt_tables,
            fields: dt_fields,
        };

        let absent = BTreeSet::from(["TBL.GONE".to_owned()]);
        let m_bad = Expr::Member(Member {
            node: Node::new(1, ""),
            base: Some(name_expr("Tbl")),
            member: "NoSuchField".to_owned(),
            args: None,
        });
        assert_eq!(
            _table_member_problem_with(Some(&m_bad), &const_tables, &dtf, &absent),
            Some(("Tbl".to_owned(), "NoSuchField".to_owned(), "(1)".to_owned()))
        );

        let m_digit = Expr::Member(Member {
            node: Node::new(1, ""),
            base: Some(name_expr("Tbl")),
            member: "1".to_owned(),
            args: None,
        });
        assert_eq!(
            _table_member_problem_with(Some(&m_digit), &const_tables, &dtf, &absent),
            None
        );

        let m_ct = Expr::Member(Member {
            node: Node::new(1, ""),
            base: Some(name_expr("Ct")),
            member: "Bad".to_owned(),
            args: Some(vec![Some(num_i(2))]),
        });
        assert_eq!(
            _table_member_problem_with(Some(&m_ct), &const_tables, &dtf, &absent),
            Some(("Ct".to_owned(), "Bad".to_owned(), "(1)".to_owned()))
        );
    }

    #[test]
    fn unclosed_structure_variables_and_value_less_tables() {
        let cr6 = profiles::get("cr6").unwrap();
        let cr300 = profiles::get("cr300").unwrap();

        let st_open = Rc::new(Stmt::Instruction(Instruction {
            keyword: "STRUCTURETYPE".to_owned(),
            args: vec![Some(name_expr("MyT"))],
            ..Instruction::default()
        }));
        let decl = Rc::new(Stmt::Declaration(Declaration {
            kind: "PUBLIC".to_owned(),
            vars: vec![Rc::new(VarDecl {
                name: "v".to_owned(),
                type_: Some("MYT".to_owned()),
                ..VarDecl::default()
            })]
            .into(),
            ..Declaration::default()
        }));
        let prog = Program::new("", vec![st_open, decl]);
        assert_eq!(
            _unclosed_structure_variables(&prog),
            BTreeSet::from(["V".to_owned()])
        );

        let dt = Rc::new(Stmt::Instruction(Instruction {
            keyword: "DATATABLE".to_owned(),
            args: vec![Some(name_expr("T"))],
            ..Instruction::default()
        }));
        let prog_dt = Rc::new(Program::new("", vec![dt]));
        let ctx6 = context::of(&prog_dt, Some(cr6), None);
        let ctx300 = context::of(&prog_dt, Some(cr300), None);
        assert_eq!(
            _value_less_tables_with(&ctx6),
            BTreeSet::from(["PUBLIC".to_owned(), "STATUS".to_owned(), "T".to_owned()])
        );
        assert_eq!(
            _value_less_tables_with(&ctx300),
            BTreeSet::from([
                "PUBLIC".to_owned(),
                "SETTINGS".to_owned(),
                "STATUS".to_owned(),
                "T".to_owned(),
            ])
        );
    }

    #[test]
    fn dims_walk_and_names_and_dims_stop_at_triggering_subscript() {
        let known = KnownNames::new(
            BTreeSet::from(["BATTERY".to_owned()]),
            BTreeSet::new(),
            true,
            BTreeSet::new(),
        );
        let var_dims = OrderedMap::new();
        let mut before = HashMap::new();
        before.insert(42, Some(0));
        let ds = DimsState::new(var_dims, Some(before), None);

        let nope1 = index_expr("Nope", vec![Some(num_i(1))]);
        let nope2 = index_expr("Nope2", vec![Some(num_i(1))]);
        let sum = Rc::new(Expr::Binary(crate::syntax::Binary {
            node: Node::new(1, ""),
            op: "+".to_owned(),
            left: Some(nope1),
            right: Some(nope2),
        }));
        let call = index_expr("Battery", vec![Some(sum)]);

        assert!(_has_undefined_index_with(
            Some(&call),
            &known,
            Some(&ds),
            Some(42),
            &[],
        ));
        let (names, dims) = _names_and_dims_with(&known, Some(&ds), Some(42), &[], &[Some(call)]);
        assert!(dims);
        assert_eq!(names, vec!["Nope".to_owned()]);

        let mut state = [Some(0)];
        let mut vd = OrderedMap::new();
        vd.insert("ARR".to_owned(), Some(2));
        let arr_ref = name_expr("Arr");
        assert!(!_dims_walk_with(Some(&arr_ref), None, &vd, &mut state));
        assert_eq!(state[0], Some(2));

        let assign = Stmt::Assignment(Assignment {
            target: Some(name_expr("A")),
            value: Some(num_i(1)),
            ..Assignment::default()
        });
        assert_eq!(_stmt_exprs_with(&assign).len(), 2);

        let kn2 = known.without_names(&BTreeSet::from(["BATTERY".to_owned()]));
        assert!(kn2.names.is_empty());

        let alias_stmt = Rc::new(Stmt::Instruction(Instruction {
            keyword: "ALIAS".to_owned(),
            args: vec![Some(name_expr("A")), Some(name_expr("Ux"))],
            ..Instruction::default()
        }));
        let alias_prog = Program::new("", vec![alias_stmt]);
        assert_eq!(_alias_names(&alias_prog), BTreeSet::from(["UX".to_owned()]));
    }
}
