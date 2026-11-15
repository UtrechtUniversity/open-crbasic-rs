//! Port of `checker/terminals.py`.

#![allow(
    clippy::used_underscore_items,
    reason = "port keeps Python's leading-underscore function names"
)]

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;

use crate::checker::context;
use crate::checker::longconv;
use crate::consteval;
use crate::diagnostics::Diagnostics;
use crate::profiles::{self, Json, Profile};
use crate::pystr;
use crate::syntax::{ConstValue, Expr, Instruction, Program, Stmt};

const INT_MIN: i64 = -2_147_483_648;
const I32_MIN_F64: f64 = -2_147_483_648.0;
const I32_MAX_BOUND_F64: f64 = 2_147_483_648.0;
const I32_MAX: i64 = 2_147_483_647;

#[allow(clippy::cast_precision_loss, reason = "Python int to float")]
fn int_to_f64(v: i64) -> f64 {
    v as f64
}

/// Port of `checker/terminals.py::_ranges`.
pub fn _ranges() {}

fn _ranges_impl(kind: &Json) -> Vec<(f64, f64)> {
    if let Some(ranges) = kind.get("ranges").and_then(Json::as_array) {
        return ranges
            .iter()
            .filter_map(|r| {
                let pair = r.as_array()?;
                let lo = pair.first().and_then(Json::as_f64)?;
                let hi = pair.get(1).and_then(Json::as_f64)?;
                Some((lo, hi))
            })
            .collect();
    }
    let lo = kind.get("min").and_then(Json::as_f64).unwrap_or(0.0);
    let hi = kind.get("max").and_then(Json::as_f64).unwrap_or(0.0);
    vec![(lo, hi)]
}

/// Port of `checker/terminals.py::_in_range`.
pub fn _in_range(_v: f64) -> bool {
    false
}

fn _in_range_impl(v: f64, kind: &Json) -> bool {
    !v.is_nan()
        && _ranges_impl(kind)
            .into_iter()
            .any(|(lo, hi)| lo <= v && v <= hi)
}

/// Port of `checker/terminals.py::_cr6_burst_kind`.
pub fn _cr6_burst_kind() -> bool {
    false
}

fn _cr6_burst_kind_impl(kind: &Json) -> bool {
    kind.get("transform").and_then(Json::as_str) == Some("abs")
        && _ranges_impl(kind) == [(201.0, 212.0)]
}

/// Port of `checker/terminals.py::_evaluate`.
pub fn _evaluate(_value: f64) -> (bool, bool, i64) {
    (false, false, 0)
}

fn _evaluate_impl(value: f64, kind: &Json) -> (bool, bool, i64) {
    let mut v = if kind.get("transform").and_then(Json::as_str) == Some("abs") {
        value.abs()
    } else {
        value
    };
    if _cr6_burst_kind_impl(kind) && -200.0 < value && value < 0.0 {
        v = value;
    }
    let in_range = if kind
        .get("port_wrap")
        .and_then(Json::as_bool)
        .unwrap_or(false)
    {
        let wrapped = if (I32_MIN_F64..I32_MAX_BOUND_F64).contains(&v) {
            pystr::py_int_from_f64(v).unwrap_or(INT_MIN)
        } else {
            INT_MIN
        };
        v = int_to_f64(wrapped);
        _in_range_impl(v, kind)
            || (wrapped >= 201 && pystr::pymod_i64(wrapped, 200).unwrap_or(0) <= 12)
    } else {
        _in_range_impl(v, kind)
    };
    let iv = pystr::py_int_from_f64(v).unwrap_or(INT_MIN);
    let odd_only = kind
        .get("odd_only")
        .and_then(Json::as_bool)
        .unwrap_or(false);
    let odd_ok = !odd_only || (in_range && pystr::pymod_i64(iv, 2) == Some(1));
    (in_range, odd_ok, iv)
}

const CR6_BRIDGES: &[&str] = &["BRHALF", "BRHALF3W", "BRHALF4W", "BRFULL", "BRFULL6W"];
const _I2C: &[&str] = &["I2COPEN", "I2CREAD", "I2CWRITE"];
const _SERIAL_BUS: &[&str] = &[
    "I2COPEN", "I2CREAD", "I2CWRITE", "SPIOPEN", "SPIREAD", "SPIWRITE",
];

/// Port of `checker/terminals.py::_bus_port`.
pub fn _bus_port() {}

fn _bus_port_impl(
    arg: Option<&Expr>,
    env: &consteval::Env,
    _profile: &Profile,
    _names: &(BTreeSet<String>, BTreeSet<String>),
) -> Option<i64> {
    if let Some(Expr::Name(n)) = arg
        && pystr::upper(&n.name) == "INF"
        && !env.consts.borrow().contains_key("INF")
    {
        return Some(I32_MAX);
    }
    let value = longconv::long_value(arg);
    if value.is_some() {
        return value;
    }
    match consteval::value_of(arg, env) {
        Ok(folded) if consteval::is_finite_number(&folded) => Some(consteval::to_long(&folded)),
        _ => None,
    }
}

/// Port of `checker/terminals.py::_check_bus_port`.
pub fn _check_bus_port() {}

fn _check_bus_port_impl(
    stmt: &Instruction,
    kind: &Json,
    value: Option<i64>,
    diags: &mut Diagnostics,
) {
    let Some(value) = value else {
        return;
    };
    let (in_range, odd_ok, shown) = _evaluate_impl(int_to_f64(value), kind);
    let shown_str = shown.to_string();
    if !in_range {
        let tmpl = kind.get("message").and_then(Json::as_str).unwrap_or("");
        let msg = pystr::format_named(tmpl, &[("v", &shown_str)]);
        diags.error_basic(stmt.base.node.line, &msg, &stmt.base.node.file);
        if !kind
            .get("secondary_when_invalid")
            .and_then(Json::as_bool)
            .unwrap_or(false)
        {
            return;
        }
    }
    if !odd_ok {
        let msg = kind.get("odd_message").and_then(Json::as_str).unwrap_or("");
        diags.error_basic(stmt.base.node.line, msg, &stmt.base.node.file);
    } else if let Some(allowed) = kind.get("allowed").and_then(Json::as_array) {
        let in_allowed = allowed.iter().filter_map(Json::as_i64).any(|x| x == shown);
        if !in_range || !in_allowed {
            let msg = kind
                .get("allowed_message")
                .and_then(Json::as_str)
                .unwrap_or("");
            diags.error_basic(stmt.base.node.line, msg, &stmt.base.node.file);
        }
    }
}

/// Port of `checker/terminals.py::check_i2c_open_order`.
pub fn check_i2c_open_order(program: &Program, profile: &Profile, diags: &mut Diagnostics) {
    let static_profile = profiles::get(profile.name).unwrap_or(&profiles::PROFILES[0]);
    let env = consteval::Env::with_consts(
        static_profile,
        Rc::new(RefCell::new(program.consts.clone())),
    );
    check_i2c_open_order_with_env(program, profile, diags, &env);
}

/// Full port of `checker/terminals.py::check_i2c_open_order` with `env`.
pub fn check_i2c_open_order_with_env(
    program: &Program,
    profile: &Profile,
    diags: &mut Diagnostics,
    env: &consteval::Env,
) {
    let kinds = profile.terminals().get("channels");
    let kind_name = if profile.name == "cr6" {
        "i2c_port"
    } else {
        "i2c_port_cr1000x"
    };
    let Some(kind) = kinds.and_then(|c| c.get(kind_name)) else {
        return;
    };
    let mut opened: BTreeSet<i64> = BTreeSet::new();
    let mut names: Option<(BTreeSet<String>, BTreeSet<String>)> = None;
    for stmt_ref in &program.statements {
        let Stmt::Instruction(stmt) = &**stmt_ref else {
            continue;
        };
        let keyword = pystr::upper(&stmt.keyword);
        if !_I2C.contains(&keyword.as_str()) || stmt.args.is_empty() {
            continue;
        }
        let expected_len = if keyword == "I2COPEN" { 2 } else { 5 };
        if stmt.args.len() != expected_len {
            continue;
        }
        let names_ref =
            names.get_or_insert_with(|| (longconv::float_names(), longconv::literal_names()));
        let Some(mut port) = _bus_port_impl(stmt.args[0].as_deref(), env, profile, names_ref)
        else {
            if keyword == "I2COPEN" {
                return;
            }
            continue;
        };
        let (in_range, odd_ok, _) = _evaluate_impl(int_to_f64(port), kind);
        if !in_range && profile.name == "cr6" {
            port = 101;
        } else if !odd_ok {
            continue;
        }
        if profile.name == "cr6" && port >= 201 {
            port -= 96;
        }
        if keyword == "I2COPEN" {
            opened.insert(port);
        } else if !opened.contains(&port) {
            let name = if keyword == "I2CREAD" {
                "I2CRead"
            } else {
                "I2CWrite"
            };
            diags.error_basic(
                stmt.base.node.line,
                &format!("I2COpen must precede {name}."),
                &stmt.base.node.file,
            );
        }
    }
}

/// Port of `checker/terminals.py::check`.
pub fn check(program: &Program, profile: &Profile, diags: &mut Diagnostics) {
    let static_profile = profiles::get(profile.name).unwrap_or(&profiles::PROFILES[0]);
    let env = consteval::Env::with_consts(
        static_profile,
        Rc::new(RefCell::new(program.consts.clone())),
    );
    check_with_env(program, profile, diags, &env);
}

/// Full port of `checker/terminals.py::check` with `env`.
pub fn check_with_env(
    program: &Program,
    profile: &Profile,
    diags: &mut Diagnostics,
    env: &consteval::Env,
) {
    let terminals = profile.terminals();
    let Some(checked_instructions) = terminals.get("instructions").and_then(Json::as_object) else {
        return;
    };
    if checked_instructions.is_empty() {
        return;
    }
    let Some(channels) = terminals.get("channels") else {
        return;
    };
    let mut bus_names: Option<(BTreeSet<String>, BTreeSet<String>)> = None;
    let flat = context::_with_inline(program);
    for stmt_ref in &flat.statements {
        let Stmt::Instruction(stmt) = &**stmt_ref else {
            continue;
        };
        let kw = pystr::upper(&stmt.keyword);
        let Some(positions_json) = checked_instructions
            .iter()
            .rev()
            .find(|(k, _)| k == &kw)
            .and_then(|(_, v)| v.as_array())
        else {
            continue;
        };
        if positions_json.is_empty() {
            continue;
        }
        if kw == "ACPOWER"
            && profile.name == "cr6"
            && let Some(first_kind_name) = positions_json[0]
                .as_array()
                .and_then(|a| a.get(1))
                .and_then(Json::as_str)
            && let Some(first_kind) = channels.get(first_kind_name)
            && _check_acpower_impl(stmt, first_kind, env, diags)
        {
            continue;
        }
        if profile.single_pass
            && CR6_BRIDGES.contains(&kw.as_str())
            && let Some(first_pos) = positions_json[0]
                .as_array()
                .and_then(|a| a.first())
                .and_then(Json::as_i64)
        {
            _check_bridge_excitation_impl(stmt, profile, env, diags, first_pos);
        }
        for entry in positions_json {
            let Some(parts) = entry.as_array() else {
                continue;
            };
            let Some(pos) = parts.first().and_then(Json::as_i64) else {
                continue;
            };
            let Some(kind_name) = parts.get(1).and_then(Json::as_str) else {
                continue;
            };
            let mut reps = parts
                .get(2)
                .and_then(Json::as_i64)
                .and_then(|rpos| _const_int_impl(stmt, rpos, env, true));
            let width = parts.get(3).and_then(Json::as_i64).unwrap_or(1);
            let Some(kind) = channels.get(kind_name) else {
                continue;
            };
            if pos == 1 && _SERIAL_BUS.contains(&kw.as_str()) && !stmt.args.is_empty() {
                let names_ref = bus_names
                    .get_or_insert_with(|| (longconv::float_names(), longconv::literal_names()));
                let val = _bus_port_impl(stmt.args[0].as_deref(), env, profile, names_ref);
                _check_bus_port_impl(stmt, kind, val, diags);
                continue;
            }
            if let Some(r) = reps
                && profile.name == "cr6"
                && CR6_BRIDGES.contains(&kw.as_str())
            {
                reps = Some(r.min(1));
            }
            if let Some(r) = reps
                && matches!(profile.name, "cr6" | "cr1000x")
                && matches!(kw.as_str(), "THERM107" | "THERM108" | "THERM109")
            {
                _check_thermistor_span_impl(stmt, profile, env, diags, r);
            }
            let pulse = kw == "PULSECOUNT" && matches!(profile.name, "cr1000x" | "cr300" | "cr350");
            let ports = if pulse {
                _pulse_config_impl(stmt, profile, kind, env, diags, reps)
            } else {
                Vec::new()
            };
            if let Some(r) = reps
                && kind.get("span").is_some()
            {
                _check_arg_impl(stmt, pos, kind, env, diags, 0, Some(r), width);
            } else {
                let span = reps.map_or(0, |r| r.max(1) * width - 1);
                _check_arg_impl(stmt, pos, kind, env, diags, span, None, 1);
            }
            for message in ports {
                diags.error_basic(stmt.base.node.line, &message, &stmt.base.node.file);
            }
        }
    }
}

/// Port of `checker/terminals.py::_const_int`.
pub fn _const_int(_stmt: &Instruction, _pos: i64, _truncate: bool) -> Option<i64> {
    None
}

fn _const_int_impl(
    stmt: &Instruction,
    pos: i64,
    env: &consteval::Env,
    truncate: bool,
) -> Option<i64> {
    if pos <= 0 {
        return None;
    }
    let idx = usize::try_from(pos - 1).ok()?;
    let arg = stmt.args.get(idx)?.as_deref()?;
    if !consteval::is_constant(Some(arg), env) {
        return None;
    }
    let value = consteval::value_of(Some(arg), env).ok()?;
    match value {
        ConstValue::Int(i) => Some(i),
        ConstValue::Float(f) | ConstValue::CFloat(f) => {
            if !f.is_finite() {
                return None;
            }
            if truncate || pystr::is_integer(f) {
                pystr::py_int_from_f64(f)
            } else {
                None
            }
        }
        ConstValue::BigInt(_) | ConstValue::Str(_) => None,
    }
}

/// Port of `checker/terminals.py::_check_thermistor_span`.
pub fn _check_thermistor_span(
    _stmt: &Instruction,
    _profile: &Profile,
    _diags: &mut Diagnostics,
    _reps: i64,
) {
}

fn _check_thermistor_span_impl(
    stmt: &Instruction,
    profile: &Profile,
    env: &consteval::Env,
    diags: &mut Diagnostics,
    reps: i64,
) {
    let Some(channel) = _const_int_impl(stmt, 3, env, true) else {
        return;
    };
    if reps <= 1 {
        return;
    }
    if profile.name == "cr6" {
        let start = channel.saturating_abs();
        if let Some(excitation) = _const_int_impl(stmt, 4, env, true)
            && (201..excitation).contains(&start)
            && excitation <= 212.min(start + reps - 1)
        {
            diags.error_basic(
                stmt.base.node.line,
                "Measurement channels and excitation channels overlap.",
                &stmt.base.node.file,
            );
        }
        if (-212..=-201).contains(&channel) && start + reps - 1 > 212 {
            diags.error_basic(
                stmt.base.node.line,
                "Channel 13 exceeds number of valid channels.",
                &stmt.base.node.file,
            );
        }
    } else if (-16..=-1).contains(&channel) && channel.saturating_abs() + reps - 1 > 16 {
        diags.error_basic(
            stmt.base.node.line,
            "Channel 17 exceeds number of valid channels.",
            &stmt.base.node.file,
        );
    }
}

/// Port of `checker/terminals.py::_check_bridge_excitation`.
pub fn _check_bridge_excitation(
    _stmt: &Instruction,
    _profile: &Profile,
    _diags: &mut Diagnostics,
    _channel_pos: i64,
) {
}

fn _check_bridge_excitation_impl(
    stmt: &Instruction,
    profile: &Profile,
    env: &consteval::Env,
    diags: &mut Diagnostics,
    channel_pos: i64,
) {
    let channel = _const_int_impl(stmt, channel_pos, env, true);
    let reps = _const_int_impl(stmt, 2, env, true);
    let excitation = _const_int_impl(stmt, channel_pos + 1, env, true);
    let measurements = _const_int_impl(stmt, channel_pos + 2, env, true);
    let (Some(channel), Some(reps), Some(mut excitation), Some(measurements)) =
        (channel, reps, excitation, measurements)
    else {
        return;
    };
    if channel < 1 || reps < 1 || measurements < 1 {
        return;
    }
    let first = profile.symbols().get("VX1").map_or(0, |s| s.value);
    let last = profile.symbols().get("VX2").map_or(0, |s| s.value);
    if profile.name == "cr350" && (excitation == 1 || excitation == 2) {
        excitation += first - 1;
    }
    let advance = pystr::floordiv_i64(reps - 1, measurements).unwrap_or(0);
    if (first..=last).contains(&excitation) && excitation + advance > last {
        diags.error_basic(
            stmt.base.node.line,
            "ExChan exceeds number of excitation channels.",
            &stmt.base.node.file,
        );
    }
}

/// Port of `checker/terminals.py::_pulse_config`.
pub fn _pulse_config(
    _stmt: &Instruction,
    _profile: &Profile,
    _diags: &mut Diagnostics,
    _reps: Option<i64>,
) -> Vec<String> {
    Vec::new()
}

fn _pulse_config_impl(
    stmt: &Instruction,
    profile: &Profile,
    kind: &Json,
    env: &consteval::Env,
    diags: &mut Diagnostics,
    reps: Option<i64>,
) -> Vec<String> {
    let channel = _const_int_impl(stmt, 3, env, true);
    let config = _const_int_impl(stmt, 4, env, true);
    let (Some(channel), Some(config), Some(reps)) = (channel, config, reps) else {
        return Vec::new();
    };
    if reps < 1 || !_in_range_impl(int_to_f64(channel), kind) {
        return Vec::new();
    }
    if profile.name == "cr1000x" {
        if ((channel == 301 || channel == 302) && config == 2)
            || ((101..=108).contains(&channel) && config == 5)
        {
            diags.error_basic(
                stmt.base.node.line,
                "Selected PulseCount config option is not compatible with channel type.",
                &stmt.base.node.file,
            );
        }
        return Vec::new();
    }
    let config = config & 0xFF;
    let switch_port = if profile.name == "cr300" { 8 } else { 7 };
    if config > 2 || (profile.name == "cr350" && channel == 8 && config == 2) {
        diags.error_basic(
            stmt.base.node.line,
            "Invalid PulseCount PConfig option.",
            &stmt.base.node.file,
        );
        return Vec::new();
    }
    if diags.errors().iter().any(|d| {
        d.line == stmt.base.node.line
            && d.file == stmt.base.node.file
            && d.message.starts_with("Variable ")
            && d.message.ends_with(" out of bounds.")
    }) {
        return Vec::new();
    }
    if config == 1 && channel == switch_port {
        return vec!["P_SW is not a valid low level AC input port.".to_string()];
    }
    let suffix = if config == 1 && channel <= 6 {
        "low level AC input port."
    } else if config == 2 && profile.name == "cr300" && (3..=6).contains(&channel) {
        "pulse count port."
    } else {
        return Vec::new();
    };
    let end = if channel <= 2 { 2 } else { 6 };
    let count = if channel + reps - 1 <= end { reps } else { 1 };
    (channel..channel + count)
        .map(|port| {
            let name = if port <= 2 {
                format!("C{port}")
            } else {
                format!("SE{}", port - 2)
            };
            format!("{name} is not a valid {suffix}")
        })
        .collect()
}

/// Port of `checker/terminals.py::_check_acpower`.
pub fn _check_acpower(_stmt: &Instruction, _diags: &mut Diagnostics) -> bool {
    false
}

fn _check_acpower_impl(
    stmt: &Instruction,
    kind: &Json,
    env: &consteval::Env,
    diags: &mut Diagnostics,
) -> bool {
    let v = _const_int_impl(stmt, 4, env, false);
    let i = _const_int_impl(stmt, 7, env, false);
    let reps = _const_int_impl(stmt, 10, env, false);
    let (Some(v), Some(i), Some(reps)) = (v, i, reps) else {
        return false;
    };
    if (reps != 1 && reps != 2) || v < 0 {
        return false;
    }
    let v_bad = !_in_range_impl(int_to_f64(v), kind);
    let i_bad = !_in_range_impl(int_to_f64(i), kind);
    let mut shown = if v_bad { vec![v, v, -v, v] } else { Vec::new() };
    for k in 0..reps {
        if k != 0 && v_bad {
            shown.push(v);
        }
        if i_bad {
            shown.push(-(i + k));
        }
    }
    let tmpl = kind.get("message").and_then(Json::as_str).unwrap_or("");
    for value in shown {
        let s = value.to_string();
        let msg = pystr::format_named(tmpl, &[("v", &s)]);
        diags.error_basic(stmt.base.node.line, &msg, &stmt.base.node.file);
    }
    true
}

/// Port of `checker/terminals.py::_check_arg`.
pub fn _check_arg(
    _stmt: &Instruction,
    _pos: i64,
    _diags: &mut Diagnostics,
    _span: i64,
    _reps: Option<i64>,
    _width: i64,
) {
}

#[allow(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    reason = "1:1 port of checker/terminals.py::_check_arg"
)]
fn _check_arg_impl(
    stmt: &Instruction,
    pos: i64,
    kind: &Json,
    env: &consteval::Env,
    diags: &mut Diagnostics,
    span: i64,
    reps: Option<i64>,
    width: i64,
) {
    let Ok(idx) = usize::try_from(pos - 1) else {
        return;
    };
    let Some(Some(arg_rc)) = stmt.args.get(idx) else {
        return;
    };
    let arg = arg_rc.as_ref();
    let mut huge = false;
    let huge_int_min = kind
        .get("huge_int_min")
        .and_then(Json::as_bool)
        .unwrap_or(false);
    let overflow_add = kind.get("overflow_add").and_then(Json::as_i64);
    if huge_int_min || overflow_add.is_some() {
        let folded = if consteval::is_constant(Some(arg), env) {
            consteval::value_of(Some(arg), env).ok()
        } else {
            None
        };
        let fi_opt = match folded {
            Some(ConstValue::Int(i)) => Some(i),
            Some(ConstValue::Float(f) | ConstValue::CFloat(f)) if f.is_finite() => {
                Some(pystr::py_int_from_f64(f).unwrap_or(i64::MAX))
            }
            _ => None,
        };
        if let Some(fi) = fi_opt {
            huge = !(INT_MIN + 1..=I32_MAX).contains(&fi);
            if let Some(add) = overflow_add
                && (huge || fi.saturating_abs().saturating_add(add) > I32_MAX)
            {
                return;
            }
        }
    }
    let flag_key = if huge {
        "huge_int_min"
    } else {
        "string_int_min"
    };
    if (matches!(arg, Expr::String(_)) || huge)
        && kind.get(flag_key).and_then(Json::as_bool).unwrap_or(false)
    {
        let value = INT_MIN;
        let (in_range, odd_ok, _) = _evaluate_impl(int_to_f64(value), kind);
        let shown = value.to_string();
        if !in_range {
            let tmpl = kind.get("message").and_then(Json::as_str).unwrap_or("");
            let msg = pystr::format_named(tmpl, &[("v", &shown)]);
            diags.error_basic(stmt.base.node.line, &msg, &stmt.base.node.file);
            if kind
                .get("secondary_when_invalid")
                .and_then(Json::as_bool)
                .unwrap_or(false)
                && (!odd_ok || kind.get("allowed").is_some())
            {
                let sec_key = if odd_ok {
                    "allowed_message"
                } else {
                    "odd_message"
                };
                let sec_tmpl = kind.get(sec_key).and_then(Json::as_str).unwrap_or("");
                let sec_msg = pystr::format_named(sec_tmpl, &[("v", &shown)]);
                diags.error_basic(stmt.base.node.line, &sec_msg, &stmt.base.node.file);
            }
        }
        return;
    }
    let non_numeric_int_min = kind
        .get("string_or_nan_int_min")
        .and_then(Json::as_bool)
        .unwrap_or(false);
    let mut folded_val = match consteval::value_of(Some(arg), env) {
        Ok(v) => v,
        Err(_) => {
            if non_numeric_int_min
                && let Expr::Name(n) = arg
                && pystr::upper(&n.name) == "NAN"
            {
                ConstValue::Int(INT_MIN)
            } else {
                return;
            }
        }
    };
    let is_str_or_nan = match &folded_val {
        ConstValue::Str(_) => true,
        ConstValue::Float(f) | ConstValue::CFloat(f) => f.is_nan(),
        ConstValue::Int(_) | ConstValue::BigInt(_) => false,
    };
    if non_numeric_int_min && is_str_or_nan {
        folded_val = ConstValue::Int(INT_MIN);
    }
    if !consteval::is_finite_number(&folded_val) {
        return;
    }
    let mut value = match folded_val {
        ConstValue::Int(i) => int_to_f64(i),
        ConstValue::Float(f) | ConstValue::CFloat(f) => f,
        ConstValue::BigInt(s) => s.parse::<f64>().unwrap_or(f64::INFINITY),
        ConstValue::Str(_) => return,
    };
    if kind
        .get("truncate")
        .and_then(Json::as_bool)
        .unwrap_or(false)
        && value.is_finite()
    {
        value = value.trunc();
    }
    let kw = pystr::upper(&stmt.keyword);
    if env.profile.single_pass
        && pos == 4
        && matches!(kw.as_str(), "VOLTDIFF" | "VOLTSE" | "TCDIFF")
        && -1.0 < value
        && value < 1.0
    {
        diags.items.retain(|d| {
            (d.file.as_str(), d.line) != (stmt.base.node.file.as_str(), stmt.base.node.line)
                || d.message != "Invalid reps 0, must be > 0."
        });
        diags.error_basic(
            stmt.base.node.line,
            "Invalid channel number.",
            &stmt.base.node.file,
        );
        return;
    }
    if kind
        .get("skip_nonpositive")
        .and_then(Json::as_bool)
        .unwrap_or(false)
        && value <= 0.0
    {
        return;
    }
    if _cr6_burst_kind_impl(kind) && value <= -200.0 && !(-212.0..=-201.0).contains(&value) {
        return;
    }
    let spec = if reps.is_some() {
        kind.get("span")
    } else {
        None
    };
    let top = _ranges_impl(kind)
        .into_iter()
        .map(|(_, hi)| hi)
        .fold(f64::NEG_INFINITY, f64::max);
    if let Some(spec) = spec
        && !spec
            .get("start_upper")
            .and_then(Json::as_bool)
            .unwrap_or(true)
        && value > top
    {
        let iv = pystr::py_int_from_f64(value).unwrap_or(INT_MIN);
        if kind
            .get("odd_only")
            .and_then(Json::as_bool)
            .unwrap_or(false)
            && pystr::pymod_i64(iv, 2) != Some(1)
        {
            let tmpl = kind.get("odd_message").and_then(Json::as_str).unwrap_or("");
            let msg = pystr::format_named(tmpl, &[("v", &iv.to_string())]);
            diags.error_basic(stmt.base.node.line, &msg, &stmt.base.node.file);
        } else {
            let shift = spec.get("shift").and_then(Json::as_i64).unwrap_or(0);
            let tmpl = spec.get("message").and_then(Json::as_str).unwrap_or("");
            let msg = pystr::format_named(tmpl, &[("v", &(iv + shift).to_string())]);
            diags.error_basic(stmt.base.node.line, &msg, &stmt.base.node.file);
        }
        return;
    }
    let (in_range, odd_ok, shown) = _evaluate_impl(value + int_to_f64(span), kind);
    let allowed_ok = match kind.get("allowed").and_then(Json::as_array) {
        None => true,
        Some(allowed) => in_range && allowed.iter().filter_map(Json::as_i64).any(|x| x == shown),
    };
    let shown_str = shown.to_string();
    if !in_range {
        let tmpl = kind.get("message").and_then(Json::as_str).unwrap_or("");
        let msg = pystr::format_named(tmpl, &[("v", &shown_str)]);
        diags.error_basic(stmt.base.node.line, &msg, &stmt.base.node.file);
        if !kind
            .get("secondary_when_invalid")
            .and_then(Json::as_bool)
            .unwrap_or(false)
        {
            return;
        }
    }
    if !odd_ok {
        let tmpl = kind.get("odd_message").and_then(Json::as_str).unwrap_or("");
        let msg = pystr::format_named(tmpl, &[("v", &shown_str)]);
        diags.error_basic(stmt.base.node.line, &msg, &stmt.base.node.file);
    } else if !allowed_ok {
        let tmpl = kind
            .get("allowed_message")
            .and_then(Json::as_str)
            .unwrap_or("");
        let msg = pystr::format_named(tmpl, &[("v", &shown_str)]);
        diags.error_basic(stmt.base.node.line, &msg, &stmt.base.node.file);
    } else if let Some(spec) = spec
        && let Some(reps) = reps
        && reps > 1
        && (value > 0.0 || !in_range)
    {
        let start = if in_range {
            value
        } else {
            spec.get("invalid_start")
                .and_then(Json::as_f64)
                .unwrap_or(0.0)
        };
        let step = spec.get("step").and_then(Json::as_f64).unwrap_or(1.0);
        let last = start + int_to_f64(reps - 1) * int_to_f64(width) * step;
        let Some((_, end)) = _ranges_impl(kind)
            .into_iter()
            .find(|(lo, hi)| *lo <= start && start <= *hi)
        else {
            return;
        };
        let check_kind = if spec.get("ranges").is_some() {
            spec
        } else {
            kind
        };
        if last > end || !_in_range_impl(last, check_kind) {
            let shift = spec.get("shift").and_then(Json::as_i64).unwrap_or(0);
            let v = pystr::py_int_from_f64(end).unwrap_or(0) + 1 + shift;
            let tmpl = spec.get("message").and_then(Json::as_str).unwrap_or("");
            let msg = pystr::format_named(tmpl, &[("v", &v.to_string())]);
            diags.error_basic(stmt.base.node.line, &msg, &stmt.base.node.file);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syntax::{ExprRef, Node, Number, NumberValue, StmtBase, StmtRef, StringLit};

    #[allow(clippy::unnecessary_wraps, reason = "test helper for arg slots")]
    fn num_i(v: i64) -> Option<ExprRef> {
        Some(Rc::new(Expr::Number(Number {
            node: Node::new(1, "test.cr6"),
            value: NumberValue::Int(v),
            text: v.to_string(),
        })))
    }

    #[allow(clippy::unnecessary_wraps, reason = "test helper for arg slots")]
    fn num_f(v: f64) -> Option<ExprRef> {
        Some(Rc::new(Expr::Number(Number {
            node: Node::new(1, "test.cr6"),
            value: NumberValue::Float(v),
            text: v.to_string(),
        })))
    }

    #[allow(clippy::unnecessary_wraps, reason = "test helper for arg slots")]
    fn str_lit(s: &str) -> Option<ExprRef> {
        Some(Rc::new(Expr::String(StringLit {
            node: Node::new(1, "test.cr6"),
            value: s.to_string(),
        })))
    }

    fn instr(line: i64, keyword: &str, args: Vec<Option<ExprRef>>) -> StmtRef {
        Rc::new(Stmt::Instruction(Instruction {
            base: StmtBase::new(line, "test.cr6"),
            keyword: keyword.to_string(),
            args,
            ..Default::default()
        }))
    }

    fn run_check(logger: &str, stmts: Vec<StmtRef>) -> Vec<(i64, String)> {
        let prof = profiles::get(logger).unwrap();
        let prog = Program::new("test.cr6", stmts);
        let mut diags = Diagnostics::new();
        let env = consteval::Env::new(prof);
        check_with_env(&prog, prof, &mut diags, &env);
        diags
            .errors()
            .into_iter()
            .map(|d| (d.line, d.message.clone()))
            .collect()
    }

    #[test]
    fn evaluate_matches_python() {
        let cr6 = profiles::get("cr6").unwrap();
        let channels = cr6.terminals().get("channels").unwrap();
        let u_diff = channels.get("u_terminal_diff").unwrap();
        assert_eq!(_evaluate_impl(-201.0, u_diff), (true, true, 201));
        assert_eq!(_evaluate_impl(-1.0, u_diff), (false, false, -1));

        let port = channels.get("port_c1c4_u1u12").unwrap();
        assert_eq!(_evaluate_impl(400.0, port), (true, true, 400));
        assert_eq!(_evaluate_impl(200.0, port), (false, true, 200));
        assert_eq!(_evaluate_impl(1e20, port), (false, true, -2_147_483_648));

        let cr300 = profiles::get("cr300").unwrap();
        let port_le8 = cr300
            .terminals()
            .get("channels")
            .unwrap()
            .get("port_le8")
            .unwrap();
        assert_eq!(_evaluate_impl(-1000.0, port_le8), (true, true, -1000));
    }

    #[test]
    fn cr6_voltdiff_acpower_therm_portset() {
        assert_eq!(
            run_check(
                "cr6",
                vec![
                    instr(4, "VOLTDIFF", vec![None, num_i(1), None, num_i(202)]),
                    instr(5, "VOLTDIFF", vec![None, num_i(2), None, num_i(211)]),
                    instr(6, "VOLTDIFF", vec![None, num_i(1), None, num_i(250)]),
                ],
            ),
            vec![
                (
                    4,
                    "Invalid start channel for Differential Measurement. Must be U1, U3, U5 ..."
                        .to_string()
                ),
                (
                    5,
                    "Channel 13 exceeds number of valid channels.".to_string()
                ),
                (
                    6,
                    "Invalid start channel for Differential Measurement. Must be U1, U3, U5 ..."
                        .to_string()
                ),
            ]
        );

        assert_eq!(
            run_check(
                "cr6",
                vec![instr(
                    4,
                    "ACPOWER",
                    vec![
                        None,
                        num_i(1),
                        num_i(50),
                        num_i(199),
                        num_i(1),
                        num_i(0),
                        num_i(300),
                        num_i(1),
                        num_i(0),
                        num_i(2),
                    ],
                )],
            ),
            vec![
                (4, "Invalid channel 199, must use U1..U12.".to_string()),
                (4, "Invalid channel 199, must use U1..U12.".to_string()),
                (4, "Invalid channel -199, must use U1..U12.".to_string()),
                (4, "Invalid channel 199, must use U1..U12.".to_string()),
                (4, "Invalid channel -300, must use U1..U12.".to_string()),
                (4, "Invalid channel 199, must use U1..U12.".to_string()),
                (4, "Invalid channel -301, must use U1..U12.".to_string()),
            ]
        );

        assert_eq!(
            run_check(
                "cr6",
                vec![
                    instr(4, "THERM107", vec![None, num_i(2), num_i(201), num_i(202)]),
                    instr(5, "THERM107", vec![None, num_i(2), num_i(-212), num_i(201)]),
                ],
            ),
            vec![
                (
                    4,
                    "Measurement channels and excitation channels overlap.".to_string()
                ),
                (
                    5,
                    "Channel 13 exceeds number of valid channels.".to_string()
                ),
            ]
        );

        assert_eq!(
            run_check(
                "cr6",
                vec![
                    instr(3, "PORTSET", vec![str_lit("fuzz"), num_i(1)]),
                    instr(4, "PORTSET", vec![num_i(400), num_i(1)]),
                    instr(5, "PORTSET", vec![num_i(200), num_i(1)]),
                ],
            ),
            vec![
                (
                    3,
                    "Invalid port -2147483648, use C1..C4 or U1..U12.".to_string()
                ),
                (5, "Invalid port 200, use C1..C4 or U1..U12.".to_string()),
            ]
        );
    }

    #[test]
    fn cr1000x_and_cr3xx_checks() {
        assert_eq!(
            run_check(
                "cr1000x",
                vec![
                    instr(4, "VOLTSE", vec![None, num_i(1), None, str_lit("abc")]),
                    instr(
                        5,
                        "VOLTDIFF",
                        vec![None, num_i(1), None, num_i(2_147_483_641)],
                    ),
                ],
            ),
            vec![(
                4,
                "Invalid channel -2147483648, must use 1..16.".to_string()
            )]
        );

        assert_eq!(
            run_check(
                "cr1000x",
                vec![
                    instr(4, "PULSECOUNT", vec![None, num_i(1), num_i(301), num_i(2)]),
                    instr(
                        5,
                        "PULSECOUNT",
                        vec![None, num_i(1), num_f(108.5), num_i(0)],
                    ),
                ],
            ),
            vec![(
                4,
                "Selected PulseCount config option is not compatible with channel type."
                    .to_string()
            )]
        );

        assert_eq!(
            run_check(
                "cr300",
                vec![
                    instr(4, "PULSECOUNT", vec![None, num_i(1), num_i(8), num_i(1)]),
                    instr(5, "PULSECOUNT", vec![None, num_i(2), num_i(1), num_i(1)]),
                    instr(6, "PULSECOUNT", vec![None, num_i(1), num_i(3), num_i(2)]),
                    instr(7, "PULSECOUNT", vec![None, num_i(1), num_i(1), num_i(3)]),
                ],
            ),
            vec![
                (
                    4,
                    "P_SW is not a valid low level AC input port.".to_string()
                ),
                (5, "C1 is not a valid low level AC input port.".to_string()),
                (5, "C2 is not a valid low level AC input port.".to_string()),
                (6, "SE1 is not a valid pulse count port.".to_string()),
                (7, "Invalid PulseCount PConfig option.".to_string()),
            ]
        );

        assert_eq!(
            run_check(
                "cr350",
                vec![
                    instr(
                        4,
                        "BRHALF",
                        vec![None, num_i(3), None, num_i(1), num_i(1), num_i(1)],
                    ),
                    instr(5, "VOLTDIFF", vec![None, num_i(0), None, num_i(0)]),
                ],
            ),
            vec![
                (
                    4,
                    "ExChan exceeds number of excitation channels.".to_string()
                ),
                (5, "Invalid channel number.".to_string()),
            ]
        );

        assert_eq!(
            run_check(
                "cr300",
                vec![
                    instr(4, "VOLTDIFF", vec![None, num_i(1), None, num_i(-4)]),
                    instr(5, "VOLTDIFF", vec![None, num_i(1), None, num_i(4)]),
                ],
            ),
            vec![(5, "Channel 4 exceeds number of valid channels.".to_string())]
        );
    }
}
