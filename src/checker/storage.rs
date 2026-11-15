//! Port of `checker/storage.py`.

#![allow(
    dead_code,
    unused_variables,
    clippy::used_underscore_items,
    reason = "port keeps Python's leading-underscore function names and stub signatures"
)]

use std::collections::BTreeSet;

use crate::consteval::{self, Env};
use crate::diagnostics::{Diagnostics, ShowFile};
use crate::profiles::Profile;
use crate::pystr;
use crate::syntax::{ConstValue, Expr, Program, Stmt, VarDecl, rc_id};

const _PAGE: i64 = 4096;
const _BLOCK_BYTES: i64 = 496;
const _INT_MIN: i64 = -(1_i64 << 31);
const _BASE: i64 = 0x0060_0000;
const _MAX_ALLOC: i64 = 0x0800_0000;
const _TWO31: f64 = 2_147_483_648.0;
const _INT32_MAX: i64 = 2_147_483_647;

pub const MSG_ALLOC: &str =
    "Unable to allocate enough data table space to manage flash write cycle limits.";
pub const MSG_STORAGE: &str = "Not enough storage area for DataTables.";
pub const MSG_OOM: &str = "Out of memory.";
pub const MSG_ALLOC_TABLE: &str = "Out of Memory; Unable to allocate table {}.";

fn _pool_pages(profile_name: &str) -> Option<i64> {
    match profile_name {
        "cr300" => Some(2560),
        "cr350" => Some(12800),
        _ => None,
    }
}

#[allow(
    clippy::cast_precision_loss,
    reason = "matches Python float(int) conversion"
)]
fn i64_to_f64(n: i64) -> f64 {
    n as f64
}

/// Allocator input specification for one data table (Python `spec` dict).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TableSpec {
    pub called: bool,
    pub n: i64,
    pub flagged: bool,
    pub b: i64,
    pub lapses: i64,
    pub card: bool,
    pub own_ms: i64,
    pub w: Option<f64>,
    pub fillstop: bool,
    pub kone: bool,
    pub day: bool,
    pub zero_interval: bool,
}

/// Mutable per-table state used during `allocate` (Python `st` element dict).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TableState {
    pub n: i64,
    pub k: i64,
    pub r: i64,
    pub lapses: i64,
    pub called: bool,
    pub flagged: bool,
    pub b: i64,
    pub card: bool,
    pub own_ms: i64,
    pub w: Option<f64>,
    pub fillstop: bool,
    pub kone: bool,
    pub day: bool,
}

/// Return value of `allocate` / `run`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AllocResult {
    pub sizes: Vec<i64>,
    pub msg1: bool,
    pub msg2: bool,
}

/// Metadata for one data table (Python `tables._build_data_table` meta dict).
#[derive(Debug, Clone, Default)]
pub struct TableMeta {
    pub size_raw: Option<ConstValue>,
    pub called: bool,
    pub bytes_per_record: i64,
    pub lapses: i64,
    pub interval_seconds: f64,
    pub zero_interval: bool,
    pub call_w: Option<f64>,
    pub fillstop: bool,
    pub display_interval: bool,
    pub has_card: bool,
    pub n_raw: Option<i64>,
    pub alloc: Option<AllocResult>,
}

/// One declared table's name and storage metadata for `check_with`.
#[derive(Debug, Clone, Default)]
pub struct TableStorageEntry {
    pub name: String,
    pub storage: TableMeta,
}

// ---------------------------------------------------------------- local tables.py helpers
// TODO(P3-h): replace with calls into `crate::tables` once `tables.rs` is ported.

#[derive(Debug, Clone, PartialEq, Eq)]
enum SizeClass {
    Auto,
    Fixed(i64, bool),
    Edge(i64),
    Inf,
    Toxic,
}

fn _f32_safe_local(x: f64) -> f64 {
    _f32(x)
}

fn _size_class(raw: Option<&ConstValue>) -> Option<SizeClass> {
    let raw = raw?;
    let v = match raw {
        ConstValue::Int(i) if *i >= -2_147_483_648 && *i < 2_147_483_648 => *i,
        ConstValue::Int(i) => {
            let f = _f32_safe_local(i64_to_f64(*i));
            return _classify_f32(f);
        }
        ConstValue::Float(f) | ConstValue::CFloat(f) => {
            let f32v = _f32_safe_local(*f);
            return _classify_f32(f32v);
        }
        ConstValue::BigInt(s) => {
            let f32v = _f32_safe_local(s.parse::<f64>().unwrap_or(f64::INFINITY));
            return _classify_f32(f32v);
        }
        ConstValue::Str(_) => return None,
    };
    if v == -1 {
        Some(SizeClass::Auto)
    } else if v == 0 {
        Some(SizeClass::Fixed(1, false))
    } else if v < -1 {
        Some(SizeClass::Fixed((-v).min(_INT32_MAX), true))
    } else {
        Some(SizeClass::Fixed(v, false))
    }
}

fn _classify_f32(f: f64) -> Option<SizeClass> {
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
    if v == -1 {
        Some(SizeClass::Auto)
    } else if v == 0 {
        Some(SizeClass::Fixed(1, false))
    } else if v < -1 {
        Some(SizeClass::Fixed((-v).min(_INT32_MAX), true))
    } else {
        Some(SizeClass::Fixed(v, false))
    }
}

fn _fold_local(expr: Option<&Expr>, env: &Env) -> Option<ConstValue> {
    let e = expr?;
    consteval::value_of(Some(e), env).ok()
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

fn _string_stored_len_local(n: i64) -> i64 {
    (pystr::floordiv_i64(n, 4).unwrap_or(0) + 1) * 4
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

fn _make_var_stored_len(v: &VarDecl, env: &Env) -> Option<i64> {
    let ty = pystr::upper(v.type_.as_deref().unwrap_or(""));
    if ty == "STRING" {
        let n = _as_int_local(_fold_local(_string_size_expr_local(v), env).as_ref(), 20);
        Some(_string_stored_len_local(n.max(1)))
    } else {
        None
    }
}

// ---------------------------------------------------------------- ported functions

/// Port of `checker/storage.py::_f32`.
#[allow(
    clippy::cast_possible_truncation,
    reason = "intentional IEEE-754 float64 to float32 rounding"
)]
pub fn _f32(x: f64) -> f64 {
    f64::from(x as f32)
}

/// Port of `checker/storage.py::_cvt_int`.
pub fn _cvt_int(x: f64) -> i64 {
    if x.is_nan() || !(-2_147_483_648.0..2_147_483_648.0).contains(&x) {
        return _INT_MIN;
    }
    pystr::py_int_from_f64(x).unwrap_or(_INT_MIN)
}

/// Port of `checker/storage.py::_wrap32`.
pub fn _wrap32(v: i64) -> i64 {
    let v = v & 0xFFFF_FFFF;
    if (v & 0x8000_0000) != 0 {
        v - (1_i64 << 32)
    } else {
        v
    }
}

/// Port of `checker/storage.py::_i32`.
pub fn _i32(v: i64) -> i64 {
    _wrap32(v)
}

/// Port of `checker/storage.py::_min_recs`.
pub fn _min_recs(k: i64, cc: i64) -> i64 {
    _min_recs_with(&TableState::default(), k, cc)
}

/// Full implementation of `checker/storage.py::_min_recs`.
pub fn _min_recs_with(sp: &TableState, k: i64, cc: i64) -> i64 {
    let period = if sp.flagged {
        86_400_000.0
    } else if sp.day {
        2_592_000_000.0
    } else if sp.own_ms != 0 {
        _f32(i64_to_f64(sp.own_ms))
    } else if let Some(w) = sp.w {
        if w == 0.0 {
            let mut p = 50.0;
            for &(lim, val) in &[
                (2, 2000.0),
                (4, 5000.0),
                (8, 10000.0),
                (12, 20000.0),
                (16, 30000.0),
                (20, 60000.0),
                (23, 120_000.0),
            ] {
                if cc > lim {
                    p = val;
                }
            }
            p
        } else {
            _f32(_f32(w) * 1000.0)
        }
    } else if cc == 0 {
        50.0
    } else if cc < 5 {
        i64_to_f64(cc * 100)
    } else if cc < 10 {
        i64_to_f64(cc * 1000)
    } else {
        i64_to_f64(cc * 5000)
    };
    let den = _f32(i64_to_f64(10 - cc).max(1.0));
    let denom = _f32(_f32(100_000.0) / _f32(den * 365.0));
    if period == 0.0 || k == 0 || denom == 0.0 {
        return _INT_MIN;
    }
    let wear = _f32(_f32(_f32(_f32(3_600_000.0 / period) * 24.0) / _f32(i64_to_f64(k))) / denom);
    _cvt_int(wear)
}

/// Port of `checker/storage.py::preserve_charge`.
pub fn preserve_charge(
    a: i64,
    d: i64,
    q_arg: i64,
    min_tick: i64,
    tick: i64,
    public_bytes: i64,
) -> i64 {
    let t = tick.max(min_tick);
    let q = if q_arg == 0 {
        pystr::floordiv_i64(public_bytes, d).unwrap_or(0).max(1)
    } else {
        q_arg
    };
    let wear = _f32(_f32(_f32(3_600_000.0 / _f32(i64_to_f64(t) / 10.0)) * 24.0) / 27.0);
    let qf = i64_to_f64(q);
    let recs = _f32(_f32(_f32(_f32(qf * 0.6) * wear) + qf) + 2.0);
    let n = _cvt_int(recs.max(5.0));
    _round_page(_wrap32(n.wrapping_mul(a)).max(0x2000))
}

/// Port of `checker/storage.py::_round_page`.
pub fn _round_page(v: i64) -> i64 {
    let u = v & 0xFFFF_FFFF;
    let pages = pystr::floordiv_i64(-u, _PAGE).unwrap_or(0);
    _wrap32((-pages).wrapping_mul(_PAGE))
}

/// Port of `checker/storage.py::allocate`.
pub fn allocate(_pool: i64) {}

/// Full implementation of `checker/storage.py::allocate`.
pub fn allocate_with(specs: &[TableSpec], pool: i64) -> AllocResult {
    let mut st: Vec<TableState> = specs
        .iter()
        .map(|sp| TableState {
            n: sp.n,
            k: 0,
            r: 0,
            lapses: sp.lapses,
            called: sp.called,
            flagged: sp.flagged,
            b: sp.b,
            card: sp.card,
            own_ms: sp.own_ms,
            w: sp.w,
            fillstop: sp.fillstop,
            kone: sp.kone,
            day: sp.day,
        })
        .collect();
    let mut e4 = pool;
    let mut ok_exit = false;
    for cc in 0..26 {
        e4 = pool;
        let mut reserve: i64 = 0;
        let mut rate: f64 = 0.0;
        let mut d8 = false;
        let mut ptr = _BASE;
        for t in &mut st {
            if !t.called {
                t.n = 1;
                reserve = _i32(reserve.wrapping_add(t.b));
                continue;
            }
            if (t.n < 0 || t.w.is_none()) && !t.flagged {
                _blockfix_with(t, 512);
                let (r, k) = (t.r, t.k);
                let c1 = (if r < 512 { 1 } else { 3 })
                    + pystr::floordiv_i64(1024, r).unwrap_or(0)
                    + i64::from(k == 1);
                let c1 = c1.wrapping_mul(r).max(0x3000);
                reserve = _i32(reserve.wrapping_add(c1));
                if !t.card {
                    reserve = _i32(reserve.wrapping_add(_lap_with(t).wrapping_mul(t.b)));
                }
                let w = match t.w {
                    None => 1.0,
                    Some(wv) => _f32(wv),
                };
                rate = _f32(_f32(i64_to_f64(r) / _f32(i64_to_f64(k) * w)) + rate);
            } else {
                let a1 = _lap_with(t);
                let arg = _i32(a1.wrapping_add(t.n));
                _blockfix_with(t, arg);
                let (r, k) = (t.r, t.k);
                let a2 = _lap_with(t);
                let mut u = _i32(
                    a2.wrapping_add(_cvt_int(_f32(
                        _f32(i64_to_f64(r)) * _f32(_f32(i64_to_f64(t.n)) / _f32(i64_to_f64(k))),
                    )))
                    .wrapping_add(0x2000),
                );
                if !t.fillstop {
                    let lo = _min_recs_with(t, k, cc);
                    if _cvt_int(_f32(_f32(i64_to_f64(u)) / _f32(i64_to_f64(r)))) < lo {
                        u = _i32(lo.wrapping_mul(r));
                    }
                }
                let (next_ptr, used) = _alloc(ptr, u);
                ptr = next_ptr;
                e4 = _i32(e4.wrapping_sub(used));
            }
        }
        let mut dur = 0.0;
        if rate != 0.0 {
            dur =
                _f32(i64_to_f64(_i32(_i32(e4.wrapping_sub(reserve)).wrapping_sub(0x1000))) / rate);
            dur = _f32(dur);
            if dur <= 0.0 {
                d8 = true;
            }
        }
        for t in &mut st {
            if t.n < 0 || (t.w.is_none() && !t.flagged) {
                if t.called && t.b != 0 {
                    let (r, k) = (t.r, t.k);
                    let w = match t.w {
                        None => 1.0,
                        Some(wv) => _f32(wv),
                    };
                    let n = if w == 0.0 {
                        _INT_MIN
                    } else {
                        _cvt_int(_f32(dur / w))
                    };
                    let mut u = _cvt_int(_f32(
                        _f32(i64_to_f64(r)) * _f32(_f32(i64_to_f64(n)) / _f32(i64_to_f64(k))),
                    ));
                    u = _i32(u.wrapping_add(_lap_with(t)).wrapping_add(0x2000));
                    if u < 0x3000 {
                        u = 0x3000;
                    }
                    if !t.fillstop {
                        let lo = _min_recs_with(t, k, cc);
                        if _tdiv(u, r) < lo {
                            d8 = true;
                        }
                    }
                    let (next_ptr, used) = _alloc(ptr, u);
                    ptr = next_ptr;
                    let b4 = _tdiv(used, r);
                    if t.n < 0 {
                        t.n = _i32(
                            (b4 - (pystr::floordiv_i64(4096, r).unwrap_or(0) + 1)).wrapping_mul(k),
                        );
                    }
                    e4 = _i32(e4.wrapping_sub(used));
                }
                if t.n < 1 {
                    t.n = 1;
                }
                if t.n < t.k {
                    t.n = t.k;
                }
            }
        }
        if e4 < 0 {
            d8 = true;
        }
        if !d8 {
            ok_exit = true;
            break;
        }
    }
    AllocResult {
        sizes: st.iter().map(|t| t.n).collect(),
        msg1: !ok_exit,
        msg2: e4 < 0,
    }
}

/// Port of `checker/storage.py::_lap`.
pub fn _lap() -> i64 {
    0
}

/// Full implementation of `checker/storage.py::_lap`.
pub fn _lap_with(t: &TableState) -> i64 {
    if t.lapses != 0 {
        _tdiv(_i32(t.lapses.wrapping_mul(16)), t.b) + 1
    } else {
        0
    }
}

/// Port of `checker/storage.py::_tdiv`.
pub fn _tdiv(a: i64, b: i64) -> i64 {
    a.wrapping_div(b)
}

/// Port of `checker/storage.py::_alloc`.
pub fn _alloc(ptr: i64, u: i64) -> (i64, i64) {
    let start = _round_page(ptr);
    let end = _round_page(_i32(start.wrapping_add(u)));
    (end, _i32(end.wrapping_sub(start)))
}

/// Port of `checker/storage.py::_blockfix`.
pub fn _blockfix(_arg: i64) {}

/// Full implementation of `checker/storage.py::_blockfix`.
pub fn _blockfix_with(t: &mut TableState, arg: i64) {
    if t.r != 0 {
        return;
    }
    if t.lapses == 0 {
        t.k = 1;
    } else {
        t.k = pystr::floordiv_i64(_BLOCK_BYTES, t.b).unwrap_or(0).min(arg);
        if t.k < 2 {
            t.k = 1;
            t.lapses = 0;
        }
        if t.kone {
            t.k = 1;
            t.lapses = 0;
        }
    }
    t.r = t.k.wrapping_mul(t.b).wrapping_add(16);
}

/// Port of `checker/storage.py::pool_bytes`.
pub fn pool_bytes(
    profile_name: &str,
    n_declared: i64,
    has_public: bool,
    n_named_const: i64,
) -> i64 {
    let extra = i64::from(has_public) + n_named_const;
    let pages = _pool_pages(profile_name).unwrap_or(0);
    (pages - 4 * (n_declared + extra)) * _PAGE
}

/// Port of `checker/storage.py::spec_of`.
pub fn spec_of() {}

/// Full implementation of `checker/storage.py::spec_of`.
pub fn spec_of_with(m: &TableMeta) -> TableSpec {
    let cls = _size_class(m.size_raw.as_ref());
    let mut flagged = false;
    let n = match cls {
        None => 1,
        Some(SizeClass::Auto) => -1,
        Some(SizeClass::Edge(sign)) => {
            flagged = sign < 0;
            _INT32_MAX
        }
        Some(SizeClass::Inf) => _INT32_MAX,
        Some(SizeClass::Toxic) => {
            flagged = true;
            _INT_MIN
        }
        Some(SizeClass::Fixed(val, fl)) => {
            flagged = fl;
            val
        }
    };
    let di = m.interval_seconds != 0.0 || m.zero_interval;
    TableSpec {
        called: m.called,
        n,
        flagged,
        b: m.bytes_per_record,
        lapses: if di {
            if m.lapses < (1_i64 << 31) {
                m.lapses
            } else {
                _INT_MIN
            }
        } else {
            0
        },
        card: false,
        own_ms: if m.interval_seconds == 0.0 {
            0
        } else {
            pystr::py_round(m.interval_seconds * 1000.0).unwrap_or(0)
        },
        w: m.call_w,
        fillstop: m.fillstop,
        kone: false,
        day: false,
        zero_interval: m.zero_interval,
    }
}

/// Port of `checker/storage.py::run`.
pub fn run(_pool: i64) {}

/// Full implementation of `checker/storage.py::run`.
pub fn run_with(metas: &[TableMeta], pool: i64) -> Option<AllocResult> {
    let mut specs: Vec<TableSpec> = metas.iter().map(spec_of_with).collect();
    for sp in &mut specs {
        if sp.zero_interval
            && let Some(w) = sp.w
            && w != 0.0
        {
            sp.own_ms = pystr::py_round(w * 1000.0).unwrap_or(0);
        }
    }
    if specs.iter().any(|sp| sp.b == 0) {
        return None;
    }
    Some(allocate_with(&specs, pool))
}

/// Port of `checker/storage.py::cr6_table_oom`.
pub fn cr6_table_oom(_auto_n: Option<i64>, _name: &str) -> Option<String> {
    None
}

/// Full implementation of `checker/storage.py::cr6_table_oom`.
pub fn cr6_table_oom_with(m: &TableMeta, auto_n: Option<i64>, name: &str) -> Option<String> {
    let cls = _size_class(m.size_raw.as_ref());
    let mut auto = false;
    let n = match cls {
        None => 1,
        Some(SizeClass::Fixed(val, false)) => val,
        Some(SizeClass::Auto | SizeClass::Fixed(_, true)) => {
            let an = auto_n?;
            if an <= -(1_i64 << 31) || an >= (1_i64 << 31) || an == 0 || an == 1 || m.has_card {
                return None;
            }
            auto = true;
            an
        }
        _ => return None,
    };
    let b = m.bytes_per_record;
    if b == 0 || !m.called || b > _INT32_MAX {
        return None;
    }
    let lap = if m.display_interval {
        if m.lapses < (1_i64 << 31) {
            m.lapses
        } else {
            _INT_MIN
        }
    } else {
        0
    };
    let a = if lap != 0 {
        let num = if lap == _INT_MIN {
            0
        } else {
            lap.abs().wrapping_mul(16)
        };
        _tdiv(_wrap32(num), b) + 1
    } else {
        0
    };
    let mm = _wrap32(n.wrapping_add(a));
    let mut k = 1;
    if lap != 0 {
        let blk = pystr::floordiv_i64(1008, b).unwrap_or(0);
        k = if auto { blk } else { blk.min(mm) };
        if k < 2 {
            k = 1;
        }
    }
    let r = k.wrapping_mul(b).wrapping_add(16);
    let c = if n == 1 {
        r
    } else {
        let term =
            pystr::floordiv_i64(2048, r).unwrap_or(0) + _tdiv(mm, k) + if r < 1024 { 1 } else { 3 };
        _wrap32(term.wrapping_mul(r))
    };
    if !(0..=_MAX_ALLOC).contains(&c) {
        return Some(MSG_OOM.to_string());
    }
    if auto && c == 0 {
        Some(pystr::format_named(MSG_ALLOC_TABLE, &[("0", name)]))
    } else {
        None
    }
}

/// Port of `checker/storage.py::_var_words`.
pub fn _var_words(_program: &Program) {}

/// Full implementation of `checker/storage.py::_var_words`.
pub fn _var_words_with(program: &Program, env: &Env, kinds: &[&str]) -> Vec<(i64, i64)> {
    let mut out = Vec::new();
    for st in &program.statements {
        let Stmt::Declaration(decl) = st.as_ref() else {
            continue;
        };
        if !kinds.contains(&decl.kind.as_str()) {
            continue;
        }
        for v in &decl.vars {
            let t = pystr::upper(v.type_.as_deref().unwrap_or("FLOAT"));
            if !matches!(
                t.as_str(),
                "FLOAT" | "LONG" | "BOOLEAN" | "DOUBLE" | "STRING"
            ) {
                continue;
            }
            let dims: Vec<i64> = v
                .dims
                .iter()
                .map(|d| _as_int_local(_fold_local(Some(d.as_ref()), env).as_ref(), 1))
                .collect();
            if dims.len() > 3 || dims.iter().any(|&d| !(1..=_INT32_MAX).contains(&d)) {
                continue;
            }
            let stored = _make_var_stored_len(v, env).unwrap_or(0);
            if t == "STRING" && stored > _INT32_MAX {
                continue;
            }
            let mut n128: i128 = 1;
            for d in dims {
                n128 *= i128::from(d);
            }
            #[allow(
                clippy::cast_possible_truncation,
                reason = "low 32 bits + bit 32 preserve both _wrap32 and >= 2^31"
            )]
            let n = if n128 > i128::from(i64::MAX) {
                (1_i64 << 32) | ((n128 as i64) & 0xFFFF_FFFF)
            } else {
                n128 as i64
            };
            let words = if t == "DOUBLE" {
                2
            } else if t == "STRING" {
                pystr::floordiv_i64(stored, 4).unwrap_or(0)
            } else {
                1
            };
            out.push((n, words));
        }
    }
    out
}

/// Port of `checker/storage.py::_segment_oom`.
pub fn _segment_oom() -> bool {
    false
}

/// Full implementation of `checker/storage.py::_segment_oom`.
pub fn _segment_oom_with(words: &[(i64, i64)]) -> bool {
    let mut total: i64 = 4;
    for &(n, w) in words {
        total = _wrap32(total.wrapping_add(_wrap32(_wrap32(n).wrapping_mul(w))));
    }
    let req = _wrap32(total.wrapping_mul(4));
    !(0..=_MAX_ALLOC).contains(&req)
}

/// Port of `checker/storage.py::variables_oom`.
pub fn variables_oom(_program: &Program) -> bool {
    false
}

/// Full implementation of `checker/storage.py::variables_oom`.
pub fn variables_oom_with(program: &Program, profile: &Profile, env: &Env) -> bool {
    matches!(profile.name, "cr6" | "cr1000x")
        && ["DIM", "PUBLIC"]
            .iter()
            .any(|&k| _segment_oom_with(&_var_words_with(program, env, &[k])))
}

/// Port of `checker/storage.py::string_length_oom_line`.
pub fn string_length_oom_line(_program: &Program) -> Option<i64> {
    None
}

/// Full implementation of `checker/storage.py::string_length_oom_line`.
pub fn string_length_oom_line_with(program: &Program, diags: &Diagnostics) -> Option<i64> {
    let tagged: BTreeSet<usize> = diags
        .errors()
        .into_iter()
        .filter(|d| d.stmt != 0 && d.message.starts_with("Variable illegal in Parameter:"))
        .map(|d| d.stmt)
        .collect();
    if tagged.is_empty() {
        return None;
    }
    for st in &program.statements {
        match st.as_ref() {
            Stmt::Declaration(decl) => {
                for v in &decl.vars {
                    if tagged.contains(&rc_id(v)) {
                        let has_eq = matches!(
                            v.string_size.as_deref(),
                            Some(Expr::Binary(b)) if b.op == "="
                        );
                        let init = v.init.as_ref().is_some_and(|vec| !vec.is_empty()) || has_eq;
                        return Some(if init { st.line() } else { program.last_line });
                    }
                }
            }
            Stmt::SubDef(sub) => {
                for p in &sub.params {
                    if tagged.contains(&rc_id(p)) {
                        let has_eq = matches!(
                            p.string_size.as_deref(),
                            Some(Expr::Binary(b)) if b.op == "="
                        );
                        return Some(if has_eq { st.line() } else { program.last_line });
                    }
                }
            }
            _ => {}
        }
    }
    None
}

/// Port of `checker/storage.py::check_pass1`.
pub fn check_pass1(_program: &Program) {}

/// Full implementation of `checker/storage.py::check_pass1`.
pub fn check_pass1_with(program: &Program, profile: &Profile, diags: &mut Diagnostics, env: &Env) {
    let mut line = string_length_oom_line_with(program, diags);
    if line.is_none() && variables_oom_with(program, profile, env) {
        line = Some(program.last_line);
    }
    if let Some(line) = line {
        diags.error(
            line,
            MSG_OOM,
            "",
            ShowFile::Bool(true),
            0,
            "out_of_memory",
            "",
        );
    }
}

/// Port of `checker/storage.py::public_table_oom`.
pub fn public_table_oom(_program: &Program) -> bool {
    false
}

/// Full implementation of `checker/storage.py::public_table_oom`.
pub fn public_table_oom_with(program: &Program, env: &Env) -> bool {
    let words = _var_words_with(program, env, &["PUBLIC"]);
    if words.iter().any(|&(n, _)| n >= (1_i64 << 31)) {
        return false;
    }
    let total_words: i128 = words
        .iter()
        .map(|&(n, w)| i128::from(n) * i128::from(w))
        .sum();
    let limit = i128::from(pystr::floordiv_i64(_MAX_ALLOC - 32, 2).unwrap_or(0));
    total_words * 4 > limit
}

/// Port of `checker/storage.py::check`.
pub fn check(_program: &Program) {}

/// Full implementation of `checker/storage.py::check`.
pub fn check_with(
    program: &Program,
    profile: &'static Profile,
    tables: &[TableStorageEntry],
    diags: &mut Diagnostics,
) {
    if matches!(profile.name, "cr6" | "cr1000x") && !diags.has_errors() {
        let mut consts = std::rc::Rc::new(std::cell::RefCell::new(program.consts.clone()));
        let env = Env::with_consts(profile, std::mem::take(&mut consts));
        if public_table_oom_with(program, &env) {
            diags.error(0, MSG_OOM, "", ShowFile::Bool(true), 0, "out_of_memory", "");
        } else {
            let msgs: Vec<Option<String>> = tables
                .iter()
                .map(|t| cr6_table_oom_with(&t.storage, t.storage.n_raw, &t.name))
                .collect();
            if msgs.iter().any(|m| m.as_deref() == Some(MSG_OOM)) {
                diags.error(0, MSG_OOM, "", ShowFile::Bool(true), 0, "out_of_memory", "");
            } else if let Some(first_msg) = msgs.into_iter().flatten().next() {
                diags.error_basic(0, &first_msg, "");
            }
        }
        return;
    }
    if _pool_pages(profile.name).is_none() || diags.has_errors() {
        return;
    }
    let Some(first) = tables.first() else {
        return;
    };
    if let Some(alloc) = &first.storage.alloc
        && alloc.msg1
    {
        diags.error_basic(0, MSG_ALLOC, "");
        if alloc.msg2 {
            diags.error_basic(0, MSG_STORAGE, "");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profiles;
    use crate::syntax::{
        Binary, Declaration, Node, Number, NumberValue, OrderedMap, StmtBase, StmtRef, VarDecl,
    };
    use std::rc::Rc;

    #[test]
    fn f32_cvt_wrap_round_tdiv_alloc_match_python() {
        assert_eq!(_f32(0.0), 0.0);
        assert!((_f32(1.1) - 1.100_000_023_841_858).abs() < 1e-15);
        assert_eq!(_f32(1e39), f64::INFINITY);
        assert_eq!(_f32(-1e39), f64::NEG_INFINITY);

        assert_eq!(_cvt_int(3.9), 3);
        assert_eq!(_cvt_int(-3.9), -3);
        assert_eq!(_cvt_int(2_147_483_648.0), -2_147_483_648);
        assert_eq!(_cvt_int(-2_147_483_648.0), -2_147_483_648);
        assert_eq!(_cvt_int(f64::NAN), -2_147_483_648);

        assert_eq!(_wrap32(0), 0);
        assert_eq!(_wrap32(0x7FFF_FFFF), 2_147_483_647);
        assert_eq!(_wrap32(0x8000_0000), -2_147_483_648);
        assert_eq!(_wrap32(-1), -1);
        assert_eq!(_wrap32(1_i64 << 32), 0);
        assert_eq!(_i32(0x8000_0000), -2_147_483_648);

        assert_eq!(_round_page(0), 0);
        assert_eq!(_round_page(1), 4096);
        assert_eq!(_round_page(4096), 4096);
        assert_eq!(_round_page(4097), 8192);
        assert_eq!(_round_page(0xFFFF_FFFF), 0);

        assert_eq!(_tdiv(7, 3), 2);
        assert_eq!(_tdiv(-7, 3), -2);
        assert_eq!(_tdiv(7, -3), -2);
        assert_eq!(_tdiv(-7, -3), 2);

        assert_eq!(_alloc(0x0060_0000, 0x2010), (6_303_744, 12_288));
        assert_eq!(_alloc(0x0060_0001, 0x1000), (6_299_648, 4096));
    }

    #[test]
    fn preserve_charge_and_pool_bytes_match_python() {
        assert_eq!(preserve_charge(16, 4, 0, 10, 100, 20), 15_364_096);
        assert_eq!(preserve_charge(32, 4, 10, 10, 1000, 100), 6_148_096);
        assert_eq!(pool_bytes("cr300", 2, true, 1), 10_420_224);
        assert_eq!(pool_bytes("cr350", 3, false, 0), 52_379_648);
    }

    #[test]
    fn min_recs_allocate_and_run_match_python() {
        let mk = |flagged, day, own_ms, w| TableState {
            flagged,
            own_ms,
            w,
            day,
            ..TableState::default()
        };
        assert_eq!(_min_recs_with(&mk(true, false, 0, Some(1.0)), 1, 0), 0);
        assert_eq!(_min_recs_with(&mk(false, true, 0, Some(1.0)), 2, 5), 0);
        assert_eq!(
            _min_recs_with(&mk(false, false, 500, Some(1.0)), 1, 15),
            630
        );
        assert_eq!(_min_recs_with(&mk(false, false, 0, None), 1, 0), 63072);
        assert_eq!(_min_recs_with(&mk(false, false, 0, Some(0.0)), 1, 10), 31);
        assert_eq!(_min_recs_with(&mk(false, false, 0, Some(1.0)), 1, 0), 3153);

        let sp = TableSpec {
            called: true,
            n: -1,
            b: 12,
            w: Some(1.0),
            ..TableSpec::default()
        };
        let sp2 = TableSpec {
            called: true,
            n: 100,
            b: 16,
            lapses: 10,
            w: Some(10.0),
            ..TableSpec::default()
        };
        let sp3 = TableSpec {
            called: false,
            n: 50,
            b: 16,
            w: None,
            ..TableSpec::default()
        };
        let res = allocate_with(&[sp, sp2, sp3], pool_bytes("cr300", 3, true, 0));
        assert_eq!(
            res,
            AllocResult {
                sizes: vec![371_272, 100, 1],
                msg1: false,
                msg2: false,
            }
        );

        let sp_auto = TableSpec {
            called: true,
            n: -1,
            b: 128,
            w: Some(0.1),
            ..TableSpec::default()
        };
        let sp_fixed = TableSpec {
            called: true,
            n: 100_000,
            b: 128,
            w: Some(1.0),
            ..TableSpec::default()
        };
        let res_fail = allocate_with(&[sp_fixed, sp_auto], pool_bytes("cr300", 2, true, 0));
        assert_eq!(
            res_fail,
            AllocResult {
                sizes: vec![100_000, 56],
                msg1: true,
                msg2: true,
            }
        );

        let m_zero = TableMeta {
            size_raw: Some(ConstValue::Int(-1)),
            called: true,
            bytes_per_record: 16,
            zero_interval: true,
            call_w: Some(2.5),
            ..TableMeta::default()
        };
        let res_zero = run_with(&[m_zero], pool_bytes("cr300", 1, false, 0));
        assert_eq!(
            res_zero,
            Some(AllocResult {
                sizes: vec![326_783],
                msg1: false,
                msg2: false,
            })
        );
    }

    #[test]
    fn cr6_table_oom_and_pass1_checks() {
        let m1 = TableMeta {
            size_raw: Some(ConstValue::Int(-1)),
            called: true,
            bytes_per_record: 16,
            call_w: Some(1.0),
            ..TableMeta::default()
        };
        assert_eq!(cr6_table_oom_with(&m1, Some(0), "T1"), None);
        assert_eq!(cr6_table_oom_with(&m1, Some(1000), "T1"), None);

        let m2 = TableMeta {
            size_raw: Some(ConstValue::Int(10_000_000)),
            called: true,
            bytes_per_record: 64,
            lapses: 5,
            interval_seconds: 1.0,
            call_w: Some(1.0),
            display_interval: true,
            ..TableMeta::default()
        };
        assert_eq!(
            cr6_table_oom_with(&m2, None, "T2").as_deref(),
            Some(MSG_OOM)
        );

        let dim_expr = Rc::new(Expr::Number(Number {
            value: NumberValue::Int(40_000_000),
            ..Number::default()
        }));
        let v = Rc::new(VarDecl {
            name: "x".to_string(),
            dims: vec![dim_expr],
            type_: Some("FLOAT".to_string()),
            ..VarDecl::default()
        });
        let decl: StmtRef = Rc::new(Stmt::Declaration(Declaration {
            base: StmtBase {
                node: Node::new(2, ""),
                ..StmtBase::default()
            },
            kind: "DIM".to_string(),
            vars: vec![v.clone()],
        }));
        let prog = Program {
            name: "test.cr6".to_string(),
            statements: vec![decl],
            last_line: 10,
            consts: OrderedMap::new(),
            ..Program::default()
        };
        let profile = profiles::get("cr6").unwrap();
        let env = Env::new(profile);
        assert!(variables_oom_with(&prog, profile, &env));

        let mut diags = Diagnostics::new();
        check_pass1_with(&prog, profile, &mut diags, &env);
        assert_eq!(diags.errors().len(), 1);
        assert_eq!(diags.errors()[0].line, 10);
        assert_eq!(diags.errors()[0].message, MSG_OOM);
        assert_eq!(diags.errors()[0].kind, "out_of_memory");

        // String length OOM with initializer reports on declaration line.
        let v_str = Rc::new(VarDecl {
            name: "s".to_string(),
            type_: Some("STRING".to_string()),
            string_size: Some(Rc::new(Expr::Binary(Binary {
                op: "=".to_string(),
                ..Binary::default()
            }))),
            ..VarDecl::default()
        });
        let decl_str: StmtRef = Rc::new(Stmt::Declaration(Declaration {
            base: StmtBase {
                node: Node::new(4, ""),
                ..StmtBase::default()
            },
            kind: "PUBLIC".to_string(),
            vars: vec![v_str.clone()],
        }));
        let prog_str = Program {
            name: "test.cr6".to_string(),
            statements: vec![decl_str],
            last_line: 20,
            ..Program::default()
        };
        let mut diags_str = Diagnostics::new();
        diags_str.error(
            4,
            "Variable illegal in Parameter: bad",
            "",
            ShowFile::Bool(true),
            rc_id(&v_str),
            "",
            "",
        );
        assert_eq!(string_length_oom_line_with(&prog_str, &diags_str), Some(4));
    }
}
