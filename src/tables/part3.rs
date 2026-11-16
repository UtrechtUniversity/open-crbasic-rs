//! Port of `tables.py` (lines 1527-2177).

#![allow(
    dead_code,
    unused_variables,
    unused_imports,
    clippy::used_underscore_items
)]

use std::collections::{BTreeSet, VecDeque};
use std::rc::Rc;

use super::part2::{
    _FFT_UNBOUNDED, _alias_source, _build_const_tables, _datatableinfo_table, PublicTdfField,
};
use super::{
    _DEFAULT_PROBE_BUFFER_BYTES, _DEFAULT_PROBE_OVERHEAD, _FREE_MEMORY, _TABLE_INIT_RESERVED,
    _as_int, _build_public_table, _build_struct_tables, _ch400status_table, _cpistatus_table,
    _emit_output_fields, _f32, _fft_field, _fold, _kw, _last_call_interval, _make_var_info,
    _scan_calls, _scan_program, _scan_unit_name, _scan_weights, _split_by_alias, _status_table,
    _type_bytes, _unit_name_by_value, _unit_ns, _unit_seconds, _unit_value, ConstTableDecl,
    DataTableDecl, OutField, Scanned, TdfTable, VarInfo, declared_size,
};
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

/// Port of `tables.py::_build_data_table` metadata dict.
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

/// Per-table attributes Python hangs on `TdfTable` (`_dup_fields`, `_interval_display`,
/// `_storage`), index-aligned with the returned table list.
#[derive(Debug, Clone, Default)]
pub struct TableExtras {
    pub dup_fields: Vec<Option<Vec<OutField>>>,
    pub interval_display: Vec<Option<(f64, String)>>,
    pub storage: Vec<Option<DataTableMeta>>,
}

#[allow(
    clippy::cast_precision_loss,
    reason = "matches Python float(int) conversion"
)]
fn i64_to_f64(n: i64) -> f64 {
    n as f64
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

// ---------------------------------------------------------------- ported functions (tables.py:1527-2177)

/// Port of `tables.py::_apply_fieldnames`.
pub fn _apply_fieldnames(
    mut fields: Vec<OutField>,
    arg: &str,
    profile_name: &str,
) -> Vec<OutField> {
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
                let mut f = OutField::new(
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
                let mut rem_f = OutField::new(
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
pub fn _interval_from_args(
    args: &[Option<ExprRef>],
    env: &Env,
    into_idx: usize,
    val_idx: usize,
    unit_idx: usize,
) -> ((i64, i64), (i64, i64), f64, String) {
    let unit_val = args
        .get(unit_idx)
        .and_then(|e| e.as_deref())
        .and_then(|e| _unit_value(Some(e), env));
    let into_val = args
        .get(into_idx)
        .and_then(|e| e.as_deref())
        .and_then(|e| _fold(Some(e), env, None))
        .filter(consteval::is_finite_number)
        .map_or(0.0, |v| _const_to_f64(&v));
    let mut interval_val = args
        .get(val_idx)
        .and_then(|e| e.as_deref())
        .and_then(|e| _fold(Some(e), env, None))
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
    let ns_per_unit = i64_to_f64(_unit_ns(unit_name).unwrap_or(1_000_000_000));
    let into_ns = pystr::py_round(into_f * ns_per_unit).unwrap_or(0);
    let interval_ns = pystr::py_round(interval_val * ns_per_unit).unwrap_or(0);
    let into = pystr::divmod_i64(into_ns, 1_000_000_000).unwrap_or((0, 0));
    let interval = pystr::divmod_i64(interval_ns, 1_000_000_000).unwrap_or((0, 0));
    (into, interval, interval_val, unit_name.to_string())
}

/// Port of `tables.py::_build_data_table`.
pub fn _build_data_table(
    dt: &DataTableDecl,
    env: &Env,
    var_info: &OrderedMap<String, VarInfo>,
    by_display: &OrderedMap<String, PublicTdfField>,
    called: &BTreeSet<String>,
    by_base: Option<&OrderedMap<String, Vec<PublicTdfField>>>,
    mut anon: Option<&mut i64>,
) -> (TdfTable, DataTableMeta, Vec<OutField>) {
    let mut into = (0, 0);
    let mut interval = (0, 0);
    let mut interval_seconds: Option<f64> = None;
    let mut display_interval: Option<(f64, String)> = None;
    let mut zero_interval = false;
    let mut has_card = false;
    let mut lapses: i64 = 10;
    let mut own_weight: Option<f64> = None;

    let mut out_fields: Vec<OutField> = Vec::new();
    let mut pending: Vec<OutField> = Vec::new();
    let mut pending_src: Option<ExprRef> = None;

    let split_pending = |pending: Vec<OutField>, pending_src: Option<&ExprRef>| {
        if let Some(src) = pending_src
            && let Some(bb) = by_base.filter(|m| !m.is_empty())
        {
            _split_by_alias(pending, Some(src.as_ref()), var_info, bb, env)
        } else {
            pending
        }
    };

    for s in &dt.body {
        let kw = _kw(s.as_ref());
        if let Stmt::Instruction(instr) = s.as_ref()
            && kw == "DATAINTERVAL"
        {
            let (i_into, i_interval, ival, unit) = _interval_from_args(&instr.args, env, 0, 1, 2);
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
                let folded = _fold(Some(arg3), env, None);
                lapses = _as_int(folded.as_ref(), 10).abs();
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
                pending = _apply_fieldnames(pending, &str_lit.value, env.profile.name);
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
            let aliased = _alias_source(instr, var_info, by_display, by_base);
            pending = _emit_output_fields(
                &aliased,
                var_info,
                by_display,
                env,
                by_base,
                anon.as_deref_mut(),
            );
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
            pending = vec![_fft_field(
                instr,
                var_info,
                by_display,
                env,
                anon.as_deref_mut(),
            )];
            pending_src = None;
        } else if kw == "ETSZ" || kw == "ETCLEARSKY" {
            if !pending.is_empty() {
                out_fields.extend(split_pending(pending, pending_src.as_ref()));
            }
            pending = ["ETrs", "Rso"]
                .iter()
                .map(|&n| {
                    let mut f = OutField::new(n, 7, true, n, 1, 1, Vec::new());
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

    let mut size_raw = _fold(dt.size_expr.as_deref(), env, None).or(Some(ConstValue::Int(-1)));
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
        .map(_field_record_bytes)
        .sum();
    let size_val = declared_size(size_raw.as_ref(), cr3xx, bytes_per_record).unwrap_or(-1);

    if interval_seconds.is_some()
        && 2 * bytes_per_record > _DEFAULT_PROBE_BUFFER_BYTES - _DEFAULT_PROBE_OVERHEAD
    {
        into = (0, 0);
        interval = (0, 0);
        interval_seconds = None;
    }

    let has_event = dt.body.iter().any(|s| _kw(s.as_ref()) == "DATAEVENT");
    let fillstop = dt.body.iter().any(|s| _kw(s.as_ref()) == "FILLSTOP");
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
pub fn _field_record_bytes(of: &OutField) -> i64 {
    if of.type_code == 11 {
        return of.dim;
    }
    of.dim * _type_bytes(of.type_code, 0)
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
pub fn _resolve_auto_sizes(
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
        let w = _f32(raw_w);
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
        let m_rate = _f32(_f32(i64_to_f64(t8)) / _f32(_f32(i64_to_f64(dflt)) * w));
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
        rate = _f32(m_rate + rate);
    }
    let mut duration = if rate == 0.0 {
        f64::INFINITY
    } else {
        _f32(_f32(i64_to_f64(_FREE_MEMORY - reserved)) / rate)
    };
    if rate != 0.0 {
        for &idx in &auto_indices {
            let m = &mut tables_meta[idx].1;
            if duration > 0.0 && duration <= m.w {
                m.forced = true;
                rate = _f32(rate - m.rate);
                reserved -= m.reserve;
                duration = if rate == 0.0 {
                    f64::INFINITY
                } else {
                    _f32(_f32(i64_to_f64(_FREE_MEMORY - reserved)) / rate)
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
            pystr::py_int_from_f64(_f32(duration / m.w)).unwrap_or(0)
        } else {
            0
        };
        m.n_raw = Some(records);
        t.size = records.max(2).max(m.dflt);
    }
}

/// Port of `tables.py::_apply_sizes`.
pub fn _apply_sizes(
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
        _resolve_auto_sizes(tables_meta, scan_weights);
    }
}

/// Port of `tables.py::_register_table_sources`.
pub fn _register_table_sources(table: &TdfTable, var_info: &mut OrderedMap<String, VarInfo>) {
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
            VarInfo {
                type_code: f.r#type,
                shape,
                stored_len,
                units: String::new(),
                struct_def: None,
                name: f.name.clone(),
            },
        );
    }
}

/// Port of `tables.py::build_tables`.
pub fn build_tables(
    program: &Program,
    profile: &'static Profile,
    consts: &OrderedMap<String, ConstValue>,
) -> (Vec<TdfTable>, String, TableExtras) {
    let consts_cell = std::cell::RefCell::new(consts.clone());
    let env = Env::with_consts(profile, Rc::new(consts_cell));
    let scanned = _scan_program(program);
    let scan_weights = _scan_weights(&program.statements, &env);
    let scan_calls = _scan_calls(&program.statements, &env);

    let mut var_info: OrderedMap<String, VarInfo> = OrderedMap::new();
    for v in &scanned.all_vars {
        let mut info = _make_var_info(v, &env);
        info.struct_def = scanned
            .struct_types
            .get(&pystr::upper(v.type_.as_deref().unwrap_or("")))
            .cloned();
        var_info.insert(pystr::upper(&v.name), info);
    }

    let public = _build_public_table(
        &scanned.publics,
        &var_info,
        &scanned.alias_stmts,
        &scanned.units_stmts,
        &scanned.readonly_names,
    );
    let by_display = public.by_display;
    let by_base = public.by_base;

    let mut tables: Vec<TdfTable> = vec![_status_table(scanned.slow_seq_count)];
    let mut extras = TableExtras::default();
    let struct_tables = _build_struct_tables(&scanned.all_vars, &scanned.struct_types, &env);
    tables.extend(struct_tables.iter().cloned());
    let mut have_struct_tables = !struct_tables.is_empty();
    if scanned.uses_ch400 {
        have_struct_tables = true;
        tables.push(_ch400status_table());
    }
    for table in &tables {
        _register_table_sources(table, &mut var_info);
    }

    let mut data_table_metas: Vec<(TdfTable, DataTableMeta)> = Vec::new();
    let mut data_table_indices: Vec<usize> = Vec::new();
    let mut dup_fields: OrderedMap<usize, Vec<OutField>> = OrderedMap::new();
    let mut anon: i64 = 0;
    for dt in &scanned.data_tables {
        let (table, meta, out_fields) = _build_data_table(
            dt,
            &env,
            &var_info,
            &by_display,
            &scanned.called,
            Some(&by_base),
            Some(&mut anon),
        );
        data_table_indices.push(tables.len());
        dup_fields.insert(tables.len(), out_fields);
        tables.push(table.clone());
        _register_table_sources(&table, &mut var_info);
        data_table_metas.push((table, meta));
    }

    if !scanned.data_tables.is_empty() || have_struct_tables {
        tables.push(_datatableinfo_table(
            i64::try_from(scanned.data_tables.len()).unwrap_or(0),
        ));
    }

    if scanned.uses_cpi {
        tables.push(_cpistatus_table());
    }

    tables.extend(_build_const_tables(&scanned.const_tables, &env));

    if let Some(public_table) = public.table {
        tables.push(public_table);
    }

    let cr3xx = matches!(profile.name, "cr300" | "cr350");
    if cr3xx {
        let last_call = _last_call_interval(&program.statements, &env);
        for (table, meta) in &mut data_table_metas {
            let last_w = last_call.get(&pystr::upper(&table.name)).copied().flatten();
            meta.call_w = meta.own_weight.or(last_w);
        }
    }
    _apply_sizes(&mut data_table_metas, &scan_weights, !cr3xx);
    let mut alloc: Option<AllocResult> = None;
    if cr3xx && !data_table_metas.is_empty() {
        let pool = storage::pool_bytes(
            profile.name,
            i64::try_from(data_table_metas.len()).unwrap_or(0),
            !scanned.publics.is_empty(),
            i64::try_from(
                scanned
                    .const_tables
                    .iter()
                    .filter(|c| c.name.as_deref().is_some_and(|n| !n.is_empty()))
                    .count(),
            )
            .unwrap_or(0),
        ) - _preserve_deduction(program, &env, &scanned, &var_info);
        let storage_metas: Vec<TableMeta> = data_table_metas
            .iter()
            .map(|(_, m)| m.to_storage_meta())
            .collect();
        alloc = storage::run(&storage_metas, pool);
        if let Some(ref res) = alloc {
            for ((table, m), &n) in data_table_metas.iter_mut().zip(res.sizes.iter()) {
                if m.called && (m.size_val == -1 || n > table.size) {
                    table.size = n;
                }
            }
        } else {
            for (table, m) in &mut data_table_metas {
                if m.called && m.size_val == -1 {
                    table.size = 1;
                }
            }
        }
    }

    extras.dup_fields = vec![None; tables.len()];
    extras.interval_display = vec![None; tables.len()];
    extras.storage = vec![None; tables.len()];
    for (k, (table, mut meta)) in data_table_metas.into_iter().enumerate() {
        let idx = data_table_indices[k];
        if let Some(ref res) = alloc {
            meta.alloc = Some(res.clone());
        }
        meta.scans = scan_calls
            .get(&pystr::upper(&table.name))
            .cloned()
            .unwrap_or_default();
        extras.interval_display[idx].clone_from(&meta.display_interval);
        extras.dup_fields[idx] = dup_fields.get(&idx).cloned();
        extras.storage[idx] = Some(meta);
        tables[idx] = table;
    }

    let table_list_text = render_table_list(&tables, &extras.interval_display);
    (tables, table_list_text, extras)
}

/// Port of `tables.py::_preserve_deduction`.
pub fn _preserve_deduction(
    program: &Program,
    env: &Env,
    scanned: &Scanned,
    var_info: &OrderedMap<String, VarInfo>,
) -> i64 {
    let stmts = &program.statements;
    let kws: Vec<String> = stmts.iter().map(|s| _kw(s.as_ref())).collect();
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
                && let Some(val) = _fold(Some(arg0.as_ref()), env, None)
                && consteval::is_finite_number(&val)
            {
                let fval = _const_to_f64(&val);
                if fval > 0.0 {
                    let unit = instr
                        .args
                        .get(1)
                        .and_then(|a| a.as_deref())
                        .map_or_else(|| "SEC".to_string(), |e| _scan_unit_name(Some(e), env));
                    let ns = i64_to_f64(_unit_ns(&unit).unwrap_or(1_000_000_000));
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

    let size = |v: &VarDecl| -> (i64, i64) {
        let info = _make_var_info(v, env);
        let width = match info.stored_len {
            Some(n) if n != 0 => n,
            _ => {
                if pystr::upper(v.type_.as_deref().unwrap_or("")) == "DOUBLE" {
                    8
                } else {
                    4
                }
            }
        };
        let count = info.shape.iter().fold(1_i64, |a, &d| a.saturating_mul(d));
        (width, count)
    };

    let mut total: i64 = 0;
    if kws.iter().any(|k| k == "PRESERVEVARIABLES") {
        let mut pub_bytes: i64 = 16;
        for v in &scanned.all_vars {
            let ty = pystr::upper(v.type_.as_deref().unwrap_or(""));
            if !scanned.struct_types.contains_key(&ty) {
                let (w, n) = size(v);
                pub_bytes += w * n;
            }
        }
        for (_, st) in scanned.struct_types.iter() {
            for m in &st.members {
                let (w, n) = size(m);
                pub_bytes += w * n;
            }
        }
        total += storage::preserve_charge(64, 60, 0, 100_000, ticks, pub_bytes);
    }

    let mut by_name: OrderedMap<String, Rc<VarDecl>> = OrderedMap::new();
    for v in &scanned.all_vars {
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
        let (w, n) = size(v);
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
pub fn _explode_indices(
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

/// Full implementation of `tables.py::render_table_list` with `_interval_display` metadata.
pub fn render_table_list(
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
            let (value, unit) = _seconds_to_display(secs);
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
                _explode_indices(&f.subdims, drop_last, f.begin_index, f.dimension)
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
pub fn _seconds_to_display(secs: i64) -> (i64, String) {
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
        let scalar = vec![OutField::new("BattV_Avg", 4, false, "Avg", 1, 1, vec![])];
        let res = _apply_fieldnames(scalar, "Battery:Volts", "cr6");
        assert_eq!(res.len(), 1);
        assert_eq!(res[0].name, "Battery");
        assert_eq!(res[0].processing, "Avg:Volts");
        assert!(res[0].renamed);

        let mut arr = OutField::new("Temp_Avg", 4, false, "Avg", 1, 4, vec![4]);
        arr.splittable = true;
        arr.elem = 1;
        arr.rng = Some((1, 4));
        let res = _apply_fieldnames(vec![arr], "T1,T2", "cr6");
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
        let mut f1 = OutField::new("S_a_Avg", 4, false, "Avg", 1, 1, vec![]);
        f1.struct_members = 2;
        f1.member = "a".to_string();
        let mut f2 = OutField::new("S_b_Avg", 4, false, "Avg", 1, 1, vec![]);
        f2.struct_members = 2;
        f2.member = "b".to_string();

        let cr300 = _apply_fieldnames(vec![f1.clone(), f2.clone()], "X:desc", "cr300");
        assert_eq!(cr300[0].name, "S_a_Avg");

        let cr6 = _apply_fieldnames(vec![f1, f2], "X:desc", "cr6");
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
        let (into, interval, ival, unit) = _interval_from_args(&args, &env, 0, 1, 2);
        assert_eq!(into, (0, 0));
        assert_eq!(interval, (300, 0));
        assert!((ival - 5.0).abs() < 1e-9);
        assert_eq!(unit, "MIN");

        // MON (unit=6)
        let args_mon = vec![Some(num_expr(1)), Some(num_expr(2)), Some(num_expr(6))];
        let (into, interval, ival, unit) = _interval_from_args(&args_mon, &env, 0, 1, 2);
        assert_eq!(into, (0, 0));
        assert_eq!(interval, (0, 0));
        assert!((ival - 2.0).abs() < 1e-9);
        assert_eq!(unit, "MON");

        // Invalid unit (99) -> Interval 1 Sec, TintoInt 0
        let args_bad = vec![Some(num_expr(5)), Some(num_expr(10)), Some(num_expr(99))];
        let (into, interval, ival, unit) = _interval_from_args(&args_bad, &env, 0, 1, 2);
        assert_eq!(into, (0, 0));
        assert_eq!(interval, (1, 0));
        assert!((ival - 1.0).abs() < 1e-9);
        assert_eq!(unit, "SEC");
    }

    #[test]
    fn field_record_bytes_and_default_min() {
        let f_ascii = OutField::new("S", 11, false, "Smp", 1, 24, vec![24]);
        assert_eq!(_field_record_bytes(&f_ascii), 24);
        let f_fp2 = OutField::new("T", 7, false, "Avg", 1, 3, vec![3]);
        assert_eq!(_field_record_bytes(&f_fp2), 6);
        let f_ieee8 = OutField::new("D", 14, false, "Smp", 1, 2, vec![2]);
        assert_eq!(_field_record_bytes(&f_ieee8), 16);

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
        _resolve_auto_sizes(&mut tables_meta, &scan_weights);
        // Verified against Python: _FREE_MEMORY=4194303, b=16, w=60, lapses=10
        assert_eq!(tables_meta[0].0.size, 257_470);
        assert_eq!(tables_meta[0].1.n_raw, Some(257_470));
    }

    #[test]
    fn explode_indices_and_render_table_list() {
        assert_eq!(_explode_indices(&[], false, 1, 1), vec![Vec::<i64>::new()]);
        assert_eq!(
            _explode_indices(&[2, 2], false, 1, 4),
            vec![vec![1, 1], vec![1, 2], vec![2, 1], vec![2, 2]]
        );
        // String slice with drop_last=true
        assert_eq!(
            _explode_indices(&[3, 24], true, 25, 48),
            vec![vec![2], vec![3]]
        );
        // Rank-1 FFT overflow past subdim
        assert_eq!(
            _explode_indices(&[3], false, 1, 5),
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
        let rendered = render_table_list(&tables, &[]);
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
        assert_eq!(_seconds_to_display(86400), (1, "DAY".to_string()));
        assert_eq!(_seconds_to_display(7200), (2, "HR".to_string()));
        assert_eq!(_seconds_to_display(180), (3, "MIN".to_string()));
        assert_eq!(_seconds_to_display(90), (90, "SEC".to_string()));
        assert_eq!(_seconds_to_display(0), (0, "SEC".to_string()));
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
        let scanned = Scanned {
            all_vars: vec![v],
            ..Scanned::default()
        };
        let ded = _preserve_deduction(&prog, &env, &scanned, &OrderedMap::new());
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
        _apply_sizes(&mut tables_meta, &scan_weights, false);
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
        _register_table_sources(&table, &mut var_info);
        let val = var_info.get("OUT.VAL").expect("OUT.VAL");
        assert_eq!(val.shape, vec![3]);
        assert_eq!(val.stored_len, None);
        let msg = var_info.get("OUT.MSG").expect("OUT.MSG");
        assert_eq!(msg.shape, vec![2]);
        assert_eq!(msg.stored_len, Some(24));
    }
}
