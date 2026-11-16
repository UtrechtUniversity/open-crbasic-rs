//! Port of `checker/instr_data.py` (lines 1-759).

#![allow(dead_code, unused_variables, unused_imports)]

use std::cell::RefCell;
use std::collections::{BTreeSet, HashMap};
use std::rc::Rc;
use std::sync::LazyLock;

use crate::checker::context;
use crate::checker::expressions::part1::{_undefined_names, _variable_refs};
use crate::checker::expressions::part2::_arg_text;
use crate::checker::var_types::Typer;
use crate::consteval::{self, Env};
use crate::diagnostics::{DiagRef, Diagnostic, Diagnostics, ShowFile};
use crate::profiles::Profile;
use crate::syntax::{
    ConstValue, Expr, Instruction, OrderedMap, Program, ProgramRef, Stmt, StmtRef, rc_id,
};
use crate::tdf::TdfTable;
use crate::{pyre, pystr};

/// Port of checker/instr_data.py::TABLE_ONLY_INSTRUCTIONS
pub static TABLE_ONLY_INSTRUCTIONS: LazyLock<OrderedMap<&'static str, &'static str>> =
    LazyLock::new(|| {
        let mut map = OrderedMap::new();
        for (k, v) in [
            ("CARDOUT", "CardOut"),
            ("MQTTPUBLISHTABLE", "MQTTPublishTable"),
            ("GOESTABLE", "GoesTable"),
            ("GOESFIELD", "GoesTable"),
            ("TABLEFILE", "TableFile"),
            ("TOTALIZE", "Totalize"),
            ("AVERAGE", "Average"),
            ("MAXIMUM", "Max"),
            ("MINIMUM", "Min"),
            ("SAMPLEMAXMIN", "SampleMaxMin"),
            ("STDDEV", "StdDev"),
            ("MOMENT", "Moment"),
            ("SAMPLE", "Sample"),
            ("HISTOGRAM", "Histogram"),
            ("HISTOGRAM4D", "Histogram4D"),
            ("COVARIANCE", "Covariance"),
            ("RAINFLOW", "RainFlow"),
            ("LEVELCROSSING", "LevelCrossing"),
            ("WINDVECTOR", "WindVector"),
            ("MEDIAN", "Median"),
            ("SAMPLEFIELDCAL", "SampleFieldCal"),
            ("FILLSTOP", "FillStop"),
            ("DATATIME", "DataTime"),
            ("OPENINTERVAL", "OpenInterval"),
            ("DATAINTERVAL", "DataInterval"),
            ("FFT", "FFT"),
            ("DATAEVENT", "DataEvent"),
            ("TABLEHIDE", "TableHide"),
            ("ETSZ", "Et"),
            ("ETCLEARSKY", "Et"),
        ] {
            map.insert(k, v);
        }
        map
    });

/// Port of checker/instr_data.py::_FIELD_PRODUCING_INSTRUCTIONS
pub static _FIELD_PRODUCING_INSTRUCTIONS: LazyLock<BTreeSet<&'static str>> = LazyLock::new(|| {
    [
        "SAMPLE",
        "AVERAGE",
        "TOTALIZE",
        "STDDEV",
        "MAXIMUM",
        "MINIMUM",
        "MEDIAN",
        "MOMENT",
        "SAMPLEMAXMIN",
        "COVARIANCE",
        "WINDVECTOR",
        "HISTOGRAM",
        "HISTOGRAM4D",
        "RAINFLOW",
        "LEVELCROSSING",
        "SAMPLEFIELDCAL",
        "MQTTPUBLISHTABLE",
        "FFT",
        "ETSZ",
        "ETCLEARSKY",
    ]
    .into_iter()
    .collect()
});

/// Port of checker/instr_data.py::_table_name
pub fn _table_name(stmt: &Instruction) -> Option<String> {
    match stmt.args.first()?.as_deref()? {
        Expr::Name(name) => Some(name.name.clone()),
        _ => None,
    }
}

/// Port of checker/instr_data.py::calltable_targets
pub fn calltable_targets(program: &ProgramRef) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    for stmt in &program.statements {
        if let Stmt::Instruction(inst) = stmt.as_ref()
            && pystr::upper(&inst.keyword) == "CALLTABLE"
            && let Some(name) = _table_name(inst)
            && !name.is_empty()
        {
            names.insert(pystr::upper(&name));
        }
    }
    for (_stmt, name, spec) in _unknown_table_references(program, None) {
        if spec.2 {
            names.insert(pystr::upper(&name));
        }
    }
    names
}

/// Port of checker/instr_data.py::unknown_calltables
pub fn unknown_calltables(program: &Program) -> Vec<(StmtRef, String)> {
    let mut closed: BTreeSet<String> = BTreeSet::new();
    let mut pending: Option<String> = None;
    let mut unknown = Vec::new();
    let names: HashMap<usize, String> = _calltable_names(program)
        .into_iter()
        .map(|(stmt, name)| (rc_id(&stmt), name))
        .collect();
    for stmt in _with_inline(&program.statements) {
        let Stmt::Instruction(inst) = stmt.as_ref() else {
            continue;
        };
        let kw = pystr::upper(&inst.keyword);
        if kw == "DATATABLE" {
            let mut name = _table_name(inst).map(|n| _const_table_name(&n, program));
            if name.as_deref().is_none_or(str::is_empty)
                && let Some(Some(arg0)) = inst.args.first()
                && let Expr::String(s) = arg0.as_ref()
            {
                name = Some(s.value.clone());
            }
            if let Some(n) = name
                && !n.is_empty()
            {
                pending = Some(pystr::upper(&n));
            }
        } else if kw == "ENDTABLE" {
            if let Some(p) = pending.take() {
                closed.insert(p);
            }
        } else if let Some(target_name) = names.get(&rc_id(&stmt))
            && !closed.contains(&pystr::upper(target_name))
        {
            unknown.push((stmt.clone(), target_name.clone()));
        }
    }
    unknown
}

/// Port of checker/instr_data.py::_with_inline
pub fn _with_inline(stmts: &[StmtRef]) -> Vec<StmtRef> {
    let mut out = Vec::new();
    for stmt in stmts {
        out.push(stmt.clone());
        if let Stmt::If(if_stmt) = stmt.as_ref() {
            if let Some(inline) = &if_stmt.inline {
                out.extend(_with_inline(inline));
            }
            if let Some(inline_else) = &if_stmt.inline_else {
                out.extend(_with_inline(inline_else));
            }
        }
    }
    out
}

/// Port of checker/instr_data.py::_const_table_name
pub fn _const_table_name(name: &str, program: &Program) -> String {
    let value = program
        .consts
        .get(&pystr::upper(name))
        .or_else(|| program.consts.get(name));
    match value {
        Some(ConstValue::Str(s)) if !s.is_empty() => s.clone(),
        _ => name.to_owned(),
    }
}

/// Port of checker/instr_data.py::_calltable_names
pub fn _calltable_names(program: &Program) -> Vec<(StmtRef, String)> {
    let mut out = Vec::new();
    for stmt in _with_inline(&program.statements) {
        let Stmt::Instruction(inst) = stmt.as_ref() else {
            continue;
        };
        if pystr::upper(&inst.keyword) != "CALLTABLE" {
            continue;
        }
        let target = inst.args.first().and_then(Option::as_deref);
        let name = match target {
            Some(Expr::Name(n)) => _const_table_name(&n.name, program),
            Some(Expr::String(s)) => s.value.clone(),
            Some(Expr::Binary(_)) => {
                let Some(Some((start, end))) = inst.arg_spans.first().copied() else {
                    continue;
                };
                let mut s = pystr::strip(pystr::slice(&inst.base.text, Some(start), Some(end)));
                while s.starts_with('(') && s.ends_with(')') {
                    s = pystr::strip(pystr::slice(s, Some(1), Some(-1)));
                }
                s.to_owned()
            }
            _ => continue,
        };
        out.push((stmt.clone(), name));
    }
    out
}

/// Port of checker/instr_data.py::TABLE_REFERENCES
pub static TABLE_REFERENCES: LazyLock<OrderedMap<&'static str, (usize, &'static str, bool)>> =
    LazyLock::new(|| {
        let mut map = OrderedMap::new();
        for (k, v) in [
            ("GETRECORD", (1, "Could not find DataTable {}.", true)),
            ("RESETTABLE", (0, "Could not find DataTable {}.", true)),
            ("FILEMARK", (0, "Could not find FileMark table {}.", true)),
            ("SENDDATA", (3, "Cannot find DataTable {}.", false)),
            ("SENDTABLEDEF", (3, "Cannot find DataTable {}.", false)),
        ] {
            map.insert(k, v);
        }
        map
    });

static _SPECIAL_TABLES: &[&str] = &["STATUS", "PUBLIC", "SETTINGS", "DATATABLEINFO", "ENDTABLE"];

/// Port of checker/instr_data.py::unknown_table_references
pub fn unknown_table_references(
    program: &ProgramRef,
    profile: Option<&'static Profile>,
) -> Vec<(StmtRef, String, bool)> {
    _unknown_table_references(program, profile)
        .into_iter()
        .map(|(stmt, name, spec)| (stmt, spec.1.replacen("{}", &name, 1), spec.2))
        .collect()
}

/// Port of checker/instr_data.py::_unknown_table_references
pub fn _unknown_table_references(
    program: &ProgramRef,
    profile: Option<&'static Profile>,
) -> Vec<(StmtRef, String, (usize, String, bool))> {
    let const_tables: BTreeSet<String> = _with_inline(&program.statements)
        .into_iter()
        .filter_map(|st| match st.as_ref() {
            Stmt::Instruction(inst) if pystr::upper(&inst.keyword) == "CONSTTABLE" => {
                match inst.args.first()?.as_deref()? {
                    Expr::Name(n) => Some(pystr::upper(&n.name)),
                    _ => None,
                }
            }
            _ => None,
        })
        .collect();
    let mut closed: BTreeSet<String> = BTreeSet::new();
    let mut pending: Option<String> = None;
    let mut out = Vec::new();
    for stmt in _with_inline(&program.statements) {
        let Stmt::Instruction(inst) = stmt.as_ref() else {
            continue;
        };
        let kw = pystr::upper(&inst.keyword);
        if kw == "DATATABLE" {
            pending = _table_name(inst)
                .filter(|n| !n.is_empty())
                .map(|n| pystr::upper(&n));
            continue;
        }
        if kw == "ENDTABLE" {
            if let Some(p) = pending.take() {
                closed.insert(p);
            }
            continue;
        }
        let Some(&(arg_idx, tmpl, ends)) = TABLE_REFERENCES.get(kw.as_str()) else {
            continue;
        };
        let Some(Some(arg)) = inst.args.get(arg_idx) else {
            continue;
        };
        let name = if matches!(kw.as_str(), "SENDDATA" | "SENDTABLEDEF") {
            pystr::strip(&_arg_text(inst, i64::try_from(arg_idx).unwrap_or(0))).to_owned()
        } else {
            match arg.as_ref() {
                Expr::Name(n) => n.name.clone(),
                Expr::String(s) => s.value.clone(),
                _ => continue,
            }
        };
        if kw == "GETRECORD"
            && let Some(prof) = profile
            && closed.contains(&pystr::upper(&name))
            && let Some(fields) = _getrecord_short(inst, &name, program, prof)
        {
            out.push((
                stmt.clone(),
                name,
                (
                    arg_idx,
                    format!("Destination array not large enough for the {fields} fields in a record\n{tmpl}"),
                    ends,
                ),
            ));
            continue;
        }
        let upper = pystr::upper(&name);
        if name.is_empty()
            || closed.contains(&upper)
            || const_tables.contains(&upper)
            || _SPECIAL_TABLES.contains(&upper.as_str())
        {
            continue;
        }
        out.push((stmt.clone(), name, (arg_idx, tmpl.to_owned(), ends)));
    }
    out
}

/// Port of checker/instr_data.py::_getrecord_short
pub fn _getrecord_short(
    stmt: &Instruction,
    table: &str,
    program: &ProgramRef,
    profile: &'static Profile,
) -> Option<i64> {
    let dest = stmt.args.first()?.as_deref()?;
    if !matches!(dest, Expr::Name(_) | Expr::Index(_)) {
        return None;
    }
    let typer = Typer::new(program, None, None);
    let var = typer.variable(Some(dest))?;
    let ty = typer.types.get(&var).map(String::as_str)?;
    if !matches!(ty, "FLOAT" | "LONG" | "BOOLEAN" | "DOUBLE") {
        return None;
    }
    let built = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        crate::tables::build_tables(program, profile, &program.consts).0
    }))
    .ok()?;
    let record = built
        .iter()
        .find(|t| pystr::upper(&t.name) == pystr::upper(table))?;
    let count: i64 = record
        .fields
        .iter()
        .map(|f| {
            if f.r#type == 11 {
                if f.subdims.is_empty() {
                    1
                } else {
                    f.subdims[..f.subdims.len() - 1].iter().product()
                }
            } else {
                f.dimension
            }
        })
        .sum();
    let env = Env::with_consts(profile, Rc::new(RefCell::new(program.consts.clone())));
    let ctx = context::of(program, Some(profile), Some(env));
    let dims = ctx.declared_all_dims().get(&var)?;
    let total: i64 = dims.iter().product();
    if total < count { Some(count) } else { None }
}

/// Port of checker/instr_data.py::check_unclosed_calltable
pub fn check_unclosed_calltable(
    program: &ProgramRef,
    diags: &mut Diagnostics,
    profile: Option<&'static Profile>,
) {
    let refs = unknown_table_references(program, profile);
    let first = unknown_calltables(program).into_iter().next();
    let ending: Vec<(StmtRef, String)> = refs
        .iter()
        .filter(|(_stmt, _msg, ends)| *ends)
        .map(|(stmt, msg, _ends)| (stmt.clone(), msg.clone()))
        .collect();
    let mut stopper: Option<(StmtRef, String)> = None;
    if let Some(end0) = ending.first()
        && first
            .as_ref()
            .is_none_or(|f| (end0.0.file(), end0.0.line()) <= (f.0.file(), f.0.line()))
    {
        stopper = Some(end0.clone());
    } else if let Some(f) = first {
        stopper = Some((f.0, format!("Could not find DataTable {}.", f.1)));
    }
    for (stmt, message, ends) in refs {
        if ends || stopper.as_ref().is_some_and(|s| stmt.line() > s.0.line()) {
            continue;
        }
        let at = diags
            .items
            .iter()
            .position(|d| {
                d.line == stmt.line() && d.file == stmt.file() && d.kind == "before_beginprog"
            })
            .unwrap_or(diags.items.len());
        diags.items.insert(
            at,
            Rc::new(Diagnostic::new(
                "error",
                stmt.line(),
                &message,
                stmt.file(),
                ShowFile::Bool(true),
                0,
                "",
                "",
            )),
        );
    }
    if let Some((stop_stmt, stop_msg)) = stopper {
        for message in stop_msg.split('\n') {
            diags.error_basic(stop_stmt.line(), message, stop_stmt.file());
        }
    }
}

static _FIELD_METADATA: &[&str] = &["FIELDNAMES", "FIELDCLASSIFY", "FIELDORIGIN"];
static _FIELDNAMES_OWNERS: LazyLock<BTreeSet<&'static str>> = LazyLock::new(|| {
    _FIELD_PRODUCING_INSTRUCTIONS
        .iter()
        .copied()
        .filter(|kw| {
            !matches!(
                *kw,
                "SAMPLEFIELDCAL" | "ETSZ" | "ETCLEARSKY" | "MQTTPUBLISHTABLE"
            )
        })
        .collect()
});
static _FIRST_WORD: LazyLock<pyre::Regex> =
    LazyLock::new(|| pyre::Regex::new(r"\s*([A-Za-z_]\w*)", 0));

/// Port of checker/instr_data.py::fieldnames_owners
pub fn fieldnames_owners(program: &Program, keyword: &str) -> OrderedMap<usize, Option<StmtRef>> {
    let mut owners: OrderedMap<usize, Option<StmtRef>> = OrderedMap::new();
    let mut owner: Option<StmtRef> = None;
    let mut in_table = false;
    for st in &program.statements {
        let kw = match st.as_ref() {
            Stmt::Instruction(inst) => pystr::upper(&inst.keyword),
            Stmt::Unparsed(u) => {
                let Some(m) = _FIRST_WORD.match_(&u.base.text) else {
                    continue;
                };
                let Some(g1) = m.group(1) else {
                    continue;
                };
                pystr::upper(g1)
            }
            _ => continue,
        };
        if matches!(kw.as_str(), "DATATABLE" | "ENDTABLE") {
            in_table = kw == "DATATABLE";
        }
        if kw == keyword {
            owners.insert(rc_id(st), owner.clone());
        } else if !_FIELD_METADATA.contains(&kw.as_str()) {
            owner = if _FIELDNAMES_OWNERS.contains(kw.as_str()) && in_table {
                Some(st.clone())
            } else {
                None
            };
        }
    }
    owners
}

static _CLASSIFY_INCOMPLETE: LazyLock<pyre::Regex> = LazyLock::new(|| {
    pyre::Regex::new(
        r"\s*(?:&H[0-9A-F]*|[0-9]+(?:\.[0-9]+)?)[ \t]+[0-9]",
        pyre::IGNORECASE,
    )
});
static _CLASSIFY_INVALID_HEX: LazyLock<pyre::Regex> = LazyLock::new(|| {
    pyre::Regex::new(
        r"\s*&H[0-9A-F]*(?:[G-Z]\w*(?:[ \t]+\d+)?|[ \t]+(?:[A-Z]\w*|&H[0-9A-F]+))\s*$",
        pyre::IGNORECASE,
    )
});

/// Port of checker/instr_data.py::check_fieldclassify
pub fn check_fieldclassify(
    program: &Program,
    profile: &Profile,
    diags: &mut Diagnostics,
    env: &Env,
) {
    if !profile.instructions().contains("FIELDCLASSIFY") {
        return;
    }
    let owners = fieldnames_owners(program, "FIELDCLASSIFY");
    if owners.is_empty() {
        return;
    }
    let mut entries: OrderedMap<usize, StmtRef> = OrderedMap::new();
    for st in &program.statements {
        let Stmt::Instruction(inst) = st.as_ref() else {
            continue;
        };
        if pystr::upper(&inst.keyword) == "ENDTABLE" {
            for entry_st in entries.values() {
                let Stmt::Instruction(entry) = entry_st.as_ref() else {
                    continue;
                };
                if entry.args.len() != 1 || entry.args[0].is_none() {
                    continue;
                }
                let Ok(value) = consteval::value_of(entry.args[0].as_deref(), env) else {
                    continue;
                };
                let ConstValue::Str(value) = value else {
                    continue;
                };
                let first = value.split_once(',').map_or(value.as_str(), |(a, _)| a);
                if value.is_empty() {
                    diags.error_basic(
                        st.line(),
                        "Invalid, or out of place expression: .",
                        st.file(),
                    );
                } else if _CLASSIFY_INCOMPLETE.match_(first).is_some() {
                    diags.error_basic(
                        st.line(),
                        &format!("Incomplete expression: {first}."),
                        st.file(),
                    );
                } else if _CLASSIFY_INVALID_HEX.match_(first).is_some() {
                    diags.error_basic(
                        st.line(),
                        &format!("Invalid symbol in expression: {first}."),
                        st.file(),
                    );
                }
            }
            entries = OrderedMap::new();
        } else if let Some(Some(owner)) = owners.get(&rc_id(st)) {
            entries.insert(rc_id(owner), st.clone());
        }
    }
}

/// Port of checker/instr_data.py::move_fieldnames_symbol_errors
pub fn move_fieldnames_symbol_errors(program: &Program, diags: &mut Diagnostics) {
    let owners = fieldnames_owners(program, "FIELDNAMES");
    let mut next_at: HashMap<usize, usize> = HashMap::new();
    for st in &program.statements {
        let Some(owner_opt) = owners.get(&rc_id(st)) else {
            continue;
        };
        let mine: Vec<DiagRef> = diags
            .items
            .iter()
            .filter(|d| {
                d.phase.starts_with("tier_b")
                    && (d.file.as_str(), d.line) == (st.file(), st.line())
                    && d.kind == "invalid_symbol"
            })
            .cloned()
            .collect();
        if mine.is_empty() {
            continue;
        }
        diags
            .items
            .retain(|d| !mine.iter().any(|m| Rc::ptr_eq(d, m)));
        let Some(owner) = owner_opt else {
            continue;
        };
        let moved: Vec<DiagRef> = mine
            .into_iter()
            .map(|d| {
                Rc::new(d.replace(|c| {
                    c.line = owner.line();
                    owner.file().clone_into(&mut c.file);
                }))
            })
            .collect();
        let pair: [DiagRef; 2] = [
            Rc::new(Diagnostic::new(
                "error",
                owner.line(),
                "FieldNames cannot be a variable.",
                owner.file(),
                ShowFile::Bool(false),
                0,
                "fieldnames_var",
                "tier_b_generic",
            )),
            Rc::new(Diagnostic::new(
                "error",
                owner.line(),
                "FieldNames canot be NULL",
                owner.file(),
                ShowFile::Bool(false),
                0,
                "fieldnames_null",
                "tier_b_generic",
            )),
        ];
        let oid = rc_id(owner);
        let at = next_at.get(&oid).copied().unwrap_or_else(|| {
            diags
                .items
                .iter()
                .position(|d| {
                    d.phase.starts_with("tier_b")
                        && (d.file.as_str(), d.line) == (owner.file(), owner.line())
                        && d.kind == "invalid_symbol"
                })
                .unwrap_or(diags.items.len())
        });
        let inserted = moved.len() + pair.len();
        let insertion = at.min(diags.items.len());
        diags
            .items
            .splice(insertion..insertion, moved.into_iter().chain(pair));
        next_at.insert(oid, at + inserted);
    }
}

/// Port of checker/instr_data.py::check_fieldnames
pub fn check_fieldnames(
    program: &ProgramRef,
    profile: &'static Profile,
    diags: &mut Diagnostics,
    env: &Env,
) {
    let ctx = context::of(program, Some(profile), Some(env.clone()));
    let tables = ctx.scope_tables(true, false);
    let owners = fieldnames_owners(program, "FIELDNAMES");
    let named: BTreeSet<usize> = owners
        .values()
        .filter_map(|o| o.as_ref().map(rc_id))
        .collect();
    let table_names: BTreeSet<String> = program
        .statements
        .iter()
        .filter_map(|st| match st.as_ref() {
            Stmt::Instruction(inst) if pystr::upper(&inst.keyword) == "DATATABLE" => {
                super::part3::_declared_table_name(inst).map(|n| pystr::upper(&n))
            }
            _ => None,
        })
        .collect();
    let reserved: &[&str] = if profile.single_pass {
        &["TABLESIZE"]
    } else {
        &["TABLESIZE", "BYTESPERRECORD"]
    };
    let mut known = None;
    let mut in_table = false;
    for st in &program.statements {
        let Stmt::Instruction(inst) = st.as_ref() else {
            continue;
        };
        let kw = pystr::upper(&inst.keyword);
        in_table = kw == "DATATABLE" || (in_table && kw != "ENDTABLE");
        if in_table
            && matches!(kw.as_str(), "SAMPLE" | "AVERAGE" | "MAXIMUM" | "TOTALIZE")
            && inst.args.len() > 1
            && !named.contains(&rc_id(st))
            && let Some(Some(source)) = inst.args.get(1)
            && let Expr::Member(m) = source.as_ref()
            && let Some(base) = m.base.as_deref()
            && let Expr::Name(base_name) = base
        {
            let base_upper = pystr::upper(&base_name.name);
            let member_upper = pystr::upper(&m.member);
            if (table_names.contains(&base_upper) || base_upper == "STATUS")
                && reserved.contains(&member_upper.as_str())
            {
                diags.error_basic(
                    st.line(),
                    &format!("Must use FieldNames with reserved field .{member_upper}."),
                    st.file(),
                );
            }
        }
        if kw != "FIELDNAMES" {
            continue;
        }
        let Some(Some(owner)) = owners.get(&rc_id(st)) else {
            continue;
        };
        if inst.args.len() != 1 || inst.args[0].is_none() {
            continue;
        }
        let line = owner.line();
        let file = owner.file();
        let arg = inst.args[0].as_deref();
        match consteval::value_of(arg, env) {
            Ok(ConstValue::Str(s)) => {
                if s.is_empty() {
                    diags.error_basic(line, "FieldNames canot be NULL", file);
                }
            }
            Ok(_) => {
                diags.error_basic(line, "FieldNames cannot be a variable.", file);
            }
            Err(_) => {
                let st_tables = tables.for_statement(st);
                let kind = match arg {
                    Some(Expr::Name(n)) => st_tables.types.get(&pystr::upper(&n.name)),
                    Some(Expr::Index(ix)) => st_tables.types.get(&pystr::upper(&ix.name)),
                    _ => None,
                };
                if kind.is_none() {
                    let var_set: BTreeSet<String> = st_tables.types.keys().cloned().collect();
                    if _variable_refs(arg, &var_set).is_empty() {
                        continue;
                    }
                    let k = *known.get_or_insert_with(|| ctx.known_names());
                    if !_undefined_names(arg, &crate::checker::expressions::KnownView::from(k))
                        .is_empty()
                    {
                        continue;
                    }
                }
                diags.error_basic(line, "FieldNames cannot be a variable.", file);
                if kind.map(String::as_str) == Some("STRING") {
                    diags.error_basic(line, "FieldNames canot be NULL", file);
                }
            }
        }
    }
}

/// Port of checker/instr_data.py::check_table_structure
pub fn check_table_structure(
    program: &Program,
    profile: &'static Profile,
    diags: &mut Diagnostics,
) -> Option<(String, i64)> {
    const TABLE_NAME_LIMIT: i64 = 20;
    let env = Env::with_consts(profile, Rc::new(RefCell::new(program.consts.clone())));
    let mut in_table = false;
    let mut table_name = String::new();
    let mut has_field = false;
    let mut goes_seen = false;
    let mut goes_orphans: usize = 0;
    for stmt in &program.statements {
        let inst = match stmt.as_ref() {
            Stmt::Instruction(inst) => inst,
            Stmt::Unparsed(_) => {
                has_field = has_field || in_table;
                continue;
            }
            _ => continue,
        };
        let kw = pystr::upper(&inst.keyword);
        if kw == "DATATABLE" {
            in_table = true;
            has_field = false;
            goes_orphans = 0;
            goes_seen = goes_seen && profile.single_pass;
            let raw = super::part3::_declared_table_name(inst).unwrap_or_default();
            pystr::slice(&raw, None, Some(TABLE_NAME_LIMIT)).clone_into(&mut table_name);
            continue;
        }
        if kw == "ENDTABLE" {
            if !in_table {
                diags.error_basic(
                    inst.base.node.line,
                    "EndTable without DataTable.",
                    &inst.base.node.file,
                );
                return Some((inst.base.node.file.clone(), inst.base.node.line));
            }
            if !has_field && !table_name.is_empty() {
                diags.error_basic(
                    inst.base.node.line,
                    &format!("Table {table_name} declared with no output fields."),
                    &inst.base.node.file,
                );
                return Some((inst.base.node.file.clone(), inst.base.node.line));
            }
            for _ in 0..goes_orphans {
                diags.error_basic(
                    inst.base.node.line,
                    "GoesField declared without a GoesTable declaration.",
                    &inst.base.node.file,
                );
            }
            in_table = false;
            continue;
        }
        if in_table {
            if _FIELD_PRODUCING_INSTRUCTIONS.contains(kw.as_str())
                && !super::part3::_fieldless(inst, profile, &env)
            {
                has_field = true;
            } else if kw == "GOESTABLE" {
                goes_seen = true;
            } else if kw == "GOESFIELD"
                && profile.instructions().contains(kw.as_str())
                && !goes_seen
            {
                diags.error_basic(
                    inst.base.node.line,
                    "GoesTable not active.",
                    &inst.base.node.file,
                );
                goes_orphans += 1;
            }
            continue;
        }
        if let Some(&display) = TABLE_ONLY_INSTRUCTIONS.get(kw.as_str())
            && profile.instructions().contains(kw.as_str())
        {
            diags.error_basic(
                inst.base.node.line,
                &format!("{display} must be used inside a table declaration."),
                &inst.base.node.file,
            );
        }
    }
    None
}

static _CH400_FIELDS: &[&str] = &[
    "ChargeAVoltage",
    "ChargeACurrent",
    "ChargeBVoltage",
    "ChargeBCurrent",
    "InputChargeStatus",
    "InputCurrentLimit",
    "BattAVoltage",
    "BattACurrent",
    "BattAChargeStatus",
    "BattASetVoltage",
    "BattACurrentLimit",
    "BattADepthofDischarge",
    "BattBVoltage",
    "BattBCurrent",
    "BattBChargeStatus",
    "BattBSetVoltage",
    "BattBCurrentLimit",
    "BattBDepthofDischarge",
    "SystemVoltage",
    "SystemCurrent",
    "AlwaysOn12Voltage",
    "AlwaysOn12Current",
    "Switched12Voltage",
    "Switched12Current",
    "Switched24Voltage",
    "Switched24Current",
    "PowerSwitchState",
    "MaxBoardTemp",
    "MinBoardTemp",
    "PanelTemp",
    "BattAProbeTemp",
    "BattBProbeTemp",
];

const _ERROR_BUFFER_BYTES: usize = 4095;

/// Bookkeeping entry mirroring `TdfTable._dup_fields` items until `tdf::TdfTable` exposes them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DupFieldEntry {
    pub name: String,
    pub renamed: bool,
    pub rng: Option<(i64, i64)>,
    pub placeholder: bool,
    pub ch400: bool,
    pub dim: i64,
}

/// Port of checker/instr_data.py::_dup_entries
pub fn _dup_entries(
    table: &TdfTable,
    dup_fields: Option<&[DupFieldEntry]>,
) -> Vec<(String, bool, Option<(i64, i64)>, bool)> {
    let mut out = Vec::new();
    let Some(fields) = dup_fields else {
        for f in &table.fields {
            out.push((
                f.name.clone(),
                false,
                Some((f.begin_index, f.begin_index + f.dimension.max(1) - 1)),
                false,
            ));
        }
        return out;
    };
    for f in fields {
        if f.ch400 {
            for _ in 0..f.dim.max(1) {
                for &n in _CH400_FIELDS {
                    out.push((n.to_owned(), false, Some((1, 1)), false));
                }
            }
        } else {
            out.push((f.name.clone(), f.renamed, f.rng, f.placeholder));
        }
    }
    out
}

/// Port of checker/instr_data.py::check_duplicate_fields
pub fn check_duplicate_fields(
    program: &Program,
    tables: &[TdfTable],
    dup_fields_by_table: Option<&[Option<Vec<DupFieldEntry>>]>,
    diags: &mut Diagnostics,
) {
    if tables.is_empty() {
        return;
    }
    let mut user_tables: Vec<String> = Vec::new();
    for stmt in &program.statements {
        if let Stmt::Instruction(inst) = stmt.as_ref()
            && pystr::upper(&inst.keyword) == "DATATABLE"
            && let Some(mut name) = _table_name(inst)
        {
            name = _const_table_name(&name, program);
            let upper = pystr::upper(&name);
            if !name.is_empty() && !user_tables.contains(&upper) {
                user_tables.push(upper);
            }
        }
    }
    let by_upper: HashMap<String, usize> = tables
        .iter()
        .enumerate()
        .map(|(i, t)| (pystr::upper(&t.name), i))
        .collect();
    let mut remaining = _ERROR_BUFFER_BYTES;
    for key in user_tables {
        let Some(&idx) = by_upper.get(&key) else {
            continue;
        };
        let table = &tables[idx];
        let dup_fields = dup_fields_by_table
            .and_then(|all| all.get(idx))
            .and_then(|o| o.as_deref());
        let mut seen: HashMap<String, Vec<(bool, Option<(i64, i64)>)>> = HashMap::new();
        let shortest = format!("Duplicate field name x in DataTable {}.", table.name)
            .chars()
            .count()
            + 2;
        for (name, renamed, rng, placeholder) in _dup_entries(table, dup_fields) {
            if remaining < shortest {
                break;
            }
            if placeholder {
                continue;
            }
            let earlier = seen.entry(pystr::upper(&name)).or_default();
            let text = format!("Duplicate field name {name} in DataTable {}.", table.name);
            let cost = text.chars().count() + 2;
            for &(e_renamed, e_rng) in earlier.iter() {
                let conflict = if renamed && e_renamed {
                    true
                } else if let (Some(r), Some(er)) = (rng, e_rng) {
                    r.0 <= er.1 && er.0 <= r.1
                } else {
                    false
                };
                if !conflict {
                    continue;
                }
                if cost <= remaining {
                    diags.error_basic(0, &text, "");
                    remaining -= cost;
                }
            }
            earlier.push((renamed, rng));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_text;
    use crate::profiles::{PROFILES, Profile};
    use crate::tdf::TdfField;

    fn cr6() -> &'static Profile {
        &PROFILES[0]
    }

    fn parse_prog(src: &str, profile: &'static Profile) -> ProgramRef {
        parse_text(src, "test.cr6", None, Some(profile)).0
    }

    #[test]
    fn calltable_targets_and_unknown_calltables() {
        let profile = cr6();
        let prog = parse_prog(
            "Const TN = \"T2\"\n\
             DataTable(T1,1,-1)\n\
               Sample(1,X,FP2)\n\
             EndTable\n\
             BeginProg\n\
               Scan(1,Sec,0,0)\n\
                 CallTable T1\n\
                 CallTable TN\n\
                 CallTable Missing\n\
               NextScan\n\
             EndProg\n",
            profile,
        );
        let targets = calltable_targets(&prog);
        assert!(targets.contains("T1"));
        assert!(targets.contains("TN"));
        assert!(targets.contains("MISSING"));

        let unknown = unknown_calltables(&prog);
        let names: Vec<&str> = unknown.iter().map(|(_, n)| n.as_str()).collect();
        assert_eq!(names, vec!["T2", "Missing"]);
    }

    #[test]
    fn check_unclosed_calltable_reports_first_stopper_and_senddata() {
        let profile = cr6();
        let prog = parse_prog(
            "BeginProg\n\
               Scan(1,Sec,0,0)\n\
                 SendData(1,0,0,NoSuch)\n\
                 CallTable Missing\n\
                 SendData(1,0,0,AfterStop)\n\
               NextScan\n\
             EndProg\n",
            profile,
        );
        let mut diags = Diagnostics::new();
        check_unclosed_calltable(&prog, &mut diags, Some(profile));
        let msgs: Vec<(i64, &str)> = diags
            .items
            .iter()
            .map(|d| (d.line, d.message.as_str()))
            .collect();
        assert_eq!(
            msgs,
            vec![
                (3, "Cannot find DataTable NoSuch."),
                (4, "Could not find DataTable Missing."),
            ]
        );
    }

    #[test]
    fn fieldnames_owners_and_move_symbol_errors() {
        let profile = cr6();
        let prog = parse_prog(
            "DataTable(T1,1,-1)\n\
               Sample(1,X,FP2)\n\
               FieldClassify(\"1\")\n\
               FieldNames(\"A\")\n\
             EndTable\n",
            profile,
        );
        let owners = fieldnames_owners(&prog, "FIELDNAMES");
        assert_eq!(owners.len(), 1);
        let owner = owners.values().next().unwrap().as_ref().unwrap();
        assert_eq!(owner.line(), 2);

        let mut diags = Diagnostics::new();
        diags.error(
            4,
            "Invalid symbol in expression: BAD.",
            "test.cr6",
            ShowFile::Bool(true),
            0,
            "invalid_symbol",
            "tier_b_expr",
        );
        move_fieldnames_symbol_errors(&prog, &mut diags);
        let msgs: Vec<(i64, &str, &str)> = diags
            .items
            .iter()
            .map(|d| (d.line, d.message.as_str(), d.kind.as_str()))
            .collect();
        assert_eq!(
            msgs,
            vec![
                (2, "Invalid symbol in expression: BAD.", "invalid_symbol"),
                (2, "FieldNames cannot be a variable.", "fieldnames_var"),
                (2, "FieldNames canot be NULL", "fieldnames_null"),
            ]
        );
    }

    #[test]
    fn repeated_fieldnames_clamps_python_slice_insertion() {
        let prog = parse_prog(
            "DataTable(T,1,-1)\nSample(1,X,FP2)\nFieldNames(A)\nFieldNames(B)\nEndTable\n",
            cr6(),
        );
        let mut diags = Diagnostics::new();
        for (line, name) in [(3, "A"), (4, "B")] {
            diags.error(
                line,
                &format!("Invalid symbol in expression: {name}."),
                "test.cr6",
                ShowFile::Bool(true),
                0,
                "invalid_symbol",
                "tier_b_expr",
            );
        }
        move_fieldnames_symbol_errors(&prog, &mut diags);
        assert_eq!(diags.items.len(), 6);
        assert!(diags.items.iter().all(|d| d.line == 2));
        assert_eq!(diags.items[0].message, "Invalid symbol in expression: A.");
        assert_eq!(diags.items[3].message, "Invalid symbol in expression: B.");
    }

    #[test]
    fn check_fieldclassify_reports_incomplete_and_invalid_hex() {
        let profile = cr6();
        let prog = parse_prog(
            "DataTable(T1,1,-1)\n\
               Sample(1,X,FP2)\n\
               FieldClassify(\"\")\n\
               Sample(1,Y,FP2)\n\
               FieldClassify(\"1 2\")\n\
               Sample(1,Z,FP2)\n\
               FieldClassify(\"&HG\")\n\
             EndTable\n",
            profile,
        );
        let env = Env::new(profile);
        let mut diags = Diagnostics::new();
        check_fieldclassify(&prog, profile, &mut diags, &env);
        let msgs: Vec<&str> = diags.items.iter().map(|d| d.message.as_str()).collect();
        assert_eq!(
            msgs,
            vec![
                "Invalid, or out of place expression: .",
                "Incomplete expression: 1 2.",
                "Invalid symbol in expression: &HG.",
            ]
        );
    }

    #[test]
    fn check_fieldnames_reports_var_null_and_reserved() {
        let profile = cr6();
        let prog = parse_prog(
            "Public S As String\n\
             DataTable(T1,1,-1)\n\
               Sample(1,T1.TableSize,FP2)\n\
               Sample(1,S,String)\n\
               FieldNames(S)\n\
             EndTable\n",
            profile,
        );
        let env = Env::with_consts(profile, Rc::new(RefCell::new(prog.consts.clone())));
        let mut diags = Diagnostics::new();
        check_fieldnames(&prog, profile, &mut diags, &env);
        let msgs: Vec<(i64, &str)> = diags
            .items
            .iter()
            .map(|d| (d.line, d.message.as_str()))
            .collect();
        assert_eq!(
            msgs,
            vec![
                (3, "Must use FieldNames with reserved field .TABLESIZE."),
                (4, "FieldNames cannot be a variable."),
                (4, "FieldNames canot be NULL"),
            ]
        );
    }

    #[test]
    fn check_table_structure_outside_and_orphan_endtable() {
        let profile = cr6();
        let prog = parse_prog(
            "BeginProg\n\
               Sample(1,X,FP2)\n\
               EndTable\n\
               Average(1,X,FP2,False)\n\
             EndProg\n",
            profile,
        );
        let mut diags = Diagnostics::new();
        let stop = check_table_structure(&prog, profile, &mut diags);
        assert_eq!(stop, Some(("test.cr6".to_owned(), 3)));
        let msgs: Vec<(i64, &str)> = diags
            .items
            .iter()
            .map(|d| (d.line, d.message.as_str()))
            .collect();
        assert_eq!(
            msgs,
            vec![
                (2, "Sample must be used inside a table declaration."),
                (3, "EndTable without DataTable."),
            ]
        );
    }

    #[test]
    fn check_duplicate_fields_overlap_and_ch400() {
        let profile = cr6();
        let prog = parse_prog("DataTable(T1,1,-1)\nEndTable\n", profile);
        let tables = vec![TdfTable {
            name: "T1".to_owned(),
            fields: vec![
                TdfField {
                    name: "A".to_owned(),
                    begin_index: 1,
                    dimension: 2,
                    ..Default::default()
                },
                TdfField {
                    name: "a".to_owned(),
                    begin_index: 2,
                    dimension: 1,
                    ..Default::default()
                },
            ],
            ..Default::default()
        }];
        let mut diags = Diagnostics::new();
        check_duplicate_fields(&prog, &tables, None, &mut diags);
        assert_eq!(diags.items.len(), 1);
        assert_eq!(
            diags.items[0].message,
            "Duplicate field name a in DataTable T1."
        );

        let entries = _dup_entries(
            &tables[0],
            Some(&[DupFieldEntry {
                name: String::new(),
                renamed: false,
                rng: None,
                placeholder: false,
                ch400: true,
                dim: 1,
            }]),
        );
        assert_eq!(entries.len(), 32);
        assert_eq!(entries[0].0, "ChargeAVoltage");
    }
}
