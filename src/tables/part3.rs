//! Port of `tables.py` (lines 1527-2177).

#![allow(
    dead_code,
    unused_variables,
    unused_imports,
    clippy::used_underscore_items
)]

use std::collections::{BTreeSet, VecDeque};
use std::rc::Rc;

use super::TdfTable;
use super::{DataTableDecl, OutField};
use crate::checker::storage::{self, AllocResult, TableMeta};
use crate::consteval::{self, Env};
use crate::diagnostics::{DiagRef, Diagnostic, Diagnostics};
use crate::profiles::Profile;
use crate::pystr;
use crate::syntax::{
    Assignment, Call, Case, ConstDecl, ConstValue, Declaration, Expr, ExprRef, Index, Instruction,
    Juxtaposed, Member, Number, OrderedMap, Program, ProgramRef, Stmt, StmtRef, SubDef, Unparsed,
    VarDecl,
};
use crate::tdf::TdfField;

// ---------------------------------------------------------------- constants from tables.py

const _FREE_MEMORY: i64 = 4_194_303;
const _DEFAULT_PROBE_BUFFER_BYTES: i64 = 1024;
const _DEFAULT_PROBE_OVERHEAD: i64 = 16;
const _TABLE_INIT_RESERVED: i64 = 4096;
const _FFT_UNBOUNDED: i64 = 1 << 30;
const _UNIT_MON: i64 = 6;
const _MAX_EXPLODE_COUNT: i64 = 2_000_000;

const _OUTPUT_KEYWORDS: &[&str] = &[
    "SAMPLE",
    "AVERAGE",
    "MAXIMUM",
    "MINIMUM",
    "TOTALIZE",
    "STDDEV",
    "MEDIAN",
    "MOMENT",
    "SAMPLEMAXMIN",
    "HISTOGRAM",
    "WINDVECTOR",
    "COVARIANCE",
];

fn _unit_seconds(unit: &str) -> f64 {
    match unit {
        "USEC" => 1e-6,
        "MSEC" => 1e-3,
        "MIN" => 60.0,
        "HR" => 3600.0,
        "DAY" => 86400.0,
        _ => 1.0,
    }
}

fn _unit_ns(unit: &str) -> i64 {
    match unit {
        "USEC" => 1_000,
        "MSEC" => 1_000_000,
        "MIN" => 60_000_000_000,
        "HR" => 3_600_000_000_000,
        "DAY" => 86_400_000_000_000,
        _ => 1_000_000_000,
    }
}

fn _unit_name_by_value(val: i64) -> Option<&'static str> {
    match val {
        0 => Some("USEC"),
        1 => Some("MSEC"),
        2 => Some("SEC"),
        3 => Some("MIN"),
        4 => Some("HR"),
        5 => Some("DAY"),
        _ => None,
    }
}

fn _type_bytes_lookup(type_code: i64) -> i64 {
    match type_code {
        1 | 17 => 1,
        2 | 7 => 2,
        11 => 0,
        14 | 18 => 8,
        _ => 4,
    }
}

// ---------------------------------------------------------------- local adapters for part1/part2 types

/// Mirror of `tables.py::_OutField` until `part2::OutField` fields are wired.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OutFieldData {
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

impl OutFieldData {
    pub fn new(
        name: impl Into<String>,
        type_code: i64,
        read_only: bool,
        processing: impl Into<String>,
        begin: i64,
        dim: i64,
        subdims: Vec<i64>,
    ) -> Self {
        Self {
            name: name.into(),
            type_code,
            read_only,
            processing: processing.into(),
            begin,
            dim,
            subdims,
            elem: 1,
            ..Self::default()
        }
    }
}

/// Mirror of `tables.py::VarInfo` until `part1::VarInfo` fields are wired.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Part3VarInfoData {
    pub type_code: i64,
    pub shape: Vec<i64>,
    pub stored_len: Option<i64>,
    pub units: String,
    pub name: String,
}

/// Mirror of `tables.py::DataTableDecl` until `part1::DataTableDecl` fields are wired.
#[derive(Debug, Clone, Default)]
pub struct DataTableDeclData {
    pub name: String,
    pub trig: Option<ExprRef>,
    pub size_expr: Option<ExprRef>,
    pub body: Vec<StmtRef>,
    pub line: i64,
}

/// Mirror of `tables.py::_build_data_table` metadata dict.
#[derive(Debug, Clone, Default)]
pub struct DataTableMeta {
    pub own_weight: Option<f64>,
    pub lapses: i64,
    pub zero_interval: bool,
    pub size_val: i64,
    pub size_raw: Option<ConstValue>,
    pub bytes_per_record: i64,
    pub has_event: bool,
    pub fillstop: bool,
    pub interval_seconds: Option<f64>,
    pub has_card: bool,
    pub display_interval: Option<(f64, String)>,
    pub called: bool,
    pub call_w: Option<f64>,
    pub alloc: Option<AllocResult>,
    pub scans: Vec<Option<f64>>,
    // Auto-size scratch fields (`_w`, `_dflt`, `_t8`, `_lapses`, `_reserve`, `_rate`, `_forced`, `_n_raw`).
    pub w: f64,
    pub dflt: i64,
    pub t8: i64,
    pub lapses_eff: i64,
    pub reserve: i64,
    pub rate: f64,
    pub forced: bool,
    pub n_raw: Option<i64>,
}

impl DataTableMeta {
    /// Convert to `checker::storage::TableMeta`.
    pub fn to_storage_meta(&self) -> TableMeta {
        TableMeta {
            size_raw: self.size_raw.clone(),
            called: self.called,
            bytes_per_record: self.bytes_per_record,
            lapses: self.lapses,
            interval_seconds: self.interval_seconds.unwrap_or(0.0),
            zero_interval: self.zero_interval,
            call_w: self.call_w,
            fillstop: self.fillstop,
            display_interval: self.display_interval.is_some(),
            has_card: self.has_card,
            n_raw: self.n_raw,
            alloc: self.alloc.clone(),
        }
    }
}

/// Hooks supplied by `part1`/`part2` for `_build_data_table_with`.
pub trait DataTableHooks {
    fn split_by_alias(
        &self,
        fields: Vec<OutFieldData>,
        src: Option<&ExprRef>,
        env: &Env,
    ) -> Vec<OutFieldData>;
    fn alias_source(&self, instr: &Instruction) -> Instruction;
    fn emit_output_fields(
        &self,
        instr: &Instruction,
        env: &Env,
        anon: &mut i64,
    ) -> Vec<OutFieldData>;
    fn fft_field(&self, instr: &Instruction, env: &Env, anon: &mut i64) -> OutFieldData;
    fn declared_size(
        &self,
        raw: Option<&ConstValue>,
        cr3xx: bool,
        bytes_per_record: i64,
    ) -> Option<i64>;
}

/// Hooks supplied by `part1`/`part2` for `build_tables_with`.
pub trait BuildTablesHooks {
    fn build_all(&self, program: &Program, env: &Env) -> BuildTablesInputs;
}

/// Pre-collected inputs for `build_tables_with`.
#[derive(Debug, Clone, Default)]
pub struct BuildTablesInputs {
    pub tables: Vec<TdfTable>,
    pub interval_displays: Vec<Option<(f64, String)>>,
    pub data_table_indices: Vec<usize>,
    pub data_table_metas: Vec<DataTableMeta>,
    pub scan_weights: OrderedMap<String, f64>,
    pub scan_calls: OrderedMap<String, Vec<Option<f64>>>,
    pub last_call_intervals: OrderedMap<String, Option<f64>>,
    pub has_publics: bool,
    pub named_const_tables: i64,
    pub all_vars: Vec<Rc<VarDecl>>,
    pub struct_types: OrderedMap<String, Vec<Rc<VarDecl>>>,
}

// ---------------------------------------------------------------- local part1 helpers

#[allow(
    clippy::cast_possible_truncation,
    reason = "intentional IEEE-754 float64 to float32 rounding"
)]
fn _f32_local(x: f64) -> f64 {
    f64::from(x as f32)
}

#[allow(
    clippy::cast_precision_loss,
    reason = "matches Python float(int) conversion"
)]
fn i64_to_f64(n: i64) -> f64 {
    n as f64
}

fn _kw_local(s: &Stmt) -> String {
    match s {
        Stmt::Instruction(i) => pystr::upper(&i.keyword),
        _ => String::new(),
    }
}

fn _fold_local(expr: Option<&Expr>, env: &Env) -> Option<ConstValue> {
    let e = expr?;
    consteval::value_of(Some(e), env).ok()
}

fn _unit_value_local(expr: Option<&Expr>, env: &Env) -> Option<i64> {
    let v = _fold_local(expr, env)?;
    match v {
        ConstValue::Int(i) => Some(i),
        ConstValue::Float(f) | ConstValue::CFloat(f) if f.is_finite() => pystr::py_int_from_f64(f),
        _ => None,
    }
}

fn _scan_unit_name_local(expr: Option<&Expr>, env: &Env) -> String {
    match _unit_value_local(expr, env) {
        None => "SEC".to_string(),
        Some(v) => _unit_name_by_value(v).unwrap_or("USEC").to_string(),
    }
}

fn _as_int_local(v: Option<&ConstValue>, default: i64) -> i64 {
    let Some(v) = v else {
        return default;
    };
    let f = match v {
        ConstValue::Int(i) => return *i,
        ConstValue::Float(f) | ConstValue::CFloat(f) => *f,
        ConstValue::BigInt(s) => s.parse::<f64>().unwrap_or(f64::INFINITY),
        ConstValue::Str(s) => match pystr::parse_float(s) {
            Some(f) => f,
            None => return default,
        },
    };
    pystr::py_round(f).unwrap_or(default)
}

fn _const_to_f64(v: &ConstValue) -> f64 {
    match v {
        ConstValue::Int(i) => i64_to_f64(*i),
        ConstValue::Float(f) | ConstValue::CFloat(f) => *f,
        ConstValue::BigInt(s) => s.parse::<f64>().unwrap_or(f64::INFINITY),
        ConstValue::Str(_) => 0.0,
    }
}

fn _const_is_zero(v: &ConstValue) -> bool {
    match v {
        ConstValue::Int(i) => *i == 0,
        ConstValue::Float(f) | ConstValue::CFloat(f) => *f == 0.0,
        ConstValue::BigInt(s) => s.parse::<f64>().is_ok_and(|f| f == 0.0),
        ConstValue::Str(_) => false,
    }
}

fn _string_size_expr_local(v: &VarDecl) -> Option<&Expr> {
    let e = v.string_size.as_deref()?;
    if let Expr::Binary(b) = e
        && b.op == "="
    {
        return b.left.as_deref();
    }
    Some(e)
}

fn _var_size_local(v: &VarDecl, env: &Env) -> (i64, i64) {
    let ty = pystr::upper(v.type_.as_deref().unwrap_or(""));
    let stored_len = if ty == "STRING" {
        let n = _as_int_local(_fold_local(_string_size_expr_local(v), env).as_ref(), 20);
        Some((pystr::floordiv_i64(n.max(1), 4).unwrap_or(0) + 1) * 4)
    } else {
        None
    };
    let mut prod: i64 = 1;
    for d in &v.dims {
        let folded = _fold_local(Some(d.as_ref()), env);
        let n = _as_int_local(folded.as_ref(), 1).max(1);
        prod = prod.saturating_mul(n);
    }
    let elem_width = stored_len.unwrap_or_else(|| if ty == "DOUBLE" { 8 } else { 4 });
    (elem_width, prod)
}

// ---------------------------------------------------------------- ported functions (tables.py:1527-2177)

/// Port of `tables.py::_apply_fieldnames`.
pub fn _apply_fieldnames(fields: Vec<OutField>, _arg: &str, _profile_name: &str) -> Vec<OutField> {
    // TODO(wire): delegate to `_apply_fieldnames_with` once `part2::OutField` has fields.
    fields
}

/// Full implementation of `tables.py::_apply_fieldnames`.
pub fn _apply_fieldnames_with(
    mut fields: Vec<OutFieldData>,
    arg: &str,
    profile_name: &str,
) -> Vec<OutFieldData> {
    let parts = pystr::split(arg, ",", None);
    let n_members = fields.first().map_or(0, |f| f.struct_members);
    if n_members > 0 && matches!(profile_name, "cr300" | "cr350") {
        return fields;
    }
    if n_members > 0 {
        let n_mem = usize::try_from(n_members).unwrap_or(1).max(1);
        let raw = pystr::split(arg, ",", None);
        for (k, f) in fields.iter_mut().enumerate() {
            let group = k / n_mem;
            if group < raw.len() {
                let (nm, _, desc) = pystr::partition(raw[group], ":");
                f.name = format!("{nm}_{}", f.member);
                if !desc.is_empty() {
                    f.processing.push(':');
                    f.processing.push_str(desc);
                }
                f.renamed = true;
            }
        }
        return fields;
    }
    if fields.len() == 1 && parts.len() == 1 && fields[0].dim == 1 {
        let (name, _, desc) = pystr::partition(parts[0], ":");
        fields[0].name = pystr::strip(name).to_string();
        fields[0].renamed = true;
        fields[0].placeholder = false;
        fields[0].begin = 1;
        fields[0].subdims.clear();
        if !desc.is_empty() {
            fields[0].processing.push(':');
            fields[0].processing.push_str(desc);
        }
        return fields;
    }
    if fields.iter().any(|f| f.dim > 1 && f.splittable) || (fields.len() == 1 && fields[0].dim > 1)
    {
        let single_field = fields.len() == 1;
        let mut out = Vec::new();
        let mut names: VecDeque<&str> = parts.into_iter().collect();
        for mut base in fields {
            if names.is_empty() {
                out.push(base);
                continue;
            }
            if !(base.dim > 1 && (base.splittable || single_field)) {
                let raw_name = names.pop_front().unwrap_or("");
                let (name, _, desc) = pystr::partition(raw_name, ":");
                base.name = pystr::strip(name).to_string();
                base.renamed = true;
                base.placeholder = false;
                if !desc.is_empty() {
                    base.processing.push(':');
                    base.processing.push_str(desc);
                }
                out.push(base);
                continue;
            }
            let es = if base.splittable { base.elem } else { 1 };
            let max_elems = pystr::floordiv_i64(base.dim, es).unwrap_or(0).max(0);
            let n_named = names
                .len()
                .min(usize::try_from(max_elems).unwrap_or(usize::MAX));
            for i in 0..n_named {
                let raw_name = names.pop_front().unwrap_or("");
                let (name, _, desc) = pystr::partition(raw_name, ":");
                let mut proc = base.processing.clone();
                if !desc.is_empty() {
                    proc.push(':');
                    proc.push_str(desc);
                }
                let idx = i64::try_from(i).unwrap_or(i64::MAX);
                let rng = base
                    .rng
                    .map(|(lo, _)| (lo + idx * es, lo + (idx + 1) * es - 1));
                let u = base
                    .unit_list
                    .as_ref()
                    .and_then(|ul| ul.get(i))
                    .cloned()
                    .unwrap_or_else(|| base.units.clone());
                let mut f = OutFieldData::new(
                    pystr::strip(name),
                    base.type_code,
                    base.read_only,
                    proc,
                    1,
                    es,
                    if es > 1 { vec![es] } else { Vec::new() },
                );
                f.units = u;
                f.renamed = true;
                f.name_only = base.name_only;
                f.placeholder = false;
                f.rng = rng;
                out.push(f);
            }
            let n_named_i64 = i64::try_from(n_named).unwrap_or(0);
            let remaining = base.dim - n_named_i64 * es;
            if remaining > 0 {
                let rng = base.rng.map(|(lo, hi)| (lo + n_named_i64 * es, hi));
                let mut rem_f = OutFieldData::new(
                    base.name,
                    base.type_code,
                    base.read_only,
                    base.processing,
                    base.begin + n_named_i64 * es,
                    remaining,
                    base.subdims,
                );
                rem_f.units = base.units;
                rem_f.name_only = base.name_only;
                rem_f.placeholder = base.placeholder;
                rem_f.rng = rng;
                rem_f.splittable = base.splittable;
                rem_f.elem = base.elem;
                out.push(rem_f);
            }
        }
        return out;
    }
    for (f, part) in fields.iter_mut().zip(parts.iter()) {
        let (name, _, desc) = pystr::partition(part, ":");
        f.name = pystr::strip(name).to_string();
        f.renamed = true;
        f.placeholder = false;
        if !desc.is_empty() {
            f.processing.push(':');
            f.processing.push_str(desc);
        }
    }
    fields
}

/// Port of `tables.py::_interval_from_args`.
pub fn _interval_from_args(_into_idx: i64, _val_idx: i64, _unit_idx: i64) {
    // TODO(wire): update signature to `(args: &[Option<ExprRef>], env: &Env, into_idx: usize, val_idx: usize, unit_idx: usize)` and call `_interval_from_args_with`.
}

/// Full implementation of `tables.py::_interval_from_args`.
pub fn _interval_from_args_with(
    args: &[Option<ExprRef>],
    env: &Env,
    into_idx: usize,
    val_idx: usize,
    unit_idx: usize,
) -> ((i64, i64), (i64, i64), f64, String) {
    let unit_val = args
        .get(unit_idx)
        .and_then(|e| e.as_deref())
        .and_then(|e| _unit_value_local(Some(e), env));
    let into_val = args
        .get(into_idx)
        .and_then(|e| e.as_deref())
        .and_then(|e| _fold_local(Some(e), env))
        .filter(consteval::is_finite_number)
        .map_or(0.0, |v| _const_to_f64(&v));
    let mut interval_val = args
        .get(val_idx)
        .and_then(|e| e.as_deref())
        .and_then(|e| _fold_local(Some(e), env))
        .filter(consteval::is_finite_number)
        .map_or(0.0, |v| _const_to_f64(&v));
    if unit_val == Some(_UNIT_MON) {
        return ((0, 0), (0, 0), interval_val, "MON".to_string());
    }
    let mut into_f = into_val;
    let mut unit_v = unit_val;
    if let Some(uv) = unit_v
        && _unit_name_by_value(uv).is_none()
    {
        into_f = 0.0;
        interval_val = 1.0;
        unit_v = Some(2);
    }
    let unit_name = match unit_v {
        None => "SEC",
        Some(uv) => _unit_name_by_value(uv).unwrap_or("SEC"),
    };
    let ns_per_unit = i64_to_f64(_unit_ns(unit_name));
    let into_ns = pystr::py_round(into_f * ns_per_unit).unwrap_or(0);
    let interval_ns = pystr::py_round(interval_val * ns_per_unit).unwrap_or(0);
    let into = pystr::divmod_i64(into_ns, 1_000_000_000).unwrap_or((0, 0));
    let interval = pystr::divmod_i64(interval_ns, 1_000_000_000).unwrap_or((0, 0));
    (into, interval, interval_val, unit_name.to_string())
}

/// Port of `tables.py::_build_data_table`.
pub fn _build_data_table(_dt: &DataTableDecl, _called: &BTreeSet<String>, _anon: Option<Vec<i64>>) {
    // TODO(wire): update signature and delegate to `_build_data_table_with`.
}

/// Full implementation of `tables.py::_build_data_table`.
pub fn _build_data_table_with(
    dt: &DataTableDeclData,
    env: &Env,
    called: &BTreeSet<String>,
    anon: &mut i64,
    hooks: &dyn DataTableHooks,
) -> (TdfTable, DataTableMeta, Vec<OutFieldData>) {
    let mut into = (0, 0);
    let mut interval = (0, 0);
    let mut interval_seconds: Option<f64> = None;
    let mut display_interval: Option<(f64, String)> = None;
    let mut zero_interval = false;
    let mut has_card = false;
    let mut lapses: i64 = 10;
    let mut own_weight: Option<f64> = None;

    let mut out_fields: Vec<OutFieldData> = Vec::new();
    let mut pending: Vec<OutFieldData> = Vec::new();
    let mut pending_src: Option<ExprRef> = None;

    let split_pending = |pending: Vec<OutFieldData>, pending_src: Option<&ExprRef>| {
        if pending_src.is_some() {
            hooks.split_by_alias(pending, pending_src, env)
        } else {
            pending
        }
    };

    for s in &dt.body {
        let kw = _kw_local(s.as_ref());
        if let Stmt::Instruction(instr) = s.as_ref()
            && kw == "DATAINTERVAL"
        {
            let (i_into, i_interval, ival, unit) =
                _interval_from_args_with(&instr.args, env, 0, 1, 2);
            into = i_into;
            interval = i_interval;
            let mut isec = ival * _unit_seconds(&unit);
            display_interval = Some((ival, unit.clone()));
            zero_interval = isec == 0.0;
            if unit == "MON" {
                zero_interval = false;
                display_interval = if ival == 0.0 {
                    None
                } else {
                    Some((ival, unit.clone()))
                };
                interval_seconds = None;
            } else if zero_interval {
                interval_seconds = None;
            } else {
                interval_seconds = Some(isec);
            }
            if unit == "MON" || zero_interval {
                isec = 0.0;
            }
            own_weight = if isec == 0.0 { None } else { Some(isec) };
            if let Some(arg3) = instr.args.get(3).and_then(|a| a.as_deref()) {
                let folded = _fold_local(Some(arg3), env);
                lapses = _as_int_local(folded.as_ref(), 10).abs();
                if let Some(fv) = folded.as_ref()
                    && _const_is_zero(fv)
                {
                    into = (0, 0);
                    interval = (0, 0);
                    interval_seconds = None;
                }
            }
        } else if kw == "DATAEVENT" {
            // interval/into stay (0, 0)
        } else if kw == "CARDOUT" || kw == "TABLEFILE" {
            has_card = true;
        } else if let Stmt::Instruction(instr) = s.as_ref()
            && kw == "FIELDNAMES"
        {
            if let Some(Some(first_arg)) = instr.args.first()
                && let Expr::String(str_lit) = first_arg.as_ref()
                && !pending.is_empty()
            {
                pending = _apply_fieldnames_with(pending, &str_lit.value, env.profile.name);
                out_fields.extend(split_pending(pending, pending_src.as_ref()));
                pending = Vec::new();
                pending_src = None;
            }
        } else if _OUTPUT_KEYWORDS.contains(&kw.as_str())
            && let Stmt::Instruction(instr) = s.as_ref()
        {
            if !pending.is_empty() {
                out_fields.extend(split_pending(pending, pending_src.as_ref()));
            }
            let aliased = hooks.alias_source(instr);
            pending = hooks.emit_output_fields(&aliased, env, anon);
            pending_src = if aliased.args.len() > 1 && !pending.iter().any(|f| f.struct_members > 0)
            {
                aliased.args[1].clone()
            } else {
                None
            };
        } else if let Stmt::Instruction(instr) = s.as_ref()
            && kw == "FFT"
        {
            if !pending.is_empty() {
                out_fields.extend(split_pending(pending, pending_src.as_ref()));
            }
            pending = vec![hooks.fft_field(instr, env, anon)];
            pending_src = None;
        } else if kw == "ETSZ" || kw == "ETCLEARSKY" {
            if !pending.is_empty() {
                out_fields.extend(split_pending(pending, pending_src.as_ref()));
            }
            pending = ["ETrs", "Rso"]
                .iter()
                .map(|&n| {
                    let mut f = OutFieldData::new(n, 7, true, n, 1, 1, Vec::new());
                    f.name_only = true;
                    f.rng = Some((1, _FFT_UNBOUNDED));
                    f
                })
                .collect();
            pending_src = None;
        }
    }
    if !pending.is_empty() {
        out_fields.extend(split_pending(pending, pending_src.as_ref()));
    }

    let mut fields = Vec::new();
    for of in &out_fields {
        if of.name_only {
            continue;
        }
        fields.push(TdfField {
            r#type: of.type_code,
            read_only: of.read_only,
            name: of.name.clone(),
            aliases: Vec::new(),
            processing: of.processing.clone(),
            units: of.units.clone(),
            description: of.processing.clone(),
            begin_index: of.begin,
            dimension: of.dim,
            subdims: of.subdims.clone(),
        });
    }

    let mut size_raw = _fold_local(dt.size_expr.as_deref(), env).or(Some(ConstValue::Int(-1)));
    let cr3xx = env.profile.single_pass;
    if cr3xx {
        let is_str = matches!(size_raw, Some(ConstValue::Str(_)));
        let is_nan_name = dt
            .size_expr
            .as_deref()
            .is_some_and(|e| matches!(e, Expr::Name(n) if pystr::upper(&n.name) == "NAN"));
        let is_h8000 = dt
            .size_expr
            .as_deref()
            .is_some_and(|e| matches!(e, Expr::Number(n) if pystr::upper(&n.text) == "&H80000000"));
        if is_str || is_nan_name || is_h8000 {
            size_raw = Some(ConstValue::Float(f64::NEG_INFINITY));
        }
    }

    let bytes_per_record: i64 = out_fields
        .iter()
        .filter(|of| !of.name_only)
        .map(_field_record_bytes_with)
        .sum();
    let size_val = hooks
        .declared_size(size_raw.as_ref(), cr3xx, bytes_per_record)
        .unwrap_or(-1);

    if interval_seconds.is_some()
        && 2 * bytes_per_record > _DEFAULT_PROBE_BUFFER_BYTES - _DEFAULT_PROBE_OVERHEAD
    {
        into = (0, 0);
        interval = (0, 0);
        interval_seconds = None;
    }

    let has_event = dt.body.iter().any(|s| _kw_local(s.as_ref()) == "DATAEVENT");
    let fillstop = dt.body.iter().any(|s| _kw_local(s.as_ref()) == "FILLSTOP");
    let is_called = called.contains(&pystr::upper(&dt.name));

    let meta = DataTableMeta {
        own_weight,
        lapses,
        zero_interval,
        size_val,
        size_raw,
        bytes_per_record,
        has_event,
        fillstop,
        interval_seconds,
        has_card,
        display_interval,
        called: is_called,
        ..DataTableMeta::default()
    };

    let table = TdfTable {
        name: dt.name.clone(),
        size: 1,
        time_type: 14,
        time_into: into,
        interval,
        fields,
    };

    (table, meta, out_fields)
}

/// Port of `tables.py::_field_record_bytes`.
pub fn _field_record_bytes(_of: OutField) -> i64 {
    // TODO(wire): delegate to `_field_record_bytes_with` once `part2::OutField` has fields.
    0
}

/// Full implementation of `tables.py::_field_record_bytes`.
pub fn _field_record_bytes_with(of: &OutFieldData) -> i64 {
    if of.type_code == 11 {
        return of.dim;
    }
    of.dim * _type_bytes_lookup(of.type_code)
}

/// Port of `tables.py::_table_default_min`.
pub fn _table_default_min(bytes_per_record: i64) -> i64 {
    if bytes_per_record <= 0 {
        return 1;
    }
    pystr::floordiv_i64(
        _DEFAULT_PROBE_BUFFER_BYTES - _DEFAULT_PROBE_OVERHEAD,
        bytes_per_record,
    )
    .unwrap_or(1)
    .max(1)
}

/// Port of `tables.py::_resolve_auto_sizes`.
pub fn _resolve_auto_sizes(_scan_weights: &OrderedMap<String, f64>) {
    // TODO(wire): update signature to `(tables_meta: &mut [(TdfTable, DataTableMeta)], scan_weights: &OrderedMap<String, f64>)` and call `_resolve_auto_sizes_with`.
}

/// Full implementation of `tables.py::_resolve_auto_sizes`.
pub fn _resolve_auto_sizes_with(
    tables_meta: &mut [(TdfTable, DataTableMeta)],
    scan_weights: &OrderedMap<String, f64>,
) {
    let auto_indices: Vec<usize> = tables_meta
        .iter()
        .enumerate()
        .filter(|(_, (_, m))| m.size_val == -1 && m.called)
        .map(|(i, _)| i)
        .collect();
    if auto_indices.is_empty() {
        return;
    }
    let mut reserved = _TABLE_INIT_RESERVED;
    let mut rate = 0.0_f64;
    for &idx in &auto_indices {
        let (ref t, ref mut m) = tables_meta[idx];
        let raw_w = match m.own_weight {
            Some(v) if v != 0.0 => v,
            _ => *scan_weights.get(&pystr::upper(&t.name)).unwrap_or(&1.0),
        };
        let w = _f32_local(raw_w);
        let b = m.bytes_per_record;
        let lapses = if m.interval_seconds.is_some_and(|v| v != 0.0) {
            consteval::to_long(&ConstValue::Int(m.lapses)).abs()
        } else {
            0
        };
        let mut dflt = if lapses != 0 {
            _table_default_min(b)
        } else {
            1
        };
        let mut lapses_eff = lapses;
        if dflt < 2 {
            dflt = 1;
            lapses_eff = 0;
        }
        let t8 = dflt * b + _DEFAULT_PROBE_OVERHEAD;
        let reserve = (pystr::floordiv_i64(2 * _DEFAULT_PROBE_BUFFER_BYTES, t8).unwrap_or(0)
            + i64::from(dflt == 1)
            + if t8 < _DEFAULT_PROBE_BUFFER_BYTES {
                1
            } else {
                3
            })
            * t8;
        let m_rate =
            _f32_local(_f32_local(i64_to_f64(t8)) / _f32_local(_f32_local(i64_to_f64(dflt)) * w));
        m.w = w;
        m.dflt = dflt;
        m.t8 = t8;
        m.lapses_eff = lapses_eff;
        m.reserve = reserve;
        m.rate = m_rate;
        reserved += reserve;
        if lapses_eff != 0 && b != 0 && !m.has_card {
            let w16 = pystr::pymod_i64(lapses_eff.wrapping_mul(16) + (1_i64 << 31), 1_i64 << 32)
                .unwrap_or(0)
                - (1_i64 << 31);
            let q = pystr::py_int_from_f64(i64_to_f64(w16) / i64_to_f64(b)).unwrap_or(0);
            reserved += (q + 1) * b;
        }
        rate = _f32_local(m_rate + rate);
    }
    let mut duration = if rate == 0.0 {
        f64::INFINITY
    } else {
        _f32_local(_f32_local(i64_to_f64(_FREE_MEMORY - reserved)) / rate)
    };
    if rate != 0.0 {
        for &idx in &auto_indices {
            let m = &mut tables_meta[idx].1;
            if duration > 0.0 && duration <= m.w {
                m.forced = true;
                rate = _f32_local(rate - m.rate);
                reserved -= m.reserve;
                duration = if rate == 0.0 {
                    f64::INFINITY
                } else {
                    _f32_local(_f32_local(i64_to_f64(_FREE_MEMORY - reserved)) / rate)
                };
            }
        }
    }
    for &idx in &auto_indices {
        let (ref mut t, ref mut m) = tables_meta[idx];
        if m.forced {
            t.size = 2;
            continue;
        }
        let records = if m.bytes_per_record != 0 {
            pystr::py_int_from_f64(_f32_local(duration / m.w)).unwrap_or(0)
        } else {
            0
        };
        m.n_raw = Some(records);
        t.size = records.max(2).max(m.dflt);
    }
}

/// Port of `tables.py::_apply_sizes`.
pub fn _apply_sizes(_scan_weights: &OrderedMap<String, f64>, _resolve_auto: bool) {
    // TODO(wire): update signature and delegate to `_apply_sizes_with`.
}

/// Full implementation of `tables.py::_apply_sizes`.
pub fn _apply_sizes_with(
    tables_meta: &mut [(TdfTable, DataTableMeta)],
    scan_weights: &OrderedMap<String, f64>,
    resolve_auto: bool,
) {
    for (t, m) in tables_meta.iter_mut() {
        let w = scan_weights.get(&pystr::upper(&t.name)).copied();
        if m.zero_interval
            && m.called
            && let Some(wv) = w
            && wv != 0.0
            && 2 * m.bytes_per_record <= _DEFAULT_PROBE_BUFFER_BYTES - _DEFAULT_PROBE_OVERHEAD
        {
            let ns = pystr::py_round(wv * 1_000_000_000.0).unwrap_or(0);
            t.interval = pystr::divmod_i64(ns, 1_000_000_000).unwrap_or((0, 0));
            m.interval_seconds = Some(wv);
            if let Some((_, ref unit)) = m.display_interval {
                let unit_owned = unit.clone();
                m.display_interval = Some((wv / _unit_seconds(&unit_owned), unit_owned));
            }
        }
        if !m.called {
            t.size = 1;
            t.interval = (0, 0);
            t.time_into = (0, 0);
        } else if m.size_val != -1 {
            t.size = m.size_val;
        }
    }
    if resolve_auto {
        _resolve_auto_sizes_with(tables_meta, scan_weights);
    }
}

/// Port of `tables.py::_register_table_sources`.
pub fn _register_table_sources(_table: &TdfTable) {
    // TODO(wire): update signature to `(table: &TdfTable, var_info: &mut OrderedMap<String, Part3VarInfoData>)` and call `_register_table_sources_with`.
}

/// Full implementation of `tables.py::_register_table_sources`.
pub fn _register_table_sources_with(
    table: &TdfTable,
    var_info: &mut OrderedMap<String, Part3VarInfoData>,
) {
    for f in &table.fields {
        let mut shape = f.subdims.clone();
        let mut stored_len = None;
        if f.r#type == 11 {
            stored_len = Some(shape.pop().unwrap_or(f.dimension));
        } else if shape.is_empty() && f.dimension > 1 {
            shape = vec![f.dimension];
        }
        let key = pystr::upper(&format!("{}.{}", table.name, f.name));
        var_info.insert(
            key,
            Part3VarInfoData {
                type_code: f.r#type,
                shape,
                stored_len,
                units: String::new(),
                name: f.name.clone(),
            },
        );
    }
}

/// Port of `tables.py::build_tables`.
pub fn build_tables(_program: &Program) {
    // TODO(wire): update signature to `(program: &Program, profile: &'static Profile, consts: &OrderedMap<String, ConstValue>) -> (Vec<TdfTable>, String, Option<()>)` and call `build_tables_with`.
}

/// Full implementation of `tables.py::build_tables`.
pub fn build_tables_with(
    program: &Program,
    profile: &'static Profile,
    consts: &OrderedMap<String, ConstValue>,
    hooks: &dyn BuildTablesHooks,
) -> (Vec<TdfTable>, String, Vec<DataTableMeta>) {
    let consts_cell = std::cell::RefCell::new(consts.clone());
    let env = Env::with_consts(profile, Rc::new(consts_cell));
    let mut inputs = hooks.build_all(program, &env);

    let cr3xx = matches!(profile.name, "cr300" | "cr350");
    let mut paired: Vec<(TdfTable, DataTableMeta)> = inputs
        .data_table_indices
        .iter()
        .zip(inputs.data_table_metas)
        .map(|(&idx, mut m)| {
            let t = inputs.tables[idx].clone();
            if cr3xx {
                let last_w = inputs
                    .last_call_intervals
                    .get(&pystr::upper(&t.name))
                    .copied()
                    .flatten();
                m.call_w = m.own_weight.or(last_w);
            }
            (t, m)
        })
        .collect();

    _apply_sizes_with(&mut paired, &inputs.scan_weights, !cr3xx);

    let mut alloc: Option<AllocResult> = None;
    if cr3xx && !paired.is_empty() {
        let pool =
            storage::pool_bytes(
                profile.name,
                i64::try_from(paired.len()).unwrap_or(0),
                inputs.has_publics,
                inputs.named_const_tables,
            ) - _preserve_deduction_with(program, &env, &inputs.all_vars, &inputs.struct_types);
        let storage_metas: Vec<TableMeta> =
            paired.iter().map(|(_, m)| m.to_storage_meta()).collect();
        alloc = storage::run_with(&storage_metas, pool);
        if let Some(ref res) = alloc {
            for ((t, m), &n) in paired.iter_mut().zip(res.sizes.iter()) {
                if m.called && (m.size_val == -1 || n > t.size) {
                    t.size = n;
                }
            }
        } else {
            for (t, m) in &mut paired {
                if m.called && m.size_val == -1 {
                    t.size = 1;
                }
            }
        }
    }

    let mut out_metas = Vec::with_capacity(paired.len());
    for (i, (t, mut m)) in paired.into_iter().enumerate() {
        let tbl_idx = inputs.data_table_indices[i];
        inputs.interval_displays[tbl_idx].clone_from(&m.display_interval);
        inputs.tables[tbl_idx] = t;
        if let Some(ref res) = alloc {
            m.alloc = Some(res.clone());
        }
        m.scans = inputs
            .scan_calls
            .get(&pystr::upper(&inputs.tables[tbl_idx].name))
            .cloned()
            .unwrap_or_default();
        out_metas.push(m);
    }

    let table_list_text = render_table_list_with(&inputs.tables, &inputs.interval_displays);
    (inputs.tables, table_list_text, out_metas)
}

/// Port of `tables.py::_preserve_deduction`.
pub fn _preserve_deduction(_program: &Program) -> i64 {
    // TODO(wire): update signature to `(program: &Program, env: &Env, all_vars: &[Rc<VarDecl>], struct_types: &OrderedMap<String, Vec<Rc<VarDecl>>>)` and call `_preserve_deduction_with`.
    0
}

/// Full implementation of `tables.py::_preserve_deduction`.
pub fn _preserve_deduction_with(
    program: &Program,
    env: &Env,
    all_vars: &[Rc<VarDecl>],
    struct_types: &OrderedMap<String, Vec<Rc<VarDecl>>>,
) -> i64 {
    let stmts = &program.statements;
    let kws: Vec<String> = stmts.iter().map(|s| _kw_local(s.as_ref())).collect();
    if !kws
        .iter()
        .any(|k| k == "PRESERVEVARIABLES" || k == "PRESERVEONEVARIABLE")
    {
        return 0;
    }
    let mut ticks: Option<i64> = None;
    let mut slow = false;
    for (s, kw) in stmts.iter().zip(kws.iter()) {
        if kw == "SLOWSEQUENCE" {
            slow = true;
        } else if let Stmt::Instruction(instr) = s.as_ref()
            && kw == "SCAN"
        {
            if slow {
                if ticks.is_none() {
                    ticks = Some(600_000);
                }
            } else if let Some(Some(arg0)) = instr.args.first()
                && let Some(val) = _fold_local(Some(arg0.as_ref()), env)
                && consteval::is_finite_number(&val)
            {
                let fval = _const_to_f64(&val);
                if fval > 0.0 {
                    let unit = instr.args.get(1).and_then(|a| a.as_deref()).map_or_else(
                        || "SEC".to_string(),
                        |e| _scan_unit_name_local(Some(e), env),
                    );
                    let ns = i64_to_f64(_unit_ns(&unit));
                    let t = pystr::py_int_from_f64((fval * ns) / 100_000.0).unwrap_or(0);
                    ticks = Some(match ticks {
                        None => t,
                        Some(prev) => prev.min(t),
                    });
                }
            }
        }
    }
    let ticks = match ticks {
        None => 100_000,
        Some(t) => t.min(600_000),
    };

    let mut total: i64 = 0;
    if kws.iter().any(|k| k == "PRESERVEVARIABLES") {
        let mut pub_bytes: i64 = 16;
        for v in all_vars {
            let ty = pystr::upper(v.type_.as_deref().unwrap_or(""));
            if !struct_types.contains_key(&ty) {
                let (w, n) = _var_size_local(v, env);
                pub_bytes += w * n;
            }
        }
        for (_, members) in struct_types.iter() {
            for m in members {
                let (w, n) = _var_size_local(m, env);
                pub_bytes += w * n;
            }
        }
        total += storage::preserve_charge(64, 60, 0, 100_000, ticks, pub_bytes);
    }

    let mut by_name: OrderedMap<String, Rc<VarDecl>> = OrderedMap::new();
    for v in all_vars {
        by_name.insert(pystr::upper(&v.name), Rc::clone(v));
    }
    let mut payloads: Vec<i64> = Vec::new();
    for (s, kw) in stmts.iter().zip(kws.iter()) {
        if kw != "PRESERVEONEVARIABLE" {
            continue;
        }
        let Stmt::Instruction(instr) = s.as_ref() else {
            continue;
        };
        let Some(Some(a)) = instr.args.first() else {
            continue;
        };
        let name = match a.as_ref() {
            Expr::Name(n) => &n.name,
            Expr::Index(idx) => &idx.name,
            _ => "",
        };
        let Some(v) = by_name.get(&pystr::upper(name)) else {
            continue;
        };
        let (w, n) = _var_size_local(v, env);
        let whole = match a.as_ref() {
            Expr::Index(idx) => !idx.args.iter().any(Option::is_some),
            _ => true,
        };
        payloads.push(if whole { w * n } else { w });
    }
    if let Some(&top) = payloads.iter().max() {
        let count = i64::try_from(payloads.len()).unwrap_or(i64::MAX);
        total += storage::preserve_charge(top + 78, top, count, 20_000, ticks, 0);
    }
    total
}

/// Port of `tables.py::_explode_indices`.
pub fn _explode_indices(_subdims: Vec<i64>, _drop_last: bool, _begin: i64, _dim: i64) {
    // TODO(wire): update return type to `Vec<Vec<i64>>` and delegate to `_explode_indices_with`.
}

/// Full implementation of `tables.py::_explode_indices`.
pub fn _explode_indices_with(
    subdims: &[i64],
    drop_last: bool,
    mut begin: i64,
    mut dim: i64,
) -> Vec<Vec<i64>> {
    let dims: &[i64] = if drop_last && !subdims.is_empty() {
        &subdims[..subdims.len() - 1]
    } else {
        subdims
    };
    if drop_last
        && let Some(&last) = subdims.last()
        && last != 0
        && pystr::pymod_i64(dim, last) == Some(0)
        && pystr::pymod_i64(begin, last) == pystr::pymod_i64(1, last)
    {
        begin = pystr::floordiv_i64(begin - 1, last).unwrap_or(0) + 1;
        dim = pystr::floordiv_i64(dim, last).unwrap_or(0);
    }
    if dims.is_empty() {
        return vec![Vec::new()];
    }
    let mut total: i64 = 1;
    for &d in dims {
        total = total.saturating_mul(d);
    }
    let mut strides = vec![1_i64; dims.len()];
    let mut acc: i64 = 1;
    for i in (0..dims.len()).rev() {
        strides[i] = acc;
        acc = acc.saturating_mul(dims[i]);
    }
    let mut start = (begin - 1).min(total - 1).max(0);
    if dims.len() == 1 && begin + dim - 1 > total && !drop_last {
        start = (begin - 1).max(0);
        let count = dim.clamp(0, _MAX_EXPLODE_COUNT);
        let mut out = Vec::with_capacity(usize::try_from(count).unwrap_or(0));
        for lin in start..start + count {
            out.push(vec![lin + 1]);
        }
        return out;
    }
    let count = dim.min(total - start).clamp(0, _MAX_EXPLODE_COUNT);
    let mut out = Vec::with_capacity(usize::try_from(count).unwrap_or(0));
    for lin in start..start + count {
        let mut coord = Vec::with_capacity(dims.len());
        let mut rem = lin;
        for &stride in &strides {
            if stride == 0 {
                coord.push(1);
            } else {
                coord.push(pystr::floordiv_i64(rem, stride).unwrap_or(0) + 1);
                rem = pystr::pymod_i64(rem, stride).unwrap_or(0);
            }
        }
        out.push(coord);
    }
    out
}

/// Port of `tables.py::render_table_list`.
#[allow(clippy::needless_pass_by_value)]
pub fn render_table_list(tables: Vec<TdfTable>) -> String {
    let displays = vec![None; tables.len()];
    render_table_list_with(&tables, &displays)
}

/// Full implementation of `tables.py::render_table_list` with `_interval_display` metadata.
pub fn render_table_list_with(
    tables: &[TdfTable],
    interval_displays: &[Option<(f64, String)>],
) -> String {
    let mut lines: Vec<String> = Vec::new();
    let sep = "-".repeat(52);
    for (idx, t) in tables.iter().enumerate() {
        if t.name == "Status" {
            continue;
        }
        lines.push(sep.clone());
        lines.push(format!("Table: {}", t.name));
        let disp = interval_displays.get(idx).and_then(|d| d.as_ref());
        if let Some((value, unit)) = disp {
            let val_str = if pystr::is_integer(*value) {
                pystr::py_int_from_f64(*value)
                    .map_or_else(|| pystr::repr_f64(*value), |i| i.to_string())
            } else {
                pystr::repr_f64(*value)
            };
            lines.push(format!("Interval: {val_str} {unit}"));
        } else if t.interval != (0, 0) {
            let secs = t.interval.0;
            let (value, unit) = _seconds_to_display_with(secs);
            lines.push(format!("Interval: {value} {unit}"));
        }
        lines.push("Fields:".to_string());
        for f in &t.fields {
            let drop_last = f.r#type == 11 && !f.subdims.is_empty();
            let dims_after_drop = if drop_last {
                &f.subdims[..f.subdims.len() - 1]
            } else {
                &f.subdims[..]
            };
            let coords: Vec<Vec<i64>> = if f.r#type == 11 && dims_after_drop.len() >= 2 {
                let stored_len = *f.subdims.last().unwrap_or(&0);
                let total = (if stored_len != 0 {
                    pystr::floordiv_i64(f.dimension, stored_len).unwrap_or(0)
                } else {
                    0
                })
                .clamp(0, _MAX_EXPLODE_COUNT);
                let mut v = Vec::with_capacity(usize::try_from(total).unwrap_or(0));
                for lin in 0..total {
                    v.push(vec![
                        pystr::floordiv_i64(lin, stored_len).unwrap_or(0) + 1,
                        pystr::pymod_i64(lin, stored_len).unwrap_or(0) + 1,
                    ]);
                }
                v
            } else {
                _explode_indices_with(&f.subdims, drop_last, f.begin_index, f.dimension)
            };
            for coord in coords {
                let suffix = if coord.is_empty() {
                    String::new()
                } else {
                    let joined: Vec<String> = coord.iter().map(i64::to_string).collect();
                    format!("({})", joined.join(","))
                };
                let mut line = format!(" {}{}", f.name, suffix);
                if !f.units.is_empty() {
                    line.push_str("\tUnits: ");
                    line.push_str(&f.units);
                }
                lines.push(line);
            }
        }
        lines.push(String::new());
    }
    if lines.is_empty() {
        String::new()
    } else {
        let mut out = lines.join("\r\n");
        out.push_str("\r\n");
        out
    }
}

/// Port of `tables.py::_seconds_to_display`.
pub fn _seconds_to_display(_secs: i64) {
    // TODO(wire): update return type to `(i64, String)` and delegate to `_seconds_to_display_with`.
}

/// Full implementation of `tables.py::_seconds_to_display`.
pub fn _seconds_to_display_with(secs: i64) -> (i64, String) {
    for (unit, mult) in [("DAY", 86400), ("HR", 3600), ("MIN", 60)] {
        if pystr::pymod_i64(secs, mult) == Some(0)
            && pystr::floordiv_i64(secs, mult).is_some_and(|q| q > 0)
        {
            return (
                pystr::floordiv_i64(secs, mult).unwrap_or(0),
                unit.to_string(),
            );
        }
    }
    (secs, "SEC".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profiles;
    use crate::syntax::{Node, NumberValue};

    fn num_expr(i: i64) -> ExprRef {
        Rc::new(Expr::Number(Number {
            node: Node::default(),
            value: NumberValue::Int(i),
            text: i.to_string(),
        }))
    }

    fn name_expr(s: &str) -> ExprRef {
        Rc::new(Expr::Name(crate::syntax::Name {
            node: Node::default(),
            name: s.to_string(),
        }))
    }

    #[test]
    fn apply_fieldnames_scalar_and_array_split() {
        let scalar = vec![OutFieldData::new(
            "BattV_Avg",
            4,
            false,
            "Avg",
            1,
            1,
            vec![],
        )];
        let res = _apply_fieldnames_with(scalar, "Battery:Volts", "cr6");
        assert_eq!(res.len(), 1);
        assert_eq!(res[0].name, "Battery");
        assert_eq!(res[0].processing, "Avg:Volts");
        assert!(res[0].renamed);

        let mut arr = OutFieldData::new("Temp_Avg", 4, false, "Avg", 1, 4, vec![4]);
        arr.splittable = true;
        arr.elem = 1;
        arr.rng = Some((1, 4));
        let res = _apply_fieldnames_with(vec![arr], "T1,T2", "cr6");
        assert_eq!(res.len(), 3);
        assert_eq!(res[0].name, "T1");
        assert_eq!(res[0].begin, 1);
        assert_eq!(res[0].dim, 1);
        assert_eq!(res[1].name, "T2");
        assert_eq!(res[1].begin, 1);
        assert_eq!(res[1].dim, 1);
        assert_eq!(res[2].name, "Temp_Avg");
        assert_eq!(res[2].begin, 3);
        assert_eq!(res[2].dim, 2);
        assert_eq!(res[2].subdims, vec![4]);
    }

    #[test]
    fn apply_fieldnames_struct_cr300_vs_cr6() {
        let mut f1 = OutFieldData::new("S_a_Avg", 4, false, "Avg", 1, 1, vec![]);
        f1.struct_members = 2;
        f1.member = "a".to_string();
        let mut f2 = OutFieldData::new("S_b_Avg", 4, false, "Avg", 1, 1, vec![]);
        f2.struct_members = 2;
        f2.member = "b".to_string();

        let cr300 = _apply_fieldnames_with(vec![f1.clone(), f2.clone()], "X:desc", "cr300");
        assert_eq!(cr300[0].name, "S_a_Avg");

        let cr6 = _apply_fieldnames_with(vec![f1, f2], "X:desc", "cr6");
        assert_eq!(cr6[0].name, "X_a");
        assert_eq!(cr6[0].processing, "Avg:desc");
        assert_eq!(cr6[1].name, "X_b");
        assert!(cr6[1].renamed);
    }

    #[test]
    fn interval_from_args_units_and_mon() {
        let profile = profiles::get("cr6").expect("cr6 profile");
        let env = Env::new(profile);

        // 0 into, 5 Min (unit=3)
        let args = vec![Some(num_expr(0)), Some(num_expr(5)), Some(num_expr(3))];
        let (into, interval, ival, unit) = _interval_from_args_with(&args, &env, 0, 1, 2);
        assert_eq!(into, (0, 0));
        assert_eq!(interval, (300, 0));
        assert!((ival - 5.0).abs() < 1e-9);
        assert_eq!(unit, "MIN");

        // MON (unit=6)
        let args_mon = vec![Some(num_expr(1)), Some(num_expr(2)), Some(num_expr(6))];
        let (into, interval, ival, unit) = _interval_from_args_with(&args_mon, &env, 0, 1, 2);
        assert_eq!(into, (0, 0));
        assert_eq!(interval, (0, 0));
        assert!((ival - 2.0).abs() < 1e-9);
        assert_eq!(unit, "MON");

        // Invalid unit (99) -> Interval 1 Sec, TintoInt 0
        let args_bad = vec![Some(num_expr(5)), Some(num_expr(10)), Some(num_expr(99))];
        let (into, interval, ival, unit) = _interval_from_args_with(&args_bad, &env, 0, 1, 2);
        assert_eq!(into, (0, 0));
        assert_eq!(interval, (1, 0));
        assert!((ival - 1.0).abs() < 1e-9);
        assert_eq!(unit, "SEC");
    }

    #[test]
    fn field_record_bytes_and_default_min() {
        let f_ascii = OutFieldData::new("S", 11, false, "Smp", 1, 24, vec![24]);
        assert_eq!(_field_record_bytes_with(&f_ascii), 24);
        let f_fp2 = OutFieldData::new("T", 7, false, "Avg", 1, 3, vec![3]);
        assert_eq!(_field_record_bytes_with(&f_fp2), 6);
        let f_ieee8 = OutFieldData::new("D", 14, false, "Smp", 1, 2, vec![2]);
        assert_eq!(_field_record_bytes_with(&f_ieee8), 16);

        assert_eq!(_table_default_min(0), 1);
        assert_eq!(_table_default_min(8), 126);
        assert_eq!(_table_default_min(2000), 1);
    }

    #[test]
    fn resolve_auto_sizes_single_and_forced() {
        let mut tables_meta = vec![(
            TdfTable {
                name: "OneMin".to_string(),
                size: 1,
                time_type: 14,
                time_into: (0, 0),
                interval: (60, 0),
                fields: vec![],
            },
            DataTableMeta {
                own_weight: Some(60.0),
                lapses: 10,
                size_val: -1,
                bytes_per_record: 16,
                interval_seconds: Some(60.0),
                called: true,
                ..DataTableMeta::default()
            },
        )];
        let scan_weights = OrderedMap::new();
        _resolve_auto_sizes_with(&mut tables_meta, &scan_weights);
        // Verified against Python: _FREE_MEMORY=4194303, b=16, w=60, lapses=10
        assert_eq!(tables_meta[0].0.size, 257_470);
        assert_eq!(tables_meta[0].1.n_raw, Some(257_470));
    }

    #[test]
    fn explode_indices_and_render_table_list() {
        assert_eq!(
            _explode_indices_with(&[], false, 1, 1),
            vec![Vec::<i64>::new()]
        );
        assert_eq!(
            _explode_indices_with(&[2, 2], false, 1, 4),
            vec![vec![1, 1], vec![1, 2], vec![2, 1], vec![2, 2]]
        );
        // String slice with drop_last=true
        assert_eq!(
            _explode_indices_with(&[3, 24], true, 25, 48),
            vec![vec![2], vec![3]]
        );
        // Rank-1 FFT overflow past subdim
        assert_eq!(
            _explode_indices_with(&[3], false, 1, 5),
            vec![vec![1], vec![2], vec![3], vec![4], vec![5]]
        );

        let tables = vec![
            TdfTable {
                name: "Status".to_string(),
                ..TdfTable::default()
            },
            TdfTable {
                name: "Tbl".to_string(),
                size: 10,
                time_type: 14,
                time_into: (0, 0),
                interval: (3600, 0),
                fields: vec![
                    TdfField {
                        r#type: 4,
                        name: "A".to_string(),
                        units: "mV".to_string(),
                        begin_index: 1,
                        dimension: 1,
                        ..TdfField::default()
                    },
                    TdfField {
                        r#type: 11,
                        name: "Files".to_string(),
                        begin_index: 1,
                        dimension: 3 * 28,
                        subdims: vec![3, 1, 28],
                        ..TdfField::default()
                    },
                ],
            },
        ];
        let rendered = render_table_list(tables);
        let expected = concat!(
            "----------------------------------------------------\r\n",
            "Table: Tbl\r\n",
            "Interval: 1 HR\r\n",
            "Fields:\r\n",
            " A\tUnits: mV\r\n",
            " Files(1,1)\r\n",
            " Files(1,2)\r\n",
            " Files(1,3)\r\n",
            "\r\n",
        );
        assert_eq!(rendered, expected);
    }

    #[test]
    fn seconds_to_display_cases() {
        assert_eq!(_seconds_to_display_with(86400), (1, "DAY".to_string()));
        assert_eq!(_seconds_to_display_with(7200), (2, "HR".to_string()));
        assert_eq!(_seconds_to_display_with(180), (3, "MIN".to_string()));
        assert_eq!(_seconds_to_display_with(90), (90, "SEC".to_string()));
        assert_eq!(_seconds_to_display_with(0), (0, "SEC".to_string()));
    }

    #[test]
    fn preserve_deduction_matches_python() {
        let profile = profiles::get("cr300").expect("cr300 profile");
        let env = Env::new(profile);
        let v = Rc::new(VarDecl {
            name: "X".to_string(),
            dims: vec![num_expr(4)],
            type_: Some("FLOAT".to_string()),
            ..VarDecl::default()
        });
        let prog = Program::new(
            "test.cr300",
            vec![
                Rc::new(Stmt::Instruction(Instruction {
                    keyword: "PreserveVariables".to_string(),
                    ..Instruction::default()
                })),
                Rc::new(Stmt::Instruction(Instruction {
                    keyword: "Scan".to_string(),
                    args: vec![Some(num_expr(1)), Some(name_expr("Sec"))],
                    ..Instruction::default()
                })),
            ],
        );
        let struct_types = OrderedMap::new();
        let ded = _preserve_deduction_with(&prog, &env, &[v], &struct_types);
        assert_eq!(ded, 16_384);
    }

    #[test]
    fn apply_sizes_zero_interval_and_uncalled() {
        let mut tables_meta = vec![
            (
                TdfTable {
                    name: "ZeroInt".to_string(),
                    size: 1,
                    time_type: 14,
                    time_into: (0, 0),
                    interval: (0, 0),
                    fields: vec![],
                },
                DataTableMeta {
                    zero_interval: true,
                    called: true,
                    size_val: 100,
                    bytes_per_record: 8,
                    display_interval: Some((0.0, "MIN".to_string())),
                    ..DataTableMeta::default()
                },
            ),
            (
                TdfTable {
                    name: "Uncalled".to_string(),
                    size: 50,
                    time_type: 14,
                    time_into: (1, 0),
                    interval: (60, 0),
                    fields: vec![],
                },
                DataTableMeta {
                    called: false,
                    size_val: 50,
                    ..DataTableMeta::default()
                },
            ),
        ];
        let mut scan_weights = OrderedMap::new();
        scan_weights.insert("ZEROINT".to_string(), 300.0);
        _apply_sizes_with(&mut tables_meta, &scan_weights, false);
        assert_eq!(tables_meta[0].0.size, 100);
        assert_eq!(tables_meta[0].0.interval, (300, 0));
        assert_eq!(
            tables_meta[0].1.display_interval,
            Some((5.0, "MIN".to_string()))
        );
        assert_eq!(tables_meta[1].0.size, 1);
        assert_eq!(tables_meta[1].0.interval, (0, 0));
        assert_eq!(tables_meta[1].0.time_into, (0, 0));
    }

    #[test]
    fn register_table_sources_populates_var_info() {
        let table = TdfTable {
            name: "Out".to_string(),
            fields: vec![
                TdfField {
                    r#type: 4,
                    name: "Val".to_string(),
                    dimension: 3,
                    subdims: vec![],
                    ..TdfField::default()
                },
                TdfField {
                    r#type: 11,
                    name: "Msg".to_string(),
                    dimension: 48,
                    subdims: vec![2, 24],
                    ..TdfField::default()
                },
            ],
            ..TdfTable::default()
        };
        let mut var_info = OrderedMap::new();
        _register_table_sources_with(&table, &mut var_info);
        let val = var_info.get("OUT.VAL").expect("OUT.VAL");
        assert_eq!(val.shape, vec![3]);
        assert_eq!(val.stored_len, None);
        let msg = var_info.get("OUT.MSG").expect("OUT.MSG");
        assert_eq!(msg.shape, vec![2]);
        assert_eq!(msg.stored_len, Some(24));
    }
}
