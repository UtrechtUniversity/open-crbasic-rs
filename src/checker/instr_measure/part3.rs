//! Port of `checker/instr_measure.py` (lines 1541-1803).

#![allow(dead_code, unused_variables, unused_imports)]

use std::collections::BTreeSet;
use std::rc::Rc;

use crate::checker::context::Context;
use crate::checker::expressions::_arg_text;
use crate::checker::longconv::{float_names, literal_names, long_value_with};
use crate::consteval::{Env, to_long, value_of};
use crate::diagnostics::{DiagRef, Diagnostic, Diagnostics, ShowFile};
use crate::profiles::Profile;
use crate::pystr;
use crate::syntax::{
    Assignment, Call, Case, ConstDecl, ConstValue, Declaration, Expr, ExprRef, Index, Instruction,
    Juxtaposed, Member, Number, NumberValue, OrderedMap, Program, ProgramRef, Stmt, StmtRef,
    SubDef, Unparsed, VarDecl,
};

fn p3_byte(value: i64) -> i64 {
    value & 0xff
}

fn p3_const_value(arg: Option<&Expr>, env: &Env) -> Option<ConstValue> {
    value_of(arg, env).ok()
}

fn p3_int_value(arg: Option<&Expr>, env: &Env) -> Option<i64> {
    match p3_const_value(arg, env)? {
        ConstValue::Int(v) => Some(v),
        _ => None,
    }
}

fn p3_builder_fixed(kw: &str) -> Option<&'static [(usize, i64)]> {
    match kw {
        "THERM107" | "THERM108" | "THERM109" => Some(&[(0, 1)]),
        _ => None,
    }
}

fn p3_sdm_mode_values(kw: &str) -> Option<&'static [i64]> {
    match kw {
        "SDMINT8" | "SDMAVW" => Some(&[1, 1, 1, 1, 1, 1, 1, 1]),
        _ => None,
    }
}

const P3_TGA_VALUES: [[i64; 3]; 5] = [[4, 4, 4], [4, 4, 4], [4, 4, 4], [4, 4, 4], [4, 4, 4]];

fn p3_ec100_value(code: i64) -> Option<i64> {
    match code {
        0..=4 => Some(12),
        _ => None,
    }
}

fn p3_ec100_invalid_count(code: i64) -> Option<i64> {
    let _ = code;
    None
}

fn p3_is_running_stat(kw: &str) -> bool {
    matches!(
        kw,
        "AVGRUN" | "MAXRUN" | "MINRUN" | "RMSRUN" | "STDDEVRUN" | "TOTALRUN"
    )
}

fn p3_dest_fixed(kw: &str) -> Option<i64> {
    let _ = kw;
    None
}

fn p3_dest_per_rep(kw: &str) -> Option<i64> {
    let _ = kw;
    None
}

fn p3_dest_negative_oob(kw: &str) -> bool {
    let _ = kw;
    false
}

fn p3_dest_kinds(kw: &str) -> &'static [&'static str] {
    let _ = kw;
    &["FLOAT"]
}

/// Port of `checker/instr_measure.py::_builder_counts`.
pub fn _builder_counts(_kw: &str) {
    todo!("checker/instr_measure.py::_builder_counts (omitted: args: list, env: consteval.Env)")
}

/// Port of `checker/instr_measure.py::_builder_counts`.
pub fn _builder_counts_with(kw: &str, args: &[Option<ExprRef>], env: &Env) -> Vec<(usize, i64)> {
    let arg = |i: usize| -> Option<i64> {
        if i < args.len() {
            p3_int_value(args[i].as_deref(), env)
        } else {
            None
        }
    };

    if let Some(pairs) = p3_builder_fixed(kw) {
        return pairs
            .iter()
            .copied()
            .filter(|(i, _)| *i < args.len())
            .collect();
    }
    if kw == "SDMGENERIC" {
        return match arg(5) {
            Some(count) => vec![(0, count)],
            None => Vec::new(),
        };
    }
    if kw == "SDMSW8A" {
        return match arg(1) {
            Some(count) => vec![(0, p3_byte(count))],
            None => Vec::new(),
        };
    }
    if kw == "SDMCD16AC" {
        return match arg(1) {
            Some(count) => vec![(0, 16 * p3_byte(count))],
            None => Vec::new(),
        };
    }
    if kw == "CSAT3" {
        let Some(count) = arg(1) else {
            return Vec::new();
        };
        let reps = p3_byte(count);
        return if reps < 128 {
            vec![(0, 5 * reps)]
        } else {
            Vec::new()
        };
    }
    if kw == "PEAKVALLEY" {
        return match arg(2) {
            Some(count) => [0usize, 1, 3]
                .into_iter()
                .filter(|&i| i < args.len())
                .map(|i| (i, count))
                .collect(),
            None => Vec::new(),
        };
    }
    if let Some(table) = p3_sdm_mode_values(kw) {
        let (Some(reps), Some(mode)) = (arg(1), arg(3)) else {
            return Vec::new();
        };
        let m = p3_byte(mode);
        if m > 7 {
            return Vec::new();
        }
        return vec![(0, table[usize::try_from(m).unwrap_or(0)] * p3_byte(reps))];
    }
    if kw == "TGA" {
        let (Some(first), Some(second)) = (arg(2), arg(3)) else {
            return Vec::new();
        };
        let f = p3_byte(first);
        let s = p3_byte(second);
        if !((1..=5).contains(&f) && (1..=3).contains(&s)) {
            return Vec::new();
        }
        return vec![(
            0,
            P3_TGA_VALUES[usize::try_from(f - 1).unwrap_or(0)][usize::try_from(s - 1).unwrap_or(0)],
        )];
    }
    Vec::new()
}

fn p3_spa_dest(kw: &str) -> Option<(i64, i64)> {
    match kw {
        "MAXSPA" | "MINSPA" => Some((2, 0)),
        "SORTSPA" => Some((0, 1)),
        "SORTSPAINDEXED" => Some((0, 2)),
        _ => None,
    }
}

fn p3_is_spa_typed(kw: &str) -> bool {
    matches!(kw, "MAXSPA" | "MINSPA" | "SORTSPA")
}

fn p3_is_spa_source(kw: &str) -> bool {
    matches!(
        kw,
        "AVGSPA" | "MAXSPA" | "MINSPA" | "RMSSPA" | "STDDEVSPA" | "SORTSPA" | "SORTSPAINDEXED"
    )
}

fn p3_expr_name(expr: &Expr) -> Option<&str> {
    match expr {
        Expr::Name(n) => Some(n.name.as_str()),
        Expr::Index(ix) => Some(ix.name.as_str()),
        _ => None,
    }
}

fn p3_overruns(
    arg: Option<&Expr>,
    need: i64,
    negative: bool,
    kinds: &[&str],
    dims: &OrderedMap<String, Vec<i64>>,
    types: &OrderedMap<String, String>,
) -> bool {
    let Some(expr) = arg else {
        return false;
    };
    let Some(raw_name) = p3_expr_name(expr) else {
        return false;
    };
    let key = pystr::upper(raw_name);
    let Some(shape) = dims.get(&key) else {
        return false;
    };
    let Some(kind) = types.get(&key) else {
        return false;
    };
    if !kinds.contains(&kind.as_str()) {
        return false;
    }
    let Some(offset) = _flat_offset_with(expr, shape, None) else {
        return false;
    };
    let mut total: i64 = 1;
    for &n in shape {
        total = total.saturating_mul(n);
    }
    if offset >= total {
        return false;
    }
    negative || need > total - offset
}

/// Port of `checker/instr_measure.py::check_dest_bounds`.
pub fn check_dest_bounds(_program: &Program, _profile: &Profile, _diags: &mut Diagnostics) {
    todo!("checker/instr_measure.py::check_dest_bounds (omitted: env: consteval.Env)")
}

/// Port of `checker/instr_measure.py::check_dest_bounds`.
pub fn check_dest_bounds_with(
    program: &ProgramRef,
    profile: &'static Profile,
    diags: &mut Diagnostics,
    env: &Env,
) {
    let ctx = Rc::new(Context::new(
        Rc::clone(program),
        Some(profile),
        Some(env.clone()),
    ));
    let tables = ctx.scope_tables(true, false);
    let with_parameters = ctx.scope_tables(true, true);
    let fnames = BTreeSet::new();
    let lnames = BTreeSet::new();

    for stmt in &program.statements {
        let Stmt::Instruction(inst) = stmt.as_ref() else {
            continue;
        };
        let mut scope = tables.for_statement(stmt);
        let kw = pystr::upper(&inst.keyword);

        if kw == "AVW200" && inst.args.len() >= 12 {
            let target = inst.args[4].as_deref();
            let kind = target
                .and_then(p3_expr_name)
                .and_then(|n| scope.types.get(&pystr::upper(n)))
                .map(String::as_str);
            if profile.single_pass && matches!(kind, Some("LONG" | "BOOLEAN" | "STRING")) {
                diags.error(
                    inst.base.node.line,
                    "Illegal destination variable type, expecting Float.",
                    &inst.base.node.file,
                    ShowFile::Bool(false),
                    0,
                    "",
                    "",
                );
                let _ = _arg_text(inst, 5);
                continue;
            }
            let mode = long_value_with(inst.args[11].as_deref(), env, profile, &fnames, &lnames);
            let valid_modes: &[i64] = if profile.single_pass {
                &[1, 2, 20]
            } else {
                &[50, 60]
            };
            if let Some(m) = mode {
                if m != 0 && !valid_modes.contains(&m) {
                    diags.error(
                        inst.base.node.line,
                        "Illegal Thermister integration; must be _60Hz, _50Hz, or 0",
                        &inst.base.node.file,
                        ShowFile::Bool(false),
                        0,
                        "",
                        "",
                    );
                }
            }
            let last = p3_int_value(inst.args[7].as_deref(), env);
            let mult = if mode.is_some_and(|m| valid_modes.contains(&m)) {
                6
            } else {
                5
            };
            let count = mult * last.unwrap_or(1);
            if let Some(t) = target {
                if let Some(tname) = p3_expr_name(t) {
                    if p3_overruns(
                        Some(t),
                        count,
                        count < 0,
                        &["FLOAT"],
                        &scope.dims,
                        &scope.types,
                    ) {
                        diags.error(
                            inst.base.node.line,
                            &format!("Variable {tname} out of bounds."),
                            &inst.base.node.file,
                            ShowFile::Bool(false),
                            0,
                            "",
                            "",
                        );
                    }
                }
            }
            if let Some(l) = last {
                if !(1..=32).contains(&l) {
                    diags.error(inst.base.node.line, "Bad parameter(s): address 1..4095, chan and muxchan 1..32, reps 1..32-muxchan-1,beg and end freq  100..65535, Vx 1..2", &inst.base.node.file, ShowFile::Bool(false), 0, "", "");
                }
            }
            continue;
        }

        if kw == "EC100" && inst.args.len() >= 3 {
            let option_arg = inst.args[2].as_deref();
            let mut option = long_value_with(option_arg, env, profile, &fnames, &lnames);
            if let Some(Expr::Name(n)) = option_arg {
                if pystr::upper(&n.name) == "INF" && env.consts.borrow().get("INF").is_none() {
                    option = Some(i64::from(i32::MAX));
                }
            }
            if option.is_none() {
                if let Some(folded) = p3_int_value(option_arg, env) {
                    option = Some(to_long(&ConstValue::Int(folded)));
                }
            }
            let Some(opt) = option else {
                continue;
            };
            let code = if opt == 2718 { 4 } else { p3_byte(opt) };
            let invalid = opt != 2718 && code > 3;
            if invalid {
                diags.error(
                    inst.base.node.line,
                    "Invalid output option; range 0..3.",
                    &inst.base.node.file,
                    ShowFile::Bool(false),
                    0,
                    "",
                    "",
                );
            }
            let count = if invalid {
                p3_ec100_invalid_count(code)
            } else {
                p3_ec100_value(code)
            };
            if let (Some(cnt), Some(t)) = (count, inst.args[0].as_deref()) {
                if let Some(tname) = p3_expr_name(t) {
                    if p3_overruns(Some(t), cnt, false, &["FLOAT"], &scope.dims, &scope.types) {
                        diags.error(
                            inst.base.node.line,
                            &format!("Variable {tname} out of bounds."),
                            &inst.base.node.file,
                            ShowFile::Bool(false),
                            0,
                            "",
                            "",
                        );
                    }
                }
            }
            continue;
        }

        if (kw == "CDM_TCCOMP" || kw == "CDM_PANELTEMP")
            && inst.args.len() > 3
            && inst.args[2].as_deref().and_then(p3_expr_name).is_some()
        {
            let reps = long_value_with(inst.args[3].as_deref(), env, profile, &fnames, &lnames);
            if let Some(r) = reps {
                if p3_overruns(
                    inst.args[2].as_deref(),
                    r,
                    r < 0,
                    &["FLOAT"],
                    &scope.dims,
                    &scope.types,
                ) {
                    if let Some(tname) = inst.args[2].as_deref().and_then(p3_expr_name) {
                        diags.error(
                            inst.base.node.line,
                            &format!("Variable {tname} out of bounds."),
                            &inst.base.node.file,
                            ShowFile::Bool(false),
                            0,
                            "",
                            "",
                        );
                    }
                }
            }
            continue;
        }

        let Some(Some(dest_rc)) = inst.args.first() else {
            continue;
        };
        let dest = dest_rc.as_ref();
        let Some(dest_name) = p3_expr_name(dest) else {
            continue;
        };
        if p3_is_running_stat(&kw) {
            scope = with_parameters.for_statement(stmt);
        }
        let name = pystr::upper(dest_name);
        let mut bad: Vec<&Expr> = Vec::new();

        if p3_is_spa_source(&kw) && inst.args.len() > 2 {
            if let Some(swath) = p3_int_value(inst.args[1].as_deref(), env) {
                let (fixed, per) = p3_spa_dest(&kw).unwrap_or((0, 0));
                if (fixed != 0 || per != 0)
                    && p3_overruns(
                        Some(dest),
                        fixed + per * swath,
                        per > 0 && swath < 0,
                        &["FLOAT", "LONG"],
                        &scope.dims,
                        &scope.types,
                    )
                {
                    bad.push(dest);
                }
                if let Some(source) = inst.args[2].as_deref() {
                    if p3_overruns(
                        Some(source),
                        swath,
                        swath < 0,
                        &["FLOAT", "LONG"],
                        &scope.dims,
                        &scope.types,
                    ) {
                        bad.push(source);
                    }
                }
            }
        } else if kw == "SPLITSTR" {
            let reps = if inst.args.len() > 3 {
                p3_int_value(inst.args[3].as_deref(), env)
            } else {
                None
            };
            if matches!(dest, Expr::Name(_))
                && scope.dims.contains_key(&name)
                && !scope.arrays.contains(&name)
                && reps.is_some_and(|r| r > 1)
                && matches!(
                    scope.types.get(&name).map(String::as_str),
                    Some("FLOAT" | "STRING")
                )
            {
                bad.push(dest);
            }
        } else if kw == "CSAT3"
            && inst.args.len() > 1
            && p3_int_value(inst.args[1].as_deref(), env).is_some_and(|r| p3_byte(r) >= 128)
        {
            diags.error(
                inst.base.node.line,
                "Out of memory.",
                &inst.base.node.file,
                ShowFile::Bool(false),
                0,
                "out_of_memory",
                "",
            );
            return;
        } else if p3_builder_fixed(&kw).is_some()
            || matches!(
                kw.as_str(),
                "SDMGENERIC" | "SDMSW8A" | "SDMCD16AC" | "CSAT3" | "PEAKVALLEY" | "TGA"
            )
            || p3_sdm_mode_values(&kw).is_some()
        {
            let kinds: &[&str] = if kw == "SDMGENERIC" {
                &["FLOAT", "LONG", "BOOLEAN"]
            } else {
                &["FLOAT"]
            };
            for (i, need) in _builder_counts_with(&kw, &inst.args, env) {
                if let Some(target) = inst.args.get(i).and_then(|a| a.as_deref()) {
                    if p3_overruns(
                        Some(target),
                        need,
                        need < 0,
                        kinds,
                        &scope.dims,
                        &scope.types,
                    ) {
                        bad.push(target);
                    }
                }
            }
        } else if p3_dest_fixed(&kw).is_some() || p3_dest_per_rep(&kw).is_some() {
            let (need, negative) = if let Some(fixed) = p3_dest_fixed(&kw) {
                (fixed, false)
            } else {
                let reps = if inst.args.len() > 1 {
                    p3_int_value(inst.args[1].as_deref(), env)
                } else {
                    None
                };
                let Some(r) = reps else {
                    continue;
                };
                if r < 0 && !p3_dest_negative_oob(&kw) {
                    continue;
                }
                let per = p3_dest_per_rep(&kw).unwrap_or(1);
                (r * per, r < 0)
            };
            if p3_overruns(
                Some(dest),
                need,
                negative,
                p3_dest_kinds(&kw),
                &scope.dims,
                &scope.types,
            ) {
                bad.push(dest);
            }
        }

        let ordered_bad: Vec<&Expr> = if profile.single_pass {
            bad.into_iter().rev().collect()
        } else {
            bad
        };
        for arg in ordered_bad {
            let arg_name = p3_expr_name(arg).unwrap_or("");
            let diag = Diagnostic::new(
                "error",
                inst.base.node.line,
                &format!("Variable {arg_name} out of bounds."),
                &inst.base.node.file,
                ShowFile::Bool(false),
                0,
                "",
                "",
            );
            if profile.single_pass {
                let at = diags
                    .items
                    .iter()
                    .position(|d| d.line == inst.base.node.line && d.file == inst.base.node.file)
                    .unwrap_or(diags.items.len());
                diags.items.insert(at, Rc::new(diag));
            } else {
                diags.items.push(Rc::new(diag));
            }
        }

        if p3_is_spa_typed(&kw)
            && inst.args.len() > 2
            && (profile.name == "cr6" || profile.name == "cr1000x")
        {
            let dest_type = scope.types.get(&name).map(String::as_str);
            let source_type = inst.args[2]
                .as_deref()
                .and_then(p3_expr_name)
                .and_then(|n| scope.types.get(&pystr::upper(n)))
                .map(String::as_str);
            if (kw == "MAXSPA" || kw == "MINSPA") && !matches!(dest_type, None | Some("FLOAT")) {
                diags.error(
                    inst.base.node.line,
                    "Dest type must be Float.",
                    &inst.base.node.file,
                    ShowFile::Bool(false),
                    0,
                    "",
                    "",
                );
            } else if kw != "MAXSPA" && kw != "MINSPA" {
                if let (Some(dt), Some(st)) = (dest_type, source_type) {
                    if dt != st {
                        diags.error(
                            inst.base.node.line,
                            "Source and Dest types must be the same.",
                            &inst.base.node.file,
                            ShowFile::Bool(false),
                            0,
                            "",
                            "",
                        );
                    }
                }
            }
        }
    }
}

/// Port of `checker/instr_measure.py::_index_value`.
pub fn _index_value(arg: Option<&Expr>) -> Option<i64> {
    _index_value_with(arg, None)
}

/// Port of `checker/instr_measure.py::_index_value`.
pub fn _index_value_with(arg: Option<&Expr>, env: Option<&Env>) -> Option<i64> {
    let expr = arg?;
    if let Expr::Number(n) = expr {
        if let NumberValue::Int(v) = n.value {
            return Some(v);
        }
    }
    let env = env?;
    match p3_const_value(Some(expr), env)? {
        ConstValue::Int(v) => Some(v),
        _ => None,
    }
}

/// Port of `checker/instr_measure.py::_flat_offset`.
#[allow(clippy::needless_pass_by_value)]
pub fn _flat_offset(dest: &Expr, shape: Vec<i64>) -> Option<i64> {
    _flat_offset_with(dest, &shape, None)
}

/// Port of `checker/instr_measure.py::_flat_offset`.
pub fn _flat_offset_with(dest: &Expr, shape: &[i64], env: Option<&Env>) -> Option<i64> {
    let Expr::Index(ix) = dest else {
        return Some(0);
    };
    if ix.args.is_empty() || ix.args.iter().all(Option::is_none) {
        return Some(0);
    }
    let mut index = Vec::with_capacity(ix.args.len());
    for a in &ix.args {
        index.push(_index_value_with(a.as_deref(), env)?);
    }
    if index.len() == 1 {
        return if index[0] >= 1 {
            Some(index[0] - 1)
        } else {
            None
        };
    }
    if index.len() != shape.len()
        || index
            .iter()
            .zip(shape.iter())
            .any(|(&k, &n)| k < 1 || k > n)
    {
        return None;
    }
    let mut offset: i64 = 0;
    for (&k, &n) in index.iter().zip(shape.iter()) {
        offset = offset * n + (k - 1);
    }
    Some(offset)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_text;
    use crate::profiles::get as get_profile;

    #[test]
    fn test_index_value_and_flat_offset() {
        let profile = get_profile("cr6").expect("cr6 profile");
        let env = Env::new(profile);
        let prog = parse_text(
            "Public x(4, 3)\nMove(x(2, 3), 1, 0, 1)",
            "test.crb",
            None,
            Some(profile),
        )
        .0;
        let dest = match prog.statements[1].as_ref() {
            Stmt::Instruction(inst) => inst.args[0].as_deref().expect("dest"),
            _ => panic!("expected instruction"),
        };
        assert_eq!(_flat_offset(dest, vec![4, 3]), Some(5));
        assert_eq!(_flat_offset_with(dest, &[4, 3], Some(&env)), Some(5));
    }

    #[test]
    fn test_builder_counts_with() {
        let profile = get_profile("cr6").expect("cr6 profile");
        let env = Env::new(profile);
        let prog = parse_text("CSAT3(dest, 2, 1)", "test.crb", None, Some(profile)).0;
        let args = match prog.statements[0].as_ref() {
            Stmt::Instruction(inst) => &inst.args,
            _ => panic!("expected instruction"),
        };
        assert_eq!(_builder_counts_with("CSAT3", args, &env), vec![(0, 10)]);
    }

    #[test]
    fn test_check_dest_bounds_with_spa() {
        let profile = get_profile("cr6").expect("cr6 profile");
        let env = Env::new(profile);
        let prog = parse_text(
            "Public dst(1), src(3)\nMaxSpa(dst(1), 3, src(1))",
            "test.crb",
            None,
            Some(profile),
        )
        .0;
        let mut diags = Diagnostics::default();
        check_dest_bounds_with(&prog, profile, &mut diags, &env);
        assert!(
            diags
                .items
                .iter()
                .any(|d| d.message == "Variable dst out of bounds."),
            "expected dst out of bounds, got {:?}",
            diags.items
        );
    }
}
