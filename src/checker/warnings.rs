//! Port of `checker/warnings.py`.

#![allow(clippy::used_underscore_items)]

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;
use std::sync::LazyLock;

use crate::consteval::{self, Env};
use crate::diagnostics::{Diagnostic, Diagnostics};
use crate::profiles::{self, Json, Profile};
use crate::pyre;
use crate::pystr;
use crate::syntax::{
    ConstValue, Expr, ExprRef, Instruction, OrderedMap, Program, Stmt, StmtRef, ends_routine,
    stmt_id,
};

use super::{context, instr_data, instr_measure, instr_rules, scan_time};

const NON_EXECUTABLE_KEYWORDS: &[&str] = &[
    "BEGINPROG",
    "ENDPROG",
    "DATATABLE",
    "ENDTABLE",
    "ALIAS",
    "UNITS",
    "CONSTTABLE",
    "ENDCONSTTABLE",
    "STRUCTURE",
    "ENDSTRUCTURE",
    "STRUCTURETYPE",
    "ENDSTRUCTURETYPE",
    "OBJECTTYPE",
    "ENDOBJECTTYPE",
    "DATA",
    "DATALONG",
    "SEQUENTIALMODE",
    "PIPELINEMODE",
];

// Local mirror of `structure.py::SCAN_ONLY` keys until `structure.rs` exports `SCAN_ONLY`.
const SCAN_ONLY_KEYWORDS: &[&str] = &["PULSECOUNT", "QUADRATURE", "TIMERINPUT"];

static DATA_WORD_RE: LazyLock<pyre::Regex> =
    LazyLock::new(|| pyre::Regex::new(r"\s*(DataLong|Data)\s", pyre::I));

static NEXT_SCAN_RE: LazyLock<pyre::Regex> =
    LazyLock::new(|| pyre::Regex::new(r"(?i)\s*Next\s+Scan\b", 0));

static TRUNCATE_CALL_RE: LazyLock<pyre::Regex> =
    LazyLock::new(|| pyre::Regex::new(r"(?i)\b(IfTime|TimeIntoInterval)\s*\(", 0));

static STRING_LIT_RE: LazyLock<pyre::Regex> = LazyLock::new(|| pyre::Regex::new(r#""[^"]*""#, 0));

static IDENT_RE: LazyLock<pyre::Regex> = LazyLock::new(|| pyre::Regex::new(r"[A-Za-z_]\w*", 0));

const ARITY_ERROR_PREFIXES: &[&str] = &[
    "Expression expects , :",
    "No Matching, or out of place )",
    "Invalid, or out of place expression:",
];

fn env_for(program: &Program, profile: &Profile) -> Env {
    let static_profile =
        profiles::get(profile.name).unwrap_or_else(|_| panic!("unknown profile: {}", profile.name));
    Env::with_consts(
        static_profile,
        Rc::new(RefCell::new(program.consts.clone())),
    )
}

fn const_eq_i64(val: &ConstValue, target: i64) -> bool {
    match val {
        ConstValue::Int(i) => *i == target,
        ConstValue::Float(f) | ConstValue::CFloat(f) => {
            f.is_finite() && f.fract() == 0.0 && pystr::py_int_from_f64(*f) == Some(target)
        }
        ConstValue::Str(_) | ConstValue::BigInt(_) => false,
    }
}

#[allow(clippy::float_cmp)]
fn const_eq_json(val: &ConstValue, j: &Json) -> bool {
    match (val, j) {
        (ConstValue::Int(i), Json::Int(k)) => *i == *k,
        (ConstValue::Int(i), Json::Float(f)) => {
            f.is_finite() && f.fract() == 0.0 && pystr::py_int_from_f64(*f) == Some(*i)
        }
        (ConstValue::Float(f) | ConstValue::CFloat(f), Json::Int(k)) => {
            f.is_finite() && f.fract() == 0.0 && pystr::py_int_from_f64(*f) == Some(*k)
        }
        (ConstValue::Float(f) | ConstValue::CFloat(f), Json::Float(g)) => *f == *g,
        (ConstValue::Str(s), Json::String(t)) => s == t,
        _ => false,
    }
}

/// Port of `checker/warnings.py::_table_arg_name`.
pub fn _table_arg_name(stmt: &Instruction) -> Option<String> {
    if let Some(Some(arg0)) = stmt.args.first()
        && let Expr::Name(n) = arg0.as_ref()
    {
        return Some(n.name.clone());
    }
    None
}

/// Helper for `_with_inline_branches` taking `statements`.
pub fn _with_inline_branches(statements: &[StmtRef]) -> Vec<StmtRef> {
    let mut out = Vec::new();
    for stmt in statements {
        out.push(stmt.clone());
        if let Stmt::If(if_stmt) = stmt.as_ref() {
            if let Some(inline) = &if_stmt.inline {
                out.extend(_with_inline_branches(inline));
            }
            if let Some(inline_else) = &if_stmt.inline_else {
                out.extend(_with_inline_branches(inline_else));
            }
        }
    }
    out
}

/// Port of `checker/warnings.py::check`.
pub fn check(program: &Program, diags: &mut Diagnostics) {
    let mut declared: OrderedMap<String, String> = OrderedMap::new();
    for stmt in &program.statements {
        if let Stmt::Instruction(inst) = stmt.as_ref()
            && pystr::upper(&inst.keyword) == "DATATABLE"
            && let Some(name) = _table_arg_name(inst)
        {
            let resolved = instr_data::_const_table_name(&name, program);
            declared.insert(pystr::upper(&resolved), resolved);
        }
    }

    let mut called: BTreeSet<String> = BTreeSet::new();
    for stmt in _with_inline_branches(&program.statements) {
        let Stmt::Instruction(inst) = stmt.as_ref() else {
            continue;
        };
        let kw_upper = pystr::upper(&inst.keyword);
        if kw_upper != "DATATABLE" {
            for arg in inst.args.iter().flatten() {
                if let Expr::Name(n) = arg.as_ref() {
                    let raw = pystr::upper(&n.name);
                    if declared.contains_key(&raw) {
                        called.insert(raw);
                    }
                    let resolved = pystr::upper(&instr_data::_const_table_name(&n.name, program));
                    if declared.contains_key(&resolved) {
                        called.insert(resolved);
                    }
                }
            }
        }
        if kw_upper == "CALLTABLE"
            && let Some(Some(arg0)) = inst.args.first()
            && let Expr::String(s) = arg0.as_ref()
        {
            called.insert(pystr::upper(&s.value));
        }
    }

    for (key, name) in declared.iter() {
        if !called.contains(key) {
            diags.warning_basic(
                0,
                &format!("Table {name}, is declared but never called."),
                "",
            );
        }
    }
}

/// Helper for `_check_uint1_dims` taking `env`.
pub fn _check_uint1_dims(program: &Program, profile: &Profile, diags: &mut Diagnostics, env: &Env) {
    if profile.name != "cr6" && profile.name != "cr1000x" {
        return;
    }
    for stmt in &program.statements {
        let Stmt::Declaration(decl) = stmt.as_ref() else {
            continue;
        };
        for v in decl.vars.borrow().iter() {
            if v.type_.as_deref() != Some("UINT1") {
                continue;
            }
            let mut count: i64 = 1;
            let mut complete = true;
            for d in &v.dims {
                let Ok(size) = consteval::value_of(Some(d.as_ref()), env) else {
                    complete = false;
                    break;
                };
                let ConstValue::Int(n) = size else {
                    complete = false;
                    break;
                };
                if n < 1 {
                    complete = false;
                    break;
                }
                count = count.saturating_mul(n);
            }
            if complete && count % 4 != 0 {
                diags.warning_basic(
                    decl.base.node.line,
                    "Adjusting dimensionality of UINT1 so number of bytes evenly divisible by 4",
                    &decl.base.node.file,
                );
            }
        }
    }
}

/// Helper for `check_early` taking `env`.
pub fn check_early(program: &Program, profile: &Profile, diags: &mut Diagnostics, env: &Env) {
    _check_uint1_dims(program, profile, diags, env);
    _check_reachability(program, profile, diags, env);
    _check_fnotch(program, profile, diags, env);
    instr_rules::check_scan_params(program, profile, diags, env);
    instr_rules::check_scan_max_interval(program, profile, diags, env);
    instr_rules::check_data_interval(program, profile, diags, env);
}

/// Port of `checker/warnings.py::_kw`.
pub fn _kw(stmt: &Stmt) -> String {
    match stmt {
        Stmt::Instruction(i) => pystr::upper(&i.keyword),
        Stmt::SubDef(s) => pystr::upper(&s.keyword),
        Stmt::Unparsed(u) => {
            if let Some(m) = DATA_WORD_RE.match_(&u.base.text)
                && let Some(word) = m.group(1)
            {
                return pystr::upper(word);
            }
            pystr::upper(stmt.type_name())
        }
        _ => pystr::upper(stmt.type_name()),
    }
}

/// Port of `checker/warnings.py::_is_program_end`.
pub fn _is_program_end(stmt: &Stmt) -> bool {
    let Stmt::Instruction(inst) = stmt else {
        return false;
    };
    if pystr::upper(&inst.keyword) != "END" {
        return false;
    }
    inst.args.is_empty()
        || (inst.args.len() == 1
            && matches!(inst.args[0].as_deref(), Some(Expr::Name(n)) if pystr::upper(&n.name) == "PROG"))
}

/// Port of `checker/warnings.py::_display_name`.
pub fn _display_name(stmt: &Stmt, profile: &Profile) -> String {
    match stmt {
        Stmt::Instruction(inst) => profile
            .instructions()
            .get(&pystr::upper(&inst.keyword))
            .map_or_else(|| inst.keyword.clone(), |r| r.name.clone()),
        Stmt::DoLoop(d) => {
            let keyword = pystr::upper(&d.keyword);
            profile
                .instructions()
                .get(&keyword)
                .map_or(keyword, |r| r.name.clone())
        }
        Stmt::For(_) => profile
            .instructions()
            .get("FOR")
            .map_or_else(|| "FOR".to_string(), |r| r.name.clone()),
        Stmt::Call(_) => "Call".to_string(),
        _ => "Expr".to_string(),
    }
}

/// Port of `checker/warnings.py::_open_conditional`.
pub fn _open_conditional(statements: &[StmtRef], start: i64, end: i64) -> i64 {
    let n = i64::try_from(statements.len()).unwrap_or(i64::MAX);
    let clamp = |idx: i64| -> usize {
        let clamped = if idx < 0 {
            (idx + n).max(0)
        } else {
            idx.min(n)
        };
        usize::try_from(clamped).unwrap_or(0)
    };
    let a = clamp(start);
    let b = clamp(end);
    if a >= b {
        return 0;
    }
    let mut depth: i64 = 0;
    for stmt in &statements[a..b] {
        let kw = _kw(stmt);
        if (kw == "IF" && stmt.inline().is_none()) || kw == "SELECT" {
            depth += 1;
        } else if (kw == "ENDIF" || kw == "ENDSELECT") && depth > 0 {
            depth -= 1;
        }
    }
    depth
}

/// Port of `checker/warnings.py::_scan_span`.
pub fn _scan_span(statements: &[StmtRef], start: i64) -> (i64, bool) {
    let mut depth: i64 = 1;
    let mut has_exit = false;
    let mut j = usize::try_from(start + 1).unwrap_or(0);
    let n = statements.len();
    while j < n && depth > 0 {
        let stmt = &statements[j];
        let kw = _kw(stmt);
        if kw == "SCAN" {
            depth += 1;
        } else if kw == "NEXTSCAN" {
            depth -= 1;
        } else if kw == "EXITSCAN"
            || stmt
                .inline()
                .into_iter()
                .flatten()
                .chain(stmt.inline_else().into_iter().flatten())
                .any(|x| _kw(x) == "EXITSCAN")
        {
            has_exit = true;
        }
        j += 1;
    }
    (i64::try_from(j).unwrap_or(i64::MAX), has_exit)
}

/// Helper for `_scan_count_is_zero` taking `env`.
pub fn _scan_count_is_zero(stmt: &Instruction, env: &Env) -> Option<bool> {
    let arg3 = stmt.args.get(3)?.as_deref()?;
    let val = consteval::value_of(Some(arg3), env).ok()?;
    Some(const_eq_i64(&val, 0))
}

/// Helper for `_check_reachability` taking `env`.
pub fn _check_reachability(
    program: &Program,
    profile: &Profile,
    diags: &mut Diagnostics,
    env: &Env,
) {
    if diags
        .items
        .iter()
        .any(|d| d.message.starts_with("Illegal END or EXIT "))
    {
        return;
    }
    let statements = &program.statements;
    let n = statements.len();
    let mut dead = false;
    let mut open_depth: i64 = 0;
    let mut sub_depth: i64 = 0;
    let mut begun = false;
    let tasks = profile.pipeline_tasks().get("tasks");
    let pipeline_delays: BTreeSet<usize> =
        crate::checker::structure::pipeline_task_errors(program, env.profile)
            .into_iter()
            .filter(|(stmt, _)| {
                _kw(stmt) == "DELAY"
                    || tasks
                        .and_then(|tasks| tasks.get(&_kw(stmt)))
                        .and_then(|rule| rule.get("hides_unreached"))
                        .and_then(crate::profiles::Json::as_bool)
                        == Some(true)
            })
            .map(|(stmt, _)| crate::syntax::stmt_id(&stmt))
            .collect();

    let mut i: usize = 0;
    while i < n {
        let stmt = &statements[i];
        let kw = _kw(stmt);

        if sub_depth > 0 {
            if kw == "SUB" || kw == "FUNCTION" {
                sub_depth += 1;
            } else if ends_routine(stmt) {
                sub_depth -= 1;
            }
            i += 1;
            continue;
        }
        if kw == "SUB" || kw == "FUNCTION" {
            sub_depth = 1;
            i += 1;
            continue;
        }

        if kw == "BEGINPROG" {
            begun = true;
        }
        if kw == "SLOWSEQUENCE" {
            return;
        }

        if kw == "SCAN"
            && let Stmt::Instruction(inst) = stmt.as_ref()
        {
            if dead {
                let name = _display_name(stmt, profile);
                diags.warning(
                    inst.base.node.line,
                    &format!("{name} never reached at run time."),
                    &inst.base.node.file,
                    "unreached",
                    "",
                );
                return;
            }
            let (j_i64, has_exit) = _scan_span(statements, i64::try_from(i).unwrap_or(i64::MAX));
            let j = usize::try_from(j_i64).unwrap_or(n);
            let count_is_zero = _scan_count_is_zero(inst, env);
            dead = count_is_zero == Some(true) && !has_exit;
            if !begun && (profile.name == "cr6" || profile.name == "cr1000x") {
                dead = false;
            }
            let closer = if j <= n && j > i + 1 {
                Some(&statements[j - 1])
            } else {
                None
            };
            if dead
                && let Some(closer) = closer
                && let Some(hoisted_last) = closer.base().hoisted_last.get()
            {
                if hoisted_last
                    && !(j < n
                        && (statements[j].line(), statements[j].file())
                            == (closer.line(), closer.file()))
                {
                    diags.warning(
                        closer.line(),
                        "EndIf never reached at run time.",
                        closer.file(),
                        "unreached",
                        "",
                    );
                }
                return;
            }
            open_depth = _open_conditional(statements, i64::try_from(i).unwrap_or(i64::MAX), j_i64);
            i = j;
            continue;
        }

        if dead && open_depth > 0 {
            if kw == "ENDIF" || kw == "ENDSELECT" {
                open_depth -= 1;
            } else if (matches!(stmt.as_ref(), Stmt::If(if_stmt) if if_stmt.inline.is_none()))
                || kw == "SELECT"
            {
                open_depth += 1;
            }
            if open_depth > 0 {
                i += 1;
                continue;
            }
        }
        if dead && pipeline_delays.contains(&stmt_id(stmt)) {
            i += 1;
            continue;
        }
        if dead && (SCAN_ONLY_KEYWORDS.contains(&kw.as_str()) || kw == "SUBSCAN") {
            return;
        }
        if dead && (matches!(stmt.as_ref(), Stmt::If(_)) || kw == "SELECT" || kw == "NEXTSCAN") {
            return;
        }
        if dead {
            let is_declaration = matches!(
                stmt.as_ref(),
                Stmt::Declaration(_) | Stmt::ConstDecl(_) | Stmt::Label(_)
            ) || NON_EXECUTABLE_KEYWORDS.contains(&kw.as_str())
                || kw == "EXITSCAN"
                || _is_program_end(stmt);
            if !is_declaration {
                let mut name = _display_name(stmt, profile);
                if let Stmt::Call(call) = stmt.as_ref()
                    && !call.explicit
                    && !call.tail_ignored
                    && (profile.name == "cr6" || profile.name == "cr1000x")
                {
                    let mut is_routine = false;
                    for st in statements {
                        if let Stmt::SubDef(sd) = st.as_ref()
                            && pystr::upper(&sd.name) == pystr::upper(&call.name)
                        {
                            is_routine = true;
                            break;
                        }
                    }
                    if !is_routine {
                        let ctx = context::of(
                            &Rc::new(program.clone()),
                            profiles::get(profile.name).ok(),
                            None,
                        );
                        if ctx
                            .declared_variable_names()
                            .contains(&pystr::upper(&call.name))
                        {
                            return;
                        }
                        name = "Expr".to_string();
                    }
                }
                diags.warning(
                    stmt.line(),
                    &format!("{name} never reached at run time."),
                    stmt.file(),
                    "unreached",
                    "",
                );
                return;
            }
        }
        i += 1;
    }
}

/// Helper for `check_background_calibration` taking `env`.
pub fn check_background_calibration(
    program: &Program,
    profile: &Profile,
    diags: &mut Diagnostics,
    env: &Env,
) {
    let data = profile.background_calibration();
    if data.is_empty() {
        return;
    }
    let Some(threshold_ms) = data.get("threshold_ms").and_then(Json::as_f64) else {
        return;
    };
    let statements = &program.statements;
    let last_line = statements.last().map_or(1, |s| s.line());

    if instr_measure::is_sequential(program, profile, env) {
        return;
    }
    let busy_slack = data.get("busy_slack").and_then(Json::as_i64).unwrap_or(0);
    let mut busy: BTreeSet<usize> = BTreeSet::new();
    let mut timed: BTreeSet<usize> = BTreeSet::new();
    for b in scan_time::budgets(program, env, profile, false) {
        if b.total != 0 {
            timed.insert(stmt_id(&b.scan));
            let gap = if b.max_integ > 0 {
                b.max_integ + scan_time::model_for(profile.name).map_or(0, |m| m.busy_extra)
            } else {
                0
            };
            if b.interval - b.total < busy_slack.max(gap) && !b.pulse_only {
                busy.insert(stmt_id(&b.scan));
            }
        }
    }

    let mut in_slow = false;
    let mut main_scan_index: i64 = 0;
    for stmt in statements {
        let kw = _kw(stmt);
        if kw == "SLOWSEQUENCE" {
            in_slow = true;
            continue;
        }
        if kw != "SCAN" || in_slow {
            continue;
        }
        let Stmt::Instruction(inst) = stmt.as_ref() else {
            continue;
        };
        main_scan_index += 1;
        let interval_ms = _scan_interval_ms(inst, env);
        let sid = stmt_id(stmt);
        if busy.contains(&sid)
            || (!timed.contains(&sid) && interval_ms.is_some_and(|ms| ms < threshold_ms))
        {
            diags.warning_basic(
                last_line,
                &format!(
                    "When Main Sequence Scan {main_scan_index} is running, background calibration will be disabled."
                ),
                "",
            );
        }
    }
}

/// Helper for `_value_in` taking `args`, `values`, and `env`.
pub fn _value_in(args: &[Option<ExprRef>], position: i64, values: &[Json], env: &Env) -> bool {
    if position < 1 {
        return false;
    }
    let Ok(idx) = usize::try_from(position - 1) else {
        return false;
    };
    let Some(Some(arg)) = args.get(idx) else {
        return false;
    };
    let Ok(val) = consteval::value_of(Some(arg.as_ref()), env) else {
        return false;
    };
    values.iter().any(|v| const_eq_json(&val, v))
}

/// Helper for `check_slow_scan_skips` taking `env`.
pub fn check_slow_scan_skips(
    program: &Program,
    profile: &Profile,
    diags: &mut Diagnostics,
    env: &Env,
) {
    let Some(tasks_arr) = profile
        .pipeline_tasks()
        .get("slow_scan_tasks")
        .and_then(Json::as_array)
    else {
        return;
    };
    if tasks_arr.is_empty() {
        return;
    }
    let tasks: BTreeSet<&str> = tasks_arr.iter().filter_map(Json::as_str).collect();
    if instr_measure::is_sequential(program, profile, env) {
        return;
    }
    let rules = profile.pipeline_tasks().get("tasks");
    let no_task_map = profile.pipeline_tasks().get("slow_scan_no_task");

    let mut mains: Vec<Option<f64>> = Vec::new();
    let mut slows: Vec<(i64, i64, Option<f64>, bool)> = Vec::new();
    let mut seq: i64 = 0;
    let mut scan_no: i64 = 0;
    let mut cur_idx: Option<usize> = None;

    for stmt in _with_inline_branches(&program.statements) {
        let mut kw = _kw(&stmt);
        if kw == "NEXT" && NEXT_SCAN_RE.match_(stmt.text()).is_some() {
            kw = "NEXTSCAN".to_string();
        }
        if kw == "SLOWSEQUENCE" {
            seq += 1;
            scan_no = 0;
        } else if kw == "SCAN"
            && let Stmt::Instruction(inst) = stmt.as_ref()
        {
            let interval = _scan_interval_ms(inst, env);
            if seq == 0 {
                mains.push(interval);
            } else {
                scan_no += 1;
                slows.push((seq, scan_no, interval, false));
                cur_idx = Some(slows.len() - 1);
            }
        } else if kw == "NEXTSCAN" {
            cur_idx = None;
        } else if let Some(ci) = cur_idx
            && tasks.contains(kw.as_str())
            && let Stmt::Instruction(inst) = stmt.as_ref()
        {
            let constants = rules
                .and_then(|r| r.get(&kw))
                .and_then(|r| r.get("constant_values"))
                .and_then(Json::as_object);
            let no_task = no_task_map
                .and_then(|m| m.get(&kw))
                .and_then(Json::as_object);

            let mut constants_ok = true;
            if let Some(entries) = constants {
                for (p, allowed) in entries {
                    let Ok(pos) = p.parse::<i64>() else {
                        constants_ok = false;
                        break;
                    };
                    if pos < 1 {
                        constants_ok = false;
                        break;
                    }
                    let Ok(idx) = usize::try_from(pos - 1) else {
                        constants_ok = false;
                        break;
                    };
                    let Some(arg) = inst.args.get(idx) else {
                        constants_ok = false;
                        break;
                    };
                    let Ok(val) = consteval::value_of(arg.as_deref(), env) else {
                        constants_ok = false;
                        break;
                    };
                    let Some(allowed_arr) = allowed.as_array() else {
                        constants_ok = false;
                        break;
                    };
                    if !allowed_arr.iter().any(|a| const_eq_json(&val, a)) {
                        constants_ok = false;
                        break;
                    }
                }
            }

            let mut any_no_task = false;
            if constants_ok && let Some(entries) = no_task {
                for (p, vals) in entries {
                    if let Ok(pos) = p.parse::<i64>()
                        && let Some(vals_arr) = vals.as_array()
                        && _value_in(&inst.args, pos, vals_arr, env)
                    {
                        any_no_task = true;
                        break;
                    }
                }
            }

            if constants_ok && !any_no_task {
                slows[ci].3 = true;
            }
        }
    }

    let last_line = program.statements.last().map_or(1, |s| s.line());
    for (seq_no, scan_no, interval, has_task) in &slows {
        for (i, main_interval) in mains.iter().enumerate() {
            let main_no = i + 1;
            if *has_task
                && let (Some(iv), Some(miv)) = (interval, main_interval)
                && iv <= miv
            {
                diags.warning_basic(
                    last_line,
                    &format!(
                        "Slow Seq {seq_no}, Scan {scan_no}, may skip scans if running with Scan {main_no}."
                    ),
                    "",
                );
            }
        }
    }
}

/// Port of `checker/warnings.py::_scan_interval_ms`.
pub fn _scan_interval_ms(stmt: &Instruction, env: &Env) -> Option<f64> {
    let split = instr_rules::scan_split(stmt, env)?;
    let sign: i64 = if split.1 >= 0 { 1 } else { -1 };
    let abs_us = split.1.wrapping_abs();
    let units = split
        .0
        .wrapping_mul(10_000)
        .wrapping_add((abs_us / 100) * sign);
    let masked = units & 0xFFFF_FFFF;
    let signed = if masked >= (1_i64 << 31) {
        masked - (1_i64 << 32)
    } else {
        masked
    };
    i32::try_from(signed).ok().map(|v| f64::from(v) / 10.0)
}

/// Helper for `_check_fnotch` taking `env`.
pub fn _check_fnotch(program: &Program, profile: &Profile, diags: &mut Diagnostics, env: &Env) {
    let data = profile.fnotch();
    let Some(valid_arr) = data.get("valid").and_then(Json::as_array) else {
        return;
    };
    let Some(message) = data.get("message").and_then(Json::as_str) else {
        return;
    };
    let Some(positions) = data.get("instructions") else {
        return;
    };

    for stmt in &program.statements {
        let Stmt::Instruction(inst) = stmt.as_ref() else {
            continue;
        };
        let Some(pos) = positions
            .get(&pystr::upper(&inst.keyword))
            .and_then(Json::as_i64)
        else {
            continue;
        };
        if pos < 1 {
            continue;
        }
        let Ok(idx) = usize::try_from(pos - 1) else {
            continue;
        };
        let Some(Some(arg)) = inst.args.get(idx) else {
            continue;
        };
        if !consteval::is_constant(Some(arg.as_ref()), env) {
            continue;
        }
        let Ok(value) = consteval::value_of(Some(arg.as_ref()), env) else {
            continue;
        };
        if !valid_arr.iter().any(|v| const_eq_json(&value, v)) {
            diags.warning(
                inst.base.node.line,
                message,
                &inst.base.node.file,
                "fnotch",
                "",
            );
        }
    }
}

/// Port of `checker/warnings.py::_call_args`.
pub fn _call_args(text: &str, start: i64) -> Option<Vec<String>> {
    let chars: Vec<char> = text.chars().collect();
    let mut depth: i64 = 1;
    let mut args: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut i = usize::try_from(start.max(0)).unwrap_or(0);
    while i < chars.len() {
        let ch = chars[i];
        if ch == '"' {
            let rel = chars[i + 1..].iter().position(|&c| c == '"')?;
            let j = i + 1 + rel;
            for &c in &chars[i..=j] {
                cur.push(c);
            }
            i = j + 1;
            continue;
        }
        if ch == '(' || ch == '{' {
            depth += 1;
        } else if ch == ')' || ch == '}' {
            depth -= 1;
            if depth == 0 {
                args.push(cur);
                if args.iter().any(|x| !pystr::strip(x).is_empty()) {
                    return Some(args);
                }
                return Some(Vec::new());
            }
        } else if ch == ',' && depth == 1 {
            args.push(cur);
            cur = String::new();
            i += 1;
            continue;
        }
        cur.push(ch);
        i += 1;
    }
    None
}

/// Port of `checker/warnings.py::add_truncate_warnings`.
pub fn add_truncate_warnings(program: &Program, diags: &mut Diagnostics) {
    let ctx = context::of(&Rc::new(program.clone()), None, None);
    let variables = ctx.declared_variable_names();
    for stmt in &program.statements {
        let text = stmt.text();
        let Some(m) = TRUNCATE_CALL_RE.search(text) else {
            continue;
        };
        let Some(call_name) = m.group(1) else {
            continue;
        };
        let end_idx = i64::try_from(m.end0()).unwrap_or(i64::MAX);
        let Some(args) = _call_args(text, end_idx) else {
            continue;
        };
        let need: usize = match pystr::upper(call_name).as_str() {
            "IFTIME" | "TIMEINTOINTERVAL" => 3,
            _ => continue,
        };
        if args.len() == need {
            continue;
        }
        let with_vars = args
            .iter()
            .filter(|a| {
                let cleaned = STRING_LIT_RE.sub("", a);
                IDENT_RE
                    .findall(&cleaned)
                    .into_iter()
                    .any(|w| variables.contains(&pystr::upper(w)))
            })
            .count();
        let count = need.saturating_sub(args.len()) + with_vars;
        if count == 0 {
            continue;
        }
        let call_lower = pystr::lower(call_name);
        let at = diags.items.iter().position(|d| {
            (d.file.as_str(), d.line) == (stmt.file(), stmt.line())
                && d.severity == "error"
                && ARITY_ERROR_PREFIXES
                    .iter()
                    .any(|p| d.message.starts_with(p))
                && pystr::lower(&d.message).contains(&call_lower)
        });
        let Some(at) = at else {
            continue;
        };
        diags.items.splice(
            at..at,
            (0..count).map(|_| {
                Rc::new(Diagnostic::basic(
                    "warning",
                    stmt.line(),
                    "Parameter will truncate to the nearest integer.",
                    stmt.file(),
                ))
            }),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syntax::{
        Assignment, Call, Declaration, DoLoop, For, If, Name, Node, Number, NumberValue, StmtBase,
        StringLit, SubDef, Unparsed, VarDecl,
    };

    fn num_expr(line: i64, file: &str, n: i64) -> ExprRef {
        Rc::new(Expr::Number(Number {
            node: Node::new(line, file),
            value: NumberValue::Int(n),
            text: n.to_string(),
        }))
    }

    fn name_expr(line: i64, file: &str, name: &str) -> ExprRef {
        Rc::new(Expr::Name(Name {
            node: Node::new(line, file),
            name: name.to_string(),
        }))
    }

    fn str_expr(line: i64, file: &str, value: &str) -> ExprRef {
        Rc::new(Expr::String(StringLit {
            node: Node::new(line, file),
            value: value.to_string(),
        }))
    }

    fn instr(line: i64, file: &str, keyword: &str, args: Vec<Option<ExprRef>>) -> StmtRef {
        Rc::new(Stmt::Instruction(Instruction {
            base: StmtBase::new(line, file),
            keyword: keyword.to_string(),
            args,
            ..Default::default()
        }))
    }

    #[test]
    fn table_arg_name_and_inline_branches() {
        let dt = Instruction {
            base: StmtBase::new(1, "t.cr6"),
            keyword: "DataTable".to_string(),
            args: vec![Some(name_expr(1, "t.cr6", "MyTable"))],
            ..Default::default()
        };
        assert_eq!(_table_arg_name(&dt).as_deref(), Some("MyTable"));

        let dt_str = Instruction {
            base: StmtBase::new(1, "t.cr6"),
            keyword: "DataTable".to_string(),
            args: vec![Some(str_expr(1, "t.cr6", "MyTable"))],
            ..Default::default()
        };
        assert_eq!(_table_arg_name(&dt_str), None);

        let a = instr(
            2,
            "t.cr6",
            "CallTable",
            vec![Some(name_expr(2, "t.cr6", "A"))],
        );
        let b = instr(
            2,
            "t.cr6",
            "CallTable",
            vec![Some(name_expr(2, "t.cr6", "B"))],
        );
        let if_stmt = Rc::new(Stmt::If(If {
            base: StmtBase::new(2, "t.cr6"),
            cond: None,
            inline: Some(vec![a.clone()]),
            inline_else: Some(vec![b.clone()]),
        }));
        let flat = _with_inline_branches(std::slice::from_ref(&if_stmt));
        assert_eq!(flat.len(), 3);
        assert!(Rc::ptr_eq(&flat[0], &if_stmt));
        assert!(Rc::ptr_eq(&flat[1], &a));
        assert!(Rc::ptr_eq(&flat[2], &b));
    }

    #[test]
    fn check_uint1_dims_matches_python() {
        let prof_cr6 = profiles::get("cr6").unwrap();
        let prof_cr300 = profiles::get("cr300").unwrap();
        let decl = Rc::new(Stmt::Declaration(Declaration {
            base: StmtBase::new(1, "t.cr6"),
            kind: "PUBLIC".to_string(),
            vars: vec![
                Rc::new(VarDecl {
                    dims: vec![num_expr(1, "t.cr6", 3)],
                    type_: Some("UINT1".to_string()),
                    ..VarDecl::new("a")
                }),
                Rc::new(VarDecl {
                    dims: vec![num_expr(1, "t.cr6", 4)],
                    type_: Some("UINT1".to_string()),
                    ..VarDecl::new("b")
                }),
            ]
            .into(),
        }));
        let prog = Program::new("t.cr6", vec![decl]);
        let mut diags = Diagnostics::new();
        _check_uint1_dims(&prog, prof_cr6, &mut diags, &Env::new(prof_cr6));
        assert_eq!(diags.items.len(), 1);
        assert_eq!(diags.items[0].line, 1);
        assert_eq!(
            diags.items[0].message,
            "Adjusting dimensionality of UINT1 so number of bytes evenly divisible by 4"
        );

        let mut diags_cr300 = Diagnostics::new();
        _check_uint1_dims(&prog, prof_cr300, &mut diags_cr300, &Env::new(prof_cr300));
        assert_eq!(diags_cr300.items.len(), 0);
    }

    #[test]
    fn kw_is_program_end_and_display_name() {
        let prof = profiles::get("cr6").unwrap();
        let delay = instr(1, "t.cr6", "delay", vec![]);
        assert_eq!(_kw(&delay), "DELAY");
        assert_eq!(_display_name(&delay, prof), "Delay");

        let unparsed = Stmt::Unparsed(Unparsed {
            base: StmtBase {
                node: Node::new(2, "t.cr6"),
                text: "  DataLong 1, 2".to_string(),
                ..Default::default()
            },
            reason: "err".to_string(),
            expr: None,
        });
        assert_eq!(_kw(&unparsed), "DATALONG");

        let end_bare = instr(3, "t.cr6", "End", vec![]);
        let end_prog = instr(4, "t.cr6", "End", vec![Some(name_expr(4, "t.cr6", "Prog"))]);
        let end_other = instr(5, "t.cr6", "End", vec![Some(name_expr(5, "t.cr6", "Foo"))]);
        assert!(_is_program_end(&end_bare));
        assert!(_is_program_end(&end_prog));
        assert!(!_is_program_end(&end_other));

        let call = Stmt::Call(Call {
            base: StmtBase::new(6, "t.cr6"),
            name: "MySub".to_string(),
            ..Default::default()
        });
        assert_eq!(_display_name(&call, prof), "Call");

        let assign = Stmt::Assignment(Assignment {
            base: StmtBase::new(7, "t.cr6"),
            ..Default::default()
        });
        assert_eq!(_display_name(&assign, prof), "Expr");

        let doloop = Stmt::DoLoop(DoLoop {
            base: StmtBase::new(8, "t.cr6"),
            keyword: "Do".to_string(),
            ..Default::default()
        });
        assert_eq!(_display_name(&doloop, prof), "Do");

        let for_stmt = Stmt::For(For {
            base: StmtBase::new(9, "t.cr6"),
            ..Default::default()
        });
        assert_eq!(_display_name(&for_stmt, prof), "FOR");
    }

    #[test]
    fn check_reachability_matches_python() {
        let prof = profiles::get("cr6").unwrap();
        let prog = Program::new(
            "t.cr6",
            vec![
                instr(1, "t.cr6", "BeginProg", vec![]),
                instr(
                    2,
                    "t.cr6",
                    "Scan",
                    vec![
                        Some(num_expr(2, "t.cr6", 1)),
                        Some(name_expr(2, "t.cr6", "Sec")),
                        Some(num_expr(2, "t.cr6", 0)),
                        Some(num_expr(2, "t.cr6", 0)),
                    ],
                ),
                instr(3, "t.cr6", "NextScan", vec![]),
                instr(4, "t.cr6", "Delay", vec![]),
                instr(5, "t.cr6", "EndProg", vec![]),
            ],
        );
        let mut diags = Diagnostics::new();
        _check_reachability(&prog, prof, &mut diags, &Env::new(prof));
        assert_eq!(diags.items.len(), 1);
        assert_eq!(diags.items[0].line, 4);
        assert_eq!(diags.items[0].message, "Delay never reached at run time.");
        assert_eq!(diags.items[0].kind, "unreached");

        // Sub/Function definitions are skipped, and SlowSequence stops reachability.
        let sub_prog = Program::new(
            "t.cr6",
            vec![
                instr(1, "t.cr6", "BeginProg", vec![]),
                instr(
                    2,
                    "t.cr6",
                    "Scan",
                    vec![
                        Some(num_expr(2, "t.cr6", 1)),
                        Some(name_expr(2, "t.cr6", "Sec")),
                        Some(num_expr(2, "t.cr6", 0)),
                        Some(num_expr(2, "t.cr6", 0)),
                    ],
                ),
                instr(3, "t.cr6", "NextScan", vec![]),
                Rc::new(Stmt::SubDef(SubDef {
                    base: StmtBase::new(4, "t.cr6"),
                    keyword: "Sub".to_string(),
                    name: "Helper".to_string(),
                    ..Default::default()
                })),
                instr(5, "t.cr6", "Delay", vec![]),
                instr(6, "t.cr6", "EndSub", vec![]),
                instr(7, "t.cr6", "SlowSequence", vec![]),
                instr(8, "t.cr6", "Delay", vec![]),
                instr(9, "t.cr6", "EndProg", vec![]),
            ],
        );
        let mut diags2 = Diagnostics::new();
        _check_reachability(&sub_prog, prof, &mut diags2, &Env::new(prof));
        assert_eq!(diags2.items.len(), 0);
    }

    #[test]
    fn check_fnotch_matches_python() {
        let prof_cr300 = profiles::get("cr300").unwrap();
        let prog = Program::new(
            "t.cr300",
            vec![
                instr(
                    4,
                    "t.cr300",
                    "PanelTemp",
                    vec![
                        Some(name_expr(4, "t.cr300", "PTemp")),
                        Some(num_expr(4, "t.cr300", 250)),
                    ],
                ),
                instr(
                    5,
                    "t.cr300",
                    "PanelTemp",
                    vec![
                        Some(name_expr(5, "t.cr300", "PTemp")),
                        Some(num_expr(5, "t.cr300", 60)),
                    ],
                ),
            ],
        );
        let mut diags = Diagnostics::new();
        _check_fnotch(&prog, prof_cr300, &mut diags, &Env::new(prof_cr300));
        assert_eq!(diags.items.len(), 1);
        assert_eq!(diags.items[0].line, 4);
        assert_eq!(
            diags.items[0].message,
            "Invalid fnotch value, must be 50,60 400 or 4000"
        );
        assert_eq!(diags.items[0].kind, "fnotch");
    }

    #[test]
    fn call_args_and_truncate_warnings_match_python() {
        assert_eq!(
            _call_args("IfTime(0,10,Sec)", 7),
            Some(vec!["0".into(), "10".into(), "Sec".into()])
        );
        assert_eq!(_call_args("IfTime()", 7), Some(vec![]));
        assert_eq!(
            _call_args("IfTime(a, \"x,y\")", 7),
            Some(vec!["a".into(), " \"x,y\"".into()])
        );
        assert_eq!(_call_args("IfTime(a", 7), None);

        let decl = Rc::new(Stmt::Declaration(Declaration {
            base: StmtBase::new(1, "t.cr6"),
            kind: "PUBLIC".to_string(),
            vars: vec![Rc::new(VarDecl::new("x"))].into(),
        }));
        let unparsed = Rc::new(Stmt::Unparsed(Unparsed {
            base: StmtBase {
                node: Node::new(2, "t.cr6"),
                text: "IfTime(x)".to_string(),
                ..Default::default()
            },
            reason: "bad".to_string(),
            expr: None,
        }));
        let prog = Program::new("t.cr6", vec![decl, unparsed]);
        let mut diags = Diagnostics::new();
        diags.error_basic(2, "Expression expects , : IfTime", "t.cr6");
        add_truncate_warnings(&prog, &mut diags);
        assert_eq!(diags.items.len(), 4);
        for d in &diags.items[..3] {
            assert_eq!(d.severity, "warning");
            assert_eq!(d.line, 2);
            assert_eq!(d.message, "Parameter will truncate to the nearest integer.");
        }
        assert_eq!(diags.items[3].severity, "error");
        assert_eq!(diags.items[3].message, "Expression expects , : IfTime");
    }
}
