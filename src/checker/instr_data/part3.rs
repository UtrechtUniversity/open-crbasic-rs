//! Port of `checker/instr_data.py` (lines 1474-1963).

#![allow(dead_code, unused_variables, unused_imports)]
#![allow(clippy::used_underscore_items)]

use std::collections::BTreeSet;
use std::rc::Rc;
use std::sync::LazyLock;

use crate::diagnostics::{DiagRef, Diagnostic, Diagnostics, ShowFile, TABLE_NAME_KINDS};
use crate::profiles::Profile;
use crate::syntax::{
    Assignment, Call, Case, ConstDecl, Declaration, Expr, ExprRef, Index, Instruction, Juxtaposed,
    Member, Number, OrderedMap, Program, ProgramRef, Stmt, StmtRef, SubDef, Unparsed, VarDecl,
};
use crate::{pyre, pystr};

/// Port of `checker/instr_data.py::_check_stage2a`.
pub fn _check_stage2a(
    _program: &Program,
    _profile: &Profile,
    _diags: &mut Diagnostics,
    _destinations: bool,
) -> bool {
    todo!("checker/instr_data.py::_check_stage2a (omitted: env: untyped)")
}

/// Port of `checker/instr_data.py::check_eventcount_tables`.
pub fn check_eventcount_tables(_program: &Program, _profile: &Profile, _diags: &mut Diagnostics) {
    todo!("checker/instr_data.py::check_eventcount_tables")
}

/// Port of `checker/instr_data.py::_statement_exprs`.
pub fn _statement_exprs(_st: &Stmt) -> Vec<ExprRef> {
    todo!("checker/instr_data.py::_statement_exprs")
}

/// Port of `checker/instr_data.py::_members`.
pub fn _members(_expr: Option<&Expr>) {
    todo!("checker/instr_data.py::_members")
}

const _TABLE_NAME_LIMIT: usize = 20;
static _EXPR_NAME: LazyLock<pyre::Regex> =
    LazyLock::new(|| pyre::Regex::new(r"[0-9][0-9A-Za-z_]*(?:[-+*/^][0-9A-Za-z_]+)+\Z", 0));
static STRING_NAME: LazyLock<pyre::Regex> =
    LazyLock::new(|| pyre::Regex::new(r"[0-9A-Za-z_.]+", 0));

/// Port of `checker/instr_data.py::_declared_table_name`.
pub fn _declared_table_name(st: &Instruction) -> Option<String> {
    match st.args.first()?.as_deref()? {
        Expr::Name(name) => Some(name.name.clone()),
        Expr::String(literal) if STRING_NAME.fullmatch(&literal.value).is_some() => {
            Some(literal.value.clone())
        }
        Expr::Binary(_) => {
            let (start, end) = (*st.arg_spans.first()?)?;
            let mut name = pystr::strip(pystr::slice(&st.base.text, Some(start), Some(end)));
            if name.starts_with('(') && name.ends_with(')') {
                name = pystr::slice(name, Some(1), Some(-1));
            }
            _EXPR_NAME.match_(name).map(|_| name.to_owned())
        }
        _ => None,
    }
}

/// Port of `checker/instr_data.py::_illegal_table_name`.
pub fn _illegal_table_name(st: &Instruction) -> Vec<DiagRef> {
    let Some(name) = _declared_table_name(st) else {
        return Vec::new();
    };
    let len = name.chars().count();
    let (text, kind) = if len > _TABLE_NAME_LIMIT {
        (
            format!(
                "DataTable name {name} is {} chars too long.",
                len - _TABLE_NAME_LIMIT
            ),
            "table_name_too_long",
        )
    } else if name.starts_with(|ch: char| ch.is_ascii_digit() || ch == '.') || name.is_empty() {
        if name.contains('.') {
            (
                format!("DataTable name {name} is illegal."),
                "table_name_illegal",
            )
        } else {
            (
                format!("Table name {name} illegal, starts with a number."),
                "table_name_number",
            )
        }
    } else {
        return Vec::new();
    };
    vec![Rc::new(Diagnostic::new(
        "error",
        st.base.node.line,
        &text,
        &st.base.node.file,
        ShowFile::Bool(true),
        0,
        kind,
        "",
    ))]
}

/// Port of `checker/instr_data.py::is_table_name_diag`.
pub fn is_table_name_diag(d: &Diagnostic) -> bool {
    TABLE_NAME_KINDS.contains(&d.kind.as_str())
}

/// Port of `checker/instr_data.py::finalize_illegal_table_names`.
pub fn finalize_illegal_table_names(
    program: &Program,
    _profile: &Profile,
    diags: &mut Diagnostics,
) {
    let Some(mark) = program
        .statements
        .iter()
        .find_map(|stmt| match stmt.as_ref() {
            Stmt::Instruction(st) if st.keyword.eq_ignore_ascii_case("DATATABLE") => {
                _illegal_table_name(st).into_iter().next()
            }
            _ => None,
        })
    else {
        return;
    };

    let mut bodies = Vec::new();
    let mut start = None;
    let mut unparsed = BTreeSet::new();
    for st in &program.statements {
        match st.as_ref() {
            Stmt::Instruction(instr) if instr.keyword.eq_ignore_ascii_case("DATATABLE") => {
                start = Some((st.file(), st.line()));
            }
            Stmt::Instruction(instr) if instr.keyword.eq_ignore_ascii_case("ENDTABLE") => {
                if let Some((file, line)) = start.take() {
                    bodies.push((file, line, st.line()));
                }
            }
            Stmt::Unparsed(_) => {
                unparsed.insert((st.file(), st.line()));
            }
            _ => {}
        }
    }
    if let Some((file, line)) = start {
        bodies.push((file, line, 1_000_000_000));
    }

    let late = |d: &Diagnostic| {
        let message = d.message.as_str();
        message.starts_with("Could not find DataTable ")
            || matches!(
                message,
                "DataTable ... EndTable not matched." | "No BeginProg in Program."
            )
            || message.ends_with("must be used inside a table declaration.")
            || message.ends_with("declared with no output fields.")
            || message.starts_with("Invalid port specified ")
    };
    let stage_b = |d: &Diagnostic| {
        late(d)
            || (d.message.ends_with(" is not defined.")
                && !unparsed.contains(&(d.file.as_str(), d.line))
                && bodies
                    .iter()
                    .any(|&(file, a, b)| file == d.file && a < d.line && d.line <= b))
    };
    let others = diags.items.iter().filter(|d| d.severity == "error");
    if others.clone().any(|d| !stage_b(d)) {
        return;
    }
    let first = std::iter::once(&mark)
        .chain(others)
        .min_by(|a, b| (late(a), a.file.as_str(), a.line).cmp(&(late(b), b.file.as_str(), b.line)))
        .expect("table name diagnostic")
        .clone();
    diags.items.retain(|d| d.severity != "error");
    diags.items.push(first);
}

/// Port of `checker/instr_data.py::_constant_data_type`.
pub fn _constant_data_type(_stmt: &Instruction, _pos: i64) -> Option<i64> {
    todo!("checker/instr_data.py::_constant_data_type (omitted: env: consteval.Env)")
}

/// Port of `checker/instr_data.py::_fieldless`.
pub fn _fieldless(_stmt: &Instruction, _profile: &Profile) -> bool {
    todo!("checker/instr_data.py::_fieldless (omitted: env: consteval.Env)")
}

/// Port of `checker/instr_data.py::check_output_datatype`.
pub fn check_output_datatype(_program: &Program, _profile: &Profile, _diags: &mut Diagnostics) {
    todo!("checker/instr_data.py::check_output_datatype (omitted: env: consteval.Env)")
}

/// Port of `checker/instr_data.py::_odd_pair_reps`.
pub fn _odd_pair_reps(_stmt: &Instruction) -> Option<String> {
    todo!("checker/instr_data.py::_odd_pair_reps (omitted: env: consteval.Env)")
}

/// Port of `checker/instr_data.py::check_odd_pair_reps`.
pub fn check_odd_pair_reps(_program: &Program, _profile: &Profile, _diags: &mut Diagnostics) {
    todo!("checker/instr_data.py::check_odd_pair_reps (omitted: env: consteval.Env)")
}
