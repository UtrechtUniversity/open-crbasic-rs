//! Port of `checker/structure.py` (lines 1-779).

#![allow(dead_code, unused_variables, unused_imports)]
#![allow(
    clippy::doc_markdown,
    reason = "AGENTS.md requires literal one-line port comments"
)]
#![allow(
    clippy::items_after_statements,
    reason = "nested helpers follow Python source order"
)]

use super::part2::{_block_word, _keyword, _scan_placement_walk};
use crate::checker::{context, instr_data, instr_rules, longconv};
use crate::consteval::{self, Env};
use crate::syntax::{ConstValue, NumberValue};
use crate::tables::part1 as tables;
use crate::{pyre, pystr};
use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;
use std::sync::LazyLock;

use crate::diagnostics::{DiagRef, Diagnostic, Diagnostics};
use crate::profiles::Profile;
use crate::syntax::{
    Assignment, Call, Case, ConstDecl, Declaration, Expr, ExprRef, Index, Instruction, Juxtaposed,
    Member, Number, OrderedMap, Program, ProgramRef, Stmt, StmtRef, SubDef, Unparsed, VarDecl,
};

pub const _MAX_ARRAY_DIMS: i64 = 3;
pub const _MAX_DIM_VALUE: i64 = 2_147_483_647;
pub const _KNOWN_PARSER_GAP_REASONS: &[&str] = &[
    "unexpected trailing input starting at '+='",
    "dotted SNMP OID literal",
];
pub const _BENIGN_REASONS: &[&str] = &["empty statement"];
pub const _UNITS_WITHOUT_EQ: &str = "Units without '='";
pub const _CONST_WITHOUT_EQ: &str = "Const without '='";
pub const _CONST_BAD_VALUE: &str = "Const bad value: ";
pub const _CONST_BAD_NAME: &str = "Const bad name: ";
pub const _CONST_LEAD: &str = "Const lead: ";
pub const _CONST_LEAD_NAME: &str = "Const lead: name ";
pub const _DO_LOOP_PARAM: &str = "Do/Loop parameter: ";
pub const _ALIAS_NOTHING_AFTER_EQ: &str = "Alias nothing after '='";
pub const _FOR_MISSING: &str = "For missing parameter";
pub const _ALIAS_WITHOUT_EQ: &str = "Alias without '='";
pub const _ALIAS_TARGET_NO_EQ: &str = "Alias target with no '='";
pub const _UNITS_TARGET_NO_EQ: &str = "Units target with no '='";
pub const _INVALID_AS_CLAUSE: &str = "Invalid As clause: ";
pub const _UNCLOSED_PAREN_EXPR: &str = crate::parser::helpers::UNCLOSED_PAREN_EXPR;
pub const _UNCLOSED_DIM_PREFIX: &str = crate::parser::helpers::UNCLOSED_DIM_PREFIX;
pub const _EXTRA_CLOSE_PAREN: &str = "unexpected trailing input starting at ')'";
pub static _ASSIGNMENT_HEAD: LazyLock<pyre::Regex> = LazyLock::new(|| {
    pyre::Regex::new(
        r"\s*[A-Za-z_]\w*\s*(?:\((?:[^()]|\((?:[^()]|\([^()]*\))*\))*\)\s*)?[-+*/\\^&]?=",
        0,
    )
});
static ASSIGNMENT_NAMES: LazyLock<pyre::Regex> =
    LazyLock::new(|| pyre::Regex::new(r"[A-Za-z_]\w*", 0));
static ASSIGNMENT_CALLS: LazyLock<pyre::Regex> =
    LazyLock::new(|| pyre::Regex::new(r"([A-Za-z_]\w*)\s*\(", 0));

/// Port of checker/structure.py::_assignment_head
pub fn _assignment_head(_text: &str) -> bool {
    todo!("checker/structure.py::_assignment_head (omitted: program: untyped, profile: untyped)")
}

/// Port of checker/structure.py::_assignment_head
pub fn _assignment_head_with(text: &str, program: &ProgramRef, profile: &'static Profile) -> bool {
    let Some(head) = _ASSIGNMENT_HEAD.match_(text) else {
        return false;
    };
    let end = i64::try_from(head.end0()).expect("source length fits i64");
    let inner = if pystr::slice(text, None, Some(end)).contains('(') {
        let start = pystr::find(text, "(", None, None).expect("opening parenthesis") + 1;
        pystr::slice(
            text,
            Some(i64::try_from(start).expect("source length fits i64")),
            Some(end),
        )
    } else {
        ""
    };
    if inner.contains('(') {
        let ctx = context::of(program, Some(profile), None);
        let known = ctx.known_names();
        if ASSIGNMENT_NAMES
            .finditer(inner)
            .iter()
            .any(|m| !known.names.contains(&pystr::upper(m.group0())))
        {
            return false;
        }
        if profile.fallback_echo_newline {
            let counts = ctx.dim_counts(false);
            if ASSIGNMENT_CALLS.finditer(inner).iter().any(|m| {
                counts
                    .counts
                    .get(&pystr::upper(m.group(1).expect("name capture")))
                    == Some(&0)
            }) {
                return false;
            }
        }
    }
    true
}

pub const _BODY_BLOCK_STARTS: &[&str] = &[
    "DIALSEQUENCE",
    "MODEMHANGUP",
    "MENU",
    "VOICE",
    "BURSTTRIGGER",
    "APPLYANDRESTARTSEQUENCE",
    "PROCHIPRI",
];
pub const _BODY_BLOCK_ENDS: &[&str] = &[
    "ENDDIALSEQUENCE",
    "ENDMODEMHANGUP",
    "ENDMENU",
    "ENDVOICE",
    "ENDBURSTTRIGGER",
    "ENDAPPLYANDRESTARTSEQUENCE",
    "ENDPROCHIPRI",
];
pub const _FORBIDDEN_BEFORE_BEGINPROG: &[&str] = &["SCAN", "DELAY"];
pub static _DECL_BLOCK_BOUNDS: LazyLock<OrderedMap<&'static str, bool>> = LazyLock::new(|| {
    let mut bounds = OrderedMap::new();
    for (key, value) in [
        ("WEBPAGEBEGIN", true),
        ("WEBPAGEEND", false),
        ("SHUTDOWNBEGIN", true),
        ("SHUTDOWNEND", false),
    ] {
        bounds.insert(key, value);
    }
    bounds
});

#[derive(Clone, Debug, Default)]
pub struct TableState {
    pub open: Option<String>,
    pub closed: Vec<String>,
}

/// Port of checker/structure.py::_track_tables
pub fn _track_tables(_stmt: &Stmt, _kw: &str) {
    todo!("checker/structure.py::_track_tables (omitted: state: dict)")
}

/// Port of checker/structure.py::_track_tables
pub fn _track_tables_with(stmt: &Stmt, kw: &str, state: &mut TableState) {
    if kw == "DATATABLE" {
        state.open = match stmt.args().first().and_then(Option::as_deref) {
            Some(Expr::Name(n)) => Some(pystr::upper(&n.name)),
            _ => None,
        };
    } else if kw == "ENDTABLE" && state.open.as_ref().is_some_and(|s| !s.is_empty()) {
        let name = state.open.take().expect("open table");
        if !state.closed.contains(&name) {
            state.closed.push(name);
        }
    }
}

/// Port of checker/structure.py::_calltable_known
pub fn _calltable_known(_stmt: &Stmt) {
    todo!("checker/structure.py::_calltable_known (omitted: state: dict)")
}

/// Port of checker/structure.py::_calltable_known
pub fn _calltable_known_with(stmt: &Stmt, state: &TableState) -> bool {
    matches!(stmt, Stmt::Instruction(_))
        && matches!(stmt.args().first().and_then(Option::as_deref), Some(Expr::Name(n)) if state.closed.contains(&pystr::upper(&n.name)))
}

pub const NESTED_SCAN: &str = "Scan without NextScan or nested Scans.";

/// Port of checker/structure.py::_scan_counts_for_table_placement
pub fn _scan_counts_for_table_placement(stmt: &Stmt) -> bool {
    _scan_counts_for_table_placement_with(stmt, None)
}

/// Port of checker/structure.py::_scan_counts_for_table_placement
pub fn _scan_counts_for_table_placement_with(stmt: &Stmt, env: Option<&Env>) -> bool {
    if let Some(Expr::Number(n)) = stmt.args().first().and_then(Option::as_deref)
        && match &n.value {
            NumberValue::Int(i) => *i == 0,
            NumberValue::Float(f) => *f == 0.0,
            NumberValue::BigInt(_) => false,
        }
    {
        return false;
    }
    if let Some(env) = env
        && let Stmt::Instruction(inst) = stmt
    {
        return instr_rules::scan_split(inst, env) != Some((0, 0));
    }
    true
}

/// Port of checker/structure.py::_check_calltable_outside_scan
pub fn _check_calltable_outside_scan(
    stmt: &Instruction,
    program: &Program,
    diags: &mut Diagnostics,
    in_sub: bool,
    zero_scan: bool,
) {
    _check_calltable_outside_scan_with(stmt, program, diags, in_sub, zero_scan, None);
}

/// Port of checker/structure.py::_check_calltable_outside_scan
pub fn _check_calltable_outside_scan_with(
    stmt: &Instruction,
    program: &Program,
    diags: &mut Diagnostics,
    in_sub: bool,
    zero_scan: bool,
    env: Option<&Env>,
) {
    let Some(Expr::Name(target)) = stmt.args.first().and_then(Option::as_deref) else {
        return;
    };
    let name = pystr::upper(&target.name);
    let mut table: Option<&Instruction> = None;
    let mut has_interval = false;
    for st in &program.statements {
        let kw = _keyword(st);
        if kw == "DATATABLE" {
            table = match st.as_ref() {
                Stmt::Instruction(inst) if matches!(inst.args.first().and_then(Option::as_deref), Some(Expr::Name(n)) if pystr::upper(&n.name) == name) => {
                    Some(inst)
                }
                _ => None,
            };
            has_interval = false;
        } else if kw == "DATAINTERVAL" && table.is_some() {
            has_interval = true;
        } else if kw == "ENDTABLE" && table.is_some() {
            break;
        }
    }
    let Some(table) = table else {
        return;
    };
    let size = table.args.get(2).and_then(Option::as_deref);
    let mut auto = match size {
        Some(Expr::Unary(u)) => u.op == "-",
        Some(Expr::Number(n)) => match &n.value {
            NumberValue::Int(i) => *i < 0,
            NumberValue::Float(f) => *f < 0.0,
            NumberValue::BigInt(s) => s.starts_with('-'),
        },
        _ => false,
    };
    if let Some(env) = env
        && let Some(size) = size
    {
        let mut raw = tables::_fold_with(Some(size), env, None);
        if let Expr::Number(n) = size
            && pystr::startswith_any(&pystr::upper(&n.text), &["&H", "&B", "&O"])
        {
            raw = longconv::long_value_with(
                Some(size),
                env,
                env.profile,
                &BTreeSet::new(),
                &BTreeSet::new(),
            )
            .map(|v| {
                if v == longconv::INT_MIN {
                    ConstValue::Float(f64::NAN)
                } else {
                    ConstValue::Int(v)
                }
            });
        }
        if raw.is_some() {
            let cr3xx = env.profile.single_pass;
            auto = tables::declared_size_with(raw.as_ref(), cr3xx, 1).is_none()
                || (cr3xx
                    && tables::size_class_with(raw.as_ref()) == Some(tables::SizeClass::Toxic));
        } else if matches!(size, Expr::Name(n) if pystr::upper(&n.name) == "NAN") {
            auto = true;
        }
    }
    if zero_scan && has_interval {
        return;
    }
    if has_interval && !in_sub {
        diags.error_basic(
            stmt.base.node.line,
            "Table with DataInterval must be called within Scan..NextScan.",
            &stmt.base.node.file,
        );
    }
    if auto && !(in_sub && has_interval) {
        let Some(Expr::Name(n)) = table.args.first().and_then(Option::as_deref) else {
            unreachable!("matched table name");
        };
        diags.error_basic(stmt.base.node.line, &format!("Cannot autoallocate memory for DataTable {} called outside of Scan, e.g., called by Sub.", n.name), &stmt.base.node.file);
    }
}

/// Port of checker/structure.py::_accept_known
pub fn _accept_known(_stmt: &Stmt) {
    todo!("checker/structure.py::_accept_known (omitted: state: dict)")
}

/// Port of checker/structure.py::_accept_known
pub fn _accept_known_with(stmt: &Stmt, state: &TableState) -> bool {
    matches!(stmt, Stmt::Instruction(_))
        && matches!(stmt.args().get(2).and_then(Option::as_deref), Some(Expr::Name(n)) if state.closed.contains(&pystr::upper(&n.name)))
}

/// Port of checker/structure.py::_check_accept_table
pub fn _check_accept_table(
    _stmt: &Instruction,
    _program: &Program,
    _diags: &mut Diagnostics,
    _outside: bool,
) {
    todo!("checker/structure.py::_check_accept_table (omitted: env: untyped)")
}

/// Port of checker/structure.py::_check_accept_table
pub fn _check_accept_table_with(
    stmt: &Instruction,
    program: &Program,
    diags: &mut Diagnostics,
    outside: bool,
    env: &Env,
) {
    let Some(Expr::Name(source)) = stmt.args.get(2).and_then(Option::as_deref) else {
        panic!("_accept_known requires a table name");
    };
    let name = pystr::upper(&source.name);
    let mut table: Option<&Instruction> = None;
    let mut driven = false;
    for st in &program.statements {
        let kw = _keyword(st);
        if kw == "DATATABLE" {
            table = match st.as_ref() {
                Stmt::Instruction(inst) if matches!(inst.args.first().and_then(Option::as_deref), Some(Expr::Name(n)) if pystr::upper(&n.name) == name) => {
                    Some(inst)
                }
                _ => None,
            };
            driven = false;
        } else if kw == "DATAINTERVAL" && table.is_some() {
            let val = st.args().get(3).map_or(Some(ConstValue::Int(10)), |v| {
                tables::_fold_with(v.as_deref(), env, None)
            });
            driven = driven
                || !matches!(val, Some(ConstValue::Int(0)))
                    && !matches!(val, Some(ConstValue::Float(f) | ConstValue::CFloat(f)) if f == 0.0);
        } else if kw == "ENDTABLE" && table.is_some() {
            break;
        }
    }
    let Some(table) = table else {
        return;
    };
    let size = table.args.get(2).and_then(Option::as_deref);
    let mut auto = match size {
        Some(Expr::Unary(u)) => u.op == "-",
        Some(Expr::Number(n)) => match &n.value {
            NumberValue::Int(i) => *i < 0,
            NumberValue::Float(f) => *f < 0.0,
            NumberValue::BigInt(s) => s.starts_with('-'),
        },
        _ => false,
    };
    let raw = tables::_fold_with(size, env, None);
    if raw.is_some() {
        auto = tables::declared_size_with(
            raw.as_ref(),
            matches!(env.profile.name, "cr300" | "cr350"),
            1,
        )
        .is_none();
    }
    if outside && auto {
        let Some(Expr::Name(n)) = table.args.first().and_then(Option::as_deref) else {
            unreachable!("matched table name");
        };
        diags.error_basic(stmt.base.node.line, &format!("Cannot autoallocate memory for DataTable {} called outside of Scan, e.g., called by Sub.", n.name), &stmt.base.node.file);
    }
    if driven {
        diags.error_basic(
            stmt.base.node.line,
            "Data Table cannot be interval driven.",
            &stmt.base.node.file,
        );
    }
}

/// Port of checker/structure.py::_ends_sub
pub fn _ends_sub(stmt: &Stmt, kw: &str) -> bool {
    matches!(kw, "ENDSUB" | "ENDFUNCTION")
        || (kw == "END" && matches!(_block_word().as_deref(), Some("SUB" | "FUNCTION")))
    // TODO: _block_word's existing signature omits stmt.args().
}

/// Port of checker/structure.py::first_misplaced_sub
pub fn first_misplaced_sub(program: &Program) {
    let _ = first_misplaced_sub_with(program);
}

/// Port of checker/structure.py::first_misplaced_sub
pub fn first_misplaced_sub_with(program: &Program) -> Option<(StmtRef, Vec<String>)> {
    let mut global_variables = BTreeSet::new();
    let mut in_routine = false;
    for stmt in &program.statements {
        let kw = _keyword(stmt);
        if matches!(stmt.as_ref(), Stmt::SubDef(_)) {
            in_routine = true;
        } else if _ends_sub(stmt, &kw) {
            in_routine = false;
        } else if !in_routine && let Stmt::Declaration(decl) = stmt.as_ref() {
            global_variables.extend(decl.vars.iter().map(|v| pystr::upper(&v.name)));
        }
    }
    let (mut open_sub, mut in_table, mut before_sequence) = (false, false, false);
    let (mut conditional_depth, mut active_scans, mut for_do_depth) = (0_i64, 0_i64, 0_i64);
    let mut routines = BTreeSet::new();
    let mut consts = BTreeSet::new();
    let mut in_program = false;
    for stmt in &program.statements {
        let kw = _keyword(stmt);
        in_program = in_program || kw == "BEGINPROG";
        if kw == "ENDPROG" {
            in_program = false;
        }
        if let Stmt::If(if_stmt) = stmt.as_ref()
            && if_stmt.inline.is_some()
            && (in_program || open_sub || in_table)
        {
            for inner in if_stmt
                .inline
                .iter()
                .flatten()
                .chain(if_stmt.inline_else.iter().flatten())
            {
                if let Stmt::SubDef(sub) = inner.as_ref() {
                    let mut msgs = Vec::new();
                    if open_sub {
                        msgs.push("Sub declared within a Sub.".to_owned());
                    }
                    msgs.push("Sub declaration inside If..EndIf.".to_owned());
                    msgs.push(format!(
                        "{} is declared in the middle of an executing sequence",
                        if pystr::upper(&sub.keyword) == "FUNCTION" {
                            "Function"
                        } else {
                            "Sub"
                        }
                    ));
                    return Some((stmt.clone(), msgs));
                }
            }
        }
        if let Stmt::ConstDecl(c) = stmt.as_ref() {
            consts.insert(pystr::upper(&c.name));
        } else if let Stmt::SubDef(sub) = stmt.as_ref() {
            if consts.contains(&pystr::upper(&sub.name)) {
                open_sub = true;
                continue;
            }
            let executing = open_sub
                || in_table
                || before_sequence
                || active_scans > 0
                || conditional_depth > 0
                || for_do_depth > 0;
            let duplicate = routines.contains(&pystr::upper(&sub.name));
            let variable = global_variables.contains(&pystr::upper(&sub.name));
            if executing || duplicate || variable {
                let mut msgs = Vec::new();
                if open_sub {
                    msgs.push("Sub declared within a Sub.".to_owned());
                }
                if duplicate {
                    msgs.push(format!("Duplicate Sub {}", sub.name));
                }
                if variable {
                    msgs.push(format!("{} already declared as a variable.", sub.name));
                }
                if conditional_depth > 0 {
                    msgs.push("Sub declaration inside If..EndIf.".to_owned());
                }
                if executing {
                    msgs.push(format!(
                        "{} is declared in the middle of an executing sequence",
                        if pystr::upper(&sub.keyword) == "FUNCTION" {
                            "Function"
                        } else {
                            "Sub"
                        }
                    ));
                }
                return Some((stmt.clone(), msgs));
            }
            routines.insert(pystr::upper(&sub.name));
            open_sub = true;
        } else if _ends_sub(stmt, &kw) {
            open_sub = false;
            conditional_depth = 0;
            for_do_depth = 0;
        } else if kw == "BEGINPROG" {
            before_sequence = true;
        } else if matches!(kw.as_str(), "ENDPROG" | "SLOWSEQUENCE") {
            before_sequence = false;
        } else if kw == "SCAN" {
            active_scans += 1;
            before_sequence = false;
        } else if kw == "NEXTSCAN" || (kw == "NEXT" && _block_word().as_deref() == Some("SCAN")) {
            active_scans = (active_scans - 1).max(0);
        } else if kw == "DATATABLE" {
            in_table = true;
        } else if kw == "ENDTABLE" {
            in_table = false;
        } else if kw == "SELECT"
            || (kw == "IF" && !matches!(stmt.as_ref(), Stmt::If(s) if s.inline.is_some()))
        {
            conditional_depth += 1;
        } else if matches!(kw.as_str(), "ENDSELECT" | "ENDIF")
            || (kw == "END" && matches!(_block_word().as_deref(), Some("SELECT" | "IF")))
        {
            conditional_depth = (conditional_depth - 1).max(0);
        } else if matches!(kw.as_str(), "FOR" | "DO" | "WHILE") {
            for_do_depth += 1;
        } else if matches!(kw.as_str(), "LOOP" | "WEND")
            || (kw == "NEXT" && _block_word().as_deref() != Some("SCAN"))
        {
            for_do_depth = (for_do_depth - 1).max(0);
        }
    }
    None
}

/// Port of checker/structure.py::check_apply_restart_sequence
pub fn check_apply_restart_sequence(program: &Program, diags: &mut Diagnostics) {
    check_apply_restart_sequence_with(program, &crate::profiles::PROFILES[0], diags);
}

/// Port of checker/structure.py::check_apply_restart_sequence
pub fn check_apply_restart_sequence_with(
    program: &Program,
    profile: &Profile,
    diags: &mut Diagnostics,
) {
    let (mut declared, mut open_sub, mut in_table, mut before_sequence) =
        (false, false, false, false);
    let (mut conditional_depth, mut active_scans, mut for_do_depth) = (0_i64, 0_i64, 0_i64);
    for stmt in &program.statements {
        let kw = _keyword(stmt);
        if kw == "APPLYANDRESTARTSEQUENCE" {
            let executing = open_sub
                || in_table
                || before_sequence
                || active_scans > 0
                || conditional_depth > 0
                || for_do_depth > 0;
            if declared {
                diags.error_basic(
                    stmt.base().node.line,
                    "ApplyAndRestartSequence already declared",
                    &stmt.base().node.file,
                );
            } else if executing {
                diags.error_basic(
                    stmt.base().node.line,
                    "ApplyAndRestartSequence is declared in the middle of an executing sequence",
                    &stmt.base().node.file,
                );
                return;
            }
            declared = true;
        } else if kw == "ENDAPPLYANDRESTARTSEQUENCE" {
            if !declared {
                diags.error_basic(
                    stmt.base().node.line,
                    "ApplyAndRestartSequence not declared",
                    &stmt.base().node.file,
                );
            }
        } else if matches!(stmt.as_ref(), Stmt::SubDef(_)) {
            open_sub = true;
        } else if _ends_sub(stmt, &kw) {
            open_sub = false;
            conditional_depth = 0;
        } else if kw == "BEGINPROG" {
            before_sequence = true;
        } else if matches!(kw.as_str(), "ENDPROG" | "SLOWSEQUENCE") {
            before_sequence = false;
        } else if kw == "SCAN" {
            active_scans += 1;
            before_sequence = false;
        } else if kw == "NEXTSCAN" || (kw == "NEXT" && _block_word().as_deref() == Some("SCAN")) {
            active_scans = (active_scans - 1).max(0);
        } else if kw == "DATATABLE" {
            in_table = true;
        } else if kw == "ENDTABLE" {
            in_table = false;
        } else if kw == "SELECT"
            || (kw == "IF" && !matches!(stmt.as_ref(), Stmt::If(s) if s.inline.is_some()))
        {
            conditional_depth += 1;
        } else if matches!(kw.as_str(), "ENDSELECT" | "ENDIF")
            || (kw == "END" && matches!(_block_word().as_deref(), Some("SELECT" | "IF")))
        {
            conditional_depth = (conditional_depth - 1).max(0);
        } else if matches!(kw.as_str(), "FOR" | "DO" | "WHILE") {
            for_do_depth += 1;
        } else if matches!(kw.as_str(), "LOOP" | "WEND")
            || (kw == "NEXT" && _block_word().as_deref() != Some("SCAN"))
        {
            for_do_depth = (for_do_depth - 1).max(0);
        }
    }
}

/// Port of checker/structure.py::first_misplaced_data_table
pub fn first_misplaced_data_table(_program: &Program) -> Option<StmtRef> {
    todo!("checker/structure.py::first_misplaced_data_table (omitted: profile: untyped)")
}

/// Port of checker/structure.py::first_misplaced_data_table
pub fn first_misplaced_data_table_with(program: &Program, profile: &Profile) -> Option<StmtRef> {
    let (mut prescan, mut in_table, mut in_apply, mut in_routine) = (false, false, false, false);
    let (mut scans, mut blocks) = (0_i64, 0_i64);
    let mut slow_block_error = false;
    for stmt in &program.statements {
        let kw = _keyword(stmt);
        if kw == "DATATABLE" {
            if !in_table
                && (in_apply
                    || in_routine
                    || (!slow_block_error && (prescan || scans != 0 || blocks != 0)))
            {
                return Some(stmt.clone());
            }
            in_table = true;
        } else if kw == "ENDTABLE" {
            in_table = false;
        } else if kw == "BEGINPROG" {
            prescan = true;
        } else if kw == "ENDPROG" {
            prescan = false;
        } else if kw == "APPLYANDRESTARTSEQUENCE" {
            in_apply = true;
        } else if kw == "ENDAPPLYANDRESTARTSEQUENCE" {
            in_apply = false;
        } else if matches!(stmt.as_ref(), Stmt::SubDef(_)) {
            in_routine = true;
        } else if crate::syntax::ends_routine(stmt) {
            in_routine = false;
        } else if kw == "SLOWSEQUENCE" {
            if blocks != 0 && matches!(profile.name, "cr6" | "cr1000x") {
                slow_block_error = true;
            }
            prescan = false;
        } else if kw == "SCAN" {
            scans += 1;
            prescan = false;
        } else if kw == "NEXTSCAN" || (kw == "NEXT" && _block_word().as_deref() == Some("SCAN")) {
            scans = (scans - 1).max(0);
        } else if kw == "SELECT"
            || (kw == "IF" && !matches!(stmt.as_ref(), Stmt::If(s) if s.inline.is_some()))
            || matches!(kw.as_str(), "FOR" | "DO" | "WHILE")
        {
            blocks += 1;
        } else if matches!(kw.as_str(), "ENDIF" | "ENDSELECT" | "LOOP" | "WEND")
            || (kw == "END" && matches!(_block_word().as_deref(), Some("IF" | "SELECT")))
            || (kw == "NEXT" && _block_word().as_deref() != Some("SCAN"))
        {
            blocks = (blocks - 1).max(0);
        }
    }
    None
}

/// Port of checker/structure.py::check_data_table_placement
pub fn check_data_table_placement(_program: &Program, _diags: &mut Diagnostics) {
    todo!("checker/structure.py::check_data_table_placement (omitted: profile: untyped)")
}

/// Port of checker/structure.py::check_data_table_placement
pub fn check_data_table_placement_with(
    program: &Program,
    profile: &Profile,
    diags: &mut Diagnostics,
) {
    if let Some(stmt) = first_misplaced_data_table_with(program, profile) {
        diags.error_basic(
            stmt.base().node.line,
            "DataTable is declared in the middle of an executing sequence",
            &stmt.base().node.file,
        );
    }
}

/// Port of checker/structure.py::pipeline_task_errors
pub fn pipeline_task_errors(_program: &Program) {
    todo!("checker/structure.py::pipeline_task_errors (omitted: profile: untyped)")
}

/// Port of checker/structure.py::pipeline_task_errors
pub fn pipeline_task_errors_with(
    program: &Program,
    profile: &'static Profile,
) -> Vec<(StmtRef, String)> {
    use crate::profiles::Json;
    let mut found = Vec::new();
    if profile.single_pass {
        return found;
    }
    let Some(task_rules) = profile
        .pipeline_tasks()
        .get("tasks")
        .filter(|v| !v.is_empty())
    else {
        return found;
    };
    let env = Env::with_consts(profile, Rc::new(RefCell::new(program.consts.clone())));
    let explicit = program
        .statements
        .iter()
        .any(|st| _keyword(st) == "PIPELINEMODE");
    let implicit =
        !explicit && crate::mode::decide_with_env(&program.statements, profile, &env) == "Pipeline";
    struct State {
        pipeline: bool,
        sequential: bool,
        scans: i64,
        depth: i64,
        slow: bool,
        closed: bool,
        outer: Vec<i64>,
    }
    #[allow(
        clippy::float_cmp,
        reason = "Python list membership uses exact numeric equality"
    )]
    fn allowed_value(allowed: &Json, value: &ConstValue) -> bool {
        allowed.as_array().is_some_and(|values| {
            values.iter().any(|v| match (v, value) {
                (Json::String(a), ConstValue::Str(b)) => a == b,
                (Json::Int(a), ConstValue::Int(b)) => a == b,
                (Json::Bool(a), ConstValue::Int(b)) => i64::from(*a) == *b,
                (Json::Float(a), ConstValue::Int(b)) => {
                    pystr::is_integer(*a) && pystr::py_int_from_f64(*a) == Some(*b)
                }
                (Json::Int(a), ConstValue::Float(b) | ConstValue::CFloat(b)) => {
                    pystr::is_integer(*b) && pystr::py_int_from_f64(*b) == Some(*a)
                }
                (Json::Bool(a), ConstValue::Float(b) | ConstValue::CFloat(b)) => {
                    pystr::is_integer(*b) && pystr::py_int_from_f64(*b) == Some(i64::from(*a))
                }
                (Json::Float(a), ConstValue::Float(b) | ConstValue::CFloat(b)) => a == b,
                _ => false,
            })
        })
    }
    fn visit(
        stmt: &StmtRef,
        inline: bool,
        state: &mut State,
        task_rules: &Json,
        profile: &Profile,
        env: &Env,
        found: &mut Vec<(StmtRef, String)>,
    ) {
        let kw = _keyword(stmt);
        let args = stmt.args();
        let word = if matches!(kw.as_str(), "END" | "NEXT") {
            _block_word()
        } else {
            None
        };
        if kw == "SEQUENTIALMODE" {
            state.sequential = true;
        } else if kw == "PIPELINEMODE" {
            state.pipeline = true;
        } else if kw == "SLOWSEQUENCE" {
            state.slow = true;
            state.closed = false;
        } else if kw == "SCAN" {
            state.scans += 1;
            state.outer.push(state.depth);
            state.depth = 0;
        } else if kw == "NEXTSCAN" || (kw == "NEXT" && word.as_deref() == Some("SCAN")) {
            state.scans = (state.scans - 1).max(0);
            state.closed = state.closed || state.scans == 0;
            state.depth = state.outer.pop().unwrap_or(state.depth);
        } else if matches!(kw.as_str(), "SELECT" | "FOR" | "DO" | "WHILE")
            || (kw == "IF" && !matches!(stmt.as_ref(), Stmt::If(s) if s.inline.is_some()))
        {
            state.depth += 1;
        } else if matches!(kw.as_str(), "ENDIF" | "ENDSELECT" | "LOOP" | "WEND")
            || (kw == "NEXT" && word.as_deref() != Some("SUBSCAN"))
            || (kw == "END" && matches!(word.as_deref(), Some("IF" | "SELECT")))
        {
            state.depth = (state.depth - 1).max(0);
        } else if let Some(rule) = task_rules.get(&kw)
            && state.pipeline
            && !state.sequential
            && let Stmt::Instruction(inst) = stmt.as_ref()
        {
            if state.slow && rule.get("main_sequence_only").and_then(Json::as_bool) == Some(true) {
                return;
            }
            for (position, allowed) in rule
                .get("constant_values")
                .and_then(Json::as_object)
                .unwrap_or(&[])
            {
                let Some(index) = pystr::parse_int(position)
                    .and_then(|v| v.checked_sub(1))
                    .and_then(|v| usize::try_from(v).ok())
                else {
                    return;
                };
                let Some(arg) = args.get(index) else {
                    return;
                };
                let Ok(value) = consteval::value_of(arg.as_deref(), env) else {
                    return;
                };
                if !allowed_value(allowed, &value) && !matches!(value, ConstValue::Str(_)) {
                    return;
                }
            }
            for (position, exempt) in rule
                .get("exempt_values")
                .and_then(Json::as_object)
                .unwrap_or(&[])
            {
                let Some(index) = pystr::parse_int(position)
                    .and_then(|v| v.checked_sub(1))
                    .and_then(|v| usize::try_from(v).ok())
                else {
                    return;
                };
                let value = if let Some(arg) = args.get(index) {
                    let Ok(value) = consteval::value_of(arg.as_deref(), env) else {
                        return;
                    };
                    Some(value)
                } else {
                    None
                };
                if value.as_ref().is_some_and(|v| allowed_value(exempt, v))
                    || (value.is_none()
                        && exempt.as_array().is_some_and(|a| a.contains(&Json::Null)))
                {
                    return;
                }
            }
            let name = profile
                .instructions()
                .get(&kw)
                .map_or(inst.keyword.as_str(), |r| r.name.as_str());
            let custom = rule.get("messages");
            let setup = rule.get("setup").and_then(Json::as_bool).unwrap_or(false);
            if state.scans == 0
                && (if setup {
                    state.closed
                } else {
                    !rule
                        .get("conditional_only")
                        .and_then(Json::as_bool)
                        .unwrap_or(false)
                })
            {
                let message = custom
                    .and_then(|v| v.get("outside"))
                    .and_then(Json::as_str)
                    .filter(|s| !s.is_empty())
                    .map_or_else(
                        || {
                            format!(
                                "{name} not allowed outside of Scan..NextScan in PipeLine Mode."
                            )
                        },
                        str::to_owned,
                    );
                found.push((stmt.clone(), message));
            } else if (state.depth != 0 || inline) && !(setup && state.scans == 0) {
                let message = custom.and_then(|v| v.get("conditional")).and_then(Json::as_str).filter(|s| !s.is_empty()).map_or_else(|| format!("{name} not allowed inside of a conditional statement in PipeLine Mode."), str::to_owned);
                found.push((stmt.clone(), message));
            }
        }
        if let Stmt::If(if_stmt) = stmt.as_ref()
            && if_stmt.inline.is_some()
        {
            for inner in if_stmt
                .inline
                .iter()
                .flatten()
                .chain(if_stmt.inline_else.iter().flatten())
            {
                visit(inner, true, state, task_rules, profile, env, found);
            }
        }
    }
    let mut state = State {
        pipeline: implicit,
        sequential: false,
        scans: 0,
        depth: 0,
        slow: false,
        closed: false,
        outer: Vec::new(),
    };
    for stmt in &program.statements {
        visit(
            stmt, false, &mut state, task_rules, profile, &env, &mut found,
        );
    }
    found
}

/// Port of checker/structure.py::check_pipeline_task_placement
pub fn check_pipeline_task_placement(_program: &Program, _diags: &mut Diagnostics) {
    todo!("checker/structure.py::check_pipeline_task_placement (omitted: profile: untyped)")
}

/// Port of checker/structure.py::check_pipeline_task_placement
pub fn check_pipeline_task_placement_with(
    program: &Program,
    profile: &'static Profile,
    diags: &mut Diagnostics,
) {
    for (stmt, message) in pipeline_task_errors_with(program, profile) {
        diags.error_basic(stmt.base().node.line, &message, &stmt.base().node.file);
    }
}

/// Port of checker/structure.py::pipeline_argument_error_first_locations
pub fn pipeline_argument_error_first_locations(_program: &Program) {
    todo!(
        "checker/structure.py::pipeline_argument_error_first_locations (omitted: profile: untyped)"
    )
}

/// Port of checker/structure.py::pipeline_argument_error_first_locations
pub fn pipeline_argument_error_first_locations_with(
    program: &Program,
    profile: &'static Profile,
) -> BTreeSet<(String, i64)> {
    let first = profile
        .pipeline_tasks()
        .get("argument_error_first")
        .and_then(crate::profiles::Json::as_array)
        .unwrap_or(&[]);
    pipeline_task_errors_with(program, profile)
        .into_iter()
        .filter(|(stmt, _)| {
            first
                .iter()
                .any(|v| v.as_str() == Some(_keyword(stmt).as_str()))
        })
        .map(|(stmt, _)| (stmt.base().node.file.clone(), stmt.base().node.line))
        .collect()
}

/// Port of checker/structure.py::check_slow_sequence_blocks
pub fn check_slow_sequence_blocks(_program: &Program, _diags: &mut Diagnostics) {
    todo!("checker/structure.py::check_slow_sequence_blocks (omitted: profile: untyped)")
}

/// Port of checker/structure.py::check_slow_sequence_blocks
pub fn check_slow_sequence_blocks_with(
    program: &Program,
    profile: &Profile,
    diags: &mut Diagnostics,
) {
    if profile.single_pass {
        return;
    }
    let (mut fors, mut dos, mut ifs) = (0_i64, 0_i64, 0_i64);
    for stmt in &program.statements {
        let kw = _keyword(stmt);
        let word = if matches!(kw.as_str(), "END" | "NEXT") {
            _block_word()
        } else {
            None
        };
        if kw == "FOR" {
            fors += 1;
        } else if kw == "NEXT" && !matches!(word.as_deref(), Some("SCAN" | "SUBSCAN")) {
            fors = (fors - 1).max(0);
        } else if matches!(kw.as_str(), "DO" | "WHILE") {
            dos += 1;
        } else if matches!(kw.as_str(), "LOOP" | "WEND") {
            dos = (dos - 1).max(0);
        } else if kw == "SELECT"
            || (kw == "IF" && !matches!(stmt.as_ref(), Stmt::If(s) if s.inline.is_some()))
        {
            ifs += 1;
        } else if matches!(kw.as_str(), "ENDIF" | "ENDSELECT")
            || (kw == "END" && matches!(word.as_deref(), Some("IF" | "SELECT")))
        {
            ifs = (ifs - 1).max(0);
        } else if kw == "SLOWSEQUENCE" {
            for (depth, message) in [
                (fors, "FOR .. NEXT cannot span across sequences."),
                (dos, "DO .. LOOP cannot span across sequences."),
                (ifs, "IF or SELECT cannot span sequences."),
            ] {
                if depth != 0 {
                    diags.error_basic(stmt.base().node.line, message, &stmt.base().node.file);
                }
            }
        }
    }
}

/// Port of checker/structure.py::check_scan_placement_late
pub fn check_scan_placement_late(_program: &Program, _diags: &mut Diagnostics) {
    todo!("checker/structure.py::check_scan_placement_late (omitted: profile: untyped)")
}

/// Port of checker/structure.py::check_scan_placement_late
pub fn check_scan_placement_late_with(
    program: &Program,
    profile: &'static Profile,
    diags: &mut Diagnostics,
) -> Option<(String, i64)> {
    let n_before = diags.items.len();
    // The dependency is still a unit-returning stub. This adapter must be removed once
    // part2 provides _scan_placement_walk_with(program, profile, diags) -> Option<(String, i64)>.
    fn walk(
        program: &Program,
        profile: &Profile,
        diags: &mut Diagnostics,
    ) -> Option<(String, i64)> {
        _scan_placement_walk(program, diags);
        todo!("wire part2::_scan_placement_walk_with with profile and stop-location result")
    }
    let stop = walk(program, profile, diags)?;
    let Some(at) = program
        .statements
        .iter()
        .position(|st| (st.base().node.file.clone(), st.base().node.line) == stop)
    else {
        return Some(stop);
    };
    let mut early = Diagnostics::default();
    let mut prefix = program.replaced();
    prefix.statements = program.statements[..at].to_vec();
    let prefix = Rc::new(prefix);
    let table_stop = instr_data::check_table_structure_with(&prefix, profile, &mut early);
    for d in early.items {
        if d.severity != "error" {
            continue;
        }
        let index = (n_before..diags.items.len())
            .find(|&i| diags.items[i].severity == "error" && diags.items[i].line > d.line)
            .unwrap_or(diags.items.len());
        diags.items.insert(index, d);
    }
    if let Some(table_stop) = table_stop {
        let tail: Vec<_> = diags
            .items
            .drain(n_before..)
            .filter(|d| d.severity != "error" || d.line <= table_stop.1)
            .collect();
        diags.items.extend(tail);
        return Some(table_stop);
    }
    Some(stop)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syntax::Name;

    fn instruction(keyword: &str, args: Vec<Option<ExprRef>>) -> Stmt {
        Stmt::Instruction(Instruction {
            keyword: keyword.to_owned(),
            args,
            ..Default::default()
        })
    }

    fn name(value: &str) -> ExprRef {
        Rc::new(Expr::Name(Name {
            name: value.to_owned(),
            ..Default::default()
        }))
    }

    #[test]
    fn assignment_heads_match_python_including_latin1_and_nested_names() {
        let profile = crate::profiles::get("cr6").unwrap();
        let program = Rc::new(Program::new("test.CR6", Vec::new()));
        for (text, expected) in [
            ("X = 1", true),
            ("A(1) += 2", true),
            ("Xé = 1", true),
            (" A(é) = 1", true),
            ("A((1)) = 2", true),
            ("A((missing)) = 2", false),
            ("X == 1", true),
            ("X = Len(S)", true),
        ] {
            assert_eq!(
                _assignment_head_with(text, &program, profile),
                expected,
                "{text}"
            );
        }
    }

    #[test]
    fn table_references_require_closed_table_and_name_expression() {
        let table = instruction("DataTable", vec![Some(name("MiXeD"))]);
        let call = instruction("CallTable", vec![Some(name("mixed"))]);
        let accept = instruction("AcceptDataRecords", vec![None, None, Some(name("mixed"))]);
        let mut state = TableState::default();
        _track_tables_with(&table, "DATATABLE", &mut state);
        assert_eq!(state.open.as_deref(), Some("MIXED"));
        assert!(!_calltable_known_with(&call, &state));
        _track_tables_with(&instruction("EndTable", Vec::new()), "ENDTABLE", &mut state);
        assert_eq!(state.open, None);
        assert_eq!(state.closed, ["MIXED"]);
        assert!(_calltable_known_with(&call, &state));
        assert!(_accept_known_with(&accept, &state));
    }

    #[test]
    fn scan_literal_zero_does_not_count_for_table_placement() {
        for (value, expected) in [
            (NumberValue::Int(0), false),
            (NumberValue::Float(0.0), false),
            (NumberValue::Int(1), true),
            (NumberValue::Int(-1), true),
        ] {
            let scan = instruction(
                "Scan",
                vec![Some(Rc::new(Expr::Number(Number {
                    value,
                    ..Default::default()
                })))],
            );
            assert_eq!(_scan_counts_for_table_placement(&scan), expected);
        }
    }
}
