//! Port of `tables.py` (lines 746-1526).

#![allow(
    dead_code,
    unused_variables,
    unused_imports,
    clippy::used_underscore_items
)]

use std::collections::BTreeSet;
use std::rc::Rc;

use super::TdfTable;
use super::{ConstTableDecl, VarInfo};
use crate::consteval;
use crate::diagnostics::{DiagRef, Diagnostic, Diagnostics};
use crate::profiles::Profile;
use crate::pystr;
use crate::syntax::{
    Assignment, Call, Case, ConstDecl, ConstValue, Declaration, Expr, ExprRef, Index, Instruction,
    Juxtaposed, Member, Node, Number, NumberValue, OrderedMap, Program, ProgramRef, Stmt, StmtRef,
    SubDef, Unparsed, VarDecl,
};
use crate::tdf::TdfField;

/// Local mirror of `tables.py::TYPE_CODES` until `part1` exports it.
fn type_code_by_name(name_upper: &str) -> Option<i64> {
    match name_upper {
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

/// Local mirror of `tables.py::_UNIT_SECONDS` until `part1` exports it.
fn unit_seconds(unit_upper: &str) -> f64 {
    match unit_upper {
        "USEC" => 1e-6,
        "MSEC" => 1e-3,
        "MIN" => 60.0,
        "HR" => 3600.0,
        "DAY" => 86400.0,
        _ => 1.0,
    }
}

/// Local mirror of `tables.py::_fold` until `part1` exports the full signature.
fn p1_fold(expr: Option<&Expr>, env: &consteval::Env) -> Option<ConstValue> {
    consteval::value_of(expr, env).ok()
}

/// Helper to convert a `ConstValue` to `f64` (Python `float(v)`).
fn const_to_f64(v: &ConstValue) -> Option<f64> {
    match v {
        #[allow(clippy::cast_precision_loss, reason = "Python int to float")]
        ConstValue::Int(i) => Some(*i as f64),
        ConstValue::Float(f) | ConstValue::CFloat(f) => Some(*f),
        ConstValue::BigInt(s) => s.parse::<f64>().ok().filter(|f| f.is_finite()),
        ConstValue::Str(s) => pystr::parse_float(s),
    }
}

/// Helper to convert a `ConstValue` to `i64` (Python `int(v)`).
fn const_to_i64(v: &ConstValue) -> Option<i64> {
    match v {
        ConstValue::Int(i) => Some(*i),
        ConstValue::Float(f) | ConstValue::CFloat(f) => pystr::py_int_from_f64(*f),
        ConstValue::BigInt(s) | ConstValue::Str(s) => pystr::parse_int(s),
    }
}

/// Helper to evaluate an expression as `f64` with a default.
fn fold_f64(expr: Option<&Expr>, env: &consteval::Env, default: f64) -> f64 {
    p1_fold(expr, env)
        .as_ref()
        .and_then(const_to_f64)
        .unwrap_or(default)
}

/// Local mirror of `tables.py::_as_int` until `part1` exports the full signature.
fn p1_as_int(v: Option<&ConstValue>, default: i64) -> i64 {
    let Some(v) = v else {
        return default;
    };
    match v {
        ConstValue::Int(i) => *i,
        _ => const_to_f64(v).and_then(pystr::py_round).unwrap_or(default),
    }
}

/// Local mirror of `tables.py::_dtype_code` until `part1` exports the full signature.
fn p1_dtype_code(expr: Option<&Expr>, env: &consteval::Env) -> i64 {
    if let Some(Expr::Name(n)) = expr
        && let Some(code) = type_code_by_name(&pystr::upper(&n.name))
    {
        return code;
    }
    if let Some(v) = p1_fold(expr, env)
        && let Some(code) = const_to_i64(&v)
        && matches!(code, 9 | 7 | 6 | 28 | 17 | 11 | 18 | 2 | 14 | 1 | 3)
    {
        return code;
    }
    9
}

/// Local mirror of `tables.py::_string_stored_len` until `part1` exports it.
fn p1_string_stored_len(n: i64) -> i64 {
    (pystr::floordiv_i64(n, 4).unwrap_or(0) + 1) * 4
}

/// Local mirror of `tables.py::_const_is_int` until `part1` exports the full signature.
fn p1_const_is_int(
    expr: Option<&Expr>,
    env: &consteval::Env,
    int_names: &OrderedMap<String, bool>,
) -> bool {
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
            if let Some(sym) = env.profile.symbols().get(&upper) {
                // In profiles.json, integer symbols have `fvalue == value as f64` or `extra == 0`?
                // In Python, `sym.get("value")` is always `int` in `symbols.json` (except NaN/INF).
                // Wait: in `Symbol`, `value` is `i64` and `fvalue` is `f64` (loaded from JSON).
                let _ = sym;
                return true;
            }
            true
        }
        Expr::Unary(u) => {
            if u.op == "NOT" {
                return true;
            }
            p1_const_is_int(u.operand.as_deref(), env, int_names)
        }
        Expr::Binary(b) => {
            if b.op == "/" {
                return false;
            }
            if matches!(b.op.as_str(), "+" | "-" | "*") {
                return p1_const_is_int(b.left.as_deref(), env, int_names)
                    && p1_const_is_int(b.right.as_deref(), env, int_names);
            }
            if matches!(
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
            ) {
                return true;
            }
            false
        }
        _ => true,
    }
}

/// Local mirror of `tables.py::VarInfo` (struct body) until `part1` defines fields.
#[derive(Debug, Clone, Default)]
pub struct StructTypeInfo {
    pub members: Vec<Rc<VarDecl>>,
    pub units: OrderedMap<String, String>,
    pub ro: BTreeSet<String>,
}

/// Local mirror of `tables.py::VarInfo` until `part1::VarInfo` defines fields.
#[derive(Debug, Clone, Default)]
pub struct VarInfoData {
    pub type_code: i64,
    pub shape: Vec<i64>,
    pub stored_len: Option<i64>,
    pub units: String,
    pub struct_info: Option<StructTypeInfo>,
    pub name: String,
}

impl VarInfoData {
    pub fn full_shape(&self) -> Vec<i64> {
        let mut s = self.shape.clone();
        if let Some(sl) = self.stored_len {
            s.push(sl);
        }
        s
    }

    pub fn collapsed(&self) -> (i64, Vec<i64>) {
        let shape = self.full_shape();
        if shape.is_empty() || (shape.len() == 1 && shape[0] == 1) {
            return (1, Vec::new());
        }
        let dim = shape.iter().product();
        (dim, shape)
    }
}

/// Local mirror of `tables.py::ConstTableDecl` until `part1::ConstTableDecl` defines fields.
#[derive(Debug, Clone, Default)]
pub struct ConstTableDeclData {
    pub name: Option<String>,
    pub hidden: i64,
    pub consts: Vec<ConstDecl>,
    pub line: i64,
}

/// Wrapper for `TdfField` carrying the ad-hoc `alias_elem` and `alias_of` attributes
/// attached in `tables.py::_build_public_table` until `TdfField` or a wrapper owns them.
#[derive(Debug, Clone, Default)]
pub struct PublicTdfField {
    pub field: TdfField,
    pub alias_elem: bool,
    pub alias_of: Option<(String, i64)>,
}

/// Return type of `_build_public_table_impl`.
#[derive(Debug, Clone, Default)]
pub struct PublicTableResult {
    pub table: Option<TdfTable>,
    pub fields_meta: Vec<PublicTdfField>,
    pub by_display: OrderedMap<String, PublicTdfField>,
    pub by_base: OrderedMap<String, Vec<PublicTdfField>>,
}

/// Local mirror of `tables.py::_make_var_info` until `part1` exports it.
fn p1_make_var_info(v: &VarDecl, env: &consteval::Env) -> VarInfoData {
    let t = v
        .type_
        .as_deref()
        .map_or_else(|| "FLOAT".to_string(), pystr::upper);
    let type_code = type_code_by_name(&t).unwrap_or(9);
    let mut shape = Vec::with_capacity(v.dims.len());
    for d in &v.dims {
        let n = p1_as_int(p1_fold(Some(d.as_ref()), env).as_ref(), 1);
        shape.push(n.max(1));
    }
    let stored_len = if t == "STRING" {
        let size_expr = match v.string_size.as_deref() {
            Some(Expr::Binary(b)) if b.op == "=" => b.left.as_deref(),
            other => other,
        };
        let n = p1_as_int(p1_fold(size_expr, env).as_ref(), 20);
        Some(p1_string_stored_len(n.max(1)))
    } else {
        None
    };
    VarInfoData {
        type_code,
        shape,
        stored_len,
        units: String::new(),
        struct_info: None,
        name: v.name.clone(),
    }
}

/// Local mirror of `tables.py::_resolve_alias_target` until `part1` exports it.
fn p1_resolve_alias_target(
    expr: Option<&Expr>,
    var_info: Option<&OrderedMap<String, VarInfoData>>,
) -> (String, Option<i64>) {
    let Some(expr) = expr else {
        return (String::new(), None);
    };
    match expr {
        Expr::Name(n) => (pystr::upper(&n.name), None),
        Expr::Index(idx) => {
            if idx.args.len() > 1
                && let Some(vi) = var_info
            {
                let upper = pystr::upper(&idx.name);
                let numbers: Vec<Option<&Number>> = idx
                    .args
                    .iter()
                    .map(|a| match a.as_deref() {
                        Some(Expr::Number(num)) => Some(num),
                        _ => None,
                    })
                    .collect();
                if let Some(info) = vi.get(&upper)
                    && info.shape.len() == idx.args.len()
                    && numbers.iter().all(Option::is_some)
                {
                    let mut flat = 0i64;
                    for (k, num_opt) in numbers.iter().enumerate() {
                        let num = num_opt.unwrap();
                        let stride: i64 = info.shape[k + 1..].iter().product();
                        let val = match &num.value {
                            NumberValue::Int(i) => *i,
                            NumberValue::Float(f) => pystr::py_round(*f).unwrap_or(0),
                            NumberValue::BigInt(s) => pystr::parse_int(s).unwrap_or(0),
                        };
                        flat += (val - 1) * stride;
                    }
                    return (upper, Some(flat + 1));
                }
            }
            if let Some(Some(first)) = idx.args.first()
                && let Expr::Number(num) = first.as_ref()
            {
                let val = match &num.value {
                    NumberValue::Int(i) => *i,
                    NumberValue::Float(f) => pystr::py_round(*f).unwrap_or(0),
                    NumberValue::BigInt(s) => pystr::parse_int(s).unwrap_or(0),
                };
                return (pystr::upper(&idx.name), Some(val));
            }
            (pystr::upper(&idx.name), None)
        }
        _ => (String::new(), None),
    }
}

/// Helper to get `Name` or `Index` name from an `Expr`.
fn expr_name(expr: Option<&Expr>) -> Option<&str> {
    match expr? {
        Expr::Name(n) => Some(&n.name),
        Expr::Index(idx) => Some(&idx.name),
        _ => None,
    }
}

/// Helper to get `getattr(expr, "name", "")` uppercased.
fn expr_name_upper(expr: Option<&Expr>) -> String {
    match expr {
        Some(Expr::Name(n)) => pystr::upper(&n.name),
        Some(Expr::Index(idx)) => pystr::upper(&idx.name),
        _ => String::new(),
    }
}

/// Port of `tables.py::_build_public_table`.
pub fn _build_public_table(
    _publics: &[Rc<VarDecl>],
    _alias_stmts: &[StmtRef],
    _units_stmts: &[StmtRef],
    _readonly_names: &BTreeSet<String>,
) {
    // Full logic is in `_build_public_table_impl`; stub signature omits `var_info` and return value.
}

/// Full implementation of `tables.py::_build_public_table`.
pub fn _build_public_table_impl(
    publics: &[Rc<VarDecl>],
    var_info: &OrderedMap<String, VarInfoData>,
    alias_stmts: &[StmtRef],
    units_stmts: &[StmtRef],
    readonly_names: &BTreeSet<String>,
) -> PublicTableResult {
    if publics.is_empty() {
        return PublicTableResult::default();
    }

    let mut aliased_elements: OrderedMap<String, OrderedMap<i64, String>> = OrderedMap::new();
    let mut whole_alias: OrderedMap<String, String> = OrderedMap::new();
    for s in alias_stmts {
        let Stmt::Instruction(a) = s.as_ref() else {
            continue;
        };
        if a.args.len() < 2 {
            continue;
        }
        let Some(Expr::Name(alias_n)) = a.args[1].as_deref() else {
            continue;
        };
        let (base, idx) = p1_resolve_alias_target(a.args[0].as_deref(), Some(var_info));
        let alias_name = alias_n.name.clone();
        match idx {
            None => {
                whole_alias.insert(base, alias_name);
            }
            Some(i) => {
                if let Some(existing) = aliased_elements.get(&base).cloned() {
                    let mut updated = existing;
                    updated.insert(i, alias_name);
                    aliased_elements.insert(base, updated);
                } else {
                    let mut m = OrderedMap::new();
                    m.insert(i, alias_name);
                    aliased_elements.insert(base, m);
                }
            }
        }
    }

    let mut fields: Vec<PublicTdfField> = Vec::new();
    let mut by_display_idx: OrderedMap<String, usize> = OrderedMap::new();
    let mut by_base_idx: OrderedMap<String, Vec<usize>> = OrderedMap::new();

    for v in publics {
        let base = pystr::upper(&v.name);
        let Some(info) = var_info.get(&base) else {
            continue;
        };
        let display_name = whole_alias
            .get(&base)
            .cloned()
            .unwrap_or_else(|| v.name.clone());
        let ro = readonly_names.contains(&base);

        if let Some(elements) = aliased_elements.get(&base)
            && !info.shape.is_empty()
        {
            let array_size: i64 = info.shape.iter().product();
            let full_shape = info.full_shape();
            let mut group_indices = Vec::new();
            let mut pos = 1i64;
            let es = info.stored_len.unwrap_or(1);
            while pos <= array_size {
                if let Some(elem_alias) = elements.get(&pos) {
                    let f = PublicTdfField {
                        field: TdfField {
                            r#type: info.type_code,
                            read_only: ro,
                            name: elem_alias.clone(),
                            aliases: Vec::new(),
                            processing: String::new(),
                            units: String::new(),
                            description: String::new(),
                            begin_index: 1,
                            dimension: es,
                            subdims: if info.stored_len.is_some() {
                                vec![es]
                            } else {
                                Vec::new()
                            },
                        },
                        alias_elem: true,
                        alias_of: Some((v.name.clone(), pos)),
                    };
                    let idx = fields.len();
                    fields.push(f);
                    group_indices.push(idx);
                    pos += 1;
                } else {
                    let run_start = pos;
                    while pos <= array_size && !elements.contains_key(&pos) {
                        pos += 1;
                    }
                    let run_len = pos - run_start;
                    let f = PublicTdfField {
                        field: TdfField {
                            r#type: info.type_code,
                            read_only: ro,
                            name: v.name.clone(),
                            aliases: Vec::new(),
                            processing: String::new(),
                            units: String::new(),
                            description: String::new(),
                            begin_index: (run_start - 1) * es + 1,
                            dimension: run_len * es,
                            subdims: full_shape.clone(),
                        },
                        alias_elem: false,
                        alias_of: None,
                    };
                    let idx = fields.len();
                    fields.push(f);
                    group_indices.push(idx);
                }
            }
            for &idx in &group_indices {
                by_display_idx.insert(pystr::upper(&fields[idx].field.name), idx);
            }
            by_base_idx.insert(base, group_indices);
            continue;
        }

        let (dim, subdims) = info.collapsed();
        let f = PublicTdfField {
            field: TdfField {
                r#type: info.type_code,
                read_only: ro,
                name: display_name.clone(),
                aliases: Vec::new(),
                processing: String::new(),
                units: String::new(),
                description: String::new(),
                begin_index: 1,
                dimension: dim,
                subdims,
            },
            alias_elem: false,
            alias_of: None,
        };
        let idx = fields.len();
        fields.push(f);
        by_display_idx.insert(pystr::upper(&display_name), idx);
        by_base_idx.insert(base, vec![idx]);
    }

    for s in units_stmts {
        let Stmt::Instruction(u) = s.as_ref() else {
            continue;
        };
        if u.args.len() < 2 {
            continue;
        }
        let Some(Expr::String(label_lit)) = u.args[1].as_deref() else {
            continue;
        };
        let label = &label_lit.value;
        let targets: Vec<usize> = match u.args[0].as_deref() {
            Some(Expr::Index(idx)) if idx.args.is_empty() => by_base_idx
                .get(&pystr::upper(&idx.name))
                .cloned()
                .unwrap_or_default(),
            Some(Expr::Name(n)) => {
                let nm = pystr::upper(&n.name);
                if let Some(idxs) = by_base_idx.get(&nm) {
                    idxs.clone()
                } else if let Some(&idx) = by_display_idx.get(&nm) {
                    vec![idx]
                } else {
                    Vec::new()
                }
            }
            Some(Expr::Index(idx)) => {
                let nm = pystr::upper(&idx.name);
                if let Some(idxs) = by_base_idx.get(&nm) {
                    idxs.clone()
                } else if let Some(&idx) = by_display_idx.get(&nm) {
                    vec![idx]
                } else {
                    Vec::new()
                }
            }
            _ => Vec::new(),
        };
        for idx in targets {
            fields[idx].field.units.clone_from(label);
        }
    }

    let mut by_display = OrderedMap::new();
    for (k, &idx) in by_display_idx.iter() {
        by_display.insert(k.clone(), fields[idx].clone());
    }
    let mut by_base = OrderedMap::new();
    for (k, idxs) in by_base_idx.iter() {
        let group = idxs.iter().map(|&idx| fields[idx].clone()).collect();
        by_base.insert(k.clone(), group);
    }

    let tdf_fields = fields.iter().map(|pf| pf.field.clone()).collect();
    PublicTableResult {
        table: Some(TdfTable {
            name: "Public".to_string(),
            size: 1,
            time_type: 14,
            time_into: (0, 0),
            interval: (0, 0),
            fields: tdf_fields,
        }),
        fields_meta: fields,
        by_display,
        by_base,
    }
}

/// Port of `tables.py::_build_const_tables`.
pub fn _build_const_tables(_const_tables: Vec<ConstTableDecl>) -> Vec<TdfTable> {
    // Full logic is in `_build_const_tables_impl`; stub signature omits `env: &consteval::Env`.
    Vec::new()
}

/// Full implementation of `tables.py::_build_const_tables`.
pub fn _build_const_tables_impl(
    const_tables: &[ConstTableDeclData],
    env: &consteval::Env,
) -> Vec<TdfTable> {
    let mut tables = Vec::new();
    let mut int_names: OrderedMap<String, bool> = OrderedMap::new();
    for ct in const_tables {
        let mut fields = Vec::new();
        for c in &ct.consts {
            let is_str = matches!(c.value.as_deref(), Some(Expr::String(_)));
            if let Some(ty) = &c.type_ {
                let type_code = type_code_by_name(&pystr::upper(ty)).unwrap_or(9);
                let is_ascii = type_code == 11;
                let (dim, subdims) = if is_ascii {
                    (80, vec![80])
                } else {
                    (1, Vec::new())
                };
                fields.push(TdfField {
                    r#type: type_code,
                    read_only: false,
                    name: c.name.clone(),
                    aliases: Vec::new(),
                    processing: String::new(),
                    units: String::new(),
                    description: String::new(),
                    begin_index: 1,
                    dimension: dim,
                    subdims,
                });
                int_names.insert(pystr::upper(&c.name), type_code == 6);
            } else if is_str {
                fields.push(TdfField {
                    r#type: 11,
                    read_only: false,
                    name: c.name.clone(),
                    aliases: Vec::new(),
                    processing: String::new(),
                    units: String::new(),
                    description: String::new(),
                    begin_index: 1,
                    dimension: 80,
                    subdims: vec![80],
                });
                int_names.insert(pystr::upper(&c.name), true);
            } else {
                let is_int = p1_const_is_int(c.value.as_deref(), env, &int_names);
                int_names.insert(pystr::upper(&c.name), is_int);
                let type_code = if is_int { 6 } else { 9 };
                fields.push(TdfField {
                    r#type: type_code,
                    read_only: false,
                    name: c.name.clone(),
                    aliases: Vec::new(),
                    processing: String::new(),
                    units: String::new(),
                    description: String::new(),
                    begin_index: 1,
                    dimension: 1,
                    subdims: Vec::new(),
                });
            }
        }
        let Some(name) = &ct.name else {
            continue;
        };
        fields.push(TdfField {
            r#type: 28,
            read_only: false,
            name: "ApplyAndRestart".to_string(),
            aliases: Vec::new(),
            processing: String::new(),
            units: String::new(),
            description: String::new(),
            begin_index: 1,
            dimension: 1,
            subdims: Vec::new(),
        });
        tables.push(TdfTable {
            name: name.clone(),
            size: 1,
            time_type: 14,
            time_into: (0, 0),
            interval: (0, 0),
            fields,
        });
    }
    tables
}

/// Port of `tables.py::_datatableinfo_table`.
pub fn _datatableinfo_table(n: i64) -> TdfTable {
    let dims = |n: i64| -> Vec<i64> { if n == 1 { Vec::new() } else { vec![n] } };

    let fields = if n == 0 {
        Vec::new()
    } else {
        vec![
            TdfField {
                r#type: 11,
                read_only: true,
                name: "DataTableName".to_string(),
                aliases: Vec::new(),
                processing: String::new(),
                units: String::new(),
                description: String::new(),
                begin_index: 1,
                dimension: 24 * n,
                subdims: vec![n, 24],
            },
            TdfField {
                r#type: 6,
                read_only: false,
                name: "SkippedRecord".to_string(),
                aliases: Vec::new(),
                processing: String::new(),
                units: String::new(),
                description: String::new(),
                begin_index: 1,
                dimension: n,
                subdims: dims(n),
            },
            TdfField {
                r#type: 6,
                read_only: true,
                name: "DataRecordSize".to_string(),
                aliases: Vec::new(),
                processing: String::new(),
                units: "records".to_string(),
                description: String::new(),
                begin_index: 1,
                dimension: 2 * n,
                subdims: vec![n, 2],
            },
            TdfField {
                r#type: 9,
                read_only: true,
                name: "SecsPerRecord".to_string(),
                aliases: Vec::new(),
                processing: String::new(),
                units: String::new(),
                description: String::new(),
                begin_index: 1,
                dimension: n,
                subdims: dims(n),
            },
            TdfField {
                r#type: 9,
                read_only: true,
                name: "DataFillDays".to_string(),
                aliases: Vec::new(),
                processing: String::new(),
                units: "days".to_string(),
                description: String::new(),
                begin_index: 1,
                dimension: 2 * n,
                subdims: vec![n, 2],
            },
            TdfField {
                r#type: 11,
                read_only: true,
                name: "DataFilled".to_string(),
                aliases: Vec::new(),
                processing: String::new(),
                units: String::new(),
                description: String::new(),
                begin_index: 1,
                dimension: 64 * n,
                subdims: vec![n, 2, 32],
            },
            TdfField {
                r#type: 14,
                read_only: true,
                name: "LastPublish".to_string(),
                aliases: Vec::new(),
                processing: String::new(),
                units: String::new(),
                description: String::new(),
                begin_index: 1,
                dimension: n,
                subdims: dims(n),
            },
        ]
    };

    TdfTable {
        name: "DataTableInfo".to_string(),
        size: 1,
        time_type: 14,
        time_into: (0, 0),
        interval: (0, 0),
        fields,
    }
}

/// Port of `tables.py::_source_info`.
pub fn _source_info(_expr: Option<&Expr>) -> Option<VarInfo> {
    // Full logic is in `_source_info_impl`; stub signature omits `var_info`.
    None
}

/// Full implementation of `tables.py::_source_info`.
pub fn _source_info_impl<'a>(
    expr: Option<&Expr>,
    var_info: &'a OrderedMap<String, VarInfoData>,
) -> Option<&'a VarInfoData> {
    let expr = expr?;
    match expr {
        Expr::Name(n) => var_info.get(&pystr::upper(&n.name)),
        Expr::Index(idx) => var_info.get(&pystr::upper(&idx.name)),
        Expr::Member(m) => {
            let base_name = expr_name(m.base.as_deref())?;
            let key = pystr::upper(&format!("{base_name}.{}", m.member));
            var_info.get(&key)
        }
        _ => None,
    }
}

/// Port of `tables.py::_source_name`.
pub fn _source_name(expr: Option<&Expr>) -> String {
    _source_name_impl(expr, None)
}

/// Full implementation of `tables.py::_source_name`.
pub fn _source_name_impl(
    expr: Option<&Expr>,
    var_info: Option<&OrderedMap<String, VarInfoData>>,
) -> String {
    if let Some(vi) = var_info
        && let Some(info) = _source_info_impl(expr, vi)
        && !info.name.is_empty()
    {
        return info.name.clone();
    }
    match expr {
        Some(Expr::Name(n)) => n.name.clone(),
        Some(Expr::Index(idx)) => idx.name.clone(),
        Some(Expr::Member(m)) => m.member.clone(),
        _ => "Anonymous".to_string(),
    }
}

/// Port of `tables.py::_is_anonymous_source`.
pub fn _is_anonymous_source(_expr: Option<&Expr>) -> bool {
    // Full logic is in `_is_anonymous_source_impl`; stub signature omits `var_info` and `env`.
    false
}

/// Full implementation of `tables.py::_is_anonymous_source`.
pub fn _is_anonymous_source_impl(
    expr: Option<&Expr>,
    var_info: &OrderedMap<String, VarInfoData>,
    env: &consteval::Env,
) -> bool {
    match expr {
        Some(Expr::Index(idx)) => !var_info.contains_key(&pystr::upper(&idx.name)),
        Some(Expr::Name(n)) => {
            let upper = pystr::upper(&n.name);
            !var_info.contains_key(&upper) && (p1_fold(expr, env).is_some() || upper == "NAN")
        }
        Some(Expr::Member(_)) | None => false,
        Some(_) => true,
    }
}

/// Port of `tables.py::_anonymous_string_len`.
pub fn _anonymous_string_len(_expr: Option<&Expr>) -> i64 {
    // Full logic is in `_anonymous_string_len_impl`; stub signature omits `var_info` and `env`.
    20
}

/// Full implementation of `tables.py::_anonymous_string_len`.
pub fn _anonymous_string_len_impl(
    expr: Option<&Expr>,
    var_info: &OrderedMap<String, VarInfoData>,
    env: &consteval::Env,
) -> i64 {
    fn known(
        e: Option<&Expr>,
        var_info: &OrderedMap<String, VarInfoData>,
        env: &consteval::Env,
    ) -> Option<i64> {
        if let Some(ConstValue::Str(s)) = p1_fold(e, env) {
            return i64::try_from(pystr::len(&s)).ok();
        }
        if let Some(info) = _source_info_impl(e, var_info) {
            return info.stored_len;
        }
        let e = e?;
        match e {
            Expr::Index(idx) => {
                let fn_rec = env.profile.functions().get(&pystr::upper(&idx.name))?;
                if fn_rec.kind != 312 {
                    return None;
                }
                match fn_rec.code {
                    229 => Some(2),
                    243 => Some(0),
                    254 => Some(104),
                    239 | 251 if !idx.args.is_empty() => {
                        let last = idx.args.last().and_then(|a| a.as_deref());
                        let n = known(last, var_info, env);
                        Some(n.unwrap_or(24) + 60)
                    }
                    252 if !idx.args.is_empty() => known(idx.args[0].as_deref(), var_info, env),
                    250 if idx.args.len() == 3 => {
                        let first = known(idx.args[0].as_deref(), var_info, env);
                        let last = known(idx.args[2].as_deref(), var_info, env);
                        Some(first.unwrap_or(24) - 1 + last.unwrap_or(24))
                    }
                    230 | 237 | 238 | 245 | 246 | 247 | 248 | 249 if !idx.args.is_empty() => {
                        let n = known(idx.args[0].as_deref(), var_info, env);
                        Some(n.unwrap_or(24))
                    }
                    _ => None,
                }
            }
            Expr::Binary(b) if b.op == "+" || b.op == "&" => {
                let l = known(b.left.as_deref(), var_info, env)?;
                let r = known(b.right.as_deref(), var_info, env)?;
                Some(l + r)
            }
            _ => None,
        }
    }

    known(expr, var_info, env).unwrap_or(20)
}

/// Port of `tables.py::_number_anonymous`.
pub fn _number_anonymous(_src: Option<&Expr>, _anon: Option<Vec<i64>>) {
    // Full logic is in `_number_anonymous_impl`; stub signature omits `fields`, `var_info`, `env`.
}

/// Full implementation of `tables.py::_number_anonymous`.
pub fn _number_anonymous_impl(
    fields: &mut [OutField],
    src: Option<&Expr>,
    var_info: &OrderedMap<String, VarInfoData>,
    env: &consteval::Env,
    anon: Option<&mut i64>,
) {
    let Some(anon) = anon else {
        return;
    };
    if !_is_anonymous_source_impl(src, var_info, env) {
        return;
    }
    *anon += 1;
    let raw = _source_name_impl(src, Some(var_info));
    let raw_len = i64::try_from(pystr::len(&raw)).unwrap_or(i64::MAX);
    for f in fields {
        if f.name.starts_with(&raw) {
            let rest = pystr::slice(&f.name, Some(raw_len), None);
            f.name = format!("Anonymous{}{}", *anon, rest);
        }
    }
}

/// Port of `tables.py::_source_shape`.
pub fn _source_shape(_expr: Option<&Expr>, _reps: i64) {
    // Full logic is in `_source_shape_impl`; stub signature omits `var_info`, `env`, and return type.
}

/// Full implementation of `tables.py::_source_shape`.
pub fn _source_shape_impl(
    expr: Option<&Expr>,
    var_info: &OrderedMap<String, VarInfoData>,
    env: &consteval::Env,
    reps: i64,
) -> (i64, Vec<i64>) {
    let Some(expr) = expr else {
        return (1, Vec::new());
    };
    if let Expr::Member(m) = expr
        && let Some(base_name) = expr_name(m.base.as_deref())
    {
        let key = pystr::upper(&format!("{base_name}.{}", m.member));
        if let Some(info) = var_info.get(&key) {
            return (1, info.shape.clone());
        }
    }
    if matches!(expr, Expr::Index(_) | Expr::Name(_)) {
        let name = expr_name(Some(expr)).unwrap_or("");
        let info = var_info.get(&pystr::upper(name));
        if let Expr::Index(idx) = expr
            && info.is_none()
            && (idx.args.len() != 1
                || matches!(
                    idx.args.first().and_then(|a| a.as_deref()),
                    Some(Expr::String(_))
                ))
        {
            return (1, Vec::new());
        }
        let mut array_shape = if let Some(inf) = info
            && !inf.shape.is_empty()
        {
            inf.shape.clone()
        } else if matches!(expr, Expr::Index(_)) {
            vec![reps]
        } else {
            Vec::new()
        };
        if array_shape == [1] {
            array_shape.clear();
        }
        let mut begin = 1i64;
        if let Expr::Index(idx) = expr
            && !idx.args.is_empty()
        {
            let idx_vals: Vec<i64> = idx
                .args
                .iter()
                .map(|a| p1_as_int(p1_fold(a.as_deref(), env).as_ref(), 1))
                .collect();
            if let Some(inf) = info
                && !inf.shape.is_empty()
                && idx_vals.len() == inf.shape.len()
                && inf.shape.len() > 1
            {
                if !idx
                    .args
                    .iter()
                    .all(|a| matches!(a.as_deref(), Some(Expr::Number(_) | Expr::Name(_))))
                {
                    return (1, array_shape);
                }
                begin = 0;
                for (k, &v) in idx_vals.iter().enumerate() {
                    let stride: i64 = inf.shape[k + 1..].iter().product();
                    begin += (v - 1) * stride;
                }
                begin += 1;
            } else {
                begin = idx_vals[0];
            }
        }
        return (begin, array_shape);
    }
    (1, Vec::new())
}

/// Port of `tables.py::_units_for_source`.
pub fn _units_for_source(_expr: Option<&Expr>) -> String {
    // Full logic is in `_units_for_source_impl`; stub signature omits `var_info` and `by_display`.
    String::new()
}

/// Full implementation of `tables.py::_units_for_source`.
pub fn _units_for_source_impl(
    expr: Option<&Expr>,
    var_info: &OrderedMap<String, VarInfoData>,
    by_display: &OrderedMap<String, PublicTdfField>,
) -> String {
    if let Some(Expr::Member(m)) = expr
        && let Some(base_name) = expr_name(m.base.as_deref())
        && let Some(info) = var_info.get(&pystr::upper(base_name))
        && let Some(st) = &info.struct_info
    {
        return st
            .units
            .get(&pystr::upper(&m.member))
            .cloned()
            .unwrap_or_default();
    }
    let name = pystr::upper(&_source_name_impl(expr, Some(var_info)));
    if let Some(f) = by_display.get(&name) {
        return f.field.units.clone();
    }
    var_info
        .get(&name)
        .map(|info| info.units.clone())
        .unwrap_or_default()
}

/// Port of `tables.py::_OutField`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutField {
    pub name: String,
    pub type_code: i64,
    pub read_only: bool,
    pub processing: String,
    pub begin: i64,
    pub dim: i64,
    pub subdims: Vec<i64>,
    pub units: String,
    pub bytes: i64,
    pub renamed: bool,
    pub name_only: bool,
    pub ch400: bool,
    pub placeholder: bool,
    pub rng: Option<(i64, i64)>,
    pub unit_list: Option<Vec<String>>,
    pub elem: i64,
    pub splittable: bool,
    pub struct_members: i64,
    pub member: String,
}

impl Default for OutField {
    fn default() -> Self {
        Self {
            name: String::new(),
            type_code: 0,
            read_only: false,
            processing: String::new(),
            begin: 0,
            dim: 0,
            subdims: Vec::new(),
            units: String::new(),
            bytes: 0,
            renamed: false,
            name_only: false,
            ch400: false,
            placeholder: false,
            rng: None,
            unit_list: None,
            elem: 1,
            splittable: false,
            struct_members: 0,
            member: String::new(),
        }
    }
}

const _FFT_UNBOUNDED: i64 = 1 << 30;

/// Port of `tables.py::_element_range`.
pub fn _element_range(_src: Option<&Expr>, _begin: i64, _dim: i64) {
    // Full logic is in `_element_range_impl`; stub signature omits `env` and return type.
}

/// Full implementation of `tables.py::_element_range`.
pub fn _element_range_impl(
    src: Option<&Expr>,
    begin: i64,
    dim: i64,
    env: &consteval::Env,
) -> Option<(i64, i64)> {
    if let Some(Expr::Index(idx)) = src
        && idx.args.len() > 1
    {
        if dim != 1 {
            return None;
        }
        let mut flat = 0i64;
        for (k, a) in idx.args.iter().enumerate() {
            let v = p1_fold(a.as_deref(), env)?;
            let int_v = match v {
                ConstValue::Int(i) => i,
                ConstValue::Float(f) | ConstValue::CFloat(f) => pystr::py_int_from_f64(f)?,
                ConstValue::BigInt(_) | ConstValue::Str(_) => return None,
            };
            let shift = u32::try_from(20 * k).ok()?;
            flat += int_v.checked_shl(shift)?;
        }
        return Some((flat, flat));
    }
    Some((begin, begin + dim.max(1) - 1))
}

/// Port of `tables.py::_type_bytes`.
pub fn _type_bytes(type_code: i64, dim_for_string: i64) -> i64 {
    if type_code == 11 {
        return dim_for_string;
    }
    match type_code {
        2 | 7 => 2,
        14 | 18 => 8,
        17 | 1 => 1,
        _ => 4,
    }
}

/// Port of `tables.py::_elem_size`.
#[allow(clippy::needless_pass_by_value)]
pub fn _elem_size(f: OutField, _info: &VarInfo) -> i64 {
    // Full logic is in `_elem_size_impl`; `part1::VarInfo` currently has no fields.
    let _ = f;
    1
}

/// Full implementation of `tables.py::_elem_size`.
pub fn _elem_size_impl(f: &OutField, stored_len: Option<i64>) -> i64 {
    match stored_len {
        Some(sl) if sl != 0 && f.type_code == 11 => sl,
        _ => 1,
    }
}

/// Port of `tables.py::_split_by_alias`.
pub fn _split_by_alias(fields: Vec<OutField>) -> Vec<OutField> {
    // Full logic is in `_split_by_alias_impl`; stub signature omits `src`, `var_info`, `by_base`, `env`.
    fields
}

/// Full implementation of `tables.py::_split_by_alias`.
pub fn _split_by_alias_impl(
    fields: Vec<OutField>,
    src: Option<&Expr>,
    var_info: &OrderedMap<String, VarInfoData>,
    by_base: &OrderedMap<String, Vec<PublicTdfField>>,
    env: &consteval::Env,
) -> Vec<OutField> {
    let Some(src_name) = expr_name(src) else {
        return fields;
    };
    let base = pystr::upper(src_name);
    let Some(groups) = by_base.get(&base) else {
        return fields;
    };
    if groups.is_empty() || !groups.iter().any(|g| g.alias_elem) {
        return fields;
    }
    let Some(base_info) = var_info.get(&base) else {
        return fields;
    };
    let declared = &base_info.name;
    let declared_len = i64::try_from(pystr::len(declared)).unwrap_or(i64::MAX);
    let mut out = Vec::new();
    for mut f in fields {
        if f.name_only || f.renamed || !f.name.starts_with(declared) {
            out.push(f);
            continue;
        }
        let suffix = pystr::lstrip_chars(
            pystr::slice(&f.name, Some(declared_len), None),
            "0123456789",
        )
        .to_string();
        let es = _elem_size_impl(&f, base_info.stored_len);
        let lo = f.begin;
        let hi = f.begin + f.dim - 1;
        if !f.splittable {
            let first = (_source_shape_impl(src, var_info, env, 1).0 - 1) * es + 1;
            let mut pos = 1i64;
            for g in groups {
                let n = if g.alias_elem { es } else { g.field.dimension };
                if pos <= first && first < pos + n {
                    if g.alias_elem {
                        f.name = format!("{}{}", g.field.name, suffix);
                    }
                    break;
                }
                pos += n;
            }
            out.push(f);
            continue;
        }
        let mut pos = 1i64;
        for g in groups {
            let (glo, ghi) = if g.alias_elem {
                (pos, pos + es - 1)
            } else {
                (
                    g.field.begin_index,
                    g.field.begin_index + g.field.dimension - 1,
                )
            };
            pos = ghi + 1;
            if ghi < lo || glo > hi {
                continue;
            }
            if g.alias_elem {
                out.push(OutField {
                    name: format!("{}{}", g.field.name, suffix),
                    type_code: f.type_code,
                    read_only: f.read_only,
                    processing: f.processing.clone(),
                    begin: 1,
                    dim: es,
                    subdims: g.field.subdims.clone(),
                    units: g.field.units.clone(),
                    rng: Some((glo, ghi)),
                    ..Default::default()
                });
            } else {
                let a = lo.max(glo);
                let b = hi.min(ghi);
                out.push(OutField {
                    name: f.name.clone(),
                    type_code: f.type_code,
                    read_only: f.read_only,
                    processing: f.processing.clone(),
                    begin: a,
                    dim: b - a + 1,
                    subdims: g.field.subdims.clone(),
                    units: f.units.clone(),
                    rng: Some((a, b)),
                    ..Default::default()
                });
            }
        }
    }
    out
}

/// Port of `tables.py::_fft_field`.
pub fn _fft_field(_s: &Instruction, _anon: Option<Vec<i64>>) -> OutField {
    // Full logic is in `_fft_field_impl`; stub signature omits `var_info`, `by_display`, `env`.
    OutField::default()
}

/// Full implementation of `tables.py::_fft_field`.
pub fn _fft_field_impl(
    s: &Instruction,
    var_info: &OrderedMap<String, VarInfoData>,
    by_display: &OrderedMap<String, PublicTdfField>,
    env: &consteval::Env,
    anon: Option<&mut i64>,
) -> OutField {
    let a = &s.args;
    let src = a.first().and_then(|x| x.as_deref());
    let placeholder = match src {
        None => true,
        Some(Expr::Member(m)) => {
            let bname = expr_name(m.base.as_deref()).unwrap_or("");
            let key = pystr::upper(&format!("{bname}.{}", m.member));
            !var_info.contains_key(&key)
        }
        _ => false,
    };
    if placeholder {
        return OutField {
            name: "Anonymous".to_string(),
            type_code: 9,
            read_only: true,
            processing: "FFT".to_string(),
            begin: 1,
            dim: _FFT_UNBOUNDED,
            subdims: Vec::new(),
            name_only: true,
            placeholder: true,
            rng: Some((1, _FFT_UNBOUNDED)),
            ..Default::default()
        };
    }

    let n = if a.len() > 2 {
        p1_as_int(p1_fold(a[2].as_deref(), env).as_ref(), 1)
    } else {
        1
    };
    let tint = if a.len() > 3 {
        fold_f64(a[3].as_deref(), env, 0.0)
    } else {
        0.0
    };
    let unit = if a.len() > 4 {
        expr_name_upper(a[4].as_deref())
    } else {
        "SEC".to_string()
    };
    let opt = if a.len() > 5 {
        p1_as_int(p1_fold(a[5].as_deref(), env).as_ref(), 0)
    } else {
        0
    };

    let half = pystr::floordiv_i64(n, 2).unwrap_or(0) + 1;
    let k = pystr::pymod_i64(opt, 10).unwrap_or(0);
    let (dim, mut subdims) = match k {
        5 => (n, vec![n]),
        1 | 3 | 4 => (half, vec![half]),
        0 | 2 => (2 * half, vec![half, 2]),
        _ => (2 * half, vec![half]),
    };

    let (first, shape) = _source_shape_impl(src, var_info, env, 1);
    if dim == 1 {
        subdims = shape;
    }

    let tint_sec = tint * unit_seconds(&unit);
    let opt_mod = pystr::pymod_i64(opt, 100).unwrap_or(0);
    let proc = format!("FFT,{n},{},{opt_mod}", pystr::format_e(tint_sec, 4));

    let suffix_idx = if first > 1 {
        first.to_string()
    } else {
        String::new()
    };
    let name = format!("{}{suffix_idx}_FFT", _source_name_impl(src, Some(var_info)));
    let type_code = if a.len() > 1 {
        p1_dtype_code(a[1].as_deref(), env)
    } else {
        9
    };
    let units = _units_for_source_impl(src, var_info, by_display);

    let mut fields = [OutField {
        name,
        type_code,
        read_only: true,
        processing: proc,
        begin: 1,
        dim,
        subdims,
        units,
        rng: Some((1, _FFT_UNBOUNDED)),
        ..Default::default()
    }];
    _number_anonymous_impl(&mut fields, src, var_info, env, anon);
    fields.into_iter().next().unwrap()
}

/// Port of `tables.py::_idx_suffix`.
pub fn _idx_suffix(_src: Option<&Expr>) -> String {
    // Full logic is in `_idx_suffix_impl`; stub signature omits `var_info` and `env`.
    String::new()
}

/// Full implementation of `tables.py::_idx_suffix`.
pub fn _idx_suffix_impl(
    src: Option<&Expr>,
    var_info: &OrderedMap<String, VarInfoData>,
    env: &consteval::Env,
) -> String {
    let (first, _) = _source_shape_impl(src, var_info, env, 1);
    if first > 1 {
        first.to_string()
    } else {
        String::new()
    }
}

/// Port of `tables.py::_alias_source`.
pub fn _alias_source(_instr: &Instruction) {
    // Full logic is in `_alias_source_impl`; stub signature omits `var_info`, `by_display`, `by_base`, and return type.
}

/// Full implementation of `tables.py::_alias_source`.
pub fn _alias_source_impl(
    instr: &Instruction,
    var_info: &OrderedMap<String, VarInfoData>,
    by_display: &OrderedMap<String, PublicTdfField>,
    by_base: Option<&OrderedMap<String, Vec<PublicTdfField>>>,
) -> Instruction {
    if by_base.is_some_and(|m| !m.is_empty())
        && instr.args.len() > 1
        && let Some(Expr::Name(a)) = instr.args[1].as_deref()
    {
        let upper = pystr::upper(&a.name);
        if !var_info.contains_key(&upper)
            && let Some(target) = by_display.get(&upper)
            && let Some((base_name, pos)) = &target.alias_of
        {
            let idx = Expr::Number(Number {
                node: a.node.clone(),
                value: NumberValue::Int(*pos),
                text: pos.to_string(),
            });
            let mut args = instr.args.clone();
            args[1] = Some(Rc::new(Expr::Index(Index {
                node: a.node.clone(),
                name: base_name.clone(),
                args: vec![Some(Rc::new(idx))],
            })));
            let mut replaced = instr.clone();
            replaced.args = args;
            return replaced;
        }
    }
    instr.clone()
}

/// Port of `tables.py::_emit_output_fields`.
pub fn _emit_output_fields(_instr: &Instruction, _anon: Option<Vec<i64>>) -> Vec<OutField> {
    // Full logic is in `_emit_output_fields_impl`; stub signature omits `var_info`, `by_display`, `env`, `by_base`.
    Vec::new()
}

/// Full implementation of `tables.py::_emit_output_fields`.
pub fn _emit_output_fields_impl(
    instr: &Instruction,
    var_info: &OrderedMap<String, VarInfoData>,
    by_display: &OrderedMap<String, PublicTdfField>,
    env: &consteval::Env,
    by_base: Option<&OrderedMap<String, Vec<PublicTdfField>>>,
    anon: Option<&mut i64>,
) -> Vec<OutField> {
    let instr = _alias_source_impl(instr, var_info, by_display, by_base);
    let mut fields = if let Some(f) = _emit_struct_fields_impl(&instr, var_info, env) {
        f
    } else {
        let mut raw = _emit_output_fields_raw_impl(&instr, var_info, by_display, env);
        if instr.args.len() > 1
            && let Some(arg1_name) = expr_name(instr.args[1].as_deref())
            && let Some(info) = var_info.get(&pystr::upper(arg1_name))
            && let Some(reps) = p1_fold(instr.args[0].as_deref(), env)
            && consteval::is_finite_number(&reps)
            && let Some(reps_i) = const_to_i64(&reps)
        {
            for f in &mut raw {
                f.elem = _elem_size_impl(f, info.stored_len);
                f.splittable = f.dim == reps_i * f.elem;
            }
        }
        raw
    };

    let src = instr.args.get(1).and_then(|a| a.as_deref());
    let kw_upper = pystr::upper(&instr.keyword);
    if kw_upper == "SAMPLE"
        && let Some(Expr::Name(n)) = src
        && pystr::upper(&n.name) == "CH400STATUS"
        && !var_info.contains_key("CH400STATUS")
    {
        for f in &mut fields {
            f.ch400 = true;
        }
    }
    for f in &mut fields {
        if f.rng.is_none() && !f.ch400 {
            f.rng = _element_range_impl(src, f.begin, f.dim, env);
        }
    }
    if kw_upper != "HISTOGRAM" {
        _number_anonymous_impl(&mut fields, src, var_info, env, anon);
    }
    fields
}

fn struct_suffix(kw: &str) -> Option<(&'static str, &'static str)> {
    match kw {
        "SAMPLE" => Some(("", "Smp")),
        "AVERAGE" => Some(("_Avg", "Avg")),
        "TOTALIZE" => Some(("_Tot", "Tot")),
        "MAXIMUM" => Some(("_Max", "Max")),
        "MINIMUM" => Some(("_Min", "Min")),
        _ => None,
    }
}

/// Port of `tables.py::_emit_struct_fields`.
pub fn _emit_struct_fields(_instr: &Instruction) {
    // Full logic is in `_emit_struct_fields_impl`; stub signature omits `var_info`, `env`, and return type.
}

/// Full implementation of `tables.py::_emit_struct_fields`.
pub fn _emit_struct_fields_impl(
    instr: &Instruction,
    var_info: &OrderedMap<String, VarInfoData>,
    env: &consteval::Env,
) -> Option<Vec<OutField>> {
    let kw = pystr::upper(&instr.keyword);
    let (suffix, tag) = struct_suffix(&kw)?;
    let a = &instr.args;
    if a.len() < 2 {
        return None;
    }
    let arg1_name = expr_name(a[1].as_deref())?;
    let info = var_info.get(&pystr::upper(arg1_name))?;
    let st = info.struct_info.as_ref()?;
    if matches!(kw.as_str(), "MAXIMUM" | "MINIMUM")
        && a.len() > 4
        && expr_name_upper(a[4].as_deref()) == "TRUE"
    {
        return None;
    }
    let mut reps = p1_as_int(p1_fold(a[0].as_deref(), env).as_ref(), 1);
    if matches!(env.profile.name, "cr300" | "cr350") {
        reps = 1;
    }
    let mut out = Vec::new();
    let member_count = i64::try_from(st.members.len()).unwrap_or(0);
    for _ in 0..reps {
        for m in &st.members {
            let mi = p1_make_var_info(m, env);
            let (dim, subdims) = mi.collapsed();
            let m_upper = pystr::upper(&m.name);
            out.push(OutField {
                name: format!("{}{suffix}", m.name),
                type_code: mi.type_code,
                read_only: st.ro.contains(&m_upper),
                processing: tag.to_string(),
                begin: 1,
                dim,
                subdims,
                units: st.units.get(&m_upper).cloned().unwrap_or_default(),
                struct_members: member_count,
                member: m.name.clone(),
                ..Default::default()
            });
        }
    }
    Some(out)
}

/// Port of `tables.py::_emit_output_fields_raw`.
pub fn _emit_output_fields_raw(_instr: &Instruction) -> Vec<OutField> {
    // Full logic is in `_emit_output_fields_raw_impl`; stub signature omits `var_info`, `by_display`, `env`.
    Vec::new()
}

/// Full implementation of `tables.py::_emit_output_fields_raw`.
pub fn _emit_output_fields_raw_impl(
    instr: &Instruction,
    var_info: &OrderedMap<String, VarInfoData>,
    by_display: &OrderedMap<String, PublicTdfField>,
    env: &consteval::Env,
) -> Vec<OutField> {
    let kw = pystr::upper(&instr.keyword);
    let a = &instr.args;

    let type_of = |idx: usize| -> i64 {
        if idx < a.len() {
            p1_dtype_code(a[idx].as_deref(), env)
        } else {
            9
        }
    };

    let reps_of = || -> i64 {
        if let Some(first) = a.first() {
            p1_as_int(p1_fold(first.as_deref(), env).as_ref(), 1)
        } else {
            1
        }
    };

    let shaped = |src: Option<&Expr>, reps: i64| -> (i64, Vec<i64>, String) {
        let (begin, shape) = _source_shape_impl(src, var_info, env, reps);
        let subdims = if !shape.is_empty() {
            shape
        } else if reps == 1 {
            Vec::new()
        } else {
            vec![reps]
        };
        let units = _units_for_source_impl(src, var_info, by_display);
        (begin, subdims, units)
    };

    if kw == "SAMPLE" {
        let reps = reps_of();
        let src = a.get(1).and_then(|x| x.as_deref());
        let tcode = type_of(2);
        let (mut begin, mut subdims, units) = shaped(src, reps);
        let info = _source_info_impl(src, var_info);
        let mut dim = reps;
        if let Some(inf) = info
            && let Some(ln) = inf.stored_len
            && tcode == 11
        {
            dim = reps * ln;
            subdims = inf.full_shape();
            if let Some(Expr::Index(idx)) = src
                && !idx.args.is_empty()
            {
                begin = (begin - 1) * ln + 1;
            }
        } else if tcode == 11 && _is_anonymous_source_impl(src, var_info, env) {
            dim = p1_string_stored_len(_anonymous_string_len_impl(src, var_info, env));
            subdims = vec![dim];
            begin = (begin.wrapping_sub(1).wrapping_mul(dim).wrapping_add(1)) & 0xFFFF_FFFF;
            return vec![OutField {
                name: _source_name_impl(src, Some(var_info)),
                type_code: tcode,
                read_only: true,
                processing: "Smp".to_string(),
                begin,
                dim,
                subdims,
                units,
                placeholder: true,
                elem: dim,
                splittable: true,
                ..Default::default()
            }];
        }
        return vec![OutField {
            name: _source_name_impl(src, Some(var_info)),
            type_code: tcode,
            read_only: true,
            processing: "Smp".to_string(),
            begin,
            dim,
            subdims,
            units,
            placeholder: !matches!(src, Some(Expr::Name(_) | Expr::Index(_))) && info.is_none(),
            ..Default::default()
        }];
    }

    if matches!(kw.as_str(), "AVERAGE" | "TOTALIZE" | "STDDEV") {
        let reps = reps_of();
        let src = a.get(1).and_then(|x| x.as_deref());
        let tcode = type_of(2);
        let (begin, subdims, units) = shaped(src, reps);
        let (suffix, tag) = match kw.as_str() {
            "AVERAGE" => ("_Avg", "Avg"),
            "TOTALIZE" => ("_Tot", "Tot"),
            _ => ("_Std", "Std"),
        };
        let name = format!("{}{suffix}", _source_name_impl(src, Some(var_info)));
        return vec![OutField {
            name,
            type_code: tcode,
            read_only: true,
            processing: tag.to_string(),
            begin,
            dim: reps,
            subdims,
            units,
            ..Default::default()
        }];
    }

    if matches!(kw.as_str(), "MAXIMUM" | "MINIMUM") {
        let reps = reps_of();
        let src = a.get(1).and_then(|x| x.as_deref());
        let tcode = type_of(2);
        let (begin, subdims, units) = shaped(src, reps);
        let time_flag = a.len() > 4 && expr_name_upper(a[4].as_deref()) == "TRUE";
        let base = _source_name_impl(src, Some(var_info));
        let (vsuf, vtag, tsuf, ttag) = if kw == "MAXIMUM" {
            ("_Max", "Max", "_TMx", "TMx")
        } else {
            ("_Min", "Min", "_TMn", "TMn")
        };
        let mut out = vec![OutField {
            name: format!("{base}{vsuf}"),
            type_code: tcode,
            read_only: true,
            processing: vtag.to_string(),
            begin,
            dim: reps,
            subdims,
            units: units.clone(),
            ..Default::default()
        }];
        if time_flag {
            out.push(OutField {
                name: format!("{base}{tsuf}"),
                type_code: 14,
                read_only: true,
                processing: ttag.to_string(),
                begin: 1,
                dim: reps,
                subdims: if reps == 1 { Vec::new() } else { vec![reps] },
                units,
                ..Default::default()
            });
        }
        return out;
    }

    if kw == "MEDIAN" {
        let reps = reps_of();
        let src = a.get(1).and_then(|x| x.as_deref());
        let tcode = type_of(3);
        let (begin, subdims, units) = shaped(src, reps);
        return vec![OutField {
            name: format!("{}_Med", _source_name_impl(src, Some(var_info))),
            type_code: tcode,
            read_only: true,
            processing: "Med".to_string(),
            begin,
            dim: reps,
            subdims,
            units,
            ..Default::default()
        }];
    }

    if kw == "MOMENT" {
        let reps = reps_of();
        let src = a.get(1).and_then(|x| x.as_deref());
        let tcode = type_of(3);
        let (begin, subdims, units) = shaped(src, reps);
        return vec![OutField {
            name: format!("{}_MMT", _source_name_impl(src, Some(var_info))),
            type_code: tcode,
            read_only: true,
            processing: "MMT".to_string(),
            begin,
            dim: reps,
            subdims,
            units,
            ..Default::default()
        }];
    }

    if kw == "SAMPLEMAXMIN" {
        let reps = reps_of();
        let src = a.get(1).and_then(|x| x.as_deref());
        let tcode = type_of(2);
        let (begin, subdims, units) = shaped(src, reps);
        return vec![OutField {
            name: format!("{}_SMM", _source_name_impl(src, Some(var_info))),
            type_code: tcode,
            read_only: true,
            processing: "SMM".to_string(),
            begin,
            dim: reps,
            subdims,
            units,
            ..Default::default()
        }];
    }

    if kw == "COVARIANCE" {
        let src = a.get(1).and_then(|x| x.as_deref());
        let tcode = type_of(2);
        let n_cov = if a.len() > 4 {
            p1_as_int(p1_fold(a[4].as_deref(), env).as_ref(), 1)
        } else {
            1
        };
        let units = _units_for_source_impl(src, var_info, by_display);
        return vec![OutField {
            name: format!(
                "{}{}_Cov",
                _source_name_impl(src, Some(var_info)),
                _idx_suffix_impl(src, var_info, env)
            ),
            type_code: tcode,
            read_only: true,
            processing: "Cov".to_string(),
            begin: 1,
            dim: n_cov,
            subdims: if n_cov == 1 { Vec::new() } else { vec![n_cov] },
            units,
            ..Default::default()
        }];
    }

    if kw == "WINDVECTOR" {
        let speed = a.get(1).and_then(|x| x.as_deref());
        let tcode = type_of(3);
        let output_opt = if a.len() > 7 {
            p1_as_int(p1_fold(a[7].as_deref(), env).as_ref(), 0)
        } else {
            0
        };
        let dim = match output_opt {
            1 | 4 => 2,
            2 => 4,
            3 => 1,
            _ => 3,
        };
        let units = _units_for_source_impl(speed, var_info, by_display);
        let ulist = match output_opt {
            0 => vec![units.clone(), "Deg".to_string(), "Deg".to_string()],
            1 => vec![units.clone(), "Deg".to_string()],
            2 => vec![
                units.clone(),
                units.clone(),
                "Deg".to_string(),
                "Deg".to_string(),
            ],
            3 => vec!["Deg".to_string()],
            4 => vec!["Deg".to_string(), "Deg".to_string()],
            _ => vec![units.clone(); usize::try_from(dim.max(0)).unwrap_or(0)],
        };
        let first_unit = ulist.first().cloned().unwrap_or_default();
        return vec![OutField {
            name: format!(
                "{}{}_WVc",
                _source_name_impl(speed, Some(var_info)),
                _idx_suffix_impl(speed, var_info, env)
            ),
            type_code: tcode,
            read_only: true,
            processing: "WVc".to_string(),
            begin: 1,
            dim,
            subdims: if dim == 1 { Vec::new() } else { vec![dim] },
            units: first_unit,
            unit_list: Some(ulist),
            ..Default::default()
        }];
    }

    if kw == "HISTOGRAM" {
        let bin_select = a.first().and_then(|x| x.as_deref());
        let tcode = type_of(1);
        let bins = if a.len() > 3 {
            p1_as_int(p1_fold(a[3].as_deref(), env).as_ref(), 1)
        } else {
            1
        };
        let wtval = if a.len() > 5 {
            fold_f64(a[5].as_deref(), env, 0.0)
        } else {
            0.0
        };
        let lolim = if a.len() > 6 {
            fold_f64(a[6].as_deref(), env, 0.0)
        } else {
            0.0
        };
        let uplim = if a.len() > 7 {
            fold_f64(a[7].as_deref(), env, 0.0)
        } else {
            0.0
        };
        let tag = format!(
            "Hst,{bins},{},{},{}",
            pystr::format_e(wtval, 4),
            pystr::format_e(lolim, 4),
            pystr::format_e(uplim, 4)
        );
        let units = _units_for_source_impl(bin_select, var_info, by_display);
        return vec![OutField {
            name: format!(
                "{}{}_Hst",
                _source_name_impl(bin_select, Some(var_info)),
                _idx_suffix_impl(bin_select, var_info, env)
            ),
            type_code: tcode,
            read_only: true,
            processing: tag,
            begin: 1,
            dim: bins,
            subdims: if bins == 1 { Vec::new() } else { vec![bins] },
            units,
            ..Default::default()
        }];
    }

    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser;
    use crate::profiles;

    fn var_decl(name: &str, dims: Vec<ExprRef>, ty: Option<&str>) -> Rc<VarDecl> {
        Rc::new(VarDecl {
            name: name.to_string(),
            dims,
            type_: ty.map(str::to_string),
            ..VarDecl::default()
        })
    }

    fn num_expr(v: i64) -> ExprRef {
        Rc::new(Expr::Number(Number {
            node: Node::default(),
            value: NumberValue::Int(v),
            text: v.to_string(),
        }))
    }

    fn name_expr(s: &str) -> ExprRef {
        Rc::new(Expr::Name(crate::syntax::Name {
            node: Node::default(),
            name: s.to_string(),
        }))
    }

    #[test]
    fn datatableinfo_table_matches_python() {
        let t0 = _datatableinfo_table(0);
        assert_eq!(t0.name, "DataTableInfo");
        assert_eq!(t0.fields, Vec::<TdfField>::new());

        let t1 = _datatableinfo_table(1);
        assert_eq!(t1.fields.len(), 7);
        assert_eq!(t1.fields[0].name, "DataTableName");
        assert_eq!(t1.fields[0].dimension, 24);
        assert_eq!(t1.fields[0].subdims, vec![1, 24]);
        assert_eq!(t1.fields[1].subdims, Vec::<i64>::new());
        assert_eq!(t1.fields[2].units, "records");
        assert_eq!(t1.fields[4].units, "days");
        assert_eq!(t1.fields[5].subdims, vec![1, 2, 32]);

        let t3 = _datatableinfo_table(3);
        assert_eq!(t3.fields.len(), 7);
        assert_eq!(t3.fields[0].dimension, 72);
        assert_eq!(t3.fields[0].subdims, vec![3, 24]);
        assert_eq!(t3.fields[1].dimension, 3);
        assert_eq!(t3.fields[1].subdims, vec![3]);
        assert_eq!(t3.fields[2].dimension, 6);
        assert_eq!(t3.fields[2].subdims, vec![3, 2]);
        assert_eq!(t3.fields[5].dimension, 192);
        assert_eq!(t3.fields[5].subdims, vec![3, 2, 32]);
        assert_eq!(t3.fields[6].r#type, 14);
        assert_eq!(t3.fields[6].subdims, vec![3]);
    }

    #[test]
    fn build_public_table_matches_python() {
        let profile = profiles::get("cr6").expect("cr6 profile");
        let env = consteval::Env::new(profile);
        let (prog, _) = parser::parse_text(
            "Public arr(4) As Float\n\
             Public scalar As Long\n\
             Alias arr(2) = mid_el\n\
             Alias scalar = whole_sc\n\
             Units arr = m/s\n\
             Units whole_sc = deg\n\
             ReadOnly scalar\n",
            "<test>",
            None,
            Some(profile),
        );

        let mut publics = Vec::new();
        let mut alias_stmts = Vec::new();
        let mut units_stmts = Vec::new();
        let mut readonly_names = BTreeSet::new();
        let mut var_info = OrderedMap::new();
        for s in &prog.statements {
            match s.as_ref() {
                Stmt::Declaration(d) => {
                    for v in d.vars.borrow().iter() {
                        var_info.insert(pystr::upper(&v.name), p1_make_var_info(v, &env));
                        publics.push(Rc::clone(v));
                    }
                }
                Stmt::Instruction(ins) => match pystr::upper(&ins.keyword).as_str() {
                    "ALIAS" => alias_stmts.push(Rc::clone(s)),
                    "UNITS" => units_stmts.push(Rc::clone(s)),
                    "READONLY" => {
                        for a in &ins.args {
                            if let Some(name) = expr_name(a.as_deref()) {
                                readonly_names.insert(pystr::upper(name));
                            }
                        }
                    }
                    _ => {}
                },
                _ => {}
            }
        }

        let res = _build_public_table_impl(
            &publics,
            &var_info,
            &alias_stmts,
            &units_stmts,
            &readonly_names,
        );
        let tbl = res.table.expect("Public table");
        assert_eq!(tbl.name, "Public");
        assert_eq!(tbl.fields.len(), 4);
        assert_eq!(
            (
                tbl.fields[0].name.as_str(),
                tbl.fields[0].r#type,
                tbl.fields[0].read_only,
                tbl.fields[0].begin_index,
                tbl.fields[0].dimension,
                &tbl.fields[0].subdims,
                tbl.fields[0].units.as_str(),
            ),
            ("arr", 9, false, 1, 1, &vec![4], "m/s")
        );
        assert_eq!(
            (
                tbl.fields[1].name.as_str(),
                tbl.fields[1].begin_index,
                tbl.fields[1].dimension,
                &tbl.fields[1].subdims,
                tbl.fields[1].units.as_str(),
                res.fields_meta[1].alias_elem,
                res.fields_meta[1].alias_of.as_ref(),
            ),
            (
                "mid_el",
                1,
                1,
                &Vec::<i64>::new(),
                "m/s",
                true,
                Some(&("arr".to_string(), 2)),
            )
        );
        assert_eq!(
            (
                tbl.fields[2].name.as_str(),
                tbl.fields[2].begin_index,
                tbl.fields[2].dimension,
                &tbl.fields[2].subdims,
                tbl.fields[2].units.as_str(),
            ),
            ("arr", 3, 2, &vec![4], "m/s")
        );
        assert_eq!(
            (
                tbl.fields[3].name.as_str(),
                tbl.fields[3].r#type,
                tbl.fields[3].read_only,
                tbl.fields[3].units.as_str(),
            ),
            ("whole_sc", 6, true, "deg")
        );
    }

    #[test]
    fn build_const_tables_matches_python() {
        let profile = profiles::get("cr6").expect("cr6 profile");
        let env = consteval::Env::new(profile);
        let (prog, _) = parser::parse_text(
            "ConstTable(MyCT)\n\
             Const A = 1 + 2\n\
             Const B = 1 / 2\n\
             Const C As Double = 3.14\n\
             Const S = \"hi\"\n\
             EndConstTable\n\
             ConstTable\n\
             Const X = 1\n\
             EndConstTable\n",
            "<test>",
            None,
            Some(profile),
        );
        let mut consts_my = Vec::new();
        let mut consts_bare = Vec::new();
        let mut in_bare = false;
        for s in &prog.statements {
            match s.as_ref() {
                Stmt::Instruction(ins) if pystr::upper(&ins.keyword) == "CONSTTABLE" => {
                    in_bare = ins.args.is_empty();
                }
                Stmt::ConstDecl(c) => {
                    if in_bare {
                        consts_bare.push(c.clone());
                    } else {
                        consts_my.push(c.clone());
                    }
                }
                _ => {}
            }
        }
        let decls = vec![
            ConstTableDeclData {
                name: Some("MyCT".to_string()),
                hidden: 0,
                consts: consts_my,
                line: 1,
            },
            ConstTableDeclData {
                name: None,
                hidden: 0,
                consts: consts_bare,
                line: 7,
            },
        ];
        let tables = _build_const_tables_impl(&decls, &env);
        assert_eq!(tables.len(), 1);
        assert_eq!(tables[0].name, "MyCT");
        let summary: Vec<(&str, i64, i64, &[i64])> = tables[0]
            .fields
            .iter()
            .map(|f| (f.name.as_str(), f.r#type, f.dimension, f.subdims.as_slice()))
            .collect();
        assert_eq!(
            summary,
            vec![
                ("A", 6, 1, &[][..]),
                ("B", 9, 1, &[][..]),
                ("C", 18, 1, &[][..]),
                ("S", 11, 80, &[80][..]),
                ("ApplyAndRestart", 28, 1, &[][..]),
            ]
        );
    }

    #[test]
    fn fft_field_matches_python() {
        let profile = profiles::get("cr6").expect("cr6 profile");
        let env = consteval::Env::new(profile);
        let (prog, _) = parser::parse_text(
            "DataTable(T,1,-1)\n\
             FFT(x,FP2,64,10,Min,0)\n\
             FFT(1.5,IEEE4,16,500,mSec,5)\n\
             EndTable\n",
            "<test>",
            None,
            Some(profile),
        );
        let mut vi = OrderedMap::new();
        vi.insert(
            "X".to_string(),
            VarInfoData {
                type_code: 9,
                shape: vec![64],
                stored_len: None,
                units: "m/s".to_string(),
                struct_info: None,
                name: "X".to_string(),
            },
        );
        let by_display = OrderedMap::new();
        let mut anon = 0i64;
        let ffts: Vec<OutField> = prog
            .statements
            .iter()
            .filter_map(|s| match s.as_ref() {
                Stmt::Instruction(ins) if pystr::upper(&ins.keyword) == "FFT" => Some(
                    _fft_field_impl(ins, &vi, &by_display, &env, Some(&mut anon)),
                ),
                _ => None,
            })
            .collect();
        assert_eq!(ffts.len(), 2);
        assert_eq!(ffts[0].name, "X_FFT");
        assert_eq!(ffts[0].type_code, 7);
        assert_eq!(ffts[0].processing, "FFT,64,6.0000e+02,0");
        assert_eq!(ffts[0].dim, 66);
        assert_eq!(ffts[0].subdims, vec![33, 2]);
        assert_eq!(ffts[0].units, "m/s");

        assert_eq!(ffts[1].name, "Anonymous1_FFT");
        assert_eq!(ffts[1].type_code, 9);
        assert_eq!(ffts[1].processing, "FFT,16,5.0000e-01,5");
        assert_eq!(ffts[1].dim, 16);
        assert_eq!(ffts[1].subdims, vec![16]);
        assert_eq!(ffts[1].units, "");
    }

    #[test]
    fn emit_output_fields_and_split_by_alias_match_python() {
        let profile = profiles::get("cr6").expect("cr6 profile");
        let env = consteval::Env::new(profile);
        let (prog_pub, _) = parser::parse_text(
            "Public arr(4) As Float\n\
             Alias arr(2) = mid_el\n\
             Units arr = m/s\n",
            "<pub>",
            None,
            Some(profile),
        );
        let mut publics = Vec::new();
        let mut alias_stmts = Vec::new();
        let mut units_stmts = Vec::new();
        let mut vi = OrderedMap::new();
        for s in &prog_pub.statements {
            match s.as_ref() {
                Stmt::Declaration(d) => {
                    for v in d.vars.borrow().iter() {
                        vi.insert(pystr::upper(&v.name), p1_make_var_info(v, &env));
                        publics.push(Rc::clone(v));
                    }
                }
                Stmt::Instruction(ins) if pystr::upper(&ins.keyword) == "ALIAS" => {
                    alias_stmts.push(Rc::clone(s));
                }
                Stmt::Instruction(ins) if pystr::upper(&ins.keyword) == "UNITS" => {
                    units_stmts.push(Rc::clone(s));
                }
                _ => {}
            }
        }
        let pub_res =
            _build_public_table_impl(&publics, &vi, &alias_stmts, &units_stmts, &BTreeSet::new());
        vi.insert(
            "WS".to_string(),
            VarInfoData {
                type_code: 9,
                shape: Vec::new(),
                stored_len: None,
                units: "m/s".to_string(),
                struct_info: None,
                name: "ws".to_string(),
            },
        );

        let (prog, _) = parser::parse_text(
            "DataTable(T,1,-1)\n\
             Sample(1,\"abc\" & \"de\",String)\n\
             WindVector(1,ws,wd,FP2,False,0,0,2)\n\
             Histogram(bin_var,IEEE4,0,5,0,1.5,-10,100)\n\
             Sample(4,arr(1),FP2)\n\
             Average(1,mid_el,FP2,False)\n\
             EndTable\n",
            "<test>",
            None,
            Some(profile),
        );
        let instrs: Vec<&Instruction> = prog
            .statements
            .iter()
            .filter_map(|s| match s.as_ref() {
                Stmt::Instruction(ins)
                    if matches!(
                        pystr::upper(&ins.keyword).as_str(),
                        "SAMPLE" | "WINDVECTOR" | "HISTOGRAM" | "AVERAGE"
                    ) =>
                {
                    Some(ins)
                }
                _ => None,
            })
            .collect();

        let mut anon0 = 0;
        let f_str = _emit_output_fields_impl(
            instrs[0],
            &vi,
            &pub_res.by_display,
            &env,
            Some(&pub_res.by_base),
            Some(&mut anon0),
        );
        assert_eq!(f_str.len(), 1);
        assert_eq!(f_str[0].name, "Anonymous1");
        assert_eq!(f_str[0].type_code, 11);
        assert_eq!(f_str[0].dim, 8);
        assert_eq!(f_str[0].subdims, vec![8]);
        assert!(f_str[0].placeholder);

        let f_wv = _emit_output_fields_impl(
            instrs[1],
            &vi,
            &pub_res.by_display,
            &env,
            Some(&pub_res.by_base),
            None,
        );
        assert_eq!(f_wv[0].name, "ws_WVc");
        assert_eq!(f_wv[0].dim, 4);
        assert_eq!(
            f_wv[0].unit_list,
            Some(vec![
                "m/s".to_string(),
                "m/s".to_string(),
                "Deg".to_string(),
                "Deg".to_string()
            ])
        );

        let f_hst = _emit_output_fields_impl(
            instrs[2],
            &vi,
            &pub_res.by_display,
            &env,
            Some(&pub_res.by_base),
            None,
        );
        assert_eq!(f_hst[0].name, "bin_var_Hst");
        assert_eq!(
            f_hst[0].processing,
            "Hst,5,1.5000e+00,-1.0000e+01,1.0000e+02"
        );
        assert_eq!(f_hst[0].dim, 5);

        let f_arr = _emit_output_fields_impl(
            instrs[3],
            &vi,
            &pub_res.by_display,
            &env,
            Some(&pub_res.by_base),
            None,
        );
        let split = _split_by_alias_impl(
            f_arr,
            instrs[3].args[1].as_deref(),
            &vi,
            &pub_res.by_base,
            &env,
        );
        assert_eq!(split.len(), 3);
        assert_eq!(
            (split[0].name.as_str(), split[0].begin, split[0].dim),
            ("arr", 1, 1)
        );
        assert_eq!(
            (
                split[1].name.as_str(),
                split[1].begin,
                split[1].dim,
                split[1].rng
            ),
            ("mid_el", 1, 1, Some((2, 2)))
        );
        assert_eq!(
            (split[2].name.as_str(), split[2].begin, split[2].dim),
            ("arr", 3, 2)
        );

        let f_mid = _emit_output_fields_impl(
            instrs[4],
            &vi,
            &pub_res.by_display,
            &env,
            Some(&pub_res.by_base),
            None,
        );
        assert_eq!(f_mid[0].name, "arr_Avg");
        assert_eq!(f_mid[0].begin, 2);
        assert_eq!(f_mid[0].rng, Some((2, 2)));
    }

    #[test]
    fn emit_struct_fields_matches_python() {
        let cr6 = profiles::get("cr6").expect("cr6");
        let cr300 = profiles::get("cr300").expect("cr300");
        let env_cr6 = consteval::Env::new(cr6);
        let env_cr300 = consteval::Env::new(cr300);

        let mut units = OrderedMap::new();
        units.insert("A".to_string(), "V".to_string());
        let mut ro = BTreeSet::new();
        ro.insert("A".to_string());
        let st = StructTypeInfo {
            members: vec![
                var_decl("a", Vec::new(), Some("Long")),
                var_decl("b", Vec::new(), None),
            ],
            units,
            ro,
        };
        let mut vi = OrderedMap::new();
        vi.insert(
            "S".to_string(),
            VarInfoData {
                type_code: 9,
                shape: vec![2],
                stored_len: None,
                units: String::new(),
                struct_info: Some(st),
                name: "s".to_string(),
            },
        );
        let ins = Instruction {
            keyword: "Average".to_string(),
            args: vec![Some(num_expr(2)), Some(name_expr("s"))],
            ..Instruction::default()
        };
        let out6 = _emit_struct_fields_impl(&ins, &vi, &env_cr6).expect("struct fields");
        assert_eq!(out6.len(), 4);
        assert_eq!(out6[0].name, "a_Avg");
        assert_eq!(out6[0].type_code, 6);
        assert!(out6[0].read_only);
        assert_eq!(out6[0].units, "V");
        assert_eq!(out6[0].struct_members, 2);

        let out300 = _emit_struct_fields_impl(&ins, &vi, &env_cr300).expect("struct fields");
        assert_eq!(out300.len(), 2);

        let ins_max_t = Instruction {
            keyword: "Maximum".to_string(),
            args: vec![
                Some(num_expr(1)),
                Some(name_expr("s")),
                Some(name_expr("FP2")),
                Some(name_expr("False")),
                Some(name_expr("True")),
            ],
            ..Instruction::default()
        };
        assert!(_emit_struct_fields_impl(&ins_max_t, &vi, &env_cr6).is_none());
    }

    #[test]
    fn type_bytes_and_elem_size_and_element_range() {
        assert_eq!(_type_bytes(11, 24), 24);
        assert_eq!(_type_bytes(7, 0), 2);
        assert_eq!(_type_bytes(18, 0), 8);
        assert_eq!(_type_bytes(17, 0), 1);
        assert_eq!(_type_bytes(9, 0), 4);

        let f_str = OutField {
            type_code: 11,
            ..OutField::default()
        };
        assert_eq!(_elem_size_impl(&f_str, Some(24)), 24);
        let f_num = OutField {
            type_code: 9,
            ..OutField::default()
        };
        assert_eq!(_elem_size_impl(&f_num, Some(24)), 1);

        let profile = profiles::get("cr6").expect("cr6");
        let env = consteval::Env::new(profile);
        let idx_2d = Expr::Index(Index {
            node: Node::default(),
            name: "m".to_string(),
            args: vec![Some(num_expr(2)), Some(num_expr(3))],
        });
        assert_eq!(
            _element_range_impl(Some(&idx_2d), 1, 1, &env),
            Some((2 + (3 << 20), 2 + (3 << 20)))
        );
        assert_eq!(_element_range_impl(Some(&idx_2d), 1, 2, &env), None);
    }
}
