//! Port of `checker/instr_rules.py` (lines 766-1538).

#![allow(dead_code, unused_variables, unused_imports)]

use crate::checker::{context, var_types::Typer};
use crate::consteval::{self, Env, NotConstant};
use crate::diagnostics::ShowFile;
use crate::syntax::{ConstValue, Name, Node, NumberValue};
use crate::{pyre, pystr};
use std::collections::BTreeSet;
use std::rc::Rc;
use std::sync::LazyLock;

use super::part1::{
    _SNMP_FLOOR, _SNMP_OID_DEST, _SNMP_OID_LITERAL, _SNMP_SPACED_OID, _SNMP_TAIL, _SNMP_TYPES,
    _snmp_components,
};
use crate::diagnostics::{DiagRef, Diagnostic, Diagnostics};
use crate::profiles::Profile;
use crate::syntax::{
    Assignment, Call, Case, ConstDecl, Declaration, Expr, ExprRef, Index, Instruction, Juxtaposed,
    Member, Number, OrderedMap, Program, ProgramRef, Stmt, StmtRef, SubDef, Unparsed, VarDecl,
};

static SNMP_DECIMAL: LazyLock<pyre::Regex> = LazyLock::new(|| pyre::Regex::new(r"\d+\.\d+", 0));
static SNMP_DOTTED: LazyLock<pyre::Regex> = LazyLock::new(|| pyre::Regex::new(r"\d+(?:\.\d+)+", 0));
static SNMP_DIGITS: LazyLock<pyre::Regex> =
    LazyLock::new(|| pyre::Regex::new(r"[\d.]*\d[\d.]*", 0));
static SNMP_RANGE: LazyLock<pyre::Regex> =
    LazyLock::new(|| pyre::Regex::new(r"(?:RANGES|VALUES)\s", 0));
static SNMP_TYPE_RE: LazyLock<pyre::Regex> = LazyLock::new(|| {
    pyre::Regex::new(
        r"\s*SNMPVARIABLE\s*\(\s*(\w+)\s*,\s*[\d.]+\s*,\s*(\w+)",
        pyre::I,
    )
});

/// Port of checker/instr_rules.py::check_snmp_oid
pub fn check_snmp_oid(
    program: &Program,
    profile: &Profile,
    diags: &mut Diagnostics,
    env: Option<&Env>,
) {
    if !profile.instructions().contains("SNMPVARIABLE") {
        return;
    }
    let cr3xx = ["cr300", "cr350"].contains(&profile.name);
    let mut seen: Vec<Vec<Option<i64>>> = Vec::new();
    let mut typer = None;
    for stmt in &program.statements {
        let mut both = false;
        let mut greater = false;
        let mut key = None;
        let before = diags.items.len();
        match stmt.as_ref() {
            Stmt::Unparsed(u) => {
                let comps = _SNMP_OID_LITERAL
                    .match_(&u.base.text)
                    .and_then(|m| _snmp_components(m.group(1).unwrap_or("")));
                let Some(comps) = comps.filter(|c| c.len() >= 3) else {
                    continue;
                };
                key = Some(comps);
            }
            Stmt::Instruction(i)
                if pystr::upper(&i.keyword) == "SNMPVARIABLE" && i.args.len() >= 2 =>
            {
                match i.args[1].as_deref() {
                    Some(Expr::Number(n)) => {
                        let value = match &n.value {
                            NumberValue::Int(i) => Some(int_float(*i)),
                            NumberValue::Float(f) => Some(*f),
                            NumberValue::BigInt(s) => pystr::parse_float(s),
                        };
                        if pystr::isdigit(&n.text) && matches!(value, Some(0.0 | 1.0)) {
                            both = true;
                            key = Some(vec![value.and_then(pystr::py_int_from_f64)]);
                        } else if SNMP_DECIMAL.fullmatch(&n.text).is_some() {
                            key = _snmp_components(&n.text);
                        } else {
                            continue;
                        }
                    }
                    Some(Expr::Unary(u))
                        if u.op == "-"
                            && matches!(u.operand.as_deref(),Some(Expr::Number(n)) if n.text=="1") =>
                    {
                        both = true;
                    }
                    Some(Expr::String(a)) => {
                        if SNMP_DOTTED.fullmatch(&a.value).is_some() {
                            greater = true;
                        } else if SNMP_DIGITS.fullmatch(&a.value).is_none() {
                            both = true;
                        }
                    }
                    Some(Expr::Name(n))
                        if env.is_some_and(|env| {
                            !env.consts.borrow().contains_key(&pystr::upper(&n.name))
                        }) && !["TRUE", "FALSE", "NAN"]
                            .contains(&pystr::upper(&n.name).as_str()) =>
                    {
                        both = true;
                    }
                    _ => continue,
                }
            }
            _ => continue,
        }
        let low = key
            .as_ref()
            .filter(|k| !k.is_empty())
            .map(|k| k.iter().map(|c| c.unwrap_or(0)).collect::<Vec<_>>());
        let base = stmt.base();
        if both {
            diags.error_basic(base.node.line, "Illegal object id.", &base.node.file);
        }
        if both
            || greater
            || low.as_ref().is_some_and(|low| {
                low.as_slice() < _SNMP_FLOOR.as_slice()
                    || (cr3xx && low.as_slice() == _SNMP_FLOOR.as_slice())
            })
        {
            diags.error_basic(
                base.node.line,
                "SNMP OID must be greater than 1.3.6.1.2.1.1.7",
                &base.node.file,
            );
        }
        if let Some(key) = key {
            let mut declared = seen.iter().filter(|earlier| **earlier == key).count();
            if cr3xx {
                let mut a: Vec<_> = _SNMP_FLOOR.iter().copied().map(Some).collect();
                let mut b = a.clone();
                a.push(Some(0));
                b.push(None);
                declared += usize::from(key == a || key == b);
            } else {
                declared += usize::from(
                    key.len() == 8
                        && key.iter().take(7).copied().eq(_SNMP_FLOOR
                            .iter()
                            .take(7)
                            .copied()
                            .map(Some))
                        && key[7].is_some_and(|n| (1..=7).contains(&n)),
                );
            }
            for _ in 0..declared {
                diags.error_basic(
                    base.node.line,
                    "Illegal OID since already declared.",
                    &base.node.file,
                );
            }
            seen.push(key);
        }
        if diags.items.len() > before
            && let Stmt::Unparsed(u) = stmt.as_ref()
        {
            let typer =
                typer.get_or_insert_with(|| Typer::new(&Rc::new(program.clone()), None, None));
            if let Some(m) = SNMP_TYPE_RE.match_(&u.base.text) {
                let name = Expr::Name(Name {
                    node: Node::new(base.node.line, ""),
                    name: m.group(1).unwrap_or("").to_owned(),
                });
                if crate::checker::var_types::_snmp_mismatch(
                    typer,
                    Some(&name),
                    m.group(2).unwrap_or(""),
                ) {
                    diags.error_basic(base.node.line, "Miss match of types", &base.node.file);
                }
            }
        }
    }
}

/// Port of checker/instr_rules.py::check_modemcallback_baud
#[allow(
    clippy::single_match_else,
    reason = "Preserve Python literal-versus-folded branch"
)]
pub fn check_modemcallback_baud(
    stmt: &Instruction,
    program: &ProgramRef,
    profile: &'static Profile,
    diags: &mut Diagnostics,
    env: &Env,
) {
    if !profile.instructions().contains("MODEMCALLBACK") || stmt.args.len() != 9 {
        return;
    }
    if !matches!(stmt.args[1].as_deref(),Some(Expr::Name(n)) if pystr::upper(&n.name)=="COMSDC7") {
        return;
    }
    let mut arg = stmt.args[2].as_deref();
    let mut sign = 1.0;
    if let Some(Expr::Unary(u)) = arg
        && u.op == "-"
    {
        sign = -1.0;
        arg = u.operand.as_deref();
    }
    let raw = match arg {
        Some(Expr::Number(n)) => match &n.value {
            NumberValue::Int(i) => Some(int_float(*i)),
            NumberValue::Float(f) => Some(*f),
            NumberValue::BigInt(s) => pystr::parse_float(s),
        }
        .map(|v| sign * v),
        _ => {
            let Ok(value) = consteval::value_of(stmt.args[2].as_deref(), env) else {
                return;
            };
            if matches!(value, ConstValue::Str(_)) {
                diags.error_basic(
                    stmt.base.node.line,
                    "-2147483648 is illegal baud rate.",
                    &stmt.base.node.file,
                );
                return;
            }
            numeric(&value)
        }
    };
    let Some(raw) = raw else {
        return;
    };
    let baud = pystr::py_int_from_f64(raw)
        .filter(|n| (-2_147_483_648..2_147_483_648).contains(n))
        .unwrap_or(-2_147_483_648);
    if baud != 0 && !(600..=460_800).contains(&baud) {
        diags.error_basic(
            stmt.base.node.line,
            &format!("{baud} is illegal baud rate."),
            &stmt.base.node.file,
        );
    }
}

/// Port of `checker/instr_rules.py::check_snmpvariable_too_many`.
#[allow(clippy::if_not_else, reason = "Preserve Python branch order")]
pub fn check_snmpvariable_too_many(program: &Program, profile: &Profile, diags: &mut Diagnostics) {
    for stmt in &program.statements {
        let Stmt::Unparsed(stmt) = stmt.as_ref() else {
            continue;
        };
        if stmt.reason != "dotted SNMP OID literal"
            && !stmt.reason.starts_with("expected ')', found")
        {
            continue;
        }
        let text = &stmt.base.text;
        if let Some(m) = _SNMP_SPACED_OID.match_(text) {
            if let Some(components) = _snmp_components(m.group(1).unwrap_or(""))
                && components
                    .iter()
                    .map(|c| c.unwrap_or(0))
                    .collect::<Vec<_>>()
                    .as_slice()
                    < _SNMP_FLOOR.as_slice()
            {
                diags.error_basic(
                    stmt.base.node.line,
                    "SNMP OID must be greater than 1.3.6.1.2.1.1.7",
                    &stmt.base.node.file,
                );
            }
            let token = super::part1::_next_simple_token(pystr::slice(
                text,
                Some(i64::try_from(m.end0()).expect("source index fits i64")),
                None,
            ));
            if !token.is_empty() {
                diags.error_basic(
                    stmt.base.node.line,
                    &format!("Too many parameters:  {token}."),
                    &stmt.base.node.file,
                );
            }
            continue;
        }
        if let Some(m) = _SNMP_TAIL.match_(text) {
            let mut starts = Vec::new();
            let mut depth = 0;
            let mut quoted = false;
            let mut begin = m.end0();
            for (i, c) in text.chars().enumerate().skip(m.end0()) {
                if c == '"' {
                    quoted = !quoted;
                } else if !quoted {
                    depth += i32::from(c == '(') - i32::from(c == ')');
                    if depth < 0 {
                        break;
                    }
                    if c == ',' && depth == 0 {
                        starts.push(begin);
                        begin = i + 1;
                    }
                }
            }
            starts.push(begin);
            let words: Vec<_> = starts
                .iter()
                .enumerate()
                .map(|(i, b)| {
                    let end = starts
                        .get(i + 1)
                        .map(|b| i64::try_from(b - 1).expect("source index fits i64"));
                    pystr::upper(pystr::strip(pystr::slice(
                        text,
                        Some(i64::try_from(*b).expect("source index fits i64")),
                        end,
                    )))
                })
                .collect();
            let mut allowed = if !_SNMP_TYPES.contains(&words[0].as_str()) {
                1
            } else {
                2
            };
            if allowed == 2 && words.len() > 1 && words[1] == "RWRITE" {
                allowed = 3;
                if words.len() > 2 && SNMP_RANGE.match_(&words[2]).is_some() {
                    allowed = words.len();
                }
            }
            if starts.len() > allowed {
                let token = super::part1::_next_simple_token(pystr::slice(
                    text,
                    Some(i64::try_from(starts[allowed]).expect("source index fits i64")),
                    None,
                ));
                if !token.is_empty() {
                    diags.error_basic(
                        stmt.base.node.line,
                        &format!("Too many parameters:  {token}."),
                        &stmt.base.node.file,
                    );
                    continue;
                }
            }
        }
        if let Some(m) = _SNMP_OID_DEST.match_(text) {
            let oid = m.group(1).unwrap_or("");
            diags.error_basic(
                stmt.base.node.line,
                "Illegal object id.",
                &stmt.base.node.file,
            );
            diags.error_basic(
                stmt.base.node.line,
                "SNMP OID must be greater than 1.3.6.1.2.1.1.7",
                &stmt.base.node.file,
            );
            diags.error_basic(
                stmt.base.node.line,
                &format!("Variable name {oid} illegal, starts with a number."),
                &stmt.base.node.file,
            );
        }
    }
}

const _SCAN_VALUE_CHECK_LOGGERS: [&str; 2] = ["cr6", "cr1000x"];

/// Port of checker/instr_rules.py::_is_negative_or_invalid
pub fn _is_negative_or_invalid(value: &ConstValue) -> bool {
    if matches!(value, ConstValue::Str(_)) {
        return true;
    }
    if consteval::is_finite_number(value) {
        return numeric(value).is_some_and(|n| n.trunc() < 0.0 || n.trunc() > 2_147_483_647.0);
    }
    false
}

/// Port of checker/instr_rules.py::check_scan_params
pub fn check_scan_params(program: &Program, profile: &Profile, diags: &mut Diagnostics, env: &Env) {
    if !_SCAN_VALUE_CHECK_LOGGERS.contains(&profile.name) {
        return;
    }
    let mut slow = false;
    for stmt in &program.statements {
        let Stmt::Instruction(stmt) = stmt.as_ref() else {
            continue;
        };
        let kw = pystr::upper(&stmt.keyword);
        if kw == "SLOWSEQUENCE" {
            slow = true;
        }
        if kw != "SCAN" {
            continue;
        }
        let mut split = None;
        for (idx, label) in [(0, "Interval"), (2, "BufferOption"), (3, "Count")] {
            let Some(arg) = stmt.args.get(idx).and_then(Option::as_deref) else {
                continue;
            };
            if !consteval::is_constant(Some(arg), env) {
                continue;
            }
            let Ok(value) = consteval::value_of(Some(arg), env) else {
                continue;
            };
            if _is_negative_or_invalid(&value) {
                diags.error_basic(
                    stmt.base.node.line,
                    &format!("Scan {label} parameter must be a non-negative integer"),
                    &stmt.base.node.file,
                );
            }
            if idx == 0 {
                split = _scan_min_interval(stmt, diags, env);
                if let Some((sec, us)) = split
                    && sec > 86400
                    && !(sec == 0 && us < 1000)
                    && _crem(us, 1000) == 0
                {
                    diags.error_basic(
                        stmt.base.node.line,
                        "Maximum Scan interval is 1 Day.",
                        &stmt.base.node.file,
                    );
                }
            }
        }
        if slow && let Some(split) = split {
            _check_slow_scan_interval(stmt, split, diags);
        }
    }
}

/// Port of `checker/instr_rules.py::_check_slow_scan_interval`.
pub fn _check_slow_scan_interval(stmt: &Instruction, split: (i64, i64), diags: &mut Diagnostics) {
    let (seconds, microseconds) = split;
    let raw_us = seconds * 1_000_000 + microseconds;
    if -1_000_000 < raw_us && raw_us < 100_000 {
        diags.error_basic(
            stmt.base.node.line,
            "Slow Scan interval must be >= 100mSec.",
            &stmt.base.node.file,
        );
    }
    if pystr::pymod_i64(raw_us, 10_000) != Some(0) {
        diags.error_basic(
            stmt.base.node.line,
            "Slow Scan interval must be a multiple of 100mSec.",
            &stmt.base.node.file,
        );
    }
    if seconds > 86400 {
        diags.error_basic(
            stmt.base.node.line,
            "Maximum Slow Scan interval is 1 Day.",
            &stmt.base.node.file,
        );
    }
}

pub(super) static _US_PER_UNIT: LazyLock<OrderedMap<i64, i64>> = LazyLock::new(|| {
    let mut m = OrderedMap::new();
    for (k, v) in [
        (0, 1),
        (1, 1_000),
        (2, 1_000_000),
        (3, 60_000_000),
        (4, 3_600_000_000),
        (5, 86_400_000_000),
    ] {
        m.insert(k, v);
    }
    m
});

/// Port of checker/instr_rules.py::_scan_min_interval
pub fn _scan_min_interval(
    stmt: &Instruction,
    diags: &mut Diagnostics,
    env: &Env,
) -> Option<(i64, i64)> {
    let split = scan_split(stmt, env)?;
    let (sec, us) = split;
    if sec == 0 && us < 1000 {
        diags.error_basic(
            stmt.base.node.line,
            &format!("Scan {us}uSec invalid must be at least 1mSec."),
            &stmt.base.node.file,
        );
    } else if _crem(us, 1000) != 0 {
        diags.error_basic(
            stmt.base.node.line,
            "Scans must be a multiple of 1mSec.",
            &stmt.base.node.file,
        );
    }
    Some(split)
}

/// Port of `checker/instr_rules.py::_crem`.
pub fn _crem(a: i64, b: i64) -> i64 {
    a - b * _cdiv(a, b)
}

/// Port of checker/instr_rules.py::scan_split
pub fn scan_split(stmt: &Instruction, env: &Env) -> Option<(i64, i64)> {
    let a = stmt.args.first()?.as_deref()?;
    let b = stmt.args.get(1)?.as_deref()?;
    if !(consteval::is_constant(Some(a), env) && consteval::is_constant(Some(b), env)) {
        return None;
    }
    let value = consteval::value_of(Some(a), env).ok()?;
    let unit = consteval::value_of(Some(b), env).ok()?;
    let mut n = consteval::to_long(&value);
    if n == -2_147_483_648 && ["cr300", "cr350"].contains(&env.profile.name) && _f32_is_2_31(&value)
    {
        n = 2_147_483_647;
    }
    if !consteval::is_finite_number(&unit) {
        return Some((0, 0));
    }
    let Some(unit) = numeric(&unit)
        .and_then(pystr::py_int_from_f64)
        .filter(|u| _US_PER_UNIT.get(u).is_some())
    else {
        return Some((0, 0));
    };
    if unit == 0 {
        let sec = _cdiv(n, 1_000_000);
        return Some((sec, n - sec * 1_000_000));
    }
    if unit == 1 {
        let sec = _cdiv(n, 1000);
        return Some((sec, (n - sec * 1000) * 1000));
    }
    if unit == 2 {
        return Some((n, 0));
    }
    let wrapped = (n * match unit {
        3 => 60,
        4 => 3600,
        _ => 86400,
    }) & 0xFFFF_FFFF;
    Some((
        if wrapped >= 2_147_483_648 {
            wrapped - 4_294_967_296
        } else {
            wrapped
        },
        0,
    ))
}

/// Port of checker/instr_rules.py::_f32_is_2_31
#[allow(
    clippy::cast_possible_truncation,
    reason = "Python struct.pack f narrows to IEEE float32"
)]
#[allow(clippy::float_cmp, reason = "Python exact float32 boundary comparison")]
pub fn _f32_is_2_31(value: &ConstValue) -> bool {
    numeric(value).is_some_and(|v| f64::from(v as f32) == 2_147_483_648.0)
}

/// Port of `checker/instr_rules.py::_cdiv`.
pub fn _cdiv(a: i64, b: i64) -> i64 {
    let (q, r) = pystr::divmod_i64(a, b).expect("nonzero divisor and representable quotient");
    if r != 0 && (a < 0) != (b < 0) {
        q + 1
    } else {
        q
    }
}

/// Port of checker/instr_rules.py::check_scan_max_interval
pub fn check_scan_max_interval(
    program: &Program,
    profile: &Profile,
    diags: &mut Diagnostics,
    env: &Env,
) {
    if _SCAN_VALUE_CHECK_LOGGERS.contains(&profile.name) {
        return;
    }
    let mut slow = false;
    for stmt in &program.statements {
        let Stmt::Instruction(stmt) = stmt.as_ref() else {
            continue;
        };
        let kw = pystr::upper(&stmt.keyword);
        if kw == "SLOWSEQUENCE" {
            slow = true;
        }
        if kw != "SCAN" {
            continue;
        }
        let Some(split) = scan_split(stmt, env) else {
            continue;
        };
        let (sec, us) = split;
        if (sec != 0 || us >= 10000) && _crem(us, 10000) != 0 {
            diags.error_basic(
                stmt.base.node.line,
                "Scans > 10mSec must be a multiple of 10mSec.",
                &stmt.base.node.file,
            );
        } else if (sec != 0 || us >= 10000) && sec > 86400 {
            diags.error_basic(
                stmt.base.node.line,
                "Maximum Scan interval is 1 Day.",
                &stmt.base.node.file,
            );
        }
        if slow {
            _check_slow_scan_interval(stmt, split, diags);
        }
    }
}

/// Port of checker/instr_rules.py::_scan_raw
pub fn _scan_raw(stmt: &Instruction, env: &Env) -> Option<(i64, i64)> {
    let a = stmt.args.first()?.as_deref()?;
    let b = stmt.args.get(1)?.as_deref()?;
    if !(consteval::is_constant(Some(a), env) && consteval::is_constant(Some(b), env)) {
        return None;
    }
    let value = consteval::value_of(Some(a), env).ok()?;
    let unit = consteval::value_of(Some(b), env).ok()?;
    if !consteval::is_finite_number(&value) {
        return None;
    }
    let n = numeric(&value).and_then(pystr::py_int_from_f64)?;
    let u = unit_key(&unit)?;
    if !(0..=2_147_483_647).contains(&n) || _US_PER_UNIT.get(&u).is_none() {
        return None;
    }
    Some((n, u))
}

/// Port of checker/instr_rules.py::_units_key
pub fn _units_key(
    units_arg: &Expr,
    env: &Env,
    truncate: bool,
) -> Result<Option<ConstValue>, NotConstant> {
    if truncate {
        return Ok(crate::tables::_unit_value_with(Some(units_arg), env).map(ConstValue::Int));
    }
    consteval::value_of(Some(units_arg), env).map(Some)
}

/// Port of checker/instr_rules.py::_interval_us
pub fn _interval_us(
    interval_arg: Option<&Expr>,
    units_arg: Option<&Expr>,
    env: &Env,
    truncate_units: bool,
) -> Option<f64> {
    let (a, b) = (interval_arg?, units_arg?);
    if !(consteval::is_constant(Some(a), env) && consteval::is_constant(Some(b), env)) {
        return None;
    }
    let value = consteval::value_of(Some(a), env).ok()?;
    let unit = _units_key(b, env, truncate_units).ok()??;
    let factor = _US_PER_UNIT.get(&unit_key(&unit)?)?;
    if !consteval::is_finite_number(&value) {
        return None;
    }
    let value = numeric(&value)?;
    if value < 0.0 {
        return None;
    }
    Some(value * int_float(*factor))
}

/// Port of checker/instr_rules.py::_divisible_interval_us
pub fn _divisible_interval_us(
    interval_arg: Option<&Expr>,
    units_arg: Option<&Expr>,
    env: &Env,
    truncate_units: bool,
) -> Option<f64> {
    _interval_us(interval_arg, units_arg, env, truncate_units)?;
    let units_arg = units_arg?;
    let value = consteval::value_of(interval_arg, env).ok()?;
    let value = numeric(&value)?;
    let unit = _units_key(units_arg, env, truncate_units).ok()??;
    Some(value.trunc() * int_float(*_US_PER_UNIT.get(&unit_key(&unit)?)?))
}

/// Port of checker/instr_rules.py::check_data_interval
pub fn check_data_interval(
    program: &Program,
    profile: &Profile,
    diags: &mut Diagnostics,
    env: &Env,
) {
    fn table_name(stmt: &Instruction, program: &Program) -> Option<String> {
        let arg = stmt.args.first()?.as_deref()?;
        if !matches!(arg, Expr::Name(_) | Expr::String(_)) {
            return None;
        }
        let mut name = crate::checker::instr_data::_declared_table_name(stmt)?;
        if !name.is_empty() && matches!(arg, Expr::Name(_)) {
            name = crate::checker::instr_data::_const_table_name(&name, program);
        }
        (!name.is_empty()).then(|| pystr::upper(&name))
    }
    let mut declared_interval_us = OrderedMap::new();
    let mut current_table = None;
    for stmt in &program.statements {
        let Stmt::Instruction(stmt) = stmt.as_ref() else {
            continue;
        };
        let kw = pystr::upper(&stmt.keyword);
        if kw == "DATATABLE" {
            current_table = table_name(stmt, program);
            continue;
        }
        if kw == "ENDTABLE" {
            current_table = None;
            continue;
        }
        if kw != "DATAINTERVAL" {
            continue;
        }
        let Some(table) = current_table.as_ref() else {
            continue;
        };
        let tinto = stmt.args.first().and_then(Option::as_deref);
        let interval = stmt.args.get(1).and_then(Option::as_deref);
        let units = stmt.args.get(2).and_then(Option::as_deref);
        if _SCAN_VALUE_CHECK_LOGGERS.contains(&profile.name) {
            for arg in [tinto, interval] {
                if arg.is_none() || !consteval::is_constant(arg, env) {
                    continue;
                }
                let Ok(value) = consteval::value_of(arg, env) else {
                    continue;
                };
                if _is_negative_or_invalid(&value) {
                    diags.error(
                        stmt.base.node.line,
                        "DataInterval TintoInt and Interval parameters must be non-negative",
                        &stmt.base.node.file,
                        ShowFile::Bool(true),
                        0,
                        "datainterval_non_negative",
                        "",
                    );
                    break;
                }
            }
        }
        if let Some(us) = _divisible_interval_us(interval, units, env, true) {
            declared_interval_us.insert(table.clone(), us);
        }
    }
    let mut scan_interval_us = None;
    for stmt in &program.statements {
        let Stmt::Instruction(stmt) = stmt.as_ref() else {
            continue;
        };
        let kw = pystr::upper(&stmt.keyword);
        if kw == "SCAN" {
            scan_interval_us = if _scan_raw(stmt, env).is_some() {
                _divisible_interval_us(
                    stmt.args.first().and_then(Option::as_deref),
                    stmt.args.get(1).and_then(Option::as_deref),
                    env,
                    false,
                )
            } else {
                None
            };
            if scan_split(stmt, env) == Some((0, 0)) {
                scan_interval_us = None;
            }
            continue;
        }
        if kw == "NEXTSCAN" {
            scan_interval_us = None;
            continue;
        }
        if kw != "CALLTABLE" || scan_interval_us.is_none_or(|v| v == 0.0) || stmt.args.is_empty() {
            continue;
        }
        let Some(name) = table_name(stmt, program) else {
            continue;
        };
        let Some(entry) = declared_interval_us.get(&name) else {
            continue;
        };
        if let Some(message) = pystr::py_int_from_f64(*entry)
            .zip(scan_interval_us.and_then(pystr::py_int_from_f64))
            .and_then(|(dt, scan)| _divisibility_message(dt, scan))
        {
            diags.error_basic(stmt.base.node.line, &message, &stmt.base.node.file);
        }
    }
}

/// Port of `checker/instr_rules.py::_divisibility_message`.
pub fn _divisibility_message(dt_us: i64, scan_us: i64) -> Option<String> {
    let (dt_sec, dt_sub) = pystr::divmod_i64(dt_us, 1_000_000)?;
    let (scan_sec, scan_sub) = pystr::divmod_i64(scan_us, 1_000_000)?;
    let scan_units = scan_sec * 10000 + pystr::floordiv_i64(scan_sub, 100)?;
    if scan_units == 0 {
        return None;
    }
    if dt_sec < 3601 {
        let dt_units = dt_sec * 10000 + pystr::floordiv_i64(dt_sub, 100)?;
        if dt_units > 0 && pystr::pymod_i64(dt_units, scan_units)? != 0 {
            return Some(format!(
                "DataTable interval {dt_units}00 uSec not integrally divisible by scan interval {scan_units}00 uSec."
            ));
        }
    } else if scan_sub == 0 {
        if pystr::pymod_i64(dt_sec, scan_sec)? != 0 {
            return Some(format!(
                "DataTable interval {dt_sec} Sec not integrally divisible by scan interval {scan_sec} Sec."
            ));
        }
    } else if scan_sec != 0 || pystr::pymod_i64(10000, scan_units)? != 0 {
        return Some("DataTable with interval > 1 hour cannot be called by scan whose interval is not an integral second is not divisible into a second.".to_owned());
    }
    None
}

pub static _PRECISION_VARIABLE_POSITIONS: LazyLock<OrderedMap<&'static str, BTreeSet<i64>>> =
    LazyLock::new(|| {
        let mut m = OrderedMap::new();
        m.insert("ADDPRECISE", BTreeSet::from([0]));
        m.insert("MOVEPRECISE", BTreeSet::from([0]));
        m
    });

/// Port of `checker/instr_rules.py::_has_call`.
pub fn _has_call(expr: Option<&Expr>, variable_names: &BTreeSet<String>) -> bool {
    match expr {
        Some(Expr::Member(_)) => true,
        Some(Expr::Index(ix)) => {
            !variable_names.contains(&pystr::upper(&ix.name))
                || ix
                    .args
                    .iter()
                    .any(|a| _has_call(a.as_deref(), variable_names))
        }
        Some(Expr::Unary(u)) => _has_call(u.operand.as_deref(), variable_names),
        Some(Expr::Binary(b)) => {
            _has_call(b.left.as_deref(), variable_names)
                || _has_call(b.right.as_deref(), variable_names)
        }
        _ => false,
    }
}

/// Port of `checker/instr_rules.py::_stringish`.
pub fn _stringish(expr: Option<&Expr>, str_consts: &BTreeSet<String>) -> bool {
    match expr {
        Some(Expr::String(_)) => true,
        Some(Expr::Name(n)) => str_consts.contains(&pystr::upper(&n.name)),
        Some(Expr::Binary(b)) if ["&", "+"].contains(&b.op.as_str()) => {
            _stringish(b.left.as_deref(), str_consts) || _stringish(b.right.as_deref(), str_consts)
        }
        _ => false,
    }
}

/// Port of checker/instr_rules.py::_string_typed
#[allow(
    clippy::match_same_arms,
    reason = "Preserve Python expression-kind branch order"
)]
pub fn _string_typed(
    expr: Option<&Expr>,
    typer: &Typer,
    str_consts: &BTreeSet<String>,
    profile: &Profile,
) -> Option<bool> {
    match expr {
        Some(Expr::String(_)) => Some(true),
        Some(Expr::Number(_)) => Some(false),
        Some(Expr::Name(_) | Expr::Index(_)) => {
            let name = match expr? {
                Expr::Name(n) => pystr::upper(&n.name),
                Expr::Index(ix) => pystr::upper(&ix.name),
                _ => unreachable!(),
            };
            if matches!(expr, Some(Expr::Name(_))) && str_consts.contains(&name) {
                return Some(true);
            }
            if typer.variable(expr).is_some() {
                return Some(typer.of(expr, 0).as_deref() == Some("STRING"));
            }
            let record = profile.functions().get(&name);
            if name == "IIF"
                && let Some(Expr::Index(ix)) = expr
                && ix.args.len() == 3
            {
                let a = _string_typed(ix.args[1].as_deref(), typer, str_consts, profile);
                let b = _string_typed(ix.args[2].as_deref(), typer, str_consts, profile);
                return if a == b { a } else { None };
            }
            if let Some(record) = record
                && matches!(expr, Some(Expr::Index(_)))
            {
                return Some(record.kind == 312);
            }
            if matches!(expr, Some(Expr::Name(_)))
                && (["TRUE", "FALSE"].contains(&name.as_str()) || profile.symbols().contains(&name))
            {
                Some(false)
            } else {
                None
            }
        }
        Some(Expr::Unary(_)) => Some(false),
        Some(Expr::Binary(b)) => {
            if !["+", "&"].contains(&b.op.as_str()) {
                return Some(false);
            }
            let a = _string_typed(b.left.as_deref(), typer, str_consts, profile);
            let c = _string_typed(b.right.as_deref(), typer, str_consts, profile);
            if a == Some(true) || c == Some(true) {
                Some(true)
            } else if a.is_none() || c.is_none() {
                None
            } else {
                Some(false)
            }
        }
        _ => None,
    }
}

/// Port of checker/instr_rules.py::check_iif_types
pub fn check_iif_types(program: &ProgramRef, profile: &'static Profile, diags: &mut Diagnostics) {
    use crate::checker::expressions::{
        part1::{_undefined_names_with, KnownView},
        part2::_stmt_exprs_with,
        part5::_index_nodes_with,
    };
    if !profile.functions().contains("IIF") {
        return;
    }
    let typer = Typer::new(program, None, None);
    let ctx = context::of(program, Some(profile), None);
    let known = ctx.known_names();
    let view = KnownView {
        names: &known.names,
        tables: &known.tables,
        out_of_scope: &known.out_of_scope,
        dotted_tables: known.dotted_tables,
    };
    let str_consts = ctx.string_consts();
    let mut busy: BTreeSet<_> = diags
        .items
        .iter()
        .filter(|d| d.severity == "error")
        .map(|d| (d.line, d.file.clone()))
        .collect();
    for stmt in &program.statements {
        let base = stmt.base();
        let location = (base.node.line, base.node.file.clone());
        if busy.contains(&location) {
            continue;
        }
        let mut done = false;
        for e in _stmt_exprs_with(stmt) {
            for ix in _index_nodes_with(e.as_deref()) {
                let expr = Expr::Index(ix.clone());
                if pystr::upper(&ix.name) != "IIF"
                    || ![2, 3].contains(&ix.args.len())
                    || typer.variable(Some(&expr)).is_some()
                {
                    continue;
                }
                let mut arms: Vec<_> = ix
                    .args
                    .iter()
                    .skip(1)
                    .map(|a| _string_typed(a.as_deref(), &typer, str_consts, profile))
                    .collect();
                if let Stmt::Assignment(a) = stmt.as_ref()
                    && ix.args.len() == 3
                    && ix.args.iter().any(Option::is_none)
                    && _undefined_names_with(a.target.as_deref(), &view).is_empty()
                    && _undefined_names_with(a.value.as_deref(), &view).is_empty()
                {
                    if ix.args[0].is_some() && arms[0] == Some(true) && ix.args[2].is_none() {
                        diags.error_basic(base.node.line, "type mismatch", &base.node.file);
                    }
                    diags.error_basic(
                        base.node.line,
                        &crate::checker::structure::_out_of_place_with(
                            profile,
                            &base.text,
                            base.at_eol.get(),
                        ),
                        &base.node.file,
                    );
                    busy.insert(location.clone());
                    done = true;
                    break;
                }
                if ix.args.len() == 2 {
                    arms.push(Some(false));
                }
                if !arms.contains(&None) && arms[0] != arms[1] {
                    diags.error_basic(base.node.line, "type mismatch", &base.node.file);
                    busy.insert(location.clone());
                    done = true;
                    break;
                }
            }
            if done {
                break;
            }
        }
    }
}

#[allow(clippy::cast_precision_loss, reason = "Python int to float")]
fn int_float(n: i64) -> f64 {
    n as f64
}
fn numeric(v: &ConstValue) -> Option<f64> {
    match v {
        ConstValue::Int(n) => Some(int_float(*n)),
        ConstValue::Float(f) | ConstValue::CFloat(f) => Some(*f),
        ConstValue::BigInt(s) => pystr::parse_float(s),
        ConstValue::Str(_) => None,
    }
}
#[allow(clippy::float_cmp, reason = "Python numeric dictionary-key equality")]
fn unit_key(v: &ConstValue) -> Option<i64> {
    let n = numeric(v)?;
    let i = pystr::py_int_from_f64(n)?;
    (int_float(i) == n).then_some(i)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn python_observed_rule_matrix() {
        // Recorded by the real Python implementation, read-only, 2026-11-15.
        let rows: serde_json::Value = serde_json::from_str(r#"[["cr6","scan","Scan(0,Sec,3,0)",[0,0],[[1,"Scan 0uSec invalid must be at least 1mSec.","scan_invalid"]]],["cr6","scan","Scan(-0.5,Sec,3,0)",[0,0],[[1,"Scan 0uSec invalid must be at least 1mSec.","scan_invalid"]]],["cr6","scan","Scan(-1,mSec,-1,2147483648)",[0,-1000],[[1,"Scan Interval parameter must be a non-negative integer","scan_param"],[1,"Scan -1000uSec invalid must be at least 1mSec.","scan_invalid"],[1,"Scan BufferOption parameter must be a non-negative integer","scan_param"],[1,"Scan Count parameter must be a non-negative integer","scan_param"]]],["cr6","scan","Scan(1500,uSec,3,0)",[0,1500],[[1,"Scans must be a multiple of 1mSec.",""]]],["cr6","scan","Scan(15,mSec,3,0)",[0,15000],[]],["cr6","scan","Scan(9500,uSec,3,0)",[0,9500],[[1,"Scans must be a multiple of 1mSec.",""]]],["cr6","scan","Scan(86401,Sec,3,0)",[86401,0],[[1,"Maximum Scan interval is 1 Day.",""]]],["cr6","scan","Scan(2147483648,Sec,3,0)",[-2147483648,0],[[1,"Scan Interval parameter must be a non-negative integer","scan_param"]]],["cr6","scan","Scan(2147483647,Min,3,0)",[-60,0],[]],["cr6","scan","Scan(\"fuzz\",Sec,3,0)",[-2147483648,0],[[1,"Scan Interval parameter must be a non-negative integer","scan_param"]]],["cr6","scan","Scan(1,1.9,3,0)",[0,1000],[]],["cr6","scan","Scan(1,9,3,0)",[0,0],[[1,"Scan 0uSec invalid must be at least 1mSec.","scan_invalid"]]],["cr6","scan","SlowSequence\nScan(99,mSec,3,0)",[0,99000],[[2,"Slow Scan interval must be >= 100mSec.","slow_scan_interval"],[2,"Slow Scan interval must be a multiple of 100mSec.","slow_scan_interval"]]],["cr6","scan","SlowSequence\nScan(95,mSec,3,0)",[0,95000],[[2,"Slow Scan interval must be >= 100mSec.","slow_scan_interval"],[2,"Slow Scan interval must be a multiple of 100mSec.","slow_scan_interval"]]],["cr6","scan","SlowSequence\nScan(100,mSec,3,0)",[0,100000],[]],["cr6","scan","SlowSequence\nScan(86401,Sec,3,0)",[86401,0],[[2,"Maximum Scan interval is 1 Day.",""],[2,"Maximum Slow Scan interval is 1 Day.",""]]],["cr6","modem","ModemCallBack(R,COMSDC7,599,0,0,0,0,0,0)",null,[[1,"599 is illegal baud rate.",""]]],["cr6","modem","ModemCallBack(R,COMSDC7,600,0,0,0,0,0,0)",null,[]],["cr6","modem","ModemCallBack(R,COMSDC7,460800,0,0,0,0,0,0)",null,[]],["cr6","modem","ModemCallBack(R,COMSDC7,460801,0,0,0,0,0,0)",null,[[1,"460801 is illegal baud rate.",""]]],["cr6","modem","ModemCallBack(R,COMSDC7,0.5,0,0,0,0,0,0)",null,[]],["cr6","modem","ModemCallBack(R,COMSDC7,2147483648,0,0,0,0,0,0)",null,[[1,"-2147483648 is illegal baud rate.",""]]],["cr6","modem","ModemCallBack(R,COMSDC7,\"x\",0,0,0,0,0,0)",null,[[1,"-2147483648 is illegal baud rate.",""]]],["cr6","modem","ModemCallBack(R,COMSDC7,Hr+1,0,0,0,0,0,0)",null,[[1,"5 is illegal baud rate.",""]]],["cr6","modem","ModemCallBack(R,COMSDC7,uSec,0,0,0,0,0,0)",null,[]],["cr6","iif","Public x\nBeginProg\nx=IIF(1,\"a\",2)",null,[[3,"type mismatch",""]]],["cr6","iif","Public x\nBeginProg\nx=IIF(1,\"a\",\"b\")",null,[]],["cr6","iif","Public x\nBeginProg\nx=IIF(1,\"a\")",null,[[3,"type mismatch",""]]],["cr6","iif","Public x\nBeginProg\nx=IIF(1,2)",null,[]],["cr6","iif","Public x\nBeginProg\nx=IIF(1,\"a\",)",null,[[3,"type mismatch",""],[3,"Invalid, or out of place expression: x=IIF(1,\"a\",).","invalid_expression"]]],["cr6","iif","Public x\nBeginProg\nx=IIF(,\"a\",2)",null,[[3,"Invalid, or out of place expression: x=IIF(,\"a\",2).","invalid_expression"]]],["cr6","iif","Public x\nBeginProg\nx=IIF(1,IIF(1,\"a\",\"b\"),2)",null,[[3,"type mismatch",""]]],["cr6","data","DataTable(T,1,-1)\nDataInterval(0,1.5,Sec,0)\nSample(1,x,FP2)\nEndTable\nBeginProg\nScan(3,Sec,3,0)\nCallTable T\nNextScan\nEndProg",null,[[7,"DataTable interval 1000000 uSec not integrally divisible by scan interval 3000000 uSec.",""]]],["cr6","data","DataTable(T,1,-1)\nDataInterval(0,3,Hr,0)\nSample(1,x,FP2)\nEndTable\nBeginProg\nScan(2,Hr,3,0)\nCallTable T\nNextScan\nEndProg",null,[[7,"DataTable interval 10800 Sec not integrally divisible by scan interval 7200 Sec.",""]]],["cr6","data","DataTable(T,1,-1)\nDataInterval(0,1,Sec,0)\nSample(1,x,FP2)\nEndTable\nBeginProg\nScan(1500,uSec,3,0)\nCallTable T\nNextScan\nEndProg",null,[[7,"DataTable interval 1000000 uSec not integrally divisible by scan interval 1500 uSec.",""]]],["cr6","data","DataTable(T,1,-1)\nDataInterval(-1,-3,Sec,0)\nSample(1,x,FP2)\nEndTable\nBeginProg\nScan(1,Sec,3,0)\nCallTable T\nNextScan\nEndProg",null,[[2,"DataInterval TintoInt and Interval parameters must be non-negative","datainterval_non_negative"]]],["cr6","data","DataTable(T,1,-1)\nDataInterval(0,2,Sec,0)\nSample(1,x,FP2)\nEndTable\nBeginProg\nScan(1,Sec,3,0)\nCallTable T\nNextScan\nEndProg",null,[]],["cr6","data","DataTable(T,1,-1)\nDataInterval(0,1,Day,0)\nSample(1,x,FP2)\nEndTable\nBeginProg\nScan(1,Sec,3,0)\nCallTable T\nNextScan\nEndProg",null,[]],["cr6","snmp","SNMPVariable(x,\"1.3.6.1\",Float,RONLY)",null,[[1,"SNMP OID must be greater than 1.3.6.1.2.1.1.7",""]]],["cr6","snmp","SNMPVariable(x,\"abc\",Float,RONLY)",null,[[1,"Illegal object id.",""],[1,"SNMP OID must be greater than 1.3.6.1.2.1.1.7",""]]],["cr6","snmp","SNMPVariable(x,-1,Float,RONLY)",null,[[1,"Illegal object id.",""],[1,"SNMP OID must be greater than 1.3.6.1.2.1.1.7",""]]],["cr6","snmp","SNMPVariable(x,0,Float,RONLY)",null,[[1,"Illegal object id.",""],[1,"SNMP OID must be greater than 1.3.6.1.2.1.1.7",""]]],["cr6","snmp","SNMPVariable(x,Unknown,Float,RONLY)",null,[[1,"Illegal object id.",""],[1,"SNMP OID must be greater than 1.3.6.1.2.1.1.7",""]]],["cr1000x","scan","Scan(0,Sec,3,0)",[0,0],[[1,"Scan 0uSec invalid must be at least 1mSec.","scan_invalid"]]],["cr1000x","scan","Scan(-0.5,Sec,3,0)",[0,0],[[1,"Scan 0uSec invalid must be at least 1mSec.","scan_invalid"]]],["cr1000x","scan","Scan(-1,mSec,-1,2147483648)",[0,-1000],[[1,"Scan Interval parameter must be a non-negative integer","scan_param"],[1,"Scan -1000uSec invalid must be at least 1mSec.","scan_invalid"],[1,"Scan BufferOption parameter must be a non-negative integer","scan_param"],[1,"Scan Count parameter must be a non-negative integer","scan_param"]]],["cr1000x","scan","Scan(1500,uSec,3,0)",[0,1500],[[1,"Scans must be a multiple of 1mSec.",""]]],["cr1000x","scan","Scan(15,mSec,3,0)",[0,15000],[]],["cr1000x","scan","Scan(9500,uSec,3,0)",[0,9500],[[1,"Scans must be a multiple of 1mSec.",""]]],["cr1000x","scan","Scan(86401,Sec,3,0)",[86401,0],[[1,"Maximum Scan interval is 1 Day.",""]]],["cr1000x","scan","Scan(2147483648,Sec,3,0)",[-2147483648,0],[[1,"Scan Interval parameter must be a non-negative integer","scan_param"]]],["cr1000x","scan","Scan(2147483647,Min,3,0)",[-60,0],[]],["cr1000x","scan","Scan(\"fuzz\",Sec,3,0)",[-2147483648,0],[[1,"Scan Interval parameter must be a non-negative integer","scan_param"]]],["cr1000x","scan","Scan(1,1.9,3,0)",[0,1000],[]],["cr1000x","scan","Scan(1,9,3,0)",[0,0],[[1,"Scan 0uSec invalid must be at least 1mSec.","scan_invalid"]]],["cr1000x","scan","SlowSequence\nScan(99,mSec,3,0)",[0,99000],[[2,"Slow Scan interval must be >= 100mSec.","slow_scan_interval"],[2,"Slow Scan interval must be a multiple of 100mSec.","slow_scan_interval"]]],["cr1000x","scan","SlowSequence\nScan(95,mSec,3,0)",[0,95000],[[2,"Slow Scan interval must be >= 100mSec.","slow_scan_interval"],[2,"Slow Scan interval must be a multiple of 100mSec.","slow_scan_interval"]]],["cr1000x","scan","SlowSequence\nScan(100,mSec,3,0)",[0,100000],[]],["cr1000x","scan","SlowSequence\nScan(86401,Sec,3,0)",[86401,0],[[2,"Maximum Scan interval is 1 Day.",""],[2,"Maximum Slow Scan interval is 1 Day.",""]]],["cr1000x","modem","ModemCallBack(R,COMSDC7,599,0,0,0,0,0,0)",null,[[1,"599 is illegal baud rate.",""]]],["cr1000x","modem","ModemCallBack(R,COMSDC7,600,0,0,0,0,0,0)",null,[]],["cr1000x","modem","ModemCallBack(R,COMSDC7,460800,0,0,0,0,0,0)",null,[]],["cr1000x","modem","ModemCallBack(R,COMSDC7,460801,0,0,0,0,0,0)",null,[[1,"460801 is illegal baud rate.",""]]],["cr1000x","modem","ModemCallBack(R,COMSDC7,0.5,0,0,0,0,0,0)",null,[]],["cr1000x","modem","ModemCallBack(R,COMSDC7,2147483648,0,0,0,0,0,0)",null,[[1,"-2147483648 is illegal baud rate.",""]]],["cr1000x","modem","ModemCallBack(R,COMSDC7,\"x\",0,0,0,0,0,0)",null,[[1,"-2147483648 is illegal baud rate.",""]]],["cr1000x","modem","ModemCallBack(R,COMSDC7,Hr+1,0,0,0,0,0,0)",null,[[1,"5 is illegal baud rate.",""]]],["cr1000x","modem","ModemCallBack(R,COMSDC7,uSec,0,0,0,0,0,0)",null,[]],["cr1000x","iif","Public x\nBeginProg\nx=IIF(1,\"a\",2)",null,[[3,"type mismatch",""]]],["cr1000x","iif","Public x\nBeginProg\nx=IIF(1,\"a\",\"b\")",null,[]],["cr1000x","iif","Public x\nBeginProg\nx=IIF(1,\"a\")",null,[[3,"type mismatch",""]]],["cr1000x","iif","Public x\nBeginProg\nx=IIF(1,2)",null,[]],["cr1000x","iif","Public x\nBeginProg\nx=IIF(1,\"a\",)",null,[[3,"type mismatch",""],[3,"Invalid, or out of place expression: x=IIF(1,\"a\",).","invalid_expression"]]],["cr1000x","iif","Public x\nBeginProg\nx=IIF(,\"a\",2)",null,[[3,"Invalid, or out of place expression: x=IIF(,\"a\",2).","invalid_expression"]]],["cr1000x","iif","Public x\nBeginProg\nx=IIF(1,IIF(1,\"a\",\"b\"),2)",null,[[3,"type mismatch",""]]],["cr1000x","data","DataTable(T,1,-1)\nDataInterval(0,1.5,Sec,0)\nSample(1,x,FP2)\nEndTable\nBeginProg\nScan(3,Sec,3,0)\nCallTable T\nNextScan\nEndProg",null,[[7,"DataTable interval 1000000 uSec not integrally divisible by scan interval 3000000 uSec.",""]]],["cr1000x","data","DataTable(T,1,-1)\nDataInterval(0,3,Hr,0)\nSample(1,x,FP2)\nEndTable\nBeginProg\nScan(2,Hr,3,0)\nCallTable T\nNextScan\nEndProg",null,[[7,"DataTable interval 10800 Sec not integrally divisible by scan interval 7200 Sec.",""]]],["cr1000x","data","DataTable(T,1,-1)\nDataInterval(0,1,Sec,0)\nSample(1,x,FP2)\nEndTable\nBeginProg\nScan(1500,uSec,3,0)\nCallTable T\nNextScan\nEndProg",null,[[7,"DataTable interval 1000000 uSec not integrally divisible by scan interval 1500 uSec.",""]]],["cr1000x","data","DataTable(T,1,-1)\nDataInterval(-1,-3,Sec,0)\nSample(1,x,FP2)\nEndTable\nBeginProg\nScan(1,Sec,3,0)\nCallTable T\nNextScan\nEndProg",null,[[2,"DataInterval TintoInt and Interval parameters must be non-negative","datainterval_non_negative"]]],["cr1000x","data","DataTable(T,1,-1)\nDataInterval(0,2,Sec,0)\nSample(1,x,FP2)\nEndTable\nBeginProg\nScan(1,Sec,3,0)\nCallTable T\nNextScan\nEndProg",null,[]],["cr1000x","data","DataTable(T,1,-1)\nDataInterval(0,1,Day,0)\nSample(1,x,FP2)\nEndTable\nBeginProg\nScan(1,Sec,3,0)\nCallTable T\nNextScan\nEndProg",null,[]],["cr1000x","snmp","SNMPVariable(x,\"1.3.6.1\",Float,RONLY)",null,[[1,"SNMP OID must be greater than 1.3.6.1.2.1.1.7",""]]],["cr1000x","snmp","SNMPVariable(x,\"abc\",Float,RONLY)",null,[[1,"Illegal object id.",""],[1,"SNMP OID must be greater than 1.3.6.1.2.1.1.7",""]]],["cr1000x","snmp","SNMPVariable(x,-1,Float,RONLY)",null,[[1,"Illegal object id.",""],[1,"SNMP OID must be greater than 1.3.6.1.2.1.1.7",""]]],["cr1000x","snmp","SNMPVariable(x,0,Float,RONLY)",null,[[1,"Illegal object id.",""],[1,"SNMP OID must be greater than 1.3.6.1.2.1.1.7",""]]],["cr1000x","snmp","SNMPVariable(x,Unknown,Float,RONLY)",null,[[1,"Illegal object id.",""],[1,"SNMP OID must be greater than 1.3.6.1.2.1.1.7",""]]],["cr300","scan","Scan(0,Sec,3,0)",[0,0],[]],["cr300","scan","Scan(-0.5,Sec,3,0)",[0,0],[]],["cr300","scan","Scan(-1,mSec,-1,2147483648)",[0,-1000],[]],["cr300","scan","Scan(1500,uSec,3,0)",[0,1500],[]],["cr300","scan","Scan(15,mSec,3,0)",[0,15000],[[1,"Scans > 10mSec must be a multiple of 10mSec.","scans_gt"]]],["cr300","scan","Scan(9500,uSec,3,0)",[0,9500],[]],["cr300","scan","Scan(86401,Sec,3,0)",[86401,0],[[1,"Maximum Scan interval is 1 Day.",""]]],["cr300","scan","Scan(2147483648,Sec,3,0)",[2147483647,0],[[1,"Maximum Scan interval is 1 Day.",""]]],["cr300","scan","Scan(2147483647,Min,3,0)",[-60,0],[]],["cr300","scan","Scan(\"fuzz\",Sec,3,0)",[-2147483648,0],[]],["cr300","scan","Scan(1,1.9,3,0)",[0,1000],[]],["cr300","scan","Scan(1,9,3,0)",[0,0],[]],["cr300","scan","SlowSequence\nScan(99,mSec,3,0)",[0,99000],[[2,"Scans > 10mSec must be a multiple of 10mSec.","scans_gt"],[2,"Slow Scan interval must be >= 100mSec.","slow_scan_interval"],[2,"Slow Scan interval must be a multiple of 100mSec.","slow_scan_interval"]]],["cr300","scan","SlowSequence\nScan(95,mSec,3,0)",[0,95000],[[2,"Scans > 10mSec must be a multiple of 10mSec.","scans_gt"],[2,"Slow Scan interval must be >= 100mSec.","slow_scan_interval"],[2,"Slow Scan interval must be a multiple of 100mSec.","slow_scan_interval"]]],["cr300","scan","SlowSequence\nScan(100,mSec,3,0)",[0,100000],[]],["cr300","scan","SlowSequence\nScan(86401,Sec,3,0)",[86401,0],[[2,"Maximum Scan interval is 1 Day.",""],[2,"Maximum Slow Scan interval is 1 Day.",""]]],["cr300","modem","ModemCallBack(R,COMSDC7,599,0,0,0,0,0,0)",null,[]],["cr300","modem","ModemCallBack(R,COMSDC7,600,0,0,0,0,0,0)",null,[]],["cr300","modem","ModemCallBack(R,COMSDC7,460800,0,0,0,0,0,0)",null,[]],["cr300","modem","ModemCallBack(R,COMSDC7,460801,0,0,0,0,0,0)",null,[]],["cr300","modem","ModemCallBack(R,COMSDC7,0.5,0,0,0,0,0,0)",null,[]],["cr300","modem","ModemCallBack(R,COMSDC7,2147483648,0,0,0,0,0,0)",null,[]],["cr300","modem","ModemCallBack(R,COMSDC7,\"x\",0,0,0,0,0,0)",null,[]],["cr300","modem","ModemCallBack(R,COMSDC7,Hr+1,0,0,0,0,0,0)",null,[]],["cr300","modem","ModemCallBack(R,COMSDC7,uSec,0,0,0,0,0,0)",null,[]],["cr300","iif","Public x\nBeginProg\nx=IIF(1,\"a\",2)",null,[[3,"type mismatch",""]]],["cr300","iif","Public x\nBeginProg\nx=IIF(1,\"a\",\"b\")",null,[]],["cr300","iif","Public x\nBeginProg\nx=IIF(1,\"a\")",null,[[3,"type mismatch",""]]],["cr300","iif","Public x\nBeginProg\nx=IIF(1,2)",null,[]],["cr300","iif","Public x\nBeginProg\nx=IIF(1,\"a\",)",null,[[3,"type mismatch",""],[3,"Invalid, or out of place expression: x=IIF(1,\"a\",)\n.","invalid_expression"]]],["cr300","iif","Public x\nBeginProg\nx=IIF(,\"a\",2)",null,[[3,"Invalid, or out of place expression: x=IIF(,\"a\",2)\n.","invalid_expression"]]],["cr300","iif","Public x\nBeginProg\nx=IIF(1,IIF(1,\"a\",\"b\"),2)",null,[[3,"type mismatch",""]]],["cr300","data","DataTable(T,1,-1)\nDataInterval(0,1.5,Sec,0)\nSample(1,x,FP2)\nEndTable\nBeginProg\nScan(3,Sec,3,0)\nCallTable T\nNextScan\nEndProg",null,[[7,"DataTable interval 1000000 uSec not integrally divisible by scan interval 3000000 uSec.",""]]],["cr300","data","DataTable(T,1,-1)\nDataInterval(0,3,Hr,0)\nSample(1,x,FP2)\nEndTable\nBeginProg\nScan(2,Hr,3,0)\nCallTable T\nNextScan\nEndProg",null,[[7,"DataTable interval 10800 Sec not integrally divisible by scan interval 7200 Sec.",""]]],["cr300","data","DataTable(T,1,-1)\nDataInterval(0,1,Sec,0)\nSample(1,x,FP2)\nEndTable\nBeginProg\nScan(1500,uSec,3,0)\nCallTable T\nNextScan\nEndProg",null,[[7,"DataTable interval 1000000 uSec not integrally divisible by scan interval 1500 uSec.",""]]],["cr300","data","DataTable(T,1,-1)\nDataInterval(-1,-3,Sec,0)\nSample(1,x,FP2)\nEndTable\nBeginProg\nScan(1,Sec,3,0)\nCallTable T\nNextScan\nEndProg",null,[]],["cr300","data","DataTable(T,1,-1)\nDataInterval(0,2,Sec,0)\nSample(1,x,FP2)\nEndTable\nBeginProg\nScan(1,Sec,3,0)\nCallTable T\nNextScan\nEndProg",null,[]],["cr300","data","DataTable(T,1,-1)\nDataInterval(0,1,Day,0)\nSample(1,x,FP2)\nEndTable\nBeginProg\nScan(1,Sec,3,0)\nCallTable T\nNextScan\nEndProg",null,[]],["cr300","snmp","SNMPVariable(x,\"1.3.6.1\",Float,RONLY)",null,[[1,"SNMP OID must be greater than 1.3.6.1.2.1.1.7",""]]],["cr300","snmp","SNMPVariable(x,\"abc\",Float,RONLY)",null,[[1,"Illegal object id.",""],[1,"SNMP OID must be greater than 1.3.6.1.2.1.1.7",""]]],["cr300","snmp","SNMPVariable(x,-1,Float,RONLY)",null,[[1,"Illegal object id.",""],[1,"SNMP OID must be greater than 1.3.6.1.2.1.1.7",""]]],["cr300","snmp","SNMPVariable(x,0,Float,RONLY)",null,[[1,"Illegal object id.",""],[1,"SNMP OID must be greater than 1.3.6.1.2.1.1.7",""]]],["cr300","snmp","SNMPVariable(x,Unknown,Float,RONLY)",null,[[1,"Illegal object id.",""],[1,"SNMP OID must be greater than 1.3.6.1.2.1.1.7",""]]],["cr350","scan","Scan(0,Sec,3,0)",[0,0],[]],["cr350","scan","Scan(-0.5,Sec,3,0)",[0,0],[]],["cr350","scan","Scan(-1,mSec,-1,2147483648)",[0,-1000],[]],["cr350","scan","Scan(1500,uSec,3,0)",[0,1500],[]],["cr350","scan","Scan(15,mSec,3,0)",[0,15000],[[1,"Scans > 10mSec must be a multiple of 10mSec.","scans_gt"]]],["cr350","scan","Scan(9500,uSec,3,0)",[0,9500],[]],["cr350","scan","Scan(86401,Sec,3,0)",[86401,0],[[1,"Maximum Scan interval is 1 Day.",""]]],["cr350","scan","Scan(2147483648,Sec,3,0)",[2147483647,0],[[1,"Maximum Scan interval is 1 Day.",""]]],["cr350","scan","Scan(2147483647,Min,3,0)",[-60,0],[]],["cr350","scan","Scan(\"fuzz\",Sec,3,0)",[-2147483648,0],[]],["cr350","scan","Scan(1,1.9,3,0)",[0,1000],[]],["cr350","scan","Scan(1,9,3,0)",[0,0],[]],["cr350","scan","SlowSequence\nScan(99,mSec,3,0)",[0,99000],[[2,"Scans > 10mSec must be a multiple of 10mSec.","scans_gt"],[2,"Slow Scan interval must be >= 100mSec.","slow_scan_interval"],[2,"Slow Scan interval must be a multiple of 100mSec.","slow_scan_interval"]]],["cr350","scan","SlowSequence\nScan(95,mSec,3,0)",[0,95000],[[2,"Scans > 10mSec must be a multiple of 10mSec.","scans_gt"],[2,"Slow Scan interval must be >= 100mSec.","slow_scan_interval"],[2,"Slow Scan interval must be a multiple of 100mSec.","slow_scan_interval"]]],["cr350","scan","SlowSequence\nScan(100,mSec,3,0)",[0,100000],[]],["cr350","scan","SlowSequence\nScan(86401,Sec,3,0)",[86401,0],[[2,"Maximum Scan interval is 1 Day.",""],[2,"Maximum Slow Scan interval is 1 Day.",""]]],["cr350","modem","ModemCallBack(R,COMSDC7,599,0,0,0,0,0,0)",null,[]],["cr350","modem","ModemCallBack(R,COMSDC7,600,0,0,0,0,0,0)",null,[]],["cr350","modem","ModemCallBack(R,COMSDC7,460800,0,0,0,0,0,0)",null,[]],["cr350","modem","ModemCallBack(R,COMSDC7,460801,0,0,0,0,0,0)",null,[]],["cr350","modem","ModemCallBack(R,COMSDC7,0.5,0,0,0,0,0,0)",null,[]],["cr350","modem","ModemCallBack(R,COMSDC7,2147483648,0,0,0,0,0,0)",null,[]],["cr350","modem","ModemCallBack(R,COMSDC7,\"x\",0,0,0,0,0,0)",null,[]],["cr350","modem","ModemCallBack(R,COMSDC7,Hr+1,0,0,0,0,0,0)",null,[]],["cr350","modem","ModemCallBack(R,COMSDC7,uSec,0,0,0,0,0,0)",null,[]],["cr350","iif","Public x\nBeginProg\nx=IIF(1,\"a\",2)",null,[[3,"type mismatch",""]]],["cr350","iif","Public x\nBeginProg\nx=IIF(1,\"a\",\"b\")",null,[]],["cr350","iif","Public x\nBeginProg\nx=IIF(1,\"a\")",null,[[3,"type mismatch",""]]],["cr350","iif","Public x\nBeginProg\nx=IIF(1,2)",null,[]],["cr350","iif","Public x\nBeginProg\nx=IIF(1,\"a\",)",null,[[3,"type mismatch",""],[3,"Invalid, or out of place expression: x=IIF(1,\"a\",)\n.","invalid_expression"]]],["cr350","iif","Public x\nBeginProg\nx=IIF(,\"a\",2)",null,[[3,"Invalid, or out of place expression: x=IIF(,\"a\",2)\n.","invalid_expression"]]],["cr350","iif","Public x\nBeginProg\nx=IIF(1,IIF(1,\"a\",\"b\"),2)",null,[[3,"type mismatch",""]]],["cr350","data","DataTable(T,1,-1)\nDataInterval(0,1.5,Sec,0)\nSample(1,x,FP2)\nEndTable\nBeginProg\nScan(3,Sec,3,0)\nCallTable T\nNextScan\nEndProg",null,[[7,"DataTable interval 1000000 uSec not integrally divisible by scan interval 3000000 uSec.",""]]],["cr350","data","DataTable(T,1,-1)\nDataInterval(0,3,Hr,0)\nSample(1,x,FP2)\nEndTable\nBeginProg\nScan(2,Hr,3,0)\nCallTable T\nNextScan\nEndProg",null,[[7,"DataTable interval 10800 Sec not integrally divisible by scan interval 7200 Sec.",""]]],["cr350","data","DataTable(T,1,-1)\nDataInterval(0,1,Sec,0)\nSample(1,x,FP2)\nEndTable\nBeginProg\nScan(1500,uSec,3,0)\nCallTable T\nNextScan\nEndProg",null,[[7,"DataTable interval 1000000 uSec not integrally divisible by scan interval 1500 uSec.",""]]],["cr350","data","DataTable(T,1,-1)\nDataInterval(-1,-3,Sec,0)\nSample(1,x,FP2)\nEndTable\nBeginProg\nScan(1,Sec,3,0)\nCallTable T\nNextScan\nEndProg",null,[]],["cr350","data","DataTable(T,1,-1)\nDataInterval(0,2,Sec,0)\nSample(1,x,FP2)\nEndTable\nBeginProg\nScan(1,Sec,3,0)\nCallTable T\nNextScan\nEndProg",null,[]],["cr350","data","DataTable(T,1,-1)\nDataInterval(0,1,Day,0)\nSample(1,x,FP2)\nEndTable\nBeginProg\nScan(1,Sec,3,0)\nCallTable T\nNextScan\nEndProg",null,[]],["cr350","snmp","SNMPVariable(x,\"1.3.6.1\",Float,RONLY)",null,[[1,"SNMP OID must be greater than 1.3.6.1.2.1.1.7",""]]],["cr350","snmp","SNMPVariable(x,\"abc\",Float,RONLY)",null,[[1,"Illegal object id.",""],[1,"SNMP OID must be greater than 1.3.6.1.2.1.1.7",""]]],["cr350","snmp","SNMPVariable(x,-1,Float,RONLY)",null,[[1,"Illegal object id.",""],[1,"SNMP OID must be greater than 1.3.6.1.2.1.1.7",""]]],["cr350","snmp","SNMPVariable(x,0,Float,RONLY)",null,[[1,"Illegal object id.",""],[1,"SNMP OID must be greater than 1.3.6.1.2.1.1.7",""]]],["cr350","snmp","SNMPVariable(x,Unknown,Float,RONLY)",null,[[1,"Illegal object id.",""],[1,"SNMP OID must be greater than 1.3.6.1.2.1.1.7",""]]]]"#).unwrap();
        for row in rows.as_array().unwrap() {
            let profile = crate::profiles::get(row[0].as_str().unwrap()).unwrap();
            let mode = row[1].as_str().unwrap();
            let source = row[2].as_str().unwrap();
            let parse_profile = if mode == "modem" {
                crate::profiles::get("cr6").unwrap()
            } else {
                profile
            };
            let (program, _) =
                crate::parser::parse_text(source, "t.cr6", None, Some(parse_profile));
            let env = Env::new(profile);
            let mut diags = Diagnostics::new();
            match mode {
                "scan" => {
                    check_scan_params(&program, profile, &mut diags, &env);
                    check_scan_max_interval(&program, profile, &mut diags, &env);
                    let scan = program
                        .statements
                        .iter()
                        .find_map(|s| match s.as_ref() {
                            Stmt::Instruction(i) if pystr::upper(&i.keyword) == "SCAN" => Some(i),
                            _ => None,
                        })
                        .unwrap();
                    assert_eq!(
                        serde_json::to_value(scan_split(scan, &env)).unwrap(),
                        row[3],
                        "split {} {source}",
                        profile.name
                    );
                }
                "modem" => {
                    let Stmt::Instruction(stmt) = program.statements[0].as_ref() else {
                        panic!("instruction")
                    };
                    check_modemcallback_baud(stmt, &program, profile, &mut diags, &env);
                }
                "iif" => check_iif_types(&program, profile, &mut diags),
                "data" => check_data_interval(&program, profile, &mut diags, &env),
                "snmp" => check_snmp_oid(&program, profile, &mut diags, Some(&env)),
                _ => panic!("unknown mode"),
            }
            let actual: Vec<_> = diags
                .items
                .iter()
                .map(|d| serde_json::json!([d.line, d.message, d.kind]))
                .collect();
            assert_eq!(
                serde_json::json!(actual),
                row[4],
                "{} {mode} {source}",
                profile.name
            );
        }
    }
}
