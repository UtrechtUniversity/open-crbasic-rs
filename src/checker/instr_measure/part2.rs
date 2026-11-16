//! Port of `checker/instr_measure.py` (lines 754-1540).

#![allow(
    dead_code,
    unused_variables,
    unused_imports,
    clippy::too_many_arguments,
    clippy::cast_precision_loss,
    clippy::used_underscore_items,
    reason = "port keeps Python signatures, leading-underscore names, and int->float semantics"
)]

use super::part1::{_const_value, _delay_option_byte, _int_value, _task_memory_full};
use super::part3::{_flat_offset, _index_value};
use crate::checker::instr_rules::_scan_raw;
use std::cell::RefCell;
use std::collections::{BTreeSet, HashMap};
use std::rc::Rc;
use std::sync::LazyLock;

use crate::checker::context::{self, ScopeLookup, ScopeTables};
use crate::checker::longconv;
use crate::checker::scan_time;
use crate::checker::var_types::Typer;
use crate::consteval::{self, Env};
use crate::diagnostics::{Diagnostic, Diagnostics};
use crate::mode;
use crate::profiles::{Json, Profile};
use crate::pyre;
use crate::pystr;
use crate::syntax::{
    self, ConstValue, Expr, Instruction, NumberValue, OrderedMap, Program, ProgramRef, Stmt,
    StmtRef, SubDef, ends_routine, stmt_id,
};

static NEXT_SCAN_OR_SUBSCAN_RE: LazyLock<pyre::Regex> =
    LazyLock::new(|| pyre::Regex::new(r"(?i)\s*Next\s+(Sub)?Scan\b", 0));
static END_IF_OR_SELECT_RE: LazyLock<pyre::Regex> =
    LazyLock::new(|| pyre::Regex::new(r"(?i)\s*End\s+(If|Select)\b", 0));
static NEXT_SCAN_RE: LazyLock<pyre::Regex> =
    LazyLock::new(|| pyre::Regex::new(r"(?i)\s*Next\s+Scan\b", 0));
static NEXT_SUBSCAN_RE: LazyLock<pyre::Regex> =
    LazyLock::new(|| pyre::Regex::new(r"(?i)\s*Next\s+SubScan\b", 0));

fn delay_us_factor(unit: i64) -> Option<i64> {
    super::part1::delay_us_per_unit(unit)
}

fn delay_option_from_const(arg: Option<&Expr>, env: &Env) -> Option<i64> {
    match _const_value(arg, env)? {
        ConstValue::Int(i) => Some(_delay_option_byte(i)),
        ConstValue::Float(f) | ConstValue::CFloat(f) if f.is_finite() => {
            if f > 3.402_823_5e38 {
                Some(255)
            } else if !(-2_147_483_648.0..2_147_483_648.0).contains(&f) {
                Some(0)
            } else {
                pystr::py_int_from_f64(f).map(|i| i.rem_euclid(256))
            }
        }
        ConstValue::BigInt(s) => {
            if let Ok(i) = s.parse::<i64>() {
                Some(_delay_option_byte(i))
            } else if let Ok(f) = s.parse::<f64>() {
                Some(if f > 3.402_823_5e38 { 255 } else { 0 })
            } else {
                Some(if s.starts_with('-') { 0 } else { 255 })
            }
        }
        ConstValue::Float(_) | ConstValue::CFloat(_) | ConstValue::Str(_) => None,
    }
}

fn expr_name_upper(expr: &Expr) -> Option<(&str, String)> {
    match expr {
        Expr::Name(n) => Some((&n.name, pystr::upper(&n.name))),
        Expr::Index(idx) => Some((&idx.name, pystr::upper(&idx.name))),
        _ => None,
    }
}

/// Port of checker/instr_measure.py::_check_delay
pub fn _check_delay(
    stmt: &Instruction,
    cfg: &Json,
    env: &Env,
    diags: &mut Diagnostics,
    sequential: bool,
    slow: bool,
    typer: &mut Typer,
) {
    let args = &stmt.args;
    let option_arg = args.first().and_then(|a| a.as_deref());
    let interval_arg = args.get(1).and_then(|a| a.as_deref());
    let units_arg = args.get(2).and_then(|a| a.as_deref());

    let option = delay_option_from_const(option_arg, env);
    if let Some(opt) = option
        && !matches!(opt, 0..=2)
    {
        diags.error_basic(
            stmt.base.node.line,
            &format!("Illegal option {opt}"),
            &stmt.base.node.file,
        );
    }

    let mut owned_env: Option<Env> = None;
    {
        let consts = env.consts.borrow();
        let has_shadow = typer.types.keys().any(|k| consts.contains_key(k));
        if has_shadow {
            let mut filtered = OrderedMap::new();
            for (name, value) in consts.iter() {
                if !typer.types.contains_key(name) {
                    filtered.insert(name.clone(), value.clone());
                }
            }
            owned_env = Some(Env::with_consts(
                env.profile,
                Rc::new(RefCell::new(filtered)),
            ));
        }
    }
    let active_env = owned_env.as_ref().unwrap_or(env);

    let interval_value = _const_value(interval_arg, active_env);
    if matches!(interval_value, Some(ConstValue::Str(_)))
        || typer.of(interval_arg, 0).as_deref() == Some("STRING")
    {
        let message = if option == Some(2) {
            "SDM Delay interval must be constant."
        } else {
            "Delay interval cannot be of type String."
        };
        diags.error_basic(stmt.base.node.line, message, &stmt.base.node.file);
        return;
    }

    let option0_limit = cfg
        .get("option0_limit")
        .and_then(Json::as_i64)
        .filter(|&v| v != 0);
    let option1_limit = cfg
        .get("option1_limit")
        .and_then(Json::as_i64)
        .filter(|&v| v != 0);

    if option == Some(0)
        && let Some(limit0) = option0_limit
    {
        if sequential {
            return;
        }
        if interval_arg.is_some() && !consteval::is_constant(interval_arg, active_env) {
            diags.error_basic(
                stmt.base.node.line,
                "Measurement delay must have constant interval.",
                &stmt.base.node.file,
            );
            return;
        }
        let unit = _const_value(units_arg, active_env);
        let interval_num = match &interval_value {
            Some(ConstValue::Int(i)) => Some(*i as f64),
            Some(ConstValue::Float(f) | ConstValue::CFloat(f)) if f.is_finite() => Some(*f),
            _ => None,
        };
        let unit_int = match unit {
            Some(ConstValue::Int(u)) if delay_us_factor(u).is_some() => Some(u),
            _ => None,
        };
        let (Some(interval_f), Some(unit_i)) = (interval_num, unit_int) else {
            return;
        };
        let us = scan_time::delay_us(interval_f, unit_i);
        if us > limit0 {
            diags.error_basic(
                stmt.base.node.line,
                "Measurement Delay Exceeds 60 min.",
                &stmt.base.node.file,
            );
        }
        if cfg
            .get("task_memory")
            .and_then(Json::as_bool)
            .unwrap_or(false)
            && scan_time::task_memory_overflow(us)
        {
            _task_memory_full(stmt, diags);
        }
    } else if (option == Some(1) || (option == Some(0) && active_env.profile.single_pass))
        && let Some(limit1) = option1_limit
    {
        if let Some(us) = _delay_interval_us(interval_arg, units_arg, active_env)
            && us >= limit1 as f64
        {
            diags.error_basic(
                stmt.base.node.line,
                "Max Delay is 24 Hours",
                &stmt.base.node.file,
            );
        }
    } else if option == Some(2)
        && cfg
            .get("option2_constant")
            .and_then(Json::as_bool)
            .unwrap_or(true)
    {
        let is_zero = match &interval_value {
            Some(ConstValue::Int(0)) => true,
            Some(ConstValue::Float(f) | ConstValue::CFloat(f)) => *f == 0.0,
            _ => false,
        };
        if interval_arg.is_some() && !consteval::is_constant(interval_arg, active_env) {
            diags.error_basic(
                stmt.base.node.line,
                "SDM Delay interval must be constant.",
                &stmt.base.node.file,
            );
        } else if is_zero && option0_limit.is_some() {
            diags.error_basic(
                stmt.base.node.line,
                "Invalid time specified.",
                &stmt.base.node.file,
            );
        } else if slow
            && cfg
                .get("sdm_slow_sequence")
                .and_then(Json::as_bool)
                .unwrap_or(false)
        {
            diags.error_basic(
                stmt.base.node.line,
                "SDM Delay Not Allowed in Slow Sequence.",
                &stmt.base.node.file,
            );
        }
    }
}

/// Port of checker/instr_measure.py::_delay_interval_us
pub fn _delay_interval_us(
    interval_arg: Option<&Expr>,
    units_arg: Option<&Expr>,
    env: &Env,
) -> Option<f64> {
    let value = _const_value(interval_arg, env)?;
    let unit = _const_value(units_arg, env)?;
    if !consteval::is_finite_number(&value) || !consteval::is_finite_number(&unit) {
        return None;
    }
    let val_f = match value {
        ConstValue::Int(i) => i as f64,
        ConstValue::Float(f) | ConstValue::CFloat(f) => f,
        ConstValue::BigInt(ref s) => s.parse::<f64>().ok()?,
        ConstValue::Str(_) => return None,
    };
    if val_f < 0.0 {
        return None;
    }
    let unit_i = match unit {
        ConstValue::Int(u) => u,
        ConstValue::Float(f) | ConstValue::CFloat(f) => pystr::py_int_from_f64(f)?,
        ConstValue::BigInt(ref s) => s.parse::<i64>().ok()?,
        ConstValue::Str(_) => return None,
    };
    if val_f >= 2_147_483_648.0 && !(env.profile.single_pass && unit_i == 1) {
        return None;
    }
    let us_per_unit = delay_us_factor(unit_i)?;
    Some(val_f * (us_per_unit as f64))
}

/// Port of checker/instr_measure.py::_check_cs616_reps
pub fn _check_cs616_reps(stmt: &Instruction, measure: &Json, env: &Env, diags: &mut Diagnostics) {
    let Some(pos) = measure.get("cs616_min_reps_pos").and_then(Json::as_i64) else {
        return;
    };
    let Ok(idx) = usize::try_from(pos - 1) else {
        return;
    };
    if idx >= stmt.args.len() {
        return;
    }
    if let Some(value) = _int_value(stmt.args[idx].as_deref(), env)
        && value <= 0
    {
        diags.error_basic(
            stmt.base.node.line,
            &format!("Invalid reps {value}, must be > 0"),
            &stmt.base.node.file,
        );
    }
}

/// Port of checker/instr_measure.py::_check_excite_magnitude
pub fn _check_excite_magnitude(
    stmt: &Instruction,
    measure: &Json,
    env: &Env,
    diags: &mut Diagnostics,
) {
    let Some(pos) = measure.get("excite_magnitude_pos").and_then(Json::as_i64) else {
        return;
    };
    let Some(max_mv) = measure.get("excite_max_mv").and_then(Json::as_f64) else {
        return;
    };
    let Ok(idx) = usize::try_from(pos - 1) else {
        return;
    };
    if idx >= stmt.args.len() {
        return;
    }
    let val = match _const_value(stmt.args[idx].as_deref(), env) {
        Some(ConstValue::Int(i)) => Some((i as f64).abs()),
        Some(ConstValue::Float(f) | ConstValue::CFloat(f)) => Some(f.abs()),
        Some(ConstValue::BigInt(ref s)) => s.parse::<f64>().ok().map(f64::abs),
        _ => None,
    };
    if let Some(abs_val) = val
        && abs_val > max_mv
    {
        diags.error_basic(
            stmt.base.node.line,
            &format!("Excitation exceeds {max_mv:.6}."),
            &stmt.base.node.file,
        );
    }
}

pub const _UNIT_NAMES: [(i64, &str); 3] = [(1, "mSec"), (2, "Sec"), (3, "Min")];
pub const _OPENERS: [&str; 2] = ["SELECT", "WHILE"];
pub const _CLOSERS: [&str; 4] = ["ENDIF", "ENDSELECT", "NEXTSUBSCAN", "WEND"];

/// Port of checker/instr_measure.py::is_sequential
pub fn is_sequential(program: &Program, profile: &Profile, env: &Env) -> bool {
    mode::decide(&program.statements, profile, env) == "Sequential"
}

/// Port of checker/instr_measure.py::check_scan_delay_budget
pub fn check_scan_delay_budget(
    program: &Program,
    profile: &Profile,
    diags: &mut Diagnostics,
    env: &Env,
    zero: bool,
) {
    if profile.name != "cr6" && profile.name != "cr1000x" {
        return;
    }
    if is_sequential(program, profile, env) {
        return;
    }
    for b in scan_time::budgets(program, env, profile, zero) {
        if (b.interval == 0) != zero {
            continue;
        }
        for (stmt, amount, unit) in &b.sub_short {
            let unit_name = usize::try_from(*unit)
                .ok()
                .and_then(|i| scan_time::UNIT_NAMES.get(i).copied())
                .unwrap_or("");
            diags.error_basic(
                stmt.line(),
                &format!("SubScan interval too short for measurement by {amount} {unit_name}."),
                stmt.file(),
            );
        }
        if b.total.rem_euclid(1_i64 << 32) > b.interval {
            let amount = scan_time::amount(scan_time::i32(b.total - b.interval), b.msg_unit);
            let unit_name = usize::try_from(b.msg_unit)
                .ok()
                .and_then(|i| scan_time::UNIT_NAMES.get(i).copied())
                .unwrap_or("");
            diags.error_basic(
                b.end.line(),
                &format!("Scan interval too short for measurement by {amount} {unit_name}."),
                b.end.file(),
            );
        }
    }
}

struct CpiScan {
    ms: i64,
    times: OrderedMap<i64, i64>,
    short: Vec<(i64, i64)>,
}

/// Port of checker/instr_measure.py::check_cpi_time
pub fn check_cpi_time(program: &Program, profile: &Profile, diags: &mut Diagnostics, env: &Env) {
    if (profile.name != "cr6" && profile.name != "cr1000x") || program.statements.is_empty() {
        return;
    }

    let mut scans: Vec<CpiScan> = Vec::new();
    let mut order: Vec<i64> = Vec::new();
    let mut cur_scan: Option<usize> = None;
    let mut sub: Option<(i64, i64)> = None;
    let mut sub_start: OrderedMap<i64, i64> = OrderedMap::new();
    let sequential = is_sequential(program, profile, env);
    let float_names = longconv::float_names(&program.statements);
    let literal_names = longconv::literal_names(&program.statements);

    let mut depth: i64 = 0;
    let mut items: Vec<(StmtRef, bool)> = Vec::new();
    for stmt in &program.statements {
        let kw = match stmt.as_ref() {
            Stmt::Instruction(instr) => pystr::upper(&instr.keyword),
            _ => String::new(),
        };
        let text = stmt.text();
        let is_opener = matches!(stmt.as_ref(), Stmt::For(_) | Stmt::DoLoop(_))
            && match stmt.as_ref() {
                Stmt::DoLoop(d) => d.keyword == "DO",
                _ => true,
            }
            || matches!(stmt.as_ref(), Stmt::If(i) if i.inline.is_none())
            || _OPENERS.contains(&kw.as_str());
        if is_opener {
            items.push((stmt.clone(), depth > 0));
            depth += 1;
            continue;
        }
        let is_closer = (kw == "NEXT" && NEXT_SCAN_OR_SUBSCAN_RE.match_(text).is_none())
            || matches!(stmt.as_ref(), Stmt::DoLoop(d) if d.keyword == "LOOP")
            || (_CLOSERS.contains(&kw.as_str()) && kw != "NEXTSUBSCAN")
            || (kw == "END" && END_IF_OR_SELECT_RE.match_(text).is_some());
        if is_closer {
            depth = (depth - 1).max(0);
        }
        items.push((stmt.clone(), depth > 0));
        if let Stmt::If(if_stmt) = stmt.as_ref() {
            if let Some(inline) = &if_stmt.inline {
                for child in inline {
                    items.push((child.clone(), true));
                }
            }
            if let Some(inline_else) = &if_stmt.inline_else {
                for child in inline_else {
                    items.push((child.clone(), true));
                }
            }
        }
    }

    for (stmt, conditional) in items {
        let mut kw = match stmt.as_ref() {
            Stmt::Instruction(instr) => pystr::upper(&instr.keyword),
            _ => String::new(),
        };
        let text = stmt.text();
        if kw == "NEXT" && NEXT_SCAN_RE.match_(text).is_some() {
            kw = "NEXTSCAN".to_string();
        } else if kw == "NEXT" && NEXT_SUBSCAN_RE.match_(text).is_some() {
            kw = "NEXTSUBSCAN".to_string();
        }
        if (kw.starts_with("CDM_") && kw != "CDM_TCCOMP" && kw != "CDM_MUXSELECT")
            || kw == "CPIADDMODULE2"
        {
            return;
        }
        if let Stmt::Instruction(instr) = stmt.as_ref()
            && kw == "SCAN"
        {
            let Some(raw) = _scan_raw(instr, env) else {
                return;
            };
            let Some(factor) = delay_us_factor(raw.1) else {
                return;
            };
            let ms = pystr::floordiv_i64(raw.0 * factor, 1000).unwrap_or(0);
            scans.push(CpiScan {
                ms,
                times: OrderedMap::new(),
                short: Vec::new(),
            });
            cur_scan = Some(scans.len() - 1);
            continue;
        }
        let Some(scan_idx) = cur_scan else {
            continue;
        };
        if kw == "NEXTSCAN" {
            if sub.is_some() {
                return;
            }
            cur_scan = None;
        } else if let Stmt::Instruction(instr) = stmt.as_ref()
            && kw == "SUBSCAN"
        {
            let args: Vec<Option<ConstValue>> = instr
                .args
                .iter()
                .map(|a| _value(a.as_deref(), env))
                .collect();
            let count = if args.len() == 3 {
                match args[2] {
                    Some(ConstValue::Int(c)) => Some(c),
                    _ => None,
                }
            } else {
                None
            };
            let mut step = 0_i64;
            let Some(count_val) = count else {
                return;
            };
            if count_val < 1 || sub.is_some() {
                return;
            }
            let arg0_is_zero = matches!(args[0], Some(ConstValue::Int(0)))
                || matches!(
                    args[0],
                    Some(ConstValue::Float(f) | ConstValue::CFloat(f)) if f == 0.0
                );
            if !arg0_is_zero {
                let unit = match args[1] {
                    Some(ConstValue::Int(u)) => delay_us_factor(u),
                    _ => None,
                };
                let (Some(unit_factor), Some(a0)) = (unit, args[0].as_ref()) else {
                    return;
                };
                if !consteval::is_finite_number(a0) {
                    return;
                }
                let a0_f = match a0 {
                    ConstValue::Int(i) => *i as f64,
                    ConstValue::Float(f) | ConstValue::CFloat(f) => *f,
                    _ => return,
                };
                if a0_f <= 0.0 {
                    return;
                }
                let a0_i = pystr::py_int_from_f64(a0_f).unwrap_or(0);
                step = pystr::floordiv_i64(a0_i * unit_factor, 100).unwrap_or(0);
                if sequential && step <= 10 {
                    return;
                }
                if step * count_val > scans[scan_idx].ms * 10 {
                    return;
                }
            }
            sub = Some((count_val, step));
            sub_start = scans[scan_idx].times.clone();
        } else if kw == "NEXTSUBSCAN" {
            let Some((count_val, step)) = sub else {
                return;
            };
            let snapshot: Vec<(i64, i64)> = scans[scan_idx]
                .times
                .iter()
                .map(|(&addr, &units)| (addr, units))
                .collect();
            for (addr, units) in snapshot {
                let start_units = sub_start.get(&addr).copied().unwrap_or(0);
                let length = units - start_units;
                if length > 0 {
                    if step != 0 && length > step {
                        scans[scan_idx].short.push((addr, (length - step) * 100));
                    }
                    let add_step = if step != 0 { step } else { length };
                    scans[scan_idx]
                        .times
                        .insert(addr, units + add_step * (count_val - 1));
                }
            }
            sub = None;
        } else if let Stmt::Instruction(instr) = stmt.as_ref()
            && kw == "CDM_MUXSELECT"
        {
            if instr.args.len() != 7 || conditional || sub.is_some() {
                return;
            }
            let lv = |idx: usize| {
                longconv::long_value(
                    instr.args[idx].as_deref(),
                    env,
                    profile,
                    &float_names,
                    &literal_names,
                )
            };
            let (Some(module), Some(addr), Some(width), count, Some(reset)) =
                (lv(0), lv(1), lv(4), lv(5), lv(6))
            else {
                return;
            };
            if !matches!(module, 220..=223)
                || !(1..=120).contains(&addr)
                || !(1..=100).contains(&width)
            {
                return;
            }
            let clocks = match count {
                None => 32,
                Some(c) => _byte(c),
            };
            let reset_bit = i64::from(reset != 0);
            let micros = width * 1000 * (2 * clocks + 1 + reset_bit);
            let ticks = pystr::floordiv_i64(micros, 100).unwrap_or(0) + 1;
            if !order.contains(&addr) {
                order.push(addr);
            }
            let prev = scans[scan_idx].times.get(&addr).copied().unwrap_or(0);
            scans[scan_idx].times.insert(addr, prev + ticks);
        } else if let Stmt::Instruction(instr) = stmt.as_ref()
            && kw == "CDM_TCCOMP"
        {
            let iv = |idx: usize| {
                if idx < instr.args.len() {
                    _int_value(instr.args[idx].as_deref(), env)
                } else {
                    None
                }
            };
            let (Some(module), Some(addr), Some(reps)) = (iv(0), iv(1), iv(3)) else {
                return;
            };
            if module != 224 || !(1..=120).contains(&addr) || reps < 1 {
                return;
            }
            if conditional && !sequential {
                return;
            }
            if !order.contains(&addr) {
                order.push(addr);
            }
            let ticks = -pystr::floordiv_i64(-(reps * 430 + 275), 100).unwrap_or(0);
            let prev = scans[scan_idx].times.get(&addr).copied().unwrap_or(0);
            scans[scan_idx].times.insert(addr, prev + ticks);
        }
    }

    let last = program.statements.last().map_or(0, |s| s.line());
    let mut leader: HashMap<usize, (i64, i64)> = HashMap::new();
    for addr in order {
        for (i, sc) in scans.iter().enumerate() {
            if sc.times.is_empty() {
                continue;
            }
            for &(a, by) in &sc.short {
                if a == addr {
                    diags.error_basic(
                        last,
                        &format!("SubScan interval too short for measurements from CPI address {a} by {by} uSec."),
                        "",
                    );
                }
            }
            let t = sc.times.get(&addr).copied().unwrap_or(0);
            let need = -pystr::floordiv_i64(-t, 10).unwrap_or(0);
            if need > leader.get(&i).copied().unwrap_or((0, 0)).0 {
                leader.insert(i, (need, addr));
            }
        }
        for (i, sc) in scans.iter().enumerate() {
            if let Some(&(need_ms, leader_addr)) = leader.get(&i)
                && sc.ms < need_ms
            {
                diags.error_basic(
                    last,
                    &format!(
                        "Insufficient time ({} ms) for measurements from CPI address {} (needs at least {} ms).",
                        sc.ms, leader_addr, need_ms
                    ),
                    "",
                );
            }
        }
    }
}

/// Port of checker/instr_measure.py::_value
pub fn _value(expr: Option<&Expr>, env: &Env) -> Option<ConstValue> {
    let e = expr?;
    if !consteval::is_constant(Some(e), env) {
        return None;
    }
    consteval::value_of(Some(e), env).ok()
}

pub const _SINGLE_PASS_REPS: [&str; 15] = [
    "VOLTSE",
    "VOLTDIFF",
    "TCSE",
    "TCDIFF",
    "BRHALF",
    "BRHALF3W",
    "BRHALF4W",
    "BRFULL",
    "BRFULL6W",
    "THERM107",
    "THERM108",
    "THERM109",
    "CURRENTSE",
    "PERIODAVG",
    "PULSECOUNT",
];

pub const _NEGATIVE_REPS_SILENT: [&str; 4] = ["THERM107", "THERM108", "THERM109", "PERIODAVG"];

pub const _NEGATIVE_REPS_DEST: [&str; 5] = [
    "THERM107",
    "THERM108",
    "THERM109",
    "PERIODAVG",
    "PULSECOUNT",
];

pub const _REPS_CHECKED_CR6: [&str; 19] = [
    "VOLTSE",
    "VOLTDIFF",
    "TCDIFF",
    "TCSE",
    "BRHALF",
    "BRHALF3W",
    "BRHALF4W",
    "BRFULL",
    "BRFULL6W",
    "RESISTANCE",
    "RESISTANCE2W",
    "RESISTANCE3W",
    "PULSECOUNT",
    "CS616",
    "PERIODAVG",
    "CURRENTSE",
    "THERM107",
    "THERM108",
    "THERM109",
];

pub const _REPS_CHECKED_CR1000X: [&str; 16] = [
    "VOLTSE",
    "VOLTDIFF",
    "TCDIFF",
    "TCSE",
    "CURRENTSE",
    "PERIODAVG",
    "PULSECOUNT",
    "BRHALF",
    "BRHALF3W",
    "BRHALF4W",
    "BRFULL",
    "BRFULL6W",
    "CS616",
    "THERM107",
    "THERM108",
    "THERM109",
];

pub fn _reps_checked_for(profile_name: &str) -> &'static [&'static str] {
    match profile_name {
        "cr6" => &_REPS_CHECKED_CR6,
        "cr1000x" => &_REPS_CHECKED_CR1000X,
        "cr300" | "cr350" => &_SINGLE_PASS_REPS,
        _ => &[],
    }
}

pub const _REPS_DEST_SIZE: [&str; 16] = [
    "VOLTSE",
    "VOLTDIFF",
    "TCDIFF",
    "TCSE",
    "CURRENTSE",
    "PERIODAVG",
    "BRHALF",
    "BRHALF3W",
    "BRHALF4W",
    "BRFULL",
    "BRFULL6W",
    "RESISTANCE",
    "RESISTANCE2W",
    "RESISTANCE3W",
    "PULSECOUNT",
    "CS616",
];

/// Port of checker/instr_measure.py::_measurement_output_count
pub fn _measurement_output_count(stmt: &Instruction, reps: i64, env: &Env) -> i64 {
    let kw = pystr::upper(&stmt.keyword);
    let option_index = match kw.as_str() {
        "BRHALF4W" | "BRFULL6W" => Some(14),
        "RESISTANCE" => Some(13),
        "RESISTANCE2W" | "RESISTANCE3W" => Some(10),
        _ => None,
    };
    let Some(idx) = option_index else {
        return reps;
    };
    if stmt.args.len() <= idx {
        return reps;
    }
    if _int_value(stmt.args[idx].as_deref(), env) != Some(1) {
        return reps;
    }
    if matches!(kw.as_str(), "BRHALF4W" | "BRFULL6W") {
        reps * 2
    } else {
        reps + 1
    }
}

/// Alias destination entry: `(backing_elements, start_offset, diagnostic_name, declared_size)`.
pub type MeasurementAliasEntry = (i64, i64, String, Option<i64>);

/// Lookup handle returned by `_build_measurement_aliases`.
#[derive(Debug, Clone)]
pub struct MeasurementAliasLookup {
    program: ProgramRef,
    tables: HashMap<usize, ScopeTables>,
    aliases: HashMap<usize, OrderedMap<String, MeasurementAliasEntry>>,
    parameters: HashMap<usize, BTreeSet<String>>,
    stmt_to_routine: HashMap<usize, usize>,
    cache: RefCell<HashMap<usize, OrderedMap<String, MeasurementAliasEntry>>>,
}

impl MeasurementAliasLookup {
    pub fn lookup(
        &self,
        stmt: &StmtRef,
        dest: &Expr,
        allow_bare_index: bool,
    ) -> Option<MeasurementAliasEntry> {
        let (_, dest_upper) = expr_name_upper(dest)?;
        let scope = self.tables.get(&stmt_id(stmt))?;
        let routine_key = self
            .stmt_to_routine
            .get(&stmt_id(stmt))
            .copied()
            .unwrap_or(0);
        if scope.dims.contains_key(&dest_upper)
            || self
                .parameters
                .get(&routine_key)
                .is_some_and(|p| p.contains(&dest_upper))
        {
            return None;
        }
        if !self.cache.borrow().contains_key(&routine_key) {
            let mut visible = OrderedMap::new();
            let view = context::of(&self.program, None, None).scoped_view(stmt, false);
            for st in &view.statements {
                if let Some(entries) = self.aliases.get(&stmt_id(st)) {
                    for (k, v) in entries.iter() {
                        visible.insert(k.clone(), v.clone());
                    }
                }
            }
            self.cache.borrow_mut().insert(routine_key, visible);
        }
        let cache = self.cache.borrow();
        let entry = cache.get(&routine_key)?.get(&dest_upper)?.clone();
        if matches!(dest, Expr::Index(_)) && entry.3.is_none() && !allow_bare_index {
            return None;
        }
        Some(entry)
    }
}

/// Port of checker/instr_measure.py::_measurement_aliases
pub fn _measurement_aliases(
    program: &ProgramRef,
    tables: &ScopeLookup,
    env: &Env,
) -> MeasurementAliasLookup {
    context::of(program, None, Some(env.clone()))
        .measurement_aliases()
        .clone()
}

/// Port of checker/instr_measure.py::_build_measurement_aliases
pub fn _build_measurement_aliases(
    program: &ProgramRef,
    tables: &ScopeLookup,
    env: &Env,
) -> MeasurementAliasLookup {
    let ctx = context::of(program, None, Some(env.clone()));
    let mut aliases: HashMap<usize, OrderedMap<String, MeasurementAliasEntry>> = HashMap::new();
    let mut parameters: HashMap<usize, BTreeSet<String>> = HashMap::new();
    let mut stmt_to_routine: HashMap<usize, usize> = HashMap::new();
    let mut routine: Option<(usize, &SubDef)> = None;

    for st in &program.statements {
        if let Stmt::SubDef(sd) = st.as_ref() {
            let rid = stmt_id(st);
            routine = Some((rid, sd));
            let pset: BTreeSet<String> = sd.params.iter().map(|p| pystr::upper(&p.name)).collect();
            parameters.insert(rid, pset);
        } else if ends_routine(st) {
            routine = None;
        }
        let rid = routine.map_or(0, |(id, _)| id);
        stmt_to_routine.insert(stmt_id(st), rid);

        let Stmt::Instruction(instr) = st.as_ref() else {
            continue;
        };
        if pystr::upper(&instr.keyword) != "ALIAS" || instr.args.is_empty() {
            continue;
        }
        let Some(target) = instr.args[0].as_deref() else {
            continue;
        };
        let Some((target_name, target_upper)) = expr_name_upper(target) else {
            continue;
        };
        let scope = tables.for_statement(st);
        let mut shape = scope.dims.get(&target_upper).cloned();
        let param = routine.and_then(|(_, sd)| {
            sd.params
                .iter()
                .find(|p| pystr::upper(&p.name) == target_upper)
        });
        if let Some(p) = param
            && pystr::upper(p.type_.as_deref().unwrap_or("FLOAT")) == "FLOAT"
        {
            if p.dim_exprs.is_empty() {
                shape = Some(vec![1]);
            } else {
                let mut dims = Vec::with_capacity(p.dim_exprs.len());
                let mut ok = true;
                for d in &p.dim_exprs {
                    match _int_value(Some(d.as_ref()), env) {
                        Some(n) if n >= 1 => dims.push(n),
                        _ => {
                            ok = false;
                            break;
                        }
                    }
                }
                if !ok {
                    continue;
                }
                shape = Some(dims);
            }
        } else if shape.is_none()
            || scope.types.get(&target_upper).map(String::as_str) != Some("FLOAT")
        {
            continue;
        }
        let Some(shape) = shape else {
            continue;
        };

        let mut offset: i64 = if let Expr::Index(idx) = target {
            let mut indices = Vec::with_capacity(idx.args.len());
            let mut ok = true;
            for a in &idx.args {
                if let Some(i) = _int_value(a.as_deref(), env) {
                    indices.push(i);
                } else {
                    ok = false;
                    break;
                }
            }
            if !ok {
                continue;
            }
            if indices.len() == 1 {
                indices[0] - 1
            } else if indices.len() == shape.len()
                && indices
                    .iter()
                    .zip(shape.iter())
                    .all(|(&i, &n)| 1 <= i && i <= n)
            {
                let mut off = 0_i64;
                for (&i, &n) in indices.iter().zip(shape.iter()) {
                    off = off * n + i - 1;
                }
                off
            } else {
                continue;
            }
        } else {
            0
        };

        let total: i64 = shape.iter().product();
        let mut declared_names: OrderedMap<String, String> = OrderedMap::new();
        for decl_stmt in &ctx.scoped_view(st, false).statements {
            if let Stmt::Declaration(decl) = decl_stmt.as_ref() {
                for v in decl.vars.borrow().iter() {
                    declared_names.insert(pystr::upper(&v.name), v.name.clone());
                }
            }
        }

        let mut entries: OrderedMap<String, MeasurementAliasEntry> = OrderedMap::new();
        for alias_opt in &instr.args[1..] {
            let Some(alias) = alias_opt.as_deref() else {
                break;
            };
            let Some((alias_name, alias_upper)) = expr_name_upper(alias) else {
                break;
            };
            let (size, is_bare) = match alias {
                Expr::Name(_) => (Some(1_i64), true),
                Expr::Index(idx) => {
                    let s = if idx.args.len() == 1 {
                        _int_value(idx.args[0].as_deref(), env)
                    } else {
                        None
                    };
                    (s, false)
                }
                _ => (None, false),
            };
            let Some(size_val) = size else {
                break;
            };
            if size_val < 1 || offset < 0 || offset >= total {
                break;
            }
            let diag_name = if offset == 0 {
                declared_names
                    .get(&target_upper)
                    .cloned()
                    .unwrap_or_else(|| target_name.to_string())
            } else {
                alias_name.to_string()
            };
            entries.insert(
                alias_upper,
                (
                    total,
                    offset,
                    diag_name,
                    if is_bare { None } else { Some(size_val) },
                ),
            );
            offset += size_val;
        }
        aliases.insert(stmt_id(st), entries);
    }

    MeasurementAliasLookup {
        program: program.clone(),
        tables: program
            .statements
            .iter()
            .map(|st| (stmt_id(st), tables.for_statement(st)))
            .collect(),
        aliases,
        parameters,
        stmt_to_routine,
        cache: RefCell::new(HashMap::new()),
    }
}

/// Port of checker/instr_measure.py::_alias_overrun
pub fn _alias_overrun(
    _stmt: &Instruction,
    dest: &Expr,
    alias: &MeasurementAliasEntry,
    count: i64,
    profile: &Profile,
    env: &Env,
    signed_index: bool,
) -> bool {
    let (total, offset, _, size) = alias;
    let mut index = 0_i64;
    if let Expr::Index(idx) = dest {
        let mut k = if idx.args.len() == 1 {
            match _index_value(idx.args[0].as_deref(), Some(env)) {
                Some(v) => v,
                None => return false,
            }
        } else {
            return false;
        };
        if signed_index {
            k = k.abs();
        }
        let max_size = size.unwrap_or(1);
        if k < 1 || k > max_size {
            return true;
        }
        index = k - 1;
    }
    let available = *total - index - if profile.single_pass { 0 } else { *offset };
    count < 0 || count > available
}

/// Port of checker/instr_measure.py::check_reps
pub fn check_reps(program: &ProgramRef, profile: &Profile, diags: &mut Diagnostics, env: &Env) {
    let checked = _reps_checked_for(profile.name);
    if checked.is_empty() {
        return;
    }
    let ctx = context::of(program, None, Some(env.clone()));
    let tables = ctx.scope_tables(true, true);
    let alias_storage = _measurement_aliases(program, &tables, env);

    for stmt_ref in &program.statements {
        let Stmt::Instruction(stmt) = stmt_ref.as_ref() else {
            continue;
        };
        let kw = pystr::upper(&stmt.keyword);
        if !checked.contains(&kw.as_str()) || stmt.args.len() < 2 {
            continue;
        }
        let scope = tables.for_statement(stmt_ref);
        let arg1 = stmt.args[1].as_deref();
        let mut reps = match _const_value(arg1, env) {
            Some(ConstValue::Int(i)) => Some(i),
            Some(ConstValue::Float(f) | ConstValue::CFloat(f)) if f.is_finite() => {
                pystr::py_int_from_f64(f)
            }
            Some(ConstValue::BigInt(ref s)) => match s.parse::<i64>() {
                Ok(i) => Some(i),
                Err(_) => Some(if s.starts_with('-') {
                    -2_147_483_648
                } else {
                    2_147_483_648
                }),
            },
            Some(ConstValue::Str(_)) => Some(if profile.single_pass {
                0
            } else {
                -2_147_483_648
            }),
            Some(ConstValue::Float(_) | ConstValue::CFloat(_)) | None => None,
        };
        let Some(mut reps_val) = reps.take() else {
            continue;
        };
        if !profile.single_pass && !(-2_147_483_648..2_147_483_648).contains(&reps_val) {
            reps_val = -2_147_483_648;
        }
        if profile.single_pass {
            let mut messages: Vec<Option<String>> = Vec::new();
            if reps_val == 0 || (reps_val < 0 && !_NEGATIVE_REPS_SILENT.contains(&kw.as_str())) {
                messages.push(Some(format!("Invalid reps {reps_val}, must be > 0.")));
            }
            if reps_val > 0 || (reps_val < 0 && _NEGATIVE_REPS_DEST.contains(&kw.as_str())) {
                if let Some(dest) = stmt.args[0].as_deref() {
                    messages.push(_dest_overrun(
                        stmt,
                        stmt_ref,
                        dest,
                        reps_val,
                        profile,
                        env,
                        &alias_storage,
                        &scope,
                    ));
                }
            }
            let at = diags
                .items
                .iter()
                .position(|d| {
                    (d.line, d.file.as_str()) == (stmt.base.node.line, stmt.base.node.file.as_str())
                })
                .unwrap_or(diags.items.len());
            let inserted: Vec<_> = messages
                .into_iter()
                .flatten()
                .map(|m| {
                    Rc::new(Diagnostic::basic(
                        "error",
                        stmt.base.node.line,
                        &m,
                        &stmt.base.node.file,
                    ))
                })
                .collect();
            diags.items.splice(at..at, inserted);
            continue;
        }
        if reps_val <= 0 && kw != "CS616" {
            diags.error_basic(
                stmt.base.node.line,
                &format!("Invalid reps {reps_val}, must be > 0"),
                &stmt.base.node.file,
            );
        }
        let Some(dest) = stmt.args[0].as_deref() else {
            continue;
        };
        if reps_val == 0 || !matches!(dest, Expr::Name(_) | Expr::Index(_)) {
            continue;
        }
        if !_REPS_DEST_SIZE.contains(&kw.as_str()) {
            if let Some(alias) = alias_storage.lookup(stmt_ref, dest, false)
                && _alias_overrun(stmt, dest, &alias, 0, profile, env, false)
            {
                diags.error_basic(
                    stmt.base.node.line,
                    &format!("Variable {} out of bounds.", alias.2),
                    &stmt.base.node.file,
                );
            }
            continue;
        }
        if let Some(message) = _dest_overrun(
            stmt,
            stmt_ref,
            dest,
            reps_val,
            profile,
            env,
            &alias_storage,
            &scope,
        ) {
            diags.error_basic(stmt.base.node.line, &message, &stmt.base.node.file);
        }
        if reps_val > 1 && kw == "CURRENTSE" && profile.name == "cr6" {
            diags.error_basic(
                stmt.base.node.line,
                "Reps >1 not allowed in this device.",
                &stmt.base.node.file,
            );
        }
    }

    _check_bridge_burst(program, profile, diags, env);
    _check_bridge_span(program, profile, diags, env);
    _check_pulse_span(program, profile, diags, env);
}

pub const _BRIDGE_SPAN: [(&str, (usize, usize, usize, i64, i64)); 5] = [
    ("BRHALF", (3, 4, 5, 1, 1)),
    ("BRHALF3W", (3, 4, 5, 2, 1)),
    ("BRFULL", (3, 4, 5, 1, 2)),
    ("BRHALF4W", (4, 5, 6, 2, 2)),
    ("BRFULL6W", (4, 5, 6, 2, 2)),
];

/// Port of checker/instr_measure.py::_check_bridge_span
pub fn _check_bridge_span(
    program: &Program,
    profile: &Profile,
    diags: &mut Diagnostics,
    env: &Env,
) {
    if profile.name != "cr6" {
        return;
    }
    for stmt_ref in &program.statements {
        let Stmt::Instruction(stmt) = stmt_ref.as_ref() else {
            continue;
        };
        let kw = pystr::upper(&stmt.keyword);
        let Some(&(_, (cp, xp, mp, groups, stride))) = _BRIDGE_SPAN.iter().find(|&&(k, _)| k == kw)
        else {
            continue;
        };
        if stmt.args.len() <= mp {
            continue;
        }
        let (Some(reps), Some(chan), Some(ex), Some(meas_raw)) = (
            _int_value(stmt.args[1].as_deref(), env),
            _int_value(stmt.args[cp].as_deref(), env),
            _int_value(stmt.args[xp].as_deref(), env),
            _int_value(stmt.args[mp].as_deref(), env),
        ) else {
            continue;
        };
        if reps < 1
            || !(201..=212).contains(&chan)
            || !(201..=212).contains(&ex)
            || (stride == 2 && chan % 2 == 0)
        {
            continue;
        }
        let s = chan - 201;
        let e = ex - 201;
        let meas = meas_raw.max(1);
        let meas_last = s + (reps - 1) * groups * stride + (groups - 1) * stride;
        let ex_last = e + (-pystr::floordiv_i64(-reps, meas).unwrap_or(0)) - 1;
        let mut found: Vec<String> = Vec::new();
        if s < e && e <= meas_last {
            found.push("Measurement channels and excitation channels overlap.".to_string());
        }
        let mut past = ((e + 1)..=ex_last).find(|&c| c + 1 > 12);
        if past.is_none() && ex_last <= 11 && reps > 1 {
            let step = usize::try_from(stride).unwrap_or(1);
            past = ((s + stride)..=meas_last)
                .step_by(step)
                .find(|&c| c + 1 > 12);
        }
        if let Some(p) = past {
            found.push(format!(
                "Channel {} exceeds number of valid channels.",
                p + 1
            ));
        }
        _insert_in_line_order(diags, stmt, found);
    }
}

/// Port of checker/instr_measure.py::_insert_in_line_order
pub fn _insert_in_line_order(diags: &mut Diagnostics, stmt: &Instruction, texts: Vec<String>) {
    let at = diags
        .items
        .iter()
        .position(|d| {
            d.severity == "error" && d.file == stmt.base.node.file && d.line > stmt.base.node.line
        })
        .unwrap_or(diags.items.len());
    let inserted: Vec<_> = texts
        .into_iter()
        .map(|text| {
            Rc::new(Diagnostic::basic(
                "error",
                stmt.base.node.line,
                &text,
                &stmt.base.node.file,
            ))
        })
        .collect();
    diags.items.splice(at..at, inserted);
}

/// Port of checker/instr_measure.py::_pulse_index
pub fn _pulse_index(code: i64) -> Option<(i64, i64)> {
    let idx = if (101..=116).contains(&code) {
        Some(code - 101)
    } else if code >= 201 && code.rem_euclid(200) <= 12 {
        Some(code.rem_euclid(200) + 3)
    } else {
        None
    }?;
    Some((idx, if idx < 4 { 4 } else { 16 }))
}

/// Port of checker/instr_measure.py::_check_pulse_span
pub fn _check_pulse_span(program: &Program, profile: &Profile, diags: &mut Diagnostics, env: &Env) {
    if profile.name != "cr6" {
        return;
    }
    for stmt_ref in &program.statements {
        let Stmt::Instruction(stmt) = stmt_ref.as_ref() else {
            continue;
        };
        if pystr::upper(&stmt.keyword) != "PULSECOUNT" || stmt.args.len() < 3 {
            continue;
        }
        let reps = _int_value(stmt.args[1].as_deref(), env);
        let chan = _int_value(stmt.args[2].as_deref(), env);
        let port = chan.and_then(_pulse_index);
        let (Some(reps_val), Some(port_val)) = (reps, port) else {
            continue;
        };
        if reps_val.max(1) - 1 + port_val.0 > port_val.1 {
            _insert_in_line_order(diags, stmt, vec!["Invalid PulseCount reps.".to_string()]);
        }
    }
}

/// Port of checker/instr_measure.py::_dest_overrun
pub fn _dest_overrun(
    stmt: &Instruction,
    stmt_ref: &StmtRef,
    dest: &Expr,
    reps: i64,
    profile: &Profile,
    env: &Env,
    alias_storage: &MeasurementAliasLookup,
    scope: &ScopeTables,
) -> Option<String> {
    let (dest_name, dest_upper) = expr_name_upper(dest)?;
    let count = _measurement_output_count(stmt, reps, env);
    if let Some(alias) = alias_storage.lookup(stmt_ref, dest, false) {
        return if _alias_overrun(stmt, dest, &alias, count, profile, env, false) {
            Some(format!("Variable {} out of bounds.", alias.2))
        } else {
            None
        };
    }
    if scope.types.get(&dest_upper).map(String::as_str) != Some("FLOAT") {
        return None;
    }
    let shape = scope.dims.get(&dest_upper)?;
    let offset = _flat_offset(dest, shape, Some(env))?;
    let total: i64 = shape.iter().product();
    if count < 0 || count > total - offset {
        Some(format!("Variable {dest_name} out of bounds."))
    } else {
        None
    }
}

/// Port of checker/instr_measure.py::_check_bridge_burst
pub fn _check_bridge_burst(
    program: &Program,
    profile: &Profile,
    diags: &mut Diagnostics,
    env: &Env,
) {
    if profile.name != "cr6" && profile.name != "cr1000x" {
        return;
    }
    for stmt_ref in &program.statements {
        let Stmt::Instruction(stmt) = stmt_ref.as_ref() else {
            continue;
        };
        let kw = pystr::upper(&stmt.keyword);
        if (kw != "BRHALF4W" && kw != "BRFULL6W") || stmt.args.len() < 7 {
            continue;
        }
        let (Some(reps), Some(channel), Some(meas)) = (
            _int_value(stmt.args[1].as_deref(), env),
            _int_value(stmt.args[4].as_deref(), env),
            _int_value(stmt.args[6].as_deref(), env),
        ) else {
            continue;
        };
        if meas < 1 {
            continue;
        }
        let valid = if profile.name == "cr6" {
            (-211..=-201).contains(&channel) && channel % 2 != 0
        } else {
            (-8..=-1).contains(&channel)
        };
        if valid && meas != reps {
            diags.error_basic(
                stmt.base.node.line,
                "When channel < 0, measurements per excitation must equal reps.",
                &stmt.base.node.file,
            );
        }
    }
}

pub const _DEST_FIXED: [(&str, i64); 1] = [("REALTIME", 9)];
pub const _DEST_KINDS_REALTIME: [&str; 4] = ["FLOAT", "LONG", "DOUBLE", "BOOLEAN"];
pub const _DEST_KINDS_MOVE: [&str; 5] = ["FLOAT", "LONG", "DOUBLE", "BOOLEAN", "STRING"];
pub const _DEST_PER_REP: [(&str, i64); 8] = [
    ("CALFILE", 1),
    ("AVGRUN", 1),
    ("MOVE", 1),
    ("MINRUN", 1),
    ("MAXRUN", 1),
    ("TOTALRUN", 1),
    ("STDDEVRUN", 1),
    ("PRTCALC", 1),
];
pub const _DEST_NEGATIVE_OOB: [&str; 2] = ["CALFILE", "PRTCALC"];
pub const _RUNNING_STATS: [&str; 4] = ["MINRUN", "MAXRUN", "TOTALRUN", "STDDEVRUN"];

pub const _SDM_MODE_VALUES: [(&str, [i64; 8]); 3] = [
    ("CS7500", [2, 2, 4, 3, 11, 3, 4, 4]),
    ("LI7200", [2, 5, 10, 12, 10, 5, 1, 5]),
    ("LI7700", [3, 5, 9, 13, 17, 3, 7, 9]),
];
pub const _TGA_VALUES: [[i64; 3]; 5] = [
    [2, 3, 4],
    [5, 7, 9],
    [8, 10, 12],
    [19, 25, 31],
    [21, 27, 33],
];
pub const _EC100_VALUES: [i64; 5] = [8, 12, 13, 15, 36];
pub const _EC100_INVALID_COUNTS: [(i64, i64); 8] = [
    (4, 36),
    (8, 254),
    (15, 185),
    (127, 57),
    (128, 192),
    (158, 254),
    (254, 146),
    (255, 188),
];
pub const _BUILDER_FIXED: [(&str, &[(usize, i64)]); 5] = [
    ("CSAT3B", &[(0, 5)]),
    ("CSAT3BMONITOR", &[(0, 4)]),
    ("HYDRAPROBE", &[(0, 11), (1, 4)]),
    ("GOESGPS", &[(0, 6), (1, 7)]),
    ("RECTPOLAR", &[(0, 2), (1, 2)]),
];

/// Port of checker/instr_measure.py::_byte
pub fn _byte(value: i64) -> i64 {
    value & 0xFF
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profiles;
    use crate::syntax::{
        Declaration, ExprRef, Index, Name, Node, Number, StmtBase, StringLit, VarDecl,
    };

    fn num_i(v: i64) -> ExprRef {
        Rc::new(Expr::Number(Number {
            node: Node::new(1, ""),
            value: NumberValue::Int(v),
            text: v.to_string(),
        }))
    }

    fn name_e(s: &str) -> ExprRef {
        Rc::new(Expr::Name(Name {
            node: Node::new(1, ""),
            name: s.to_string(),
        }))
    }

    fn str_e(s: &str) -> ExprRef {
        Rc::new(Expr::String(StringLit {
            node: Node::new(1, ""),
            value: s.to_string(),
        }))
    }

    fn idx_e(s: &str, args: Vec<i64>) -> ExprRef {
        Rc::new(Expr::Index(Index {
            node: Node::new(1, ""),
            name: s.to_string(),
            args: args.into_iter().map(|a| Some(num_i(a))).collect(),
        }))
    }

    fn instr_val(line: i64, kw: &str, args: Vec<Option<ExprRef>>) -> Instruction {
        Instruction {
            base: StmtBase {
                node: Node::new(line, ""),
                ..Default::default()
            },
            keyword: kw.to_string(),
            args,
            ..Default::default()
        }
    }

    fn instr_stmt(line: i64, kw: &str, args: Vec<Option<ExprRef>>) -> StmtRef {
        Rc::new(Stmt::Instruction(instr_val(line, kw, args)))
    }

    fn dim_stmt(line: i64, name: &str, dims: Vec<i64>, ty: Option<&str>) -> StmtRef {
        Rc::new(Stmt::Declaration(Declaration {
            base: StmtBase {
                node: Node::new(line, ""),
                ..Default::default()
            },
            kind: "PUBLIC".to_string(),
            vars: vec![Rc::new(VarDecl {
                name: name.to_string(),
                dims: dims.into_iter().map(num_i).collect(),
                type_: ty.map(ToString::to_string),
                ..Default::default()
            })]
            .into(),
        }))
    }

    fn prog(statements: Vec<StmtRef>) -> ProgramRef {
        Rc::new(Program {
            statements,
            ..Default::default()
        })
    }

    #[test]
    fn delay_checks_options_limits_and_strings() {
        let cr6 = profiles::get("cr6").unwrap();
        let env = Env::new(cr6);
        let cfg = cr6.measure().get("delay").unwrap();
        let p = prog(vec![dim_stmt(1, "S", vec![], Some("String"))]);
        let mut typer = Typer::new(&p, None, None);
        let mut diags = Diagnostics::new();

        let bad_opt = instr_val(
            2,
            "Delay",
            vec![Some(num_i(-1)), Some(num_i(1)), Some(num_i(2))],
        );
        _check_delay(&bad_opt, cfg, &env, &mut diags, false, false, &mut typer);
        assert_eq!(diags.items.len(), 1);
        assert_eq!(diags.items[0].message, "Illegal option 255");

        diags.items.clear();
        let str_interval = instr_val(
            3,
            "Delay",
            vec![Some(num_i(0)), Some(name_e("S")), Some(num_i(2))],
        );
        _check_delay(
            &str_interval,
            cfg,
            &env,
            &mut diags,
            false,
            false,
            &mut typer,
        );
        assert_eq!(
            diags.items[0].message,
            "Delay interval cannot be of type String."
        );

        diags.items.clear();
        let too_long = instr_val(
            4,
            "Delay",
            vec![Some(num_i(0)), Some(num_i(61)), Some(num_i(3))],
        );
        _check_delay(&too_long, cfg, &env, &mut diags, false, false, &mut typer);
        assert_eq!(diags.items[0].message, "Measurement Delay Exceeds 60 min.");
    }

    #[test]
    fn cs616_and_excite_magnitude() {
        let cr6 = profiles::get("cr6").unwrap();
        let env = Env::new(cr6);
        let measure = cr6.measure();
        let mut diags = Diagnostics::new();

        let cs616 = instr_val(5, "CS616", vec![Some(name_e("X")), Some(num_i(0))]);
        _check_cs616_reps(&cs616, measure, &env, &mut diags);
        assert_eq!(diags.items.len(), 1);
        assert_eq!(diags.items[0].message, "Invalid reps 0, must be > 0");

        diags.items.clear();
        let excite = instr_val(6, "ExciteV", vec![Some(num_i(201)), Some(num_i(2501))]);
        _check_excite_magnitude(&excite, measure, &env, &mut diags);
        assert_eq!(diags.items.len(), 1);
        assert_eq!(diags.items[0].message, "Excitation exceeds 2500.000000.");
    }

    #[test]
    fn pulse_index_and_byte() {
        assert_eq!(_pulse_index(101), Some((0, 4)));
        assert_eq!(_pulse_index(105), Some((4, 16)));
        assert_eq!(_pulse_index(201), Some((4, 16)));
        assert_eq!(_pulse_index(412), Some((15, 16)));
        assert_eq!(_pulse_index(999), None);
        assert_eq!(_byte(-1), 255);
        assert_eq!(_byte(260), 4);
    }

    #[test]
    fn reps_and_alias_overrun() {
        let cr6 = profiles::get("cr6").unwrap();
        let env = Env::new(cr6);
        let p = prog(vec![
            dim_stmt(1, "Arr", vec![4], Some("Float")),
            instr_stmt(
                2,
                "Alias",
                vec![Some(idx_e("Arr", vec![2])), Some(idx_e("A", vec![3]))],
            ),
            instr_stmt(
                3,
                "VoltSe",
                vec![
                    Some(idx_e("A", vec![2])),
                    Some(num_i(3)),
                    Some(num_i(800)),
                    Some(num_i(201)),
                ],
            ),
            instr_stmt(
                4,
                "PulseCount",
                vec![Some(name_e("Arr")), Some(num_i(5)), Some(num_i(102))],
            ),
            instr_stmt(
                5,
                "BrHalf",
                vec![
                    Some(name_e("Arr")),
                    Some(num_i(3)),
                    Some(num_i(800)),
                    Some(num_i(201)),
                    Some(num_i(202)),
                    Some(num_i(1)),
                    Some(num_i(2500)),
                ],
            ),
        ]);
        let mut diags = Diagnostics::new();
        check_reps(&p, cr6, &mut diags, &env);
        let msgs: Vec<&str> = diags.items.iter().map(|d| d.message.as_str()).collect();
        assert!(msgs.contains(&"Variable A out of bounds."));
        assert!(msgs.contains(&"Invalid PulseCount reps."));
        assert!(msgs.contains(&"Measurement channels and excitation channels overlap."));
    }

    #[test]
    fn cpi_time_reports_short_scan() {
        let cr6 = profiles::get("cr6").unwrap();
        let env = Env::new(cr6);
        let p = prog(vec![
            instr_stmt(1, "Scan", vec![Some(num_i(1)), Some(num_i(1))]), // 1 mSec
            instr_stmt(
                2,
                "CDM_TCComp",
                vec![
                    Some(num_i(224)),
                    Some(num_i(1)),
                    Some(name_e("T")),
                    Some(num_i(5)),
                ],
            ),
            instr_stmt(3, "NextScan", vec![]),
        ]);
        let mut diags = Diagnostics::new();
        check_cpi_time(&p, cr6, &mut diags, &env);
        assert_eq!(diags.items.len(), 1);
        assert_eq!(
            diags.items[0].message,
            "Insufficient time (1 ms) for measurements from CPI address 1 (needs at least 3 ms)."
        );
    }

    #[test]
    fn scan_delay_budget_reports_short_scan() {
        let cr6 = profiles::get("cr6").unwrap();
        let env = Env::new(cr6);
        let p = prog(vec![
            instr_stmt(1, "Scan", vec![Some(num_i(50)), Some(num_i(1))]), // 50 mSec
            instr_stmt(
                2,
                "Delay",
                vec![Some(num_i(0)), Some(num_i(1)), Some(num_i(2))],
            ), // 1 Sec
            instr_stmt(3, "NextScan", vec![]),
        ]);
        let mut diags = Diagnostics::new();
        check_scan_delay_budget(&p, cr6, &mut diags, &env, false);
        assert_eq!(diags.items.len(), 1);
        assert_eq!(
            diags.items[0].message,
            "Scan interval too short for measurement by 951 mSec."
        );
    }

    #[test]
    fn check_reps_string_reps_single_pass() {
        let cr300 = profiles::get("cr300").unwrap();
        let env = Env::new(cr300);
        let p = prog(vec![
            dim_stmt(1, "T", vec![], Some("Float")),
            instr_stmt(2, "Therm107", vec![Some(name_e("T")), Some(str_e("fuzz"))]),
        ]);
        let mut diags = Diagnostics::new();
        check_reps(&p, cr300, &mut diags, &env);
        assert_eq!(diags.items.len(), 1);
        assert_eq!(diags.items[0].message, "Invalid reps 0, must be > 0.");
    }
}
