//! Port of `tables.py` (lines 1-745).

#![allow(
    dead_code,
    unused_variables,
    unused_imports,
    clippy::used_underscore_items
)]

use std::collections::BTreeSet;
use std::rc::Rc;
use std::sync::LazyLock;

use serde::Deserialize;

use super::TdfTable;
use crate::consteval::{self, Env};
use crate::diagnostics::{DiagRef, Diagnostic, Diagnostics};
use crate::profiles::{self, Profile};
use crate::pystr;
use crate::syntax::{
    Assignment, Call, Case, ConstDecl, ConstValue, Declaration, Expr, ExprRef, Index, Instruction,
    Juxtaposed, Member, Number, NumberValue, OrderedMap, Program, ProgramRef, Stmt, StmtRef,
    SubDef, Unparsed, VarDecl,
};
use crate::tdf::TdfField;

/// Port of `tables.py::TABLE_PROPERTIES`.
pub const TABLE_PROPERTIES: [&str; 9] = [
    "BytesPerRecord",
    "EventCount",
    "EventEnd",
    "Output",
    "Record",
    "TableBytes",
    "TableFull",
    "TableSize",
    "TimeStamp",
];

/// Port of `tables.py::TYPE_CODES`.
pub fn type_code_for(upper: &str) -> Option<i64> {
    match upper {
        "FLOAT" | "IEEE4" => Some(9),
        "FP2" => Some(7),
        "LONG" => Some(6),
        "BOOLEAN" => Some(28),
        "BOOL8" => Some(17),
        "STRING" => Some(11),
        "DOUBLE" | "IEEE8" => Some(18),
        "UINT2" => Some(2),
        "NSEC" => Some(14),
        "UINT1" => Some(1),
        "UINT4" => Some(3),
        _ => None,
    }
}

fn is_valid_type_code(v: i64) -> bool {
    matches!(v, 1 | 2 | 3 | 6 | 7 | 9 | 11 | 14 | 17 | 18 | 28)
}

/// Port of `tables.py::_dtype_code`.
pub fn _dtype_code(expr: Option<&Expr>, env: &Env) -> i64 {
    if let Some(Expr::Name(n)) = expr
        && let Some(code) = type_code_for(&pystr::upper(&n.name))
    {
        return code;
    }
    match _fold(expr, env, None) {
        Some(ConstValue::Int(i)) if is_valid_type_code(i) => i,
        Some(ConstValue::Float(f) | ConstValue::CFloat(f))
            if f.is_finite() && pystr::is_integer(f) =>
        {
            if let Some(i) = pystr::py_int_from_f64(f)
                && is_valid_type_code(i)
            {
                i
            } else {
                9
            }
        }
        _ => 9,
    }
}

pub(crate) fn _unit_seconds(name: &str) -> f64 {
    match name {
        "USEC" => 1e-6,
        "MSEC" => 1e-3,
        "MIN" => 60.0,
        "HR" => 3600.0,
        "DAY" => 86400.0,
        _ => 1.0,
    }
}

pub(crate) fn _unit_ns(name: &str) -> Option<i64> {
    match name {
        "USEC" => Some(1_000),
        "MSEC" => Some(1_000_000),
        "SEC" => Some(1_000_000_000),
        "MIN" => Some(60_000_000_000),
        "HR" => Some(3_600_000_000_000),
        "DAY" => Some(86_400_000_000_000),
        _ => None,
    }
}

pub(crate) fn _unit_name_by_value(v: i64) -> Option<&'static str> {
    match v {
        0 => Some("USEC"),
        1 => Some("MSEC"),
        2 => Some("SEC"),
        3 => Some("MIN"),
        4 => Some("HR"),
        5 => Some("DAY"),
        _ => None,
    }
}

#[derive(Debug, Deserialize)]
struct StatusFieldJson {
    #[serde(rename = "type")]
    type_: i64,
    read_only: bool,
    name: String,
    aliases: Vec<String>,
    processing: String,
    units: String,
    description: String,
    begin: i64,
    dim: i64,
    subdims: Vec<i64>,
}

#[derive(Debug, Deserialize)]
struct StatusTableJson {
    time_type: i64,
    time_into: (i64, i64),
    interval: (i64, i64),
    fields: Vec<StatusFieldJson>,
}

static STATUS_JSON: LazyLock<StatusTableJson> = LazyLock::new(|| {
    serde_json::from_str(include_str!("../../data/status_table.json"))
        .expect("embedded data/status_table.json parses")
});

pub(crate) const _FREE_MEMORY: i64 = 4_194_303;
pub(crate) const _DEFAULT_PROBE_BUFFER_BYTES: i64 = 1024;
pub(crate) const _DEFAULT_PROBE_OVERHEAD: i64 = 16;
pub(crate) const _TABLE_INIT_RESERVED: i64 = 4096;

/// Port of `tables.py::_f32`.
#[allow(
    clippy::cast_possible_truncation,
    reason = "intentional IEEE-754 float64 to float32 rounding"
)]
pub fn _f32(x: f64) -> f64 {
    f64::from(x as f32)
}

/// Port of `tables.py::_f32_safe`.
pub fn _f32_safe(x: f64) -> f64 {
    _f32(x)
}

pub(crate) const _INT32_MAX: i64 = 2_147_483_647;
const _TWO31: f64 = 2_147_483_648.0;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SizeClass {
    Auto,
    Fixed(i64, bool),
    Edge(i64),
    Inf,
    Toxic,
}

/// Port of `tables.py::size_class`.
pub fn size_class(raw: Option<&ConstValue>) -> Option<SizeClass> {
    let raw = raw?;
    let v = match raw {
        ConstValue::Int(i) if *i >= -2_147_483_648 && *i < 2_147_483_648 => *i,
        ConstValue::Int(i) => {
            #[allow(clippy::cast_precision_loss, reason = "Python int to float")]
            let f = _f32_safe(*i as f64);
            return classify_f32(f);
        }
        ConstValue::Float(f) | ConstValue::CFloat(f) => {
            let f32v = _f32_safe(*f);
            return classify_f32(f32v);
        }
        ConstValue::BigInt(s) => {
            let f = pystr::parse_float(s).unwrap_or(f64::INFINITY);
            let f32v = _f32_safe(f);
            return classify_f32(f32v);
        }
        ConstValue::Str(_) => return None,
    };
    Some(classify_i64(v))
}

fn classify_f32(f: f64) -> Option<SizeClass> {
    if f.is_nan() {
        return None;
    }
    if f.is_infinite() && f.is_sign_positive() {
        return Some(SizeClass::Inf);
    }
    if f == _TWO31 {
        return Some(SizeClass::Edge(1));
    }
    if f == -_TWO31 {
        return Some(SizeClass::Edge(-1));
    }
    if f.abs() > _TWO31 {
        return Some(SizeClass::Toxic);
    }
    let v = pystr::py_int_from_f64(f)?;
    Some(classify_i64(v))
}

fn classify_i64(v: i64) -> SizeClass {
    if v == -1 {
        SizeClass::Auto
    } else if v == 0 {
        SizeClass::Fixed(1, false)
    } else if v < -1 {
        SizeClass::Fixed((-v).min(_INT32_MAX), true)
    } else {
        SizeClass::Fixed(v, false)
    }
}

/// Port of `tables.py::declared_size`.
pub fn declared_size(raw: Option<&ConstValue>, cr3xx: bool, bytes_per_record: i64) -> Option<i64> {
    let c = size_class(raw)?;
    match c {
        SizeClass::Inf => Some(_INT32_MAX),
        SizeClass::Edge(_) if cr3xx => Some(_INT32_MAX),
        SizeClass::Toxic if cr3xx => {
            let rec = bytes_per_record + 16;
            let a = pystr::floordiv_i64(12288, rec).unwrap_or(0);
            let b = pystr::floordiv_i64(4096, rec).unwrap_or(0) + 1;
            Some((a - b).max(1))
        }
        SizeClass::Fixed(n, _) if cr3xx => Some(n),
        SizeClass::Auto | SizeClass::Edge(_) | SizeClass::Toxic | SizeClass::Fixed(_, true) => None,
        SizeClass::Fixed(n, false) => Some(n),
    }
}

/// Port of `tables.py::_fold`.
pub fn _fold(expr: Option<&Expr>, env: &Env, default: Option<ConstValue>) -> Option<ConstValue> {
    let e = expr?;
    match consteval::value_of(Some(e), env) {
        Ok(v) => Some(v),
        Err(_) => default,
    }
}

/// Port of `tables.py::_unit_value`.
pub fn _unit_value(expr: Option<&Expr>, env: &Env) -> Option<i64> {
    let v = _fold(expr, env, None)?;
    match v {
        ConstValue::Int(i) => Some(i),
        ConstValue::Float(f) | ConstValue::CFloat(f) if f.is_finite() => pystr::py_int_from_f64(f),
        _ => None,
    }
}

/// Port of `tables.py::_scan_unit_name`.
pub fn _scan_unit_name(expr: Option<&Expr>, env: &Env) -> String {
    let Some(v) = _unit_value(expr, env) else {
        return "SEC".to_string();
    };
    _unit_name_by_value(v).unwrap_or("USEC").to_string()
}

/// Port of `tables.py::_as_int`.
pub fn _as_int(v: Option<&ConstValue>, default: i64) -> i64 {
    let Some(v) = v else {
        return default;
    };
    match v {
        ConstValue::Int(i) => {
            #[allow(clippy::cast_precision_loss, reason = "Python int to float")]
            let f = *i as f64;
            pystr::py_round(f).unwrap_or(default)
        }
        ConstValue::Float(f) | ConstValue::CFloat(f) => pystr::py_round(*f).unwrap_or(default),
        ConstValue::BigInt(s) | ConstValue::Str(s) => pystr::parse_float(s)
            .and_then(pystr::py_round)
            .unwrap_or(default),
    }
}

fn _number_as_int(v: &NumberValue, default: i64) -> i64 {
    match v {
        NumberValue::Int(i) => {
            #[allow(clippy::cast_precision_loss, reason = "Python int to float")]
            let f = *i as f64;
            pystr::py_round(f).unwrap_or(default)
        }
        NumberValue::Float(f) => pystr::py_round(*f).unwrap_or(default),
        NumberValue::BigInt(s) => pystr::parse_float(s)
            .and_then(pystr::py_round)
            .unwrap_or(default),
    }
}

/// Port of `tables.py::_const_is_int`.
pub fn _const_is_int(expr: Option<&Expr>, env: &Env, int_names: &OrderedMap<String, bool>) -> bool {
    let Some(expr) = expr else {
        return true;
    };
    match expr {
        Expr::Number(n) => matches!(n.value, NumberValue::Int(_) | NumberValue::BigInt(_)),
        Expr::Name(n) => {
            let upper = pystr::upper(&n.name);
            if let Some(&b) = int_names.get(&upper) {
                return b;
            }
            if env.profile.symbols().get(&upper).is_some() {
                return true;
            }
            true
        }
        Expr::Unary(u) => {
            if u.op == "NOT" {
                return true;
            }
            _const_is_int(u.operand.as_deref(), env, int_names)
        }
        Expr::Binary(b) => {
            if b.op == "/" {
                return false;
            }
            if matches!(b.op.as_str(), "+" | "-" | "*") {
                return _const_is_int(b.left.as_deref(), env, int_names)
                    && _const_is_int(b.right.as_deref(), env, int_names);
            }
            matches!(
                b.op.as_str(),
                "\\" | "MOD"
                    | "AND"
                    | "OR"
                    | "XOR"
                    | "="
                    | "<>"
                    | "<"
                    | ">"
                    | "<="
                    | ">="
                    | "EQV"
                    | "IMP"
                    | "<<"
                    | ">>"
            )
        }
        _ => true,
    }
}

/// Port of `tables.py::VarInfo`.
#[derive(Debug, Clone, Default)]
pub struct VarInfo {
    pub type_code: i64,
    pub shape: Vec<i64>,
    pub stored_len: Option<i64>,
    pub units: String,
    pub struct_def: Option<StructTypeDef>,
    pub name: String,
}

impl VarInfo {
    /// Port of `tables.py::VarInfo::full_shape`.
    pub fn full_shape(&self) -> Vec<i64> {
        let mut s = self.shape.clone();
        if let Some(len) = self.stored_len {
            s.push(len);
        }
        s
    }

    /// Port of `tables.py::VarInfo::collapsed`.
    pub fn collapsed(&self) -> (i64, Vec<i64>) {
        let shape = self.full_shape();
        if shape.is_empty() {
            return (1, Vec::new());
        }
        if shape.len() == 1 && shape[0] == 1 {
            return (1, Vec::new());
        }
        let mut dim = 1;
        for &n in &shape {
            dim *= n;
        }
        (dim, shape)
    }
}

/// Port of `tables.py::_string_stored_len`.
pub fn _string_stored_len(n: i64) -> i64 {
    (pystr::floordiv_i64(n, 4).unwrap_or(0) + 1) * 4
}

/// Port of `tables.py::_var_shape`.
pub fn _var_shape(v: &VarDecl, env: &Env) -> Vec<i64> {
    let mut dims = Vec::with_capacity(v.dims.len());
    for d in &v.dims {
        let n = _as_int(_fold(Some(d.as_ref()), env, None).as_ref(), 1);
        dims.push(n.max(1));
    }
    dims
}

/// Port of `tables.py::_var_type_code`.
pub fn _var_type_code(v: &VarDecl) -> i64 {
    let t = pystr::upper(v.type_.as_deref().unwrap_or("FLOAT"));
    type_code_for(&t).unwrap_or(9)
}

/// Port of `tables.py::_string_size_expr`.
pub fn _string_size_expr(v: &VarDecl) -> Option<ExprRef> {
    let e = v.string_size.as_ref()?;
    if let Expr::Binary(b) = e.as_ref()
        && b.op == "="
    {
        return b.left.clone();
    }
    Some(Rc::clone(e))
}

/// Port of `tables.py::_make_var_info`.
pub fn _make_var_info(v: &VarDecl, env: &Env) -> VarInfo {
    let type_code = _var_type_code(v);
    let shape = _var_shape(v, env);
    let mut stored_len = None;
    if pystr::upper(v.type_.as_deref().unwrap_or("")) == "STRING" {
        let size_expr = _string_size_expr(v);
        let n = _as_int(_fold(size_expr.as_deref(), env, None).as_ref(), 20);
        stored_len = Some(_string_stored_len(n.max(1)));
    }
    VarInfo {
        type_code,
        shape,
        stored_len,
        units: String::new(),
        struct_def: None,
        name: v.name.clone(),
    }
}

/// Port of `tables.py::_kw`.
pub fn _kw(s: &Stmt) -> String {
    match s {
        Stmt::Instruction(inst) => pystr::upper(&inst.keyword),
        _ => String::new(),
    }
}

/// Port of `tables.py::ConstTableDecl`.
#[derive(Debug, Clone, Default)]
pub struct ConstTableDecl {
    pub name: Option<String>,
    pub hidden: i64,
    pub consts: Vec<ConstDecl>,
    pub line: i64,
}

/// Port of `tables.py::DataTableDecl`.
#[derive(Debug, Clone, Default)]
pub struct DataTableDecl {
    pub name: String,
    pub trig: Option<ExprRef>,
    pub size_expr: Option<ExprRef>,
    pub body: Vec<StmtRef>,
    pub line: i64,
}

/// `StructureType` body collected by `_scan_program`.
#[derive(Debug, Clone, Default)]
pub struct StructTypeDef {
    pub members: Vec<Rc<VarDecl>>,
    pub units: OrderedMap<String, String>,
    pub ro: BTreeSet<String>,
}

/// Port of `tables.py::_Scanned`.
#[derive(Debug, Clone, Default)]
pub struct Scanned {
    pub uses_ch400: bool,
    pub struct_types: OrderedMap<String, StructTypeDef>,
    pub uses_cpi: bool,
    pub publics: Vec<Rc<VarDecl>>,
    pub all_vars: Vec<Rc<VarDecl>>,
    pub const_tables: Vec<ConstTableDecl>,
    pub data_tables: Vec<DataTableDecl>,
    pub alias_stmts: Vec<Instruction>,
    pub units_stmts: Vec<Instruction>,
    pub readonly_names: BTreeSet<String>,
    pub slow_seq_count: i64,
    pub called: BTreeSet<String>,
}

/// Port of `tables.py::_scan_program`.
pub fn _scan_program(program: &Program) -> Scanned {
    let mut publics: Vec<Rc<VarDecl>> = Vec::new();
    let mut all_vars: Vec<Rc<VarDecl>> = Vec::new();
    let mut const_tables: Vec<ConstTableDecl> = Vec::new();
    let mut data_tables: Vec<DataTableDecl> = Vec::new();
    let mut alias_stmts: Vec<Instruction> = Vec::new();
    let mut units_stmts: Vec<Instruction> = Vec::new();
    let mut readonly_names: BTreeSet<String> = BTreeSet::new();
    let mut slow_seq_count: i64 = 0;
    let mut struct_types: OrderedMap<String, StructTypeDef> = OrderedMap::new();

    let stmts = &program.statements;
    let n = stmts.len();
    let mut i = 0;
    while i < n {
        let s = &stmts[i];
        let kw = _kw(s);
        if let Stmt::Instruction(inst) = s.as_ref()
            && kw == "STRUCTURETYPE"
        {
            let mut st = StructTypeDef::default();
            i += 1;
            while i < n && _kw(&stmts[i]) != "ENDSTRUCTURETYPE" {
                let m = &stmts[i];
                match m.as_ref() {
                    Stmt::Declaration(d) => {
                        st.members.extend(d.vars.borrow().iter().cloned());
                    }
                    Stmt::Instruction(m_inst)
                        if _kw(m) == "UNITS"
                            && m_inst.args.len() > 1
                            && let Some(Some(arg1)) = m_inst.args.get(1)
                            && let Expr::String(str_lit) = arg1.as_ref() =>
                    {
                        let target_name = match m_inst.args.first().and_then(|a| a.as_deref()) {
                            Some(Expr::Name(nm)) => pystr::upper(&nm.name),
                            Some(Expr::Index(idx)) => pystr::upper(&idx.name),
                            _ => String::new(),
                        };
                        st.units.insert(target_name, str_lit.value.clone());
                    }
                    Stmt::Instruction(m_inst) if _kw(m) == "READONLY" => {
                        for a in &m_inst.args {
                            if let Some(arg) = a
                                && let Expr::Name(nm) = arg.as_ref()
                            {
                                st.ro.insert(pystr::upper(&nm.name));
                            }
                        }
                    }
                    _ => {}
                }
                i += 1;
            }
            if let Some(Some(arg0)) = inst.args.first()
                && let Expr::Name(nm) = arg0.as_ref()
            {
                struct_types.insert(pystr::upper(&nm.name), st);
            }
            i += 1;
            continue;
        }
        if let Stmt::Declaration(d) = s.as_ref()
            && matches!(d.kind.as_str(), "PUBLIC" | "DIM" | "STATIC")
        {
            all_vars.extend(d.vars.borrow().iter().cloned());
            if d.kind == "PUBLIC" {
                publics.extend(d.vars.borrow().iter().cloned());
            }
            i += 1;
            continue;
        }
        if let Stmt::Instruction(inst) = s.as_ref()
            && kw == "CONSTTABLE"
        {
            let name = inst.args.first().and_then(|a| a.as_deref()).and_then(|e| {
                if let Expr::Name(nm) = e {
                    Some(nm.name.clone())
                } else {
                    None
                }
            });
            let hidden = if let Some(Some(arg1)) = inst.args.get(1)
                && let Expr::Number(num) = arg1.as_ref()
            {
                _number_as_int(&num.value, 0)
            } else {
                0
            };
            let mut j = i + 1;
            let mut const_body = Vec::new();
            while j < n && _kw(&stmts[j]) != "ENDCONSTTABLE" {
                if let Stmt::ConstDecl(cd) = stmts[j].as_ref() {
                    const_body.push(cd.clone());
                }
                j += 1;
            }
            const_tables.push(ConstTableDecl {
                name,
                hidden,
                consts: const_body,
                line: s.line(),
            });
            i = j + 1;
            continue;
        }
        if let Stmt::Instruction(inst) = s.as_ref()
            && kw == "DATATABLE"
        {
            let mut name = inst
                .args
                .first()
                .and_then(|a| a.as_deref())
                .and_then(|e| {
                    if let Expr::Name(nm) = e {
                        Some(nm.name.clone())
                    } else {
                        None
                    }
                })
                .unwrap_or_default();
            if !name.is_empty()
                && let Some(ConstValue::Str(val)) = program.consts.get(&pystr::upper(&name))
                && !val.is_empty()
            {
                name.clone_from(val);
            }
            let trig = inst.args.get(1).and_then(Clone::clone);
            let size_expr = inst.args.get(2).and_then(Clone::clone);
            let mut j = i + 1;
            let mut body = Vec::new();
            while j < n && _kw(&stmts[j]) != "ENDTABLE" {
                body.push(Rc::clone(&stmts[j]));
                j += 1;
            }
            data_tables.push(DataTableDecl {
                name,
                trig,
                size_expr,
                body,
                line: s.line(),
            });
            i = j + 1;
            continue;
        }
        if kw == "SLOWSEQUENCE" {
            slow_seq_count += 1;
            i += 1;
            continue;
        }
        if let Stmt::Instruction(inst) = s.as_ref()
            && kw == "ALIAS"
        {
            alias_stmts.push(inst.clone());
            i += 1;
            continue;
        }
        if let Stmt::Instruction(inst) = s.as_ref()
            && kw == "UNITS"
        {
            units_stmts.push(inst.clone());
            i += 1;
            continue;
        }
        if let Stmt::Instruction(inst) = s.as_ref()
            && kw == "READONLY"
        {
            for arg in &inst.args {
                if let Some(e) = arg
                    && let Expr::Name(nm) = e.as_ref()
                {
                    readonly_names.insert(pystr::upper(&nm.name));
                }
            }
            i += 1;
            continue;
        }
        i += 1;
    }

    let mut called: BTreeSet<String> = BTreeSet::new();
    for s in _flat(stmts) {
        let kw2 = _kw(&s);
        if kw2 == "CALLTABLE"
            && let Stmt::Instruction(inst) = s.as_ref()
            && let Some(Some(arg0)) = inst.args.first()
            && let Expr::Name(nm) = arg0.as_ref()
        {
            called.insert(pystr::upper(&nm.name));
        } else if kw2 == "ACCEPTDATARECORDS"
            && let Stmt::Instruction(inst) = s.as_ref()
            && inst.args.len() > 2
            && let Some(arg2) = &inst.args[2]
            && let Expr::Name(nm) = arg2.as_ref()
        {
            called.insert(pystr::upper(&nm.name));
        }
    }

    publics.retain(|v| {
        struct_types
            .get(&pystr::upper(v.type_.as_deref().unwrap_or("")))
            .is_none()
    });
    let uses_ch400 = stmts.iter().any(|s| _kw(s) == "CH400STATUS");
    let uses_cpi = stmts.iter().any(|s| {
        let k = _kw(s);
        k.starts_with("CDM_") || k == "CPIFILESEND"
    });

    Scanned {
        uses_ch400,
        struct_types,
        uses_cpi,
        publics,
        all_vars,
        const_tables,
        data_tables,
        alias_stmts,
        units_stmts,
        readonly_names,
        slow_seq_count,
        called,
    }
}

/// Port of `tables.py::_flat`.
pub fn _flat(ss: &[StmtRef]) -> Vec<StmtRef> {
    fn push(ss: &[StmtRef], out: &mut Vec<StmtRef>) {
        for x in ss {
            out.push(Rc::clone(x));
            if let Stmt::If(if_stmt) = x.as_ref() {
                if let Some(inline) = &if_stmt.inline {
                    push(inline, out);
                }
                if let Some(inline_else) = &if_stmt.inline_else {
                    push(inline_else, out);
                }
            }
        }
    }
    let mut out = Vec::with_capacity(ss.len());
    push(ss, &mut out);
    out
}

fn const_numeric_f64(val: &ConstValue) -> Option<f64> {
    match val {
        ConstValue::Int(i) =>
        {
            #[allow(clippy::cast_precision_loss, reason = "Python int to float")]
            Some(*i as f64)
        }
        ConstValue::Float(f) | ConstValue::CFloat(f) => Some(*f),
        ConstValue::BigInt(s) => Some(pystr::parse_float(s).unwrap_or(f64::INFINITY)),
        ConstValue::Str(_) => None,
    }
}

/// Port of `tables.py::_scan_weights`.
pub fn _scan_weights(stmts: &[StmtRef], env: &Env) -> OrderedMap<String, f64> {
    let mut stack: Vec<f64> = Vec::new();
    let mut weights: OrderedMap<String, f64> = OrderedMap::new();
    for s in stmts {
        let kw = _kw(s);
        if let Stmt::Instruction(inst) = s.as_ref()
            && kw == "SCAN"
        {
            let mut secs: Option<f64> = None;
            if let Some(Some(arg0)) = inst.args.first()
                && let Some(val) = _fold(Some(arg0.as_ref()), env, None)
                && let Some(f) = const_numeric_f64(&val)
            {
                let unit_name = if let Some(Some(arg1)) = inst.args.get(1) {
                    _scan_unit_name(Some(arg1.as_ref()), env)
                } else {
                    "SEC".to_string()
                };
                secs = Some(f * _unit_seconds(&unit_name));
            }
            stack.push(match secs {
                Some(v) if v != 0.0 => v,
                _ => 1.0,
            });
        } else if kw == "NEXTSCAN" {
            stack.pop();
        } else if let Stmt::Instruction(inst) = s.as_ref()
            && kw == "CALLTABLE"
            && !stack.is_empty()
            && let Some(Some(arg0)) = inst.args.first()
            && let Expr::Name(nm) = arg0.as_ref()
        {
            weights.insert(pystr::upper(&nm.name), *stack.last().unwrap());
        }
    }
    weights
}

/// Port of `tables.py::_scan_calls`.
pub fn _scan_calls(stmts: &[StmtRef], env: &Env) -> OrderedMap<String, Vec<Option<f64>>> {
    let mut stack: Vec<Option<f64>> = Vec::new();
    let mut calls: OrderedMap<String, Vec<Option<f64>>> = OrderedMap::new();
    for s in _flat(stmts) {
        let kw = _kw(&s);
        if let Stmt::Instruction(inst) = s.as_ref() {
            if kw == "SCAN" {
                let mut secs: Option<f64> = None;
                if let Some(Some(arg0)) = inst.args.first()
                    && let Some(val) = _fold(Some(arg0.as_ref()), env, None)
                    && let Some(f) = const_numeric_f64(&val)
                    && f > 0.0
                {
                    let unit_name = if let Some(Some(arg1)) = inst.args.get(1) {
                        _scan_unit_name(Some(arg1.as_ref()), env)
                    } else {
                        "SEC".to_string()
                    };
                    secs = Some(f * _unit_seconds(&unit_name));
                }
                stack.push(secs);
            } else if kw == "NEXTSCAN" {
                stack.pop();
            } else if kw == "CALLTABLE"
                && !stack.is_empty()
                && let Some(Some(arg0)) = inst.args.first()
                && let Expr::Name(nm) = arg0.as_ref()
            {
                let key = pystr::upper(&nm.name);
                let top = *stack.last().unwrap();
                let mut list = calls.get(&key).cloned().unwrap_or_default();
                list.push(top);
                calls.insert(key, list);
            }
        }
    }
    calls
}

/// Port of `tables.py::_last_call_interval`.
pub fn _last_call_interval(stmts: &[StmtRef], env: &Env) -> OrderedMap<String, Option<f64>> {
    let mut stack: Vec<Option<f64>> = Vec::new();
    let mut last: OrderedMap<String, Option<f64>> = OrderedMap::new();
    for s in _flat(stmts) {
        let kw = _kw(&s);
        if let Stmt::Instruction(inst) = s.as_ref() {
            if kw == "SCAN" {
                let mut secs: Option<f64> = None;
                if let Some(Some(arg0)) = inst.args.first()
                    && let Some(val) = _fold(Some(arg0.as_ref()), env, None)
                    && let Some(f) = const_numeric_f64(&val)
                    && f > 0.0
                {
                    let unit_name = if let Some(Some(arg1)) = inst.args.get(1) {
                        _scan_unit_name(Some(arg1.as_ref()), env)
                    } else {
                        "SEC".to_string()
                    };
                    secs = Some(f * _unit_seconds(&unit_name));
                }
                stack.push(secs);
            } else if kw == "NEXTSCAN" {
                stack.pop();
            } else if kw == "CALLTABLE"
                && let Some(Some(arg0)) = inst.args.first()
                && let Expr::Name(nm) = arg0.as_ref()
            {
                let top = stack.last().copied().flatten();
                last.insert(pystr::upper(&nm.name), top);
            }
        }
    }
    last
}

const _STATUS_INSERTIONS: [(&str, &str, i64, bool, &str); 4] = [
    ("SkippedSystemScan", "SkippedSlowScan", 6, false, ""),
    ("LastSystemScan", "LastSlowScan", 14, true, ""),
    ("SystemProcTime", "SlowProcTime", 6, true, "usec"),
    ("MaxSystemProcTime", "MaxSlowProcTime", 6, false, "usec"),
];

/// Port of `tables.py::_status_table`.
pub fn _status_table(slow_seq_count: i64) -> TdfTable {
    let sj = &*STATUS_JSON;
    let mut fields: Vec<TdfField> = sj
        .fields
        .iter()
        .map(|f| TdfField {
            r#type: f.type_,
            read_only: f.read_only,
            name: f.name.clone(),
            aliases: f.aliases.clone(),
            processing: f.processing.clone(),
            units: f.units.clone(),
            description: f.description.clone(),
            begin_index: f.begin,
            dimension: f.dim,
            subdims: f.subdims.clone(),
        })
        .collect();
    if slow_seq_count >= 1 {
        let (dim, subdims) = if slow_seq_count == 1 {
            (1, Vec::new())
        } else {
            (slow_seq_count, vec![slow_seq_count])
        };
        for &(after, name, type_code, ro, units) in &_STATUS_INSERTIONS {
            let new_field = TdfField {
                r#type: type_code,
                read_only: ro,
                name: name.to_string(),
                aliases: Vec::new(),
                processing: String::new(),
                units: units.to_string(),
                description: String::new(),
                begin_index: 1,
                dimension: dim,
                subdims: subdims.clone(),
            };
            let idx = fields
                .iter()
                .position(|existing| existing.name == after)
                .expect("status_table.json contains insertion anchor");
            fields.insert(idx + 1, new_field);
        }
    }
    TdfTable {
        name: "Status".to_string(),
        size: 1,
        time_type: sj.time_type,
        time_into: sj.time_into,
        interval: sj.interval,
        fields,
    }
}

/// Port of `tables.py::_build_struct_tables`.
pub fn _build_struct_tables(
    all_vars: &[Rc<VarDecl>],
    struct_types: &OrderedMap<String, StructTypeDef>,
    env: &Env,
) -> Vec<TdfTable> {
    let mut out = Vec::new();
    for v in all_vars.iter().rev() {
        let Some(st) = struct_types.get(&pystr::upper(v.type_.as_deref().unwrap_or(""))) else {
            continue;
        };
        let mut size = 1;
        for d in _var_shape(v, env) {
            size *= d;
        }
        let mut fields = Vec::with_capacity(st.members.len());
        for m in &st.members {
            let info = _make_var_info(m, env);
            let (dim, subdims) = info.collapsed();
            let m_upper = pystr::upper(&m.name);
            fields.push(TdfField {
                r#type: info.type_code,
                read_only: st.ro.contains(&m_upper),
                name: m.name.clone(),
                aliases: Vec::new(),
                processing: String::new(),
                units: st.units.get(&m_upper).cloned().unwrap_or_default(),
                description: String::new(),
                begin_index: 1,
                dimension: dim,
                subdims,
            });
        }
        out.push(TdfTable {
            name: v.name.clone(),
            size,
            time_type: 14,
            time_into: (0, 0),
            interval: (0, 0),
            fields,
        });
    }
    out
}

const _DEG_C: &str = "\u{ef}\u{bf}\u{bd}C";

const _CH400_FIELDS: [(&str, Option<&str>); 32] = [
    ("ChargeAVoltage", Some("Volts")),
    ("ChargeACurrent", Some("Amps")),
    ("ChargeBVoltage", Some("Volts")),
    ("ChargeBCurrent", Some("Amps")),
    ("InputChargeStatus", None),
    ("InputCurrentLimit", Some("Amps")),
    ("BattAVoltage", Some("Volts")),
    ("BattACurrent", Some("Amps")),
    ("BattAChargeStatus", None),
    ("BattASetVoltage", Some("Volts")),
    ("BattACurrentLimit", Some("Amps")),
    ("BattADepthofDischarge", Some("%")),
    ("BattBVoltage", Some("Volts")),
    ("BattBCurrent", Some("Amps")),
    ("BattBChargeStatus", None),
    ("BattBSetVoltage", Some("Volts")),
    ("BattBCurrentLimit", Some("Amps")),
    ("BattBDepthofDischarge", Some("%")),
    ("SystemVoltage", Some("Volts")),
    ("SystemCurrent", Some("Amps")),
    ("AlwaysOn12Voltage", Some("Volts")),
    ("AlwaysOn12Current", Some("Amps")),
    ("Switched12Voltage", Some("Volts")),
    ("Switched12Current", Some("Amps")),
    ("Switched24Voltage", Some("Volts")),
    ("Switched24Current", Some("Amps")),
    ("PowerSwitchState", None),
    ("MaxBoardTemp", Some(_DEG_C)),
    ("MinBoardTemp", Some(_DEG_C)),
    ("PanelTemp", Some(_DEG_C)),
    ("BattAProbeTemp", Some(_DEG_C)),
    ("BattBProbeTemp", Some(_DEG_C)),
];

/// Port of `tables.py::_ch400status_table`.
pub fn _ch400status_table() -> TdfTable {
    let fields = _CH400_FIELDS
        .iter()
        .map(|&(n, u)| TdfField {
            r#type: if u.is_none() { 11 } else { 9 },
            read_only: true,
            name: n.to_string(),
            aliases: Vec::new(),
            processing: String::new(),
            units: u.unwrap_or("").to_string(),
            description: String::new(),
            begin_index: 1,
            dimension: if u.is_none() { 28 } else { 1 },
            subdims: if u.is_none() { vec![28] } else { Vec::new() },
        })
        .collect();
    TdfTable {
        name: "CH400Status".to_string(),
        size: 1,
        time_type: 14,
        time_into: (0, 0),
        interval: (0, 0),
        fields,
    }
}

/// Port of `tables.py::_cpistatus_table`.
pub fn _cpistatus_table() -> TdfTable {
    let f = |t: i64, ro: bool, name: &str, dim: i64, sub: &[i64]| TdfField {
        r#type: t,
        read_only: ro,
        name: name.to_string(),
        aliases: Vec::new(),
        processing: String::new(),
        units: String::new(),
        description: String::new(),
        begin_index: 1,
        dimension: dim,
        subdims: sub.to_vec(),
    };
    let mut fields = vec![f(9, true, "BusLoad", 1, &[])];
    for n in [
        "ModuleReportCount",
        "ActiveModules",
        "BuffErr",
        "RxErrMax",
        "TxErrMax",
        "FrameErr",
    ] {
        fields.push(f(6, false, n, 1, &[]));
    }
    fields.push(f(11, false, "ModuleInfo", 6400, &[50, 128]));
    TdfTable {
        name: "CPIStatus".to_string(),
        size: 1,
        time_type: 14,
        time_into: (0, 0),
        interval: (0, 0),
        fields,
    }
}

/// Port of `tables.py::_resolve_alias_target`.
pub fn _resolve_alias_target(
    expr: Option<&Expr>,
    var_info: Option<&OrderedMap<String, VarInfo>>,
) -> (String, Option<i64>) {
    let Some(expr) = expr else {
        return (String::new(), None);
    };
    if let Expr::Name(n) = expr {
        return (pystr::upper(&n.name), None);
    }
    if let Expr::Index(idx) = expr {
        let upper = pystr::upper(&idx.name);
        if idx.args.len() > 1
            && let Some(vi) = var_info
        {
            let info = vi.get(&upper);
            let numbers: Vec<&Number> = idx
                .args
                .iter()
                .filter_map(|a| {
                    a.as_deref().and_then(|e| {
                        if let Expr::Number(num) = e {
                            Some(num)
                        } else {
                            None
                        }
                    })
                })
                .collect();
            if let Some(info) = info
                && info.shape.len() == idx.args.len()
                && idx.args.len() == numbers.len()
            {
                let mut flat = 0;
                for (k, a) in numbers.iter().enumerate() {
                    let mut stride = 1;
                    for &d in &info.shape[k + 1..] {
                        stride *= d;
                    }
                    flat += (_number_as_int(&a.value, 0) - 1) * stride;
                }
                return (upper, Some(flat + 1));
            }
        }
        if let Some(Some(first)) = idx.args.first()
            && let Expr::Number(num) = first.as_ref()
        {
            return (upper, Some(_number_as_int(&num.value, 0)));
        }
        return (upper, None);
    }
    (String::new(), None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syntax::{Binary, Name, Node, Unary};

    #[test]
    fn size_class_and_declared_size_match_python() {
        assert_eq!(
            size_class(Some(&ConstValue::Int(-1))),
            Some(SizeClass::Auto)
        );
        assert_eq!(
            size_class(Some(&ConstValue::Int(0))),
            Some(SizeClass::Fixed(1, false))
        );
        assert_eq!(
            size_class(Some(&ConstValue::Int(-5))),
            Some(SizeClass::Fixed(5, true))
        );
        assert_eq!(
            size_class(Some(&ConstValue::Int(100))),
            Some(SizeClass::Fixed(100, false))
        );
        assert_eq!(
            size_class(Some(&ConstValue::Int(1_i64 << 31))),
            Some(SizeClass::Edge(1))
        );
        assert_eq!(
            size_class(Some(&ConstValue::Int(-(1_i64 << 31)))),
            Some(SizeClass::Fixed(2_147_483_647, true))
        );
        assert_eq!(
            size_class(Some(&ConstValue::Float(1e20))),
            Some(SizeClass::Toxic)
        );
        assert_eq!(
            size_class(Some(&ConstValue::Float(f64::INFINITY))),
            Some(SizeClass::Inf)
        );
        assert_eq!(
            size_class(Some(&ConstValue::Float(f64::NEG_INFINITY))),
            Some(SizeClass::Toxic)
        );
        assert_eq!(size_class(Some(&ConstValue::Float(f64::NAN))), None);
        assert_eq!(size_class(Some(&ConstValue::Str("abc".to_string()))), None);

        assert_eq!(declared_size(Some(&ConstValue::Int(-1)), false, 4), None);
        assert_eq!(declared_size(Some(&ConstValue::Int(0)), false, 4), Some(1));
        assert_eq!(declared_size(Some(&ConstValue::Int(-5)), false, 4), None);
        assert_eq!(
            declared_size(Some(&ConstValue::Int(100)), false, 4),
            Some(100)
        );
        assert_eq!(
            declared_size(Some(&ConstValue::Int(1_i64 << 31)), false, 4),
            None
        );
        assert_eq!(
            declared_size(Some(&ConstValue::Float(1e20)), false, 4),
            None
        );
        assert_eq!(
            declared_size(Some(&ConstValue::Float(f64::INFINITY)), false, 4),
            Some(2_147_483_647)
        );

        assert_eq!(declared_size(Some(&ConstValue::Int(-1)), true, 4), None);
        assert_eq!(declared_size(Some(&ConstValue::Int(-5)), true, 4), Some(5));
        assert_eq!(
            declared_size(Some(&ConstValue::Int(1_i64 << 31)), true, 4),
            Some(2_147_483_647)
        );
        assert_eq!(
            declared_size(Some(&ConstValue::Int(-(1_i64 << 31))), true, 4),
            Some(2_147_483_647)
        );
        assert_eq!(
            declared_size(Some(&ConstValue::Float(1e20)), true, 4),
            Some(409)
        );
        assert_eq!(
            declared_size(Some(&ConstValue::Float(f64::INFINITY)), true, 4),
            Some(2_147_483_647)
        );
    }

    #[test]
    fn dtype_unit_const_is_int_and_stored_len() {
        let env = Env::new(&profiles::PROFILES[0]);
        let fp2 = Expr::Name(Name {
            node: Node::default(),
            name: "fp2".to_string(),
        });
        assert_eq!(_dtype_code(Some(&fp2), &env), 7);

        let num7 = Expr::Number(Number {
            node: Node::default(),
            value: NumberValue::Int(7),
            text: "7".to_string(),
        });
        assert_eq!(_dtype_code(Some(&num7), &env), 7);

        let num75 = Expr::Number(Number {
            node: Node::default(),
            value: NumberValue::Float(7.5),
            text: "7.5".to_string(),
        });
        assert_eq!(_dtype_code(Some(&num75), &env), 9);

        let min_name = Expr::Name(Name {
            node: Node::default(),
            name: "Min".to_string(),
        });
        assert_eq!(_unit_value(Some(&min_name), &env), Some(3));
        assert_eq!(_scan_unit_name(Some(&min_name), &env), "MIN");

        let num_neg = Expr::Number(Number {
            node: Node::default(),
            value: NumberValue::Float(-0.5),
            text: "-0.5".to_string(),
        });
        assert_eq!(_unit_value(Some(&num_neg), &env), Some(0));
        assert_eq!(_scan_unit_name(Some(&num_neg), &env), "USEC");
        assert_eq!(_scan_unit_name(None, &env), "SEC");

        let mut int_names = OrderedMap::new();
        int_names.insert("FCONST".to_string(), false);
        assert!(_const_is_int(Some(&num7), &env, &int_names));
        assert!(!_const_is_int(Some(&num75), &env, &int_names));
        let div = Expr::Binary(Binary {
            node: Node::default(),
            op: "/".to_string(),
            left: Some(Rc::new(num7.clone())),
            right: Some(Rc::new(num7.clone())),
        });
        assert!(!_const_is_int(Some(&div), &env, &int_names));
        let neg = Expr::Unary(Unary {
            node: Node::default(),
            op: "-".to_string(),
            operand: Some(Rc::new(num7)),
        });
        assert!(_const_is_int(Some(&neg), &env, &int_names));
        let fref = Expr::Name(Name {
            node: Node::default(),
            name: "fconst".to_string(),
        });
        assert!(!_const_is_int(Some(&fref), &env, &int_names));

        assert_eq!(_string_stored_len(1), 4);
        assert_eq!(_string_stored_len(4), 8);
        assert_eq!(_string_stored_len(5), 8);
        assert_eq!(_string_stored_len(20), 24);
        assert_eq!(_string_stored_len(24), 28);
    }

    #[test]
    fn status_ch400_and_cpi_tables_match_python() {
        let s0 = _status_table(0);
        let s1 = _status_table(1);
        let s3 = _status_table(3);
        assert_eq!(s0.fields.len(), 52);
        assert_eq!(s1.fields.len(), 56);
        assert_eq!(s3.fields.len(), 56);
        let s1_slow: Vec<_> = s1
            .fields
            .iter()
            .filter(|f| f.name.contains("Slow"))
            .map(|f| (f.name.as_str(), f.dimension, f.subdims.clone()))
            .collect();
        assert_eq!(
            s1_slow,
            vec![
                ("SkippedSlowScan", 1, vec![]),
                ("LastSlowScan", 1, vec![]),
                ("SlowProcTime", 1, vec![]),
                ("MaxSlowProcTime", 1, vec![]),
            ]
        );
        let s3_slow: Vec<_> = s3
            .fields
            .iter()
            .filter(|f| f.name.contains("Slow"))
            .map(|f| (f.name.as_str(), f.dimension, f.subdims.clone()))
            .collect();
        assert_eq!(
            s3_slow,
            vec![
                ("SkippedSlowScan", 3, vec![3]),
                ("LastSlowScan", 3, vec![3]),
                ("SlowProcTime", 3, vec![3]),
                ("MaxSlowProcTime", 3, vec![3]),
            ]
        );

        let ch = _ch400status_table();
        assert_eq!(ch.fields.len(), 32);
        assert_eq!(ch.fields[4].r#type, 11);
        assert_eq!(ch.fields[4].dimension, 28);
        assert_eq!(
            pystr::encode_latin1(&ch.fields[27].units),
            Some(vec![0xef, 0xbf, 0xbd, b'C'])
        );

        let cpi = _cpistatus_table();
        assert_eq!(cpi.fields.len(), 8);
        let last = cpi.fields.last().unwrap();
        assert_eq!(last.name, "ModuleInfo");
        assert_eq!(last.dimension, 6400);
        assert_eq!(last.subdims, vec![50, 128]);
    }

    #[test]
    fn scan_program_weights_calls_struct_tables_and_aliases() {
        let src = r#"
Const N = 3
Const TNAME = "MyDynTable"
StructureType MyStruct
    a As Long
    b As String * 10
    Units a = mV
    ReadOnly a
EndStructureType
Public s(2) As MyStruct
Public x As Float
Public arr(2, 3) As Long
Dim d As String
ConstTable MyConsts, 1
    Const K = 42
EndConstTable
DataTable(TNAME, True, -1)
    Sample(1, x, FP2)
EndTable
DataTable(Other, True, 100)
    Sample(1, x, IEEE4)
EndTable
Alias arr(2, 1) = a21
Alias arr(3) = a3
BeginProg
    Scan(5, Sec, 0, 0)
        If x > 0 Then CallTable MyDynTable
    NextScan
    SlowSequence
        Scan(2, Min, 0, 0)
            CallTable Other
            CallTable MyDynTable
        NextScan
    SlowSequence
EndProg
"#;
        let profile = &profiles::PROFILES[0];
        let (prog, _) = crate::parser::parse_text(src, "<test>.CR6", None, Some(profile));
        let env = Env::new(profile);
        let sc = _scan_program(&prog);

        assert_eq!(
            sc.called.iter().cloned().collect::<Vec<_>>(),
            vec!["MYDYNTABLE", "OTHER"]
        );
        assert_eq!(sc.slow_seq_count, 2);
        assert_eq!(
            sc.publics
                .iter()
                .map(|v| v.name.as_str())
                .collect::<Vec<_>>(),
            vec!["x", "arr"]
        );
        assert_eq!(
            sc.all_vars
                .iter()
                .map(|v| v.name.as_str())
                .collect::<Vec<_>>(),
            vec!["s", "x", "arr", "d"]
        );
        assert_eq!(sc.const_tables.len(), 1);
        assert_eq!(sc.const_tables[0].name.as_deref(), Some("MyConsts"));
        assert_eq!(sc.const_tables[0].hidden, 1);
        assert_eq!(sc.const_tables[0].consts.len(), 1);

        assert_eq!(sc.data_tables.len(), 2);
        assert_eq!(sc.data_tables[0].name, "MyDynTable");
        assert_eq!(sc.data_tables[0].line, 17);
        assert_eq!(sc.data_tables[1].name, "Other");
        assert_eq!(sc.data_tables[1].line, 20);

        let weights = _scan_weights(&prog.statements, &env);
        assert_eq!(
            weights
                .iter()
                .map(|(k, v)| (k.as_str(), *v))
                .collect::<Vec<_>>(),
            vec![("OTHER", 120.0), ("MYDYNTABLE", 120.0)]
        );

        let calls = _scan_calls(&prog.statements, &env);
        assert_eq!(
            calls.get("MYDYNTABLE").cloned(),
            Some(vec![Some(5.0), Some(120.0)])
        );
        assert_eq!(calls.get("OTHER").cloned(), Some(vec![Some(120.0)]));

        let last_call = _last_call_interval(&prog.statements, &env);
        assert_eq!(last_call.get("MYDYNTABLE").copied(), Some(Some(120.0)));
        assert_eq!(last_call.get("OTHER").copied(), Some(Some(120.0)));

        let st_tables = _build_struct_tables(&sc.all_vars, &sc.struct_types, &env);
        assert_eq!(st_tables.len(), 1);
        assert_eq!(st_tables[0].name, "s");
        assert_eq!(st_tables[0].size, 2);
        assert_eq!(st_tables[0].fields.len(), 2);
        assert_eq!(st_tables[0].fields[0].name, "a");
        assert_eq!(st_tables[0].fields[0].r#type, 6);
        assert!(st_tables[0].fields[0].read_only);
        assert_eq!(st_tables[0].fields[0].units, "mV");
        assert_eq!(st_tables[0].fields[0].dimension, 1);
        assert_eq!(st_tables[0].fields[1].name, "b");
        assert_eq!(st_tables[0].fields[1].r#type, 11);
        assert_eq!(st_tables[0].fields[1].dimension, 12);
        assert_eq!(st_tables[0].fields[1].subdims, vec![12]);

        let mut vi = OrderedMap::new();
        for v in &sc.all_vars {
            vi.insert(pystr::upper(&v.name), _make_var_info(v, &env));
        }
        assert_eq!(vi.get("ARR").unwrap().collapsed(), (6, vec![2, 3]));
        assert_eq!(vi.get("D").unwrap().collapsed(), (24, vec![24]));

        assert_eq!(
            _resolve_alias_target(sc.alias_stmts[0].args[0].as_deref(), Some(&vi)),
            ("ARR".to_string(), Some(4))
        );
        assert_eq!(
            _resolve_alias_target(sc.alias_stmts[1].args[0].as_deref(), Some(&vi)),
            ("ARR".to_string(), Some(3))
        );
    }
}
