//! Port of `checker/var_types.py`.

#![allow(
    dead_code,
    unused_variables,
    unused_imports,
    clippy::used_underscore_items,
    reason = "port keeps Python's leading-underscore function names and stubbed-dependency shims"
)]

use std::cell::RefCell;
use std::collections::{BTreeSet, HashSet};
use std::rc::Rc;
use std::sync::LazyLock;

use serde::Deserialize;

use crate::consteval::{self, Env};
use crate::diagnostics::Diagnostics;
use crate::profiles::Profile;
use crate::pyre;
use crate::pystr;
use crate::syntax::{
    ConstDecl, ConstValue, Expr, Name, Node, NumberValue, OrderedMap, Program, ProgramRef, Stmt,
    stmt_id,
};

use super::{context, expressions, structs, structure};

static STRING_FUNCTIONS: LazyLock<HashSet<String>> = LazyLock::new(|| {
    #[derive(Deserialize)]
    struct FnRow {
        name: String,
        kind: Option<i64>,
    }
    let rows: Vec<FnRow> = serde_json::from_str(include_str!("../../data/functions.json"))
        .expect("embedded data/functions.json parses");
    rows.into_iter()
        .filter(|r| r.kind == Some(312) && r.name != "CTYPE")
        .map(|r| r.name)
        .collect()
});

static STATUS_STRING_FIELDS: LazyLock<BTreeSet<String>> = LazyLock::new(|| {
    #[derive(Deserialize)]
    struct StatusField {
        name: String,
        #[serde(rename = "type")]
        type_: i64,
    }
    #[derive(Deserialize)]
    struct StatusTable {
        fields: Vec<StatusField>,
    }
    let table: StatusTable = serde_json::from_str(include_str!("../../data/status_table.json"))
        .expect("embedded data/status_table.json parses");
    table
        .fields
        .into_iter()
        .filter(|f| f.type_ == 11)
        .map(|f| pystr::upper(&f.name))
        .collect()
});

static SNMP_TYPES: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    HashSet::from([
        "INTEGER",
        "COUNTER",
        "STRING",
        "TIMETICKS",
        "OPAQUE",
        "GAUGE",
        "FLOAT",
    ])
});

static SNMP_RE: LazyLock<pyre::Regex> = LazyLock::new(|| {
    pyre::Regex::new(
        r"\s*SNMPVARIABLE\s*\(\s*(\w+)\s*,\s*[\d.]+\s*,\s*(\w+)",
        pyre::I,
    )
});

static STRING_ILLEGAL_OPS: LazyLock<HashSet<&'static str>> =
    LazyLock::new(|| HashSet::from(["AND", "OR", "XOR", "IMP", "EQV", "<<", ">>"]));

static STRING_ARITH_OPS: LazyLock<HashSet<&'static str>> =
    LazyLock::new(|| HashSet::from(["*", "/", "\\", "MOD"]));

static CTYPE_NAMES: LazyLock<HashSet<&'static str>> =
    LazyLock::new(|| HashSet::from(["LONG", "FLOAT", "DOUBLE", "STRING"]));

static POINTER_ARG_INSTRUCTIONS: LazyLock<HashSet<&'static str>> =
    LazyLock::new(|| HashSet::from(["DELAY", "MOVE", "PORTSET", "RANDOMIZE", "WHILE"]));

static POINTER_UNPARSED_RE: LazyLock<pyre::Regex> =
    LazyLock::new(|| pyre::Regex::new(r"\s*!\s*(\w+)(?:\s+\w+)?\s*", 0));

const POINTER_SUFFIX: &str =
    " must be declared as a pointer type, e.e., AS Float!, Long!, Double!, String!,...";

fn is_numeric_type(ty: Option<&str>) -> bool {
    matches!(ty, Some("FLOAT" | "LONG"))
}

/// Port of `checker/var_types.py::_types`.
pub fn _types(program: &Program, scoped: bool) {
    let _ = _types_with(&Rc::new(program.clone()), scoped);
}

/// Port of `checker/var_types.py::_types` (returning `(types, alias)`).
pub fn _types_with(
    program: &ProgramRef,
    scoped: bool,
) -> (OrderedMap<String, String>, OrderedMap<String, String>) {
    let mut types = context::of(program, None, None).variable_types(scoped);
    let mut alias: OrderedMap<String, String> = OrderedMap::new();
    for st in &program.statements {
        if scoped && let Stmt::SubDef(sub) = st.as_ref() {
            for p in &sub.params {
                let name = pystr::upper(&p.name);
                let ty = pystr::upper(p.type_.as_deref().unwrap_or("FLOAT"));
                types.insert(name.clone(), ty);
                alias.remove(&name);
            }
        }
        if let Stmt::Instruction(inst) = st.as_ref()
            && pystr::upper(&inst.keyword) == "ALIAS"
            && let Some(Some(first)) = inst.args.first().map(|a| a.as_deref())
        {
            let target_name = match first {
                Expr::Name(n) => Some(n.name.as_str()),
                Expr::Index(ix) => Some(ix.name.as_str()),
                _ => None,
            };
            if let Some(target) = target_name {
                let target_upper = pystr::upper(target);
                for a in &inst.args[1..] {
                    if let Some(Expr::Name(n)) = a.as_deref() {
                        alias.insert(pystr::upper(&n.name), target_upper.clone());
                    }
                }
            }
        }
    }
    (types, alias)
}

/// Port of `checker/var_types.py::_Typer`.
#[derive(Debug, Default, Clone)]
pub struct Typer {
    pub types: OrderedMap<String, String>,
    pub alias: OrderedMap<String, String>,
    pub consts: OrderedMap<String, ConstDecl>,
    pub function_types: OrderedMap<String, String>,
    pub table_types: OrderedMap<String, OrderedMap<String, String>>,
    pub structure_types: OrderedMap<String, OrderedMap<String, String>>,
}

impl Typer {
    /// Port of `checker/var_types.py::_Typer::__init__`.
    pub fn __init__(
        &self,
        _program: &Program,
        _scope: Option<&Program>,
        _profile: Option<&Profile>,
    ) {
        // Signature kept for stub compatibility; construct via `Typer::new`.
    }

    /// Port of `checker/var_types.py::_Typer::__init__` (constructor).
    pub fn new(
        program: &ProgramRef,
        scope: Option<&ProgramRef>,
        profile: Option<&'static Profile>,
    ) -> Self {
        let visible = scope.unwrap_or(program);
        let (types, alias) = _types_with(visible, scope.is_some());
        let mut consts = OrderedMap::new();
        for st in &visible.statements {
            if let Stmt::ConstDecl(c) = st.as_ref() {
                consts.insert(pystr::upper(&c.name), c.clone());
            }
        }
        let mut function_types = OrderedMap::new();
        for st in &program.statements {
            if let Stmt::SubDef(sub) = st.as_ref()
                && pystr::upper(&sub.keyword) == "FUNCTION"
                && let Some(rt) = &sub.return_type
                && !rt.is_empty()
            {
                let first_part = pystr::split(rt, "*", None)[0];
                function_types.insert(
                    pystr::upper(&sub.name),
                    pystr::upper(pystr::strip(first_part)),
                );
            }
        }
        let mut table_types = OrderedMap::new();
        let mut structure_types = OrderedMap::new();
        if let Some(prof) = profile {
            structure_types = structs::structure_members(visible);
            let mut status_map = OrderedMap::new();
            for n in STATUS_STRING_FIELDS.iter() {
                if prof.status_members().contains(n) {
                    status_map.insert(n.clone(), "STRING".to_owned());
                }
            }
            table_types.insert("STATUS".to_owned(), status_map);
            if prof.single_pass {
                let public: Vec<_> = visible
                    .statements
                    .iter()
                    .filter(|st| matches!(st.as_ref(), Stmt::Declaration(d) if d.kind == "PUBLIC"))
                    .cloned()
                    .collect();
                let pub_prog = Rc::new(Program::new(&visible.name, public));
                let pub_types = context::of(&pub_prog, Some(prof), None).variable_types(false);
                table_types.insert("PUBLIC".to_owned(), pub_types);
            }
        }
        Self {
            types,
            alias,
            consts,
            function_types,
            table_types,
            structure_types,
        }
    }

    /// Port of `checker/var_types.py::_Typer::base`.
    pub fn base(&self, name: &str) -> String {
        let up = pystr::upper(name);
        self.alias.get(&up).cloned().unwrap_or(up)
    }

    /// Port of `checker/var_types.py::_Typer::variable`.
    pub fn variable(&self, expr: Option<&Expr>) -> Option<String> {
        let raw_name = match expr? {
            Expr::Name(n) => &n.name,
            Expr::Index(ix) => &ix.name,
            _ => return None,
        };
        let name = self.base(raw_name);
        self.types.contains_key(&name).then_some(name)
    }

    /// Port of `checker/var_types.py::_Typer::of`.
    pub fn of(&self, expr: Option<&Expr>, depth: i64) -> Option<String> {
        let expr = expr?;
        if let Expr::Number(n) = expr {
            let text = pystr::upper(&n.text);
            if text.starts_with('&') {
                return Some("LONG".to_owned());
            }
            return Some(
                if text.contains('.') || text.contains('E') {
                    "FLOAT"
                } else {
                    "LONG"
                }
                .to_owned(),
            );
        }
        if matches!(expr, Expr::String(_)) {
            return Some("STRING".to_owned());
        }
        if let Some(var) = self.variable(Some(expr)) {
            return self.types.get(&var).cloned();
        }
        if let Expr::Member(m) = expr {
            let member = pystr::upper(&m.member);
            if let Some(Expr::Name(base)) = m.base.as_deref() {
                let base_up = pystr::upper(&base.name);
                if let Some(tbl) = self.table_types.get(&base_up) {
                    return tbl.get(&member).cloned();
                }
            }
            if !self.structure_types.is_empty() {
                let kind = self.of(m.base.as_deref(), depth + 1).unwrap_or_default();
                return self
                    .structure_types
                    .get(&kind)
                    .and_then(|fields| fields.get(&member))
                    .cloned();
            }
        }
        if let Expr::Index(ix) = expr {
            let name = pystr::upper(&ix.name);
            if STRING_FUNCTIONS.contains(&name)
                || self.function_types.get(&name).map(String::as_str) == Some("STRING")
                || (name == "IIF"
                    && ix.args.len() > 1
                    && self.of(ix.args[1].as_deref(), depth + 1).as_deref() == Some("STRING"))
                || (name == "CTYPE"
                    && ix.args.len() > 1
                    && matches!(
                        ix.args[1].as_deref(),
                        Some(Expr::Name(n)) if pystr::upper(&n.name) == "STRING"
                    ))
            {
                return Some("STRING".to_owned());
            }
        }
        if let Expr::Name(n) = expr {
            let up = pystr::upper(&n.name);
            if up == "TRUE" || up == "FALSE" {
                return Some("LONG".to_owned());
            }
            if up == "NAN" {
                return Some("FLOAT".to_owned());
            }
            let c = self.consts.get(&up)?;
            if depth > 8 {
                return None;
            }
            if let Some(t) = &c.type_
                && !t.is_empty()
            {
                return Some(pystr::upper(t));
            }
            return self.of(c.value.as_deref(), depth + 1);
        }
        if let Expr::Unary(u) = expr
            && (u.op == "-" || u.op == "+")
        {
            let inner = self.of(u.operand.as_deref(), depth + 1);
            return if is_numeric_type(inner.as_deref()) {
                inner
            } else {
                None
            };
        }
        if let Expr::Binary(b) = expr
            && matches!(b.op.as_str(), "+" | "-" | "*" | "/" | "&")
        {
            let left = self.of(b.left.as_deref(), depth + 1);
            let right = self.of(b.right.as_deref(), depth + 1);
            if (left.as_deref() == Some("STRING") || right.as_deref() == Some("STRING"))
                && (b.op == "+" || b.op == "&")
            {
                return Some("STRING".to_owned());
            }
            if is_numeric_type(left.as_deref()) && is_numeric_type(right.as_deref()) {
                return Some(
                    if left.as_deref() == Some("LONG")
                        && right.as_deref() == Some("LONG")
                        && b.op != "/"
                    {
                        "LONG"
                    } else {
                        "FLOAT"
                    }
                    .to_owned(),
                );
            }
        }
        None
    }
}

fn subexpressions<'a>(expr: Option<&'a Expr>, out: &mut Vec<&'a Expr>) {
    let Some(expr) = expr else {
        return;
    };
    out.push(expr);
    match expr {
        Expr::Binary(b) => {
            subexpressions(b.left.as_deref(), out);
            subexpressions(b.right.as_deref(), out);
        }
        Expr::Unary(u) => {
            subexpressions(u.operand.as_deref(), out);
        }
        Expr::Index(ix) => {
            for a in &ix.args {
                subexpressions(a.as_deref(), out);
            }
        }
        Expr::Member(m) => {
            for a in m.args.as_deref().into_iter().flatten() {
                subexpressions(a.as_deref(), out);
            }
        }
        Expr::ArrayLiteral(_) | Expr::Juxtaposed(_) => {
            for a in expr.array_items().into_iter().flatten() {
                subexpressions(a.as_deref(), out);
            }
        }
        Expr::Number(_) | Expr::String(_) | Expr::Name(_) => {}
    }
}

/// Port of `checker/var_types.py::_subexpressions`.
pub fn _subexpressions(expr: Option<&Expr>) {
    let mut out = Vec::new();
    subexpressions(expr, &mut out);
}

fn statement_expressions<'a>(stmt: &'a Stmt, out: &mut Vec<Option<&'a Expr>>) {
    match stmt {
        Stmt::Assignment(a) => out.push(a.value.as_deref()),
        Stmt::Instruction(i) => {
            for a in &i.args {
                out.push(a.as_deref());
            }
        }
        Stmt::Call(c) => {
            for a in &c.args {
                out.push(a.as_deref());
            }
        }
        Stmt::If(i) => {
            out.push(i.cond.as_deref());
            for inner in i
                .inline
                .iter()
                .flatten()
                .chain(i.inline_else.iter().flatten())
            {
                statement_expressions(inner, out);
            }
        }
        Stmt::ElseIf(e) => out.push(e.cond.as_deref()),
        _ => {}
    }
}

/// Port of `checker/var_types.py::_statement_expressions`.
pub fn _statement_expressions(stmt: &Stmt) {
    let mut out = Vec::new();
    statement_expressions(stmt, &mut out);
}

/// Port of `checker/var_types.py::_string_typed`.
pub fn _string_typed(typer: &mut Typer, mut node: Option<&Expr>) -> bool {
    while let Some(Expr::Binary(b)) = node {
        if STRING_ILLEGAL_OPS.contains(b.op.as_str()) {
            node = b.right.as_deref();
        } else {
            break;
        }
    }
    typer.of(node, 0).as_deref() == Some("STRING")
}

fn in_source_order<'a>(expr: Option<&'a Expr>, out: &mut Vec<&'a Expr>) {
    let Some(expr) = expr else {
        return;
    };
    match expr {
        Expr::Binary(b) => {
            in_source_order(b.left.as_deref(), out);
            out.push(expr);
            in_source_order(b.right.as_deref(), out);
        }
        Expr::Unary(u) => in_source_order(u.operand.as_deref(), out),
        Expr::Index(ix) => {
            for a in &ix.args {
                in_source_order(a.as_deref(), out);
            }
        }
        Expr::Member(m) => {
            for a in m.args.as_deref().into_iter().flatten() {
                in_source_order(a.as_deref(), out);
            }
        }
        Expr::ArrayLiteral(_) | Expr::Juxtaposed(_) => {
            for a in expr.array_items().into_iter().flatten() {
                in_source_order(a.as_deref(), out);
            }
        }
        Expr::Number(_) | Expr::String(_) | Expr::Name(_) => {}
    }
}

/// Port of `checker/var_types.py::_in_source_order`.
pub fn _in_source_order(expr: Option<&Expr>) {
    let mut out = Vec::new();
    in_source_order(expr, &mut out);
}

/// Port of `checker/var_types.py::_string_operand_messages`.
pub fn _string_operand_messages(typer: &mut Typer, stmt: &Stmt) -> Vec<String> {
    let mut found = Vec::new();
    let mut tops = Vec::new();
    statement_expressions(stmt, &mut tops);
    for top in tops {
        let mut nodes = Vec::new();
        in_source_order(top, &mut nodes);
        for node in nodes {
            let Expr::Binary(b) = node else {
                continue;
            };
            let left = _string_typed(typer, b.left.as_deref());
            let right = _string_typed(typer, b.right.as_deref());
            if b.op == "TO" {
                if right {
                    found.push("String is illegal here".to_owned());
                }
            } else if STRING_ILLEGAL_OPS.contains(b.op.as_str()) && (left || right) {
                found.push("String is illegal here".to_owned());
            } else if STRING_ARITH_OPS.contains(b.op.as_str()) && (left || right) {
                if left {
                    found.push("Illegal String operand".to_owned());
                }
                if right {
                    found.push("Illegal operand of type STRING".to_owned());
                }
            }
        }
    }
    found
}

/// Port of `checker/var_types.py::_snmp_mismatch`.
pub fn _snmp_mismatch(typer: &mut Typer, name: Option<&Expr>, snmp_type: &str) -> bool {
    let Some(var) = typer.variable(name) else {
        return false;
    };
    let snmp_upper = pystr::upper(snmp_type);
    if !SNMP_TYPES.contains(snmp_upper.as_str()) {
        return false;
    }
    let var_type = typer.types.get(&var).map_or("", String::as_str);
    match var_type {
        "FLOAT" => snmp_upper != "FLOAT",
        "STRING" => snmp_upper != "STRING" && snmp_upper != "FLOAT",
        _ => snmp_upper == "STRING",
    }
}

/// Port of `checker/var_types.py::_same_type`.
pub fn _same_type(dest: &str, source: &str) -> bool {
    dest == source || (dest == "STRING" && source == "FLOAT")
}

/// Port of `checker/var_types.py::_indexed_past_first`.
pub fn _indexed_past_first(_typer: &mut Typer, _dest: Option<&Expr>) -> bool {
    false
}

/// Port of `checker/var_types.py::_indexed_past_first` (with `env`).
pub fn _indexed_past_first_with(typer: &Typer, dest: Option<&Expr>, env: &Env) -> bool {
    let Some(Expr::Index(ix)) = dest else {
        return false;
    };
    for a in &ix.args {
        let Some(expr) = a.as_deref() else {
            continue;
        };
        let is_past = match expr {
            Expr::Binary(_) | Expr::String(_) => true,
            Expr::Name(_) if typer.variable(Some(expr)).is_some() => true,
            Expr::Number(n) => match n.value {
                NumberValue::Int(i) => i >= 2,
                NumberValue::Float(f) => f >= 2.0,
                NumberValue::BigInt(_) => true,
            },
            _ => false,
        };
        if is_past {
            return true;
        }
        if matches!(expr, Expr::Name(_))
            && let Ok(val) = consteval::value_of(Some(expr), env)
        {
            match val {
                ConstValue::Int(i) if i >= 2 => return true,
                ConstValue::Float(f) | ConstValue::CFloat(f) if f >= 2.0 => return true,
                _ => {}
            }
        }
    }
    false
}

/// Port of `checker/var_types.py::check_string_operands`.
pub fn check_string_operands(program: &Program, diags: &mut Diagnostics) {
    check_string_operands_with(&Rc::new(program.clone()), diags);
}

/// Port of `checker/var_types.py::check_string_operands` (taking `ProgramRef`).
pub fn check_string_operands_with(program: &ProgramRef, diags: &mut Diagnostics) {
    let mut typer = Typer::new(program, None, None);
    for stmt in &program.statements {
        for text in _string_operand_messages(&mut typer, stmt) {
            diags.error_basic(stmt.line(), &text, stmt.file());
        }
    }
}

/// Port of `checker/var_types.py::check`.
pub fn check(program: &Program, diags: &mut Diagnostics) {
    let prog_ref = Rc::new(program.clone());
    let ctx = context::of(&prog_ref, None, None);
    let profile = ctx
        .profile
        .expect("var_types::check requires an active context with profile");
    let env = ctx.env().clone();
    check_with(&prog_ref, profile, diags, &env);
}

/// Port of `checker/var_types.py::check` (with `profile` and `env`).
pub fn check_with(
    program: &ProgramRef,
    profile: &'static Profile,
    diags: &mut Diagnostics,
    env: &Env,
) {
    let mut typer = Typer::new(program, None, None);
    let tables = context::of(program, None, Some(env.clone())).scope_tables(false, false);
    let mut in_prog = false;
    for stmt in &program.statements {
        if let Stmt::Instruction(inst) = stmt.as_ref()
            && pystr::upper(&inst.keyword) == "BEGINPROG"
        {
            in_prog = true;
        }
        if let Stmt::Unparsed(u) = stmt.as_ref() {
            if let Some(m) = SNMP_RE.match_(&u.base.text)
                && profile.instructions().contains("SNMPVARIABLE")
            {
                let name_expr = Expr::Name(Name {
                    node: Node {
                        line: stmt.line(),
                        ..Default::default()
                    },
                    name: m.group(1).unwrap_or("").to_owned(),
                });
                if _snmp_mismatch(&mut typer, Some(&name_expr), m.group(2).unwrap_or("")) {
                    diags.error_basic(stmt.line(), "Miss match of types", stmt.file());
                }
            }
            continue;
        }
        let Stmt::Instruction(inst) = stmt.as_ref() else {
            continue;
        };
        let kw = pystr::upper(&inst.keyword);
        let args = &inst.args;
        if !profile.instructions().contains(&kw) {
            continue;
        }
        let mut message: Option<&str> = None;
        let mut extra: Option<&str> = None;
        if kw == "ROUTERSNEIGHBORS" && !args.is_empty() {
            let arg0 = args[0].as_deref();
            let kind = if matches!(arg0, Some(Expr::String(_))) {
                Some("STRING".to_owned())
            } else {
                typer.of(arg0, 0)
            };
            if typer.variable(arg0).is_some() || matches!(arg0, Some(Expr::String(_))) {
                if kind.as_deref() != Some("LONG") {
                    message = Some("Destination variable must be type LONG");
                } else if _indexed_past_first_with(&typer, arg0, env) {
                    message = Some("Destination variable cannot be indexed");
                }
            }
        } else if kw == "INSTRUCTIONTIMES"
            && !args.is_empty()
            && typer.variable(args[0].as_deref()).is_some()
        {
            let kind = typer.of(args[0].as_deref(), 0);
            if let Some(k) = kind.as_deref()
                && k != "LONG"
            {
                message = Some("Destination variable must be type LONG");
            } else if kind.as_deref() == Some("LONG") && in_prog {
                message = Some("TimesOfInstructions() must be declared before BeginProg.");
            }
        } else if kw == "ARGOSERROR" && !args.is_empty() {
            let arg0 = args[0].as_deref();
            let bad_var = typer.variable(arg0).is_some()
                && !matches!(typer.of(arg0, 0).as_deref(), None | Some("STRING"));
            let bad_expr = matches!(
                arg0,
                Some(Expr::String(_) | Expr::Number(_) | Expr::Binary(_) | Expr::Unary(_))
            );
            if bad_var || bad_expr {
                message = Some("A string variable destination is required");
            }
        } else if kw == "SORTSPAINDEXED" && args.len() > 2 {
            let dest = typer.of(args[0].as_deref(), 0);
            let source = typer.of(args[2].as_deref(), 0);
            if typer.variable(args[0].as_deref()).is_some()
                && let (Some(d), Some(s)) = (dest.as_deref(), source.as_deref())
                && !d.is_empty()
                && !s.is_empty()
                && !_same_type(d, s)
            {
                message = Some("Source and Dest types must be the same.");
            }
        } else if kw == "SAMPLE"
            && args.len() > 2
            && matches!(args[2].as_deref(), Some(Expr::Name(n)) if pystr::upper(&n.name) == "NSEC")
        {
            let source = typer.of(args[1].as_deref(), 0);
            if matches!(source.as_deref(), Some("STRING" | "BOOLEAN" | "DOUBLE")) {
                message =
                    Some("Source type must be either LONG or FLOAT or NSEC for data type NSEC");
            } else if source.as_deref() == Some("FLOAT")
                && !diags
                    .errors()
                    .iter()
                    .any(|d| d.line == stmt.line() && d.file == stmt.file())
            {
                let scope = tables.for_statement(stmt);
                let size = _array_size(&mut typer, &scope.dims, args[1].as_deref());
                let reps = consteval::value_of(args[0].as_deref(), env).ok();
                if let (Some(sz), Some(ConstValue::Int(r))) = (size, reps)
                    && r >= 1
                    && r.checked_mul(7).is_none_or(|need| sz < need)
                {
                    message = Some("Float to NSEC expects source array size >= 7");
                }
            }
        } else if (kw == "SDMCD16MASK" || kw == "SDMCD16AC") && !args.is_empty() {
            if kw == "SDMCD16MASK"
                && args.iter().take(2).any(|a| {
                    typer
                        .variable(a.as_deref())
                        .and_then(|v| typer.types.get(&v))
                        .is_some_and(|t| t == "BOOLEAN")
                })
            {
                message = Some("Source and Mask type must be LONG.");
            }
            let is_num_or_const = matches!(args[0].as_deref(), Some(Expr::Number(_)))
                || matches!(args[0].as_deref(), Some(Expr::Name(n)) if typer.consts.contains_key(&pystr::upper(&n.name)));
            if is_num_or_const {
                extra = Some("Invalid Source.");
            }
        } else if kw == "SNMPVARIABLE"
            && args.len() > 2
            && let Some(Expr::Name(n)) = args[2].as_deref()
            && _snmp_mismatch(&mut typer, args[0].as_deref(), &n.name)
        {
            message = Some("Miss match of types");
        } else if kw == "MENURECOMPILE"
            && args.len() > 1
            && matches!(args[1].as_deref(), Some(Expr::Name(_) | Expr::Index(_)))
            && typer.of(args[1].as_deref(), 0).as_deref() == Some("STRING")
        {
            message = Some("String type variable not allowed.");
        } else if kw == "SDMGENERIC"
            && args.len() > 6
            && typer.of(args[0].as_deref(), 0).as_deref() == Some("STRING")
            && typer.of(args[4].as_deref(), 0).as_deref() == Some("STRING")
            && consteval::is_constant(args[6].as_deref(), env)
        {
            let bytes_per_value = consteval::value_of(args[6].as_deref(), env).ok();
            let is_one = matches!(bytes_per_value, Some(ConstValue::Int(1)))
                || matches!(
                    bytes_per_value,
                    Some(ConstValue::Float(f) | ConstValue::CFloat(f)) if (f - 1.0).abs() == 0.0
                );
            if !is_one {
                message =
                    Some("If source and destination are of type STRING, bytes per value must be 1");
            }
        }
        for text in [message, extra].into_iter().flatten() {
            diags.error_basic(stmt.line(), text, stmt.file());
        }
    }
}

/// Port of `checker/var_types.py::_array_size`.
pub fn _array_size(
    typer: &mut Typer,
    dims: &OrderedMap<String, Vec<i64>>,
    expr: Option<&Expr>,
) -> Option<i64> {
    if matches!(expr, Some(Expr::Name(_) | Expr::Index(_)))
        && let Some(variable) = typer.variable(expr)
    {
        return dims.get(&variable).map(|shape| shape.iter().product());
    }
    Some(1)
}

/// Port of `checker/var_types.py::_ctype_illegal`.
pub fn _ctype_illegal(_node: &crate::syntax::Index) -> bool {
    false
}

/// Port of `checker/var_types.py::_ctype_illegal` (with `env`, `legal`, `known`).
pub fn _ctype_illegal_with(
    node: &crate::syntax::Index,
    env: &Env,
    legal: &BTreeSet<i64>,
    known: Option<&BTreeSet<String>>,
) -> bool {
    let args = &node.args;
    if args.iter().any(Option::is_none) {
        return false;
    }
    if args.len() == 1 {
        return true;
    }
    if args.len() != 2 {
        return false;
    }
    let kind = args[1].as_deref();
    if let Some(k) = known
        && !expressions::part1::_undefined_names(args[0].as_deref(), k).is_empty()
    {
        return true;
    }
    if let Some(Expr::Name(n)) = kind
        && CTYPE_NAMES.contains(pystr::upper(&n.name).as_str())
    {
        return false;
    }
    let Ok(value) = consteval::value_of(kind, env) else {
        return true;
    };
    match value {
        ConstValue::Str(_) | ConstValue::BigInt(_) => true,
        ConstValue::Int(i) => !legal.contains(&i),
        ConstValue::Float(f) | ConstValue::CFloat(f) => {
            !pystr::is_integer(f) || pystr::py_int_from_f64(f).is_none_or(|i| !legal.contains(&i))
        }
    }
}

/// Port of `checker/var_types.py::check_ctype`.
pub fn check_ctype(program: &Program, diags: &mut Diagnostics) {
    let prog_ref = Rc::new(program.clone());
    let ctx = context::of(&prog_ref, None, None);
    let profile = ctx
        .profile
        .expect("var_types::check_ctype requires an active context with profile");
    check_ctype_with(&prog_ref, profile, diags);
}

/// Port of `checker/var_types.py::check_ctype` (with `profile`).
pub fn check_ctype_with(program: &ProgramRef, profile: &'static Profile, diags: &mut Diagnostics) {
    if !profile.functions().contains("CTYPE") {
        return;
    }
    let env = Env::with_consts(profile, Rc::new(RefCell::new(program.consts.clone())));
    let mut legal = BTreeSet::new();
    for n in ["UINT1", "IEEE4", "IEEE8"] {
        if let Some(ConstValue::Int(v)) = _symbol_value_with(&env, n) {
            legal.insert(v);
        }
    }
    // TODO(integrator): pass `known` from `context::of(program, Some(profile), None).known_names()`
    // when `KnownNames` and `expressions::part1::_undefined_names` are ported.
    let known: Option<&BTreeSet<String>> = None;
    for stmt in &program.statements {
        let mut tops = Vec::new();
        statement_expressions(stmt, &mut tops);
        for top in tops {
            let mut nodes = Vec::new();
            subexpressions(top, &mut nodes);
            for node in nodes {
                if let Expr::Index(ix) = node
                    && pystr::upper(&ix.name) == "CTYPE"
                    && _ctype_illegal_with(ix, &env, &legal, known)
                {
                    diags.error_basic(stmt.line(), "Illegal type", stmt.file());
                }
            }
        }
    }
}

/// Port of `checker/var_types.py::_symbol_value`.
pub fn _symbol_value(_name: &str) {}

/// Port of `checker/var_types.py::_symbol_value` (with `env` and return value).
pub fn _symbol_value_with(env: &Env, name: &str) -> Option<ConstValue> {
    let expr = Expr::Name(Name {
        node: Node {
            line: 0,
            ..Default::default()
        },
        name: name.to_owned(),
    });
    consteval::value_of(Some(&expr), env).ok()
}

fn dereference_names(top: Option<&Expr>) -> Vec<Option<String>> {
    let mut nodes = Vec::new();
    subexpressions(top, &mut nodes);
    let mut out = Vec::new();
    for node in nodes {
        if let Expr::Unary(u) = node
            && u.op == "!"
        {
            let name = match u.operand.as_deref() {
                Some(Expr::Name(n)) => Some(n.name.clone()),
                Some(Expr::Index(ix)) => Some(ix.name.clone()),
                _ => None,
            };
            out.push(name);
        }
    }
    out
}

/// Port of `checker/var_types.py::_dereference_names`.
pub fn _dereference_names(top: Option<&Expr>) {
    let _ = dereference_names(top);
}

/// Local mirror of `expressions.py::_stmt_exprs` until `expressions::part2::_stmt_exprs` returns `Vec<Option<ExprRef>>`.
fn exprs_stmt_exprs(stmt: &Stmt) -> Vec<Option<&Expr>> {
    match stmt {
        Stmt::Assignment(a) => vec![a.target.as_deref(), a.value.as_deref()],
        Stmt::If(i) => vec![i.cond.as_deref()],
        Stmt::ElseIf(e) => vec![e.cond.as_deref()],
        Stmt::DoLoop(d) => vec![d.cond.as_deref()],
        Stmt::For(f) => vec![
            f.var.as_deref(),
            f.start.as_deref(),
            f.end.as_deref(),
            f.step.as_deref(),
        ],
        Stmt::Call(c) if c.tail_expr.is_some() => vec![c.tail_expr.as_deref()],
        Stmt::Instruction(i) => i.args.iter().map(|a| a.as_deref()).collect(),
        Stmt::Call(c) => c.args.iter().map(|a| a.as_deref()).collect(),
        _ => Vec::new(),
    }
}

fn has_deref_in_text(text: &str, name: &str) -> bool {
    let re = pyre::Regex::new(&format!(r"!\s*{}\b", pyre::escape(name)), pyre::I);
    re.search(text).is_some()
}

fn offender(
    scope: &OrderedMap<String, (String, String)>,
    tops: &[Option<&Expr>],
) -> Option<String> {
    for &top in tops {
        for operand in dereference_names(top) {
            let entry = operand
                .as_deref()
                .and_then(|op| scope.get(&pystr::upper(op)));
            let (orig_name, ty) = entry?;
            if ty.is_empty() {
                return None;
            }
            if !ty.ends_with('!') {
                return Some(orig_name.clone());
            }
        }
    }
    None
}

/// Port of `checker/var_types.py::check_pointer_deref`.
pub fn check_pointer_deref(program: &Program, diags: &mut Diagnostics) {
    let prog_ref = Rc::new(program.clone());
    let ctx = context::of(&prog_ref, None, None);
    let profile = ctx
        .profile
        .expect("var_types::check_pointer_deref requires an active context with profile");
    check_pointer_deref_with(&prog_ref, profile, diags);
}

/// Port of `checker/var_types.py::check_pointer_deref` (with `profile`).
pub fn check_pointer_deref_with(
    program: &ProgramRef,
    profile: &'static Profile,
    diags: &mut Diagnostics,
) {
    if profile.single_pass {
        return;
    }
    let ctx = context::of(program, Some(profile), None);
    let scopes = ctx.routine_scopes();
    // TODO(integrator): use `ctx.known_names()` once `KnownNames` is ported.
    let known_empty: BTreeSet<String> = BTreeSet::new();
    for stmt in &program.statements {
        let Some(scope) = scopes.get(&stmt_id(stmt)) else {
            continue;
        };
        let derefs: Vec<Option<String>> = exprs_stmt_exprs(stmt)
            .into_iter()
            .flat_map(dereference_names)
            .map(|n| n.filter(|name| has_deref_in_text(stmt.text(), name)))
            .collect();
        let has_undeclared = derefs.iter().any(|opt| {
            opt.as_ref().is_some_and(|n| {
                let up = pystr::upper(n);
                !scope.contains_key(&up) && !known_empty.contains(&up)
            })
        });
        if has_undeclared {
            diags.error_basic(stmt.line(), "Illegal ! operation", stmt.file());
            let entry = if let Stmt::Assignment(a) = stmt.as_ref() {
                match a.target.as_deref() {
                    Some(Expr::Name(n)) => scope.get(&pystr::upper(&n.name)),
                    Some(Expr::Index(ix)) => scope.get(&pystr::upper(&ix.name)),
                    _ => None,
                }
            } else {
                None
            };
            if let Some((orig, ty)) = entry
                && !ty.is_empty()
                && !ty.ends_with('!')
                && let Some(Some(first_deref)) = derefs.first()
            {
                let first_up = pystr::upper(first_deref);
                if !scope.contains_key(&first_up) && !known_empty.contains(&first_up) {
                    diags.error_basic(stmt.line(), &format!("{orig}{POINTER_SUFFIX}"), stmt.file());
                    diags.error_basic(
                        stmt.line(),
                        &structure::part2::_out_of_place(stmt.text(), stmt.at_eol()),
                        stmt.file(),
                    );
                }
            }
            continue;
        }
        let mut name: Option<String> = None;
        let mut echo: Option<String> = None;
        match stmt.as_ref() {
            Stmt::Assignment(a) => {
                let target = a.target.as_deref();
                let base = match target {
                    Some(Expr::Unary(u)) if u.op == "!" => u.operand.as_deref(),
                    other => other,
                };
                let base_name = match base {
                    Some(Expr::Name(n)) => Some(n.name.as_str()),
                    Some(Expr::Index(ix)) => Some(ix.name.as_str()),
                    _ => None,
                };
                let Some(bn) = base_name else {
                    continue;
                };
                if !scope.contains_key(&pystr::upper(bn)) {
                    continue;
                }
                name = offender(scope, &[target, a.value.as_deref()]);
                echo = Some(structure::part2::_out_of_place(stmt.text(), stmt.at_eol()));
            }
            Stmt::Unparsed(_) => {
                if let Some(m) = POINTER_UNPARSED_RE.fullmatch(stmt.text())
                    && let Some(g1) = m.group(1)
                    && let Some((orig, ty)) = scope.get(&pystr::upper(g1))
                    && !ty.is_empty()
                    && !ty.ends_with('!')
                {
                    name = Some(orig.clone());
                    echo = Some(structure::part2::_out_of_place(stmt.text(), stmt.at_eol()));
                }
            }
            Stmt::If(i) => {
                let Some(text) = expressions::part2::_if_condition_text(stmt) else {
                    continue;
                };
                name = offender(scope, &[i.cond.as_deref()]);
                echo = Some(format!("Invalid, or out of place expression: {text}."));
            }
            Stmt::ElseIf(e) => {
                let Some(text) = expressions::part2::_if_condition_text(stmt) else {
                    continue;
                };
                name = offender(scope, &[e.cond.as_deref()]);
                echo = Some(format!("Invalid, or out of place expression: {text}."));
            }
            Stmt::Call(c) => {
                for (i, arg) in c.args.iter().enumerate() {
                    if let Some(found) = offender(scope, &[arg.as_deref()]) {
                        name = Some(found);
                        let arg_txt = expressions::part2::_call_arg_text(
                            stmt.text(),
                            i64::try_from(i).unwrap_or(0),
                        );
                        echo = Some(format!("Invalid, or out of place expression: {arg_txt}."));
                        break;
                    }
                }
            }
            Stmt::Instruction(inst)
                if POINTER_ARG_INSTRUCTIONS.contains(pystr::upper(&inst.keyword).as_str()) =>
            {
                for (i, arg) in inst.args.iter().enumerate() {
                    if let Some(found) = offender(scope, &[arg.as_deref()]) {
                        name = Some(found);
                        let arg_txt =
                            expressions::part2::_arg_text(inst, i64::try_from(i).unwrap_or(0));
                        echo = Some(format!("Invalid, or out of place expression: {arg_txt}."));
                        break;
                    }
                }
            }
            _ => {}
        }
        if let (Some(n), Some(e)) = (name, echo)
            && has_deref_in_text(stmt.text(), &n)
        {
            diags.error_basic(stmt.line(), &format!("{n}{POINTER_SUFFIX}"), stmt.file());
            diags.error_basic(stmt.line(), &e, stmt.file());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profiles;
    use crate::syntax::{
        Assignment, Binary, Declaration, ExprRef, If, Index, Instruction, Number, StmtBase,
        StmtRef, StringLit, SubDef, Unary, Unparsed, VarDecl,
    };

    fn prog(stmts: Vec<StmtRef>) -> ProgramRef {
        Rc::new(Program::new("test.cr6", stmts))
    }

    fn decl(line: i64, kind: &str, name: &str, ty: Option<&str>, dims: Vec<ExprRef>) -> StmtRef {
        Rc::new(Stmt::Declaration(Declaration {
            base: StmtBase::new(line, "test.cr6"),
            kind: kind.to_owned(),
            vars: vec![Rc::new(VarDecl {
                name: name.to_owned(),
                dims,
                type_: ty.map(str::to_owned),
                ..VarDecl::default()
            })],
        }))
    }

    fn const_decl(line: i64, name: &str, value: ExprRef, ty: Option<&str>) -> StmtRef {
        Rc::new(Stmt::ConstDecl(ConstDecl {
            base: StmtBase::new(line, "test.cr6"),
            name: name.to_owned(),
            value: Some(value),
            type_: ty.map(str::to_owned),
            value_span: None,
        }))
    }

    fn instr(line: i64, kw: &str, args: Vec<Option<ExprRef>>) -> StmtRef {
        Rc::new(Stmt::Instruction(Instruction {
            base: StmtBase::new(line, "test.cr6"),
            keyword: kw.to_owned(),
            args,
            ..Instruction::default()
        }))
    }

    fn assign(line: i64, target: ExprRef, value: ExprRef) -> StmtRef {
        Rc::new(Stmt::Assignment(Assignment {
            base: StmtBase::new(line, "test.cr6"),
            target: Some(target),
            value: Some(value),
            compound: false,
        }))
    }

    fn e_name(name: &str) -> ExprRef {
        Rc::new(Expr::Name(Name {
            node: Node::default(),
            name: name.to_owned(),
        }))
    }

    fn e_str(value: &str) -> ExprRef {
        Rc::new(Expr::String(StringLit {
            node: Node::default(),
            value: value.to_owned(),
        }))
    }

    fn e_int(v: i64, text: &str) -> ExprRef {
        Rc::new(Expr::Number(Number {
            node: Node::default(),
            value: NumberValue::Int(v),
            text: text.to_owned(),
        }))
    }

    fn e_float(v: f64, text: &str) -> ExprRef {
        Rc::new(Expr::Number(Number {
            node: Node::default(),
            value: NumberValue::Float(v),
            text: text.to_owned(),
        }))
    }

    fn e_idx(name: &str, args: Vec<Option<ExprRef>>) -> ExprRef {
        Rc::new(Expr::Index(Index {
            node: Node::default(),
            name: name.to_owned(),
            args,
        }))
    }

    fn e_bin(op: &str, left: ExprRef, right: ExprRef) -> ExprRef {
        Rc::new(Expr::Binary(Binary {
            node: Node::default(),
            op: op.to_owned(),
            left: Some(left),
            right: Some(right),
        }))
    }

    fn e_unary(op: &str, operand: ExprRef) -> ExprRef {
        Rc::new(Expr::Unary(Unary {
            node: Node::default(),
            op: op.to_owned(),
            operand: Some(operand),
        }))
    }

    fn e_member(base: ExprRef, member: &str) -> ExprRef {
        Rc::new(Expr::Member(crate::syntax::Member {
            node: Node::default(),
            base: Some(base),
            member: member.to_owned(),
            args: None,
        }))
    }

    #[test]
    fn test_typer_types_and_of() {
        let prof = profiles::get("cr6").expect("cr6 profile");
        let p = prog(vec![
            decl(1, "PUBLIC", "S", Some("String"), vec![]),
            decl(2, "PUBLIC", "L", Some("Long"), vec![]),
            decl(3, "PUBLIC", "F", Some("Float"), vec![]),
            instr(4, "Alias", vec![Some(e_name("L")), Some(e_name("AL"))]),
            const_decl(5, "C", e_int(2, "2"), None),
            const_decl(6, "CF", e_int(1, "1"), Some("Float")),
            Rc::new(Stmt::SubDef(SubDef {
                base: StmtBase::new(7, "test.cr6"),
                keyword: "Function".to_owned(),
                name: "Foo".to_owned(),
                params: vec![],
                return_type: Some("String * 10".to_owned()),
                header_errors: vec![],
            })),
            instr(8, "StructureType", vec![Some(e_name("Pair"))]),
            decl(9, "STRUCTUREMEMBER", "A", Some("String"), vec![]),
            instr(10, "EndStructureType", vec![]),
            decl(11, "PUBLIC", "P", Some("Pair"), vec![]),
        ]);
        let typer = Typer::new(&p, None, Some(prof));
        assert_eq!(typer.base("al"), "L");
        assert_eq!(typer.of(Some(&e_name("AL")), 0).as_deref(), Some("LONG"));
        assert_eq!(typer.of(Some(&e_name("C")), 0).as_deref(), Some("LONG"));
        assert_eq!(typer.of(Some(&e_name("CF")), 0).as_deref(), Some("FLOAT"));
        assert_eq!(
            typer.of(Some(&e_idx("Foo", vec![])), 0).as_deref(),
            Some("STRING")
        );
        assert_eq!(
            typer
                .of(Some(&e_member(e_name("Status"), "OSVersion")), 0)
                .as_deref(),
            Some("STRING")
        );
        assert_eq!(
            typer.of(Some(&e_member(e_name("P"), "A")), 0).as_deref(),
            Some("STRING")
        );
        assert_eq!(
            typer
                .of(
                    Some(&e_idx(
                        "IIF",
                        vec![Some(e_int(1, "1")), Some(e_name("S")), Some(e_int(0, "0"))]
                    )),
                    0
                )
                .as_deref(),
            Some("STRING")
        );
        assert_eq!(
            typer
                .of(
                    Some(&e_idx(
                        "CType",
                        vec![Some(e_name("L")), Some(e_name("String"))]
                    )),
                    0
                )
                .as_deref(),
            Some("STRING")
        );
        assert_eq!(
            typer
                .of(
                    Some(&e_bin(
                        "+",
                        e_int(2000, "2000"),
                        e_idx("Mid", vec![Some(e_name("S")), Some(e_int(1, "1"))])
                    )),
                    0
                )
                .as_deref(),
            Some("STRING")
        );
        assert_eq!(
            typer
                .of(Some(&e_bin("/", e_int(1, "1"), e_int(2, "2"))), 0)
                .as_deref(),
            Some("FLOAT")
        );
        assert_eq!(
            typer
                .of(Some(&e_bin("+", e_int(1, "1"), e_int(2, "2"))), 0)
                .as_deref(),
            Some("LONG")
        );
        assert_eq!(
            typer.of(Some(&e_int(255, "&HFF")), 0).as_deref(),
            Some("LONG")
        );
        assert_eq!(typer.of(Some(&e_name("True")), 0).as_deref(), Some("LONG"));
        assert_eq!(typer.of(Some(&e_name("NaN")), 0).as_deref(), Some("FLOAT"));
    }

    #[test]
    fn test_string_operands_matches_python() {
        let p = prog(vec![
            decl(1, "PUBLIC", "S", Some("String"), vec![]),
            decl(2, "PUBLIC", "X", Some("Long"), vec![]),
            assign(
                3,
                e_name("X"),
                e_bin("AND", e_bin("AND", e_str("a"), e_str("b")), e_str("c")),
            ),
            assign(4, e_name("X"), e_bin("*", e_name("S"), e_int(2, "2"))),
            assign(5, e_name("X"), e_bin("*", e_int(2, "2"), e_name("S"))),
            assign(
                6,
                e_name("X"),
                e_bin(
                    "+",
                    e_bin("*", e_name("S"), e_int(2, "2")),
                    e_bin("AND", e_name("S"), e_int(1, "1")),
                ),
            ),
            assign(
                7,
                e_name("X"),
                e_bin("AND", e_bin("AND", e_name("X"), e_str("a")), e_name("X")),
            ),
            assign(
                8,
                e_name("X"),
                e_bin("AND", e_bin("AND", e_str("a"), e_name("X")), e_name("X")),
            ),
            Rc::new(Stmt::If(If {
                base: StmtBase::new(9, "test.cr6"),
                cond: Some(e_bin("AND", e_name("S"), e_int(1, "1"))),
                inline: None,
                inline_else: None,
            })),
        ]);
        let mut diags = Diagnostics::new();
        check_string_operands_with(&p, &mut diags);
        let got: Vec<(i64, String)> = diags
            .errors()
            .into_iter()
            .map(|d| (d.line, d.message.clone()))
            .collect();
        assert_eq!(
            got,
            vec![
                (3, "String is illegal here".to_owned()),
                (3, "String is illegal here".to_owned()),
                (4, "Illegal String operand".to_owned()),
                (5, "Illegal operand of type STRING".to_owned()),
                (6, "Illegal String operand".to_owned()),
                (6, "String is illegal here".to_owned()),
                (7, "String is illegal here".to_owned()),
                (7, "String is illegal here".to_owned()),
                (8, "String is illegal here".to_owned()),
                (9, "String is illegal here".to_owned()),
            ]
        );
    }

    #[test]
    fn test_check_var_types_matches_python() {
        let prof = profiles::get("cr6").expect("cr6 profile");
        let p = prog(vec![
            decl(1, "PUBLIC", "F", Some("Float"), vec![]),
            decl(2, "PUBLIC", "S", Some("String"), vec![]),
            decl(3, "PUBLIC", "L", Some("Long"), vec![e_int(4, "4")]),
            decl(4, "PUBLIC", "F3", Some("Float"), vec![e_int(3, "3")]),
            decl(5, "PUBLIC", "F7", Some("Float"), vec![e_int(7, "7")]),
            decl(6, "PUBLIC", "B", Some("Boolean"), vec![]),
            const_decl(7, "Two", e_int(2, "2"), None),
            instr(8, "InstructionTimes", vec![Some(e_name("L"))]),
            instr(9, "BeginProg", vec![]),
            instr(10, "InstructionTimes", vec![Some(e_name("L"))]),
            instr(11, "InstructionTimes", vec![Some(e_name("F"))]),
            Rc::new(Stmt::Unparsed(Unparsed {
                base: StmtBase {
                    node: Node {
                        line: 12,
                        file: "test.cr6".to_owned(),
                    },
                    text: "SNMPVariable(F, 1.2.3, INTEGER)".to_owned(),
                    ..StmtBase::default()
                },
                reason: "unparsed".to_owned(),
                expr: None,
            })),
            instr(
                13,
                "SNMPVariable",
                vec![
                    Some(e_name("S")),
                    Some(e_int(1, "1")),
                    Some(e_name("FLOAT")),
                ],
            ),
            instr(14, "RoutersNeighbors", vec![Some(e_name("F"))]),
            instr(
                15,
                "RoutersNeighbors",
                vec![Some(e_idx("L", vec![Some(e_int(1, "1"))]))],
            ),
            instr(
                16,
                "RoutersNeighbors",
                vec![Some(e_idx("L", vec![Some(e_int(2, "2"))]))],
            ),
            instr(
                17,
                "RoutersNeighbors",
                vec![Some(e_idx("L", vec![Some(e_name("Two"))]))],
            ),
            instr(18, "ArgosError", vec![Some(e_str("fuzz"))]),
            instr(
                19,
                "SortSpaIndexed",
                vec![Some(e_name("L")), Some(e_int(1, "1")), Some(e_name("S"))],
            ),
            instr(
                20,
                "Sample",
                vec![Some(e_int(1, "1")), Some(e_name("S")), Some(e_name("NSec"))],
            ),
            instr(
                21,
                "Sample",
                vec![
                    Some(e_int(1, "1")),
                    Some(e_name("F3")),
                    Some(e_name("NSec")),
                ],
            ),
            instr(
                22,
                "Sample",
                vec![
                    Some(e_int(2, "2")),
                    Some(e_name("F7")),
                    Some(e_name("NSec")),
                ],
            ),
            instr(
                23,
                "SDMCD16Mask",
                vec![Some(e_name("B")), Some(e_name("L"))],
            ),
            instr(
                24,
                "MenuRecompile",
                vec![Some(e_int(1, "1")), Some(e_name("S"))],
            ),
            instr(
                25,
                "SDMGeneric",
                vec![
                    Some(e_name("S")),
                    Some(e_int(1, "1")),
                    Some(e_int(1, "1")),
                    Some(e_int(1, "1")),
                    Some(e_name("S")),
                    Some(e_int(1, "1")),
                    Some(e_int(2, "2")),
                ],
            ),
        ]);
        let mut consts = p.consts.clone();
        consts.insert("TWO".to_owned(), ConstValue::Int(2));
        let env = Env::with_consts(prof, Rc::new(RefCell::new(consts)));
        let mut diags = Diagnostics::new();
        check_with(&p, prof, &mut diags, &env);
        let got: Vec<(i64, String)> = diags
            .errors()
            .into_iter()
            .map(|d| (d.line, d.message.clone()))
            .collect();
        assert_eq!(
            got,
            vec![
                (
                    10,
                    "TimesOfInstructions() must be declared before BeginProg.".to_owned()
                ),
                (11, "Destination variable must be type LONG".to_owned()),
                (12, "Miss match of types".to_owned()),
                (14, "Destination variable must be type LONG".to_owned()),
                (16, "Destination variable cannot be indexed".to_owned()),
                (17, "Destination variable cannot be indexed".to_owned()),
                (18, "A string variable destination is required".to_owned()),
                (19, "Source and Dest types must be the same.".to_owned()),
                (
                    20,
                    "Source type must be either LONG or FLOAT or NSEC for data type NSEC"
                        .to_owned()
                ),
                (
                    21,
                    "Float to NSEC expects source array size >= 7".to_owned()
                ),
                (
                    22,
                    "Float to NSEC expects source array size >= 7".to_owned()
                ),
                (23, "Source and Mask type must be LONG.".to_owned()),
                (24, "String type variable not allowed.".to_owned()),
                (
                    25,
                    "If source and destination are of type STRING, bytes per value must be 1"
                        .to_owned()
                ),
            ]
        );
    }

    #[test]
    fn test_check_ctype_matches_python() {
        let prof = profiles::get("cr6").expect("cr6 profile");
        let p = prog(vec![
            decl(1, "PUBLIC", "X", Some("Float"), vec![]),
            assign(2, e_name("X"), e_idx("CType", vec![Some(e_name("X"))])),
            assign(
                3,
                e_name("X"),
                e_idx("CType", vec![Some(e_name("X")), Some(e_name("Boolean"))]),
            ),
            assign(
                4,
                e_name("X"),
                e_idx("CType", vec![Some(e_name("X")), Some(e_name("IEEE4"))]),
            ),
            assign(
                5,
                e_name("X"),
                e_idx("CType", vec![Some(e_name("X")), Some(e_name("String"))]),
            ),
            assign(
                6,
                e_name("X"),
                e_idx("CType", vec![Some(e_name("X")), Some(e_float(1.5, "1.5"))]),
            ),
        ]);
        let mut diags = Diagnostics::new();
        check_ctype_with(&p, prof, &mut diags);
        let got: Vec<(i64, String)> = diags
            .errors()
            .into_iter()
            .map(|d| (d.line, d.message.clone()))
            .collect();
        assert_eq!(
            got,
            vec![
                (2, "Illegal type".to_owned()),
                (3, "Illegal type".to_owned()),
                (6, "Illegal type".to_owned()),
            ]
        );
    }

    #[test]
    fn test_dereference_names_and_same_type() {
        assert!(_same_type("STRING", "FLOAT"));
        assert!(!_same_type("LONG", "FLOAT"));
        let expr = e_bin(
            "+",
            e_unary("!", e_name("a")),
            e_bin(
                "+",
                e_unary("!", e_idx("b", vec![Some(e_int(1, "1"))])),
                e_unary("!", e_bin("+", e_name("a"), e_name("b"))),
            ),
        );
        assert_eq!(
            dereference_names(Some(&expr)),
            vec![Some("a".to_owned()), Some("b".to_owned()), None]
        );
    }
}
