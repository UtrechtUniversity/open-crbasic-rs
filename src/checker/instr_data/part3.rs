//! Port of `checker/instr_data.py` (lines 1474-1963).

#![allow(dead_code, unused_variables, unused_imports)]
#![allow(clippy::used_underscore_items)]

use std::collections::{BTreeSet, HashSet};
use std::rc::Rc;
use std::sync::LazyLock;

use crate::checker::context::{self, ScopeLookup};
use crate::checker::expressions::KnownNames;
use crate::consteval::{self, Env};
use crate::diagnostics::{DiagRef, Diagnostic, Diagnostics, ShowFile, TABLE_NAME_KINDS};
use crate::profiles::{self, Profile};
use crate::syntax::{
    Assignment, Call, Case, ConstDecl, ConstValue, Declaration, Expr, ExprRef, Index, Instruction,
    Juxtaposed, Member, Name, Number, OrderedMap, Program, ProgramRef, Stmt, StmtRef, SubDef,
    Unparsed, VarDecl, stmt_id,
};
use crate::tables::part1::{
    _dtype_code, _make_var_info, _scan_program, _string_stored_len, VarInfo,
};
use crate::tables::part2::{_anonymous_string_len, _is_anonymous_source};
use crate::{pyre, pystr};

const STAGE2A_INT32_MIN: i64 = -2_147_483_648;
fn stage3_expr_name_attr(expr: Option<&Expr>) -> &str {
    match expr {
        Some(Expr::Name(n)) => &n.name,
        Some(Expr::Index(ix)) => &ix.name,
        _ => "",
    }
}

/// Port of checker/instr_data.py::_check_stage2a
pub fn _check_stage2a(
    program: &ProgramRef,
    profile: &'static Profile,
    diags: &mut Diagnostics,
    env: &Env,
    destinations: bool,
) -> bool {
    let ctx = context::of(program, Some(profile), Some(env.clone()));
    let known = ctx.known_names();
    let reported = false;
    let tables = ctx.scope_tables(false, false);
    let aliases =
        super::part2::_alias_sources(program, &|st| tables.for_statement(st).clone(), env);
    let mut source_vars: Option<OrderedMap<String, VarInfo>> = None;
    let mut in_table = false;
    let mut sources_ok = true;
    let top_level: HashSet<usize> = program.statements.iter().map(stmt_id).collect();
    for st in super::part1::_with_inline(&program.statements) {
        let Stmt::Instruction(inst) = st.as_ref() else {
            continue;
        };
        let scope_tables = tables.for_statement(&st);
        let dims = &scope_tables.dims;
        let kw = pystr::upper(&inst.keyword);
        let inline = !top_level.contains(&stmt_id(&st));
        if destinations && !inline && kw == "ENDTABLE" && !in_table {
            return reported;
        }
        if !inline {
            in_table = kw == "DATATABLE" || (in_table && kw != "ENDTABLE");
        }
        let Some(record) = profile.instructions().get(&kw) else {
            continue;
        };
        if inline && !destinations {
            continue;
        }
        if kw == "WINDVECTOR" && !inst.args.is_empty() && !inline {
            let reps = super::part2::_reps_value(inst.args[0].as_deref(), env);
            if let Some(r) = reps
                && STAGE2A_INT32_MIN < r
                && r < 0
            {
                diags.error(
                    inst.base.node.line,
                    "Out of memory.",
                    &inst.base.node.file,
                    ShowFile::Bool(true),
                    0,
                    "out_of_memory",
                    "",
                );
                return true;
            }
            if in_table && inst.args.len() >= 8 {
                if profile.single_pass
                    && reps.is_some_and(|r| r > 0)
                    && super::part2::_short_array(
                        inst.args[2].as_deref(),
                        reps.unwrap_or(0),
                        dims,
                        env,
                    )
                {
                    let name = stage3_expr_name_attr(inst.args[2].as_deref());
                    diags.error_basic(
                        inst.base.node.line,
                        &format!("Variable {name} out of bounds."),
                        &inst.base.node.file,
                    );
                }
                let short_speed = profile.single_pass
                    && reps.is_some_and(|r| r > 0)
                    && super::part2::_short_array(
                        inst.args[1].as_deref(),
                        reps.unwrap_or(0),
                        dims,
                        env,
                    );
                if super::part2::_wind_options(inst, reps, env, diags, short_speed) {
                    return true;
                }
            }
        }
        if kw == "DEBUG"
            && !inline
            && crate::checker::instr_rules::debug_history_oom(inst, env, profile, program)
        {
            diags.error(
                inst.base.node.line,
                "Out of memory.",
                &inst.base.node.file,
                ShowFile::Bool(true),
                0,
                "out_of_memory",
                "",
            );
            return true;
        }
        if inline {
            continue;
        }
        if destinations
            && in_table
            && super::part2::_TABLE_OUTPUT.contains(&kw.as_str())
            && sources_ok
        {
            sources_ok = crate::checker::structure::_undefined_table_source(
                inst,
                profile,
                &crate::checker::expressions::KnownView::from(known),
            )
            .is_none();
        }
        let mut wind_reps = if kw == "WINDVECTOR" && !inst.args.is_empty() {
            super::part2::_reps_value(inst.args[0].as_deref(), env)
        } else {
            None
        };
        if wind_reps == Some(STAGE2A_INT32_MIN) {
            wind_reps = Some(1_i64 << 31);
        }
        let wind = kw == "WINDVECTOR"
            && in_table
            && inst.args.len() >= 3
            && matches!(
                inst.args.get(1).and_then(|a| a.as_deref()),
                Some(Expr::Name(_) | Expr::Index(_))
            )
            && wind_reps.unwrap_or(0) >= 1;
        if wind
            && destinations
            && super::part2::_short_array(
                inst.args[2].as_deref(),
                wind_reps.unwrap_or(0),
                dims,
                env,
            )
        {
            let name = stage3_expr_name_attr(inst.args[2].as_deref());
            diags.error_basic(
                inst.base.node.line,
                &format!("Variable {name} out of bounds."),
                &inst.base.node.file,
            );
        }
        let mut source_type: Option<String> = None;
        if profile.single_pass && in_table {
            if let Some(message) = _odd_pair_reps(inst, env) {
                diags.error_basic(inst.base.node.line, &message, &inst.base.node.file);
            }
            let source = inst.args.get(1).and_then(|a| a.as_deref());
            match source {
                Some(Expr::Name(n)) => {
                    source_type = scope_tables.types.get(&pystr::upper(&n.name)).cloned();
                }
                Some(Expr::Index(ix)) => {
                    source_type = scope_tables.types.get(&pystr::upper(&ix.name)).cloned();
                }
                _ => {}
            }
            if kw == "SAMPLE"
                && inst.args.len() >= 3
                && _constant_data_type(inst, 2, env) == Some(1)
                && !matches!(source_type.as_deref(), None | Some("UINT1"))
                && let Some(reps) = super::part2::_reps_value(inst.args[0].as_deref(), env)
                && !super::part2::_short_array(source, reps, dims, env)
            {
                diags.error_basic(
                    inst.base.node.line,
                    "Sample type UINT1 can only sample a variable of type UINT1.",
                    &inst.base.node.file,
                );
            }
        }
        if (super::part2::_TABLE_OUTPUT.contains(&kw.as_str()) || wind)
            && inst.args.len() >= 2
            && sources_ok
            && (in_table || !destinations)
        {
            let reps = super::part2::_reps_value(inst.args[0].as_deref(), env);
            if let Some(r) = reps
                && r < 1
            {
                diags.error_basic(
                    inst.base.node.line,
                    &format!("Illegal Reps {r}"),
                    &inst.base.node.file,
                );
                return true;
            }
            if !destinations
                && in_table
                && super::part2::_report_undefined_source(inst, record, known, profile, diags)
            {
                return true;
            }
            let source = inst.args[1].as_deref();
            if kw == "SAMPLE"
                && inst.args.len() > 2
                && _dtype_code(inst.args[2].as_deref(), env) == 11
            {
                if source_vars.is_none() {
                    let scanned = _scan_program(program);
                    let mut map = OrderedMap::new();
                    for v in &scanned.all_vars {
                        let info = _make_var_info(v, env);
                        map.insert(pystr::upper(&v.name), info);
                    }
                    source_vars = Some(map);
                }
                if let Some(sv) = source_vars.as_ref()
                    && _is_anonymous_source(source, sv, env)
                    && _string_stored_len(_anonymous_string_len(source, sv, env)) <= 0
                {
                    diags.error(
                        inst.base.node.line,
                        "Out of memory.",
                        &inst.base.node.file,
                        ShowFile::Bool(true),
                        0,
                        "out_of_memory",
                        "",
                    );
                    return true;
                }
            }
            if let Some(r) = reps
                && r > 1
                && !matches!(source, Some(Expr::Name(_) | Expr::Index(_)))
            {
                diags.error_basic(
                    inst.base.node.line,
                    "Reps must be 1 when the source is an expression.",
                    &inst.base.node.file,
                );
                diags.error_basic(
                    inst.base.node.line,
                    &format!("Variable Anonymous1, index 1, reps {r}, out of bounds."),
                    &inst.base.node.file,
                );
                return true;
            }
            let Some(reps) = reps else {
                continue;
            };
            let source_name = match source {
                Some(Expr::Name(n)) => n.name.as_str(),
                Some(Expr::Index(ix)) => ix.name.as_str(),
                _ => continue,
            };
            let source_upper = pystr::upper(source_name);
            let shape = dims.get(&source_upper);
            if shape.is_none()
                && matches!(source, Some(Expr::Name(_)))
                && let Some((first, alias_name, backing, offset, total)) =
                    aliases.get(&source_upper)
            {
                if reps > total - offset {
                    if destinations
                        && in_table
                        && !wind
                        && let Some(message) = _odd_pair_reps(inst, env)
                    {
                        diags.error_basic(inst.base.node.line, &message, &inst.base.node.file);
                    }
                    if profile.single_pass && reps <= *total {
                        diags.error_basic(
                            inst.base.node.line,
                            &format!(
                                "Variable {backing}, index {}, reps {reps}, out of bounds.",
                                offset + 1
                            ),
                            &inst.base.node.file,
                        );
                    } else {
                        diags.error_basic(
                            inst.base.node.line,
                            &format!("Variable {first} out of bounds."),
                            &inst.base.node.file,
                        );
                        diags.error_basic(
                            inst.base.node.line,
                            &format!("Variable {alias_name}, index 1, reps {reps}, out of bounds."),
                            &inst.base.node.file,
                        );
                    }
                    return true;
                }
                continue;
            }
            let Some(shape) = shape else {
                continue;
            };
            if super::part2::_is_struct_typed(program, source_name) {
                continue;
            }
            let mut start: i64 = 1;
            let mut offset: i64 = 0;
            let empty = matches!(source, Some(Expr::Index(ix)) if ix.args.is_empty());
            if let Some(Expr::Index(ix)) = source
                && !empty
            {
                if ix.args.len() != shape.len() || ix.args.iter().any(Option::is_none) {
                    continue;
                }
                let mut subs = Vec::with_capacity(ix.args.len());
                for a in &ix.args {
                    if let Some(k) = super::part2::_reps_value(a.as_deref(), env) {
                        subs.push(k);
                    }
                }
                if subs.len() != ix.args.len() {
                    continue;
                }
                start = if shape.len() == 1 { subs[0] } else { 1 };
                for (sub, n) in subs.iter().zip(shape.iter()) {
                    offset = offset * n + sub - 1;
                }
            }
            let total: i64 = shape.iter().copied().fold(1_i64, i64::saturating_mul);
            if offset + reps > total {
                if destinations
                    && in_table
                    && let Some(message) = _odd_pair_reps(inst, env)
                {
                    diags.error_basic(inst.base.node.line, &message, &inst.base.node.file);
                }
                let name = program.statements.iter().find_map(|d| match d.as_ref() {
                    Stmt::Declaration(decl) => decl
                        .vars
                        .borrow()
                        .iter()
                        .find(|v| pystr::upper(&v.name) == source_upper)
                        .map(|v| v.name.clone()),
                    _ => None,
                });
                let name = name.as_deref().unwrap_or(source_name);
                if source_type.as_deref() != Some("UINT1")
                    && match source {
                        Some(Expr::Index(ix)) => ix
                            .args
                            .iter()
                            .all(|a| matches!(a.as_deref(), Some(Expr::Number(_)))),
                        _ => true,
                    }
                {
                    diags.error_basic(
                        inst.base.node.line,
                        &format!("Variable {name} out of bounds."),
                        &inst.base.node.file,
                    );
                }
                if empty {
                    diags.error_basic(
                        inst.base.node.line,
                        "Invalid, or out of place expression: ().",
                        &inst.base.node.file,
                    );
                }
                if start + reps - 1 > total {
                    diags.error_basic(
                        inst.base.node.line,
                        &format!("Variable {name}, index {start}, reps {reps}, out of bounds."),
                        &inst.base.node.file,
                    );
                    return true;
                }
            }
        }
    }
    reported
}

/// Port of checker/instr_data.py::check_eventcount_tables
pub fn check_eventcount_tables(program: &Program, _profile: &Profile, diags: &mut Diagnostics) {
    let mut tables: OrderedMap<String, bool> = OrderedMap::new();
    let mut current: Option<String> = None;
    for st in &program.statements {
        let kw = match st.as_ref() {
            Stmt::Instruction(inst) => pystr::upper(&inst.keyword),
            _ => String::new(),
        };
        if let Stmt::Instruction(inst) = st.as_ref()
            && kw == "DATATABLE"
            && let Some(Some(Expr::Name(n))) = inst.args.first().map(|a| a.as_deref())
        {
            let upper = pystr::upper(&n.name);
            if !tables.contains_key(&upper) {
                tables.insert(upper.clone(), false);
            }
            current = Some(upper);
        } else if kw == "ENDTABLE" {
            current = None;
        } else if kw == "DATAEVENT"
            && let Some(cur) = &current
        {
            tables.insert(cur.clone(), true);
        }
    }
    if tables.is_empty() {
        return;
    }
    for st in &program.statements {
        let mut found = false;
        for expr in _statement_exprs(st) {
            for node in _members(Some(&expr)) {
                if let Some(Expr::Name(base)) = node.base.as_deref()
                    && pystr::upper(&node.member) == "EVENTCOUNT"
                    && tables.get(&pystr::upper(&base.name)) == Some(&false)
                {
                    diags.error_basic(
                        st.line(),
                        &format!(
                            "DataTable {} must have DataEvent() to request EventCount.",
                            base.name
                        ),
                        st.file(),
                    );
                    found = true;
                    break;
                }
            }
            if found {
                break;
            }
        }
    }
}

/// Port of checker/instr_data.py::_statement_exprs
pub fn _statement_exprs(st: &Stmt) -> Vec<ExprRef> {
    let mut out: Vec<Option<ExprRef>> = Vec::new();
    match st {
        Stmt::Assignment(a) => {
            out.push(a.target.clone());
            out.push(a.value.clone());
        }
        Stmt::If(s) => out.push(s.cond.clone()),
        Stmt::ElseIf(s) => out.push(s.cond.clone()),
        Stmt::DoLoop(s) => out.push(s.cond.clone()),
        Stmt::Instruction(s) => out.extend(s.args.iter().cloned()),
        Stmt::Call(s) => out.extend(s.args.iter().cloned()),
        Stmt::For(s) => {
            out.push(s.start.clone());
            out.push(s.end.clone());
            out.push(s.step.clone());
        }
        _ => {}
    }
    out.into_iter().flatten().collect()
}

/// Port of checker/instr_data.py::_members
pub fn _members(expr: Option<&Expr>) -> Vec<Member> {
    fn walk(expr: Option<&Expr>, out: &mut Vec<Member>) {
        let Some(e) = expr else {
            return;
        };
        match e {
            Expr::Member(m) => {
                out.push(m.clone());
                walk(m.base.as_deref(), out);
                if let Some(args) = &m.args {
                    for a in args {
                        walk(a.as_deref(), out);
                    }
                }
            }
            Expr::Index(ix) => {
                for a in &ix.args {
                    walk(a.as_deref(), out);
                }
            }
            Expr::Binary(b) => {
                walk(b.left.as_deref(), out);
                walk(b.right.as_deref(), out);
            }
            Expr::Unary(u) => walk(u.operand.as_deref(), out),
            _ => {}
        }
    }
    let mut out = Vec::new();
    walk(expr, &mut out);
    out
}

const _TABLE_NAME_LIMIT: usize = 20;
static _EXPR_NAME: LazyLock<pyre::Regex> =
    LazyLock::new(|| pyre::Regex::new(r"[0-9][0-9A-Za-z_]*(?:[-+*/^][0-9A-Za-z_]+)+\Z", 0));
static STRING_NAME: LazyLock<pyre::Regex> =
    LazyLock::new(|| pyre::Regex::new(r"[0-9A-Za-z_.]+", 0));

/// Port of checker/instr_data.py::_declared_table_name
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

/// Port of checker/instr_data.py::_illegal_table_name
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

/// Port of checker/instr_data.py::is_table_name_diag
pub fn is_table_name_diag(d: &Diagnostic) -> bool {
    TABLE_NAME_KINDS.contains(&d.kind.as_str())
}

/// Port of checker/instr_data.py::finalize_illegal_table_names
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

const _CR6_WIDE_SAMPLE_FIELDLESS: &[i64] = &[
    -65536, -32768, -257, -256, -255, -129, -128, 256, 257, 258, 259, 512, 513, 32767, 32768,
    65535, 65537,
];

fn _is_cr6_illegal(v: i64) -> bool {
    (0..128).contains(&v)
        && !matches!(
            v,
            1 | 2 | 3 | 6 | 7 | 9 | 11 | 14 | 17 | 18 | 20 | 24 | 25 | 28
        )
}

fn _is_cr300_illegal(v: i64) -> bool {
    matches!(v, 2 | 3 | 4 | 5 | 8 | 10 | 12 | 13 | 14 | 15 | 19 | 26 | 27)
}

fn _is_illegal_data_type(profile_name: &str, v: i64) -> bool {
    match profile_name {
        "cr6" | "cr1000x" => _is_cr6_illegal(v),
        "cr300" | "cr350" => _is_cr300_illegal(v),
        _ => false,
    }
}

fn _is_fieldless_data_type(v: i64) -> bool {
    matches!(
        v,
        0 | 16 | 29..=43 | 48..=56 | 60..=63 | 94
    )
}

fn _is_silent_fieldless(v: i64) -> bool {
    matches!(
        v,
        156..=159 | 180..=183 | 204..=211 | 220..=223 | 236..=239 | 252..=255
    )
}

fn _data_type_arg(kw: &str) -> Option<usize> {
    match kw {
        "SAMPLE" | "AVERAGE" | "MAXIMUM" | "MINIMUM" => Some(2),
        "WINDVECTOR" => Some(3),
        _ => None,
    }
}

/// Port of checker/instr_data.py::_constant_data_type
pub fn _constant_data_type(stmt: &Instruction, pos: i64, env: &Env) -> Option<i64> {
    let idx = usize::try_from(pos).ok()?;
    let arg = stmt.args.get(idx)?;
    let Ok(value) = consteval::value_of(arg.as_deref(), env) else {
        return None;
    };
    match value {
        ConstValue::Int(i) if (0..256).contains(&i) => Some(i),
        ConstValue::Float(f) | ConstValue::CFloat(f)
            if f.is_finite() && (0.0..256.0).contains(&f) =>
        {
            pystr::py_int_from_f64(f)
        }
        _ => None,
    }
}

/// Port of checker/instr_data.py::_fieldless
pub fn _fieldless(stmt: &Instruction, profile: &Profile, env: &Env) -> bool {
    let kw = pystr::upper(&stmt.keyword);
    let Some(pos) = _data_type_arg(&kw) else {
        return false;
    };
    if !matches!(profile.name, "cr6" | "cr1000x")
        || pos >= stmt.args.len()
        || stmt.args[pos].is_none()
    {
        return false;
    }
    if kw == "SAMPLE" {
        let empty = BTreeSet::new();
        if let Some(code) = crate::checker::longconv::long_value(
            stmt.args[pos].as_deref(),
            env,
            profile,
            &empty,
            &empty,
        ) && _CR6_WIDE_SAMPLE_FIELDLESS.contains(&code)
        {
            return true;
        }
    }
    _constant_data_type(stmt, i64::try_from(pos).unwrap_or(0), env)
        .is_some_and(|v| _is_fieldless_data_type(v) || _is_silent_fieldless(v))
}

/// Port of checker/instr_data.py::check_output_datatype
pub fn check_output_datatype(
    program: &ProgramRef,
    profile: &'static Profile,
    diags: &mut Diagnostics,
    env: &Env,
) {
    if !matches!(profile.name, "cr6" | "cr1000x" | "cr300" | "cr350") {
        return;
    }
    let ctx = context::of(program, Some(profile), Some(env.clone()));
    let tables = ctx.scope_tables(true, false);
    let nsec: i64 = if profile.single_pass { 23 } else { 14 };
    let mut in_table = false;
    for stmt_ref in &program.statements {
        let Stmt::Instruction(stmt) = stmt_ref.as_ref() else {
            continue;
        };
        let kw = pystr::upper(&stmt.keyword);
        in_table = kw == "DATATABLE" || (in_table && kw != "ENDTABLE");
        let Some(pos) = _data_type_arg(&kw) else {
            continue;
        };
        if !in_table || pos >= stmt.args.len() || stmt.args[pos].is_none() {
            continue;
        }
        let mut value = _constant_data_type(stmt, i64::try_from(pos).unwrap_or(0), env);
        if value.is_none() && kw == "SAMPLE" && !profile.single_pass {
            let empty = BTreeSet::new();
            if let Some(wide) = crate::checker::longconv::long_value(
                stmt.args[pos].as_deref(),
                env,
                profile,
                &empty,
                &empty,
            ) && wide != -1
                && wide != -2
                && (wide & 0x80) == 0
            {
                value = Some(wide);
            }
        }
        if value == Some(nsec) && kw == "WINDVECTOR" {
            diags.error_basic(
                stmt.base.node.line,
                "NSEC type is legal only with Sample().",
                &stmt.base.node.file,
            );
        }
        if value == Some(nsec)
            && kw == "SAMPLE"
            && let Some(arg) = stmt.args.get(1).and_then(|a| a.as_deref())
        {
            let arg_name = match arg {
                Expr::Name(n) => Some(n.name.as_str()),
                Expr::Index(ix) => Some(ix.name.as_str()),
                _ => None,
            };
            if let Some(arg_name) = arg_name {
                let scope = tables.for_statement(stmt_ref);
                let name = pystr::upper(arg_name);
                let kind = scope.types.get(&name).map(String::as_str);
                if kind == Some("FLOAT")
                    && let Some(dims) = scope.dims.get(&name)
                    && dims.iter().copied().fold(1_i64, i64::saturating_mul) < 7
                {
                    diags.error_basic(
                        stmt.base.node.line,
                        "Float to NSEC expects source array size >= 7",
                        &stmt.base.node.file,
                    );
                } else if matches!(kind, Some("DOUBLE" | "BOOLEAN")) {
                    diags.error_basic(
                        stmt.base.node.line,
                        "Source type must be either LONG or FLOAT or NSEC for data type NSEC",
                        &stmt.base.node.file,
                    );
                }
            }
        }
        if let Some(v) = value
            && (_is_illegal_data_type(profile.name, v)
                || (!profile.single_pass && !(0..=255).contains(&v)))
        {
            let name = profile
                .instructions()
                .get(&kw)
                .map_or(stmt.keyword.as_str(), |r| r.name.as_str());
            diags.error_basic(
                stmt.base.node.line,
                &format!("Illegal data type parameter {v} in {name} instruction."),
                &stmt.base.node.file,
            );
        }
    }
}

/// Port of checker/instr_data.py::_odd_pair_reps
pub fn _odd_pair_reps(stmt: &Instruction, env: &Env) -> Option<String> {
    let pos = _data_type_arg(&pystr::upper(&stmt.keyword))?;
    if pos >= stmt.args.len() || stmt.args[pos].is_none() || stmt.args.first()?.is_none() {
        return None;
    }
    let dtype = _constant_data_type(stmt, i64::try_from(pos).unwrap_or(0), env)?;
    if !matches!(dtype, 1 | 17) {
        return None;
    }
    let reps = super::part2::_reps_value(stmt.args[0].as_deref(), env)?;
    if reps > 0 && reps % 2 != 0 {
        Some(format!(
            "Reps {reps} illegal; with BOOL8 and UINT1 data types, reps must be evenly divisible by 2"
        ))
    } else {
        None
    }
}

/// Port of checker/instr_data.py::check_odd_pair_reps
pub fn check_odd_pair_reps(
    program: &Program,
    profile: &Profile,
    diags: &mut Diagnostics,
    env: &Env,
) {
    if !matches!(profile.name, "cr6" | "cr1000x") {
        return;
    }
    let mut in_table = false;
    let mut types: OrderedMap<String, String> = OrderedMap::new();
    for st in &program.statements {
        if let Stmt::Declaration(decl) = st.as_ref() {
            for v in decl.vars.borrow().iter() {
                types.insert(
                    pystr::upper(&v.name),
                    v.type_.clone().unwrap_or_else(|| "FLOAT".to_owned()),
                );
            }
        }
    }
    for st in &program.statements {
        if let Stmt::Instruction(inst) = st.as_ref()
            && pystr::upper(&inst.keyword) == "ALIAS"
            && inst.args.len() > 1
            && let Some(Expr::Name(Name { name: src, .. }) | Expr::Index(Index { name: src, .. })) =
                inst.args[0].as_deref()
            && let Some(Expr::Name(alias)) = inst.args[1].as_deref()
        {
            let target_type = types
                .get(&pystr::upper(src))
                .cloned()
                .unwrap_or_else(|| "UINT1".to_owned());
            let alias_key = pystr::upper(&alias.name);
            if !types.contains_key(&alias_key) {
                types.insert(alias_key, target_type);
            }
        }
    }
    for st in &program.statements {
        let Stmt::Instruction(stmt) = st.as_ref() else {
            continue;
        };
        let kw = pystr::upper(&stmt.keyword);
        in_table = kw == "DATATABLE" || (in_table && kw != "ENDTABLE");
        if in_table && let Some(message) = _odd_pair_reps(stmt, env) {
            diags.error_basic(stmt.base.node.line, &message, &stmt.base.node.file);
        }
        if in_table
            && kw == "SAMPLE"
            && stmt.args.len() > 2
            && stmt.args[2].is_some()
            && _constant_data_type(stmt, 2, env) == Some(1)
            && let Some(Expr::Name(Name { name: src, .. }) | Expr::Index(Index { name: src, .. })) =
                stmt.args[1].as_deref()
            && types.get(&pystr::upper(src)).is_some_and(|t| t != "UINT1")
        {
            diags.error_basic(
                stmt.base.node.line,
                "Sample type UINT1 can only sample a variable of type UINT1.",
                &stmt.base.node.file,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_text;
    use crate::profiles::PROFILES;

    fn cr6() -> &'static Profile {
        PROFILES.iter().find(|p| p.name == "cr6").unwrap()
    }

    fn cr300() -> &'static Profile {
        PROFILES.iter().find(|p| p.name == "cr300").unwrap()
    }

    #[test]
    fn eventcount_tables_reports_missing_dataevent() {
        let prof = cr6();
        let src = "\
Public X
DataTable(Tb,True,-1)
  Sample(1,X,FP2)
EndTable
DataTable(TbOk,True,-1)
  DataEvent(1,X>0,0,1)
  Sample(1,X,FP2)
EndTable
BeginProg
  X = Tb.EventCount(1,1)
  X = TbOk.EventCount(1,1)
EndProg
";
        let (prog, _) = parse_text(src, "test.CR6", None, Some(prof));
        let mut diags = Diagnostics::default();
        check_eventcount_tables(&prog, prof, &mut diags);
        let msgs: Vec<_> = diags
            .errors()
            .into_iter()
            .map(|d| d.message.clone())
            .collect();
        assert_eq!(
            msgs,
            vec!["DataTable Tb must have DataEvent() to request EventCount."]
        );
    }

    #[test]
    fn statement_exprs_and_members_walk_nested_expressions() {
        let prof = cr6();
        let src = "\
BeginProg
  If A.B + C.D(E.F) > 0 Then
    X.Y = Z
  EndIf
EndProg
";
        let (prog, _) = parse_text(src, "test.CR6", None, Some(prof));
        let mut all_members = Vec::new();
        for st in &prog.statements {
            for expr in _statement_exprs(st) {
                for m in _members(Some(&expr)) {
                    all_members.push(m.member);
                }
            }
        }
        assert_eq!(all_members, vec!["B", "D", "F", "Y"]);
    }

    #[test]
    fn constant_data_type_and_fieldless_handle_truncation_and_wide_codes() {
        let prof = cr6();
        let prof300 = cr300();
        let env = Env::new(prof);
        let src = "\
Public X
DataTable(T,True,-1)
  Sample(1,X,2.9)
  Sample(1,X,29)
  Sample(1,X,156)
  Sample(1,X,256)
  Sample(1,X,2)
  Sample(1,X,300)
EndTable
";
        let (prog, _) = parse_text(src, "test.CR6", None, Some(prof));
        let instrs: Vec<&Instruction> = prog
            .statements
            .iter()
            .filter_map(|s| match s.as_ref() {
                Stmt::Instruction(i) if pystr::upper(&i.keyword) == "SAMPLE" => Some(i),
                _ => None,
            })
            .collect();
        assert_eq!(_constant_data_type(instrs[0], 2, &env), Some(2));
        assert_eq!(_constant_data_type(instrs[3], 2, &env), None);
        assert_eq!(_constant_data_type(instrs[5], 2, &env), None);

        assert!(!_fieldless(instrs[0], prof, &env));
        assert!(_fieldless(instrs[1], prof, &env));
        assert!(_fieldless(instrs[2], prof, &env));
        assert!(_fieldless(instrs[3], prof, &env));
        assert!(!_fieldless(instrs[4], prof, &env));
        assert!(!_fieldless(instrs[1], prof300, &env));
    }

    #[test]
    fn output_datatype_checks_illegal_and_nsec_rules() {
        let prof = cr6();
        let env = Env::new(prof);
        let src = "\
Public F(4) As Float
Public D As Double
DataTable(T,True,-1)
  Sample(1,F,4)
  Sample(1,F,14)
  Sample(1,D,14)
  WindVector(1,F,F,14,0,0,0,0)
EndTable
";
        let (prog, _) = parse_text(src, "test.CR6", None, Some(prof));
        let mut diags = Diagnostics::default();
        check_output_datatype(&prog, prof, &mut diags, &env);
        let msgs: Vec<_> = diags
            .errors()
            .into_iter()
            .map(|d| d.message.clone())
            .collect();
        assert_eq!(
            msgs,
            vec![
                "Illegal data type parameter 4 in Sample instruction.",
                "Float to NSEC expects source array size >= 7",
                "Source type must be either LONG or FLOAT or NSEC for data type NSEC",
                "NSEC type is legal only with Sample().",
            ]
        );

        let prof300 = cr300();
        let env300 = Env::new(prof300);
        let src300 = "\
Public X
DataTable(T,True,-1)
  Sample(1,X,2)
EndTable
";
        let (prog300, _) = parse_text(src300, "test.CR300", None, Some(prof300));
        let mut diags300 = Diagnostics::default();
        check_output_datatype(&prog300, prof300, &mut diags300, &env300);
        let msgs300: Vec<_> = diags300
            .errors()
            .into_iter()
            .map(|d| d.message.clone())
            .collect();
        assert_eq!(
            msgs300,
            vec!["Illegal data type parameter 2 in Sample instruction."]
        );
    }

    #[test]
    fn odd_pair_reps_checks_uint1_and_bool8_and_aliases() {
        let prof = cr6();
        let env = Env::new(prof);
        let src = "\
Public U(4) As UINT1
Public F(4) As Float
Alias U = A
DataTable(T,True,-1)
  Sample(3,U,UINT1)
  Sample(2,F,UINT1)
  Sample(2,A,UINT1)
  Sample(1,U,BOOL8)
EndTable
";
        let (prog, _) = parse_text(src, "test.CR6", None, Some(prof));
        let mut diags = Diagnostics::default();
        check_odd_pair_reps(&prog, prof, &mut diags, &env);
        let msgs: Vec<_> = diags
            .errors()
            .into_iter()
            .map(|d| d.message.clone())
            .collect();
        assert_eq!(
            msgs,
            vec![
                "Reps 3 illegal; with BOOL8 and UINT1 data types, reps must be evenly divisible by 2",
                "Sample type UINT1 can only sample a variable of type UINT1.",
                "Reps 1 illegal; with BOOL8 and UINT1 data types, reps must be evenly divisible by 2",
            ]
        );
    }

    #[test]
    fn stage2a_checks_reps_bounds_and_expr_sources() {
        let prof = cr6();
        let env = Env::new(prof);
        let src_neg = "\
Public X
DataTable(T,True,-1)
  Sample(-1,X,FP2)
EndTable
";
        let (prog_neg, _) = parse_text(src_neg, "test.CR6", None, Some(prof));
        let mut d_neg = Diagnostics::default();
        assert!(_check_stage2a(&prog_neg, prof, &mut d_neg, &env, false));
        assert_eq!(
            d_neg
                .errors()
                .into_iter()
                .map(|d| d.message.clone())
                .collect::<Vec<_>>(),
            vec!["Illegal Reps -1"]
        );

        let src_expr = "\
Public X
DataTable(T,True,-1)
  Sample(2,X+1,FP2)
EndTable
";
        let (prog_expr, _) = parse_text(src_expr, "test.CR6", None, Some(prof));
        let mut d_expr = Diagnostics::default();
        assert!(_check_stage2a(&prog_expr, prof, &mut d_expr, &env, false));
        assert_eq!(
            d_expr
                .errors()
                .into_iter()
                .map(|d| d.message.clone())
                .collect::<Vec<_>>(),
            vec![
                "Reps must be 1 when the source is an expression.",
                "Variable Anonymous1, index 1, reps 2, out of bounds.",
            ]
        );

        let src_oob = "\
Public X(3)
DataTable(T,True,-1)
  Sample(5,X,FP2)
EndTable
";
        let (prog_oob, _) = parse_text(src_oob, "test.CR6", None, Some(prof));
        let mut d_oob = Diagnostics::default();
        assert!(_check_stage2a(&prog_oob, prof, &mut d_oob, &env, false));
        assert_eq!(
            d_oob
                .errors()
                .into_iter()
                .map(|d| d.message.clone())
                .collect::<Vec<_>>(),
            vec![
                "Variable X out of bounds.",
                "Variable X, index 1, reps 5, out of bounds.",
            ]
        );
    }
}
