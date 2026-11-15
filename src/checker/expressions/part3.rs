//! Port of `checker/expressions.py` (lines 1571-2352).

#![allow(
    dead_code,
    unused_variables,
    unused_imports,
    clippy::used_underscore_items,
    reason = "port keeps Python's leading-underscore function names and pending wiring to sibling parts"
)]

use std::collections::{BTreeSet, HashSet};
use std::rc::Rc;
use std::sync::LazyLock;

use crate::checker::context;
use crate::consteval::Env;
use crate::diagnostics::{DiagRef, Diagnostic, Diagnostics, ShowFile};
use crate::profiles::Profile;
use crate::syntax::{
    COMPOUND_ASSIGNMENT_OPS, ConstDecl, Expr, ExprRef, Member, OrderedMap, Program, ProgramRef,
    Stmt, StmtRef, expr_id, stmt_id,
};
use crate::{pyre, pystr};

use super::part2::{DimsState, KnownNames, TablesWithFields};

const TABLE_FIELDS: &[&str] = &["TIMESTAMP", "RECORD", "RECNUM"];
const CR3XX_SETTINGS_TABLE_PROPERTIES: &[&str] = &[
    "RECORD",
    "TIMESTAMP",
    "OUTPUT",
    "TABLESIZE",
    "TABLEFULL",
    "EVENTCOUNT",
    "EVENTEND",
];
const CONST_TYPES: &[&str] = &["LONG", "FLOAT", "DOUBLE", "STRING", "BOOLEAN"];

static DO_LOOP_COND_RE: LazyLock<pyre::Regex> =
    LazyLock::new(|| pyre::Regex::new(r"(?i)\b(?:While|Until)\b\s*(.*)$", 0));

fn is_whole_expr_echo_logger(profile: &Profile) -> bool {
    matches!(profile.name, "cr6" | "cr1000x")
}

/// Port of `checker/expressions.py::check_table_members`.
pub fn check_table_members(_program: &Program, _diags: &mut Diagnostics) {
    todo!("checker/expressions.py::check_table_members (omitted: profile: untyped)")
}

/// Port of `checker/expressions.py::check_table_members`.
pub fn check_table_members_with(
    program: &ProgramRef,
    profile: &'static Profile,
    diags: &mut Diagnostics,
) {
    let ctx = context::of(program, Some(profile), None);
    let const_tables = ctx.const_table_fields();
    let mut data_table_names: BTreeSet<String> = BTreeSet::new();
    for stmt in &program.statements {
        if let Stmt::Instruction(inst) = &**stmt
            && pystr::upper(&inst.keyword) == "DATATABLE"
            && let Some(Some(arg0)) = inst.args.first()
            && let Expr::Name(n) = &**arg0
        {
            data_table_names.insert(pystr::upper(&n.name));
        }
    }
    if const_tables.is_empty() && data_table_names.is_empty() {
        return;
    }
    let data_tables = pending_wiring::build_tables_with_fields(
        &data_table_names,
        pending_wiring::_data_table_fields(program, profile),
    );
    let mut renamed: BTreeSet<String> = BTreeSet::new();
    let mut table: Option<String> = None;
    for st in &program.statements {
        let Stmt::Instruction(inst) = &**st else {
            continue;
        };
        let kw = pystr::upper(&inst.keyword);
        if kw == "DATATABLE" {
            table = match inst.args.first() {
                Some(Some(arg0)) => match &**arg0 {
                    Expr::Name(n) => Some(pystr::upper(&n.name)),
                    _ => None,
                },
                _ => None,
            };
        } else if kw == "ENDTABLE" {
            table = None;
        } else if kw == "FIELDNAMES"
            && let Some(ref tbl) = table
            && inst.args.iter().flatten().any(|a| match &**a {
                Expr::String(s) => pystr::split(&s.value, ",", None)
                    .into_iter()
                    .any(|f| pystr::upper(pystr::strip(f)) == "BYTESPERRECORD"),
                _ => false,
            })
        {
            renamed.insert(tbl.clone());
        }
    }
    let absent: BTreeSet<String> = if profile.single_pass {
        data_table_names
            .difference(&renamed)
            .map(|t| format!("{t}.BYTESPERRECORD"))
            .collect()
    } else {
        BTreeSet::new()
    };
    let known = if profile.single_pass {
        Some(ctx.known_names())
    } else {
        None
    };
    for stmt in &program.statements {
        let exprs: Vec<Option<ExprRef>> = match &**stmt {
            Stmt::Assignment(a) => {
                if let Some(k) = known
                    && !pending_wiring::_undefined_names(a.target.as_deref(), k).is_empty()
                {
                    continue;
                }
                vec![a.target.clone(), a.value.clone()]
            }
            Stmt::If(i) => vec![i.cond.clone()],
            Stmt::Instruction(i) => i.args.clone(),
            Stmt::Call(c) => c.args.clone(),
            _ => Vec::new(),
        };
        for expr in exprs {
            if let Some((tbl, member, index)) = pending_wiring::_table_member_problem(
                expr.as_deref(),
                const_tables,
                &data_tables,
                &absent,
            ) {
                diags.error_basic(
                    stmt.line(),
                    &format!("Could not find {tbl}.{member}{index}."),
                    stmt.file(),
                );
                break;
            }
        }
    }
}

/// Port of `checker/expressions.py::_dotted_members`.
pub fn _dotted_members(_expr: Option<&Expr>) {
    todo!("checker/expressions.py::_dotted_members")
}

/// Port of `checker/expressions.py::_dotted_members`.
pub fn _dotted_members_with(expr: Option<&Expr>) -> Vec<(String, Option<Member>)> {
    let Some(expr) = expr else {
        return Vec::new();
    };
    match expr {
        Expr::Name(n) => {
            if n.name.ends_with('.')
                && pystr::len(&n.name) > 1
                && (pystr::isalpha(pystr::slice(&n.name, Some(0), Some(1)))
                    || pystr::char_at(&n.name, 0) == Some('_'))
            {
                vec![(pystr::slice(&n.name, None, Some(-1)).to_owned(), None)]
            } else {
                Vec::new()
            }
        }
        Expr::Member(m) => {
            let mut out = _dotted_members_with(m.base.as_deref());
            if let Some(base) = m.base.as_deref()
                && let Expr::Name(n) = base
                && !n.name.ends_with('.')
            {
                out.push((n.name.clone(), Some(m.clone())));
            }
            if let Some(args) = &m.args {
                for a in args {
                    out.extend(_dotted_members_with(a.as_deref()));
                }
            }
            out
        }
        Expr::Index(ix) => ix
            .args
            .iter()
            .flat_map(|a| _dotted_members_with(a.as_deref()))
            .collect(),
        Expr::Unary(u) => _dotted_members_with(u.operand.as_deref()),
        Expr::Binary(b) => {
            let mut out = _dotted_members_with(b.left.as_deref());
            out.extend(_dotted_members_with(b.right.as_deref()));
            out
        }
        Expr::ArrayLiteral(_) | Expr::Juxtaposed(_) => expr
            .array_items()
            .into_iter()
            .flatten()
            .flat_map(|item| _dotted_members_with(item.as_deref()))
            .collect(),
        Expr::Number(_) | Expr::String(_) => Vec::new(),
    }
}

/// Port of `checker/expressions.py::_unknown_status_member`.
pub fn _unknown_status_member() -> bool {
    todo!("checker/expressions.py::_unknown_status_member (omitted: m: untyped, profile: untyped)")
}

/// Port of `checker/expressions.py::_unknown_status_member`.
pub fn _unknown_status_member_with(m: &Member, profile: &Profile) -> bool {
    let status_members = profile.status_members();
    !pystr::isdigit(pystr::slice(&m.member, Some(0), Some(1)))
        && !status_members.is_empty()
        && !status_members.contains(&pystr::upper(&m.member))
}

/// Port of `checker/expressions.py::_unknown_setting`.
pub fn _unknown_setting() -> bool {
    todo!("checker/expressions.py::_unknown_setting (omitted: m: untyped, profile: untyped)")
}

/// Port of `checker/expressions.py::_unknown_setting`.
pub fn _unknown_setting_with(m: &Member, profile: &Profile) -> bool {
    if pystr::isdigit(pystr::slice(&m.member, Some(0), Some(1))) {
        return false;
    }
    let upper = pystr::upper(&m.member);
    let known_settings: BTreeSet<String> = profile
        .settings_fields()
        .get("settings")
        .and_then(crate::profiles::Json::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|v| v.as_str().map(pystr::upper))
                .collect()
        })
        .unwrap_or_default();
    !known_settings.contains(&upper)
}

/// Port of `checker/expressions.py::_unknown_public_member`.
pub fn _unknown_public_member(_variables: &BTreeSet<String>) -> bool {
    todo!("checker/expressions.py::_unknown_public_member (omitted: m: untyped)")
}

/// Port of `checker/expressions.py::_unknown_public_member`.
pub fn _unknown_public_member_with(m: &Member, variables: &BTreeSet<String>) -> bool {
    if pystr::isdigit(pystr::slice(&m.member, Some(0), Some(1))) {
        return false;
    }
    let upper = pystr::upper(&m.member);
    !variables.contains(&upper) && !TABLE_FIELDS.contains(&upper.as_str())
}

/// Port of `checker/expressions.py::check_status_member_reads_late`.
pub fn check_status_member_reads_late(_program: &Program, _diags: &mut Diagnostics) {
    todo!("checker/expressions.py::check_status_member_reads_late (omitted: profile: untyped)")
}

/// Port of `checker/expressions.py::check_status_member_reads_late`.
pub fn check_status_member_reads_late_with(
    program: &ProgramRef,
    profile: &'static Profile,
    diags: &mut Diagnostics,
) {
    if !profile.single_pass {
        return;
    }
    let ctx = context::of(program, Some(profile), None);
    let variables: BTreeSet<String> = ctx
        .declared_variable_names()
        .iter()
        .filter(|n| !pending_wiring::is_reserved_predefined_name(n, profile))
        .cloned()
        .collect();
    for stmt in &program.statements {
        let (exprs, target): (Vec<Option<ExprRef>>, usize) = match &**stmt {
            Stmt::Assignment(a) => (
                vec![a.target.clone(), a.value.clone()],
                _dotted_members_with(a.target.as_deref()).len(),
            ),
            Stmt::If(i) => (vec![i.cond.clone()], 0),
            Stmt::Instruction(i) => (i.args.clone(), 0),
            Stmt::Call(c) => (c.args.clone(), 0),
            _ => continue,
        };
        let all_members = exprs
            .iter()
            .flat_map(|e| _dotted_members_with(e.as_deref()));
        for (k, (base, m)) in all_members.enumerate() {
            let Some(m) = m else {
                continue;
            };
            if k < target {
                continue;
            }
            let upper = pystr::upper(&base);
            if (upper == "STATUS" && _unknown_status_member_with(&m, profile))
                || (upper == "PUBLIC" && _unknown_public_member_with(&m, &variables))
            {
                let idx_text = pending_wiring::_member_index_text(m.args.as_deref());
                diags.error_basic(
                    stmt.line(),
                    &format!(
                        "Could not find {base}.{member}{idx_text}.",
                        member = m.member
                    ),
                    stmt.file(),
                );
            }
        }
    }
}

/// Port of `checker/expressions.py::check_undefined_dotted_bases`.
pub fn check_undefined_dotted_bases(_program: &Program, _diags: &mut Diagnostics) {
    todo!("checker/expressions.py::check_undefined_dotted_bases (omitted: profile: untyped)")
}

/// Port of `checker/expressions.py::check_undefined_dotted_bases`.
pub fn check_undefined_dotted_bases_with(
    program: &ProgramRef,
    profile: &'static Profile,
    diags: &mut Diagnostics,
) {
    if profile.single_pass {
        return;
    }
    let ctx = context::of(program, Some(profile), None);
    let known = ctx.known_names();
    for stmt in &program.statements {
        if let Stmt::Instruction(i) = &**stmt
            && pystr::upper(&i.keyword) == "ALIAS"
        {
            continue;
        }
        let exprs = pending_wiring::_stmt_exprs(stmt);
        let bases: Vec<String> = exprs
            .iter()
            .flat_map(|e| _dotted_members_with(e.as_deref()))
            .filter_map(|(b, m)| {
                (m.is_some() && !pending_wiring::known_contains(known, &pystr::upper(&b)))
                    .then_some(b)
            })
            .collect();
        if bases.is_empty() {
            continue;
        }
        let base_uppers: BTreeSet<String> = bases.iter().map(|b| pystr::upper(b)).collect();
        let others: Vec<String> = exprs
            .iter()
            .flat_map(|e| pending_wiring::_undefined_names_walk(e.as_deref(), known))
            .filter(|n| !base_uppers.contains(&pystr::upper(n)))
            .collect();
        let line = stmt.line();
        let file = stmt.file();
        let generic = ["invalid_symbol", "invalid_expression"];
        let here: Vec<DiagRef> = diags
            .items
            .iter()
            .filter(|d| d.phase.starts_with("tier_b") && d.line == line && d.file == file)
            .cloned()
            .collect();
        if here
            .iter()
            .any(|d| d.severity == "error" && !generic.contains(&d.kind.as_str()))
        {
            continue;
        }
        if others.is_empty() {
            diags.items.retain(|d| {
                !(d.phase.starts_with("tier_b")
                    && d.line == line
                    && d.file == file
                    && generic.contains(&d.kind.as_str()))
            });
        }
        let mut new_diags = vec![Rc::new(Diagnostic::new(
            "error",
            line,
            &format!("Could not find {}.", bases[0]),
            file,
            ShowFile::Bool(true),
            0,
            "could_not_find",
            "",
        ))];
        if let Stmt::Assignment(a) = &**stmt
            && !_dotted_members_with(a.target.as_deref()).is_empty()
        {
            new_diags.push(Rc::new(Diagnostic::basic(
                "error",
                line,
                &format!("Invalid assignment in expression: {}.", stmt.text()),
                file,
            )));
        }
        let at = diags
            .items
            .iter()
            .position(|d| d.phase.starts_with("tier_b") && here.iter().any(|h| Rc::ptr_eq(d, h)))
            .unwrap_or(diags.items.len());
        diags.items.splice(at..at, new_diags);
    }
}

/// Port of `checker/expressions.py::check_digit_members`.
pub fn check_digit_members(_program: &Program, _diags: &mut Diagnostics) {
    todo!("checker/expressions.py::check_digit_members (omitted: profile: untyped)")
}

/// Port of `checker/expressions.py::check_digit_members`.
pub fn check_digit_members_with(
    program: &ProgramRef,
    profile: &'static Profile,
    diags: &mut Diagnostics,
) {
    let ctx = context::of(program, Some(profile), None);
    let mut variables: BTreeSet<String> = ctx
        .declared_variable_names()
        .iter()
        .filter(|n| !pending_wiring::is_reserved_predefined_name(n, profile))
        .cloned()
        .collect();
    let struct_types = pending_wiring::structure_members(program);
    let kinds = pending_wiring::variable_kinds(program, &struct_types);
    variables.retain(|n| !kinds.get(n).is_some_and(|k| k.starts_with("STRUCT")));
    for st in &program.statements {
        if let Stmt::SubDef(sd) = &**st {
            for p in &sd.params {
                if let Some(ref ty) = p.type_
                    && struct_types.contains_key(&pystr::upper(ty))
                {
                    variables.remove(&pystr::upper(&p.name));
                }
            }
        }
    }
    for n in program.consts.keys() {
        variables.insert(pystr::upper(n));
    }
    for stmt in &program.statements {
        match &**stmt {
            Stmt::Instruction(inst)
                if pystr::upper(&inst.keyword) == "ALIAS" && inst.args.len() == 2 =>
            {
                if let Some(Some(arg1)) = inst.args.get(1)
                    && let Expr::Name(n) = &**arg1
                    && pystr::upper(&n.name) != "PUBLIC"
                {
                    variables.insert(pystr::upper(&n.name));
                }
            }
            Stmt::SubDef(sd) if pystr::upper(&sd.keyword) == "FUNCTION" => {
                variables.insert(pystr::upper(&sd.name));
            }
            _ => {}
        }
    }
    let mut data_tables: BTreeSet<String> = BTreeSet::new();
    data_tables.insert("STATUS".to_owned());
    for stmt in &program.statements {
        if let Stmt::Instruction(inst) = &**stmt
            && pystr::upper(&inst.keyword) == "DATATABLE"
            && let Some(Some(arg0)) = inst.args.first()
            && let Expr::Name(n) = &**arg0
        {
            data_tables.insert(pystr::upper(&n.name));
        }
    }
    let known = ctx.known_names();
    for stmt in &program.statements {
        let exprs: Vec<Option<ExprRef>> = match &**stmt {
            Stmt::Assignment(a) => vec![a.target.clone(), a.value.clone()],
            Stmt::If(i) => vec![i.cond.clone()],
            Stmt::For(f) => vec![
                f.var.clone(),
                f.start.clone(),
                f.end.clone(),
                f.step.clone(),
            ],
            Stmt::Instruction(i) => i.args.clone(),
            Stmt::Call(c) => c.args.clone(),
            _ => continue,
        };
        let members: Vec<(String, Option<Member>)> = exprs
            .iter()
            .flat_map(|e| _dotted_members_with(e.as_deref()))
            .collect();
        if members.is_empty()
            || exprs
                .iter()
                .any(|e| !pending_wiring::_undefined_names(e.as_deref(), known).is_empty())
        {
            continue;
        }
        let mut lines: Vec<String> = Vec::new();
        let target = if let Stmt::Assignment(a) = &**stmt {
            _dotted_members_with(a.target.as_deref()).len()
        } else {
            0
        };
        for (k, (base, m)) in members.into_iter().enumerate() {
            if k == target && !lines.is_empty() {
                break;
            }
            let upper = pystr::upper(&base);
            let Some(m) = m else {
                if !data_tables.contains(&upper) {
                    lines.push(if profile.single_pass {
                        format!("Could not find DataTable {base}.")
                    } else {
                        format!("Could not find {base}.")
                    });
                }
                continue;
            };
            if upper == "SETTINGS"
                && _unknown_setting_with(&m, profile)
                && (!profile.single_pass
                    || (matches!(&**stmt, Stmt::Assignment(_) | Stmt::If(_))
                        && !CR3XX_SETTINGS_TABLE_PROPERTIES
                            .contains(&pystr::upper(&m.member).as_str())))
            {
                let idx = pending_wiring::_member_index_text(m.args.as_deref());
                lines.push(format!("Could not find Settings.{}{idx}.", m.member));
            } else if upper == "STATUS"
                && !profile.single_pass
                && _unknown_status_member_with(&m, profile)
            {
                let idx = pending_wiring::_member_index_text(m.args.as_deref());
                lines.push(format!("Could not find Status.{}{idx}.", m.member));
            } else if data_tables.contains(&upper) {
                if pystr::isdigit(pystr::slice(&m.member, Some(0), Some(1))) {
                    let idx = pending_wiring::_member_index_text(m.args.as_deref());
                    lines.push(format!("Could not find {base}.{}{idx}.", m.member));
                }
            } else if upper == "PUBLIC"
                && k >= target
                && !profile.single_pass
                && _unknown_public_member_with(&m, &variables)
            {
                let idx = pending_wiring::_member_index_text(m.args.as_deref());
                lines.push(format!("Could not find Public.{}{idx}.", m.member));
            } else if variables.contains(&upper) {
                lines.push(if profile.single_pass {
                    format!("Could not find DataTable {base}.")
                } else {
                    format!("Could not find {base}.")
                });
            }
        }
        if lines.is_empty() {
            continue;
        }
        for line in &lines {
            diags.error_basic(stmt.line(), line, stmt.file());
        }
        if let Stmt::Assignment(a) = &**stmt
            && _dotted_members_with(a.target.as_deref())
                .iter()
                .any(|(_, m)| m.is_some())
        {
            let sep = if profile.fallback_echo_newline && stmt.at_eol() {
                "\n"
            } else {
                ""
            };
            diags.error_basic(
                stmt.line(),
                &format!("Invalid assignment in expression: {}{sep}.", stmt.text()),
                stmt.file(),
            );
        }
    }
}

/// Port of `checker/expressions.py::check_const_family_args`.
pub fn check_const_family_args(_program: &Program, _diags: &mut Diagnostics) {
    todo!("checker/expressions.py::check_const_family_args (omitted: profile: untyped)")
}

/// Port of `checker/expressions.py::check_const_family_args`.
pub fn check_const_family_args_with(
    program: &ProgramRef,
    profile: &'static Profile,
    diags: &mut Diagnostics,
) {
    if !is_whole_expr_echo_logger(profile) {
        return;
    }
    let ctx = context::of(program, Some(profile), None);
    let known = ctx.known_names();
    let mut variable_names: BTreeSet<String> = ctx
        .declared_variable_names()
        .iter()
        .filter(|name| !pending_wiring::is_reserved_predefined_name(name, profile))
        .cloned()
        .collect();
    for name in super::part2::_alias_names(program) {
        if !profile.symbols().contains(&name) {
            variable_names.insert(name);
        }
    }
    let scopes = ctx.statement_scopes();
    for stmt in &program.statements {
        let Stmt::Instruction(inst) = &**stmt else {
            continue;
        };
        let Some(record) = profile.instructions().get(&pystr::upper(&inst.keyword)) else {
            continue;
        };
        if inst.args.iter().flatten().any(|a| {
            pending_wiring::_find_arity_problem(Some(a), profile, None).is_some()
                || pending_wiring::_find_bare_function_ref(Some(a), profile, &variable_names)
        }) {
            continue;
        }
        let positions = pending_wiring::_position_kinds(&record.params, &inst.keyword, profile);
        if !positions.is_empty() {
            let (params, later) = &scopes[&stmt_id(stmt)];
            let excluded: BTreeSet<String> = later
                .difference(params)
                .filter(|n| !variable_names.contains(*n))
                .cloned()
                .collect();
            let scoped_known = pending_wiring::known_without(known, &excluded);
            let scoped_vars: BTreeSet<String> = variable_names.union(params).cloned().collect();
            pending_wiring::_report_statement(
                inst,
                &positions,
                &scoped_known,
                &scoped_vars,
                profile,
                diags,
                "const",
                None,
                None,
            );
        }
    }
}

/// Port of `checker/expressions.py::_names_before_invalid_compound`.
pub fn _names_before_invalid_compound() -> Option<Vec<String>> {
    todo!(
        "checker/expressions.py::_names_before_invalid_compound (omitted: expr: untyped, known: untyped, read_only: untyped)"
    )
}

/// Port of `checker/expressions.py::_names_before_invalid_compound`.
pub fn _names_before_invalid_compound_with(
    expr: Option<&Expr>,
    known: &KnownNames,
    read_only: &BTreeSet<String>,
) -> Option<Vec<String>> {
    let expr = expr?;
    match expr {
        Expr::Binary(b) => {
            if let Some(names) =
                _names_before_invalid_compound_with(b.left.as_deref(), known, read_only)
            {
                return Some(names);
            }
            if COMPOUND_ASSIGNMENT_OPS.contains(&b.op.as_str()) {
                let target = b.left.as_deref();
                let invalid = match target {
                    Some(Expr::Name(n)) => read_only.contains(&pystr::upper(&n.name)),
                    Some(Expr::Index(ix)) => read_only.contains(&pystr::upper(&ix.name)),
                    Some(Expr::Member(_)) => false,
                    _ => true,
                };
                if invalid {
                    return Some(pending_wiring::_undefined_names_str(target, known));
                }
            }
            if let Some(names) =
                _names_before_invalid_compound_with(b.right.as_deref(), known, read_only)
            {
                let mut out = pending_wiring::_undefined_names_str(b.left.as_deref(), known);
                out.extend(names);
                return Some(out);
            }
            None
        }
        Expr::Unary(u) => {
            _names_before_invalid_compound_with(u.operand.as_deref(), known, read_only)
        }
        Expr::Index(ix) => {
            let children: Vec<Option<&Expr>> = ix.args.iter().map(|a| a.as_deref()).collect();
            _names_before_invalid_compound_children(&children, known, read_only)
        }
        Expr::Member(m) => {
            let children: Vec<Option<&Expr>> = m
                .args
                .as_deref()
                .unwrap_or(&[])
                .iter()
                .map(|a| a.as_deref())
                .collect();
            _names_before_invalid_compound_children(&children, known, read_only)
        }
        Expr::ArrayLiteral(_) | Expr::Juxtaposed(_) => {
            let children: Vec<Option<&Expr>> = expr
                .array_items()
                .map(|items| items.iter().map(|a| a.as_deref()).collect())
                .unwrap_or_default();
            _names_before_invalid_compound_children(&children, known, read_only)
        }
        Expr::Number(_) | Expr::String(_) | Expr::Name(_) => None,
    }
}

fn _names_before_invalid_compound_children(
    children: &[Option<&Expr>],
    known: &KnownNames,
    read_only: &BTreeSet<String>,
) -> Option<Vec<String>> {
    for (i, &child) in children.iter().enumerate() {
        if let Some(names) = _names_before_invalid_compound_with(child, known, read_only) {
            let mut out = Vec::new();
            for &before in &children[..i] {
                out.extend(pending_wiring::_undefined_names_str(before, known));
            }
            out.extend(names);
            return Some(out);
        }
    }
    None
}

/// Port of `checker/expressions.py::_report_invalid_compound`.
pub fn _report_invalid_compound() -> bool {
    todo!(
        "checker/expressions.py::_report_invalid_compound (omitted: stmt: untyped, expr: untyped, text: untyped, known: untyped, read_only: untyped, profile: untyped, diags: untyped, whole_statement: untyped)"
    )
}

/// Port of `checker/expressions.py::_report_invalid_compound`.
#[allow(clippy::too_many_arguments)]
pub fn _report_invalid_compound_with(
    stmt: &StmtRef,
    expr: Option<&Expr>,
    text: &str,
    known: &KnownNames,
    read_only: &BTreeSet<String>,
    profile: &Profile,
    diags: &mut Diagnostics,
    whole_statement: bool,
) -> bool {
    let Some(names) = _names_before_invalid_compound_with(expr, known, read_only) else {
        return false;
    };
    if !names.is_empty() && !profile.single_pass {
        diags.error(
            stmt.line(),
            &profile.unresolved_symbol_message.format(text),
            stmt.file(),
            ShowFile::Bool(true),
            stmt_id(stmt),
            "",
            "",
        );
        return true;
    }
    for name in names {
        diags.error_basic(
            stmt.line(),
            &profile.unresolved_symbol_message.format(&name),
            stmt.file(),
        );
    }
    let sep = if whole_statement && profile.fallback_echo_newline && stmt.at_eol() {
        "\n"
    } else {
        ""
    };
    diags.error(
        stmt.line(),
        &format!("Invalid assignment in expression: {text}{sep}."),
        stmt.file(),
        ShowFile::Bool(true),
        stmt_id(stmt),
        "",
        "",
    );
    true
}

/// Port of `checker/expressions.py::check`.
pub fn check(_program: &Program, _diags: &mut Diagnostics) {
    todo!(
        "checker/expressions.py::check (omitted: profile: untyped, env: consteval.Env, skip: frozenset)"
    )
}

/// Port of `checker/expressions.py::check`.
pub fn check_with<S: std::hash::BuildHasher>(
    program: &ProgramRef,
    profile: &'static Profile,
    diags: &mut Diagnostics,
    env: &Env,
    skip: &HashSet<usize, S>,
) {
    let ctx = context::of(program, Some(profile), Some(env.clone()));
    let known = ctx.known_names();
    let mut variable_names = ctx.declared_variable_names().clone();
    if is_whole_expr_echo_logger(profile) {
        for n in super::part2::_alias_names(program) {
            if !profile.symbols().contains(&n) {
                variable_names.insert(n);
            }
        }
    }
    let ds = ctx.dims_state();
    let dim_counts = ctx.dim_counts(true);
    let scopes = ctx.statement_scopes();
    let mut read_only: BTreeSet<String> = program
        .consts
        .keys()
        .cloned()
        .chain(profile.functions().iter().map(|(k, _)| k.to_owned()))
        .collect();
    for v in &variable_names {
        read_only.remove(v);
    }
    let mut first_variables: OrderedMap<String, (String, i64)> = OrderedMap::new();
    if profile.single_pass {
        for st in &program.statements {
            if let Stmt::Declaration(d) = &**st
                && matches!(d.kind.as_str(), "PUBLIC" | "DIM" | "STATIC")
            {
                for var in &d.vars {
                    let key = pystr::upper(&var.name);
                    if !first_variables.contains_key(&key) {
                        first_variables.insert(key, (st.file().to_owned(), st.line()));
                    }
                }
            }
        }
    }

    let visible_at = |stmt: &StmtRef| -> KnownNames {
        let (params, later) = &scopes[&stmt_id(stmt)];
        let mut excluded: BTreeSet<String> = later
            .difference(params)
            .filter(|n| !variable_names.contains(*n))
            .cloned()
            .collect();
        for (name, (file, line)) in first_variables.iter() {
            if file == stmt.file()
                && *line > stmt.line()
                && !params.contains(name)
                && !profile.symbols().contains(name)
                && !profile.functions().contains(name)
            {
                excluded.insert(name.clone());
            }
        }
        pending_wiring::known_without(known, &excluded)
    };

    let mut seen_avw200 = false;
    for stmt in &program.statements {
        let Stmt::Instruction(inst) = &**stmt else {
            continue;
        };
        if skip.contains(&stmt_id(stmt)) {
            continue;
        }
        let kw = pystr::upper(&inst.keyword);
        let Some(record) = profile.instructions().get(&kw) else {
            continue;
        };
        if kw == "SLOWSEQUENCE" && is_whole_expr_echo_logger(profile) {
            continue;
        }
        let mut positions = pending_wiring::_position_kinds(&record.params, &inst.keyword, profile);
        if kw == "SDMSIO4" && pending_wiring::_sdmsio4_count(inst, env) == Some(0) {
            positions.remove(&0);
        }
        if kw == "AVW200" && is_whole_expr_echo_logger(profile) {
            if seen_avw200 {
                positions.remove(&1);
                positions.remove(&2);
            }
            seen_avw200 = true;
        }
        if !positions.is_empty()
            && kw == "SERIALOPEN"
            && let Some(Some(arg0)) = inst.args.first()
            && is_whole_expr_echo_logger(profile)
            && !pending_wiring::_undefined_names(Some(arg0), known).is_empty()
        {
            continue;
        }
        if !positions.is_empty() {
            let reported = diags.items.len();
            let (here, here_variables): (KnownNames, BTreeSet<String>) = if profile.single_pass {
                let (params, later) = &scopes[&stmt_id(stmt)];
                let excluded: BTreeSet<String> = later
                    .difference(params)
                    .filter(|n| !variable_names.contains(*n))
                    .cloned()
                    .collect();
                let mut h = pending_wiring::known_without(known, &excluded);
                if kw == "REALTIME" {
                    h = visible_at(stmt);
                }
                (h, variable_names.union(params).cloned().collect())
            } else {
                (pending_wiring::known_clone(known), variable_names.clone())
            };
            pending_wiring::_report_statement(
                inst,
                &positions,
                &here,
                &here_variables,
                profile,
                diags,
                "general",
                Some(dim_counts),
                Some(ds),
            );
            if kw == "MATRIX"
                && is_whole_expr_echo_logger(profile)
                && diags.items.len() > reported
                && diags.items[reported..]
                    .iter()
                    .all(|d| d.message.starts_with("Invalid symbol in expression: "))
                && let Some(rewritten) =
                    pending_wiring::matrix_undefined_operands(inst, program, env, known, profile)
            {
                diags.items.truncate(reported);
                for message in rewritten {
                    diags.error_basic(stmt.line(), &message, stmt.file());
                }
            }
        }
    }

    let mut suppressed_inline: HashSet<usize> = HashSet::new();
    for stmt in &program.statements {
        let Stmt::If(if_stmt) = &**stmt else {
            continue;
        };
        let here = if profile.single_pass {
            visible_at(stmt)
        } else {
            pending_wiring::known_clone(known)
        };
        let names = pending_wiring::_undefined_names(if_stmt.cond.as_deref(), &here);
        let text = super::part2::_if_condition_text(stmt)
            .or_else(|| super::part2::_if_condition_text_no_then(stmt));
        let mut ro_scoped = read_only.clone();
        for p in &scopes[&stmt_id(stmt)].0 {
            ro_scoped.remove(p);
        }
        if let Some(ref t) = text
            && _report_invalid_compound_with(
                stmt,
                if_stmt.cond.as_deref(),
                t,
                &here,
                &ro_scoped,
                profile,
                diags,
                false,
            )
        {
            if !profile.single_pass {
                if let Some(inline) = &if_stmt.inline {
                    for body in inline {
                        suppressed_inline.insert(stmt_id(body));
                    }
                }
                if let Some(inline_else) = &if_stmt.inline_else {
                    for body in inline_else {
                        suppressed_inline.insert(stmt_id(body));
                    }
                }
            }
            continue;
        }
        if let Some(ref t) = text
            && names.is_empty()
            && pending_wiring::_over_indexed(
                if_stmt.cond.as_deref(),
                dim_counts,
                pending_wiring::_dim_limit(profile),
            )
        {
            diags.error_basic(
                stmt.line(),
                &pending_wiring::dimensions_message(t, profile, false, false),
                stmt.file(),
            );
            continue;
        }
        let Some(t) = text else {
            continue;
        };
        if names.is_empty() {
            continue;
        }
        let (names_out, dims) =
            pending_wiring::_names_and_dims(&here, Some(ds), stmt, &[], if_stmt.cond.as_deref());
        let before = diags.items.len();
        if is_whole_expr_echo_logger(profile) {
            diags.error_basic(
                stmt.line(),
                &format!("Invalid symbol in expression: {t}."),
                stmt.file(),
            );
        } else {
            for name in &names_out {
                diags.error_basic(
                    stmt.line(),
                    &pending_wiring::format_unresolved_maybe_dotted(profile, name),
                    stmt.file(),
                );
            }
            if dims {
                diags.error_basic(
                    stmt.line(),
                    &pending_wiring::dimensions_message(&t, profile, false, false),
                    stmt.file(),
                );
            }
        }
        if if_stmt.inline.is_some()
            && let Some(at) = diags.items[..before]
                .iter()
                .position(|d| d.line == stmt.line() && d.file == stmt.file())
        {
            let moved: Vec<DiagRef> = diags.items.drain(before..).collect();
            diags.items.splice(at..at, moved);
        }
    }

    let mut open_do: usize = 0;
    for stmt in &program.statements {
        let Stmt::DoLoop(dl) = &**stmt else {
            continue;
        };
        if pystr::upper(&dl.keyword) == "DO" {
            open_do += 1;
        } else if open_do > 0 {
            open_do -= 1;
        } else if is_whole_expr_echo_logger(profile) {
            continue;
        }
        if dl.cond.is_none() {
            continue;
        }
        let names = pending_wiring::_undefined_names(dl.cond.as_deref(), known);
        let cond_text = DO_LOOP_COND_RE
            .search(stmt.text())
            .and_then(|m| m.group(1).map(str::to_owned));
        let mut ro_scoped = read_only.clone();
        for p in &scopes[&stmt_id(stmt)].0 {
            ro_scoped.remove(p);
        }
        if let Some(ref ct) = cond_text
            && _report_invalid_compound_with(
                stmt,
                dl.cond.as_deref(),
                ct,
                known,
                &ro_scoped,
                profile,
                diags,
                false,
            )
        {
            continue;
        }
        if let Some(ref ct) = cond_text
            && names.is_empty()
            && pending_wiring::_over_indexed(
                dl.cond.as_deref(),
                dim_counts,
                pending_wiring::_dim_limit(profile),
            )
        {
            diags.error_basic(
                stmt.line(),
                &pending_wiring::dimensions_message(ct, profile, false, false),
                stmt.file(),
            );
            continue;
        }
        let Some(ct) = cond_text else {
            continue;
        };
        if names.is_empty() {
            continue;
        }
        if is_whole_expr_echo_logger(profile) {
            diags.error_basic(
                stmt.line(),
                &format!("Invalid symbol in expression: {ct}."),
                stmt.file(),
            );
        } else {
            for name in &names {
                diags.error_basic(
                    stmt.line(),
                    &pending_wiring::format_unresolved_maybe_dotted(profile, name),
                    stmt.file(),
                );
            }
        }
    }

    pending_wiring::_check_for_headers(program, profile, diags, known);
    pending_wiring::_check_elseif_and_case(program, profile, diags, known);
    pending_wiring::_check_function_statements(program, profile, diags, known);

    let struct_vars = super::part4::_struct_var_names(program);
    let functions: BTreeSet<String> = program
        .statements
        .iter()
        .filter_map(|st| match &**st {
            Stmt::SubDef(sd) if pystr::upper(&sd.keyword) == "FUNCTION" => {
                Some(pystr::upper(&sd.name))
            }
            _ => None,
        })
        .collect();
    for stmt in &program.statements {
        if let Stmt::Call(c) = &**stmt {
            if !c.explicit
                && functions.contains(&pystr::upper(&c.name))
                && c.tail_expr.is_none()
                && c.juxtaposed.is_none()
                && !c
                    .args
                    .iter()
                    .any(|a| !pending_wiring::_undefined_names(a.as_deref(), known).is_empty())
                && c.args.iter().any(|a| {
                    pending_wiring::_over_indexed(
                        a.as_deref(),
                        dim_counts,
                        pending_wiring::_dim_limit(profile),
                    )
                })
            {
                diags.error_basic(
                    stmt.line(),
                    &pending_wiring::dimensions_message(stmt.text(), profile, true, stmt.at_eol()),
                    stmt.file(),
                );
            }
            if let Some(ref tail) = c.tail_expr
                && pending_wiring::_undefined_names(Some(tail), known).is_empty()
                && pending_wiring::_over_indexed(
                    Some(tail),
                    dim_counts,
                    pending_wiring::_dim_limit(profile),
                )
            {
                diags.error_basic(
                    stmt.line(),
                    &pending_wiring::dimensions_message(stmt.text(), profile, true, stmt.at_eol()),
                    stmt.file(),
                );
            }
        }
        let Stmt::Assignment(a) = &**stmt else {
            continue;
        };
        if suppressed_inline.contains(&stmt_id(stmt)) {
            continue;
        }
        if !pending_wiring::_undefined_names(a.target.as_deref(), known).is_empty() {
            continue;
        }
        if profile.single_pass {
            let visible = visible_at(stmt);
            if !pending_wiring::known_eq(&visible, known) {
                let exprs_pair = [a.target.as_deref(), a.value.as_deref()];
                let (early, dims) =
                    pending_wiring::_names_and_dims_multi(&visible, None, stmt, &[], &exprs_pair);
                if !early.is_empty()
                    && early.iter().any(|name| {
                        let up = pystr::upper(name.as_str());
                        pending_wiring::known_contains(known, &up)
                            && !pending_wiring::known_contains(&visible, &up)
                    })
                {
                    for name in &early {
                        diags.error_basic(
                            stmt.line(),
                            &pending_wiring::format_unresolved_maybe_dotted(profile, name),
                            stmt.file(),
                        );
                    }
                    if dims {
                        diags.error_basic(
                            stmt.line(),
                            &pending_wiring::dimensions_message(
                                stmt.text(),
                                profile,
                                true,
                                stmt.at_eol(),
                            ),
                            stmt.file(),
                        );
                    }
                    continue;
                }
            }
        }
        if super::part4::_multi_indexed_struct(a.target.as_deref(), &struct_vars)
            || super::part4::_multi_indexed_struct(a.value.as_deref(), &struct_vars)
        {
            continue;
        }
        if profile.single_pass
            && pending_wiring::_function_target_message(a, profile, &BTreeSet::new()).is_some()
        {
            continue;
        }
        let mut ro_scoped = read_only.clone();
        for p in &scopes[&stmt_id(stmt)].0 {
            ro_scoped.remove(p);
        }
        if _report_invalid_compound_with(
            stmt,
            a.value.as_deref(),
            stmt.text(),
            known,
            &ro_scoped,
            profile,
            diags,
            true,
        ) {
            continue;
        }
        let names = pending_wiring::_undefined_names(a.value.as_deref(), known);
        let whole_echo = is_whole_expr_echo_logger(profile);
        if (profile.single_pass || !names.is_empty())
            && let Some(target_expr) = a.target.as_deref()
            && let Expr::Member(m) = target_expr
            && let Some(base_expr) = m.base.as_deref()
            && let Expr::Name(n) = base_expr
            && pystr::upper(&n.name) == "STATUS"
            && _unknown_status_member_with(m, profile)
        {
            let base = if profile.single_pass {
                n.name.as_str()
            } else {
                "Status"
            };
            let idx = pending_wiring::_member_index_text(m.args.as_deref());
            diags.error_basic(
                stmt.line(),
                &format!("Could not find {base}.{member}{idx}.", member = m.member),
                stmt.file(),
            );
            if profile.single_pass {
                for name in &names {
                    diags.error_basic(
                        stmt.line(),
                        &pending_wiring::format_unresolved_maybe_dotted(profile, name),
                        stmt.file(),
                    );
                }
                let sep = if stmt.at_eol() { "\n" } else { "" };
                diags.error_basic(
                    stmt.line(),
                    &format!("Invalid assignment in expression: {}{sep}.", stmt.text()),
                    stmt.file(),
                );
                continue;
            }
        }
        if pending_wiring::_over_indexed(
            a.target.as_deref(),
            dim_counts,
            if whole_echo { Some(3) } else { None },
        ) {
            diags.error(
                stmt.line(),
                &pending_wiring::structure_out_of_place(profile, stmt.text(), stmt.at_eol()),
                stmt.file(),
                ShowFile::Cr3xx,
                0,
                "",
                "",
            );
            continue;
        }
        if names.is_empty() {
            if pending_wiring::_over_indexed(
                a.value.as_deref(),
                dim_counts,
                pending_wiring::_dim_limit(profile),
            ) {
                diags.error_basic(
                    stmt.line(),
                    &pending_wiring::dimensions_message(stmt.text(), profile, true, stmt.at_eol()),
                    stmt.file(),
                );
            }
            continue;
        }
        if whole_echo {
            diags.error(
                stmt.line(),
                &profile.unresolved_symbol_message.format(stmt.text()),
                stmt.file(),
                ShowFile::Bool(true),
                stmt_id(stmt),
                "",
                "",
            );
        } else {
            let prior = [a.target.as_deref()];
            let (mut resolved_names, dims) =
                pending_wiring::_names_and_dims(known, Some(ds), stmt, &prior, a.value.as_deref());
            if !dims && pending_wiring::_over_indexed(a.value.as_deref(), dim_counts, None) {
                resolved_names = pending_wiring::_names_through_over_index(
                    a.value.as_deref(),
                    known,
                    dim_counts,
                );
            }
            for name in &resolved_names {
                diags.error_basic(
                    stmt.line(),
                    &pending_wiring::format_unresolved_maybe_dotted(profile, name),
                    stmt.file(),
                );
            }
            if dims || pending_wiring::_over_indexed(a.value.as_deref(), dim_counts, None) {
                diags.error_basic(
                    stmt.line(),
                    &pending_wiring::dimensions_message(stmt.text(), profile, true, stmt.at_eol()),
                    stmt.file(),
                );
            }
        }
    }
}

/// Port of `checker/expressions.py::check_consts`.
pub fn check_consts(_program: &Program, _diags: &mut Diagnostics) {
    todo!("checker/expressions.py::check_consts (omitted: profile: untyped)")
}

/// Port of `checker/expressions.py::check_consts`.
pub fn check_consts_with(program: &ProgramRef, profile: &'static Profile, diags: &mut Diagnostics) {
    let ctx = context::of(program, Some(profile), None);
    let known = ctx.known_names();
    let unready = ctx.not_yet_declared();
    let read_only: BTreeSet<String> = program
        .consts
        .keys()
        .cloned()
        .chain(profile.functions().iter().map(|(k, _)| k.to_owned()))
        .collect();
    for stmt in &program.statements {
        let Stmt::ConstDecl(c) = &**stmt else {
            continue;
        };
        if let Some(ref ty) = c.type_
            && !CONST_TYPES.contains(&pystr::upper(ty).as_str())
        {
            continue;
        }
        let empty = BTreeSet::new();
        let unready_set = unready.get(&stmt_id(stmt)).unwrap_or(&empty);
        let visible = pending_wiring::known_without(known, unready_set);
        if let Some((start, end)) = c.value_span {
            let val_text = pystr::slice(stmt.text(), Some(start), Some(end));
            if _report_invalid_compound_with(
                stmt,
                c.value.as_deref(),
                val_text,
                &visible,
                &read_only,
                profile,
                diags,
                false,
            ) {
                continue;
            }
        }
        let names = pending_wiring::_undefined_names_str(c.value.as_deref(), &visible);
        let problem = if names.is_empty() {
            None
        } else {
            pending_wiring::_find_arity_problem(c.value.as_deref(), profile, Some(&visible))
        };
        let names_first = profile.single_pass
            && matches!(problem.as_deref(), Some("constint" | "network_constint"));
        if names.is_empty()
            || (problem.is_some() && problem.as_deref() != Some("deferred") && !names_first)
        {
            continue;
        }
        _report_const_names_with(c, &names, profile, diags);
    }
}

/// Port of `checker/expressions.py::_report_const_names`.
pub fn _report_const_names(_stmt: &ConstDecl, _names: Vec<String>, _diags: &mut Diagnostics) {
    todo!("checker/expressions.py::_report_const_names (omitted: profile: untyped)")
}

/// Port of `checker/expressions.py::_report_const_names`.
pub fn _report_const_names_with(
    stmt: &ConstDecl,
    names: &[String],
    profile: &Profile,
    diags: &mut Diagnostics,
) {
    if is_whole_expr_echo_logger(profile) {
        let text = if let Some((start, end)) = stmt.value_span {
            pystr::slice(&stmt.base.text, Some(start), Some(end)).to_owned()
        } else if let Some(Expr::Name(n)) = stmt.value.as_deref() {
            n.name.clone()
        } else {
            names[0].clone()
        };
        diags.error_basic(
            stmt.base.node.line,
            &profile.unresolved_symbol_message.format(&text),
            &stmt.base.node.file,
        );
    } else {
        for name in names {
            diags.error_basic(
                stmt.base.node.line,
                &profile.unresolved_symbol_message.format(name),
                &stmt.base.node.file,
            );
        }
    }
}

/// Typed bridge to sibling functions whose `port` stub signatures omit parameters or return types.
/// `@integrator-2`: replace each call here with the merged sibling signature.
mod pending_wiring {
    use std::collections::BTreeSet;

    use crate::checker::context::DimCounts;
    use crate::consteval::Env;
    use crate::diagnostics::Diagnostics;
    use crate::profiles::Profile;
    use crate::syntax::{
        Assignment, Expr, ExprRef, Instruction, OrderedMap, Program, Stmt, StmtRef,
    };

    use super::super::part2::{DimsState, KnownNames, TablesWithFields};

    #[derive(Debug, Clone)]
    pub enum MaybeDottedName {
        Plain(String),
        Dotted(crate::profiles::DottedName),
    }

    impl MaybeDottedName {
        pub fn as_str(&self) -> &str {
            match self {
                Self::Plain(s) => s,
                Self::Dotted(d) => &d.0,
            }
        }
    }

    pub fn format_unresolved_maybe_dotted(profile: &Profile, name: &MaybeDottedName) -> String {
        match name {
            MaybeDottedName::Plain(s) => profile.unresolved_symbol_message.format(s),
            MaybeDottedName::Dotted(d) => profile.unresolved_symbol_message.format_dotted(d),
        }
    }

    pub fn build_tables_with_fields(
        _names: &BTreeSet<String>,
        _fields: OrderedMap<String, BTreeSet<String>>,
    ) -> TablesWithFields {
        todo!("wire TablesWithFields once part2 lands")
    }

    pub fn _data_table_fields(
        _program: &Program,
        _profile: &Profile,
    ) -> OrderedMap<String, BTreeSet<String>> {
        todo!("wire part2::_data_table_fields_with (omits profile & return type on port)")
    }

    pub fn _table_member_problem(
        _expr: Option<&Expr>,
        _const_tables: &OrderedMap<String, BTreeSet<String>>,
        _data_tables: &TablesWithFields,
        _absent: &BTreeSet<String>,
    ) -> Option<(String, String, String)> {
        todo!("wire part2::_table_member_problem_with (omits const_tables & return type on port)")
    }

    pub fn _undefined_names(_expr: Option<&Expr>, _known: &KnownNames) -> Vec<MaybeDottedName> {
        todo!(
            "wire part1::_undefined_names (takes &BTreeSet<String> on port instead of KnownNames)"
        )
    }

    pub fn _undefined_names_str(_expr: Option<&Expr>, _known: &KnownNames) -> Vec<String> {
        todo!(
            "wire part1::_undefined_names (takes &BTreeSet<String> on port instead of KnownNames)"
        )
    }

    pub fn _undefined_names_walk(_expr: Option<&Expr>, _known: &KnownNames) -> Vec<String> {
        todo!(
            "wire part1::_undefined_names_walk (takes &BTreeSet<String> on port instead of KnownNames)"
        )
    }

    pub fn _member_index_text(_args: Option<&[Option<ExprRef>]>) -> String {
        todo!("wire part2::_member_index_text_with (omits args on port)")
    }

    pub fn _stmt_exprs(_stmt: &Stmt) -> Vec<Option<ExprRef>> {
        todo!("wire part2::_stmt_exprs (returns () on port)")
    }

    pub fn is_reserved_predefined_name(_name: &str, _profile: &Profile) -> bool {
        todo!("wire symbols::is_reserved_predefined_name_with (omits profile on port)")
    }

    pub fn structure_members(_program: &Program) -> OrderedMap<String, BTreeSet<String>> {
        todo!("wire structs::_structure_members (returns () on port)")
    }

    pub fn variable_kinds(
        _program: &Program,
        _structs: &OrderedMap<String, BTreeSet<String>>,
    ) -> OrderedMap<String, String> {
        todo!("wire structs::_variable_kinds (omits structs & return type on port)")
    }

    pub fn known_contains(_known: &KnownNames, _name: &str) -> bool {
        todo!("wire KnownNames::contains once part2 lands")
    }

    pub fn known_without(_known: &KnownNames, _names: &BTreeSet<String>) -> KnownNames {
        todo!("wire KnownNames::without_with (omits names on port)")
    }

    pub fn known_clone(_known: &KnownNames) -> KnownNames {
        todo!("wire KnownNames: Clone once part2 lands")
    }

    pub fn known_eq(_a: &KnownNames, _b: &KnownNames) -> bool {
        todo!("wire KnownNames: PartialEq once part2 lands")
    }

    pub fn _find_arity_problem(
        _expr: Option<&Expr>,
        _profile: &Profile,
        _known: Option<&KnownNames>,
    ) -> Option<String> {
        todo!("wire part4::_find_arity_problem_with (omits profile on port)")
    }

    pub fn _find_bare_function_ref(
        _expr: Option<&Expr>,
        _profile: &Profile,
        _declared_variables: &BTreeSet<String>,
    ) -> bool {
        todo!("wire part4::_find_bare_function_ref_with (omits profile on port)")
    }

    pub fn _position_kinds(
        _params: &[i64],
        _keyword: &str,
        _profile: &Profile,
    ) -> OrderedMap<usize, String> {
        todo!("wire part1::_position_kinds_with (omits profile on port)")
    }

    #[allow(clippy::too_many_arguments)]
    pub fn _report_statement(
        _stmt: &Instruction,
        _positions: &OrderedMap<usize, String>,
        _known: &KnownNames,
        _variable_names: &BTreeSet<String>,
        _profile: &Profile,
        _diags: &mut Diagnostics,
        _families: &str,
        _dim_counts: Option<&DimCounts>,
        _ds: Option<&DimsState>,
    ) -> bool {
        todo!("wire part2::_report_statement_with (omits profile & takes Owned types on port)")
    }

    pub fn _sdmsio4_count(_stmt: &Instruction, _env: &Env) -> Option<i64> {
        todo!("wire part5::_sdmsio4_count_with (omits env on port)")
    }

    pub fn matrix_undefined_operands(
        _stmt: &Instruction,
        _program: &Program,
        _env: &Env,
        _known: &KnownNames,
        _profile: &Profile,
    ) -> Option<Vec<String>> {
        todo!("wire instr_rules::matrix_undefined_operands_with")
    }

    pub fn _dim_limit(_profile: &Profile) -> Option<i64> {
        todo!("wire part2::_dim_limit_with (omits profile on port)")
    }

    pub fn _over_indexed(
        _expr: Option<&Expr>,
        _dim_counts: &DimCounts,
        _limit: Option<i64>,
    ) -> bool {
        todo!("wire part2::_over_indexed (takes OrderedMap instead of DimCounts on port)")
    }

    pub fn dimensions_message(
        _text: &str,
        _profile: &Profile,
        _whole_statement: bool,
        _at_eol: bool,
    ) -> String {
        todo!("wire part2::dimensions_message_with (omits profile on port)")
    }

    pub fn _names_and_dims(
        _known: &KnownNames,
        _ds: Option<&DimsState>,
        _stmt: &StmtRef,
        _prior: &[Option<&Expr>],
        _expr: Option<&Expr>,
    ) -> (Vec<MaybeDottedName>, bool) {
        todo!("wire part2::_names_and_dims_with (omits prior, expr, return type on port)")
    }

    pub fn _names_and_dims_multi(
        _known: &KnownNames,
        _ds: Option<&DimsState>,
        _stmt: &StmtRef,
        _prior: &[Option<&Expr>],
        _exprs: &[Option<&Expr>],
    ) -> (Vec<MaybeDottedName>, bool) {
        todo!("wire part2::_names_and_dims_with (omits prior, exprs, return type on port)")
    }

    pub fn _names_through_over_index(
        _expr: Option<&Expr>,
        _known: &KnownNames,
        _dim_counts: &DimCounts,
    ) -> Vec<MaybeDottedName> {
        todo!("wire part2::_names_through_over_index_with (omits dim_counts on port)")
    }

    pub fn _check_for_headers(
        _program: &Program,
        _profile: &Profile,
        _diags: &mut Diagnostics,
        _known: &KnownNames,
    ) {
        todo!("wire part5::_check_for_headers_with (omits profile & known on port)")
    }

    pub fn _check_elseif_and_case(
        _program: &Program,
        _profile: &Profile,
        _diags: &mut Diagnostics,
        _known: &KnownNames,
    ) {
        todo!("wire part5::_check_elseif_and_case_with (omits profile & known on port)")
    }

    pub fn _check_function_statements(
        _program: &Program,
        _profile: &Profile,
        _diags: &mut Diagnostics,
        _known: &KnownNames,
    ) {
        todo!("wire part5::_check_function_statements_with (omits profile & known on port)")
    }

    pub fn _function_target_message(
        _stmt: &Assignment,
        _profile: &Profile,
        _declared: &BTreeSet<String>,
    ) -> Option<String> {
        todo!("wire part4::_function_target_message_with (omits profile on port)")
    }

    pub fn structure_out_of_place(_profile: &Profile, _text: &str, _at_eol: bool) -> String {
        todo!("wire structure::_out_of_place_with (omits profile on port)")
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::rc::Rc;

    use super::{
        _dotted_members_with, _names_before_invalid_compound_with, _report_const_names_with,
        _unknown_public_member_with, _unknown_setting_with, _unknown_status_member_with,
    };
    use crate::checker::expressions::part2::KnownNames;
    use crate::diagnostics::Diagnostics;
    use crate::profiles;
    use crate::syntax::{Binary, ConstDecl, Expr, Member, Name, Node, StmtBase};

    fn name_expr(name: &str) -> Rc<Expr> {
        Rc::new(Expr::Name(Name {
            node: Node::default(),
            name: name.to_owned(),
        }))
    }

    fn member_expr(
        base: Option<Rc<Expr>>,
        member: &str,
        args: Option<Vec<Option<Rc<Expr>>>>,
    ) -> Member {
        Member {
            node: Node::default(),
            base,
            member: member.to_owned(),
            args,
        }
    }

    #[test]
    fn dotted_members_collects_in_source_order() {
        let cd = Rc::new(Expr::Member(member_expr(Some(name_expr("C")), "D", None)));
        let ab = Rc::new(Expr::Member(member_expr(
            Some(name_expr("A")),
            "B",
            Some(vec![Some(cd)]),
        )));
        let tail = Rc::new(Expr::Binary(Binary {
            node: Node::default(),
            op: "+".to_owned(),
            left: Some(name_expr("_u.")),
            right: Some(name_expr("1.")),
        }));
        let rhs = Rc::new(Expr::Binary(Binary {
            node: Node::default(),
            op: "+".to_owned(),
            left: Some(name_expr("T.")),
            right: Some(tail),
        }));
        let expr = Expr::Binary(Binary {
            node: Node::default(),
            op: "+".to_owned(),
            left: Some(ab),
            right: Some(rhs),
        });
        let got: Vec<(String, Option<String>)> = _dotted_members_with(Some(&expr))
            .into_iter()
            .map(|(b, m)| (b, m.map(|mm| mm.member)))
            .collect();
        assert_eq!(
            got,
            vec![
                ("A".to_owned(), Some("B".to_owned())),
                ("C".to_owned(), Some("D".to_owned())),
                ("T".to_owned(), None),
                ("_u".to_owned(), None),
            ]
        );
    }

    #[test]
    fn unknown_status_setting_and_public_members_match_python() {
        let cr6 = profiles::get("cr6").unwrap();
        let cr300 = profiles::get("cr300").unwrap();
        let bat = member_expr(Some(name_expr("Status")), "Battery", None);
        let nope = member_expr(Some(name_expr("Status")), "Nope", None);
        let num = member_expr(Some(name_expr("Status")), "1", None);
        assert!(!_unknown_status_member_with(&bat, cr6));
        assert!(_unknown_status_member_with(&nope, cr6));
        assert!(!_unknown_status_member_with(&num, cr6));
        assert!(!_unknown_status_member_with(&bat, cr300));
        assert!(_unknown_status_member_with(&nope, cr300));
        assert!(!_unknown_status_member_with(&num, cr300));

        let set_ok = member_expr(Some(name_expr("Settings")), "PakBusAddress", None);
        let set_bad = member_expr(Some(name_expr("Settings")), "Nope", None);
        assert!(!_unknown_setting_with(&set_ok, cr6));
        assert!(_unknown_setting_with(&set_bad, cr6));

        let vars: BTreeSet<String> = ["FOO".to_owned()].into_iter().collect();
        assert!(!_unknown_public_member_with(
            &member_expr(None, "TimeStamp", None),
            &vars
        ));
        assert!(!_unknown_public_member_with(
            &member_expr(None, "Foo", None),
            &vars
        ));
        assert!(_unknown_public_member_with(
            &member_expr(None, "Bar", None),
            &vars
        ));
    }

    #[test]
    fn names_before_invalid_compound_returns_none_for_valid_or_non_compound() {
        let known = KnownNames::default();
        let ro: BTreeSet<String> = ["PI".to_owned()].into_iter().collect();
        let valid_compound = Expr::Binary(Binary {
            node: Node::default(),
            op: "+=".to_owned(),
            left: Some(name_expr("X")),
            right: Some(name_expr("1")),
        });
        assert_eq!(
            _names_before_invalid_compound_with(Some(&valid_compound), &known, &ro),
            None
        );
    }

    #[test]
    fn report_const_names_matches_cr6_and_cr300() {
        let cr6 = profiles::get("cr6").unwrap();
        let cr300 = profiles::get("cr300").unwrap();
        let stmt = ConstDecl {
            base: StmtBase {
                node: Node {
                    line: 3,
                    file: "main.cr6".to_owned(),
                },
                text: "Const Foo = Baz + Qux".to_owned(),
                ..StmtBase::default()
            },
            name: "Foo".to_owned(),
            value: Some(name_expr("Baz")),
            type_: None,
            value_span: Some((12, 21)),
        };
        let names = vec!["Baz".to_owned(), "Qux".to_owned()];

        let mut d6 = Diagnostics::new();
        _report_const_names_with(&stmt, &names, cr6, &mut d6);
        let m6: Vec<&str> = d6.items.iter().map(|d| d.message.as_str()).collect();
        assert_eq!(m6, vec!["Invalid symbol in expression: Baz + Qux."]);

        let mut d3 = Diagnostics::new();
        _report_const_names_with(&stmt, &names, cr300, &mut d3);
        let m3: Vec<&str> = d3.items.iter().map(|d| d.message.as_str()).collect();
        assert_eq!(m3, vec!["Baz is not defined.", "Qux is not defined."]);
    }
}
