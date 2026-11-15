//! Port of `checker/__init__.py`.

#![allow(dead_code, unused_variables, clippy::used_underscore_items)]

use std::any::Any;
use std::cell::RefCell;
use std::collections::{BTreeSet, HashMap};
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};
use std::rc::Rc;

use crate::compiler::CompileResult;
use crate::consteval::Env;
use crate::diagnostics::{
    COULD_NOT_FIND_KINDS, DiagRef, Diagnostic, Diagnostics, INVALID_PORT_KINDS,
    PLACEMENT_MENU_KINDS, PhaseGuard, ShowFile, TABLE_NAME_KINDS,
};
use crate::profiles::Profile;
use crate::pystr;
use crate::syntax::{Program, ProgramRef, Stmt, StmtRef, cut_after_hash, hash_statement, stmt_id};

pub mod context;
pub mod cr3xx;
pub mod ess;
pub mod expressions;
pub mod instr_data;
pub mod instr_measure;
pub mod instr_rules;
pub mod intervals;
pub mod longconv;
pub mod scan_time;
pub mod storage;
pub mod structs;
pub mod structure;
pub mod symbols;
pub mod terminals;
pub mod var_types;
pub mod warnings;

thread_local! {
    static CURRENT_RESULT: RefCell<Option<*mut CompileResult>> = const { RefCell::new(None) };
}

struct ResultGuard {
    prev: Option<*mut CompileResult>,
}

impl ResultGuard {
    fn new(result: *mut CompileResult) -> Self {
        let prev = CURRENT_RESULT.with(|r| r.borrow_mut().replace(result));
        Self { prev }
    }
}

impl Drop for ResultGuard {
    fn drop(&mut self) {
        let prev = self.prev.take();
        CURRENT_RESULT.with(|r| *r.borrow_mut() = prev);
    }
}

const ASIDE_CUTOFF_PHASES: &[&str] = &["bounds", "pipe", "leading", "claims", "ranges", "fields"];
const TIER_C_PHASES: &[&str] = &["tier_c", "tier_c_apply", "tier_c_tail"];
const EARLY_PHASES: &[&str] = &["early", "early_scan"];
const PASS2_ORDER_PHASES: &[&str] = &[
    "tier_b",
    "tier_b_generic",
    "tier_b_calltable",
    "late",
    "late_scan",
    "held",
    "tier_c",
    "tier_c_apply",
    "tier_c_tail",
];
const POST_TIER_C_PHASES: &[&str] = &["structs_late", "end_blocks", "units"];

/// Port of `checker/__init__.py::_is_unreached`.
pub fn _is_unreached(d: &Diagnostic) -> bool {
    d.severity == "warning" && d.kind == "unreached"
}

/// Port of `checker/__init__.py::_pos_or_line`.
pub fn _pos_or_line(d: &Diagnostic) -> i64 {
    if d.pos >= 0 { d.pos } else { d.line }
}

/// Port of `checker/__init__.py::_before_cutoff`.
pub fn _before_cutoff(d: &Diagnostic, stop_at: Option<i64>) -> bool {
    match stop_at {
        None => true,
        Some(stop) => d.pos < 0 || d.pos <= stop,
    }
}

/// Port of `checker/__init__.py::_blocks_tier_c`.
pub fn _blocks_tier_c(d: &Diagnostic, single_pass: bool) -> bool {
    if d.phase == "tier_b_calltable" && d.kind == "could_not_find_table" {
        return false;
    }
    if !single_pass
        && (d.phase == "tier_b_generic"
            || d.phase == "held"
            || TABLE_NAME_KINDS.contains(&d.kind.as_str()))
    {
        return false;
    }
    true
}

/// Port of `checker/__init__.py::_is_tier_b_generic`.
pub fn _is_tier_b_generic(d: &Diagnostic) -> bool {
    d.phase == "tier_b_generic"
        || d.phase == "held"
        || (d.phase == "tier_b_calltable" && d.kind == "could_not_find_table")
}

fn panic_message(payload: &(dyn Any + Send)) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "panic".to_string()
    }
}

/// Port of `checker/__init__.py::_run_pass`.
pub fn _run_pass(name: &str, phase: &str, f: impl FnOnce()) {
    let _guard = (!phase.is_empty()).then(|| PhaseGuard::new(phase));
    if let Err(payload) = catch_unwind(AssertUnwindSafe(f)) {
        _record_exception(name, &*payload);
        if std::env::var("OPENCRBASIC_STRICT").as_deref() == Ok("1") {
            resume_unwind(payload);
        }
    }
}

/// Port of `checker/__init__.py::_run_aside`.
pub fn _run_aside(
    name: &str,
    phase: &str,
    diags: &mut Diagnostics,
    f: impl FnOnce(&mut Diagnostics),
) {
    let mut bucket = Diagnostics::new();
    let guard = PhaseGuard::new(phase);
    let res = catch_unwind(AssertUnwindSafe(|| f(&mut bucket)));
    drop(guard);
    diags.asides.extend(bucket.items);
    if let Err(payload) = res {
        _record_exception(name, &*payload);
        if std::env::var("OPENCRBASIC_STRICT").as_deref() == Ok("1") {
            resume_unwind(payload);
        }
    }
}

fn placement_first_kinds(kw: &str) -> Option<&'static [&'static str]> {
    match kw {
        "TIMERINPUT" => Some(&["placement_scan"]),
        "MENUITEM" | "MENURECOMPILE" => Some(&PLACEMENT_MENU_KINDS),
        _ => None,
    }
}

/// Port of `checker/__init__.py::_placement_rejected`.
pub fn _placement_rejected(
    program: &Program,
    profile: &Profile,
    diags: &Diagnostics,
) -> BTreeSet<usize> {
    if profile.single_pass {
        return BTreeSet::new();
    }
    let mut found = BTreeSet::new();
    for stmt in &program.statements {
        let Stmt::Instruction(instr) = &**stmt else {
            continue;
        };
        let Some(kinds) = placement_first_kinds(&pystr::upper(&instr.keyword)) else {
            continue;
        };
        if diags.items.iter().any(|d| {
            d.severity == "error"
                && d.line == stmt.line()
                && d.file == stmt.file()
                && kinds.contains(&d.kind.as_str())
        }) {
            found.insert(stmt_id(stmt));
        }
    }
    found
}

/// Port of `checker/__init__.py::_run_pass_capture`.
pub fn _run_pass_capture<R>(name: &str, phase: &str, f: impl FnOnce() -> R) -> Option<R> {
    let _guard = (!phase.is_empty()).then(|| PhaseGuard::new(phase));
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(val) => Some(val),
        Err(payload) => {
            _record_exception(name, &*payload);
            if std::env::var("OPENCRBASIC_STRICT").as_deref() == Ok("1") {
                resume_unwind(payload);
            }
            None
        }
    }
}

/// Port of `checker/__init__.py::_record_exception`.
pub fn _record_exception(where_: &str, payload: &(dyn Any + Send)) {
    let msg = panic_message(payload);
    CURRENT_RESULT.with(|r| {
        if let Some(ptr) = *r.borrow() {
            let _ = (ptr, where_, msg);
            // TODO(P3-s): call `(*ptr).record_implementation_failure(where_, &msg)` once `CompileResult` is ported.
        }
    });
}

/// Port of `checker/__init__.py::check`.
pub fn check(
    program: &ProgramRef,
    profile: &'static Profile,
    diags: &mut Diagnostics,
    result: &mut CompileResult,
) {
    let _guard = ResultGuard::new(std::ptr::from_mut(result));
    _check(program, profile, diags, result);
}

/// Port of `checker/__init__.py::_reorder_scan_placement_after_symbols`.
pub fn _reorder_scan_placement_after_symbols(items: &[DiagRef]) -> Vec<DiagRef> {
    let mut out: Vec<DiagRef> = items.to_vec();
    let targets: Vec<DiagRef> = out
        .iter()
        .filter(|d| d.phase.starts_with("tier_b") && d.kind == "placement_scan")
        .cloned()
        .collect();
    for d in targets {
        let last_sym = out
            .iter()
            .enumerate()
            .filter(|(_, o)| {
                o.phase.starts_with("tier_b")
                    && o.file == d.file
                    && o.line == d.line
                    && o.kind == "invalid_symbol"
            })
            .map(|(i, _)| i)
            .next_back();
        if let Some(after_idx) = last_sym
            && let Some(cur_idx) = out.iter().position(|o| Rc::ptr_eq(o, &d))
            && after_idx > cur_idx
        {
            out.remove(cur_idx);
            out.insert(after_idx, d);
        }
    }
    out
}

/// Port of `checker/__init__.py::_place_index_bounds`.
pub fn _place_index_bounds(
    bounds_items: &[DiagRef],
    resolved: &[DiagRef],
    program: &Program,
    single_pass: bool,
) -> (Vec<DiagRef>, Vec<DiagRef>) {
    let portget: BTreeSet<(String, i64)> = if single_pass {
        BTreeSet::new()
    } else {
        program
            .statements
            .iter()
            .filter_map(|st| match &**st {
                Stmt::Instruction(i) if pystr::upper(&i.keyword) == "PORTGET" => {
                    Some((st.file().to_string(), st.line()))
                }
                _ => None,
            })
            .collect()
    };
    let mut out = resolved.to_vec();
    let mut printed: Vec<(&Diagnostic, usize)> = Vec::new();
    for d in resolved {
        if let Some((_, c)) = printed.iter_mut().find(|(k, _)| *k == &**d) {
            *c += 1;
        } else {
            printed.push((&**d, 1));
        }
    }
    let mut seen_bounds: Vec<(&Diagnostic, usize)> = Vec::new();
    let mut kept = Vec::new();
    for d in bounds_items {
        let pcount = printed
            .iter()
            .find(|(k, _)| *k == &**d)
            .map_or(0, |(_, c)| *c);
        let seen = if let Some((_, c)) = seen_bounds.iter_mut().find(|(k, _)| *k == &**d) {
            c
        } else {
            seen_bounds.push((&**d, 0));
            &mut seen_bounds.last_mut().expect("just pushed").1
        };
        if *seen < pcount {
            *seen += 1;
            continue;
        }
        *seen += 1;
        let at = out.iter().position(|o| {
            o.file == d.file && o.line == d.line && INVALID_PORT_KINDS.contains(&o.kind.as_str())
        });
        if let Some(idx) = at
            && portget.contains(&(d.file.clone(), d.line))
        {
            out.insert(idx, d.clone());
        } else {
            kept.push(d.clone());
        }
    }
    (out, kept)
}

/// Port of `checker/__init__.py::_resolve`.
pub fn _resolve(
    diags: &mut Diagnostics,
    program: &Program,
    flat: &Program,
    profile: &'static Profile,
    stop_at: Option<i64>,
    table_stop_pos: Option<i64>,
) {
    let mut resolved = diags.items.clone();
    let mut asides = diags.asides.clone();

    for extra in asides.iter().filter(|d| d.phase == "tier_c_apply") {
        let at = resolved
            .iter()
            .position(|d| {
                (d.phase == "tier_c" && d.severity == "error" && d.line > extra.line)
                    || d.phase == "tier_c_tail"
                    || POST_TIER_C_PHASES.contains(&d.phase.as_str())
            })
            .unwrap_or(resolved.len());
        resolved.insert(at, extra.clone());
    }

    let oom_pair = resolved
        .iter()
        .enumerate()
        .find(|(_, d)| TIER_C_PHASES.contains(&d.phase.as_str()) && d.kind == "out_of_memory")
        .map(|(i, d)| (i, d.clone()));
    if let Some((oom_index, oom)) = oom_pair {
        let at = _pos_or_line(&oom);
        resolved = resolved
            .into_iter()
            .enumerate()
            .filter(|(i, d)| {
                if TIER_C_PHASES.contains(&d.phase.as_str()) {
                    d.severity != "error"
                        || d.line == 0
                        || _pos_or_line(d) < at
                        || (d.line == oom.line && *i <= oom_index)
                } else {
                    POST_TIER_C_PHASES.contains(&d.phase.as_str())
                        || d.severity != "warning"
                        || d.line == 0
                        || _pos_or_line(d) < at
                }
            })
            .map(|(_, d)| d)
            .collect();
        asides.retain(|d| (d.phase != "ranges" && d.phase != "bounds") || _pos_or_line(d) < at);
    }

    let has_generic = resolved.iter().any(|d| _is_tier_b_generic(d));
    if has_generic {
        resolved.retain(|d| {
            !TIER_C_PHASES.contains(&d.phase.as_str())
                || d.severity != "error"
                || d.line == 0
                || _before_cutoff(d, stop_at)
        });
    }
    if profile.single_pass {
        let end_pos = resolved
            .iter()
            .filter(|d| d.phase == "tier_b_calltable" && d.kind == "could_not_find_table")
            .map(|d| _pos_or_line(d))
            .min();
        if let Some(end_p) = end_pos {
            resolved.retain(|d| {
                !TIER_C_PHASES.contains(&d.phase.as_str())
                    || d.severity != "error"
                    || d.line == 0
                    || _pos_or_line(d) <= end_p
            });
        }
    }

    if has_generic
        && resolved
            .iter()
            .any(|d| TIER_C_PHASES.contains(&d.phase.as_str()))
    {
        let slots: Vec<usize> = resolved
            .iter()
            .enumerate()
            .filter(|(_, d)| {
                PASS2_ORDER_PHASES.contains(&d.phase.as_str())
                    && d.severity == "error"
                    && d.line != 0
            })
            .map(|(i, _)| i)
            .collect();
        let mut ordered: Vec<DiagRef> = slots.iter().map(|&i| resolved[i].clone()).collect();
        ordered.sort_by_key(|d| _pos_or_line(d));
        for (slot, d) in slots.into_iter().zip(ordered) {
            resolved[slot] = d;
        }
    }
    if let Some(tstop) = table_stop_pos {
        resolved.retain(|d| {
            !TIER_C_PHASES.contains(&d.phase.as_str())
                || d.severity != "error"
                || d.pos < 0
                || d.pos <= tstop
        });
    }

    let tier_c_boundary = resolved
        .iter()
        .position(|d| {
            TIER_C_PHASES.contains(&d.phase.as_str())
                || POST_TIER_C_PHASES.contains(&d.phase.as_str())
        })
        .unwrap_or(resolved.len());
    let argument_error_first: BTreeSet<(String, i64)> = _run_pass_capture(
        "structure::pipeline_argument_error_first_locations",
        "",
        || structure::pipeline_argument_error_first_locations(program, profile),
    )
    .unwrap_or_default();
    let mut ordinary_pipe = Vec::new();
    let mut delayed_pipe = Vec::new();
    for d in &asides {
        if d.phase != "pipe" {
            continue;
        }
        let loc = (d.file.clone(), d.line);
        if argument_error_first.contains(&loc)
            && resolved.iter().any(|item| {
                TIER_C_PHASES.contains(&item.phase.as_str())
                    && item.severity == "error"
                    && item.file == loc.0
                    && item.line == loc.1
            })
        {
            delayed_pipe.push(d.clone());
        } else {
            ordinary_pipe.push(d.clone());
        }
    }
    resolved.splice(tier_c_boundary..tier_c_boundary, ordinary_pipe);
    for d in delayed_pipe {
        if let Some(first_err) = resolved
            .iter()
            .position(|item| item.severity == "error" && item.file == d.file && item.line == d.line)
        {
            resolved.insert(first_err + 1, d);
        }
    }

    let mut early_items: Vec<DiagRef> = asides
        .iter()
        .filter(|d| d.phase == "early_scan")
        .cloned()
        .chain(asides.iter().filter(|d| d.phase == "early").cloned())
        .collect();
    if !early_items.is_empty() {
        early_items.retain(|d| _before_cutoff(d, stop_at));
        if let Some(tstop) = table_stop_pos {
            early_items.retain(|d| (if d.pos < 0 { 0 } else { d.pos }) <= tstop);
        }
    }

    let mut bounds_items: Vec<DiagRef> = asides
        .iter()
        .filter(|d| d.phase == "bounds")
        .cloned()
        .collect();
    if !bounds_items.is_empty() {
        let (next_resolved, next_bounds) =
            _place_index_bounds(&bounds_items, &resolved, flat, profile.single_pass);
        resolved = next_resolved;
        bounds_items = next_bounds;
    }
    let fields_items: Vec<DiagRef> = asides
        .iter()
        .filter(|d| d.phase == "fields")
        .cloned()
        .collect();
    let ranges_items: Vec<DiagRef> = asides
        .iter()
        .filter(|d| d.phase == "ranges")
        .cloned()
        .collect();
    resolved.extend(bounds_items);
    resolved.extend(fields_items);
    resolved.extend(ranges_items);
    resolved.extend(early_items);

    let soft_items: Vec<DiagRef> = asides
        .iter()
        .filter(|d| d.phase == "soft")
        .cloned()
        .collect();
    for d in soft_items.into_iter().rev() {
        let here = (_pos_or_line(&d), d.line);
        let at = resolved
            .iter()
            .position(|o| o.severity == "error" && (_pos_or_line(o), o.line) >= here)
            .unwrap_or(resolved.len());
        resolved.insert(at, d);
    }

    let mut leading_items: Vec<DiagRef> = asides
        .iter()
        .filter(|d| d.phase == "leading")
        .cloned()
        .collect();
    let claims_items: Vec<DiagRef> = asides
        .iter()
        .filter(|d| d.phase == "claims")
        .cloned()
        .collect();
    let others: Vec<i64> = resolved
        .iter()
        .chain(leading_items.iter())
        .chain(claims_items.iter())
        .filter(|d| d.severity == "error" && d.line != 0)
        .map(|d| _pos_or_line(d))
        .collect();
    for d in claims_items {
        if d.kind != "sdi12_conflict" || !others.iter().any(|&p| p < _pos_or_line(&d)) {
            leading_items.push(d);
        }
    }
    let mut last_leading: HashMap<(String, i64), DiagRef> = HashMap::new();
    for d in leading_items {
        let key = (d.file.clone(), d.line);
        let at = if let Some(prev) = last_leading.get(&key) {
            resolved
                .iter()
                .position(|o| Rc::ptr_eq(o, prev))
                .map_or(resolved.len(), |i| i + 1)
        } else {
            resolved
                .iter()
                .position(|o| o.file == key.0 && o.line == key.1)
                .unwrap_or(resolved.len())
        };
        resolved.insert(at, d.clone());
        last_leading.insert(key, d);
    }

    diags.items = resolved;
    diags.asides.clear();
}

/// Port of `checker/__init__.py::_check`.
pub fn _check(
    program: &ProgramRef,
    profile: &'static Profile,
    diags: &mut Diagnostics,
    result: &mut CompileResult,
) {
    let stop = program
        .statements
        .iter()
        .position(|st| hash_statement(st).is_some());
    let mut prog = program.clone();
    if let Some(stop_idx) = stop {
        let mut stmts: Vec<StmtRef> = prog.statements[..stop_idx].to_vec();
        stmts.push(cut_after_hash(&prog.statements[stop_idx]));
        let mut replaced = prog.replaced();
        replaced.statements = stmts;
        prog = Rc::new(replaced);
        diags.trim(prog.hash_cut);
    }
    if let Some(rotated) = _run_pass_capture("structure::open_program_at_slowsequence", "", || {
        structure::open_program_at_slowsequence(&prog, profile)
    }) {
        prog = rotated;
    }
    let env = Env::with_consts(profile, Rc::new(RefCell::new(prog.consts.clone())));
    let ctx = Rc::new(context::Context::new(
        prog.clone(),
        Some(profile),
        Some(env.clone()),
    ));
    let _ctx_guard = context::activate(ctx.clone());
    diags.items = diags
        .items
        .iter()
        .map(|d| Rc::new(d.replace(|_| {})))
        .collect();
    _check_with_context(&prog, profile, diags, result, &env, &ctx);
}

/// Port of `checker/__init__.py::_check_with_context`.
pub fn _check_with_context(
    program: &ProgramRef,
    profile: &'static Profile,
    diags: &mut Diagnostics,
    result: &mut CompileResult,
    env: &Env,
    ctx: &Rc<context::Context>,
) {
    _run_pass("expressions::set_arity_context", "", || {
        expressions::set_arity_context(
            program,
            Some(profile),
            Some(ctx.declared_variable_names().clone()),
        );
    });
    _run_pass("structure::check_pass1", "pass1", || {
        structure::check_pass1(program, profile, diags, Some(env));
    });
    _run_pass("expressions::check_consts", "pass1", || {
        expressions::check_consts(program, profile, diags);
    });
    _run_pass("instr_data::check_constant_destinations", "pass1", || {
        instr_data::check_constant_destinations(program, profile, diags, env);
    });
    _run_pass("expressions::check_const_family_args", "pass1", || {
        expressions::check_const_family_args(ctx.flat(), profile, diags);
    });
    _run_pass("instr_measure::check_pass1_values", "pass1", || {
        instr_measure::check_pass1_values(program, profile, diags);
    });
    _run_pass("var_types::check_string_operands", "pass1", || {
        var_types::check_string_operands(program, diags);
    });
    _run_pass("intervals::check", "pass1", || {
        intervals::check(program, diags);
    });
    _run_pass("instr_rules::check_const_int_args", "pass1", || {
        instr_rules::check_const_int_args(program, profile, diags, env);
    });
    _run_pass("expressions::check_function_call_counts", "pass1", || {
        expressions::check_function_call_counts(program, profile, diags);
    });
    _run_pass("expressions::check_recursive_functions", "pass1", || {
        expressions::check_recursive_functions(program, profile, diags);
    });
    _run_pass("expressions::check_data_params", "pass1", || {
        expressions::check_data_params(program, profile, diags);
    });
    _run_pass("cr3xx::check", "pass1", || {
        cr3xx::check(program, diags);
    });
    _run_pass("cr3xx::check_dropped_tail", "pass1", || {
        cr3xx::check_dropped_tail(program, diags);
    });
    _run_pass("instr_rules::check", "pass1", || {
        instr_rules::check(ctx.flat(), profile, diags);
    });
    _run_pass("instr_rules::check_return_placement", "pass1", || {
        instr_rules::check_return_placement(program, profile, diags);
    });
    _run_pass("structure::check_missing_param_followup", "pass1", || {
        structure::check_missing_param_followup(program, profile, diags);
    });
    _run_pass("instr_rules::check_too_many", "pass1", || {
        instr_rules::check_too_many(ctx.flat(), profile, diags);
    });
    _run_pass("instr_rules::check_essinitialize_too_many", "pass1", || {
        instr_rules::check_essinitialize_too_many(program, profile, diags);
    });
    _run_pass("instr_rules::check_snmpvariable_too_many", "pass1", || {
        instr_rules::check_snmpvariable_too_many(program, profile, diags);
    });
    _run_pass("instr_rules::check_snmp_oid", "pass1", || {
        instr_rules::check_snmp_oid(program, profile, diags, Some(env));
    });
    _run_pass("instr_rules::run_rules(pass1)", "pass1", || {
        instr_rules::run_rules("pass1", ctx.flat(), profile, diags, env);
    });
    _run_pass("instr_rules::check_emailrecv_destination", "pass1", || {
        instr_rules::check_emailrecv_destination(program, profile, diags);
    });
    _run_pass("instr_rules::check_tcpopen_connections", "pass1", || {
        instr_rules::check_tcpopen_connections(program, profile, diags, env);
    });
    _run_pass("instr_rules::check_tcpopen_handle_type", "pass1", || {
        instr_rules::check_tcpopen_handle_type(program, profile, diags);
    });
    _run_pass("instr_rules::check_iif_types", "pass1", || {
        instr_rules::check_iif_types(ctx.flat(), profile, diags);
    });
    _run_pass("var_types::check_ctype", "pass1", || {
        var_types::check_ctype(program, diags);
    });
    _run_pass("var_types::check_pointer_deref", "pass1", || {
        var_types::check_pointer_deref(program, diags);
    });
    _run_pass("expressions::check_function_arity", "pass1", || {
        expressions::check_function_arity(ctx.flat(), profile, diags, None);
    });

    if !profile.single_pass && !diags.has_errors() {
        _run_pass("structs::check_members", "pass1", || {
            structs::check_members(program, diags, ctx.flat());
        });
    }

    if !profile.early_warnings_gated {
        let errors_before_early = diags.errors().len();
        _run_pass("warnings::check_early", "early", || {
            warnings::check_early(program, profile, diags);
        });
        if !profile.single_pass && errors_before_early == 0 {
            _run_pass("instr_measure::check_scan_delay_budget", "early", || {
                instr_measure::check_scan_delay_budget(program, profile, diags, true);
            });
        }
        let early_errors: Vec<DiagRef> = diags
            .items
            .iter()
            .filter(|d| d.phase == "early" && d.severity == "error")
            .cloned()
            .collect();
        diags.asides.extend(early_errors);
        diags
            .items
            .retain(|d| !(d.phase == "early" && d.severity == "error"));
    }

    let held_items: Vec<DiagRef> = diags
        .items
        .iter()
        .filter(|d| d.phase == "held")
        .cloned()
        .collect();
    if !held_items.is_empty() {
        diags.items.retain(|d| d.phase != "held");
        if !diags.has_errors() {
            diags.asides.extend(held_items);
        }
    }

    _run_pass("storage::check_pass1", "pass1", || {
        storage::check_pass1(program);
    });
    _run_pass("instr_data::drop_unreached_destinations", "pass1", || {
        instr_data::drop_unreached_destinations(program, profile, diags, env);
    });
    let pass1_errors = diags.has_errors();
    let _ = result;
    // TODO(P3-s): `result.pass1_errors = Some(pass1_errors);` once `CompileResult` is ported.
    if pass1_errors {
        if !profile.single_pass {
            diags.items.retain(|d| !_is_unreached(d));
        }
        diags.asides.retain(|d| d.phase != "early");
    }

    if !diags.has_errors() {
        _run_pass("instr_data::check_stage2a", "stage2a", || {
            instr_data::check_stage2a(program, profile, diags, env);
        });
        let soft: Vec<DiagRef> = diags
            .items
            .iter()
            .filter(|d| d.phase == "soft")
            .cloned()
            .collect();
        diags.asides.extend(soft);
        diags.items.retain(|d| d.phase != "soft");
        if diags.has_errors() {
            diags.items.retain(|d| !_is_unreached(d));
            let order: HashMap<(String, i64), usize> = program
                .statements
                .iter()
                .enumerate()
                .map(|(i, st)| ((st.file().to_string(), st.line()), i))
                .collect();
            let first = diags
                .errors()
                .iter()
                .map(|d| order.get(&(d.file.clone(), d.line)).copied().unwrap_or(0))
                .min()
                .unwrap_or(0);
            diags.asides.retain(|d| {
                d.phase != "early"
                    || (d.kind == "datainterval_non_negative"
                        && order.get(&(d.file.clone(), d.line)).copied().unwrap_or(0) < first)
            });
        }
    }

    let mut stop_at: Option<i64> = None;
    let mut flat = program.clone();
    let mut links = structure::CallLinks::default();
    for stmt in &ctx.flat().statements {
        let key = (stmt.file().to_owned(), stmt.line());
        if let Some(pos) = ctx.positions().get(&key) {
            links.positions.insert(key, *pos);
        }
    }
    if !diags.has_errors() || profile.single_pass {
        _run_aside(
            "instr_rules::check_spi_open_order",
            "leading",
            diags,
            |aside| {
                instr_rules::check_spi_open_order(ctx.flat(), profile, aside);
            },
        );
        _run_aside(
            "terminals::check_i2c_open_order",
            "ranges",
            diags,
            |aside| {
                terminals::check_i2c_open_order(ctx.flat(), profile, aside);
            },
        );
        if profile.single_pass {
            _run_aside(
                "structure::check_block_closers",
                "leading",
                diags,
                |aside| {
                    structure::check_block_closers(program, profile, aside);
                },
            );
        } else {
            _run_pass("structure::check_block_closers", "tier_b", || {
                structure::check_block_closers(program, profile, diags);
            });
        }

        flat = ctx.flat().clone();
        let stop_line = _run_pass_capture("structure::check_pass2", "tier_b_generic", || {
            structure::check_pass2(&flat, profile, diags, Some(env.clone()), Some(&mut links))
        });
        if !profile.single_pass {
            let early_scan: Vec<DiagRef> = diags
                .items
                .iter()
                .filter(|d| d.phase == "early_scan")
                .cloned()
                .collect();
            diags.asides.extend(early_scan);
            diags.items.retain(|d| d.phase != "early_scan");
        }

        let mut stops: Vec<(String, i64)> = if stop_line.flatten().is_some() {
            links.cutoff.clone().into_iter().collect()
        } else {
            Vec::new()
        };
        let mut unreached_off: BTreeSet<(String, i64)> = BTreeSet::new();
        if let Some(Some(stray_stop)) =
            _run_pass_capture("structure::check_stray_controls", "tier_b", || {
                structure::check_stray_controls(program, profile, diags)
            })
        {
            stops.push(stray_stop);
        }
        _run_aside(
            "structure::check_pipeline_task_placement",
            "pipe",
            diags,
            |aside| {
                structure::check_pipeline_task_placement(program, profile, aside);
            },
        );
        _run_aside(
            "structure::check_slow_sequence_blocks",
            "pipe",
            diags,
            |aside| {
                structure::check_slow_sequence_blocks(program, profile, aside);
            },
        );
        if let Some(Some(menu_stop)) = _run_pass_capture(
            "instr_data::check_table_and_menu_placement",
            "tier_b",
            || instr_data::check_table_and_menu_placement(program, profile, diags, Some(env), None),
        ) {
            stops.push(menu_stop);
        }

        if !profile.single_pass {
            for (stmt, _name) in instr_data::unknown_calltables(program) {
                let key = (stmt.file().to_owned(), stmt.line());
                stops.push(key.clone());
                unreached_off.insert(key);
            }
            for (stmt, _message, ends) in
                instr_data::unknown_table_references(program, Some(profile))
            {
                if ends {
                    stops.push((stmt.file().to_owned(), stmt.line()));
                }
            }
            if let Some(Some(early_table_stop)) =
                _run_pass_capture("instr_data::check_table_structure", "", || {
                    let mut tmp = Diagnostics::new();
                    instr_data::check_table_structure(ctx.flat(), profile, &mut tmp)
                })
            {
                stops.push(early_table_stop);
            }
        }
        stop_at = stops
            .iter()
            .filter_map(|k| ctx.positions().get(k).copied())
            .min();

        diags.items.retain(|d| {
            !(_is_unreached(d)
                && (!_before_cutoff(d, stop_at)
                    || unreached_off.contains(&(d.file.clone(), d.line))))
        });

        let placement_rej = _placement_rejected(&flat, profile, diags);
        _run_pass("expressions::check", "tier_b_generic", || {
            expressions::check(&flat, profile, diags, env, &placement_rej);
        });
        _run_pass(
            "expressions::check_timedcontrol_constants",
            "tier_b_generic",
            || {
                expressions::check_timedcontrol_constants(&flat, profile, diags);
            },
        );
        _run_pass("expressions::check_table_members", "tier_b_generic", || {
            expressions::check_table_members(&flat, profile, diags);
        });
        _run_pass("expressions::check_digit_members", "tier_b_generic", || {
            expressions::check_digit_members(&flat, profile, diags);
        });
        _run_pass(
            "expressions::check_undefined_dotted_bases",
            "tier_b_generic",
            || {
                expressions::check_undefined_dotted_bases(&flat, profile, diags);
            },
        );
        _run_pass(
            "instr_rules::run_rules(tier_b_generic)",
            "tier_b_generic",
            || {
                instr_rules::run_rules("tier_b_generic", &flat, profile, diags, env);
            },
        );
        _run_aside(
            "instr_rules::run_rules(leading)",
            "leading",
            diags,
            |aside| {
                instr_rules::run_rules("leading", &flat, profile, aside, env);
            },
        );
        if !profile.single_pass {
            diags.items = diags
                .items
                .iter()
                .map(|d| {
                    if d.phase.starts_with("tier_b") && d.kind == "invalid_symbol" {
                        Rc::new(d.replace(|r| r.show_file = ShowFile::Bool(false)))
                    } else {
                        d.clone()
                    }
                })
                .collect();
            _run_pass(
                "instr_data::move_fieldnames_symbol_errors",
                "tier_b_generic",
                || {
                    instr_data::move_fieldnames_symbol_errors(&flat, diags);
                },
            );
            diags.items = _reorder_scan_placement_after_symbols(&diags.items);
        }

        _run_pass("instr_rules::check_mqtt_publish_payload", "tier_b", || {
            instr_rules::check_mqtt_publish_payload(&flat, profile, diags);
        });
        if !profile.single_pass {
            _run_pass("instr_data::check_eventcount_tables", "tier_b", || {
                instr_data::check_eventcount_tables(&flat, profile, diags);
            });
        }
        _run_pass("ess::check", "tier_b", || {
            ess::check(&flat, diags);
        });

        if !profile.single_pass {
            _run_aside(
                "expressions::check_instruction_index_bounds",
                "bounds",
                diags,
                |aside| {
                    expressions::check_instruction_index_bounds(&flat, profile, aside, env);
                },
            );
            _run_aside("instr_rules::run_rules(bounds)", "bounds", diags, |aside| {
                instr_rules::run_rules("bounds", &flat, profile, aside, env);
            });
            _run_aside(
                "instr_data::check_output_datatype",
                "bounds",
                diags,
                |aside| {
                    instr_data::check_output_datatype(&flat, profile, aside, env);
                },
            );
            _run_aside(
                "instr_data::check_odd_pair_reps",
                "leading",
                diags,
                |aside| {
                    instr_data::check_odd_pair_reps(&flat, profile, aside, env);
                },
            );
            _run_aside(
                "instr_rules::check_option_values",
                "bounds",
                diags,
                |aside| {
                    instr_rules::check_option_values(&flat, profile, aside, env);
                },
            );
            _run_aside(
                "instr_rules::check_serialinrecord",
                "bounds",
                diags,
                |aside| {
                    instr_rules::check_serialinrecord(&flat, profile, aside, env);
                },
            );
            _run_aside(
                "expressions::check_splitstr_whole_array",
                "bounds",
                diags,
                |aside| {
                    expressions::check_splitstr_whole_array(&flat, profile, aside, env);
                },
            );
            _run_aside(
                "expressions::check_splitstr_element_reps",
                "bounds",
                diags,
                |aside| {
                    expressions::check_splitstr_element_reps(&flat, profile, aside, env);
                },
            );
            _run_aside(
                "expressions::check_sdmsio4_count",
                "bounds",
                diags,
                |aside| {
                    expressions::check_sdmsio4_count(&flat, profile, aside, env);
                },
            );
            _run_aside(
                "expressions::check_calibrate_size",
                "bounds",
                diags,
                |aside| {
                    expressions::check_calibrate_size(&flat, profile, aside, env);
                },
            );
            _run_aside(
                "expressions::check_subcall_element_bounds",
                "bounds",
                diags,
                |aside| {
                    expressions::check_subcall_element_bounds(&flat, profile, aside, env);
                },
            );
            _run_aside(
                "instr_rules::check_fieldcal_variable_args",
                "bounds",
                diags,
                |aside| {
                    instr_rules::check_fieldcal_variable_args(&flat, profile, aside, Some(env));
                },
            );
            _run_aside(
                "instr_rules::check_pulsecount_channels",
                "bounds",
                diags,
                |aside| {
                    instr_rules::check_pulsecount_channels(&flat, profile, aside);
                },
            );
            _run_aside(
                "instr_rules::check_terminal_conflicts",
                "claims",
                diags,
                |aside| {
                    instr_rules::check_terminal_conflicts(&flat, profile, aside, env);
                },
            );
            _run_aside("instr_rules::check_dnp_update", "leading", diags, |aside| {
                instr_rules::check_dnp_update(&flat, profile, aside, Some(env));
            });
            _run_aside(
                "instr_rules::check_cdm_address_types",
                "bounds",
                diags,
                |aside| {
                    instr_rules::check_cdm_address_types(&flat, profile, aside, env);
                },
            );
            _run_aside(
                "instr_rules::check_avw200_scan_time",
                "bounds",
                diags,
                |aside| {
                    instr_rules::check_avw200_scan_time(&flat, profile, aside, env);
                },
            );
            _run_aside(
                "instr_rules::check_epi_scan_buffers",
                "bounds",
                diags,
                |aside| {
                    instr_rules::check_epi_scan_buffers(&flat, profile, aside, env);
                },
            );
        }

        _run_pass("instr_rules::run_rules(tier_b)", "tier_b", || {
            instr_rules::run_rules("tier_b", program, profile, diags, env);
        });
        if !profile.single_pass {
            _run_aside(
                "instr_measure::check_const_ranges",
                "ranges",
                diags,
                |aside| {
                    instr_measure::check_const_ranges(profile, aside);
                },
            );
        }
        _run_pass(
            "instr_data::check_unclosed_calltable",
            "tier_b_calltable",
            || {
                instr_data::check_unclosed_calltable(program, diags, Some(profile));
            },
        );
        if !profile.single_pass {
            _run_aside(
                "instr_rules::check_settings_fields",
                "fields",
                diags,
                |aside| {
                    let mut tmp = Diagnostics::new();
                    instr_rules::check_settings_fields(
                        &flat,
                        profile,
                        &mut tmp,
                        Some(aside),
                        Some(env),
                    );
                },
            );
        }

        _run_pass("cr3xx::check_unknown_line_words", "tier_b", || {
            cr3xx::check_unknown_line_words(&flat, diags);
        });
        _run_pass("cr3xx::check_for_header", "tier_b", || {
            cr3xx::check_for_header(&flat, diags);
        });
        _run_pass("cr3xx::check_float_destinations", "tier_b", || {
            cr3xx::check_float_destinations(&flat, diags);
        });
        _run_pass(
            "cr3xx::drop_too_many_after_variable_illegal",
            "tier_b",
            || {
                cr3xx::drop_too_many_after_variable_illegal(&flat, diags);
            },
        );
        _run_pass(
            "cr3xx::drop_calltable_error_with_other_errors",
            "tier_b",
            || {
                cr3xx::drop_calltable_error_with_other_errors(program, diags);
            },
        );

        diags.items.retain(|d| {
            !d.phase.starts_with("tier_b")
                || d.severity != "error"
                || d.line == 0
                || _before_cutoff(d, stop_at)
        });
        diags.asides.retain(|d| {
            !ASIDE_CUTOFF_PHASES.contains(&d.phase.as_str()) || _before_cutoff(d, stop_at)
        });
        if !profile.single_pass
            && diags.items.iter().any(|d| {
                d.phase.starts_with("tier_b")
                    && d.severity == "error"
                    && d.kind == "dimensions_not_declared"
            })
        {
            diags.items = diags
                .items
                .iter()
                .filter(|d| !(d.phase.starts_with("tier_b") && d.kind == "invalid_symbol"))
                .map(|d| {
                    if d.phase.starts_with("tier_b") && d.kind == "dimensions_not_declared" {
                        Rc::new(d.replace(|r| r.phase = "tier_b".to_string()))
                    } else {
                        d.clone()
                    }
                })
                .collect();
            diags.asides.retain(|d| {
                !ASIDE_CUTOFF_PHASES.contains(&d.phase.as_str())
                    && !EARLY_PHASES.contains(&d.phase.as_str())
            });
        }
    }

    if profile.single_pass {
        _run_pass("structure::commit_deferred_sub_error", "tier_b", || {
            structure::commit_deferred_sub_error(&links, profile, diags);
        });
    }
    if profile.single_pass && !diags.has_errors() {
        _run_pass("structure::check_data_table_placement", "tier_b", || {
            structure::check_data_table_placement(program, profile, diags);
        });
    }

    let mut late_stop: Option<(String, i64)> = None;
    if profile.single_pass
        && diags
            .errors()
            .iter()
            .all(|d| COULD_NOT_FIND_KINDS.contains(&d.kind.as_str()))
    {
        _run_aside(
            "instr_rules::check_settings_fields",
            "fields",
            diags,
            |aside| {
                let mut tmp = Diagnostics::new();
                instr_rules::check_settings_fields(
                    ctx.flat(),
                    profile,
                    &mut tmp,
                    Some(aside),
                    Some(env),
                );
            },
        );
        _run_aside(
            "instr_rules::check_option_values",
            "bounds",
            diags,
            |aside| {
                instr_rules::check_option_values(ctx.flat(), profile, aside, env);
            },
        );
    }
    if !diags.has_errors() {
        if profile.single_pass {
            _run_aside(
                "expressions::check_instruction_index_bounds",
                "bounds",
                diags,
                |aside| {
                    expressions::check_instruction_index_bounds(program, profile, aside, env);
                },
            );
            _run_aside(
                "instr_data::check_output_datatype",
                "bounds",
                diags,
                |aside| {
                    instr_data::check_output_datatype(program, profile, aside, env);
                },
            );
            _run_aside(
                "expressions::check_status_member_reads_late",
                "bounds",
                diags,
                |aside| {
                    expressions::check_status_member_reads_late(program, profile, aside);
                },
            );
            _run_aside(
                "expressions::check_splitstr_whole_array",
                "bounds",
                diags,
                |aside| {
                    expressions::check_splitstr_whole_array(program, profile, aside, env);
                },
            );
            _run_aside(
                "expressions::check_splitstr_element_reps",
                "bounds",
                diags,
                |aside| {
                    expressions::check_splitstr_element_reps(program, profile, aside, env);
                },
            );
            _run_aside(
                "expressions::check_calibrate_size",
                "bounds",
                diags,
                |aside| {
                    expressions::check_calibrate_size(program, profile, aside, env);
                },
            );
            _run_aside(
                "expressions::check_subcall_element_bounds",
                "bounds",
                diags,
                |aside| {
                    expressions::check_subcall_element_bounds(program, profile, aside, env);
                },
            );
            _run_aside(
                "instr_rules::check_fieldcal_variable_args",
                "bounds",
                diags,
                |aside| {
                    instr_rules::check_fieldcal_variable_args(program, profile, aside, Some(env));
                },
            );
            _run_aside("instr_rules::run_rules(bounds)", "bounds", diags, |aside| {
                instr_rules::run_rules("bounds", program, profile, aside, env);
            });
            _run_aside(
                "instr_data::check_eventcount_tables",
                "bounds",
                diags,
                |aside| {
                    instr_data::check_eventcount_tables(program, profile, aside);
                },
            );
        }
        _run_pass("instr_data::check_table_reps_late", "late", || {
            instr_data::check_table_reps_late(program, profile, diags, env);
        });
        if profile.single_pass {
            _run_pass("instr_rules::check_dnp_update", "late", || {
                instr_rules::check_dnp_update(ctx.flat(), profile, diags, Some(env));
            });
            _run_pass("instr_rules::run_rules(late)", "late", || {
                instr_rules::run_rules("late", ctx.flat(), profile, diags, env);
            });
        }
    }

    if (!diags.has_errors()
        || (profile.single_pass
            && diags
                .errors()
                .iter()
                .all(|d| COULD_NOT_FIND_KINDS.contains(&d.kind.as_str()))))
        && profile.early_warnings_gated
    {
        _run_pass("warnings::check_early", "late", || {
            warnings::check_early(program, profile, diags);
        });
        let stop = _run_pass_capture("structure::check_scan_placement_late", "late_scan", || {
            structure::check_scan_placement_late(program, profile, diags)
        })
        .flatten();
        let mut ref_lines: Vec<i64> = instr_data::unknown_table_references(program, Some(profile))
            .into_iter()
            .filter_map(|(st, _, ends)| ends.then_some(st.line()))
            .collect();
        if profile.single_pass {
            ref_lines.extend(
                instr_data::unknown_calltables(program)
                    .into_iter()
                    .map(|(st, _)| st.line()),
            );
        }
        if !ref_lines.is_empty() {
            let first_ref = *ref_lines.iter().min().unwrap_or(&0);
            diags
                .items
                .retain(|d| d.phase != "late_scan" || d.severity != "error" || d.line <= first_ref);
            if profile.single_pass {
                diags
                    .items
                    .retain(|d| !(_is_unreached(d) && d.line >= first_ref));
            }
        }
        if profile.single_pass
            && let Some(ref st) = stop
            && !ref_lines.is_empty()
            && st.1 < *ref_lines.iter().min().unwrap_or(&0)
        {
            diags.items.retain(|d| {
                !(d.severity == "error"
                    && (d.kind == "could_not_find_table" || d.kind == "could_not_find")
                    && d.file == st.0
                    && d.line > st.1)
            });
        }
        late_stop.clone_from(&stop);
        if let Some(ref st) = stop {
            diags
                .items
                .retain(|d| d.severity != "warning" || d.file != st.0 || d.line <= st.1);
        }
    }

    let mut held_active: Vec<DiagRef> = diags
        .asides
        .iter()
        .filter(|d| d.phase == "held")
        .cloned()
        .collect();
    diags.asides.retain(|d| d.phase != "held");
    if profile.single_pass {
        held_active.retain(|d| late_stop.as_ref().is_none_or(|st| d.line <= st.1));
        if diags.errors().iter().any(|d| d.kind != "before_beginprog") {
            held_active.clear();
        }
    } else {
        held_active.retain(|d| _before_cutoff(d, stop_at));
    }
    if !held_active.is_empty() {
        let own: BTreeSet<(String, i64)> = held_active
            .iter()
            .map(|d| (d.file.clone(), d.line))
            .collect();
        let own_stmts: BTreeSet<usize> = held_active
            .iter()
            .filter(|d| d.stmt != 0)
            .map(|d| d.stmt)
            .collect();
        diags.items.retain(|d| {
            let matches_loc = if d.stmt != 0 {
                own_stmts.contains(&d.stmt)
            } else {
                own.contains(&(d.file.clone(), d.line))
            };
            !(matches_loc
                && d.severity == "error"
                && (d.kind == "invalid_symbol" || d.kind == "not_defined"))
        });
        diags.items.extend(held_active);
    }

    let mut table_stop: Option<(String, i64)> = None;
    if !diags
        .errors()
        .iter()
        .any(|d| _blocks_tier_c(d, profile.single_pass))
    {
        _run_pass("instr_measure::check_reps", "tier_c", || {
            instr_measure::check_reps(program, profile, diags);
        });
        _run_pass("instr_measure::check_dest_bounds", "tier_c", || {
            instr_measure::check_dest_bounds(program, profile, diags);
        });
        _run_pass("var_types::check", "tier_c", || {
            var_types::check(program, diags);
        });
        if profile.single_pass {
            _run_pass("instr_measure::check_const_ranges", "tier_c", || {
                instr_measure::check_const_ranges(profile, diags);
            });
        }
        _run_pass("terminals::check", "tier_c", || {
            terminals::check(program, profile, diags);
        });
        table_stop = _run_pass_capture("instr_data::check_table_structure", "tier_c", || {
            instr_data::check_table_structure(ctx.flat(), profile, diags)
        })
        .flatten();
        _run_pass("instr_data::check_fieldnames", "tier_c", || {
            instr_data::check_fieldnames(program, profile, diags, env);
        });
        _run_pass("instr_data::check_fieldclassify", "tier_c", || {
            instr_data::check_fieldclassify(program, profile, diags, env);
        });
        _run_aside(
            "structure::check_apply_restart_sequence",
            "tier_c_apply",
            diags,
            |aside| {
                structure::check_apply_restart_sequence(program, profile, aside);
            },
        );
        _run_pass("instr_rules::check_samplefieldcal", "tier_c_tail", || {
            instr_rules::check_samplefieldcal(ctx.flat(), profile, diags);
        });
        _run_pass("instr_measure::check", "tier_c_tail", || {
            instr_measure::check(program, profile, diags);
        });
        _run_pass(
            "instr_measure::check_scan_delay_budget",
            "tier_c_tail",
            || {
                instr_measure::check_scan_delay_budget(program, profile, diags, false);
            },
        );
    }

    let aside_phases: BTreeSet<String> = diags.asides.iter().map(|d| d.phase.clone()).collect();
    let mut has_errors = diags.has_errors()
        || diags
            .asides
            .iter()
            .any(|d| d.phase == "tier_c_apply" && d.severity == "error");
    if profile.single_pass && !has_errors {
        let flat_program = ctx.flat();
        _run_pass("structs::check_members", "structs_late", || {
            structs::check_members(program, diags, flat_program);
        });
        _run_pass("structs::check_struct_assignment", "structs_late", || {
            structs::check_struct_assignment(program, diags, flat_program);
        });
        has_errors = diags.has_errors() || has_errors;
    }

    if !has_errors
        && !["fields", "pipe", "early", "early_scan"]
            .iter()
            .any(|p| aside_phases.contains(*p))
    {
        _run_pass("structure::check_end_blocks", "end_blocks", || {
            structure::check_end_blocks(program, diags);
        });
        has_errors = diags.has_errors() || has_errors;
    }
    if !has_errors
        && ![
            "early",
            "early_scan",
            "pipe",
            "fields",
            "bounds",
            "ranges",
            "leading",
        ]
        .iter()
        .any(|p| aside_phases.contains(*p))
    {
        _run_pass("instr_data::check_units_targets", "units", || {
            instr_data::check_units_targets(program, profile, diags);
        });
    }

    let table_stop_pos = table_stop
        .as_ref()
        .and_then(|ts| ctx.positions().get(ts).copied());
    _resolve(diags, program, &flat, profile, stop_at, table_stop_pos);

    _run_pass("structure::finalize_call_links", "link", || {
        structure::finalize_call_links(&links, profile, diags, pass1_errors);
    });

    _run_pass(
        "cr3xx::drop_block_closer_errors_after_errors",
        "cr3xx",
        || {
            cr3xx::drop_block_closer_errors_after_errors(program, diags);
        },
    );
    _run_pass("cr3xx::order_names_around_arity", "cr3xx", || {
        cr3xx::order_names_around_arity(program, diags);
    });
    _run_pass("cr3xx::order_names_before_call_arity", "cr3xx", || {
        cr3xx::order_names_before_call_arity(program, diags);
    });
    _run_pass("cr3xx::order_email_destination", "cr3xx", || {
        cr3xx::order_email_destination(program, diags);
    });
    _run_pass("cr3xx::drop_shortfall_with_names", "cr3xx", || {
        cr3xx::drop_shortfall_with_names(program, diags);
    });
    if profile.single_pass {
        _run_pass("instr_data::finalize_illegal_table_names", "cr3xx", || {
            instr_data::finalize_illegal_table_names(program, profile, diags);
        });
    }
    _run_pass("cr3xx::order_block_keyword_calls", "cr3xx", || {
        cr3xx::order_block_keyword_calls(program, diags);
    });
    _run_pass("cr3xx::fix_then_operand", "cr3xx", || {
        cr3xx::fix_then_operand(program, diags);
    });
    _run_pass("cr3xx::order_invalid_assignments", "cr3xx", || {
        cr3xx::order_invalid_assignments(program, diags);
    });
    _run_pass("cr3xx::order_unterminated_string", "cr3xx", || {
        cr3xx::order_unterminated_string(program, diags);
    });
    _run_pass("cr3xx::check_tcpopen_handle", "cr3xx", || {
        cr3xx::check_tcpopen_handle(program, diags);
    });
    _run_pass("cr3xx::add_string_illegal", "cr3xx", || {
        cr3xx::add_string_illegal(program, diags);
    });
    _run_pass("warnings::add_truncate_warnings", "cr3xx", || {
        warnings::add_truncate_warnings(ctx.flat(), diags);
    });
    _run_pass("cr3xx::semicolon_headers", "cr3xx", || {
        cr3xx::semicolon_headers(program, diags);
    });
    _run_pass("intervals::fold_timeout_expressions", "cr3xx", || {
        intervals::fold_timeout_expressions(ctx.flat(), diags);
    });
    _run_pass("intervals::order_timeout_negative", "cr3xx", || {
        intervals::order_timeout_negative(diags, program);
    });

    if let Err(payload) = catch_unwind(AssertUnwindSafe(|| {
        result.mode = Some(crate::mode::decide_with_env(
            &program.statements,
            profile,
            env,
        ));
    })) {
        _record_exception("mode::decide", &*payload);
        result.mode = None;
        if std::env::var("OPENCRBASIC_STRICT").as_deref() == Ok("1") {
            resume_unwind(payload);
        }
    }

    if !diags.has_errors() {
        _run_pass("warnings::check_background_calibration", "post", || {
            warnings::check_background_calibration(program, profile, diags);
        });
        _run_pass("warnings::check_slow_scan_skips", "post", || {
            warnings::check_slow_scan_skips(program, profile, diags);
        });
        _run_pass("instr_measure::check_cpi_time", "post", || {
            instr_measure::check_cpi_time(program, profile, diags);
        });
        _run_pass("instr_data::check_duplicate_fields", "post", || {
            // TODO(wire): tables builder must provide duplicate-field bookkeeping.
            instr_data::check_duplicate_fields(program, &result.tables, None, diags);
        });
        if !diags.has_errors() {
            _run_pass("storage::check", "post", || {
                storage::check(program);
            });
        }
        if !diags.has_errors() {
            _run_pass("warnings::check", "post", || {
                warnings::check(program, diags);
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profiles;
    use crate::syntax::{Instruction, StmtBase};

    fn diag(severity: &str, line: i64, msg: &str, kind: &str, phase: &str, pos: i64) -> DiagRef {
        let mut d = Diagnostic::new(
            severity,
            line,
            msg,
            "f.cr6",
            ShowFile::Bool(true),
            0,
            kind,
            phase,
        );
        d.pos = pos;
        Rc::new(d)
    }

    #[test]
    fn run_pass_and_aside_catch_panics_and_stamp_phase() {
        let mut diags = Diagnostics::new();
        _run_pass("ok_pass", "pass1", || {
            assert_eq!(crate::diagnostics::active_phase(), "pass1");
        });
        assert_eq!(crate::diagnostics::active_phase(), "");

        // Panic inside `_run_pass` is caught and phase is restored.
        _run_pass("panicking_pass", "tier_b", || {
            panic!("boom");
        });
        assert_eq!(crate::diagnostics::active_phase(), "");

        // `_run_aside` records diagnostics into `diags.asides` with `phase` stamped.
        _run_aside("aside_pass", "bounds", &mut diags, |bucket| {
            bucket.error_basic(5, "Variable x out of bounds.", "f.cr6");
        });
        assert_eq!(diags.items.len(), 0);
        assert_eq!(diags.asides.len(), 1);
        assert_eq!(diags.asides[0].phase, "bounds");

        // `_run_pass_capture` returns `None` on panic and `Some(v)` on success.
        assert_eq!(_run_pass_capture("cap_ok", "late", || 42), Some(42));
        assert_eq!(
            _run_pass_capture::<i32>("cap_panic", "late", || panic!("boom")),
            None
        );
        assert_eq!(crate::diagnostics::active_phase(), "");
    }

    #[test]
    fn place_index_bounds_suppresses_duplicates_and_orders_portget() {
        let portget = Rc::new(Stmt::Instruction(Instruction {
            base: StmtBase::new(3, "f.cr6"),
            keyword: "PORTGET".to_string(),
            ..Default::default()
        }));
        let prog = Program::new("f.cr6", vec![portget]);
        let inv_port = diag("error", 3, "Invalid port", "invalid_port", "tier_c", 0);
        let dup = diag(
            "error",
            3,
            "Variable x out of bounds.",
            "out_of_bounds",
            "bounds",
            0,
        );
        let dup_printed = diag(
            "error",
            3,
            "Variable x out of bounds.",
            "out_of_bounds",
            "tier_c",
            0,
        );
        let b_portget = diag(
            "error",
            3,
            "Variable y out of bounds.",
            "out_of_bounds",
            "bounds",
            0,
        );
        let b_tail = diag(
            "error",
            4,
            "Variable z out of bounds.",
            "out_of_bounds",
            "bounds",
            1,
        );

        let (out, kept) = _place_index_bounds(
            &[dup, b_portget.clone(), b_tail.clone()],
            &[inv_port.clone(), dup_printed],
            &prog,
            false,
        );
        // `b_portget` is inserted ahead of `invalid_port`; `dup` was suppressed; `b_tail` stays in `kept`.
        assert_eq!(out.len(), 3);
        assert!(Rc::ptr_eq(&out[0], &b_portget));
        assert!(Rc::ptr_eq(&out[1], &inv_port));
        assert_eq!(kept.len(), 1);
        assert!(Rc::ptr_eq(&kept[0], &b_tail));
    }

    #[test]
    fn resolve_merges_leading_claims_soft_and_oom_cutoff() {
        let prof = profiles::get("cr6").unwrap();
        let prog = Program::new("f.cr6", vec![]);
        let mut diags = Diagnostics::new();
        diags
            .items
            .push(diag("error", 1, "e1", "other", "tier_c", 0));
        diags
            .items
            .push(diag("error", 2, "oom", "out_of_memory", "tier_c", 1));
        diags
            .items
            .push(diag("error", 3, "after oom", "other", "tier_c", 2));
        diags
            .asides
            .push(diag("error", 1, "lead1", "other", "leading", 0));
        diags
            .asides
            .push(diag("error", 2, "sdi", "sdi12_conflict", "claims", 1));

        _resolve(&mut diags, &prog, &prog, prof, None, None);
        // `after oom` cut off; `sdi12_conflict` dropped because `e1` precedes it; `lead1` placed before `e1`.
        let msgs: Vec<&str> = diags.items.iter().map(|d| d.message.as_str()).collect();
        assert_eq!(msgs, ["lead1", "e1", "oom"]);
        assert_eq!(diags.asides.len(), 0);
    }
}
