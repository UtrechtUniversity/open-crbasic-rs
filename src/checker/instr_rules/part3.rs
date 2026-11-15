//! Port of `checker/instr_rules.py` (lines 1539-2311).

#![allow(dead_code, unused_variables, unused_imports)]
#![allow(
    clippy::used_underscore_items,
    clippy::float_cmp,
    clippy::nonminimal_bool
)]
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss
)]

use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;
use std::sync::LazyLock;

use super::part1::{_next_simple_token, _real_params, _required_count, given_count};
use super::part2::{_cdiv, _crem, _has_call, _stringish};
use crate::checker::expressions::{self, KnownView};
use crate::checker::{context, longconv, var_types};
use crate::consteval::{self, Env};
use crate::diagnostics::{DiagRef, Diagnostic, Diagnostics, ShowFile};
use crate::profiles::{self, Profile};
use crate::syntax::{
    ConstValue, Expr, ExprRef, Instruction, NumberValue, OrderedMap, Program, ProgramRef, Stmt,
    StmtBase, StmtRef, stmt_id,
};
use crate::{pyre, pystr};

// ---------------------------------------------------------------------------------------------
// Local helpers (not in Python)

/// Env, shared program and static profile for a rule stub whose signature lacks them.
fn _p3_rule_env(
    program: &Program,
    profile: &Profile,
) -> Option<(ProgramRef, &'static Profile, Env)> {
    let p = profiles::get(profile.name).ok()?;
    let prog = Rc::new(program.clone());
    let env = context::of(&prog, Some(p), None).env().clone();
    Some((prog, p, env))
}

/// The `Rc` of `target` among `program`'s statements (Python `id(stmt)`).
fn _p3_find_stmt<'a>(program: &'a Program, target: &Instruction) -> Option<&'a StmtRef> {
    program.statements.iter().find(|s| match s.as_ref() {
        Stmt::Instruction(i) => std::ptr::eq(i, target),
        _ => false,
    })
}

fn _p3_err(diags: &mut Diagnostics, base: &StmtBase, message: &str) {
    diags.error_basic(base.node.line, message, &base.node.file);
}

fn _p3_known_view(known: &expressions::KnownNames) -> KnownView<'_> {
    KnownView {
        names: &known.names,
        tables: &known.tables,
        out_of_scope: &known.out_of_scope,
        dotted_tables: known.dotted_tables,
    }
}

fn _p3_f64(value: &ConstValue) -> Option<f64> {
    match value {
        ConstValue::Int(i) => Some(*i as f64),
        ConstValue::Float(f) | ConstValue::CFloat(f) => Some(*f),
        ConstValue::BigInt(s) => pystr::parse_float(s),
        ConstValue::Str(_) => None,
    }
}

/// Python `struct.unpack("f", struct.pack("f", value))[0]` with the overflow case saturating to
/// +-inf (`math.copysign(math.inf, value)`).
fn _p3_float32(value: f64) -> f64 {
    f64::from(value as f32)
}

/// Placeholder for `instr_rules.py::shortfall_message(record, given)` (part1 stub lacks `record`).
fn _p3_shortfall_message(record: &profiles::Instruction, given: i64) -> Option<String> {
    todo!("instr_rules/part1 shortfall_message_with(record: &profiles::Instruction, given: i64)")
}

/// Placeholder for `instr_rules.py::_earlier_error_in_stmt(program, stmt, diags, prefixes)`.
fn _p3_earlier_error_in_stmt(
    program: &Program,
    stmt: &StmtRef,
    diags: &Diagnostics,
    prefixes: &[&str],
) -> bool {
    todo!(
        "instr_rules/part1 _earlier_error_in_stmt_with(program: &Program, stmt: &StmtRef, diags: &Diagnostics, prefixes: &[&str])"
    )
}

/// `instr_rules.py::_PRECISION_VARIABLE_POSITIONS` (a part2 constant, not yet ported).
fn _p3_precision_positions(kw: &str) -> Option<&'static [usize]> {
    match kw {
        "ADDPRECISE" | "MOVEPRECISE" => Some(&[0]),
        _ => None,
    }
}

// ---------------------------------------------------------------------------------------------

/// Port of `checker/instr_rules.py::check_precision_variable`.
pub fn check_precision_variable(
    stmt: &Instruction,
    program: &Program,
    profile: &Profile,
    diags: &mut Diagnostics,
) {
    if let Some((prog, p, env)) = _p3_rule_env(program, profile) {
        check_precision_variable_with(stmt, &prog, p, diags, &env);
    }
}

/// Port of `checker/instr_rules.py::check_precision_variable` (full parameter list).
pub fn check_precision_variable_with(
    stmt: &Instruction,
    program: &ProgramRef,
    profile: &'static Profile,
    diags: &mut Diagnostics,
    env: &Env,
) {
    let kw = pystr::upper(&stmt.keyword);
    let Some(positions) = _p3_precision_positions(&kw) else {
        return;
    };
    if !profile.instructions().contains(&kw) {
        return;
    }
    let ctx = context::of(program, Some(profile), Some(env.clone()));
    let known = ctx.known_names();
    let variable_names = ctx.declared_variable_names();
    let Some(record) = profile.instructions().get(&kw) else {
        return;
    };
    if given_count(stmt, profile) < _required_count(_real_params(record.params.clone()))
        && matches!(profile.name, "cr6" | "cr1000x")
    {
        return;
    }
    for &i in positions {
        if i >= stmt.args.len() || stmt.args[i].is_none() {
            continue;
        }
        let arg = stmt.args[i].as_deref();
        if !expressions::_undefined_names_with(arg, &_p3_known_view(known)).is_empty()
            || !expressions::_variable_refs(arg, variable_names).is_empty()
        {
            continue;
        }
        if _has_call(arg, variable_names) || _stringish(arg, ctx.string_consts()) {
            continue;
        }
        _p3_err(
            diags,
            &stmt.base,
            &format!(
                "Invalid Variable or Array: {}.",
                expressions::_arg_text(stmt, i as i64)
            ),
        );
    }
}

/// Port of `checker/instr_rules.py::check_vibrating_wire_diagnostics`.
pub fn check_vibrating_wire_diagnostics(
    stmt: &Instruction,
    program: &Program,
    profile: &Profile,
    diags: &mut Diagnostics,
) {
    if let Some((prog, p, env)) = _p3_rule_env(program, profile) {
        check_vibrating_wire_diagnostics_with(stmt, &prog, p, diags, &env);
    }
}

/// Port of `checker/instr_rules.py::check_vibrating_wire_diagnostics` (full parameter list).
pub fn check_vibrating_wire_diagnostics_with(
    stmt: &Instruction,
    program: &ProgramRef,
    profile: &'static Profile,
    diags: &mut Diagnostics,
    env: &Env,
) {
    if profile.name != "cr6" || stmt.args.len() <= 7 {
        return;
    }
    let ctx = context::of(program, Some(profile), Some(env.clone()));
    let strings = &ctx.dim_counts(false).strings;
    let variables = ctx.declared_variable_names();
    let arg = stmt.args[7].as_deref();
    let named = match arg {
        Some(Expr::Name(n)) => Some(&n.name),
        Some(Expr::Index(ix)) => Some(&ix.name),
        _ => None,
    };
    if let Some(name) = named
        && variables.contains(&pystr::upper(name))
        && !strings.contains(&pystr::upper(name))
    {
        _p3_err(diags, &stmt.base, "Invalid diagnostic parameter.");
    } else if let Some(Expr::Number(n)) = arg {
        _p3_err(
            diags,
            &stmt.base,
            &format!("Variable name {} illegal, starts with a number.", n.text),
        );
    }
}

/// `instr_rules.py::_SDM_ADDRESS_ARG`.
pub const _SDM_ADDRESS_ARG: &[(&str, usize)] = &[
    ("CS7500", 2),
    ("CSAT3", 2),
    ("CSAT3B", 2),
    ("CSAT3BMONITOR", 2),
    ("EC100", 1),
    ("EC100CONFIGURE", 1),
    ("LI7200", 2),
    ("LI7700", 2),
    ("SDMCD16AC", 2),
    ("SDMCD16MASK", 2),
    ("SDMGENERIC", 1),
    ("SDMIO16", 2),
    ("SDMSIO4", 2),
    ("SDMSW8A", 2),
    ("SDMX50", 0),
    ("TDR100", 1),
    ("TDR200", 1),
];

/// Port of `checker/instr_rules.py::check_sdm_address`.
pub fn check_sdm_address(
    stmt: &Instruction,
    program: &Program,
    profile: &Profile,
    diags: &mut Diagnostics,
) {
    if let Some((prog, p, env)) = _p3_rule_env(program, profile) {
        check_sdm_address_with(stmt, &prog, p, diags, &env);
    }
}

/// Port of `checker/instr_rules.py::check_sdm_address` (full parameter list).
pub fn check_sdm_address_with(
    stmt: &Instruction,
    program: &ProgramRef,
    profile: &'static Profile,
    diags: &mut Diagnostics,
    env: &Env,
) {
    let kw = pystr::upper(&stmt.keyword);
    let Some(&(_, pos)) = _SDM_ADDRESS_ARG.iter().find(|(k, _)| *k == kw) else {
        return;
    };
    if !profile.instructions().contains(&kw) || pos >= stmt.args.len() {
        return;
    }
    let arg = stmt.args[pos].as_deref();
    if arg.is_none() || !consteval::is_constant(arg, env) {
        return;
    }
    let Ok(value) = consteval::value_of(arg, env) else {
        return;
    };
    let Some(mut vf) = _p3_f64(&value) else {
        return;
    };
    let mut exact = match &value {
        ConstValue::Int(i) => Some(*i),
        _ => None,
    };
    let is_float = consteval::is_float(arg, env).unwrap_or_else(|| match &value {
        ConstValue::Int(_) => false,
        ConstValue::Float(f) | ConstValue::CFloat(f) => {
            f.abs().partial_cmp(&9_223_372_036_854_775_808.0) != Some(std::cmp::Ordering::Less)
                || f.fract() != 0.0
        }
        _ => true,
    });
    let long_range = |v: f64| (-2_147_483_648.0..2_147_483_648.0).contains(&v);
    if is_float || !long_range(vf) {
        let rounded = _p3_float32(vf);
        vf = if rounded.is_infinite() {
            if rounded > 0.0 { 2_147_483_647.0 } else { 0.0 }
        } else {
            rounded
        };
        exact = None;
        if !long_range(vf) {
            return;
        }
    }
    let address = exact.unwrap_or(vf.trunc() as i64) & 0xFF;
    if address >= 15 {
        _p3_err(
            diags,
            &stmt.base,
            &format!("Invalid SDM Address: {address}, must be 0 - 14."),
        );
    }
}

const _AVGRUN_TYPE_ERROR: &[&str] = &["FLOAT", "STRING"];
const _RUNNING_AVG: &[&str] = &["AVGRUN", "TOTALRUN", "MINRUN", "MAXRUN", "STDDEVRUN"];

fn _instances_at(kw: &str) -> Option<usize> {
    match kw {
        "AVGRUN" | "TOTALRUN" | "STDDEVRUN" => Some(6),
        "MINRUN" | "MAXRUN" => Some(5),
        _ => None,
    }
}

/// Port of `checker/instr_rules.py::_operator_tree`.
pub fn _operator_tree(expr: Option<&Expr>) -> bool {
    match expr {
        Some(Expr::Unary(u)) => _operator_tree(u.operand.as_deref()),
        Some(Expr::Binary(b)) => {
            _operator_tree(b.left.as_deref()) && _operator_tree(b.right.as_deref())
        }
        Some(Expr::Number(_) | Expr::String(_) | Expr::Name(_)) => true,
        _ => false,
    }
}

/// Port of `checker/instr_rules.py::check_avgrun_options`.
pub fn check_avgrun_options(
    stmt: &Instruction,
    program: &Program,
    profile: &Profile,
    diags: &mut Diagnostics,
) {
    if let Some((prog, p, env)) = _p3_rule_env(program, profile) {
        check_avgrun_options_with(stmt, &prog, p, diags, &env);
    }
}

/// Port of `checker/instr_rules.py::check_avgrun_options` (full parameter list).
pub fn check_avgrun_options_with(
    stmt: &Instruction,
    program: &ProgramRef,
    profile: &'static Profile,
    diags: &mut Diagnostics,
    env: &Env,
) {
    let kw = pystr::upper(&stmt.keyword);
    if !profile.instructions().contains(&kw) {
        return;
    }
    let ctx = context::of(program, Some(profile), Some(env.clone()));
    let scopes = ctx.statement_scopes();
    let names = ctx.float_names();
    let literals = ctx.literal_names();
    let Some(stmt_ref) = _p3_find_stmt(program, stmt) else {
        return;
    };

    let variable_type = |arg: Option<&Expr>, view: &ProgramRef| -> Option<String> {
        let name = match arg {
            Some(Expr::Name(n)) => &n.name,
            Some(Expr::Index(ix)) => &ix.name,
            _ => return None,
        };
        context::of(view, Some(profile), Some(env.clone()))
            .variable_types(false)
            .get(&pystr::upper(name))
            .cloned()
    };
    let number = |arg: Option<&Expr>| -> Option<i64> {
        if let Some(converted) = longconv::long_value_with(arg, env, profile, names, literals) {
            return Some(converted);
        }
        let value = consteval::value_of(arg, env).ok()?;
        Some(consteval::to_long(&value))
    };

    let mut args: Vec<Option<ExprRef>> = stmt.args.clone();
    while args.len() < 8 {
        args.push(None);
    }
    let view = ctx.scoped_view(stmt_ref, true);
    let type_error =
        |t: Option<String>| t.is_some_and(|t| _AVGRUN_TYPE_ERROR.contains(&t.as_str()));
    if matches!(kw.as_str(), "TOTALRUN" | "STDDEVRUN")
        && args[4].is_some()
        && type_error(variable_type(args[5].as_deref(), &view))
    {
        _p3_err(
            diags,
            &stmt.base,
            "Optional Count parameter must be type long.",
        );
    }
    if kw == "AVGRUN" && args[4].is_some() && type_error(variable_type(args[5].as_deref(), &view)) {
        _p3_err(
            diags,
            &stmt.base,
            "Optional Count parameter must be type long.",
        );
    }
    if let Some(at) = _instances_at(&kw)
        && args[4].is_some()
        && args[at].is_some()
        && profile
            .instructions()
            .get(&kw)
            .is_some_and(|r| r.params.len() > at + 1)
    {
        let view_ctx = context::of(&view, Some(profile), Some(env.clone()));
        let mut visible: BTreeSet<String> = view_ctx.declared_variable_names().clone();
        visible.extend(view_ctx.alias_names().iter().cloned());
        if let Some((own, _)) = scopes.get(&stmt_id(stmt_ref)) {
            visible.extend(own.iter().cloned());
        }
        let at_arg = args[at].as_deref();
        let variable = !expressions::_variable_refs(at_arg, &visible).is_empty()
            || expressions::_has_dotted_field(at_arg);
        let value = consteval::value_of(at_arg, env).ok();
        let mut constant =
            !variable && !matches!(value, Some(ConstValue::Str(_))) && _operator_tree(at_arg);
        let instances = number(at_arg);
        if constant
            && instances.is_none()
            && !expressions::_undefined_names_with(at_arg, &_p3_known_view(ctx.known_names()))
                .is_empty()
        {
            constant = false;
        }
        if constant && instances.is_some_and(|n| n < 0) {
            _p3_err(diags, &stmt.base, "Out of memory.");
            return;
        }
        if !constant || variable_type(at_arg, &view).is_some() || instances == Some(0) {
            _p3_err(
                diags,
                &stmt.base,
                "Optional number of distinct instances must be a constant of type long.",
            );
        }
        if args[at + 1].is_none() {
            _p3_err(diags, &stmt.base, "Missing instance number");
        }
    }
    let reps = number(args[1].as_deref());
    let count = number(args[3].as_deref());
    if let (Some(reps), Some(count)) = (reps, count)
        && (reps < 1 || count < 1)
    {
        _p3_err(
            diags,
            &stmt.base,
            &format!("number of averages {count} and reps {reps} in RunningAvg must be > 0."),
        );
    }
}

const _CDM_SETTLING_ARG: &[(&str, usize)] = &[("CDM_TCSE", 9), ("CDM_VOLTSE", 7)];

/// Port of `checker/instr_rules.py::check_option_values`.
pub fn check_option_values(program: &Program, profile: &Profile, diags: &mut Diagnostics) {
    if let Some((prog, p, env)) = _p3_rule_env(program, profile) {
        check_option_values_with(&prog, p, diags, &env);
    }
}

/// Port of `checker/instr_rules.py::check_option_values` (full parameter list).
pub fn check_option_values_with(
    program: &ProgramRef,
    profile: &'static Profile,
    diags: &mut Diagnostics,
    env: &Env,
) {
    let ctx = context::of(program, Some(profile), Some(env.clone()));
    let names = ctx.float_names();
    let literals = ctx.literal_names();
    let cr3 = matches!(profile.name, "cr300" | "cr350");
    let mut matrix_shapes: Option<OrderedMap<String, (i64, i64)>> = None;
    let mut seen_beginprog = false;
    let supported = |kw: &str| {
        matches!(
            kw,
            "SPLITSTR"
                | "IPROUTE"
                | "MATRIX"
                | "CDM_DELAY"
                | "MOVEBYTES"
                | "TRIGGERSEQUENCE"
                | "SDMSW8A"
                | "TIMEDCONTROL"
        ) || _CDM_SETTLING_ARG.iter().any(|(k, _)| *k == kw)
    };
    for st in &program.statements {
        let (kw, args, base, instr): (
            String,
            &Vec<Option<ExprRef>>,
            &StmtBase,
            Option<&Instruction>,
        ) = match st.as_ref() {
            Stmt::Instruction(i) => (pystr::upper(&i.keyword), &i.args, &i.base, Some(i)),
            Stmt::Call(c) => (pystr::upper(&c.name), &c.args, &c.base, None),
            _ => continue,
        };
        if kw == "BEGINPROG" {
            seen_beginprog = true;
        }
        if !supported(&kw) || !profile.instructions().contains(&kw) {
            continue;
        }
        let value = |i: usize| -> Option<i64> {
            if i < args.len() {
                longconv::long_value_with(args[i].as_deref(), env, profile, names, literals)
            } else {
                None
            }
        };
        let arg_is_named = |a: Option<&Expr>| match a {
            Some(Expr::Name(n)) => Some(n.name.clone()),
            Some(Expr::Index(ix)) => Some(ix.name.clone()),
            _ => None,
        };
        match kw.as_str() {
            "SPLITSTR" => {
                let v = value(4);
                let dest = args.first().and_then(|a| a.as_deref());
                let empty_index = cr3
                    && instr.is_some()
                    && matches!(dest, Some(Expr::Index(ix)) if ix.args.is_empty());
                if let Some(v) = v
                    && v < 0
                {
                    _p3_err(diags, base, &format!("Invalid option {v}"));
                } else if let Some(dest_name) = arg_is_named(dest)
                    && (empty_index || v.is_some_and(|v| v >= 10))
                {
                    let scope = context::of(program, None, None).scoped_view(st, false);
                    let typer = var_types::Typer::new(program, Some(&scope), Some(profile));
                    if typer
                        .types
                        .get(&pystr::upper(&dest_name))
                        .is_some_and(|t| typer.structure_types.contains_key(t))
                    {
                        if empty_index {
                            let Some(instr) = instr else { continue };
                            _p3_err(
                                diags,
                                base,
                                &format!(
                                    "Invalid, or out of place expression: {}.",
                                    pystr::strip(&expressions::_arg_text(instr, 0))
                                ),
                            );
                        } else {
                            _p3_err(
                                diags,
                                base,
                                "Justify option (option >= 10) is illegal when spliting into a structure",
                            );
                        }
                    }
                }
            }
            "MOVEBYTES" => {
                let mut v = value(5);
                if args.len() > 5
                    && matches!(args[5].as_deref(), Some(Expr::Name(n)) if pystr::upper(&n.name) == "INF")
                {
                    v = Some(2_147_483_647);
                }
                if let Some(v) = v
                    && v & 0x7FFF > 4
                {
                    _p3_err(
                        diags,
                        base,
                        &format!("Invalid transfer option {v}, must be 0..4."),
                    );
                }
            }
            "TRIGGERSEQUENCE" => {
                if let Some(evt) = value(0)
                    && !(1..=4).contains(&evt.rem_euclid(256))
                {
                    _p3_err(diags, base, "Illegal EVT, 1..4 are legal");
                }
            }
            "SDMSW8A" => {
                if let Some(option) = value(3)
                    && option.rem_euclid(256) > 3
                {
                    _p3_err(diags, base, "Invalid function option in SDMSW8A.");
                }
            }
            "TIMEDCONTROL" => {
                if let Some(mode) = value(6)
                    && !(1..=(if seen_beginprog { 2 } else { 3 })).contains(&mode)
                {
                    let message = if seen_beginprog {
                        "Invalid reset mode, must be 1..2."
                    } else {
                        "Invalid clock set mode, must be 1..3."
                    };
                    _p3_err(diags, base, message);
                }
            }
            k if _CDM_SETTLING_ARG.iter().any(|(n, _)| *n == k) => {
                let at = _CDM_SETTLING_ARG
                    .iter()
                    .find(|(n, _)| *n == k)
                    .map_or(0, |p| p.1);
                if let Some(v) = value(at)
                    && v != 0
                    && !(100..=100_000).contains(&v)
                {
                    _p3_err(
                        diags,
                        base,
                        &format!(
                            "Settling time: {v}, out of range (100 - 100000, or 0 for default)."
                        ),
                    );
                }
            }
            "CDM_DELAY" => {
                let v = value(2);
                if let Some(v) = v
                    && !matches!(v.rem_euclid(256), 0 | 1)
                {
                    _p3_err(
                        diags,
                        base,
                        &format!("Illegal option {}", v.rem_euclid(256)),
                    );
                } else if let Some(v) = v
                    && v.rem_euclid(256) == 0
                {
                    for _ in 0.._cdm_delay_overlong_with(args, env, profile, names, literals) {
                        _p3_err(diags, base, "Measurement Delay Exceeds 60 min.");
                    }
                }
            }
            "MATRIX" => {
                let v = value(0);
                if let Some(v) = v
                    && !(1..=5).contains(&(v & 0x7FFF_FFFF))
                {
                    _p3_err(diags, base, "Illegal option.");
                } else if let Some(v) = v
                    && args.len() == 4
                {
                    let shapes =
                        matrix_shapes.get_or_insert_with(|| _matrix_shapes_with(program, env));
                    for message in _matrix_dimension_errors_with(v & 0x7FFF_FFFF, args, shapes) {
                        _p3_err(diags, base, &message);
                    }
                }
            }
            _ => {
                if args
                    .iter()
                    .flatten()
                    .all(|a| consteval::is_constant(Some(a), env))
                {
                    let net = value(1);
                    if let Some(net) = net
                        && !(0..=(if cr3 { 4 } else { 5 })).contains(&net)
                    {
                        _p3_err(diags, base, &format!("{net} is illegal network"));
                        continue;
                    }
                    if args.len() > 2 && matches!(args[2].as_deref(), Some(Expr::String(_))) {
                        continue;
                    }
                    if let Some(opt) = value(2)
                        && !matches!(opt, 0 | 1)
                    {
                        _p3_err(diags, base, &format!("{opt} is illegal option"));
                    }
                }
            }
        }
    }
}

/// Port of `checker/instr_rules.py::_cdm_delay_overlong`.
pub fn _cdm_delay_overlong(_profile: &Profile) -> i64 {
    todo!(
        "checker/instr_rules.py::_cdm_delay_overlong (omitted: args: list, env: consteval.Env, names: untyped, literals: untyped); use _cdm_delay_overlong_with"
    )
}

/// Port of `checker/instr_rules.py::_cdm_delay_overlong` (full parameter list).
pub fn _cdm_delay_overlong_with(
    args: &[Option<ExprRef>],
    env: &Env,
    profile: &Profile,
    names: &BTreeSet<String>,
    literals: &BTreeSet<String>,
) -> i64 {
    if args.len() < 5 || args[3].is_none() || args[4].is_none() {
        return 0;
    }
    let (Ok(raw), Ok(unit)) = (
        consteval::value_of(args[3].as_deref(), env),
        consteval::value_of(args[4].as_deref(), env),
    ) else {
        return 0;
    };
    let Some(raw_f) = _p3_f64(&raw) else {
        return 0;
    };
    let Some(unit_f) = _p3_f64(&unit) else {
        return 0;
    };
    if raw_f.fract() != 0.0
        || !raw_f.is_finite()
        || !(-2_147_483_648.0..2_147_483_648.0).contains(&raw_f)
        || !(unit_f.fract() == 0.0 && (0.0..=5.0).contains(&unit_f))
    {
        return 0;
    }
    let unit = unit_f as i64;
    let Some(n) = longconv::long_value_with(args[3].as_deref(), env, profile, names, literals)
    else {
        return 0;
    };
    let (sec, us) = match unit {
        0 => (_cdiv(n, 1_000_000), _crem(n, 1_000_000)),
        1 => (_cdiv(n, 1000), _crem(n, 1000) * 1000),
        2 => (n, 0),
        _ => {
            let mult = match unit {
                3 => 60,
                4 => 3600,
                _ => 86400,
            };
            let wrapped = (n * mult) & 0xFFFF_FFFF;
            (
                if wrapped >= 1 << 31 {
                    wrapped - (1 << 32)
                } else {
                    wrapped
                },
                0,
            )
        }
    };
    let mut count = 0;
    if sec < 0 || us < 0 || sec > 3600 || (sec == 3600 && us > 0) {
        count += 1;
    }
    if ((us + sec * 1_000_000) & 0xFFFF_FFFF) > 3_600_000_000 {
        count += 1;
    }
    count
}

/// `instr_rules.py::_ILLEGAL_MATRIX`.
pub const _ILLEGAL_MATRIX: (i64, i64) = (-1, -1);
/// `instr_rules.py::_UNDEFINED_MATRIX`.
pub const _UNDEFINED_MATRIX: (i64, i64) = (-842_150_451, -842_150_451);

/// Port of `checker/instr_rules.py::_matrix_shapes`.
pub fn _matrix_shapes(_program: &Program) -> OrderedMap<String, (i64, i64)> {
    todo!(
        "checker/instr_rules.py::_matrix_shapes (omitted: env: consteval.Env); use _matrix_shapes_with"
    )
}

/// Port of `checker/instr_rules.py::_matrix_shapes` (full parameter list).
pub fn _matrix_shapes_with(program: &ProgramRef, env: &Env) -> OrderedMap<String, (i64, i64)> {
    let ctx = context::of(program, None, Some(env.clone()));
    let dims = ctx.declared_all_dims();
    let types = ctx.variable_types(false);
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut repeated: BTreeSet<String> = BTreeSet::new();
    for st in &program.statements {
        if let Stmt::Declaration(d) = st.as_ref()
            && matches!(d.kind.as_str(), "PUBLIC" | "DIM" | "STATIC")
        {
            for v in &d.vars {
                let up = pystr::upper(&v.name);
                if seen.contains(&up) {
                    repeated.insert(up);
                } else {
                    seen.insert(up);
                }
            }
        }
    }
    let mut out = OrderedMap::new();
    for (n, d) in dims.iter() {
        if d.len() <= 3 && types.get(n).is_some_and(|t| t == "FLOAT") && !repeated.contains(n) {
            let shape = if d.len() > 2 {
                _ILLEGAL_MATRIX
            } else {
                (
                    d.first().copied().unwrap_or(1),
                    if d.len() == 2 { d[1] } else { 1 },
                )
            };
            out.insert(n.clone(), shape);
        }
    }
    out
}

/// Port of `checker/instr_rules.py::matrix_undefined_operands`.
pub fn matrix_undefined_operands(_program: &Program, _profile: &Profile) -> Option<Vec<String>> {
    todo!(
        "checker/instr_rules.py::matrix_undefined_operands (omitted: stmt: untyped, env: untyped, known: untyped); use matrix_undefined_operands_with"
    )
}

/// Port of `checker/instr_rules.py::matrix_undefined_operands` (full parameter list).
pub fn matrix_undefined_operands_with(
    stmt: &Instruction,
    program: &ProgramRef,
    env: &Env,
    known: &BTreeSet<String>,
    profile: &'static Profile,
) -> Option<Vec<String>> {
    let args = &stmt.args;
    if args.len() != 4 || args.iter().any(Option::is_none) {
        return None;
    }
    let ctx = context::of(program, Some(profile), Some(env.clone()));
    let mut option = longconv::long_value_with(
        args[0].as_deref(),
        env,
        profile,
        ctx.float_names(),
        ctx.literal_names(),
    )?;
    if !(1..=5).contains(&(option & 0x7FFF_FFFF)) {
        return None;
    }
    option &= 0x7FFF_FFFF;
    let ops: Vec<&Option<ExprRef>> = if option <= 3 {
        vec![&args[1], &args[2], &args[3]]
    } else {
        vec![&args[1], &args[3]]
    };
    if option > 3 && !matches!(args[2].as_deref(), Some(Expr::Number(_) | Expr::Name(_))) {
        return None;
    }
    let shapes = _matrix_shapes_with(program, env);
    let mut out: Vec<String> = Vec::new();
    let mut shape: Vec<(i64, i64)> = Vec::new();
    for a in ops {
        let Some(Expr::Name(n)) = a.as_deref() else {
            return None;
        };
        let up = pystr::upper(&n.name);
        if known.contains(&up) {
            shape.push(*shapes.get(&up)?);
        } else {
            out.push(format!("Invalid symbol in expression: {}.", n.name));
            out.push("Illegal Matrix".to_string());
            shape.push(_UNDEFINED_MATRIX);
        }
    }
    if out.is_empty() {
        return None;
    }
    let result = *shape.last()?;
    if option == 3 {
        let (m, n) = shape[0];
        let (p, q) = shape[1];
        if n != p {
            out.push("multiply A columns and B rows must be the same".to_string());
        }
        if result == _UNDEFINED_MATRIX || (result.0 != m && result.1 != q) {
            out.push(format!(
                "multiply result illegal dimensioned, must be {m},{q}"
            ));
        }
    } else {
        out.push("Illegal Matrix dimensioned".to_string());
    }
    Some(out)
}

/// Port of `checker/instr_rules.py::_matrix_dimension_errors`.
pub fn _matrix_dimension_errors(_option: i64) -> Vec<String> {
    todo!(
        "checker/instr_rules.py::_matrix_dimension_errors (omitted: args: untyped, shapes: untyped); use _matrix_dimension_errors_with"
    )
}

/// Port of `checker/instr_rules.py::_matrix_dimension_errors` (full parameter list).
pub fn _matrix_dimension_errors_with(
    option: i64,
    args: &[Option<ExprRef>],
    shapes: &OrderedMap<String, (i64, i64)>,
) -> Vec<String> {
    let at = |i: usize| args.get(i).and_then(|a| a.as_deref());
    let ops: Vec<Option<&Expr>> = if option <= 3 {
        vec![at(1), at(2), at(3)]
    } else {
        vec![at(1), at(3)]
    };
    let mut shape: Vec<(i64, i64)> = Vec::new();
    for (i, a) in ops.iter().enumerate() {
        if (matches!(a, Some(Expr::String(_))) && i + 1 < ops.len())
            || (matches!(a, Some(Expr::Number(_))) && option <= 3 && i == 1)
        {
            shape.push(_ILLEGAL_MATRIX);
        } else if let Some(Expr::Name(n)) = a
            && let Some(s) = shapes.get(&pystr::upper(&n.name))
        {
            shape.push(*s);
        } else {
            return Vec::new();
        }
    }
    let mut out: Vec<String> = Vec::new();
    if shape.contains(&_ILLEGAL_MATRIX) {
        out.push("Illegal Matrix".to_string());
    }
    for sh in &shape {
        if sh.0.max(sh.1) > 4 {
            out.push("Matrix cannot be > 4X4".to_string());
        }
    }
    let result = shape[shape.len() - 1];
    if option == 3 {
        let (m, n) = shape[0];
        let (p, q) = shape[1];
        if n != p {
            out.push("multiply A columns and B rows must be the same".to_string());
        }
        if result.0 != m && result.1 != q {
            out.push(format!(
                "multiply result illegal dimensioned, must be {m},{q}"
            ));
        }
    } else if shape.iter().any(|sh| *sh != shape[0]) || (option > 3 && shape[0].0 != shape[0].1) {
        out.push("Illegal Matrix dimensioned".to_string());
    }
    out
}

/// Port of `checker/instr_rules.py::check_const_int_args`.
pub fn check_const_int_args(program: &Program, profile: &Profile, diags: &mut Diagnostics) {
    if let Some((prog, p, env)) = _p3_rule_env(program, profile) {
        check_const_int_args_with(&prog, p, diags, &env);
    }
}

static _P3_MINUS_SPACE_RE: LazyLock<pyre::Regex> = LazyLock::new(|| pyre::Regex::new(r"-\s", 0));

/// Port of `checker/instr_rules.py::check_const_int_args` (full parameter list).
pub fn check_const_int_args_with(
    program: &ProgramRef,
    profile: &'static Profile,
    diags: &mut Diagnostics,
    env: &Env,
) {
    for st in &program.statements {
        let Stmt::Instruction(stmt) = st.as_ref() else {
            continue;
        };
        let kw = pystr::upper(&stmt.keyword);
        let Some(record) = profile.instructions().get(&kw) else {
            continue;
        };
        if matches!(kw.as_str(), "DO" | "LOOP" | "ELSE") {
            continue;
        }
        if !profile.single_pass
            && _p3_earlier_error_in_stmt(
                program,
                st,
                diags,
                &[
                    "Invalid symbol in expression",
                    "Variable illegal in Parameter",
                ],
            )
        {
            continue;
        }
        for (i, &code) in record.params.iter().take(stmt.args.len()).enumerate() {
            let arg = stmt.args[i].as_deref();
            if !matches!(code, 4 | 104) || arg.is_none() {
                continue;
            }
            let text = _strip_parens(pystr::strip(&expressions::_arg_text(stmt, i as i64)));
            if text.is_empty() {
                continue;
            }
            if let Some(a) = arg
                && _is_integer_token_with(&text, a, env)
            {
                continue;
            }
            let echo = if _P3_MINUS_SPACE_RE.match_(&text).is_some() {
                "-"
            } else {
                text.as_str()
            };
            diags.error(
                stmt.base.node.line,
                &format!("Illegal parameter {echo}, expecting constant integer."),
                &stmt.base.node.file,
                ShowFile::Bool(true),
                stmt_id(st),
                "",
                "",
            );
        }
    }
}

/// Port of `checker/instr_rules.py::_strip_parens`.
pub fn _strip_parens(text: &str) -> String {
    let mut text = text.to_string();
    while text.starts_with('(')
        && text.ends_with(')')
        && _paren_depth_ok(pystr::slice(&text, Some(1), Some(-1)))
    {
        text = pystr::strip(pystr::slice(&text, Some(1), Some(-1))).to_string();
    }
    text
}

/// Port of `checker/instr_rules.py::_paren_depth_ok`.
pub fn _paren_depth_ok(text: &str) -> bool {
    let mut depth: i64 = 0;
    for ch in text.chars() {
        depth += i64::from(ch == '(') - i64::from(ch == ')');
        if depth < 0 {
            return false;
        }
    }
    depth == 0
}

static _P3_INTEGER_TOKEN_RE: LazyLock<pyre::Regex> =
    LazyLock::new(|| pyre::Regex::new(r"(?:\+?\d+|-?(?:\d+|&[Hh][0-9A-Fa-f]+|&[Bb][01]+))", 0));
static _P3_IDENT_RE: LazyLock<pyre::Regex> =
    LazyLock::new(|| pyre::Regex::new(r"[A-Za-z_][A-Za-z0-9_]*", 0));

/// Port of `checker/instr_rules.py::_is_integer_token`.
pub fn _is_integer_token(_text: &str, _arg: &Expr) -> bool {
    todo!(
        "checker/instr_rules.py::_is_integer_token (omitted: env: consteval.Env); use _is_integer_token_with"
    )
}

/// Port of `checker/instr_rules.py::_is_integer_token` (full parameter list).
pub fn _is_integer_token_with(text: &str, arg: &Expr, env: &Env) -> bool {
    if _P3_INTEGER_TOKEN_RE.fullmatch(text).is_some() {
        return true;
    }
    if _P3_IDENT_RE.fullmatch(text).is_none() || !consteval::is_constant(Some(arg), env) {
        return false;
    }
    match consteval::value_of(Some(arg), env) {
        Ok(ConstValue::Int(_) | ConstValue::BigInt(_)) => true,
        Ok(ConstValue::Float(f) | ConstValue::CFloat(f)) => f.is_finite() && f.fract() == 0.0,
        _ => false,
    }
}

/// `instr_rules.py::_SERIALIN_PORTS`: the ports of a logger, `None` for CR300/CR350.
fn _serialin_ports(profile_name: &str) -> Option<Vec<i64>> {
    match profile_name {
        "cr6" => Some((1..19).chain(32..48).collect()),
        "cr1000x" => Some((1..15).chain(32..48).collect()),
        _ => None,
    }
}

/// Port of `checker/instr_rules.py::check_serialbrk_comport`.
pub fn check_serialbrk_comport(
    stmt: &Instruction,
    program: &Program,
    profile: &Profile,
    diags: &mut Diagnostics,
) {
    if let Some((prog, p, env)) = _p3_rule_env(program, profile) {
        check_serialbrk_comport_with(stmt, &prog, p, diags, &env);
    }
}

/// Port of `checker/instr_rules.py::check_serialbrk_comport` (full parameter list).
pub fn check_serialbrk_comport_with(
    stmt: &Instruction,
    program: &ProgramRef,
    profile: &'static Profile,
    diags: &mut Diagnostics,
    env: &Env,
) {
    let bounds = match profile.name {
        "cr6" => (16, "COMC1, COMC3, or COMU1..COMU7"),
        "cr1000x" => (12, "COMC1, COMC3..COMC7"),
        _ => return,
    };
    if stmt.args.len() != 2 || stmt.args.iter().any(Option::is_none) {
        return;
    }
    if let Ok(ConstValue::Str(_)) = consteval::value_of(stmt.args[0].as_deref(), env) {
        return;
    }
    let ctx = context::of(program, Some(profile), Some(env.clone()));
    let port = longconv::long_value_with(
        stmt.args[0].as_deref(),
        env,
        profile,
        ctx.float_names(),
        ctx.literal_names(),
    );
    if let Some(port) = port
        && !(9..=bounds.0).contains(&port)
    {
        _p3_err(
            diags,
            &stmt.base,
            &format!("Invalid COM port, must use {}", bounds.1),
        );
    }
}

const _SERIALIN_DEST_MESSAGE: &str = "A string variable destination is required";

/// Port of `checker/instr_rules.py::_serialin_bad_destination`.
pub fn _serialin_bad_destination(arg: &Expr, types: &OrderedMap<String, String>) -> Option<bool> {
    match arg {
        Expr::Name(n) => {
            let kind = types.get(&pystr::upper(&n.name))?;
            if kind == "FUNCTION" {
                None
            } else {
                Some(kind != "STRING")
            }
        }
        Expr::Index(ix) => {
            let kind = types.get(&pystr::upper(&ix.name))?;
            Some(kind != "STRING")
        }
        Expr::Number(_) | Expr::String(_) | Expr::Binary(_) | Expr::Unary(_) => Some(true),
        _ => None,
    }
}

/// Port of `checker/instr_rules.py::check_serialin_destination`.
pub fn check_serialin_destination(
    stmt: &Instruction,
    program: &Program,
    profile: &Profile,
    diags: &mut Diagnostics,
) {
    if let Some((prog, p, env)) = _p3_rule_env(program, profile) {
        check_serialin_destination_with(stmt, &prog, p, diags, &env);
    }
}

/// Port of `checker/instr_rules.py::check_serialin_destination` (full parameter list).
pub fn check_serialin_destination_with(
    stmt: &Instruction,
    program: &ProgramRef,
    profile: &'static Profile,
    diags: &mut Diagnostics,
    env: &Env,
) {
    if !profile.instructions().contains("SERIALIN") || stmt.args.is_empty() {
        return;
    }
    let Some(dest) = stmt.args[0].as_deref() else {
        return;
    };
    let types = context::of(program, Some(profile), Some(env.clone())).serialin_types();
    if _serialin_bad_destination(dest, &types) != Some(true) {
        return;
    }
    let (line, file) = (stmt.base.node.line, &stmt.base.node.file);
    let same = |d: &DiagRef| d.line == line && &d.file == file;
    let mut at = diags
        .items
        .iter()
        .position(same)
        .unwrap_or(diags.items.len());
    let mut new = vec![Rc::new(Diagnostic::basic(
        "error",
        line,
        _SERIALIN_DEST_MESSAGE,
        file,
    ))];
    if profile.single_pass {
        diags.items.retain(|d| !same(d));
        at = at.min(diags.items.len());
        if stmt.args.len() > 1 && stmt.args[1].is_some() {
            let full = expressions::_arg_text(stmt, 1);
            let token = _next_simple_token(&full);
            let text = if token.is_empty() { full } else { token };
            new.push(Rc::new(Diagnostic::basic(
                "error",
                line,
                &format!("Too many parameters:  {text}."),
                file,
            )));
        }
    }
    diags.items.splice(at..at, new);
}

/// Port of `checker/instr_rules.py::check_serialin_comport`.
pub fn check_serialin_comport(
    stmt: &Instruction,
    program: &Program,
    profile: &Profile,
    diags: &mut Diagnostics,
) {
    if let Some((prog, p, env)) = _p3_rule_env(program, profile) {
        check_serialin_comport_with(stmt, &prog, p, diags, &env);
    }
}

/// Port of `checker/instr_rules.py::check_serialin_comport` (full parameter list).
pub fn check_serialin_comport_with(
    stmt: &Instruction,
    program: &ProgramRef,
    profile: &'static Profile,
    diags: &mut Diagnostics,
    env: &Env,
) {
    let Some(ports) = _serialin_ports(profile.name) else {
        return;
    };
    if stmt.args.len() <= 1 {
        return;
    }
    let Some(record) = profile.instructions().get("SERIALIN") else {
        return;
    };
    if _p3_shortfall_message(record, given_count(stmt, profile)).is_some() {
        return;
    }
    let ctx = context::of(program, Some(profile), Some(env.clone()));
    let v = longconv::long_value_with(
        stmt.args[1].as_deref(),
        env,
        profile,
        ctx.float_names(),
        ctx.literal_names(),
    );
    if let Some(v) = v {
        let shown = if v == -(1 << 31) { v } else { v.abs() };
        if !ports.contains(&shown) {
            _p3_err(diags, &stmt.base, &format!("ComPort {shown} illegal."));
        }
    }
}

/// Port of `checker/instr_rules.py::check_serialinrecord`.
pub fn check_serialinrecord(program: &Program, profile: &Profile, diags: &mut Diagnostics) {
    if let Some((prog, p, env)) = _p3_rule_env(program, profile) {
        check_serialinrecord_with(&prog, p, diags, &env);
    }
}

/// Port of `checker/instr_rules.py::check_serialinrecord` (full parameter list).
pub fn check_serialinrecord_with(
    program: &ProgramRef,
    profile: &'static Profile,
    diags: &mut Diagnostics,
    env: &Env,
) {
    if profile.single_pass {
        return;
    }
    // ponytail: a BigInt constant (beyond i64) reads as "not a number" here; Python keeps the int.
    let value = |arg: Option<&ExprRef>| -> Option<i64> {
        let v = consteval::value_of(arg.map(|a| &**a), env).ok()?;
        match v {
            ConstValue::Int(i) => Some(i),
            ConstValue::Float(f) | ConstValue::CFloat(f) => pystr::py_int_from_f64(f),
            _ => None,
        }
    };
    let mut buffers: BTreeMap<i64, Option<i64>> = BTreeMap::new();
    let mut unknown_port = false;
    for st in &program.statements {
        let (name, args) = match st.as_ref() {
            Stmt::Instruction(i) => (pystr::upper(&i.keyword), &i.args),
            Stmt::Call(c) => (pystr::upper(&c.name), &c.args),
            _ => continue,
        };
        if name != "SERIALOPEN" || args.is_empty() {
            continue;
        }
        let Some(port) = value(args[0].as_ref()) else {
            unknown_port = true;
            continue;
        };
        let size = if args.len() > 4 {
            value(args[4].as_ref())
        } else {
            Some(0)
        };
        let old = buffers.get(&port).copied().unwrap_or(Some(0));
        buffers.insert(
            port,
            match (size, old) {
                (Some(s), Some(o)) => Some(o.max(256).max(s)),
                _ => None,
            },
        );
    }
    let word = |v: i64| (v + (1 << 15)).rem_euclid(1 << 16) - (1 << 15);
    for st in &program.statements {
        let Stmt::Instruction(i) = st.as_ref() else {
            continue;
        };
        if pystr::upper(&i.keyword) != "SERIALINRECORD" || i.args.len() < 5 {
            continue;
        }
        let port = value(i.args[0].as_ref());
        let start = value(i.args[2].as_ref());
        let count = value(i.args[3].as_ref());
        let end = value(i.args[4].as_ref());
        if let (Some(start), Some(count), Some(end)) = (start, count, end)
            && start != 0
            && count != 0
            && end != 0
            && word(start).min(count).min(word(end)) < 0
        {
            _p3_err(
                diags,
                &i.base,
                "StartWord, NumberBytes and EndWord cannot all be not used",
            );
        }
        let (Some(port), Some(count)) = (port, count) else {
            continue;
        };
        if unknown_port {
            continue;
        }
        let size = buffers.get(&port).copied().unwrap_or(Some(0));
        if let Some(size) = size
            && count.rem_euclid(1 << 32) >= size
        {
            _p3_err(
                diags,
                &i.base,
                &format!("Number of bytes {count} must be less than input buffer {size}"),
            );
        }
    }
}

const _DEBUG_HISTORY_LIMIT: i64 = 26_843_544;

/// Port of `checker/instr_rules.py::debug_history_oom`.
pub fn debug_history_oom(stmt: &Instruction, profile: &Profile, program: &Program) -> bool {
    match _p3_rule_env(program, profile) {
        Some((prog, p, env)) => debug_history_oom_with(stmt, &env, p, &prog),
        None => false,
    }
}

/// Port of `checker/instr_rules.py::debug_history_oom` (full parameter list).
pub fn debug_history_oom_with(
    stmt: &Instruction,
    env: &Env,
    profile: &'static Profile,
    program: &ProgramRef,
) -> bool {
    if stmt.args.len() != 5 || stmt.args[1].is_none() {
        return false;
    }
    let arg = stmt.args[1].as_deref();
    let ctx = context::of(program, Some(profile), Some(env.clone()));
    let Some(mut v) =
        longconv::long_value_with(arg, env, profile, ctx.float_names(), ctx.literal_names())
    else {
        return false;
    };
    match arg {
        Some(Expr::Name(n))
            if pystr::upper(&n.name) == "INF" && !env.consts.borrow().contains_key("INF") =>
        {
            v = (1 << 31) - 1;
        }
        Some(Expr::Number(n))
            if !n.text.starts_with('&')
                && (n.text.contains('.') || pystr::upper(&n.text).contains('E')) =>
        {
            let value = match &n.value {
                NumberValue::Int(i) => *i as f64,
                NumberValue::Float(f) => *f,
                NumberValue::BigInt(s) => pystr::parse_float(s).unwrap_or(f64::INFINITY),
            };
            let f = _p3_float32(value);
            v = if (-2_147_483_648.0..2_147_483_648.0).contains(&f) {
                f.trunc() as i64
            } else if f.is_infinite() && f > 0.0 {
                (1 << 31) - 1
            } else {
                -(1 << 31)
            };
        }
        _ => {}
    }
    ((v * 4) & 0xFFFF_FFFF) > _DEBUG_HISTORY_LIMIT
}

/// Port of `checker/instr_rules.py::check_debug_args`.
pub fn check_debug_args(
    stmt: &Instruction,
    program: &Program,
    profile: &Profile,
    diags: &mut Diagnostics,
) {
    if let Some((prog, p, env)) = _p3_rule_env(program, profile) {
        check_debug_args_with(stmt, &prog, p, diags, &env);
    }
}

/// Port of `checker/instr_rules.py::check_debug_args` (full parameter list).
pub fn check_debug_args_with(
    stmt: &Instruction,
    program: &ProgramRef,
    profile: &'static Profile,
    diags: &mut Diagnostics,
    env: &Env,
) {
    if !profile.instructions().contains("DEBUG")
        || stmt.args.len() != 5
        || stmt.args.iter().any(Option::is_none)
    {
        return;
    }
    let types = context::of(program, Some(profile), Some(env.clone())).serialin_types();
    let (line, file) = (stmt.base.node.line, &stmt.base.node.file);
    let same: Vec<DiagRef> = diags
        .items
        .iter()
        .filter(|d| d.line == line && &d.file == file)
        .cloned()
        .collect();
    let mut bad = false;
    let debug_arg_types: [(usize, &[&str]); 3] = [
        (2, &["LONG", "BOOLEAN"]),
        (3, &["LONG", "BOOLEAN"]),
        (4, &["STRING"]),
    ];
    for (i, allowed) in debug_arg_types {
        let arg = stmt.args[i].as_deref();
        let named = match arg {
            Some(Expr::Name(n)) => Some(&n.name),
            Some(Expr::Index(ix)) => Some(&ix.name),
            _ => None,
        };
        if let Some(name) = named {
            match types.get(&pystr::upper(name)) {
                None => {
                    let text = pystr::strip(&expressions::_arg_text(stmt, i as i64)).to_string();
                    let wanted = format!("Invalid symbol in expression: {text}.");
                    bad |= same.iter().any(|d| d.message == wanted)
                        || (matches!(arg, Some(Expr::Name(_)))
                            && (matches!(pystr::upper(name).as_str(), "NAN" | "INF")
                                || consteval::is_constant(arg, env)));
                }
                Some(kind) => bad |= !allowed.contains(&kind.as_str()),
            }
        } else if let Some(Expr::Member(m)) = arg {
            bad |= matches!(m.base.as_deref(), Some(Expr::Name(b)) if pystr::upper(&b.name) == "STATUS");
        } else {
            bad |= matches!(
                arg,
                Some(Expr::Number(_) | Expr::String(_) | Expr::Binary(_) | Expr::Unary(_))
            );
        }
    }
    if bad {
        let at = diags
            .items
            .iter()
            .rposition(|d| d.line == line && &d.file == file)
            .map_or(diags.items.len(), |i| i + 1);
        diags.items.insert(
            at,
            Rc::new(Diagnostic::basic(
                "error",
                line,
                "Illegal parameter(s)",
                file,
            )),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_text;

    fn run(
        src: &str,
        f: impl Fn(&ProgramRef, &'static Profile, &mut Diagnostics, &Env),
    ) -> Vec<String> {
        let profile = profiles::get("cr6").unwrap();
        let prog = parse_text(src, "t.crb", None, Some(profile)).0;
        let env = context::of(&prog, Some(profile), None).env().clone();
        let mut diags = Diagnostics::new();
        f(&prog, profile, &mut diags, &env);
        diags.items.iter().map(|d| d.message.clone()).collect()
    }

    #[test]
    fn strip_parens_and_depth() {
        assert_eq!(_strip_parens("((1+2))"), "1+2");
        assert_eq!(_strip_parens("(1)+(2)"), "(1)+(2)");
        assert!(_paren_depth_ok("(a)(b)"));
        assert!(!_paren_depth_ok(")("));
    }

    #[test]
    fn sdm_address_and_debug_args() {
        let msgs = run(
            "Public X\nBeginProg\n  SDMX50(99, X)\nEndProg\n",
            |p, pr, d, e| {
                for st in &p.statements {
                    if let Stmt::Instruction(i) = st.as_ref() {
                        check_sdm_address_with(i, p, pr, d, e);
                    }
                }
            },
        );
        assert_eq!(msgs, vec!["Invalid SDM Address: 99, must be 0 - 14."]);
    }

    #[test]
    fn matrix_errors() {
        let mut shapes = OrderedMap::new();
        shapes.insert("A".to_string(), (2, 3));
        shapes.insert("B".to_string(), (2, 3));
        let src = "BeginProg\n  Matrix(3, A, B, A)\nEndProg\n";
        let profile = profiles::get("cr6").unwrap();
        let prog = parse_text(src, "t.crb", None, Some(profile)).0;
        let Stmt::Instruction(i) = prog.statements[1].as_ref() else {
            panic!("not an instruction");
        };
        assert_eq!(
            _matrix_dimension_errors_with(3, &i.args, &shapes),
            vec!["multiply A columns and B rows must be the same"]
        );
    }
}
