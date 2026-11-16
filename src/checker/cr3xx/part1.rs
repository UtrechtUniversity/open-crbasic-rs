//! Port of `checker/cr3xx.py` (lines 1-768).

#![allow(dead_code, unused_variables, unused_imports)]

use std::cell::RefCell;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::rc::Rc;
use std::sync::LazyLock;

use crate::consteval::{self, Env, NotConstant};
use crate::diagnostics::{DiagRef, Diagnostic, Diagnostics};
use crate::profiles::Profile;
use crate::pyre;
use crate::pystr;
use crate::syntax::{
    Call, ConstValue, Expr, NumberValue, OrderedMap, Program, ProgramRef, Stmt, StmtRef,
    ends_routine, stmt_id,
};

use super::super::{context, expressions, longconv};

const _SERIALOPEN: &str = "SERIALOPEN";

static _SERIALOPEN_PORTS: LazyLock<HashMap<&'static str, BTreeSet<i64>>> = LazyLock::new(|| {
    let mut map = HashMap::new();
    let mut cr6: BTreeSet<i64> = (1..19).collect();
    cr6.extend(31..100);
    map.insert("cr6", cr6);

    let mut cr1000x: BTreeSet<i64> = (1..15).collect();
    cr1000x.extend(31..100);
    map.insert("cr1000x", cr1000x);

    let mut cr300: BTreeSet<i64> = (1..13).collect();
    cr300.extend(32..48);
    map.insert("cr300", cr300);

    let mut cr350: BTreeSet<i64> = (1..18).collect();
    cr350.extend(32..48);
    map.insert("cr350", cr350);

    map
});

const _COMPORT_ARG_POSITION: usize = 0;

static _SERIAL_FUNCTIONS_WITH_COMPORT_ARG: LazyLock<BTreeSet<&'static str>> = LazyLock::new(|| {
    [
        "SERIALOPEN",
        "SERIALCLOSE",
        "SERIALFLUSH",
        "SERIALOUT",
        "SERIALIN",
        "SERIALINBLOCK",
        "SERIALINCHK",
        "SERIALBRK",
    ]
    .into_iter()
    .collect()
});

static _BAUDS: LazyLock<BTreeSet<i64>> = LazyLock::new(|| {
    [
        0, 600, 1200, 2400, 4800, 9600, 19200, 38400, 57600, 76800, 115_200, 230_400, 460_800,
    ]
    .into_iter()
    .collect()
});

static _COMMS_MODES: LazyLock<OrderedMap<&'static str, BTreeSet<i64>>> = LazyLock::new(|| {
    let mut map = OrderedMap::new();
    map.insert("COMC1", (0..8).collect());
    map.insert("COMC5", (0..8).collect());
    map.insert("COMC3", [0, 1, 2, 3, 4, 6].into_iter().collect());
    map.insert("COMC7", [0, 1, 2, 3, 4, 6].into_iter().collect());
    map
});

static _CR350_MAX_COMMS_MODE: LazyLock<HashMap<i64, i64>> = LazyLock::new(|| {
    [
        (1, 0),
        (2, 0),
        (3, 0),
        (4, 1),
        (7, 1),
        (8, 1),
        (9, 1),
        (10, 1),
    ]
    .into_iter()
    .collect()
});

static COM_U_SDC_RE: LazyLock<pyre::Regex> =
    LazyLock::new(|| pyre::Regex::new(r"COM(U|SDC)\d+", 0));

const I32_MIN_I64: i64 = -(1_i64 << 31);
const I32_MAX_EXCL_I64: i64 = 1_i64 << 31;
const I32_MIN_F64: f64 = -2_147_483_648.0;
const I32_MAX_EXCL_F64: f64 = 2_147_483_648.0;

fn env_with_consts(
    profile: &'static Profile,
    consts: Option<&OrderedMap<String, ConstValue>>,
) -> Env {
    match consts {
        Some(c) => Env::with_consts(profile, Rc::new(RefCell::new(c.clone()))),
        None => Env::new(profile),
    }
}

fn const_to_i64_exact(val: &ConstValue) -> Option<i64> {
    match val {
        ConstValue::Int(i) => Some(*i),
        ConstValue::Float(f) | ConstValue::CFloat(f) => {
            if f.is_finite() && pystr::is_integer(*f) {
                pystr::py_int_from_f64(*f)
            } else {
                None
            }
        }
        ConstValue::BigInt(s) => s.parse::<i64>().ok(),
        ConstValue::Str(_) => None,
    }
}

fn port_eq_i64(port: &ConstValue, target: i64) -> bool {
    const_to_i64_exact(port) == Some(target)
}

fn is_i32_range_const(val: &ConstValue) -> bool {
    match val {
        ConstValue::Int(i) => (I32_MIN_I64..I32_MAX_EXCL_I64).contains(i),
        ConstValue::Float(f) | ConstValue::CFloat(f) => {
            f.is_finite() && (I32_MIN_F64..I32_MAX_EXCL_F64).contains(f) && pystr::is_integer(*f)
        }
        ConstValue::BigInt(s) => s
            .parse::<i64>()
            .is_ok_and(|i| (I32_MIN_I64..I32_MAX_EXCL_I64).contains(&i)),
        ConstValue::Str(_) => false,
    }
}

fn with_inline_stmts(statements: &[StmtRef]) -> Vec<StmtRef> {
    let mut out = Vec::with_capacity(statements.len());
    for st in statements {
        out.push(st.clone());
        if let Stmt::If(if_st) = &**st {
            if let Some(inline) = &if_st.inline {
                out.extend(inline.iter().cloned());
            }
            if let Some(inline_else) = &if_st.inline_else {
                out.extend(inline_else.iter().cloned());
            }
        }
    }
    out
}

/// Port of checker/cr3xx.py::_check_cr350_comms_mode
pub fn _check_cr350_comms_mode(
    stmt: &Call,
    port: i64,
    profile: &'static Profile,
    consts: &OrderedMap<String, ConstValue>,
    diags: &mut Diagnostics,
    invalid_format: bool,
    shifted: bool,
) {
    let Some(&maximum) = _CR350_MAX_COMMS_MODE.get(&port) else {
        return;
    };
    let arg_len_bad = if shifted {
        stmt.args.len() < 5
    } else {
        stmt.args.len() != 6
    };
    if arg_len_bad {
        return;
    }
    let mode_arg = stmt.args[if shifted { 4 } else { 5 }].as_deref();
    let empty_set = BTreeSet::new();
    if longconv::_float_typed(mode_arg, &empty_set) {
        return;
    }
    let env = env_with_consts(profile, Some(consts));
    let Ok(raw) = consteval::value_of(mode_arg, &env) else {
        return;
    };
    if !consteval::is_finite_number(&raw) {
        return;
    }
    if let ConstValue::Float(f) | ConstValue::CFloat(f) = raw
        && (!(I32_MIN_F64..I32_MAX_EXCL_F64).contains(&f) || !pystr::is_integer(f))
    {
        return;
    }
    let fmt = if shifted {
        None
    } else {
        longconv::long_value(
            stmt.args[2].as_deref(),
            &env,
            profile,
            &empty_set,
            &empty_set,
        )
    };
    if !shifted && !invalid_format && !fmt.is_some_and(|v| (-1..=15).contains(&v)) {
        return;
    }
    if let Some(value) = longconv::long_value(mode_arg, &env, profile, &empty_set, &empty_set) {
        let mode = value & 0xFF;
        if mode > maximum {
            diags.error_basic(
                stmt.base.node.line,
                &format!("{mode} is an invalid CommsMode parameter."),
                &stmt.base.node.file,
            );
        }
    }
}

/// Port of checker/cr3xx.py::_commsmode_problem
pub fn _commsmode_problem(
    stmt: &Call,
    profile: &'static Profile,
    consts: Option<&OrderedMap<String, ConstValue>>,
    index: i64,
    mode: Option<ConstValue>,
) -> Option<(bool, String)> {
    let idx = usize::try_from(index).ok()?;
    let mode_arg = stmt.args.get(idx)?.as_deref()?;
    let port_arg = stmt.args.first().and_then(|a| a.as_deref());
    let env = env_with_consts(profile, consts);
    let port = consteval::value_of(port_arg, &env).ok()?;
    let mode_val = match mode {
        Some(m) => m,
        None => consteval::value_of(Some(mode_arg), &env).ok()?,
    };
    if !consteval::is_finite_number(&mode_val) {
        return None;
    }
    let mode_int = const_to_i64_exact(&mode_val)?;
    for (key, modes) in _COMMS_MODES.iter() {
        if let Some(record) = profile.symbols().get(key)
            && port_eq_i64(&port, record.value)
        {
            return if modes.contains(&mode_int) {
                None
            } else {
                Some((
                    true,
                    format!(
                        "CommsMode Option {} not supported on {}.",
                        mode_int, record.name
                    ),
                ))
            };
        }
    }
    if mode_int == 1 || mode_int == 2 {
        return None;
    }
    for (key, record) in profile.symbols().iter() {
        if port_eq_i64(&port, record.value)
            && (key == "COMRS232" || key == "COMME" || COM_U_SDC_RE.fullmatch(key).is_some())
        {
            let shown = match key {
                "COMRS232" => "COM232",
                "COMME" => "COMME",
                _ => record.name.as_str(),
            };
            return Some((
                false,
                format!("CommsMode Option {mode_int} not supported on {shown}."),
            ));
        }
    }
    None
}

/// Port of `checker/cr3xx.py::bare_unknown_delay`.
pub fn bare_unknown_delay(stmt: &Call, known: &BTreeSet<String>) -> bool {
    stmt.args.len() >= 5
        && matches!(
            stmt.args[3].as_deref(),
            Some(Expr::Name(n)) if !known.contains(&pystr::upper(&n.name))
        )
}

/// Port of checker/cr3xx.py::_format_needs_com1_4
pub fn _format_needs_com1_4(
    stmt: &Call,
    profile: &'static Profile,
    consts: Option<&OrderedMap<String, ConstValue>>,
) -> bool {
    let env = env_with_consts(profile, consts);
    let port_arg = stmt.args.first().and_then(|a| a.as_deref());
    let fmt_arg = stmt.args.get(2).and_then(|a| a.as_deref());
    let (Ok(port), Ok(fmt)) = (
        consteval::value_of(port_arg, &env),
        consteval::value_of(fmt_arg, &env),
    ) else {
        return false;
    };
    let (ConstValue::Int(p), ConstValue::Int(f)) = (port, fmt) else {
        return false;
    };
    (1..=8).contains(&p) && (16..I32_MAX_EXCL_I64).contains(&f)
}

/// Port of checker/cr3xx.py::_non_finite
pub fn _non_finite(
    arg: Option<&Expr>,
    profile: &'static Profile,
    consts: Option<&OrderedMap<String, ConstValue>>,
) -> Option<String> {
    let arg = arg?;
    if let Expr::Name(n) = arg
        && pystr::upper(&n.name) == "INF"
        && !consts.is_some_and(|c| c.contains_key("INF"))
    {
        return Some("float".to_string());
    }
    let env = env_with_consts(profile, consts);
    match consteval::value_of(Some(arg), &env) {
        Err(NotConstant::DivideByZero { .. }) => Some("divide".to_string()),
        Ok(ConstValue::Float(f) | ConstValue::CFloat(f)) if !f.is_finite() => {
            Some("float".to_string())
        }
        Err(_) | Ok(_) => None,
    }
}

/// Port of checker/cr3xx.py::_invalid_cr3xx_serial_format
pub fn _invalid_cr3xx_serial_format(arg: &Expr, env: &Env) -> bool {
    if !matches!(arg, Expr::Name(_)) && consteval::is_float(Some(arg), env) == Some(true) {
        return false;
    }
    let Ok(value) = consteval::value_of(Some(arg), env) else {
        return false;
    };
    if !consteval::is_finite_number(&value) {
        return false;
    }
    match value {
        ConstValue::Int(i) => (16..=I32_MAX_EXCL_I64).contains(&i),
        ConstValue::Float(f) | ConstValue::CFloat(f) => {
            (16.0..=I32_MAX_EXCL_F64).contains(&f) && pystr::is_integer(f)
        }
        ConstValue::BigInt(s) => s
            .parse::<i64>()
            .is_ok_and(|i| (16..=I32_MAX_EXCL_I64).contains(&i)),
        ConstValue::Str(_) => false,
    }
}

/// Port of checker/cr3xx.py::_non_numeric
pub fn _non_numeric(
    expr: Option<&Expr>,
    profile: &'static Profile,
    consts: Option<&OrderedMap<String, ConstValue>>,
    variable_names: &BTreeSet<String>,
) -> bool {
    if !expressions::part1::_variable_refs(expr, variable_names).is_empty() {
        return true;
    }
    let env = env_with_consts(profile, consts);
    let Ok(value) = consteval::value_of(expr, &env) else {
        return false;
    };
    matches!(value, ConstValue::Str(_)) || expressions::part1::_not_constant_integer(expr)
}

/// Port of checker/cr3xx.py::_check_baud
pub fn _check_baud(
    stmt: &Call,
    profile: &'static Profile,
    diags: &mut Diagnostics,
    consts: Option<&OrderedMap<String, ConstValue>>,
    port11: bool,
) {
    let port = stmt.args.first().and_then(|a| a.as_deref());
    let rs232 = matches!(port, Some(Expr::Name(n)) if pystr::upper(&n.name) == "COMRS232");
    let known_port = matches!(port, Some(Expr::Number(_)))
        || matches!(port, Some(Expr::Name(n)) if {
            let u = pystr::upper(&n.name);
            profile.symbols().contains(&u)
                || consts.is_some_and(|c| c.keys().any(|k| pystr::upper(k) == u))
        });
    if stmt.args.len() < 2 || !(rs232 || port11 || (!profile.single_pass && known_port)) {
        return;
    }
    let Some(mut arg) = stmt.args[1].as_deref() else {
        return;
    };
    let mut sign: i64 = 1;
    if let Expr::Unary(u) = arg
        && u.op == "-"
    {
        sign = -1;
        let Some(op) = u.operand.as_deref() else {
            return;
        };
        arg = op;
    }
    let baud = if let Expr::Number(n) = arg
        && let NumberValue::Int(i) = n.value
    {
        sign * i
    } else {
        let env = env_with_consts(profile, consts);
        let Ok(value) = consteval::value_of(stmt.args[1].as_deref(), &env) else {
            return;
        };
        if matches!(arg, Expr::Number(_)) || matches!(value, ConstValue::Str(_)) {
            return;
        }
        let Some(iv) = const_to_i64_exact(&value) else {
            return;
        };
        iv
    };
    if !(I32_MIN_I64..I32_MAX_EXCL_I64).contains(&baud) {
        return;
    }
    let ok = if matches!(profile.name, "cr300" | "cr350") {
        baud != 600 && _BAUDS.contains(&baud)
    } else {
        _BAUDS.contains(&baud.abs())
    };
    if !ok {
        diags.error_basic(
            stmt.base.node.line,
            &format!("{baud} is illegal baud rate."),
            &stmt.base.node.file,
        );
    }
}

/// Port of checker/cr3xx.py::check_dropped_tail
pub fn check_dropped_tail(
    program: &ProgramRef,
    profile: &'static Profile,
    diags: &mut Diagnostics,
) {
    let known_ctx = context::of(program, Some(profile), None);
    let known = &known_ctx.known_names().names;
    if !matches!(profile.name, "cr300" | "cr350") {
        return;
    }
    for stmt in &program.statements {
        let tail_ref = stmt.base().dropped_tail.borrow();
        let Some(tail) = tail_ref.as_deref() else {
            continue;
        };
        if tail.is_empty()
            || tail
                .iter()
                .any(|t| t.kind == "OP" && matches!(t.text.as_str(), "(" | "." | ","))
        {
            continue;
        }
        for t in tail {
            if t.kind == "IDENT" && !known.contains(&t.upper) {
                diags.error_basic(
                    stmt.base().node.line,
                    &profile.unresolved_symbol_message.format(&t.text),
                    &stmt.base().node.file,
                );
            }
        }
    }
}

/// Port of checker/cr3xx.py::check
pub fn check(program: &ProgramRef, profile: &'static Profile, diags: &mut Diagnostics) {
    let known_ctx = context::of(program, Some(profile), None);
    let known = &known_ctx.known_names().names;
    let cr3 = matches!(profile.name, "cr300" | "cr350");
    let ctx = context::of(program, Some(profile), None);
    let variable_names = ctx.declared_variable_names();
    let stmts_owned;
    let stmts: &[StmtRef] = if cr3 {
        stmts_owned = with_inline_stmts(&program.statements);
        &stmts_owned
    } else {
        &program.statements
    };

    for st in stmts {
        let Stmt::Call(stmt) = &**st else {
            continue;
        };
        let name = pystr::upper(&stmt.name);
        if !_SERIAL_FUNCTIONS_WITH_COMPORT_ARG.contains(name.as_str()) {
            continue;
        }
        if name == _SERIALOPEN && !stmt.explicit && stmt.args.is_empty() {
            diags.error_basic(
                stmt.base.node.line,
                "ComPort 0 illegal.",
                &stmt.base.node.file,
            );
        }
        let Some(arg) = stmt
            .args
            .get(_COMPORT_ARG_POSITION)
            .and_then(|a| a.as_deref())
        else {
            continue;
        };
        let names = expressions::part1::_undefined_names(
            Some(arg),
            &crate::checker::expressions::KnownView::from_set(known),
        )
        .into_iter()
        .map(crate::checker::expressions::UndefinedName::into_string)
        .collect::<Vec<_>>();
        if cr3 {
            for undefined_name in &names {
                diags.error_basic(
                    stmt.base.node.line,
                    &profile.unresolved_symbol_message.format(undefined_name),
                    &stmt.base.node.file,
                );
            }
        }
        if name != _SERIALOPEN {
            continue;
        }
        let is_variable = !expressions::part1::_variable_refs(Some(arg), variable_names).is_empty();
        let ports = _SERIALOPEN_PORTS.get(profile.name);
        let mut port: Option<ConstValue> = None;
        let sep = if cr3 && stmt.base.at_eol.get() {
            "\n"
        } else {
            ""
        };

        if let Some(ports_set) = ports
            && !(!names.is_empty() || is_variable || matches!(arg, Expr::String(_)))
        {
            let env = env_with_consts(profile, Some(&program.consts));
            port = consteval::value_of(Some(arg), &env).ok();
            let mut clamped_port = false;
            if cr3 && let Some(ref p) = port {
                let p_exact = const_to_i64_exact(p);
                if p_exact == Some(I32_MIN_I64) || p_exact == Some(I32_MAX_EXCL_I64) {
                    let clamped = if p_exact == Some(I32_MIN_I64) {
                        I32_MIN_I64 + 1
                    } else {
                        I32_MAX_EXCL_I64 - 1
                    };
                    port = Some(ConstValue::Int(clamped));
                    clamped_port = true;
                }
            }
            if !clamped_port {
                let bad_finite = port.as_ref().is_some_and(|p| {
                    consteval::is_finite_number(p)
                        && (!is_i32_range_const(p)
                            || (cr3 && expressions::part1::_not_constant_integer(Some(arg))))
                });
                let bad_str = cr3 && matches!(port, Some(ConstValue::Str(_)));
                if bad_finite || bad_str {
                    diags.error_basic(
                        stmt.base.node.line,
                        "ComPort 0 illegal.",
                        &stmt.base.node.file,
                    );
                    diags.error_basic(
                        stmt.base.node.line,
                        &format!(
                            "Parameter must be a constant integer: {}{sep}.",
                            stmt.base.text
                        ),
                        &stmt.base.node.file,
                    );
                    continue;
                }
            }
            if let Some(ref p) = port
                && consteval::is_finite_number(p)
                && let Some(p_int) = const_to_i64_exact(p)
                && !ports_set.contains(&p_int)
            {
                diags.items.push(Rc::new(Diagnostic::basic(
                    "error",
                    stmt.base.node.line,
                    &format!("ComPort {p_int} illegal."),
                    &stmt.base.node.file,
                )));
                if profile.name != "cr350" {
                    diags.error_basic(
                        stmt.base.node.line,
                        &format!(
                            "No Matching, or out of place ) in expression: {}{sep}.",
                            stmt.base.text
                        ),
                        &stmt.base.node.file,
                    );
                }
                continue;
            }
        }

        _check_baud(
            stmt,
            profile,
            diags,
            Some(&program.consts),
            cr3 && port.as_ref().is_some_and(|p| port_eq_i64(p, 11)),
        );

        let constant_port = !(!names.is_empty() || is_variable || matches!(arg, Expr::String(_)));
        let format_arg = stmt.args.get(2).and_then(|a| a.as_deref());
        let env = env_with_consts(profile, Some(&program.consts));
        let invalid_format = cr3
            && constant_port
            && port.as_ref().is_some_and(consteval::is_finite_number)
            && format_arg.is_some_and(|fa| {
                expressions::part1::_variable_refs(Some(fa), variable_names).is_empty()
                    && _invalid_cr3xx_serial_format(fa, &env)
            });
        if invalid_format {
            diags.error_basic(
                stmt.base.node.line,
                "Invalid format option.",
                &stmt.base.node.file,
            );
        }

        let empty_float_names = BTreeSet::new();
        let cr3_shifted = cr3
            && constant_port
            && !invalid_format
            && format_arg.is_some_and(|fa| {
                _non_numeric(Some(fa), profile, Some(&program.consts), variable_names)
                    || (!matches!(fa, Expr::Name(_))
                        && longconv::_float_typed(Some(fa), &empty_float_names))
            });
        if cr3_shifted {
            if profile.name == "cr350"
                && let Some(ref p) = port
                && consteval::is_finite_number(p)
                && let Some(p_int) = const_to_i64_exact(p)
            {
                _check_cr350_comms_mode(stmt, p_int, profile, &program.consts, diags, false, true);
            } else if stmt.args.len() > 5 {
                diags.error_basic(
                    stmt.base.node.line,
                    &format!(
                        "No Matching, or out of place ) in expression: {}{sep}.",
                        stmt.base.text
                    ),
                    &stmt.base.node.file,
                );
            }
            continue;
        }

        let mut shifted = !cr3
            && constant_port
            && format_arg.is_some_and(|fa| {
                _non_numeric(Some(fa), profile, Some(&program.consts), variable_names)
            });
        shifted = shifted || (!cr3 && constant_port && bare_unknown_delay(stmt, known));

        if !cr3
            && constant_port
            && !shifted
            && format_arg.is_some()
            && _format_needs_com1_4(stmt, profile, Some(&program.consts))
        {
            diags.error_basic(
                stmt.base.node.line,
                "Format option only available on COM1..COM4",
                &stmt.base.node.file,
            );
        }

        if constant_port
            && !shifted
            && let Some(Some(delay_rc)) = stmt.args.get(3)
        {
            let delay = &**delay_rc;
            if !expressions::part1::_variable_refs(Some(delay), variable_names).is_empty() {
                diags.error_basic(
                    stmt.base.node.line,
                    &format!(
                        "Parameter must be a constant integer: {}{sep}.",
                        stmt.base.text
                    ),
                    &stmt.base.node.file,
                );
                continue;
            }
            if let Ok(value) = consteval::value_of(Some(delay), &env)
                && consteval::is_finite_number(&value)
                && is_i32_range_const(&value)
                && let Some(v_int) = const_to_i64_exact(&value)
                && !(0..=14_000_000).contains(&v_int)
            {
                diags.error_basic(
                    stmt.base.node.line,
                    &format!("{v_int} usec delay before transmit is illegal, must be 0..14000000"),
                    &stmt.base.node.file,
                );
            }
        }

        let mode_arg_opt = stmt.args.get(5).and_then(|a| a.as_deref());
        let check_mode = !cr3 && constant_port && !shifted && mode_arg_opt.is_some();
        let mode_kind = if check_mode {
            _non_finite(mode_arg_opt, profile, Some(&program.consts))
        } else {
            None
        };
        let bad_mode = check_mode
            && (expressions::part1::_not_constant_integer(mode_arg_opt)
                || !expressions::part1::_variable_refs(mode_arg_opt, variable_names).is_empty()
                || mode_kind.is_some());

        if !cr3
            && constant_port
            && let Some(problem) = _commsmode_problem(
                stmt,
                profile,
                Some(&program.consts),
                if shifted { 4 } else { 5 },
                if bad_mode {
                    Some(ConstValue::Int(0))
                } else {
                    None
                },
            )
        {
            if problem.0 {
                diags.error_basic(stmt.base.node.line, &problem.1, &stmt.base.node.file);
            } else {
                diags.warning_basic(stmt.base.node.line, &problem.1, &stmt.base.node.file);
            }
        }

        if bad_mode {
            let verb = if mode_kind.as_deref() == Some("divide") {
                "Divide by zero in expression"
            } else {
                "Parameter must be a constant integer"
            };
            diags.error_basic(
                stmt.base.node.line,
                &format!("{verb}: {}.", stmt.base.text),
                &stmt.base.node.file,
            );
            continue;
        }
        if shifted && stmt.args.len() > 5 {
            diags.error_basic(
                stmt.base.node.line,
                &format!(
                    "No Matching, or out of place ) in expression: {}.",
                    stmt.base.text
                ),
                &stmt.base.node.file,
            );
            continue;
        }

        let buf_arg = stmt.args.get(4).and_then(|a| a.as_deref());
        if !(!names.is_empty() || is_variable || matches!(arg, Expr::String(_)))
            && stmt.args.len() > 4
            && !expressions::part1::_variable_refs(buf_arg, variable_names).is_empty()
        {
            diags.error_basic(
                stmt.base.node.line,
                &format!(
                    "Parameter must be a constant integer: {}{sep}.",
                    stmt.base.text
                ),
                &stmt.base.node.file,
            );
            continue;
        }
        if constant_port
            && stmt.args.len() > 4
            && let Some(kind) = _non_finite(buf_arg, profile, Some(&program.consts))
        {
            let verb = if kind == "divide" && !cr3 {
                "Divide by zero in expression"
            } else {
                "Parameter must be a constant integer"
            };
            diags.error_basic(
                stmt.base.node.line,
                &format!("{verb}: {}{sep}.", stmt.base.text),
                &stmt.base.node.file,
            );
            continue;
        }

        if profile.name == "cr300"
            && port.as_ref().is_some_and(|p| port_eq_i64(p, 11))
            && stmt.args.len() > 4
            && !expressions::part1::_not_constant_integer(buf_arg)
            && let Ok(size) = consteval::value_of(buf_arg, &env)
            && consteval::is_finite_number(&size)
        {
            let in_range = match &size {
                ConstValue::Int(i) => (1..I32_MAX_EXCL_I64).contains(i),
                ConstValue::Float(f) | ConstValue::CFloat(f) => *f > 0.0 && *f < I32_MAX_EXCL_F64,
                _ => false,
            };
            if in_range {
                diags.items.push(Rc::new(Diagnostic::basic(
                    "error",
                    stmt.base.node.line,
                    "Out of memory.",
                    &stmt.base.node.file,
                )));
            }
        }

        if profile.name == "cr350"
            && let Some(ref p) = port
            && consteval::is_finite_number(p)
            && let Some(p_int) = const_to_i64_exact(p)
        {
            _check_cr350_comms_mode(
                stmt,
                p_int,
                profile,
                &program.consts,
                diags,
                invalid_format,
                false,
            );
        }

        if profile.name == "cr300"
            && constant_port
            && stmt.args.len() == 6
            && let Some(mode_arg) = mode_arg_opt
        {
            let mode = consteval::value_of(Some(mode_arg), &env).ok();
            let bad_cr300_mode = match &mode {
                None => true,
                Some(m) => {
                    !consteval::is_finite_number(m)
                        || const_to_i64_exact(m).is_none()
                        || (!matches!(mode_arg, Expr::Name(_))
                            && consteval::is_float(Some(mode_arg), &env) == Some(true))
                        || !expressions::part1::_variable_refs(Some(mode_arg), variable_names)
                            .is_empty()
                }
            };
            if bad_cr300_mode {
                diags.error_basic(
                    stmt.base.node.line,
                    &format!(
                        "Parameter must be a constant integer: {}{sep}.",
                        stmt.base.text
                    ),
                    &stmt.base.node.file,
                );
            }
        }

        if !names.is_empty() || is_variable || matches!(arg, Expr::String(_)) {
            diags.error_basic(
                stmt.base.node.line,
                "ComPort 0 illegal.",
                &stmt.base.node.file,
            );
            if !cr3 && !names.is_empty() && !is_variable {
                continue;
            }
            diags.error_basic(
                stmt.base.node.line,
                &format!(
                    "Parameter must be a constant integer: {}{sep}.",
                    stmt.base.text
                ),
                &stmt.base.node.file,
            );
        }
    }
}

pub(super) const _UNRESOLVED_SUFFIX: &str = " is not defined.";
pub(super) const _VARIABLE_ILLEGAL_PREFIX: &str = "Variable illegal in Parameter:";
const _TOO_MANY_PREFIX: &str = "Too many parameters:";

const _FLOAT_DEST: i64 = 8;
const _DEST_MESSAGE: &str = "Illegal destination variable type, expecting Float.";

/// Port of checker/cr3xx.py::_destination_types
pub fn _destination_types(
    program: &ProgramRef,
    profile: &'static Profile,
) -> (
    OrderedMap<String, String>,
    HashMap<usize, OrderedMap<String, String>>,
) {
    let mut types = context::of(program, Some(profile), None).variable_types(false);
    let mut declared: HashSet<String> = HashSet::new();
    for st in &program.statements {
        if let Stmt::Declaration(decl) = &**st {
            for v in decl.vars.borrow().iter() {
                declared.insert(pystr::upper(&v.name));
            }
        }
    }
    let mut scoped_cells: HashMap<usize, Rc<RefCell<OrderedMap<String, String>>>> = HashMap::new();
    let mut local: Option<Rc<RefCell<OrderedMap<String, String>>>> = None;
    for st in &program.statements {
        match &**st {
            Stmt::SubDef(_) => {
                local = Some(Rc::new(RefCell::new(OrderedMap::new())));
            }
            s if ends_routine(s) => {
                local = None;
            }
            _ => {}
        }
        if let Some(ref loc) = local {
            scoped_cells.insert(stmt_id(st), loc.clone());
        }
        if let Stmt::Instruction(inst) = &**st
            && pystr::upper(&inst.keyword) == "ALIAS"
            && inst.args.len() > 1
            && let Some(src_name) = inst.args[0].as_deref().and_then(|e| match e {
                Expr::Name(n) => Some(pystr::upper(&n.name)),
                Expr::Index(idx) => Some(pystr::upper(&idx.name)),
                _ => None,
            })
            && let Some(src_type) = types.get(&src_name).cloned()
        {
            for alias_opt in &inst.args[1..] {
                if let Some(alias_name) = alias_opt.as_deref().and_then(|e| match e {
                    Expr::Name(n) => Some(pystr::upper(&n.name)),
                    Expr::Index(idx) => Some(pystr::upper(&idx.name)),
                    _ => None,
                }) && !declared.contains(&alias_name)
                {
                    if let Some(ref loc) = local {
                        let mut loc_mut = loc.borrow_mut();
                        if !loc_mut.contains_key(&alias_name) {
                            loc_mut.insert(alias_name, src_type.clone());
                        }
                    } else if !types.contains_key(&alias_name) {
                        types.insert(alias_name, src_type.clone());
                    }
                }
            }
        }
    }
    let scoped = scoped_cells
        .into_iter()
        .map(|(id, cell)| (id, cell.borrow().clone()))
        .collect();
    (types, scoped)
}

/// Port of `checker/cr3xx.py::_is_bad_destination`.
pub fn _is_bad_destination(expr: &Expr, types: &OrderedMap<String, String>) -> Option<bool> {
    match expr {
        Expr::Name(n) => types.get(&pystr::upper(&n.name)).map(|k| k != "FLOAT"),
        Expr::Index(idx) => types.get(&pystr::upper(&idx.name)).map(|k| k != "FLOAT"),
        Expr::Number(_) | Expr::String(_) | Expr::Binary(_) | Expr::Unary(_) => Some(true),
        _ => Some(false),
    }
}

/// Port of checker/cr3xx.py::check_float_destinations
pub fn check_float_destinations(
    program: &ProgramRef,
    profile: &'static Profile,
    diags: &mut Diagnostics,
) {
    if !matches!(profile.name, "cr300" | "cr350") {
        return;
    }
    let ctx = context::of(program, Some(profile), None);
    let known_view = ctx.known_names();
    let ds = ctx.dims_state();
    let destination_env = super::super::instr_data::_destination_env(&Env::new(profile));
    let mut types_cache: Option<(
        OrderedMap<String, String>,
        HashMap<usize, OrderedMap<String, String>>,
    )> = None;
    let mut constant_names: BTreeSet<String> = profile
        .symbols()
        .iter()
        .map(|(k, _)| k.to_string())
        .collect();
    constant_names.insert("NAN".to_string());
    constant_names.insert("INF".to_string());
    let mut function_names: BTreeSet<String> = profile
        .functions()
        .iter()
        .map(|(k, _)| k.to_string())
        .collect();
    let dest_ctx = context::of(program, Some(profile), None);
    let destination_variables = dest_ctx.declared_variable_names();
    let mut params: HashMap<String, String> = HashMap::new();
    let mut scalar_params: BTreeSet<String> = BTreeSet::new();
    let mut per_line: HashMap<(i64, String), usize> = HashMap::new();
    for st in &program.statements {
        let key = (st.base().node.line, st.base().node.file.clone());
        *per_line.entry(key).or_insert(0) += 1;
    }

    for stmt in &program.statements {
        match &**stmt {
            Stmt::SubDef(sd) => {
                params = sd
                    .params
                    .iter()
                    .map(|p| {
                        (
                            pystr::upper(&p.name),
                            pystr::upper(p.type_.as_deref().unwrap_or("FLOAT")),
                        )
                    })
                    .collect();
                scalar_params = sd
                    .params
                    .iter()
                    .filter(|p| {
                        p.dim_exprs.is_empty()
                            && (p.dims == 0 || p.type_.is_none())
                            && pystr::upper(p.type_.as_deref().unwrap_or("")) != "STRING"
                    })
                    .map(|p| pystr::upper(&p.name))
                    .collect();
                if pystr::upper(&sd.keyword) == "FUNCTION" {
                    function_names.insert(pystr::upper(&sd.name));
                }
            }
            Stmt::ConstDecl(cd) => {
                let name = pystr::upper(&cd.name);
                constant_names.insert(name.clone());
                if let Some(val) = program.consts.get(&name) {
                    destination_env
                        .consts
                        .borrow_mut()
                        .insert(name, val.clone());
                }
            }
            s if ends_routine(s) => {
                params.clear();
                scalar_params.clear();
            }
            _ => {}
        }

        let Stmt::Instruction(inst) = &**stmt else {
            continue;
        };
        let key = (inst.base.node.line, inst.base.node.file.clone());
        if inst.args.is_empty()
            || inst.args[0].is_none()
            || per_line.get(&key).copied().unwrap_or(0) > 1
        {
            continue;
        }
        let kw_upper = pystr::upper(&inst.keyword);
        let info = profile.instructions().get(&kw_upper);
        let send_variables = kw_upper == "SENDVARIABLES";
        let named_destination =
            send_variables || matches!(kw_upper.as_str(), "MOVE" | "ERASE" | "READIO" | "REALTIME");
        let dest_vars_with_params: BTreeSet<String> = if named_destination {
            destination_variables
                .iter()
                .cloned()
                .chain(params.keys().cloned())
                .collect()
        } else {
            BTreeSet::new()
        };
        let invalid_destination = info.and_then(|inf| {
            if inf.params.is_empty() {
                return None;
            }
            inf.params
                .iter()
                .zip(inst.args.iter())
                .enumerate()
                .find_map(|(i, (&code, a_opt))| {
                    if code != 27 && code != 29 {
                        return None;
                    }
                    let a = a_opt.as_deref();
                    let bad = if named_destination {
                        super::super::instr_data::constant_destination(
                            a,
                            &destination_env,
                            &dest_vars_with_params,
                        )
                    } else {
                        a.is_some_and(|ex| {
                            !matches!(ex, Expr::String(_)) && _expr_names(Some(ex)).is_empty()
                        })
                    };
                    if bad { Some(i) } else { None }
                })
        });

        if let Some(inv_idx) = invalid_destination
            && per_line.get(&key).copied().unwrap_or(0) == 1
            && (send_variables
                || !diags.items.iter().any(|d| {
                    d.severity == "error"
                        && d.line == inst.base.node.line
                        && d.file == inst.base.node.file
                }))
        {
            let mut at = diags
                .items
                .iter()
                .position(|d| d.file == inst.base.node.file && d.line > inst.base.node.line)
                .unwrap_or(diags.items.len());
            let arg_text = expressions::part2::_arg_text(inst, i64::try_from(inv_idx).unwrap_or(0));
            let mut new_diags = vec![Rc::new(Diagnostic::basic(
                "error",
                inst.base.node.line,
                &format!("Invalid Variable or Array: {}.", pystr::strip(&arg_text)),
                &inst.base.node.file,
            ))];
            let following = inv_idx + 1;
            if let Some(Some(_)) = inst.args.get(following) {
                let raw =
                    expressions::part2::_arg_text(inst, i64::try_from(following).unwrap_or(0));
                let tok = super::super::instr_rules::_next_simple_token(&raw);
                let text = if tok.is_empty() { raw } else { tok };
                new_diags.push(Rc::new(Diagnostic::basic(
                    "error",
                    inst.base.node.line,
                    &format!("Too many parameters:  {text}."),
                    &inst.base.node.file,
                )));
            }
            if send_variables {
                let removed_before = diags.items[..at]
                    .iter()
                    .filter(|d| {
                        d.line == inst.base.node.line
                            && d.file == inst.base.node.file
                            && d.severity == "error"
                    })
                    .count();
                at -= removed_before;
                diags.items.retain(|d| {
                    d.line != inst.base.node.line
                        || d.file != inst.base.node.file
                        || d.severity != "error"
                });
            }
            diags.items.splice(at..at, new_diags);
            continue;
        }

        let Some(inf) = info else {
            continue;
        };
        if inf.params.first().copied() != Some(_FLOAT_DEST) {
            continue;
        }
        if types_cache.is_none() {
            types_cache = Some(_destination_types(program, profile));
        }
        let (types, scoped) = types_cache.as_ref().unwrap();
        let known = &known_view.names;
        let arg = inst.args[0].as_deref().unwrap();
        if let Expr::Name(n) = arg
            && profile.functions().contains(&pystr::upper(&n.name))
        {
            continue;
        }
        if let Expr::Index(idx) = arg
            && scalar_params.contains(&pystr::upper(&idx.name))
            && idx.args.iter().any(Option::is_some)
        {
            let raw = pystr::strip(&expressions::part2::_arg_text(inst, 0)).to_string();
            diags.items.retain(|d| {
                (d.line != inst.base.node.line || d.file != inst.base.node.file)
                    || (!d.message.starts_with("Dimensions not declared:")
                        && !d.message.starts_with("Too many parameters:"))
            });
            diags.items.push(Rc::new(Diagnostic::basic(
                "error",
                inst.base.node.line,
                &format!("Dimensions not declared: {raw}."),
                &inst.base.node.file,
            )));
            if let Some(Some(_)) = inst.args.get(1) {
                let raw_1 = expressions::part2::_arg_text(inst, 1);
                let tok = super::super::instr_rules::_next_simple_token(&raw_1);
                let token = if tok.is_empty() { raw_1 } else { tok };
                diags.error_basic(
                    inst.base.node.line,
                    &format!("Too many parameters:  {token}."),
                    &inst.base.node.file,
                );
            }
            continue;
        }

        let mut undefined: Vec<String> = _expr_names(Some(arg))
            .into_iter()
            .filter(|n| !known.contains(&pystr::upper(n)))
            .collect();
        let mut variables = types.clone();
        if let Some(loc) = scoped.get(&stmt_id(stmt)) {
            for (k, v) in loc.iter() {
                variables.insert(k.clone(), v.clone());
            }
        }
        for (k, v) in &params {
            variables.insert(k.clone(), v.clone());
        }
        let call = matches!(
            arg,
            Expr::Index(idx)
                if !variables.contains_key(&pystr::upper(&idx.name))
                    && function_names.contains(&pystr::upper(&idx.name))
        );
        let mut expression = false;
        let mut dims = false;
        let keep: Vec<String> = if !undefined.is_empty() {
            if matches!(arg, Expr::Binary(_) | Expr::Unary(_)) || call {
                expression = true;
                (undefined, dims) = expressions::_names_and_dims(
                    known_view,
                    Some(ds),
                    Some(stmt_id(stmt)),
                    &[],
                    &inst.args[..1],
                );
            } else if !matches!(arg, Expr::Name(_) | Expr::Index(_)) {
                continue;
            }
            undefined
                .iter()
                .map(|n| profile.unresolved_symbol_message.format(n))
                .collect()
        } else if call
            || _is_bad_destination(arg, &variables) == Some(true)
            || (matches!(
                arg,
                Expr::Name(n)
                    if !variables.contains_key(&pystr::upper(&n.name))
                        && constant_names.contains(&pystr::upper(&n.name))
            ))
        {
            Vec::new()
        } else {
            continue;
        };

        let own: Vec<DiagRef> = diags
            .items
            .iter()
            .filter(|d| {
                d.line == inst.base.node.line
                    && d.file == inst.base.node.file
                    && d.severity == "error"
            })
            .cloned()
            .collect();
        if call
            && own.iter().any(|d| {
                d.message.starts_with("Incomplete expression:")
                    || d.message
                        .starts_with("Invalid, or out of place expression:")
            })
        {
            continue;
        }
        if keep.is_empty()
            && own
                .iter()
                .any(|d| d.message.starts_with("Dimensions not declared"))
        {
            continue;
        }
        if !keep.is_empty()
            && (own.len() < keep.len()
                || !own
                    .iter()
                    .take(keep.len())
                    .map(|d| d.message.as_str())
                    .eq(keep.iter().map(String::as_str)))
        {
            continue;
        }
        let at = if let Some(first_own) = own.first() {
            diags
                .items
                .iter()
                .position(|d| Rc::ptr_eq(d, first_own))
                .unwrap_or(diags.items.len())
        } else {
            diags
                .items
                .iter()
                .position(|d| d.file == inst.base.node.file && d.line > inst.base.node.line)
                .unwrap_or(diags.items.len())
        };
        let mut new_diags: Vec<DiagRef> = if keep.is_empty() {
            vec![Rc::new(Diagnostic::basic(
                "error",
                inst.base.node.line,
                _DEST_MESSAGE,
                &inst.base.node.file,
            ))]
        } else {
            own.iter().take(keep.len()).cloned().collect()
        };
        let undef_idx = if keep.is_empty() {
            false
        } else if expression {
            dims
        } else {
            expressions::_has_undefined_index(
                inst.args[0].as_ref(),
                known_view,
                Some(ds),
                Some(stmt_id(stmt)),
                &[],
            )
        };
        if !keep.is_empty() && undef_idx {
            let arg0_raw = expressions::part2::_arg_text(inst, 0);
            let arg0_stripped = pystr::strip(&arg0_raw);
            let msg = expressions::dimensions_message(
                arg0_stripped,
                profile,
                false,
                inst.base.at_eol.get(),
            );
            new_diags.push(Rc::new(Diagnostic::basic(
                "error",
                inst.base.node.line,
                &msg,
                &inst.base.node.file,
            )));
        } else if !keep.is_empty() && expression {
            new_diags.push(Rc::new(Diagnostic::basic(
                "error",
                inst.base.node.line,
                _DEST_MESSAGE,
                &inst.base.node.file,
            )));
        }
        if let Some(Some(_)) = inst.args.get(1) {
            let raw_1 = expressions::part2::_arg_text(inst, 1);
            let tok = super::super::instr_rules::_next_simple_token(&raw_1);
            let text = if tok.is_empty() { raw_1 } else { tok };
            new_diags.push(Rc::new(Diagnostic::basic(
                "error",
                inst.base.node.line,
                &format!("Too many parameters:  {text}."),
                &inst.base.node.file,
            )));
        }
        let mut updated: Vec<DiagRef> = diags.items[..at]
            .iter()
            .filter(|d| {
                d.line != inst.base.node.line
                    || d.file != inst.base.node.file
                    || d.severity != "error"
            })
            .cloned()
            .collect();
        updated.extend(new_diags);
        updated.extend(
            diags.items[at..]
                .iter()
                .filter(|d| {
                    d.line != inst.base.node.line
                        || d.file != inst.base.node.file
                        || d.severity != "error"
                })
                .cloned(),
        );
        diags.items = updated;
    }
}

const _ABORTING_PARAMS: &[i64] = &[2, 8, 24, 25, 26, 27, 28, 29, 33, 40, 41, 47];

/// Port of checker/cr3xx.py::drop_too_many_after_variable_illegal
pub fn drop_too_many_after_variable_illegal(
    program: &ProgramRef,
    profile: &Profile,
    diags: &mut Diagnostics,
) {
    if !matches!(profile.name, "cr300" | "cr350") {
        return;
    }
    let mut per_line: HashMap<(i64, String), usize> = HashMap::new();
    let mut declarations: HashSet<(i64, String)> = HashSet::new();
    for st in &program.statements {
        let key = (st.base().node.line, st.base().node.file.clone());
        *per_line.entry(key.clone()).or_insert(0) += 1;
        if matches!(&**st, Stmt::Declaration(_) | Stmt::SubDef(_)) {
            declarations.insert(key);
        }
    }
    let bad: HashSet<(i64, String)> = diags
        .errors()
        .into_iter()
        .filter(|d| {
            let key = (d.line, d.file.clone());
            d.message.starts_with(_VARIABLE_ILLEGAL_PREFIX)
                && per_line.get(&key).copied().unwrap_or(0) == 1
                && !declarations.contains(&key)
        })
        .map(|d| (d.line, d.file.clone()))
        .collect();
    if !bad.is_empty() {
        diags.items.retain(|d| {
            !(bad.contains(&(d.line, d.file.clone())) && d.message.starts_with(_TOO_MANY_PREFIX))
        });
    }
    let items = &mut diags.items;
    for i in (0..items.len()).rev() {
        let (line, file, is_too_many) = {
            let d = &items[i];
            (
                d.line,
                d.file.clone(),
                d.severity == "error" && d.message.starts_with(_TOO_MANY_PREFIX),
            )
        };
        if !is_too_many || per_line.get(&(line, file.clone())).copied().unwrap_or(0) != 1 {
            continue;
        }
        let last_later = ((i + 1)..items.len()).rfind(|&j| {
            items[j].line == line
                && items[j].file == file
                && items[j].message.ends_with(_UNRESOLVED_SUFFIX)
        });
        if let Some(target) = last_later {
            let item = items.remove(i);
            items.insert(target, item);
        }
    }
}

/// Port of `checker/cr3xx.py::_expr_names`.
pub fn _expr_names(expr: Option<&Expr>) -> Vec<String> {
    let Some(expr) = expr else {
        return Vec::new();
    };
    match expr {
        Expr::Name(n) => vec![n.name.clone()],
        Expr::Index(idx) => {
            let mut out = vec![idx.name.clone()];
            for a in &idx.args {
                out.extend(_expr_names(a.as_deref()));
            }
            out
        }
        Expr::Binary(b) => {
            let mut out = _expr_names(b.left.as_deref());
            out.extend(_expr_names(b.right.as_deref()));
            out
        }
        Expr::Unary(u) => _expr_names(u.operand.as_deref()),
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profiles;
    use crate::syntax::{
        Binary, ExprRef, Index, Instruction, Name, Node, Number, Param, StmtBase, StringLit,
        SubDef, Token, TokenValue, Unary,
    };

    fn name_expr(name: &str) -> ExprRef {
        Rc::new(Expr::Name(Name {
            node: Node::new(1, "a"),
            name: name.to_string(),
        }))
    }

    fn int_expr(v: i64) -> ExprRef {
        Rc::new(Expr::Number(Number {
            node: Node::new(1, "a"),
            value: NumberValue::Int(v),
            text: v.to_string(),
        }))
    }

    fn str_expr(v: &str) -> ExprRef {
        Rc::new(Expr::String(StringLit {
            node: Node::new(1, "a"),
            value: v.to_string(),
        }))
    }

    #[test]
    fn expr_names_and_is_bad_destination() {
        let expr = Expr::Binary(Binary {
            node: Node::new(1, "a"),
            op: "+".to_string(),
            left: Some(Rc::new(Expr::Index(Index {
                node: Node::new(1, "a"),
                name: "Arr".to_string(),
                args: vec![Some(name_expr("Idx"))],
            }))),
            right: Some(Rc::new(Expr::Unary(Unary {
                node: Node::new(1, "a"),
                op: "-".to_string(),
                operand: Some(name_expr("Off")),
            }))),
        });
        assert_eq!(_expr_names(Some(&expr)), vec!["Arr", "Idx", "Off"]);
        assert_eq!(_expr_names(None), Vec::<String>::new());

        let mut types = OrderedMap::new();
        types.insert("F".to_string(), "FLOAT".to_string());
        types.insert("L".to_string(), "LONG".to_string());
        assert_eq!(_is_bad_destination(&name_expr("f"), &types), Some(false));
        assert_eq!(_is_bad_destination(&name_expr("l"), &types), Some(true));
        assert_eq!(_is_bad_destination(&name_expr("u"), &types), None);
        assert_eq!(_is_bad_destination(&int_expr(1), &types), Some(true));
    }

    #[test]
    fn bare_unknown_delay_and_format_needs_com1_4() {
        let call = Call {
            base: StmtBase {
                node: Node::new(1, "a.cr6"),
                text: "SerialOpen(1,9600,0,UndefDelay,100)".to_string(),
                ..StmtBase::default()
            },
            name: "SerialOpen".to_string(),
            args: vec![
                Some(int_expr(1)),
                Some(int_expr(9600)),
                Some(int_expr(16)),
                Some(name_expr("UndefDelay")),
                Some(int_expr(100)),
            ],
            ..Call::default()
        };
        let mut known = BTreeSet::new();
        assert!(bare_unknown_delay(&call, &known));
        known.insert("UNDEFDELAY".to_string());
        assert!(!bare_unknown_delay(&call, &known));

        let cr6 = profiles::get("cr6").unwrap();
        assert!(_format_needs_com1_4(&call, cr6, None));
    }

    #[test]
    fn non_finite_and_commsmode_problem() {
        let cr6 = profiles::get("cr6").unwrap();
        assert_eq!(
            _non_finite(Some(&name_expr("INF")), cr6, None),
            Some("float".to_string())
        );
        let div0 = Expr::Binary(Binary {
            node: Node::new(1, "a.cr6"),
            op: "/".to_string(),
            left: Some(int_expr(1)),
            right: Some(int_expr(0)),
        });
        assert_eq!(
            _non_finite(Some(&div0), cr6, None),
            Some("divide".to_string())
        );

        let call = Call {
            base: StmtBase {
                node: Node::new(2, "a.cr6"),
                text: "SerialOpen(ComC1,9600,0,0,100,9)".to_string(),
                ..StmtBase::default()
            },
            name: "SerialOpen".to_string(),
            args: vec![
                Some(name_expr("ComC1")),
                Some(int_expr(9600)),
                Some(int_expr(0)),
                Some(int_expr(0)),
                Some(int_expr(100)),
                Some(int_expr(9)),
            ],
            ..Call::default()
        };
        assert_eq!(
            _commsmode_problem(&call, cr6, None, 5, None),
            Some((
                true,
                "CommsMode Option 9 not supported on ComC1.".to_string()
            ))
        );
    }

    #[test]
    fn check_serialopen_and_dropped_tail() {
        let cr300 = profiles::get("cr300").unwrap();
        let st_call = Rc::new(Stmt::Call(Call {
            base: StmtBase {
                node: Node::new(1, "a.cr300"),
                text: "SerialOpen(\"fuzz\",9600,0,0,100)".to_string(),
                at_eol: std::cell::Cell::new(false),
                ..StmtBase::default()
            },
            name: "SerialOpen".to_string(),
            args: vec![
                Some(str_expr("fuzz")),
                Some(int_expr(9600)),
                Some(int_expr(0)),
                Some(int_expr(0)),
                Some(int_expr(100)),
            ],
            ..Call::default()
        }));
        *st_call.base().dropped_tail.borrow_mut() = Some(vec![Token {
            kind: "IDENT".to_string(),
            text: "TailWord".to_string(),
            upper: "TAILWORD".to_string(),
            col: 0,
            line: 1,
            value: TokenValue::None,
        }]);
        let prog = Rc::new(Program::new("a.cr300", vec![st_call]));
        let mut diags = Diagnostics::new();
        check(&prog, cr300, &mut diags);
        assert_eq!(diags.items.len(), 2);
        assert_eq!(diags.items[0].message, "ComPort 0 illegal.");
        assert_eq!(
            diags.items[1].message,
            "Parameter must be a constant integer: SerialOpen(\"fuzz\",9600,0,0,100)."
        );

        let mut tail_diags = Diagnostics::new();
        check_dropped_tail(&prog, cr300, &mut tail_diags);
        assert_eq!(tail_diags.items.len(), 1);
        assert_eq!(tail_diags.items[0].message, "TailWord is not defined.");

        let cr6 = profiles::get("cr6").unwrap();
        let prog6 = Rc::new(Program::new(
            "a.cr6",
            vec![Rc::new(Stmt::Call(Call {
                base: StmtBase {
                    node: Node::new(2, "a.cr6"),
                    text: "SerialOpen(ComRS232,300,0,0,100)".to_string(),
                    ..StmtBase::default()
                },
                name: "SerialOpen".to_string(),
                args: vec![
                    Some(name_expr("ComRS232")),
                    Some(int_expr(300)),
                    Some(int_expr(0)),
                    Some(int_expr(0)),
                    Some(int_expr(100)),
                ],
                ..Call::default()
            }))],
        ));
        let mut diags6 = Diagnostics::new();
        check(&prog6, cr6, &mut diags6);
        assert!(
            diags6
                .items
                .iter()
                .any(|d| d.message == "300 is illegal baud rate.")
        );
    }

    #[test]
    fn destination_types_and_drop_too_many() {
        let cr300 = profiles::get("cr300").unwrap();
        let prog = Rc::new(Program::new(
            "a.cr300",
            vec![
                Rc::new(Stmt::SubDef(SubDef {
                    base: StmtBase {
                        node: Node::new(1, "a.cr300"),
                        ..StmtBase::default()
                    },
                    keyword: "Sub".to_string(),
                    name: "S".to_string(),
                    params: vec![Rc::new(Param {
                        name: "P".to_string(),
                        ..Param::default()
                    })],
                    ..SubDef::default()
                })),
                Rc::new(Stmt::Call(Call {
                    base: StmtBase {
                        node: Node::new(2, "a.cr300"),
                        ..StmtBase::default()
                    },
                    name: "Noop".to_string(),
                    ..Call::default()
                })),
            ],
        ));
        let (_types, scoped) = _destination_types(&prog, cr300);
        assert_eq!(scoped.len(), 2);

        let mut diags = Diagnostics::new();
        diags.error_basic(2, "Too many parameters:  bar.", "a.cr300");
        diags.error_basic(2, "foo is not defined.", "a.cr300");
        drop_too_many_after_variable_illegal(&prog, cr300, &mut diags);
        assert_eq!(diags.items[0].message, "foo is not defined.");
        assert_eq!(diags.items[1].message, "Too many parameters:  bar.");
    }

    #[test]
    fn cr350_comms_mode_invalid_format_and_float_dest() {
        let cr350 = profiles::get("cr350").unwrap();
        let env = Env::new(cr350);
        assert!(_invalid_cr3xx_serial_format(&int_expr(16), &env));
        assert!(!_invalid_cr3xx_serial_format(&int_expr(15), &env));

        let call = Call {
            base: StmtBase {
                node: Node::new(3, "a.cr350"),
                text: "SerialOpen(1,9600,0,0,100,2)".to_string(),
                ..StmtBase::default()
            },
            name: "SerialOpen".to_string(),
            args: vec![
                Some(int_expr(1)),
                Some(int_expr(9600)),
                Some(int_expr(0)),
                Some(int_expr(0)),
                Some(int_expr(100)),
                Some(int_expr(2)),
            ],
            ..Call::default()
        };
        let mut diags = Diagnostics::new();
        _check_cr350_comms_mode(
            &call,
            1,
            cr350,
            &OrderedMap::new(),
            &mut diags,
            false,
            false,
        );
        assert_eq!(diags.items.len(), 1);
        assert_eq!(
            diags.items[0].message,
            "2 is an invalid CommsMode parameter."
        );

        let cr300 = profiles::get("cr300").unwrap();
        let prog = Rc::new(Program::new(
            "a.cr300",
            vec![Rc::new(Stmt::Instruction(Instruction {
                base: StmtBase {
                    node: Node::new(1, "a.cr300"),
                    text: "Battery(1)".to_string(),
                    ..StmtBase::default()
                },
                keyword: "Battery".to_string(),
                args: vec![Some(int_expr(1))],
                ..Instruction::default()
            }))],
        ));
        let mut fdiags = Diagnostics::new();
        check_float_destinations(&prog, cr300, &mut fdiags);
        assert_eq!(fdiags.items.len(), 1);
        assert_eq!(fdiags.items[0].message, _DEST_MESSAGE);
    }
}
