//! Port of `checker/scan_time.py`.

#![allow(
    dead_code,
    unused_variables,
    clippy::cast_precision_loss,
    clippy::too_many_arguments,
    clippy::used_underscore_items,
    reason = "port keeps Python signatures, leading-underscore names, and int->float semantics"
)]

use std::rc::Rc;
use std::sync::LazyLock;

use crate::consteval::{self, Env};
use crate::profiles::{self, Profile};
use crate::pyre;
use crate::pystr;
use crate::syntax::{ConstValue, Expr, ExprRef, Instruction, Program, Stmt, StmtRef};

const US_PER_UNIT: [i64; 6] = [
    1,
    1_000,
    1_000_000,
    60_000_000,
    3_600_000_000,
    86_400_000_000,
];
pub const UNIT_FACTOR: [i64; 6] = [0, 10, 10_000, 600_000, 36_000_000, 864_000_000];
pub const UNIT_NAMES: [&str; 6] = ["uSec", "mSec", "Sec", "Min", "Hr", "Day"];
pub const SCAN_UNITS: [i64; 3] = [1, 2, 3];

static NEXT_SCAN_RE: LazyLock<pyre::Regex> =
    LazyLock::new(|| pyre::Regex::new(r"(?i)\s*Next\s+Scan\b", 0));
static NEXT_SUBSCAN_RE: LazyLock<pyre::Regex> =
    LazyLock::new(|| pyre::Regex::new(r"(?i)\s*Next\s+SubScan\b", 0));

fn us_per_unit(unit: i64) -> Option<i64> {
    usize::try_from(unit)
        .ok()
        .and_then(|i| US_PER_UNIT.get(i).copied())
}

/// Exact `int(x) % 2**32` for any finite `f64` (including `|x| >= 2^63`).
fn trunc_mod_u32(x: f64) -> i64 {
    let bits = x.to_bits();
    let sign = (bits >> 63) != 0;
    let exp_bits = i64::try_from((bits >> 52) & 0x7ff).unwrap_or(0);
    if exp_bits == 0 {
        return 0;
    }
    let frac = bits & ((1_u64 << 52) - 1);
    let mant = (1_u64 << 52) | frac;
    let exp = exp_bits - 1023 - 52;
    let rem_u32: u64 = if exp < 0 {
        let shift = -exp;
        if shift >= 53 {
            0
        } else {
            (mant >> shift) & 0xFFFF_FFFF
        }
    } else if exp >= 32 {
        0
    } else {
        (mant << exp) & 0xFFFF_FFFF
    };
    let rem = i64::try_from(rem_u32).unwrap_or(0);
    if sign && rem != 0 {
        (1_i64 << 32) - rem
    } else {
        rem
    }
}

/// Port of `checker/scan_time.py::delay_us`.
pub fn delay_us(mut value: f64, unit: i64) -> i64 {
    if !(-2_147_483_648.0..2_147_483_648.0).contains(&value) {
        let b = value.to_le_bytes();
        value = f64::from(f32::from_le_bytes([b[0], b[1], b[2], b[3]]));
        if !value.is_finite() {
            return 1_i64 << 31;
        }
    }
    let us = us_per_unit(unit).expect("invalid delay unit");
    trunc_mod_u32(value * (us as f64))
}

/// Port of `checker/scan_time.py::excite_delay_us`.
pub fn excite_delay_us(value: f64) -> i64 {
    let l = consteval::to_long(&ConstValue::Float(value));
    pystr::pymod_i64(l, 1_i64 << 32).unwrap_or(0)
}

/// Port of `checker/scan_time.py::task_memory_overflow`.
pub fn task_memory_overflow(us: i64) -> bool {
    (pystr::floordiv_i64(us, 43_000).unwrap_or(0) & 0x8000) != 0
}

/// Port of `checker/scan_time.py::amount`.
pub fn amount(excess: i64, unit: i64) -> i64 {
    if unit == 0 {
        excess * 100
    } else {
        let factor = usize::try_from(unit)
            .ok()
            .and_then(|i| UNIT_FACTOR.get(i).copied())
            .expect("invalid unit factor");
        -pystr::floordiv_i64(-excess, factor).unwrap_or(0)
    }
}

/// Port of `checker/scan_time.py::_f32`.
#[allow(clippy::cast_possible_truncation, reason = "C float32 narrowing cast")]
pub fn _f32(x: f64) -> f64 {
    f64::from(x as f32)
}

/// Port of `checker/scan_time.py::i32`.
pub fn i32(x: i64) -> i64 {
    (x.rem_euclid(1_i64 << 32) ^ (1_i64 << 31)) - (1_i64 << 31)
}

/// Port of `checker/scan_time.py::_cint`.
pub fn _cint(x: f64) -> i64 {
    if (-2_147_483_648.0..2_147_483_648.0).contains(&x) {
        pystr::py_int_from_f64(x).unwrap_or(-2_147_483_648)
    } else {
        -2_147_483_648
    }
}

/// Port of `checker/scan_time.py::_cdiv`.
pub fn _cdiv(a: i64, b: i64) -> i64 {
    a.checked_div(b).unwrap_or(0)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum ModelKind {
    #[default]
    Cr6,
    Cr1000x,
}

/// Port of `checker/scan_time.py::Model`.
#[derive(Debug, Clone)]
pub struct Model {
    pub overhead: i64,
    pub refund: i64,
    pub integ_num: f64,
    pub integ_zero: i64,
    pub default_settle: i64,
    pub round_up: i64,
    pub busy_extra: i64,
    kind: ModelKind,
}

impl Default for Model {
    fn default() -> Self {
        CR6_MODEL.clone()
    }
}

pub const CR6_MODEL: Model = Model {
    overhead: 78,
    refund: 77,
    integ_num: 93750.0,
    integ_zero: 24,
    default_settle: 500,
    round_up: 0x28,
    busy_extra: 13,
    kind: ModelKind::Cr6,
};

pub const CR1000X_MODEL: Model = Model {
    overhead: 16,
    refund: 15,
    integ_num: 31250.0,
    integ_zero: 8,
    default_settle: 450,
    round_up: 0x50,
    busy_extra: 17,
    kind: ModelKind::Cr1000x,
};

pub fn model_for(name: &str) -> Option<&'static Model> {
    match name {
        "cr6" => Some(&CR6_MODEL),
        "cr1000x" => Some(&CR1000X_MODEL),
        _ => None,
    }
}

impl Model {
    /// Port of `checker/scan_time.py::Model::integ_code`.
    pub fn integ_code(&self, fnotch: f64) -> Option<i64> {
        if _f32(fnotch) == 0.0 {
            return Some(self.integ_zero);
        }
        if !fnotch.is_finite() || fnotch <= 0.0 {
            return None;
        }
        let code = pystr::py_int_from_f64(_f32(_f32(self.integ_num) / _f32(fnotch)) + 0.5)?;
        if code > 0xFFFF {
            return None;
        }
        Some(if code == 0 { 1 } else { code })
    }

    /// Port of `checker/scan_time.py::Model::integ_time`.
    pub fn integ_time(&self, k: i64) -> i64 {
        match self.kind {
            ModelKind::Cr6 => CR6::default().integ_time(k),
            ModelKind::Cr1000x => CR1000X::default().integ_time(k),
        }
    }

    /// Port of `checker/scan_time.py::Model::settle_code`.
    pub fn settle_code(&self, settling: i64) -> i64 {
        match self.kind {
            ModelKind::Cr6 => CR6::default().settle_code(settling),
            ModelKind::Cr1000x => CR1000X::default().settle_code(settling),
        }
    }

    /// Port of `checker/scan_time.py::Model::settle_units`.
    pub fn settle_units(&self, code: i64) -> i64 {
        match self.kind {
            ModelKind::Cr6 => CR6::default().settle_units(code),
            ModelKind::Cr1000x => CR1000X::default().settle_units(code),
        }
    }

    /// Port of `checker/scan_time.py::Model::open_detect_code`.
    pub fn open_detect_code(&self) -> i64 {
        match self.kind {
            ModelKind::Cr6 => CR6::default().open_detect_code(),
            ModelKind::Cr1000x => CR1000X::default().open_detect_code(),
        }
    }

    /// Port of `checker/scan_time.py::Model::fixed_code`.
    pub fn fixed_code(&self, constant: i64) -> i64 {
        match self.kind {
            ModelKind::Cr6 => CR6::default().fixed_code(constant),
            ModelKind::Cr1000x => CR1000X::default().fixed_code(constant),
        }
    }
}

/// Port of `checker/scan_time.py::_CR6`.
#[derive(Debug, Default)]
pub struct CR6 {}

impl CR6 {
    /// Port of `checker/scan_time.py::_CR6::integ_time`.
    pub fn integ_time(&self, k: i64) -> i64 {
        pystr::floordiv_i64(k << 5, 3).unwrap_or(0) + 1
    }

    /// Port of `checker/scan_time.py::_CR6::settle_code`.
    pub fn settle_code(&self, settling: i64) -> i64 {
        let s = if settling == 0 { 500 } else { settling };
        _cint(_f32(_f32(_f32(i32(s + 810) as f64) * 3.0) / 32.0) + 0.5)
    }

    /// Port of `checker/scan_time.py::_CR6::settle_units`.
    pub fn settle_units(&self, code: i64) -> i64 {
        i32(_cint(_f32(_f32((code as f64) * 32.0) / 3.0)) + 5)
    }

    /// Port of `checker/scan_time.py::_CR6::open_detect_code`.
    pub fn open_detect_code(&self) -> i64 {
        5
    }

    /// Port of `checker/scan_time.py::_CR6::fixed_code`.
    pub fn fixed_code(&self, constant: i64) -> i64 {
        pystr::py_int_from_f64(_f32(_f32((constant as f64) * 3.0) / 32.0) + 0.5).unwrap_or(0)
    }
}

/// Port of `checker/scan_time.py::_CR1000X`.
#[derive(Debug, Default)]
pub struct CR1000X {}

impl CR1000X {
    /// Port of `checker/scan_time.py::_CR1000X::integ_time`.
    pub fn integ_time(&self, k: i64) -> i64 {
        k * 32 + 180
    }

    /// Port of `checker/scan_time.py::_CR1000X::settle_code`.
    pub fn settle_code(&self, settling: i64) -> i64 {
        let s = if settling == 0 { 450 } else { settling };
        _cint(_f32(_f32(s as f64) / 32.0) + 0.5)
    }

    /// Port of `checker/scan_time.py::_CR1000X::settle_units`.
    pub fn settle_units(&self, code: i64) -> i64 {
        i32(_cint(_f32((code as f64) * 32.0)) + 5)
    }

    /// Port of `checker/scan_time.py::_CR1000X::open_detect_code`.
    pub fn open_detect_code(&self) -> i64 {
        2
    }

    /// Port of `checker/scan_time.py::_CR1000X::fixed_code`.
    pub fn fixed_code(&self, constant: i64) -> i64 {
        pystr::py_int_from_f64(_f32((constant as f64) / 32.0) + 0.5).unwrap_or(0)
    }
}

/// Port of `checker/scan_time.py::analog_units`.
pub fn analog_units(
    m: &Model,
    reps: i64,
    gain: i64,
    fnotch: f64,
    settling: i64,
    single_ended: bool,
    meas_off: bool,
    reverse: bool,
    excite: bool,
    open_detect: bool,
    groups: i64,
) -> Option<i64> {
    let k = m.integ_code(fnotch)?;
    if !(1..=100_000).contains(&reps) {
        return None;
    }
    let integ = m.integ_time(k);
    let settle = m
        .settle_units(m.settle_code(settling) + if open_detect { m.open_detect_code() } else { 0 });
    let mut total = i32((settle + integ) * reps);
    if gain == 3 {
        total = i32(total + reps * settle);
    }
    if excite {
        total = i32(total * 2);
    }
    total = i32(total * groups);
    if meas_off && single_ended {
        total = i32(total + integ + settle);
    }
    if reverse && !single_ended {
        total = i32(total * 2);
    }
    if open_detect {
        total = i32(total + reps * 0x32);
    }
    Some(_cdiv(i32(total + m.round_up), 100) + 1)
}

/// Port of `checker/scan_time.py::panel_units`.
pub fn panel_units(m: &Model, fnotch: f64) -> Option<i64> {
    let k = m.integ_code(fnotch)?;
    Some(
        pystr::floordiv_i64(
            m.settle_units(m.fixed_code(900)) + m.integ_time(k) + m.round_up,
            100,
        )? + 1,
    )
}

/// Port of `checker/scan_time.py::battery_units`.
pub fn battery_units(m: &Model) -> i64 {
    pystr::floordiv_i64(
        m.settle_units(m.fixed_code(200)) + m.integ_time(10) + m.round_up,
        100,
    )
    .unwrap_or(0)
        + 1
}

/// Port of `checker/scan_time.py::Budget`.
#[derive(Debug, Clone)]
pub struct Budget {
    pub scan: StmtRef,
    pub end: StmtRef,
    pub value: i64,
    pub unit: i64,
    pub interval: i64,
    pub msg_unit: i64,
    pub total: i64,
    pub analog: bool,
    pub pulse_only: bool,
    pub max_integ: i64,
    pub partial: bool,
    pub sub_short: Vec<(StmtRef, i64, i64)>,
}

impl Default for Budget {
    fn default() -> Self {
        let dummy = Rc::new(Stmt::Instruction(Instruction::default()));
        Self {
            scan: dummy.clone(),
            end: dummy,
            value: 0,
            unit: 0,
            interval: 0,
            msg_unit: 0,
            total: 0,
            analog: false,
            pulse_only: true,
            max_integ: 0,
            partial: false,
            sub_short: Vec::new(),
        }
    }
}

/// Port of `checker/scan_time.py::_value`.
pub fn _value(expr: Option<&Expr>, env: &Env) -> Option<ConstValue> {
    let expr = expr?;
    if !consteval::is_constant(Some(expr), env) {
        return None;
    }
    consteval::value_of(Some(expr), env).ok()
}

/// Port of `checker/scan_time.py::_num`.
pub fn _num(expr: Option<&Expr>, env: &Env) -> Option<f64> {
    match _value(expr, env)? {
        ConstValue::Int(i) => Some(i as f64),
        ConstValue::Float(f) | ConstValue::CFloat(f) if f.is_finite() => Some(f),
        _ => None,
    }
}

/// Port of `checker/scan_time.py::_long`.
pub fn _long(expr: Option<&Expr>, env: &Env) -> Option<i64> {
    match _value(expr, env)? {
        ConstValue::Str(_) => None,
        v => Some(consteval::to_long(&v)),
    }
}

/// Port of `checker/scan_time.py::_settling`.
pub fn _settling(expr: Option<&Expr>, env: &Env) -> Option<i64> {
    if let Some(Expr::Name(n)) = expr
        && pystr::upper(&n.name) == "INF"
        && !env.consts.borrow().contains_key("INF")
    {
        return Some(consteval::to_long(&ConstValue::Float(f64::INFINITY)));
    }
    if let Some(ConstValue::Str(s)) = _value(expr, env) {
        return Some(consteval::to_long(&ConstValue::Str(s)));
    }
    _long(expr, env)
}

fn range_info(code: i64) -> Option<(i64, bool)> {
    match code {
        800 => Some((0, false)),
        802 => Some((1, false)),
        804 => Some((2, false)),
        810 => Some((3, false)),
        820 => Some((0, true)),
        822 => Some((1, true)),
        824 => Some((2, true)),
        830 => Some((3, true)),
        _ => None,
    }
}

fn is_plain_range(info: (i64, bool)) -> bool {
    matches!(info, (0..=2, false))
}

struct AnalogSpec {
    reps: usize,
    range: usize,
    flag: usize,
    settle: usize,
    fnotch: usize,
    single_ended: bool,
}

fn analog_spec(kw: &str) -> Option<AnalogSpec> {
    match kw {
        "VOLTSE" | "CURRENTSE" => Some(AnalogSpec {
            reps: 2,
            range: 3,
            flag: 5,
            settle: 6,
            fnotch: 7,
            single_ended: true,
        }),
        "VOLTDIFF" => Some(AnalogSpec {
            reps: 2,
            range: 3,
            flag: 5,
            settle: 6,
            fnotch: 7,
            single_ended: false,
        }),
        "TCSE" => Some(AnalogSpec {
            reps: 2,
            range: 3,
            flag: 7,
            settle: 8,
            fnotch: 9,
            single_ended: true,
        }),
        "TCDIFF" => Some(AnalogSpec {
            reps: 2,
            range: 3,
            flag: 7,
            settle: 8,
            fnotch: 9,
            single_ended: false,
        }),
        _ => None,
    }
}

#[derive(Clone, Copy)]
pub struct BridgeSpec {
    ranges: &'static [usize],
    channel: usize,
    meas_pex: usize,
    rev_ex: usize,
    rev_diff: Option<usize>,
    settle: usize,
    fnotch: usize,
    single_ended: bool,
    groups: i64,
}

fn bridge_spec(kw: &str) -> Option<BridgeSpec> {
    match kw {
        "BRHALF" => Some(BridgeSpec {
            ranges: &[3],
            channel: 4,
            meas_pex: 6,
            rev_ex: 8,
            rev_diff: None,
            settle: 9,
            fnotch: 10,
            single_ended: true,
            groups: 1,
        }),
        "BRHALF3W" => Some(BridgeSpec {
            ranges: &[3],
            channel: 4,
            meas_pex: 6,
            rev_ex: 8,
            rev_diff: None,
            settle: 9,
            fnotch: 10,
            single_ended: true,
            groups: 2,
        }),
        "BRFULL" => Some(BridgeSpec {
            ranges: &[3],
            channel: 4,
            meas_pex: 6,
            rev_ex: 8,
            rev_diff: Some(9),
            settle: 10,
            fnotch: 11,
            single_ended: false,
            groups: 1,
        }),
        "BRHALF4W" | "BRFULL6W" => Some(BridgeSpec {
            ranges: &[3, 4],
            channel: 5,
            meas_pex: 7,
            rev_ex: 9,
            rev_diff: Some(10),
            settle: 11,
            fnotch: 12,
            single_ended: false,
            groups: 2,
        }),
        _ => None,
    }
}

/// Port of `checker/scan_time.py::_bridge_units`.
pub fn _bridge_units(
    m: &Model,
    args: &[Option<ExprRef>],
    env: &Env,
    spec: BridgeSpec,
) -> Option<(i64, bool, i64)> {
    if args.len() < spec.fnotch {
        return None;
    }
    let arg_expr = |pos_1based: usize| args[pos_1based - 1].as_deref();
    let reps = _long(arg_expr(2), env)?;
    let chan = _long(arg_expr(spec.channel), env)?;
    let mut meas = _long(arg_expr(spec.meas_pex), env)?;
    let revex = _long(arg_expr(spec.rev_ex), env)?;
    let revdiff = match spec.rev_diff {
        Some(dp) => _long(arg_expr(dp), env)?,
        None => 0,
    };
    let mut rngs = Vec::with_capacity(spec.ranges.len());
    for &r in spec.ranges {
        rngs.push(_long(arg_expr(r), env)?);
    }
    let settling = _settling(arg_expr(spec.settle), env)?;
    let fn_val = _num(arg_expr(spec.fnotch), env)?;

    if chan <= 0
        || !(1..=100_000).contains(&reps)
        || rngs.iter().any(|&r| range_info(r).is_none())
        || (spec.groups > 1
            && rngs
                .iter()
                .any(|&r| range_info(r).is_none_or(|info| !is_plain_range(info))))
    {
        return None;
    }
    meas = meas.max(1);
    let mut chunks = Vec::new();
    if meas == reps {
        chunks.push(reps);
    } else {
        let q = pystr::floordiv_i64(reps, meas).unwrap_or(0);
        let r = pystr::pymod_i64(reps, meas).unwrap_or(0);
        for _ in 0..q {
            chunks.push(meas);
        }
        if r != 0 {
            chunks.push(r);
        }
    }
    let first_range = range_info(rngs[0])?;
    let mut total: i64 = 0;
    for n in chunks {
        let units = analog_units(
            m,
            n,
            first_range.0,
            fn_val,
            settling,
            spec.single_ended,
            false,
            (revdiff & 0xFF) != 0,
            (revex & 0xFF) != 0,
            first_range.1,
            spec.groups,
        )?;
        total = i32(total + units);
    }
    let code = m.integ_code(fn_val)?;
    Some((
        total,
        true,
        pystr::floordiv_i64(m.integ_time(code), 100).unwrap_or(0),
    ))
}

fn profile_static(profile: &Profile) -> &'static Profile {
    profiles::get(profile.name).unwrap_or_else(|_| profiles::get("cr6").expect("cr6 profile"))
}

/// Port of `checker/scan_time.py::instruction_units`.
pub fn instruction_units(
    stmt: &Instruction,
    env: &Env,
    profile: &Profile,
) -> Option<(i64, bool, i64)> {
    let kw = pystr::upper(&stmt.keyword);
    let args = &stmt.args;
    if kw == "DELAY" {
        if args.len() != 3 {
            return None;
        }
        let option = _num(args[0].as_deref(), env)?;
        let value = _num(args[1].as_deref(), env)?;
        let unit = _num(args[2].as_deref(), env)?;
        if pystr::py_int_from_f64(option) != Some(0) {
            return None;
        }
        let unit_i = pystr::py_int_from_f64(unit)?;
        us_per_unit(unit_i)?;
        if (0.0..1.0).contains(&value) {
            return None;
        }
        let micro = delay_us(value, unit_i);
        if micro == 0 || task_memory_overflow(micro) {
            return None;
        }
        if micro >= (1_i64 << 31) {
            return Some((-pystr::floordiv_i64(-micro, 100).unwrap_or(0), false, 0));
        }
        return Some((
            pystr::floordiv_i64(2 * pystr::floordiv_i64(micro, 2).unwrap_or(0) + 100, 100)
                .unwrap_or(0),
            false,
            0,
        ));
    }
    if kw == "EXCITEV" {
        let delay = if args.len() == 3 {
            _long(args[2].as_deref(), env)?
        } else {
            return None;
        };
        let delay = excite_delay_us(delay as f64);
        if task_memory_overflow((delay - 10).max(0)) {
            return None;
        }
        let u = pystr::floordiv_i64(pystr::pymod_i64(delay + 100, 1_i64 << 32).unwrap_or(0), 100)
            .unwrap_or(0);
        return Some((u, false, 0));
    }
    if kw == "PORTSET" {
        return Some((1, false, 0));
    }
    if kw == "SW12" || kw == "PULSECOUNT" {
        return Some((1, false, 0));
    }
    let m = model_for(profile.name)?;
    if kw == "PORTGET" || (kw == "BATTERY" && profile.name == "cr6") {
        return Some((0, false, 0));
    }
    if kw == "BATTERY" {
        return Some((battery_units(m), true, 0));
    }
    if kw == "PANELTEMP" && args.len() == 2 {
        let fn_val = _num(args[1].as_deref(), env)?;
        let units = panel_units(m, fn_val)?;
        if units == 0 {
            return None;
        }
        let code = m.integ_code(fn_val)?;
        return Some((
            2 * units,
            true,
            pystr::floordiv_i64(m.integ_time(code), 100).unwrap_or(0),
        ));
    }
    if let Some(spec) = analog_spec(&kw) {
        if args.len() < spec.fnotch {
            return None;
        }
        let reps = _long(args[spec.reps - 1].as_deref(), env)?;
        let rng = _long(args[spec.range - 1].as_deref(), env)?;
        let flag = _long(args[spec.flag - 1].as_deref(), env)?;
        let settling = _settling(args[spec.settle - 1].as_deref(), env)?;
        let fn_val = _num(args[spec.fnotch - 1].as_deref(), env)?;
        let (gain, mut open_detect) = range_info(rng)?;
        let cr6_current = kw == "CURRENTSE" && profile.name == "cr6";
        let on = (flag & 0xFF) != 0 && !cr6_current;
        open_detect = open_detect && !cr6_current;
        let units = analog_units(
            m,
            reps,
            gain,
            fn_val,
            settling,
            spec.single_ended,
            on && spec.single_ended,
            on && !spec.single_ended,
            false,
            open_detect,
            1,
        )?;
        let code = m.integ_code(fn_val)?;
        return Some((
            units,
            true,
            pystr::floordiv_i64(m.integ_time(code), 100).unwrap_or(0),
        ));
    }
    if let Some(spec) = bridge_spec(&kw) {
        return _bridge_units(m, args, env, spec);
    }
    if (kw == "THERM107" || kw == "THERM108" || kw == "THERM109") && args.len() >= 6 {
        let reps = _long(args[1].as_deref(), env)?;
        let settling = _long(args[4].as_deref(), env)?;
        let fn_val = _num(args[5].as_deref(), env)?;
        if !(1..=255).contains(&reps) || !(0..6_000_000).contains(&settling) {
            return None;
        }
        let units = analog_units(
            m, reps, 0, fn_val, settling, true, false, false, true, false, 1,
        )?;
        if units == 0 {
            return None;
        }
        let code = m.integ_code(fn_val)?;
        return Some((
            units,
            true,
            pystr::floordiv_i64(m.integ_time(code), 100).unwrap_or(0),
        ));
    }
    if kw == "CS616" && args.len() >= 2 {
        let reps = _long(args[1].as_deref(), env)?;
        if (1..=255).contains(&reps) {
            return Some((reps * 5 + 2, false, 0));
        }
        return None;
    }
    None
}

/// Local copy of `instr_rules.py::_scan_raw` until `instr_rules::_scan_raw` is ported.
fn scan_raw(stmt: &Instruction, env: &Env) -> Option<(i64, i64)> {
    let args = &stmt.args;
    if args.len() < 2 {
        return None;
    }
    let a0 = args[0].as_deref()?;
    let a1 = args[1].as_deref()?;
    if !(consteval::is_constant(Some(a0), env) && consteval::is_constant(Some(a1), env)) {
        return None;
    }
    let value = consteval::value_of(Some(a0), env).ok()?;
    let unit = consteval::value_of(Some(a1), env).ok()?;
    if !consteval::is_finite_number(&value) {
        return None;
    }
    let unit_i = match unit {
        ConstValue::Int(u) if us_per_unit(u).is_some() => u,
        ConstValue::Float(f) | ConstValue::CFloat(f)
            if pystr::is_integer(f) && (0.0..=5.0).contains(&f) =>
        {
            pystr::py_int_from_f64(f)?
        }
        _ => return None,
    };
    let val_i = match value {
        ConstValue::Int(i) => i,
        ConstValue::Float(f) | ConstValue::CFloat(f) => {
            if f <= -1.0 || f >= 2_147_483_648.0 {
                return None;
            }
            pystr::py_int_from_f64(f)?
        }
        ConstValue::Str(_) | ConstValue::BigInt(_) => return None,
    };
    if !(0..=2_147_483_647).contains(&val_i) {
        return None;
    }
    Some((val_i, unit_i))
}

fn is_analog_kind(kw: &str) -> bool {
    matches!(
        kw,
        "VOLTSE"
            | "VOLTDIFF"
            | "TCSE"
            | "TCDIFF"
            | "PANELTEMP"
            | "THERM107"
            | "THERM108"
            | "THERM109"
            | "BRHALF"
            | "BRHALF3W"
            | "BRHALF4W"
            | "BRFULL"
            | "BRFULL6W"
            | "RESISTANCE"
            | "CURRENTSE"
    )
}

fn is_known_timed(kw: &str) -> bool {
    matches!(
        kw,
        "VOLTSE"
            | "VOLTDIFF"
            | "TCSE"
            | "TCDIFF"
            | "PANELTEMP"
            | "THERM107"
            | "THERM108"
            | "THERM109"
            | "CS616"
            | "DELAY"
            | "BRHALF"
            | "BRHALF3W"
            | "BRHALF4W"
            | "BRFULL"
            | "BRFULL6W"
            | "RESISTANCE"
            | "CURRENTSE"
            | "PERIODAVG"
            | "AM25T"
            | "THERMISTOR"
            | "VIBRATINGWIRE"
            | "EXCITEV"
            | "EXCITEI"
            | "CALIBRATE"
            | "PANELTEMP2"
            | "RESISTANCE3W"
            | "RESISTANCE2W"
            | "TCPSYC"
            | "MUXSELECT"
    )
}

/// Port of `checker/scan_time.py::budgets`.
pub fn budgets(program: &Program, env: &Env, profile: &Profile, include_zero: bool) -> Vec<Budget> {
    let model = model_for(profile.name).unwrap_or(&CR6_MODEL);
    let mut slow = false;
    let mut cur: Option<Budget> = None;
    let mut frames: Vec<Option<(i64, i64, i64, Option<i64>, i64)>> = Vec::new();
    let mut total: i64 = 0;
    let mut out = Vec::new();

    for stmt_rc in &program.statements {
        let mut kw = match stmt_rc.as_ref() {
            Stmt::Instruction(instr) => pystr::upper(&instr.keyword),
            _ => String::new(),
        };
        let text = stmt_rc.text();
        if kw == "NEXT" && NEXT_SCAN_RE.match_(text).is_some() {
            kw = "NEXTSCAN".to_string();
        } else if kw == "NEXT" && NEXT_SUBSCAN_RE.match_(text).is_some() {
            kw = "NEXTSUBSCAN".to_string();
        }

        if kw == "SLOWSEQUENCE" {
            slow = true;
            continue;
        }
        if let Stmt::Instruction(instr) = stmt_rc.as_ref()
            && kw == "SCAN"
            && !slow
        {
            let mut raw = scan_raw(instr, env);
            if include_zero && instr.args.len() > 1 {
                let v = _value(instr.args[0].as_deref(), env);
                let u = _value(instr.args[1].as_deref(), env);
                #[allow(
                    clippy::float_cmp,
                    reason = "Python `u in (2, 3)` exact float equality"
                )]
                let u_ok = match u {
                    Some(ConstValue::Int(2)) => Some(2),
                    Some(ConstValue::Int(3)) => Some(3),
                    Some(ConstValue::Float(f) | ConstValue::CFloat(f)) if f == 2.0 => Some(2),
                    Some(ConstValue::Float(f) | ConstValue::CFloat(f)) if f == 3.0 => Some(3),
                    _ => None,
                };
                let v_ok = match v {
                    Some(ConstValue::Int(i)) => (0..1).contains(&i) || i > 2_147_483_647,
                    Some(ConstValue::Float(f) | ConstValue::CFloat(f)) => {
                        (0.0..1.0).contains(&f) || f > 2_147_483_647.0
                    }
                    _ => false,
                };
                if let Some(u_val) = u_ok
                    && v_ok
                {
                    raw = Some((0, u_val));
                }
            }
            if let Some((value, unit)) = raw
                && SCAN_UNITS.contains(&unit)
            {
                let us = us_per_unit(unit).unwrap_or(0);
                let interval = pystr::floordiv_i64(value * us, 100).unwrap_or(0);
                cur = Some(Budget {
                    scan: stmt_rc.clone(),
                    end: stmt_rc.clone(),
                    value,
                    unit,
                    interval,
                    msg_unit: unit,
                    total: 0,
                    analog: false,
                    pulse_only: true,
                    max_integ: 0,
                    partial: false,
                    sub_short: Vec::new(),
                });
            } else {
                cur = None;
            }
            frames.clear();
            total = 0;
            continue;
        }
        let Some(ref mut b) = cur else {
            continue;
        };
        if matches!(stmt_rc.as_ref(), Stmt::Instruction(_)) && kw == "NEXTSCAN" {
            if !frames.is_empty() {
                b.partial = true;
            }
            b.end = stmt_rc.clone();
            b.total = total;
            if b.analog {
                b.total = i32(b.total + model.overhead);
                if i32(b.interval - b.total) < 500 {
                    b.total = i32(b.total - model.refund);
                }
            }
            out.push(cur.take().unwrap());
            continue;
        }
        if let Stmt::Instruction(instr) = stmt_rc.as_ref()
            && kw == "SUBSCAN"
        {
            let args: Vec<Option<ConstValue>> = instr
                .args
                .iter()
                .map(|a| _value(a.as_deref(), env))
                .collect();
            let count = if args.len() == 3 {
                match args[2] {
                    Some(ConstValue::Int(c)) if c > 0 => Some(c),
                    _ => None,
                }
            } else {
                None
            };
            let mut ok = count.is_some();
            let mut step: i64 = 0;
            if ok {
                let is_zero = matches!(
                    args[0],
                    Some(ConstValue::Int(0) | ConstValue::Float(0.0) | ConstValue::CFloat(0.0))
                );
                if !is_zero {
                    let u = match args[1] {
                        Some(ConstValue::Int(u_key)) => us_per_unit(u_key),
                        _ => None,
                    };
                    let pos_finite = match &args[0] {
                        Some(ConstValue::Int(i)) => *i > 0,
                        Some(ConstValue::Float(f) | ConstValue::CFloat(f)) => {
                            f.is_finite() && *f > 0.0
                        }
                        _ => false,
                    };
                    if let (Some(u_val), true) = (u, pos_finite) {
                        let a0_int = match args[0] {
                            Some(ConstValue::Int(i)) => Some(i),
                            Some(ConstValue::Float(f) | ConstValue::CFloat(f)) => {
                                pystr::py_int_from_f64(f)
                            }
                            _ => None,
                        };
                        if let Some(v) = a0_int {
                            step = pystr::floordiv_i64(v * u_val, 100).unwrap_or(0);
                        } else {
                            ok = false;
                        }
                    } else {
                        ok = false;
                    }
                }
            }
            if !ok {
                b.partial = true;
            }
            let sub_unit = if ok {
                match args[1] {
                    Some(ConstValue::Int(u_key)) if us_per_unit(u_key).is_some() => Some(u_key),
                    _ => None,
                }
            } else {
                None
            };
            if ok && sub_unit.is_none() {
                b.partial = true;
            }
            frames.push(if ok {
                Some((total, count.unwrap_or(0), step, sub_unit, b.msg_unit))
            } else {
                None
            });
            if let Some(su) = sub_unit {
                b.msg_unit = su;
            }
            continue;
        }
        if kw == "NEXTSUBSCAN" {
            let Some(Some((start, count, step, _sub_unit, saved))) = frames.pop() else {
                continue;
            };
            let body = total - start;
            let mut restore = true;
            if count < 2 || step == 0 || body == 0 {
                total += (count - 1) * body;
            } else if step < body {
                b.sub_short
                    .push((stmt_rc.clone(), amount(body - step, b.msg_unit), b.msg_unit));
                total += (count - 1) * body;
            } else if step - body > 1 {
                total += (count - 1) * step;
                restore = false;
            } else {
                total += (count - 1) * body;
            }
            if restore {
                b.msg_unit = saved;
            }
            continue;
        }
        if let Stmt::Instruction(instr) = stmt_rc.as_ref() {
            if let Some(got) = instruction_units(instr, env, profile) {
                total = i32(total + got.0);
                if kw != "PULSECOUNT" {
                    b.pulse_only = false;
                }
                b.analog = b.analog || got.1;
                b.max_integ = b.max_integ.max(got.2);
            } else if is_known_timed(&kw) {
                b.partial = true;
                b.analog = b.analog || is_analog_kind(&kw);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syntax::{Name, Node, Number, NumberValue, StmtBase};

    fn num_expr(v: NumberValue) -> ExprRef {
        Rc::new(Expr::Number(Number {
            node: Node::default(),
            value: v,
            text: String::new(),
        }))
    }

    fn name_expr(s: &str) -> ExprRef {
        Rc::new(Expr::Name(Name {
            node: Node::default(),
            name: s.to_string(),
        }))
    }

    fn instr_stmt(kw: &str, text: &str, args: Vec<Option<ExprRef>>) -> StmtRef {
        Rc::new(Stmt::Instruction(Instruction {
            base: StmtBase {
                node: Node::new(1, ""),
                text: text.to_string(),
                ..StmtBase::default()
            },
            keyword: kw.to_string(),
            args,
            ..Instruction::default()
        }))
    }

    #[test]
    fn helpers_match_python() {
        assert_eq!(delay_us(10.0, 1), 10_000);
        assert_eq!(delay_us(1e30, 1), 0);
        assert_eq!(delay_us(1e300, 1), 0);
        assert_eq!(excite_delay_us(500.0), 500);
        assert_eq!(excite_delay_us(-1.0), 4_294_967_295);
        assert!(!task_memory_overflow(43_000));
        assert!(task_memory_overflow(43_000 * 0x8000));
        assert_eq!(amount(5, 0), 500);
        assert_eq!(amount(11, 1), 2);
        assert_eq!(_f32(1e300), f64::INFINITY);
        assert_eq!(_f32(1e-50), 0.0);
        assert_eq!(i32(2_147_483_648), -2_147_483_648);
        assert_eq!(_cint(3e9), -2_147_483_648);
        assert_eq!(_cdiv(-7, 2), -3);
    }

    #[test]
    fn model_and_analog_match_python() {
        let cr6 = model_for("cr6").unwrap();
        let cr1000x = model_for("cr1000x").unwrap();
        assert_eq!(cr6.integ_code(0.0), Some(24));
        assert_eq!(cr6.integ_code(60.0), Some(1563));
        assert_eq!(cr6.integ_code(1e-50), Some(24));
        assert_eq!(cr6.integ_code(-1.0), None);
        assert_eq!(cr1000x.integ_code(60.0), Some(521));

        assert_eq!(
            analog_units(cr6, 1, 0, 60.0, 500, true, false, false, false, false, 1),
            Some(181)
        );
        assert_eq!(
            analog_units(cr1000x, 2, 3, 50.0, 0, false, false, true, true, true, 1),
            Some(1699)
        );
        assert_eq!(panel_units(cr6, 60.0), Some(177));
        assert_eq!(battery_units(cr1000x), 8);
    }

    #[test]
    fn instruction_and_budgets_match_python() {
        let cr6 = profiles::get("cr6").unwrap();
        let env = Env::new(cr6);
        let prog = Program::new(
            "test",
            vec![
                instr_stmt(
                    "Scan",
                    "Scan(1, Sec, 0, 0)",
                    vec![
                        Some(num_expr(NumberValue::Int(1))),
                        Some(num_expr(NumberValue::Int(2))),
                        Some(num_expr(NumberValue::Int(0))),
                        Some(num_expr(NumberValue::Int(0))),
                    ],
                ),
                instr_stmt(
                    "SubScan",
                    "SubScan(5, mSec, 3)",
                    vec![
                        Some(num_expr(NumberValue::Int(5))),
                        Some(num_expr(NumberValue::Int(1))),
                        Some(num_expr(NumberValue::Int(3))),
                    ],
                ),
                instr_stmt(
                    "VoltSe",
                    "VoltSe(X, 1, mV1000, 1, True, 0, 60, 1, 0)",
                    vec![
                        Some(name_expr("X")),
                        Some(num_expr(NumberValue::Int(1))),
                        Some(num_expr(NumberValue::Int(800))),
                        Some(num_expr(NumberValue::Int(1))),
                        Some(num_expr(NumberValue::Int(1))),
                        Some(num_expr(NumberValue::Int(0))),
                        Some(num_expr(NumberValue::Int(60))),
                        Some(num_expr(NumberValue::Int(1))),
                        Some(num_expr(NumberValue::Int(0))),
                    ],
                ),
                instr_stmt("Next", "Next SubScan", vec![]),
                instr_stmt("Next", "Next Scan", vec![]),
            ],
        );
        let bs = budgets(&prog, &env, cr6, false);
        assert_eq!(bs.len(), 1);
        let b = &bs[0];
        assert_eq!(b.interval, 10_000);
        assert_eq!(b.total, 1161);
        assert!(b.analog);
        assert!(!b.pulse_only);
        assert_eq!(b.max_integ, 166);
        assert_eq!(b.sub_short.len(), 1);
        assert_eq!((b.sub_short[0].1, b.sub_short[0].2), (32, 1));
    }

    #[test]
    fn instruction_units_match_python() {
        let cr6 = profiles::get("cr6").unwrap();
        let cr1000x = profiles::get("cr1000x").unwrap();
        let env = Env::new(cr6);
        let num = |v: i64| Some(num_expr(NumberValue::Int(v)));
        let name = |s: &str| Some(name_expr(s));
        let run = |kw: &str, args: Vec<Option<ExprRef>>, prof: &Profile| {
            let Stmt::Instruction(instr) = instr_stmt(kw, kw, args).as_ref().clone() else {
                unreachable!()
            };
            instruction_units(&instr, &env, prof)
        };

        assert_eq!(
            run("Delay", vec![num(0), num(10), num(1)], cr6),
            Some((101, false, 0))
        );
        assert_eq!(
            run("ExciteV", vec![num(1), num(2500), num(500)], cr6),
            Some((6, false, 0))
        );
        assert_eq!(
            run("PortSet", vec![num(1), num(1)], cr6),
            Some((1, false, 0))
        );
        assert_eq!(run("Battery", vec![name("B")], cr6), Some((0, false, 0)));
        assert_eq!(run("Battery", vec![name("B")], cr1000x), Some((8, true, 0)));
        assert_eq!(
            run("PanelTemp", vec![name("T"), num(60)], cr6),
            Some((354, true, 166))
        );
        assert_eq!(
            run("CS616", vec![name("V"), num(4)], cr6),
            Some((22, false, 0))
        );
        assert_eq!(
            run(
                "Therm107",
                vec![
                    name("T"),
                    num(2),
                    num(1),
                    num(1),
                    num(500),
                    num(60),
                    num(1),
                    num(0),
                ],
                cr6,
            ),
            Some((721, true, 166))
        );
        assert_eq!(
            run(
                "BrHalf",
                vec![
                    name("V"),
                    num(2),
                    num(800),
                    num(1),
                    num(1),
                    num(1),
                    num(2500),
                    num(1),
                    num(500),
                    num(60),
                    num(1),
                    num(0),
                ],
                cr6,
            ),
            Some((722, true, 166))
        );
    }
}
