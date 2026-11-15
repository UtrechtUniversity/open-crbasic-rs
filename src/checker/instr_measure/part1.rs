//! Port of `checker/instr_measure.py` (lines 1-753).

#![allow(
    dead_code,
    unused_variables,
    unused_imports,
    clippy::used_underscore_items,
    clippy::too_many_arguments,
    clippy::cast_precision_loss
)]

use std::collections::BTreeSet;
use std::rc::Rc;

use crate::checker::{context, expressions, instr_rules, longconv, scan_time, symbols, var_types};
use crate::consteval::{self, Env};
use crate::diagnostics::{Diagnostic, Diagnostics};
use crate::profiles::{Json, Profile};
use crate::pystr;
use crate::syntax::{
    ConstValue, Expr, Instruction, OrderedMap, Program, ProgramRef, Stmt, StmtRef, VarDecl,
};

use super::{part2, part3};

fn _flatten_inline_stmts(stmts: &[StmtRef]) -> Vec<StmtRef> {
    let mut out = Vec::with_capacity(stmts.len());
    for st in stmts {
        out.push(st.clone());
        if let Stmt::If(if_stmt) = st.as_ref() {
            if let Some(inline) = &if_stmt.inline {
                out.extend(_flatten_inline_stmts(inline));
            }
            if let Some(inline_else) = &if_stmt.inline_else {
                out.extend(_flatten_inline_stmts(inline_else));
            }
        }
    }
    out
}

/// Port of `checker/instr_measure.py::check`.
pub fn check(_program: &Program, _profile: &Profile, _diags: &mut Diagnostics) {
    todo!("checker/instr_measure.py::check (omitted: env: consteval.Env)")
}

/// Port of `checker/instr_measure.py::check`.
pub fn check_with(
    program: &ProgramRef,
    profile: &'static Profile,
    diags: &mut Diagnostics,
    env: &Env,
) {
    let measure = profile.measure();
    if measure.is_empty() {
        return;
    }
    let mut file_reported = false;
    let mut sequential: Option<bool> = None;
    let mut slow = false;
    let statements = _flatten_inline_stmts(&program.statements);
    let mut tablefile_context: Option<((BTreeSet<String>, BTreeSet<String>), var_types::Typer)> =
        None;
    if profile.instructions().contains("TIMEDCONTROL") {
        _check_timed_sdm_with(program, &statements, profile, env, diags);
    }
    for (pos, stmt_ref) in statements.iter().enumerate() {
        let Stmt::Instruction(stmt) = stmt_ref.as_ref() else {
            continue;
        };
        let kw = pystr::upper(&stmt.keyword);
        slow = kw == "SLOWSEQUENCE" || (slow && kw != "ENDPROG");
        if kw == "SW12" && measure.get("sw12_ports").is_some() {
            _check_sw12_with(stmt, measure, env, diags);
        }
        if kw == "DELAY"
            && let Some(delay_cfg) = measure.get("delay")
        {
            if sequential.is_none() {
                sequential = Some(part2::is_sequential(program, profile));
            }
            let prefix = Rc::new(Program::new(&program.name, statements[..=pos].to_vec()));
            let scope =
                context::of(&prefix, Some(profile), Some(env.clone())).scoped_view(stmt_ref, true);
            let mut typer = var_types::Typer::new(program, Some(&scope), Some(profile));
            let _ = delay_cfg;
            part2::_check_delay(stmt, diags, sequential.unwrap_or(false), slow, &mut typer);
        }
        if kw == "EXCITEV"
            && measure
                .get("delay")
                .and_then(|d| d.get("task_memory"))
                .and_then(Json::as_bool)
                .unwrap_or(false)
            && stmt.args.len() == 3
            && let Some(delay) = _int_value_with(stmt.args[2].as_deref(), env)
            && scan_time::task_memory_overflow(
                (scan_time::excite_delay_us(delay as f64) - 10).max(0),
            )
        {
            _task_memory_full(stmt, diags);
        }
        if kw == "CS616" && measure.get("cs616_min_reps_pos").is_some() {
            part2::_check_cs616_reps(stmt, diags);
        }
        if kw == "PERIODAVG" && profile.single_pass {
            _check_periodavg_channels_with(stmt, profile, env, diags);
        }
        if (kw == "EXCITEV" || kw == "EXCITEI") && measure.get("excite_max_mv").is_some() {
            part2::_check_excite_magnitude(stmt, diags);
        }
        if kw == "GPS"
            && let Some(gps) = measure.get("gps")
        {
            _check_structure_members_with(stmt, program, gps, env, diags);
            _check_gps_nmea_with(stmt_ref, program, env, diags);
            _check_gps_port_with(stmt, gps, env, diags);
        }
        if kw == "CH400STATUS" && profile.instructions().contains("CH400STATUS") {
            _check_ch400status_with(stmt, env, diags);
        }
        if kw == "FFT" && profile.instructions().contains("FFT") {
            let scope_view =
                context::of(program, Some(profile), Some(env.clone())).scoped_view(stmt_ref, false);
            let ctx = context::of(&scope_view, Some(profile), Some(env.clone()));
            let dims = ctx.declared_all_dims();
            _check_fft_source_with(stmt, dims, env, diags);
        }
        if kw == "SDI12RECORDER" && profile.single_pass {
            _check_sdi12_structure_dest_with(stmt, program, diags);
        }
        if kw == "TABLEFILE"
            && let Some(tf_cfg) = measure.get("tablefile")
        {
            let (names, typer) = tablefile_context.get_or_insert_with(|| {
                (
                    (
                        longconv::float_names_in(&statements),
                        longconv::literal_names_in(&statements),
                    ),
                    var_types::Typer::new(program, None, Some(profile)),
                )
            });
            _check_tablefile_with(stmt, tf_cfg, env, diags, profile, names, typer);
        }
        if let Some(rules) = measure.get("string_prefixes").and_then(Json::as_array) {
            for rule in rules {
                let instr = rule.get("instr").and_then(Json::as_str).unwrap_or("");
                if kw == instr && !(file_reported && rule.get("ext").is_some()) {
                    file_reported =
                        _check_string_prefix_with(stmt, rule, env, diags) || file_reported;
                }
            }
        }
    }
}

/// Port of `checker/instr_measure.py::_check_timed_sdm`.
pub fn _check_timed_sdm(_program: &Program, _profile: &Profile, _diags: &mut Diagnostics) {
    todo!(
        "checker/instr_measure.py::_check_timed_sdm (omitted: statements: list, env: consteval.Env)"
    )
}

/// Port of `checker/instr_measure.py::_check_timed_sdm`.
pub fn _check_timed_sdm_with(
    program: &ProgramRef,
    statements: &[StmtRef],
    profile: &'static Profile,
    env: &Env,
    diags: &mut Diagnostics,
) {
    if !statements.iter().any(|s| {
        matches!(s.as_ref(), Stmt::Instruction(i) if pystr::upper(&i.keyword) == "TIMEDCONTROL")
    }) {
        return;
    }
    let scope = context::of(program, None, Some(env.clone())).scope_tables(true, false);
    let names = (
        longconv::float_names_in(statements),
        longconv::literal_names_in(statements),
    );
    let sequential = part2::is_sequential(program, profile);
    let mut aliases: OrderedMap<String, (String, i64)> = OrderedMap::new();
    let mut registered: BTreeSet<(String, i64)> = BTreeSet::new();
    let mut in_program = false;
    let mut in_scan = false;
    let mut slow = false;
    let mut in_routine = false;

    let storage = |stmt_ref: &StmtRef,
                   arg: Option<&Expr>,
                   aliases: &OrderedMap<String, (String, i64)>|
     -> Option<(String, i64)> {
        let arg = arg?;
        let raw_name = match arg {
            Expr::Name(n) => &n.name,
            Expr::Index(ix) => &ix.name,
            _ => return None,
        };
        let name = pystr::upper(raw_name);
        if matches!(arg, Expr::Name(_))
            && let Some(target) = aliases.get(&name)
        {
            return Some(target.clone());
        }
        let tables = scope.for_statement(stmt_ref);
        if tables.types.get(&name).map(String::as_str) != Some("LONG") {
            return None;
        }
        let dims = tables.dims.get(&name)?;
        let offset = part3::_flat_offset(arg, dims.clone())?;
        Some((name, offset))
    };

    for stmt_ref in statements {
        if matches!(stmt_ref.as_ref(), Stmt::SubDef(_)) {
            in_routine = true;
        } else if crate::syntax::ends_routine(stmt_ref.as_ref()) {
            in_routine = false;
        }
        if in_routine {
            continue;
        }
        let Stmt::Instruction(stmt) = stmt_ref.as_ref() else {
            continue;
        };
        let keyword = pystr::upper(&stmt.keyword);
        match keyword.as_str() {
            "BEGINPROG" => in_program = true,
            "ENDPROG" => in_program = false,
            "SCAN" => in_scan = true,
            "NEXTSCAN" => in_scan = false,
            "SLOWSEQUENCE" => slow = true,
            "ALIAS"
                if !in_program
                    && stmt.args.len() == 2
                    && matches!(stmt.args[1].as_deref(), Some(Expr::Name(_))) =>
            {
                if let Some(Expr::Name(alias_name)) = stmt.args[1].as_deref()
                    && let Some(target) = storage(stmt_ref, stmt.args[0].as_deref(), &aliases)
                {
                    aliases.insert(pystr::upper(&alias_name.name), target);
                }
            }
            "TIMEDCONTROL" if !in_program && stmt.args.len() == 7 => {
                if let Some(target) = storage(stmt_ref, stmt.args[5].as_deref(), &aliases) {
                    registered.insert(target);
                }
            }
            "SDMCD16AC" if in_program && in_scan && stmt.args.len() == 3 => {
                let Some(target) = storage(stmt_ref, stmt.args[0].as_deref(), &aliases) else {
                    continue;
                };
                if !registered.contains(&target) {
                    continue;
                }
                let reps = longconv::long_value_with(
                    stmt.args[1].as_deref(),
                    env,
                    profile,
                    &names.0,
                    &names.1,
                );
                let mut errors: Vec<String> = Vec::new();
                if let Some(r) = reps
                    && part2::_byte(r) > 1
                {
                    errors.push("When using TimedControl reps must be 1.".to_owned());
                }
                if slow {
                    errors.push("TimedControl only allowed within main scan.".to_owned());
                }
                if sequential {
                    errors.push("TimedControl must use PipeLineMode.".to_owned());
                }
                if !errors.is_empty() {
                    part2::_insert_in_line_order(diags, stmt, errors);
                }
            }
            _ => {}
        }
    }
}

/// Port of `checker/instr_measure.py::_check_periodavg_channels`.
pub fn _check_periodavg_channels(
    _stmt: &Instruction,
    _profile: &Profile,
    _diags: &mut Diagnostics,
) {
    todo!("checker/instr_measure.py::_check_periodavg_channels (omitted: env: consteval.Env)")
}

/// Port of `checker/instr_measure.py::_check_periodavg_channels`.
pub fn _check_periodavg_channels_with(
    stmt: &Instruction,
    profile: &Profile,
    env: &Env,
    diags: &mut Diagnostics,
) {
    if stmt.args.len() != 8 {
        return;
    }
    let empty = BTreeSet::new();
    let Some(channel) =
        longconv::long_value_with(stmt.args[2].as_deref(), env, profile, &empty, &empty)
    else {
        return;
    };
    let reps = _int_value_with(stmt.args[1].as_deref(), env);
    let maximum: i64 = if profile.name == "cr300" { 4 } else { 6 };
    if (channel < -1 && channel != longconv::INT_MIN) || channel > 9 {
        return;
    }
    if !(1..=maximum).contains(&channel) {
        diags.error_basic(
            stmt.base.node.line,
            &format!("Invalid channel: {channel}."),
            &stmt.base.node.file,
        );
    }
    if let Some(r) = reps
        && r > 0
        && channel > 0
        && channel + r - 1 > maximum
    {
        diags.error_basic(
            stmt.base.node.line,
            "Reps exceeds valid channels.",
            &stmt.base.node.file,
        );
    }
    let port = if profile.name == "cr300" {
        match channel {
            -1 => Some("(null)"),
            8 => Some("RS232"),
            9 => Some("?"),
            _ => None,
        }
    } else {
        match channel {
            -1 => Some("(null)"),
            7 => Some("SW12_1"),
            8 => Some("SW12_2"),
            9 => Some("RS232"),
            _ => None,
        }
    };
    if let Some(port) = port {
        diags.error_basic(
            stmt.base.node.line,
            &format!("{port} is not a valid control port."),
            &stmt.base.node.file,
        );
    }
}

/// Port of `checker/instr_measure.py::check_const_ranges`.
pub fn check_const_ranges(_profile: &Profile, _diags: &mut Diagnostics) {
    todo!("checker/instr_measure.py::check_const_ranges (omitted: flat: list, env: consteval.Env)")
}

/// Port of `checker/instr_measure.py::check_const_ranges`.
pub fn check_const_ranges_with(
    flat: &[StmtRef],
    profile: &Profile,
    diags: &mut Diagnostics,
    env: &Env,
) {
    let mut by_instr: std::collections::BTreeMap<String, Vec<&Json>> =
        std::collections::BTreeMap::new();
    if let Some(rules) = profile
        .measure()
        .get("const_ranges")
        .and_then(Json::as_array)
    {
        for rule in rules {
            if let Some(instr) = rule.get("instr").and_then(Json::as_str) {
                by_instr.entry(instr.to_owned()).or_default().push(rule);
            }
        }
    }
    if by_instr.is_empty() {
        return;
    }
    let names = (
        longconv::float_names_in(flat),
        longconv::literal_names_in(flat),
    );
    for stmt_ref in flat {
        let Stmt::Instruction(stmt) = stmt_ref.as_ref() else {
            continue;
        };
        let kw = pystr::upper(&stmt.keyword);
        if kw.starts_with("CDM_")
            && (instr_rules::part5::cdm_null_module(stmt)
                || instr_rules::part4::cdm_unsupported_module(stmt).is_some())
        {
            continue;
        }
        let tccomp = !stmt.args.is_empty()
            && kw == "CDM_TCCOMP"
            && _int_value_with(stmt.args[0].as_deref(), env) == Some(224);
        if let Some(rules) = by_instr.get(&kw) {
            for rule in rules {
                let pos = rule.get("pos").and_then(Json::as_i64).unwrap_or(0);
                if tccomp && pos == 5 {
                    _tccomp_before_channel_with(stmt, env, profile, &names, diags);
                }
                _apply_const_rule_with(stmt, rule, env, profile, &names, diags);
                if tccomp && pos == 5 {
                    _tccomp_after_channel_with(stmt, env, profile, &names, diags);
                }
            }
        }
    }
}

/// Port of `checker/instr_measure.py::_tccomp_before_channel`.
pub fn _tccomp_before_channel() {
    todo!(
        "checker/instr_measure.py::_tccomp_before_channel (omitted: stmt: untyped, env: untyped, profile: untyped, names: untyped, diags: untyped)"
    )
}

/// Port of `checker/instr_measure.py::_tccomp_before_channel`.
pub fn _tccomp_before_channel_with(
    stmt: &Instruction,
    env: &Env,
    profile: &Profile,
    names: &(BTreeSet<String>, BTreeSet<String>),
    diags: &mut Diagnostics,
) {
    let empty_rule = Json::Object(Vec::new());
    if let Some(channel) = _long_arg_with(stmt, 5, &empty_rule, env, profile, names)
        && channel < 0
    {
        diags.error_basic(
            stmt.base.node.line,
            "Burst measurements unsupported, input channel must be > 0!",
            &stmt.base.node.file,
        );
    }
}

/// Port of `checker/instr_measure.py::_tccomp_after_channel`.
pub fn _tccomp_after_channel() {
    todo!(
        "checker/instr_measure.py::_tccomp_after_channel (omitted: stmt: untyped, env: untyped, profile: untyped, names: untyped, diags: untyped)"
    )
}

/// Port of `checker/instr_measure.py::_tccomp_after_channel`.
pub fn _tccomp_after_channel_with(
    stmt: &Instruction,
    env: &Env,
    profile: &Profile,
    names: &(BTreeSet<String>, BTreeSet<String>),
    diags: &mut Diagnostics,
) {
    let empty_rule = Json::Object(Vec::new());
    let channel = _long_arg_with(stmt, 5, &empty_rule, env, profile, names);
    let reps = _long_arg_with(stmt, 4, &empty_rule, env, profile, names);
    if let (Some(mut ch), Some(r)) = (channel, reps) {
        if !(1..=20).contains(&ch) {
            ch = 1;
        }
        if ch - 1 + r > 20 {
            diags.error_basic(
                stmt.base.node.line,
                "Reps cycle beyond last channel!",
                &stmt.base.node.file,
            );
        }
    }
    if let Some(tc_type) = _long_arg_with(stmt, 6, &empty_rule, env, profile, names)
        && !(0..=7).contains(&tc_type)
    {
        diags.error_basic(
            stmt.base.node.line,
            "Thermocouple type unsupported.",
            &stmt.base.node.file,
        );
    }
}

/// Port of `checker/instr_measure.py::_apply_cdm_channel`.
pub fn _apply_cdm_channel() {
    todo!(
        "checker/instr_measure.py::_apply_cdm_channel (omitted: stmt: untyped, rule: untyped, env: untyped, profile: untyped, names: untyped, diags: untyped)"
    )
}

/// Port of `checker/instr_measure.py::_apply_cdm_channel`.
pub fn _apply_cdm_channel_with(
    stmt: &Instruction,
    rule: &Json,
    env: &Env,
    profile: &Profile,
    names: &(BTreeSet<String>, BTreeSet<String>),
    diags: &mut Diagnostics,
) {
    if stmt.args.is_empty() {
        return;
    }
    let mod_key = _int_value_with(stmt.args[0].as_deref(), env)
        .map_or_else(|| "None".to_owned(), |v| v.to_string());
    let last = rule
        .get("channel_by_module")
        .and_then(|m| m.get(&mod_key))
        .and_then(Json::as_i64);
    let pos = rule.get("pos").and_then(Json::as_i64).unwrap_or(0);
    let value = _long_arg_with(stmt, pos, rule, env, profile, names);
    let (Some(last), Some(mut value)) = (last, value) else {
        return;
    };
    let int_min: i64 = -(1_i64 << 31);
    let magnitude = if int_min < value && value < 0 {
        -value
    } else {
        value
    };
    let role = rule.get("role").and_then(Json::as_str).unwrap_or("");
    match role {
        "panel_temp" => {
            let reps = _long_arg_with(stmt, 4, rule, env, profile, names);
            if last == 30 && value < 0 {
                diags.error_basic(
                    stmt.base.node.line,
                    "Burst measurements unsupported, input channel must be > 0!",
                    &stmt.base.node.file,
                );
            }
            if last != 30
                && let Some(r) = reps
                && r < 1
            {
                diags.error_basic(
                    stmt.base.node.line,
                    &format!("Reps: {r}, out of range (1 - 65536)"),
                    &stmt.base.node.file,
                );
                diags.error_basic(
                    stmt.base.node.line,
                    &format!("MeasPEx: {r}, must be greater than 0."),
                    &stmt.base.node.file,
                );
            }
            if !(1..=last).contains(&value) {
                let message = if last == 30 {
                    format!("PanelTemp channel: {value}, out of range (1 - 30).")
                } else {
                    rule.get("message")
                        .and_then(Json::as_str)
                        .unwrap_or("")
                        .to_owned()
                };
                diags.error_basic(stmt.base.node.line, &message, &stmt.base.node.file);
                value = 1;
            }
            if let Some(r) = reps
                && value - 1 + r > last
            {
                diags.error_basic(
                    stmt.base.node.line,
                    "PanelTemp reps beyond last channel!",
                    &stmt.base.node.file,
                );
            }
        }
        "ic" => {
            if value == int_min || magnitude < 1 || magnitude > last {
                let shown_val = if value == int_min { value } else { magnitude }.to_string();
                let max_str = last.to_string();
                let template = rule.get("message").and_then(Json::as_str).unwrap_or("");
                let msg =
                    pystr::format_named(template, &[("value", &shown_val), ("max", &max_str)]);
                diags.error_basic(stmt.base.node.line, &msg, &stmt.base.node.file);
            }
        }
        "rc" => {
            let reps = if stmt.args.len() >= 4 {
                _long_arg_with(stmt, 4, rule, env, profile, names)
            } else {
                None
            };
            if value > last
                || (1 <= value
                    && reps.is_some_and(|r| (1..=65536).contains(&r) && value - 1 + r > last))
            {
                let msg = rule.get("message").and_then(Json::as_str).unwrap_or("");
                diags.error_basic(stmt.base.node.line, msg, &stmt.base.node.file);
            }
        }
        "tm" if int_min < value && value < 0 && magnitude > last => {
            let msg = rule.get("message").and_then(Json::as_str).unwrap_or("");
            diags.error_basic(stmt.base.node.line, msg, &stmt.base.node.file);
        }
        _ => {}
    }
}

/// Port of `checker/instr_measure.py::_apply_const_rule`.
pub fn _apply_const_rule() {
    todo!(
        "checker/instr_measure.py::_apply_const_rule (omitted: stmt: untyped, rule: untyped, env: untyped, profile: untyped, names: untyped, diags: untyped)"
    )
}

/// Port of `checker/instr_measure.py::_apply_const_rule`.
pub fn _apply_const_rule_with(
    stmt: &Instruction,
    rule: &Json,
    env: &Env,
    profile: &Profile,
    names: &(BTreeSet<String>, BTreeSet<String>),
    diags: &mut Diagnostics,
) {
    if rule.get("channel_by_module").is_some() {
        _apply_cdm_channel_with(stmt, rule, env, profile, names, diags);
        return;
    }
    if let Some(skip_modules) = rule.get("skip_modules").and_then(Json::as_array)
        && !stmt.args.is_empty()
    {
        let mod_key = _int_value_with(stmt.args[0].as_deref(), env)
            .map_or_else(|| "None".to_owned(), |v| v.to_string());
        if skip_modules
            .iter()
            .any(|m| m.as_str() == Some(mod_key.as_str()))
        {
            return;
        }
    }
    let pos = rule.get("pos").and_then(Json::as_i64).unwrap_or(0);
    let Some(raw) = _raw_arg_with(stmt, pos, env) else {
        return;
    };
    if rule
        .get("skip_strings")
        .and_then(Json::as_bool)
        .unwrap_or(false)
        && matches!(raw, ConstValue::Str(_))
    {
        return;
    }
    if let Some(min_mag) = rule.get("minimum_magnitude").and_then(Json::as_f64) {
        let too_small = match &raw {
            ConstValue::Str(_) => true,
            ConstValue::Int(i) => (*i as f64).abs() < min_mag,
            ConstValue::Float(f) | ConstValue::CFloat(f) => f.abs() < min_mag,
            ConstValue::BigInt(s) => pystr::parse_float(s).is_some_and(|f| f.abs() < min_mag),
        };
        if too_small {
            let before_warning = diags
                .items
                .iter()
                .position(|d| {
                    (d.file.as_str(), d.line) == (stmt.base.node.file.as_str(), stmt.base.node.line)
                        && d.kind == "fnotch"
                })
                .unwrap_or(diags.items.len());
            let msg = rule.get("message").and_then(Json::as_str).unwrap_or("");
            diags.items.insert(
                before_warning,
                Rc::new(Diagnostic::basic(
                    "error",
                    stmt.base.node.line,
                    msg,
                    &stmt.base.node.file,
                )),
            );
        }
        return;
    }
    if let Some(invalid_raw) = rule.get("invalid_raw") {
        if _matches_invalid_raw_with(&raw, invalid_raw) {
            let val_str = consteval::to_long(&raw).to_string();
            let template = rule.get("message").and_then(Json::as_str).unwrap_or("");
            let msg = pystr::format_named(template, &[("value", &val_str)]);
            diags.error_basic(stmt.base.node.line, &msg, &stmt.base.node.file);
        }
        return;
    }
    let Some(value) = _long_arg_with(stmt, pos, rule, env, profile, names) else {
        return;
    };
    if let Some(lower_gate) = rule.get("lower_gate").and_then(Json::as_i64)
        && value < lower_gate
    {
        return;
    }
    if let Some(allow) = rule.get("allow").and_then(Json::as_array)
        && allow.iter().any(|v| v.as_i64() == Some(value))
    {
        return;
    }
    if let Some(warn_values) = rule.get("warn_values").and_then(Json::as_array)
        && warn_values.iter().any(|v| v.as_i64() == Some(value))
    {
        let warn_msg = rule
            .get("warn_message")
            .and_then(Json::as_str)
            .unwrap_or("");
        diags.warning_basic(stmt.base.node.line, warn_msg, &stmt.base.node.file);
        return;
    }
    if let Some(by_module) = rule.get("by_module") {
        let mod_key = if stmt.args.is_empty() {
            "None".to_owned()
        } else {
            _int_value_with(stmt.args[0].as_deref(), env)
                .map_or_else(|| "None".to_owned(), |v| v.to_string())
        };
        let allowed = by_module.get(&mod_key).or_else(|| rule.get("values"));
        if allowed.is_none()
            || allowed
                .and_then(Json::as_array)
                .is_some_and(|arr| arr.iter().any(|v| v.as_i64() == Some(value)))
        {
            return;
        }
    } else if let Some(values) = rule.get("values").and_then(Json::as_array) {
        if values.iter().any(|v| v.as_i64() == Some(value)) {
            return;
        }
    } else {
        let min = rule.get("min").and_then(Json::as_i64).unwrap_or(i64::MIN);
        let max = rule.get("max").and_then(Json::as_i64).unwrap_or(i64::MAX);
        if (min..=max).contains(&value) {
            return;
        }
    }
    let show_pos = rule.get("show").and_then(Json::as_i64).unwrap_or(pos);
    if let Some(shown) = _long_arg_with(stmt, show_pos, rule, env, profile, names) {
        let shown_str = shown.to_string();
        let template = rule.get("message").and_then(Json::as_str).unwrap_or("");
        let msg = pystr::format_named(template, &[("value", &shown_str)]);
        diags.error_basic(stmt.base.node.line, &msg, &stmt.base.node.file);
    }
}

/// Port of `checker/instr_measure.py::_matches_invalid_raw`.
pub fn _matches_invalid_raw() -> bool {
    todo!("checker/instr_measure.py::_matches_invalid_raw (omitted: value: object, invalid: dict)")
}

/// Port of `checker/instr_measure.py::_matches_invalid_raw`.
pub fn _matches_invalid_raw_with(value: &ConstValue, invalid: &Json) -> bool {
    if invalid
        .get("strings")
        .and_then(Json::as_bool)
        .unwrap_or(false)
        && matches!(value, ConstValue::Str(_))
    {
        return true;
    }
    let outside_long = invalid
        .get("outside_long")
        .and_then(Json::as_bool)
        .unwrap_or(false);
    match value {
        ConstValue::Str(_) => false,
        ConstValue::Int(i) => {
            if outside_long && (*i < -2_147_483_648 || *i > 2_147_483_647) {
                return true;
            }
            invalid
                .get("values")
                .and_then(Json::as_array)
                .is_some_and(|arr| arr.iter().any(|v| v.as_i64() == Some(*i)))
        }
        ConstValue::BigInt(_) => outside_long,
        ConstValue::Float(f) | ConstValue::CFloat(f) => {
            if invalid.get("nan").and_then(Json::as_bool).unwrap_or(false) && f.is_nan() {
                return true;
            }
            if f.is_infinite() {
                return false;
            }
            if outside_long && (*f < -2_147_483_648.0 || *f > 2_147_483_647.0) {
                return true;
            }
            invalid
                .get("values")
                .and_then(Json::as_array)
                .is_some_and(|arr| arr.iter().any(|v| v.as_f64() == Some(*f)))
        }
    }
}

/// Port of `checker/instr_measure.py::_long_arg`.
pub fn _long_arg(_stmt: &Instruction, _pos: i64, _profile: &Profile) -> Option<i64> {
    todo!(
        "checker/instr_measure.py::_long_arg (omitted: rule: dict, env: consteval.Env, names: untyped)"
    )
}

/// Port of `checker/instr_measure.py::_long_arg`.
pub fn _long_arg_with(
    stmt: &Instruction,
    pos: i64,
    rule: &Json,
    env: &Env,
    profile: &Profile,
    names: &(BTreeSet<String>, BTreeSet<String>),
) -> Option<i64> {
    const INF_MAX_THRESHOLD: f64 = 3.402_823_466_385_288_6e38; // 2**128 - 2**103
    let value = _raw_arg_with(stmt, pos, env)?;
    if rule
        .get("string_zero")
        .and_then(Json::as_bool)
        .unwrap_or(false)
        && matches!(value, ConstValue::Str(_))
    {
        return Some(0);
    }
    if rule.get("inf_max").and_then(Json::as_bool).unwrap_or(false) {
        let is_inf_max = match &value {
            ConstValue::Float(f) | ConstValue::CFloat(f) => *f >= INF_MAX_THRESHOLD,
            ConstValue::BigInt(s) => pystr::parse_float(s).is_some_and(|f| f >= INF_MAX_THRESHOLD),
            ConstValue::Int(_) | ConstValue::Str(_) => false,
        };
        if is_inf_max {
            return Some(2_147_483_647);
        }
    }
    if !rule.get("wrap16").and_then(Json::as_bool).unwrap_or(false) {
        let idx = usize::try_from(pos - 1).ok()?;
        let arg = stmt.args.get(idx).and_then(|a| a.as_deref());
        let converted = longconv::long_value_with(arg, env, profile, &names.0, &names.1);
        let result = converted.unwrap_or_else(|| consteval::to_long(&value));
        return Some(
            if rule.get("wrap8").and_then(Json::as_bool).unwrap_or(false) {
                part2::_byte(result)
            } else {
                result
            },
        );
    }
    match value {
        ConstValue::Str(_) => Some(0),
        ConstValue::Float(f) | ConstValue::CFloat(f) => {
            if f.is_nan() {
                Some(0)
            } else if f.is_infinite() {
                None
            } else if f.abs() >= 2_147_483_648.0 {
                rule.get("overflow").and_then(Json::as_i64)
            } else {
                pystr::py_int_from_f64(f).map(|i| i & 0xFFFF)
            }
        }
        ConstValue::Int(i) => {
            if i.abs() >= 2_147_483_648 {
                rule.get("overflow").and_then(Json::as_i64)
            } else {
                Some(i & 0xFFFF)
            }
        }
        ConstValue::BigInt(_) => rule.get("overflow").and_then(Json::as_i64),
    }
}

/// Port of `checker/instr_measure.py::_raw_arg`.
pub fn _raw_arg(_stmt: &Instruction, _pos: i64) {
    todo!("checker/instr_measure.py::_raw_arg (omitted: env: consteval.Env)")
}

/// Port of `checker/instr_measure.py::_raw_arg`.
pub fn _raw_arg_with(stmt: &Instruction, pos: i64, env: &Env) -> Option<ConstValue> {
    let idx = usize::try_from(pos - 1).ok()?;
    let arg = stmt.args.get(idx)?.as_deref()?;
    _fnotch_value_with(Some(arg), env)
}

/// Port of `checker/instr_measure.py::_check_string_prefix`.
pub fn _check_string_prefix(_stmt: &Instruction, _diags: &mut Diagnostics) -> bool {
    todo!(
        "checker/instr_measure.py::_check_string_prefix (omitted: rule: dict, env: consteval.Env)"
    )
}

/// Port of `checker/instr_measure.py::_check_string_prefix`.
pub fn _check_string_prefix_with(
    stmt: &Instruction,
    rule: &Json,
    env: &Env,
    diags: &mut Diagnostics,
) -> bool {
    let pos = rule.get("pos").and_then(Json::as_i64).unwrap_or(0);
    let Ok(idx) = usize::try_from(pos - 1) else {
        return false;
    };
    if idx >= stmt.args.len() || stmt.args[idx].is_none() {
        return false;
    }
    let Some(ConstValue::Str(value)) = _const_value_with(stmt.args[idx].as_deref(), env) else {
        return false;
    };
    let drive = pystr::slice(&value, None, Some(4));
    let drive_up = pystr::upper(drive);
    let prefix_ok = rule
        .get("prefixes")
        .and_then(Json::as_array)
        .is_some_and(|arr| {
            arr.iter()
                .any(|p| p.as_str().is_some_and(|s| format!("{s}:") == drive_up))
        });
    if !prefix_ok {
        let template = rule.get("message").and_then(Json::as_str).unwrap_or("");
        let msg = pystr::format_named(template, &[("drive", drive)]);
        diags.error_basic(stmt.base.node.line, &msg, &stmt.base.node.file);
        return true;
    }
    let ext_rule = rule.get("ext").and_then(Json::as_str);
    let option = if ext_rule.is_some() && stmt.args.len() > 1 {
        _int_value_with(stmt.args[1].as_deref(), env)
    } else {
        None
    };
    if let (Some(opt), Some(ext_prefix)) = (option, ext_rule)
        && opt.abs() < (1_i64 << 31)
        && (opt & 6) != 0
    {
        let ext = if value.contains('.') {
            pystr::upper(pystr::rpartition(pystr::strip(&value), ".").2)
        } else {
            String::new()
        };
        if !(ext.starts_with("CRB") || ext.starts_with(ext_prefix)) {
            diags.error_basic(
                stmt.base.node.line,
                &format!("FileManage name {value} invalid extension."),
                &stmt.base.node.file,
            );
            return true;
        }
    }
    false
}

/// Port of `checker/instr_measure.py::_check_ch400status`.
pub fn _check_ch400status(_stmt: &Instruction, _diags: &mut Diagnostics) {
    todo!("checker/instr_measure.py::_check_ch400status (omitted: env: consteval.Env)")
}

/// Port of `checker/instr_measure.py::_check_ch400status`.
pub fn _check_ch400status_with(stmt: &Instruction, env: &Env, diags: &mut Diagnostics) {
    if stmt.args.len() < 2 {
        return;
    }
    let Some(value) = _int_value_with(stmt.args[0].as_deref(), env) else {
        return;
    };
    let Some(unit_val) = _const_value_with(stmt.args[1].as_deref(), env) else {
        return;
    };
    let unit = match unit_val {
        ConstValue::Int(i) => i,
        ConstValue::Float(f) | ConstValue::CFloat(f) => match pystr::py_int_from_f64(f) {
            Some(i) => i,
            None => return,
        },
        ConstValue::Str(_) | ConstValue::BigInt(_) => return,
    };
    let Some(us_per_unit) = delay_us_per_unit(unit) else {
        return;
    };
    if pystr::pymod_i64(value.saturating_mul(us_per_unit), 1_000_000) != Some(0) {
        diags.error_basic(
            stmt.base.node.line,
            "CH400Status update interval must be an integer multiple of 1 second.",
            &stmt.base.node.file,
        );
    }
}

/// Port of `checker/instr_measure.py::_check_fft_source`.
pub fn _check_fft_source(_stmt: &Instruction, _diags: &mut Diagnostics) {
    todo!("checker/instr_measure.py::_check_fft_source (omitted: dims: dict, env: consteval.Env)")
}

/// Port of `checker/instr_measure.py::_check_fft_source`.
pub fn _check_fft_source_with(
    stmt: &Instruction,
    dims: &OrderedMap<String, Vec<i64>>,
    env: &Env,
    diags: &mut Diagnostics,
) {
    if stmt.args.len() < 3 {
        return;
    }
    let Some(arg0) = stmt.args[0].as_deref() else {
        return;
    };
    let src_name = match arg0 {
        Expr::Name(n) => &n.name,
        Expr::Index(ix) => &ix.name,
        _ => return,
    };
    let Some(n) = _int_value_with(stmt.args[2].as_deref(), env) else {
        return;
    };
    let Some(shape) = dims.get(&pystr::upper(src_name)) else {
        return;
    };
    if n < 1 || (n & (n - 1)) != 0 {
        return;
    }
    let offset = part3::_flat_offset(arg0, shape.clone());
    if let Some(off) = offset
        && shape.iter().product::<i64>() - off < n
    {
        diags.error_basic(
            stmt.base.node.line,
            &format!("{src_name} must be of length >= {n}."),
            &stmt.base.node.file,
        );
    }
}

/// Port of `checker/instr_measure.py::_check_sdi12_structure_dest`.
pub fn _check_sdi12_structure_dest(stmt: &Instruction, program: &Program, diags: &mut Diagnostics) {
    _check_sdi12_structure_dest_with(stmt, &Rc::new(program.clone()), diags);
}

/// Port of `checker/instr_measure.py::_check_sdi12_structure_dest`.
pub fn _check_sdi12_structure_dest_with(
    stmt: &Instruction,
    program: &ProgramRef,
    diags: &mut Diagnostics,
) {
    let Some(dest) = stmt.args.first().and_then(|a| a.as_deref()) else {
        return;
    };
    let dest_name = match dest {
        Expr::Name(n) => &n.name,
        Expr::Index(ix) if !ix.args.iter().any(Option::is_none) => &ix.name,
        _ => return,
    };
    let var_types_map = context::of(program, None, None).variable_types(false);
    let Some(type_name) = var_types_map.get(&pystr::upper(dest_name)) else {
        return;
    };
    let has_struct = program.statements.iter().any(|st| {
        if let Stmt::Instruction(inst) = st.as_ref()
            && pystr::upper(&inst.keyword) == "STRUCTURETYPE"
            && let Some(Some(arg0)) = inst.args.first()
            && let Expr::Name(n) = arg0.as_ref()
        {
            pystr::upper(&n.name) == *type_name
        } else {
            false
        }
    });
    if has_struct {
        let arg_text = expressions::part2::_arg_text(stmt, 0);
        diags.error_basic(
            stmt.base.node.line,
            &format!("Could not find DataTable {arg_text}."),
            &stmt.base.node.file,
        );
    }
}

/// Port of `checker/instr_measure.py::_check_tablefile`.
pub fn _check_tablefile(
    _stmt: &Instruction,
    _diags: &mut Diagnostics,
    _profile: &Profile,
    _typer: &mut crate::checker::var_types::Typer,
) {
    todo!(
        "checker/instr_measure.py::_check_tablefile (omitted: cfg: dict, env: consteval.Env, names: tuple[frozenset[str], frozenset[str]])"
    )
}

/// Port of `checker/instr_measure.py::_check_tablefile`.
pub fn _check_tablefile_with(
    stmt: &Instruction,
    cfg: &Json,
    env: &Env,
    diags: &mut Diagnostics,
    profile: &Profile,
    names: &(BTreeSet<String>, BTreeSet<String>),
    typer: &var_types::Typer,
) {
    let name = stmt
        .args
        .first()
        .and_then(|a| _name_text_with(a.as_deref(), env));
    let Some(name) = name.filter(|s| !s.is_empty()) else {
        return;
    };
    let drive = pystr::upper(pystr::slice(&name, None, Some(4)));
    let valid_drive = cfg
        .get("drives")
        .and_then(Json::as_array)
        .is_some_and(|arr| {
            arr.iter()
                .any(|d| d.as_str().is_some_and(|s| format!("{s}:") == drive))
        });
    if !valid_drive {
        diags.error_basic(
            stmt.base.node.line,
            &format!("Invalid drive or no drive specified in filename {name}."),
            &stmt.base.node.file,
        );
        if !profile.single_pass {
            return;
        }
    }
    let option = if stmt.args.len() > 1 {
        longconv::long_value_with(stmt.args[1].as_deref(), env, profile, &names.0, &names.1)
    } else {
        None
    };
    if let Some(opt) = option {
        let (tob3, invalid) = if cfg
            .get("option_mod100")
            .and_then(Json::as_bool)
            .unwrap_or(false)
        {
            let low = opt.abs() % 100;
            let in_tob3_drives =
                cfg.get("tob3_drives")
                    .and_then(Json::as_array)
                    .is_some_and(|arr| {
                        arr.iter()
                            .any(|d| d.as_str().is_some_and(|s| format!("{s}:") == drive))
                    });
            (
                low >= 64 && in_tob3_drives,
                (24..=31).contains(&low) || low >= 40,
            )
        } else {
            let mut code = if opt == longconv::INT_MIN {
                opt
            } else {
                opt.abs()
            };
            if (100..=135).contains(&code) || (356..=363).contains(&code) {
                code -= 100;
            }
            let is_tob3 = (code & 0x40) != 0;
            let tob3 = is_tob3 && drive != "CRD:";
            let shifted = code >> 3;
            let invalid = !is_tob3 && !matches!(shifted, 0 | 1 | 2 | 4 | 8 | 32 | 64 | 128);
            (tob3, invalid)
        };
        if tob3 {
            diags.error_basic(
                stmt.base.node.line,
                "TOB3 TableFile output only allowed on CRD: drive.",
                &stmt.base.node.file,
            );
        }
        if invalid {
            diags.error_basic(
                stmt.base.node.line,
                "Invalid file type option.",
                &stmt.base.node.file,
            );
        }
    }
    if stmt.args.len() > 7
        && let Some(result) = typer.variable(stmt.args[7].as_deref())
        && typer.types.get(&result).map(String::as_str) != Some("STRING")
    {
        diags.error_basic(
            stmt.base.node.line,
            "Invalid parameter type, STRING type required.",
            &stmt.base.node.file,
        );
    }
}

/// Port of `checker/instr_measure.py::_name_text`.
pub fn _name_text(_arg: Option<&Expr>) -> Option<String> {
    todo!("checker/instr_measure.py::_name_text (omitted: env: consteval.Env)")
}

/// Port of `checker/instr_measure.py::_name_text`.
pub fn _name_text_with(arg: Option<&Expr>, env: &Env) -> Option<String> {
    let arg = arg?;
    if let Expr::Member(m) = arg
        && m.args.is_none()
        && let Some(base) = m.base.as_deref()
        && let Expr::Name(base_name) = base
        && pystr::upper(&base_name.name) == "STATUS"
        && pystr::upper(&m.member) == "SERIALNUMBER"
    {
        return Some(String::new());
    }
    if let Expr::Binary(b) = arg
        && (b.op == "+" || b.op == "&")
    {
        let left = _name_text_with(b.left.as_deref(), env)?;
        let right = _name_text_with(b.right.as_deref(), env)?;
        return Some(format!("{left}{right}"));
    }
    match _const_value_with(Some(arg), env)? {
        ConstValue::Str(s) => Some(s),
        _ => None,
    }
}

/// Port of `checker/instr_measure.py::_check_structure_members`.
pub fn _check_structure_members(_stmt: &Instruction, _program: &Program, _diags: &mut Diagnostics) {
    todo!(
        "checker/instr_measure.py::_check_structure_members (omitted: cfg: dict, env: consteval.Env)"
    )
}

/// Port of `checker/instr_measure.py::_check_structure_members`.
pub fn _check_structure_members_with(
    stmt: &Instruction,
    program: &ProgramRef,
    cfg: &Json,
    env: &Env,
    diags: &mut Diagnostics,
) {
    let Some(rules) = cfg.get("structure_members").and_then(Json::as_array) else {
        return;
    };
    if rules.is_empty() || stmt.args.is_empty() {
        return;
    }
    let Some(dest) = stmt.args[0].as_deref() else {
        return;
    };
    let dest_name = match dest {
        Expr::Name(n) => &n.name,
        Expr::Index(ix) => &ix.name,
        _ => return,
    };
    let var_types_map = context::of(program, None, None).variable_types(false);
    let Some(type_name) = var_types_map.get(&pystr::upper(dest_name)) else {
        return;
    };
    let members = _structure_members_with(program, type_name);
    for rule in rules {
        let member_key = rule.get("member").and_then(Json::as_str).unwrap_or("");
        let Some(member) = members.get(member_key) else {
            continue;
        };
        let member_type = pystr::upper(member.type_.as_deref().unwrap_or("FLOAT"));
        let expected_type = rule.get("type").and_then(Json::as_str).unwrap_or("");
        if member_type != expected_type {
            continue;
        }
        let min_size = rule
            .get("min_string_size")
            .and_then(Json::as_i64)
            .unwrap_or(0);
        if let Some(size) = _int_value_with(member.string_size.as_deref(), env)
            && size < min_size
        {
            let msg = rule.get("message").and_then(Json::as_str).unwrap_or("");
            diags.error_basic(stmt.base.node.line, msg, &stmt.base.node.file);
        }
    }
}

/// Port of `checker/instr_measure.py::_structure_members`.
pub fn _structure_members(_program: &Program, _type_name: &str) {
    todo!("checker/instr_measure.py::_structure_members")
}

/// Port of `checker/instr_measure.py::_structure_members`.
pub fn _structure_members_with(program: &Program, type_name: &str) -> OrderedMap<String, VarDecl> {
    let mut members: OrderedMap<String, VarDecl> = OrderedMap::new();
    let mut active = false;
    for candidate in &program.statements {
        if let Stmt::Instruction(inst) = candidate.as_ref() {
            let kw = pystr::upper(&inst.keyword);
            if kw == "STRUCTURETYPE" {
                active = inst.args.first().is_some_and(|a| {
                    matches!(a.as_deref(), Some(Expr::Name(n)) if pystr::upper(&n.name) == type_name)
                });
                continue;
            }
            if kw == "ENDSTRUCTURETYPE" && active {
                break;
            }
        }
        if active
            && let Stmt::Declaration(decl) = candidate.as_ref()
            && decl.kind == "STRUCTUREMEMBER"
        {
            for var in &decl.vars {
                members.insert(pystr::upper(&var.name), var.as_ref().clone());
            }
        }
    }
    members
}

/// Port of `checker/instr_measure.py::check_pass1_values`.
pub fn check_pass1_values(_program: &Program, _profile: &Profile, _diags: &mut Diagnostics) {
    todo!("checker/instr_measure.py::check_pass1_values (omitted: env: consteval.Env)")
}

/// Port of `checker/instr_measure.py::check_pass1_values`.
pub fn check_pass1_values_with(
    program: &ProgramRef,
    profile: &'static Profile,
    diags: &mut Diagnostics,
    env: &Env,
) {
    let measure = profile.measure();
    let fnotch = profile.fnotch();
    let ranges = measure.get("range_code_instructions");
    let fn_pos = if fnotch.get("min_hz").is_some() {
        fnotch.get("instructions")
    } else {
        None
    };
    if ranges.is_none() && fn_pos.is_none() {
        return;
    }
    let ctx = context::of(program, Some(profile), None);
    let variable_names: BTreeSet<String> = ctx
        .declared_variable_names()
        .iter()
        .filter(|name| !symbols::is_reserved_predefined_name_with_profile(name, profile))
        .cloned()
        .collect();
    let earlier: BTreeSet<(i64, String)> = diags
        .items
        .iter()
        .filter(|d| d.severity == "error")
        .map(|d| (d.line, d.file.clone()))
        .collect();
    let mut flat: Vec<StmtRef> = Vec::new();
    for st in &program.statements {
        flat.push(st.clone());
        if let Stmt::If(if_stmt) = st.as_ref() {
            if let Some(inline) = &if_stmt.inline {
                flat.extend(inline.iter().cloned());
            }
            if let Some(inline_else) = &if_stmt.inline_else {
                flat.extend(inline_else.iter().cloned());
            }
        }
    }
    let empty_consts = BTreeSet::new();
    let integ_num = scan_time::model_for(profile.name).map_or(0.0, |m| m.integ_num);
    for stmt_ref in &flat {
        let Stmt::Instruction(stmt) = stmt_ref.as_ref() else {
            continue;
        };
        let kw = pystr::upper(&stmt.keyword);
        let range_entry = ranges.and_then(|r| r.get(&kw));
        let fn_entry = fn_pos.and_then(|f| f.get(&kw));
        if range_entry.is_none() && fn_entry.is_none() {
            continue;
        }
        if earlier.contains(&(stmt.base.node.line, stmt.base.node.file.clone())) {
            continue;
        }
        let has_problem = stmt.args.iter().flatten().any(|a| {
            expressions::part4::_find_arity_problem_with(
                Some(a.as_ref()),
                profile,
                None,
                &empty_consts,
            )
            .is_some()
                || expressions::part4::_find_bare_function_ref_with(
                    Some(a.as_ref()),
                    profile,
                    &variable_names,
                )
        });
        if has_problem {
            continue;
        }
        if let Some(entry) = range_entry {
            if let Some(arr) = entry.as_array() {
                for p in arr {
                    if let Some(pos) = p.as_i64() {
                        _check_range_code_with(stmt, measure, pos, env, diags);
                    }
                }
            } else if let Some(pos) = entry.as_i64() {
                _check_range_code_with(stmt, measure, pos, env, diags);
            }
        }
        if let Some(pos) = fn_entry.and_then(Json::as_i64) {
            _check_fnotch_min_with(stmt, fnotch, pos, env, diags, integ_num);
        }
    }
}

fn _inf_name_value(name_upper: &str) -> Option<f64> {
    match name_upper {
        "INF" => Some(f64::INFINITY),
        "NAN" => Some(f64::NAN),
        _ => None,
    }
}

/// Port of `checker/instr_measure.py::_fnotch_value`.
pub fn _fnotch_value(_arg: Option<&Expr>) {
    todo!("checker/instr_measure.py::_fnotch_value (omitted: env: consteval.Env)")
}

/// Port of `checker/instr_measure.py::_fnotch_value`.
pub fn _fnotch_value_with(arg: Option<&Expr>, env: &Env) -> Option<ConstValue> {
    let mut inner = arg;
    let mut sign = 1.0_f64;
    while let Some(Expr::Unary(u)) = inner
        && u.op == "-"
    {
        inner = u.operand.as_deref();
        sign = -sign;
    }
    if let Some(Expr::Name(n)) = inner {
        let up = pystr::upper(&n.name);
        if let Some(val) = _inf_name_value(&up)
            && env.consts.borrow().get(&up).is_none()
        {
            return Some(ConstValue::Float(sign * val));
        }
    }
    _const_value_with(arg, env)
}

/// Port of `checker/instr_measure.py::_check_fnotch_min`.
pub fn _check_fnotch_min(
    _stmt: &Instruction,
    _pos: i64,
    _diags: &mut Diagnostics,
    _integ_num: f64,
) {
    todo!("checker/instr_measure.py::_check_fnotch_min (omitted: cfg: dict, env: consteval.Env)")
}

/// Port of `checker/instr_measure.py::_check_fnotch_min`.
pub fn _check_fnotch_min_with(
    stmt: &Instruction,
    cfg: &Json,
    pos: i64,
    env: &Env,
    diags: &mut Diagnostics,
    integ_num: f64,
) {
    let Ok(idx) = usize::try_from(pos - 1) else {
        return;
    };
    if idx >= stmt.args.len() {
        return;
    }
    let Some(arg) = stmt.args[idx].as_deref() else {
        return;
    };
    let Some(val) = _fnotch_value_with(Some(arg), env) else {
        return;
    };
    let bad = match val {
        ConstValue::Str(_) => true,
        ConstValue::Int(i) => _fnotch_float_bad(i as f64, cfg, integ_num),
        ConstValue::Float(v) | ConstValue::CFloat(v) => _fnotch_float_bad(v, cfg, integ_num),
        ConstValue::BigInt(s) => _fnotch_float_bad(
            pystr::parse_float(&s).unwrap_or(f64::INFINITY),
            cfg,
            integ_num,
        ),
    };
    if bad {
        let kw = pystr::upper(&stmt.keyword);
        let twice = cfg
            .get("twice")
            .and_then(Json::as_array)
            .is_some_and(|arr| arr.iter().any(|x| x.as_str() == Some(kw.as_str())));
        let times = if twice { 2 } else { 1 };
        let msg = cfg.get("message").and_then(Json::as_str).unwrap_or("");
        for _ in 0..times {
            diags.error_basic(stmt.base.node.line, msg, &stmt.base.node.file);
        }
    }
}

fn _fnotch_float_bad(value: f64, cfg: &Json, integ_num: f64) -> bool {
    let f = scan_time::_f32(value);
    if f.is_nan() {
        return true;
    }
    if f == 0.0 || f.abs() == f64::INFINITY {
        return false;
    }
    if f > 0.0 {
        let min_hz = cfg.get("min_hz").and_then(Json::as_f64).unwrap_or(0.0);
        value < min_hz
    } else {
        scan_time::_f32(scan_time::_f32(integ_num) / f) <= -1.5
    }
}

/// Port of `checker/instr_measure.py::_const_value`.
pub fn _const_value(_arg: Option<&Expr>) {
    todo!("checker/instr_measure.py::_const_value (omitted: env: consteval.Env)")
}

/// Port of `checker/instr_measure.py::_const_value`.
pub fn _const_value_with(arg: Option<&Expr>, env: &Env) -> Option<ConstValue> {
    let arg = arg?;
    if !consteval::is_constant(Some(arg), env) {
        return None;
    }
    consteval::value_of(Some(arg), env).ok()
}

/// Port of `checker/instr_measure.py::_int_value`.
pub fn _int_value(_arg: Option<&Expr>) -> Option<i64> {
    todo!("checker/instr_measure.py::_int_value (omitted: env: consteval.Env)")
}

/// Port of `checker/instr_measure.py::_int_value`.
pub fn _int_value_with(arg: Option<&Expr>, env: &Env) -> Option<i64> {
    match _const_value_with(arg, env)? {
        ConstValue::Int(i) => Some(i),
        ConstValue::Float(f) | ConstValue::CFloat(f) if f.is_finite() => pystr::py_int_from_f64(f),
        _ => None,
    }
}

/// Port of `checker/instr_measure.py::_check_gps_nmea`.
pub fn _check_gps_nmea(_stmt: &Instruction, _program: &Program, _diags: &mut Diagnostics) {
    todo!("checker/instr_measure.py::_check_gps_nmea (omitted: env: consteval.Env)")
}

/// Port of `checker/instr_measure.py::_check_gps_nmea`.
pub fn _check_gps_nmea_with(
    stmt_ref: &StmtRef,
    program: &ProgramRef,
    env: &Env,
    diags: &mut Diagnostics,
) {
    let Stmt::Instruction(stmt) = stmt_ref.as_ref() else {
        return;
    };
    if stmt.args.len() < 5 {
        return;
    }
    let Some(arg) = stmt.args[4].as_deref() else {
        return;
    };
    let bad = match arg {
        Expr::String(_) => true,
        Expr::Name(_) | Expr::Index(_) => {
            let arg_name = match arg {
                Expr::Name(n) => pystr::upper(&n.name),
                Expr::Index(ix) => pystr::upper(&ix.name),
                _ => unreachable!(),
            };
            let view = context::of(program, None, None).scoped_view(stmt_ref, false);
            let decl = view.statements.iter().rev().find_map(|st| {
                if let Stmt::Declaration(d) = st.as_ref()
                    && matches!(d.kind.as_str(), "PUBLIC" | "DIM" | "STATIC")
                {
                    d.vars.iter().find(|v| pystr::upper(&v.name) == arg_name)
                } else {
                    None
                }
            });
            let Some(decl) = decl else {
                return;
            };
            let decl_type = pystr::upper(decl.type_.as_deref().unwrap_or("FLOAT"));
            if decl_type == "STRING" {
                let size_expr = match decl.string_size.as_deref() {
                    Some(Expr::Binary(b)) if b.op == "=" => b.left.as_deref(),
                    other => other,
                };
                let size = if let Some(se) = size_expr {
                    _int_value_with(Some(se), env)
                } else {
                    Some(24)
                };
                size.is_some_and(|s| s < 96)
            } else {
                matches!(decl_type.as_str(), "FLOAT" | "LONG" | "BOOLEAN" | "DOUBLE")
            }
        }
        _ => return,
    };
    if bad {
        diags.error_basic(
            stmt.base.node.line,
            "Optional parameter must be an array of type string of length >= 100",
            &stmt.base.node.file,
        );
    }
}

/// Port of `checker/instr_measure.py::_check_gps_port`.
pub fn _check_gps_port(_stmt: &Instruction, _diags: &mut Diagnostics) {
    todo!("checker/instr_measure.py::_check_gps_port (omitted: cfg: dict, env: consteval.Env)")
}

/// Port of `checker/instr_measure.py::_check_gps_port`.
pub fn _check_gps_port_with(stmt: &Instruction, cfg: &Json, env: &Env, diags: &mut Diagnostics) {
    if stmt.args.len() < 2 {
        return;
    }
    let Some(port) = _int_value_with(stmt.args[1].as_deref(), env) else {
        return;
    };
    let valid_ports = cfg
        .get("ports")
        .and_then(Json::as_array)
        .is_some_and(|arr| arr.iter().any(|p| p.as_i64() == Some(port.abs())));
    if !valid_ports {
        let msg = cfg.get("port_message").and_then(Json::as_str).unwrap_or("");
        diags.error_basic(stmt.base.node.line, msg, &stmt.base.node.file);
    } else if port > 0 && Some(port) != cfg.get("pps_port").and_then(Json::as_i64) {
        let msg = cfg.get("pps_message").and_then(Json::as_str).unwrap_or("");
        diags.error_basic(stmt.base.node.line, msg, &stmt.base.node.file);
    }
}

/// Port of `checker/instr_measure.py::_check_range_code`.
pub fn _check_range_code(_stmt: &Instruction, _pos: i64, _diags: &mut Diagnostics) {
    todo!(
        "checker/instr_measure.py::_check_range_code (omitted: measure: dict, env: consteval.Env)"
    )
}

/// Port of `checker/instr_measure.py::_check_range_code`.
pub fn _check_range_code_with(
    stmt: &Instruction,
    measure: &Json,
    pos: i64,
    env: &Env,
    diags: &mut Diagnostics,
) {
    let Ok(idx) = usize::try_from(pos - 1) else {
        return;
    };
    if idx >= stmt.args.len() {
        return;
    }
    let Some(value) = _const_value_with(stmt.args[idx].as_deref(), env) else {
        return;
    };
    let is_valid = match value {
        ConstValue::Str(_) | ConstValue::BigInt(_) => false,
        ConstValue::Int(i) => measure
            .get("range_codes")
            .and_then(Json::as_array)
            .is_some_and(|arr| arr.iter().any(|v| v.as_i64() == Some(i))),
        ConstValue::Float(f) | ConstValue::CFloat(f) => {
            pystr::py_int_from_f64(f).is_some_and(|i| {
                measure
                    .get("range_codes")
                    .and_then(Json::as_array)
                    .is_some_and(|arr| arr.iter().any(|v| v.as_i64() == Some(i)))
            })
        }
    };
    if !is_valid {
        let msg = measure
            .get("range_code_message")
            .and_then(Json::as_str)
            .unwrap_or("");
        diags.error_basic(stmt.base.node.line, msg, &stmt.base.node.file);
    }
}

/// Port of `checker/instr_measure.py::_check_sw12`.
pub fn _check_sw12(_stmt: &Instruction, _diags: &mut Diagnostics) {
    todo!("checker/instr_measure.py::_check_sw12 (omitted: measure: dict, env: consteval.Env)")
}

/// Port of `checker/instr_measure.py::_check_sw12`.
pub fn _check_sw12_with(stmt: &Instruction, measure: &Json, env: &Env, diags: &mut Diagnostics) {
    if stmt.args.is_empty() {
        return;
    }
    let mut value = _int_value_with(stmt.args[0].as_deref(), env);
    if matches!(stmt.args[0].as_deref(), Some(Expr::String(_)))
        && measure
            .get("string_port_int_min")
            .and_then(Json::as_bool)
            .unwrap_or(false)
    {
        value = Some(-2_147_483_648);
    }
    let Some(value) = value else {
        return;
    };
    let valid = measure
        .get("sw12_ports")
        .and_then(Json::as_array)
        .is_some_and(|arr| arr.iter().any(|v| v.as_i64() == Some(value)));
    if !valid {
        let key = value.to_string();
        let text = measure
            .get("sw12_port_messages")
            .and_then(|m| m.get(&key))
            .and_then(Json::as_str)
            .map_or_else(|| format!("Invalid SW12 port {value}."), str::to_owned);
        diags.error_basic(stmt.base.node.line, &text, &stmt.base.node.file);
    }
}

pub const _DELAY_US_PER_UNIT: [(i64, i64); 6] = [
    (0, 1),
    (1, 1_000),
    (2, 1_000_000),
    (3, 60_000_000),
    (4, 3_600_000_000),
    (5, 86_400_000_000),
];

fn delay_us_per_unit(unit: i64) -> Option<i64> {
    _DELAY_US_PER_UNIT
        .iter()
        .find(|(k, _)| *k == unit)
        .map(|(_, v)| *v)
}

/// Port of `checker/instr_measure.py::_delay_option_byte`.
pub fn _delay_option_byte(value: i64) -> i64 {
    if !(-(1_i64 << 31)..(1_i64 << 31)).contains(&value) {
        return 0;
    }
    pystr::pymod_i64(value, 256).unwrap_or(0)
}

pub const _TASK_MEMORY_COPIES: usize = 100;

/// Port of `checker/instr_measure.py::_task_memory_full`.
pub fn _task_memory_full(stmt: &Instruction, diags: &mut Diagnostics) {
    for _ in 0.._TASK_MEMORY_COPIES {
        diags.error_basic(
            stmt.base.node.line,
            "Program too big for task memory",
            &stmt.base.node.file,
        );
    }
}

#[cfg(test)]
#[allow(clippy::unnecessary_wraps)]
mod tests {
    use super::*;
    use crate::profiles;
    use crate::syntax::{Binary, Name, Node, Number, NumberValue, StmtBase, StringLit, Unary};

    fn nd() -> Node {
        Node {
            line: 1,
            file: "test.crb".to_owned(),
        }
    }

    fn sb(line: i64) -> StmtBase {
        StmtBase {
            node: Node {
                line,
                file: "test.crb".to_owned(),
            },
            ..Default::default()
        }
    }

    fn num_i(i: i64) -> Option<crate::syntax::ExprRef> {
        Some(Rc::new(Expr::Number(Number {
            node: nd(),
            text: i.to_string(),
            value: NumberValue::Int(i),
        })))
    }

    fn num_f(f: f64, text: &str) -> Option<crate::syntax::ExprRef> {
        Some(Rc::new(Expr::Number(Number {
            node: nd(),
            text: text.to_owned(),
            value: NumberValue::Float(f),
        })))
    }

    fn str_lit(s: &str) -> Option<crate::syntax::ExprRef> {
        Some(Rc::new(Expr::String(StringLit {
            node: nd(),
            value: s.to_owned(),
        })))
    }

    fn name_expr(s: &str) -> Option<crate::syntax::ExprRef> {
        Some(Rc::new(Expr::Name(Name {
            node: nd(),
            name: s.to_owned(),
        })))
    }

    fn instr(line: i64, kw: &str, args: Vec<Option<crate::syntax::ExprRef>>) -> Instruction {
        Instruction {
            base: sb(line),
            keyword: kw.to_owned(),
            args,
            ..Default::default()
        }
    }

    #[test]
    fn delay_option_byte_matches_python() {
        assert_eq!(_delay_option_byte(0), 0);
        assert_eq!(_delay_option_byte(257), 1);
        assert_eq!(_delay_option_byte(-1), 255);
        assert_eq!(_delay_option_byte(-254), 2);
        assert_eq!(_delay_option_byte(1_i64 << 31), 0);
        assert_eq!(_delay_option_byte(-(1_i64 << 31) - 1), 0);
    }

    #[test]
    fn task_memory_full_emits_100_copies() {
        let st = instr(10, "ExciteV", vec![]);
        let mut diags = Diagnostics::new();
        _task_memory_full(&st, &mut diags);
        assert_eq!(diags.items.len(), 100);
        assert_eq!(diags.items[0].message, "Program too big for task memory");
    }

    #[test]
    fn sw12_checks_ports_and_string_int_min() {
        let cr6 = profiles::get("cr6").unwrap();
        let env = Env::new(cr6);
        let mut diags = Diagnostics::new();

        let ok = instr(1, "SW12", vec![num_i(1), num_i(1)]);
        _check_sw12_with(&ok, cr6.measure(), &env, &mut diags);
        assert_eq!(diags.items.len(), 0);

        let bad = instr(2, "SW12", vec![num_i(99), num_i(1)]);
        _check_sw12_with(&bad, cr6.measure(), &env, &mut diags);
        assert_eq!(diags.items.len(), 1);
        assert_eq!(diags.items[0].message, "Invalid SW12 port 99.");

        let str_port = instr(3, "SW12", vec![str_lit("x"), num_i(0)]);
        _check_sw12_with(&str_port, cr6.measure(), &env, &mut diags);
        assert_eq!(diags.items.len(), 2);
        assert_eq!(diags.items[1].message, "Invalid SW12 port -2147483648.");
    }

    #[test]
    fn range_code_and_fnotch_min() {
        let cr6 = profiles::get("cr6").unwrap();
        let env = Env::new(cr6);
        let mut diags = Diagnostics::new();

        let ok_rc = instr(1, "VoltSe", vec![name_expr("x"), num_i(1), num_i(800)]);
        _check_range_code_with(&ok_rc, cr6.measure(), 3, &env, &mut diags);
        assert_eq!(diags.items.len(), 0);

        let bad_rc = instr(2, "VoltSe", vec![name_expr("x"), num_i(1), str_lit("800")]);
        _check_range_code_with(&bad_rc, cr6.measure(), 3, &env, &mut diags);
        assert_eq!(diags.items.len(), 1);
        assert!(diags.items[0].message.contains("Invalid range code"));

        // fN1: 0 and INF pass; 1.0 (< 1.430525) fails twice for THERM107
        let ok_fn = instr(3, "PanelTemp", vec![name_expr("p"), num_f(0.0, "0")]);
        _check_fnotch_min_with(
            &ok_fn,
            cr6.fnotch(),
            2,
            &env,
            &mut diags,
            scan_time::CR6_MODEL.integ_num,
        );
        assert_eq!(diags.items.len(), 1);

        let neg_inf = Some(Rc::new(Expr::Unary(Unary {
            node: nd(),
            op: "-".to_owned(),
            operand: name_expr("INF"),
        })));
        let ok_inf = instr(4, "PanelTemp", vec![name_expr("p"), neg_inf]);
        _check_fnotch_min_with(
            &ok_inf,
            cr6.fnotch(),
            2,
            &env,
            &mut diags,
            scan_time::CR6_MODEL.integ_num,
        );
        assert_eq!(diags.items.len(), 1);

        let bad_fn = instr(
            5,
            "Therm107",
            vec![
                name_expr("x"),
                num_i(1),
                num_i(1),
                num_i(1),
                num_i(0),
                num_f(1.0, "1.0"),
            ],
        );
        _check_fnotch_min_with(
            &bad_fn,
            cr6.fnotch(),
            6,
            &env,
            &mut diags,
            scan_time::CR6_MODEL.integ_num,
        );
        assert_eq!(diags.items.len(), 3);
        assert_eq!(
            diags.items[1].message,
            "Invalid fnotch, minimum allowed is 2 Hz."
        );
        assert_eq!(
            diags.items[2].message,
            "Invalid fnotch, minimum allowed is 2 Hz."
        );
    }

    #[test]
    fn gps_port_and_ch400status() {
        let cr6 = profiles::get("cr6").unwrap();
        let env = Env::new(cr6);
        let mut diags = Diagnostics::new();
        let gps_cfg = cr6.measure().get("gps").unwrap();

        let bad_port = instr(1, "GPS", vec![name_expr("g"), num_i(2)]);
        _check_gps_port_with(&bad_port, gps_cfg, &env, &mut diags);
        assert_eq!(diags.items.len(), 1);

        let bad_pps = instr(2, "GPS", vec![name_expr("g"), num_i(1)]);
        _check_gps_port_with(&bad_pps, gps_cfg, &env, &mut diags);
        assert_eq!(diags.items.len(), 2);

        let ok_pps = instr(3, "GPS", vec![name_expr("g"), num_i(11)]);
        _check_gps_port_with(&ok_pps, gps_cfg, &env, &mut diags);
        assert_eq!(diags.items.len(), 2);

        let bad_ch400 = instr(4, "CH400Status", vec![num_i(500), num_i(1)]);
        _check_ch400status_with(&bad_ch400, &env, &mut diags);
        assert_eq!(diags.items.len(), 3);
        assert_eq!(
            diags.items[2].message,
            "CH400Status update interval must be an integer multiple of 1 second."
        );

        let ok_ch400 = instr(5, "CH400Status", vec![num_i(1000), num_i(1)]);
        _check_ch400status_with(&ok_ch400, &env, &mut diags);
        assert_eq!(diags.items.len(), 3);
    }

    #[test]
    fn name_text_and_string_prefix() {
        let cr6 = profiles::get("cr6").unwrap();
        let env = Env::new(cr6);
        let mut diags = Diagnostics::new();

        let concat = Expr::Binary(Binary {
            node: nd(),
            op: "&".to_owned(),
            left: str_lit("CRD:out_"),
            right: Some(Rc::new(Expr::Member(crate::syntax::Member {
                node: nd(),
                base: name_expr("Status"),
                member: "SerialNumber".to_owned(),
                args: None,
            }))),
        });
        assert_eq!(
            _name_text_with(Some(&concat), &env).as_deref(),
            Some("CRD:out_")
        );

        let rule = cr6
            .measure()
            .get("string_prefixes")
            .and_then(Json::as_array)
            .unwrap()[0]
            .clone();
        let bad_drive = instr(1, "FileManage", vec![str_lit("XYZ:foo.cr6"), num_i(0)]);
        assert!(_check_string_prefix_with(
            &bad_drive, &rule, &env, &mut diags
        ));
        assert_eq!(diags.items[0].message, "FileManage invalid drive: XYZ:.");

        let bad_ext = instr(2, "FileManage", vec![str_lit("CPU:foo.txt"), num_i(2)]);
        assert!(_check_string_prefix_with(&bad_ext, &rule, &env, &mut diags));
        assert_eq!(
            diags.items[1].message,
            "FileManage name CPU:foo.txt invalid extension."
        );
    }

    #[test]
    fn periodavg_channels_cr300_and_cr6() {
        let cr300 = profiles::get("cr300").unwrap();
        let env = Env::new(cr300);
        let mut diags = Diagnostics::new();
        let st = instr(
            1,
            "PeriodAvg",
            vec![
                name_expr("d"),
                num_i(3),
                num_i(3),
                num_i(0),
                num_i(0),
                num_i(10),
                num_i(10),
                num_i(1),
            ],
        );
        _check_periodavg_channels_with(&st, cr300, &env, &mut diags);
        assert_eq!(diags.items.len(), 1);
        assert_eq!(diags.items[0].message, "Reps exceeds valid channels.");

        let st_port = instr(
            2,
            "PeriodAvg",
            vec![
                name_expr("d"),
                num_i(1),
                num_i(8),
                num_i(0),
                num_i(0),
                num_i(10),
                num_i(10),
                num_i(1),
            ],
        );
        _check_periodavg_channels_with(&st_port, cr300, &env, &mut diags);
        assert_eq!(diags.items.len(), 4);
        assert_eq!(diags.items[1].message, "Invalid channel: 8.");
        assert_eq!(diags.items[2].message, "Reps exceeds valid channels.");
        assert_eq!(diags.items[3].message, "RS232 is not a valid control port.");
    }
}
