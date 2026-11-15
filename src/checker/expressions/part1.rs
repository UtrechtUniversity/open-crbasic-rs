//! Port of `checker/expressions.py` (lines 1-797).

#![allow(
    dead_code,
    unused_variables,
    unused_imports,
    clippy::used_underscore_items
)]

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::rc::Rc;
use std::sync::LazyLock;

use serde::de::{self, MapAccess, Visitor};
use serde::{Deserialize, Deserializer};

use crate::checker::{context, intervals};
use crate::consteval::{self, Env};
use crate::diagnostics::{DiagRef, Diagnostic, Diagnostics};
use crate::profiles::{DottedName, Profile};
use crate::pystr;
use crate::syntax::{
    Assignment, Call, Case, ConstDecl, ConstValue, Declaration, Expr, ExprRef, Index, Instruction,
    Juxtaposed, Member, Number, NumberValue, OrderedMap, Program, ProgramRef, Stmt, StmtRef,
    SubDef, Unparsed, VarDecl,
};

const TYPE_CODES_JSON: &str = include_str!("../../../data/type_codes.json");

pub(crate) const _RESOLVED_KINDS: &[&str] = &[
    "const_expr",
    "expr",
    "const_expr_only",
    "bool_expr",
    "variable_or_array",
    "expr_or_name",
    "expr_typed",
    "expr_special",
    "expr_pair",
    "expr_list",
    "datatype_kw",
];

pub(crate) const _CONST_FAMILY_KINDS: &[&str] = &["const_expr", "const_expr_only", "datatype_kw"];

pub(crate) const _WHOLE_EXPR_ECHO_LOGGERS: &[&str] = &["cr6", "cr1000x"];

pub(crate) const _EXTRA_PREDEFINED_SYMBOLS: &[&str] = &["NAN", "INF"];

pub(crate) const _CONST_ONLY_VARIABLE_REJECT_KINDS: &[&str] = &["const_expr", "const_expr_only"];

/// Port of checker/expressions.py::_has_dotted_field
#[must_use]
pub fn _has_dotted_field(expr: Option<&Expr>) -> bool {
    let Some(expr) = expr else {
        return false;
    };
    match expr {
        Expr::Member(m) => {
            matches!(m.base.as_deref(), Some(Expr::Name(_))) || _has_dotted_field(m.base.as_deref())
        }
        Expr::Unary(u) => _has_dotted_field(u.operand.as_deref()),
        Expr::Binary(b) => {
            _has_dotted_field(b.left.as_deref()) || _has_dotted_field(b.right.as_deref())
        }
        Expr::Index(idx) => idx.args.iter().any(|a| _has_dotted_field(a.as_deref())),
        _ => false,
    }
}

/// Port of checker/expressions.py::_variable_refs
#[must_use]
pub fn _variable_refs(expr: Option<&Expr>, variable_names: &BTreeSet<String>) -> Vec<String> {
    let Some(expr) = expr else {
        return Vec::new();
    };
    match expr {
        Expr::Name(n) => {
            if variable_names.contains(&pystr::upper(&n.name)) {
                vec![n.name.clone()]
            } else {
                Vec::new()
            }
        }
        Expr::Unary(u) => _variable_refs(u.operand.as_deref(), variable_names),
        Expr::Binary(b) => {
            let mut out = _variable_refs(b.left.as_deref(), variable_names);
            out.extend(_variable_refs(b.right.as_deref(), variable_names));
            out
        }
        Expr::Index(idx) => {
            let mut head = if variable_names.contains(&pystr::upper(&idx.name)) {
                vec![idx.name.clone()]
            } else {
                Vec::new()
            };
            for a in &idx.args {
                head.extend(_variable_refs(a.as_deref(), variable_names));
            }
            head
        }
        Expr::Member(m) => _variable_refs(m.base.as_deref(), variable_names),
        Expr::ArrayLiteral(_) | Expr::Juxtaposed(_) => {
            let mut out = Vec::new();
            if let Some(items) = expr.array_items() {
                for item in items {
                    out.extend(_variable_refs(item.as_deref(), variable_names));
                }
            }
            out
        }
        _ => Vec::new(),
    }
}

pub(crate) const _VARIABLE_REQUIRED_KINDS: &[&str] = &["variable_or_array"];

pub(crate) static _MULTI_REPORT_ORDER: LazyLock<OrderedMap<&'static str, &'static [usize]>> =
    LazyLock::new(|| {
        let mut m = OrderedMap::new();
        m.insert("MOVE", &[1, 0, 3, 2][..]);
        m.insert("RMSSPA", &[1, 2, 0][..]);
        m.insert("STDDEVSPA", &[1, 2, 0][..]);
        m.insert("MAXSPA", &[1, 2, 0][..]);
        m.insert("MINSPA", &[1, 2, 0][..]);
        m.insert("AVGSPA", &[1, 2, 0][..]);
        m.insert("CPIFILESEND", &[0, 1, 2, 3, 4][..]);
        m.insert("GOESSETUP", &[0, 1, 2, 3, 4, 5, 6, 9, 7, 8][..]);
        m.insert("GOESGPS", &[0, 1][..]);
        m.insert("CH400SENDOS", &[0, 1][..]);
        m.insert("MOVEBYTES", &[0, 1, 3, 4, 2][..]);
        m.insert("READIO", &[1, 0][..]);
        m.insert("IPROUTE", &[0, 1, 2][..]);
        m.insert("MQTTPUBLISHMETA", &[0, 1, 2, 3, 4, 5][..]);
        m.insert("ARGOSDATAREPEAT", &[0, 3][..]);
        m.insert("AVGRUN", &[0, 2, 4, 5][..]);
        m.insert("AVW200", &[4, 0][..]);
        m.insert("PRT", &[0, 2][..]);
        m.insert("STRAINCALC", &[0, 2, 3][..]);
        m.insert("CDM_TCSE", &[7, 2][..]);
        m.insert("DEBUG", &[2, 3, 4][..]);
        m.insert("DEWPOINT", &[0, 1, 2][..]);
        m.insert("SENDVARIABLES", &[0, 1, 8][..]);
        m.insert("EC100CONFIGURE", &[0, 3][..]);
        m.insert("GETVARIABLES", &[0, 8][..]);
        m.insert("GPS", &[4, 0][..]);
        m.insert("HYDRAPROBE", &[0, 1][..]);
        m.insert("MATRIX", &[1, 2, 3][..]);
        m.insert("PEAKVALLEY", &[0, 1, 3][..]);
        m.insert("SATVP", &[0, 1][..]);
        m.insert("TIMEDCONTROL", &[4, 5][..]);
        m.insert("SDMCD16MASK", &[0, 1][..]);
        m.insert("SDMGENERIC", &[4, 0][..]);
        m.insert("SDMIO16", &[0, 1][..]);
        m.insert("WETDRYBULB", &[0, 1, 2, 3][..]);
        m.insert("SERIALINRECORD", &[1, 5][..]);
        m.insert("SORTSPA", &[0, 2][..]);
        m.insert("SORTSPAINDEXED", &[0, 2][..]);
        m.insert("SPLITSTR", &[1, 0][..]);
        m.insert("TCPACTIVECONNECTIONS", &[0, 1][..]);
        m.insert("TOTALRUN", &[0, 2, 4][..]);
        m.insert("VAPORPRESSURE", &[0, 1, 2][..]);
        m.insert("MODBUSCLIENT", &[0, 5][..]);
        m.insert("STDDEVRUN", &[0, 2, 4, 5][..]);
        m.insert("TCDIFF", &[5, 0][..]);
        m.insert("PORTBRIDGE", &[1, 0][..]);
        m.insert("SDMAO4A", &[0, 1][..]);
        m.insert("FFTSPA", &[2, 0][..]);
        m
    });

pub(crate) static _UNDEFINED_CHECK_EXEMPT_POSITIONS: LazyLock<
    OrderedMap<&'static str, &'static [usize]>,
> = LazyLock::new(|| {
    let mut m = OrderedMap::new();
    m.insert("ADDPRECISE", &[0, 1][..]);
    m.insert("MOVEPRECISE", &[0, 1][..]);
    m.insert("MODEMCALLBACK", &[0][..]);
    m
});

pub(crate) static _EXEMPT_WHOLE_ECHO: LazyLock<OrderedMap<&'static str, &'static [usize]>> =
    LazyLock::new(|| {
        let mut m = OrderedMap::new();
        m.insert("MONITORCOMMS", &[0, 1, 2][..]);
        m.insert("MODEMCALLBACK", &[0][..]);
        m.insert("IPNETPOWER", &[1, 2][..]);
        m.insert("LOGIP", &[0, 1][..]);
        m.insert("NEWFILE", &[2][..]);
        m.insert("STDDEVRUN", &[8][..]);
        m.insert("TIMEDCONTROL", &[0, 1][..]);
        m
    });

pub(crate) static _ARITY_MIN_SAFE: LazyLock<OrderedMap<&'static str, usize>> =
    LazyLock::new(|| {
        let mut m = OrderedMap::new();
        m.insert("CHECKSUM", 3);
        m.insert("DAYLIGHTSAVING", 8);
        m.insert("DIALMODEM", 4);
        m.insert("EMAILRECV", 6);
        m.insert("EMAILSEND", 6);
        m.insert("ENCRYPTION", 7);
        m.insert("FILECOPY", 2);
        m.insert("FILELIST", 2);
        m.insert("FILEOPEN", 3);
        m.insert("FILEREAD", 3);
        m.insert("FILEREADLINE", 3);
        m.insert("FILERENAME", 2);
        m.insert("FILEWRITE", 3);
        m.insert("FINDSPA", 4);
        m.insert("FORMATFLOAT", 2);
        m.insert("FORMATLONG", 2);
        m.insert("FTPCLIENT", 6);
        m.insert("HTTPGET", 3);
        m.insert("IIF", 3);
        m.insert("INSTR", 4);
        m.insert("IPINFO", 2);
        m.insert("LEFT", 2);
        m.insert("MID", 3);
        m.insert("MQTTPUBLISH", 4);
        m.insert("MQTTSUBSCRIBE", 4);
        m.insert("REPLACE", 3);
        m.insert("RIGHT", 2);
        m.insert("ROUND", 2);
        m.insert("SECSSINCE1990", 2);
        m.insert("SERIALINBLOCK", 3);
        m.insert("SERIALOPEN", 5);
        m.insert("SERIALOUT", 5);
        m.insert("SMSRECV", 3);
        m.insert("STRCOMP", 2);
        m.insert("TCPOPEN", 3);
        m.insert("TIMEISBETWEEN", 4);
        m.insert("UDPDATAGRAM", 6);
        m.insert("UDPOPEN", 3);
        m.insert("WATCHDOGTIMER", 2);
        m.insert("XMLPARSE", 6);
        m.insert("AVE", 2);
        m.insert("PWR", 2);
        m.insert("ATN2", 2);
        m.insert("IFTIME", 3);
        m.insert("TIMEINTOINTERVAL", 3);
        m
    });

pub(crate) static _ARITY_MAX_SAFE: LazyLock<OrderedMap<&'static str, usize>> =
    LazyLock::new(|| {
        let mut m = OrderedMap::new();
        m.insert("ARRAYINDEX", 1);
        m.insert("ASCII", 1);
        m.insert("CHECKPORT", 1);
        m.insert("CHR", 1);
        m.insert("CHR$", 1);
        m.insert("COMPORTISACTIVE", 1);
        m.insert("DAYLIGHTSAVING", 8);
        m.insert("DAYLIGHTSAVINGUS", 1);
        m.insert("ENCRYPTION", 7);
        m.insert("FILECLOSE", 1);
        m.insert("FILECOPY", 2);
        m.insert("FILEENCRYPT", 1);
        m.insert("FILELIST", 2);
        m.insert("FILEOPEN", 3);
        m.insert("FILEREAD", 3);
        m.insert("FILEREADLINE", 3);
        m.insert("FILERENAME", 2);
        m.insert("FILESIZE", 1);
        m.insert("FILETIME", 1);
        m.insert("FILEWRITE", 3);
        m.insert("FINDSPA", 4);
        m.insert("FORMATFLOAT", 2);
        m.insert("FORMATLONG", 2);
        m.insert("FORMATLONGLONG", 1);
        m.insert("HEX", 1);
        m.insert("HEXTODEC", 1);
        m.insert("IIF", 3);
        m.insert("INSTR", 4);
        m.insert("IPINFO", 2);
        m.insert("IPTRACE", 1);
        m.insert("LEFT", 2);
        m.insert("LEN", 1);
        m.insert("LOADFIELDCAL", 1);
        m.insert("LOWERCASE", 1);
        m.insert("LTRIM", 1);
        m.insert("MID", 3);
        m.insert("MQTTUNSUBSCRIBE", 1);
        m.insert("NEWFIELDCAL", 0);
        m.insert("PINGIP", 3);
        m.insert("PPPCLOSE", 0);
        m.insert("REPLACE", 3);
        m.insert("RIGHT", 2);
        m.insert("ROUND", 2);
        m.insert("ROUTE", 1);
        m.insert("RTRIM", 1);
        m.insert("SECSSINCE1990", 2);
        m.insert("SERIALCLOSE", 1);
        m.insert("SERIALFLUSH", 1);
        m.insert("SERIALINBLOCK", 3);
        m.insert("SERIALINCHK", 1);
        m.insert("SERIALOUT", 5);
        m.insert("SMSRECV", 3);
        m.insert("STRCOMP", 2);
        m.insert("TCPCLOSE", 1);
        m.insert("TIMEISBETWEEN", 4);
        m.insert("TRIM", 1);
        m.insert("UPPERCASE", 1);
        m.insert("VOICEKEY", 1);
        m.insert("VOICENUMBER", 1);
        m.insert("WATCHDOGTIMER", 2);
        m.insert("SIN", 1);
        m.insert("COS", 1);
        m.insert("LOG", 1);
        m.insert("LN", 1);
        m.insert("SQR", 1);
        m.insert("EXP", 1);
        m.insert("CSGN", 1);
        m.insert("INT", 1);
        m.insert("FRAC", 1);
        m.insert("SGN", 1);
        m.insert("ATN", 1);
        m.insert("TAN", 1);
        m.insert("FIX", 1);
        m.insert("ASIN", 1);
        m.insert("ACOS", 1);
        m.insert("LOG10", 1);
        m.insert("SINH", 1);
        m.insert("TANH", 1);
        m.insert("COSH", 1);
        m.insert("FLOOR", 1);
        m.insert("CEILING", 1);
        m.insert("RND", 0);
        m.insert("CLOCKCHANGE", 0);
        m.insert("PWR", 2);
        m.insert("ATN2", 2);
        m.insert("IFTIME", 3);
        m.insert("TIMEINTOINTERVAL", 3);
        m
    });

pub(crate) static _FUNCTION_CONST_INTEGER_STRING_ARGS: LazyLock<
    OrderedMap<&'static str, &'static [usize]>,
> = LazyLock::new(|| {
    let mut m = OrderedMap::new();
    m.insert("TIMER", &[1][..]);
    m
});

pub(crate) static _CONST_INTEGER_OVERFLOW_STRING: LazyLock<OrderedMap<&'static str, usize>> =
    LazyLock::new(|| {
        let mut m = OrderedMap::new();
        m.insert("TCPOPEN", 3);
        m
    });

pub(crate) static _CONST_INTEGER_BAD_ARGS: LazyLock<OrderedMap<&'static str, &'static [usize]>> =
    LazyLock::new(|| {
        let mut m = OrderedMap::new();
        m.insert("SERIALOPEN", &[3, 4][..]);
        m
    });

pub(crate) static _NETWORK_INTEGER_ARGS: LazyLock<OrderedMap<&'static str, usize>> =
    LazyLock::new(|| {
        let mut m = OrderedMap::new();
        m.insert("HTTPGET", 3);
        m.insert("TCPOPEN", 2);
        m
    });

pub(crate) static _CONST_INTEGER_TYPE_ARGS: LazyLock<OrderedMap<&'static str, &'static [usize]>> =
    LazyLock::new(|| {
        let mut m = OrderedMap::new();
        m.insert("HTTPPUT", &[8][..]);
        m.insert("FTPCLIENT", &[10][..]);
        m
    });

pub(crate) static _CONST_INTEGER_TYPE_ARGS_ALWAYS: LazyLock<
    OrderedMap<&'static str, &'static [usize]>,
> = LazyLock::new(|| {
    let mut m = OrderedMap::new();
    m.insert("TIMER", &[2][..]);
    m
});

#[derive(Clone)]
pub(crate) struct TimerContext {
    pub env: Env,
    pub declared: BTreeSet<String>,
    pub const_exprs: OrderedMap<String, Option<ExprRef>>,
}

thread_local! {
    pub(crate) static _TIMEOUT_GATE: RefCell<Option<bool>> = const { RefCell::new(None) };
    pub(crate) static _TIMEOUT_WIDE: RefCell<BTreeSet<String>> = const { RefCell::new(BTreeSet::new()) };
    pub(crate) static _TIMER_CONTEXT: RefCell<Option<TimerContext>> = const { RefCell::new(None) };
}

/// Port of checker/expressions.py::set_arity_context
pub fn set_arity_context(
    program: &ProgramRef,
    profile: Option<&'static Profile>,
    declared: Option<BTreeSet<String>>,
) {
    let ctx = context::of(program, profile, None);
    let env = ctx.env().clone();
    let declared_upper = match declared {
        Some(d) => d.into_iter().map(|n| pystr::upper(&n)).collect(),
        None => ctx
            .declared_variable_names()
            .iter()
            .map(|n| pystr::upper(n))
            .collect(),
    };
    let mut const_exprs = OrderedMap::new();
    for st in &program.statements {
        if let Stmt::ConstDecl(cd) = &**st {
            const_exprs.insert(pystr::upper(&cd.name), cd.value.clone());
        }
    }
    _TIMER_CONTEXT.with(|slot| {
        *slot.borrow_mut() = Some(TimerContext {
            env,
            declared: declared_upper,
            const_exprs,
        });
    });
}

/// Result of `checker/expressions.py::_timer_arg`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimerArg {
    Int(i64),
    Float,
}

/// Port of checker/expressions.py::_timer_arg
#[must_use]
pub fn _timer_arg(arg: Option<&Expr>, env: &Env, declared: &BTreeSet<String>) -> Option<TimerArg> {
    let arg = arg?;
    let wide = |value: i64| -> TimerArg {
        if (env.profile.name == "cr300" || env.profile.name == "cr350")
            && value.unsigned_abs() < (1_u64 << 32)
        {
            TimerArg::Int(-(1_i64 << 31))
        } else {
            TimerArg::Float
        }
    };
    match arg {
        Expr::Number(n) => match &n.value {
            NumberValue::Float(_) => Some(TimerArg::Float),
            NumberValue::Int(i) => {
                if pystr::slice(&n.text, None, Some(1)) == "&" {
                    Some(TimerArg::Int(intervals::_signed32(*i)))
                } else if (-(1_i64 << 31)..(1_i64 << 31)).contains(i) {
                    Some(TimerArg::Int(*i))
                } else {
                    Some(wide(*i))
                }
            }
            NumberValue::BigInt(s) => {
                if pystr::slice(&n.text, None, Some(1)) == "&" {
                    let rem = s.bytes().fold(0_i64, |acc, b| {
                        if b.is_ascii_digit() {
                            (acc * 10 + i64::from(b - b'0')) & 0xFFFF_FFFF
                        } else {
                            acc
                        }
                    });
                    Some(TimerArg::Int(intervals::_signed32(rem)))
                } else if let Ok(v) = s.parse::<i128>()
                    && (env.profile.name == "cr300" || env.profile.name == "cr350")
                    && v.unsigned_abs() < (1_u128 << 32)
                {
                    Some(TimerArg::Int(-(1_i64 << 31)))
                } else {
                    Some(TimerArg::Float)
                }
            }
        },
        Expr::Name(n) => {
            let name = pystr::upper(&n.name);
            if name == "INF" || name == "NAN" || declared.contains(&name) {
                return Some(TimerArg::Float);
            }
            let Ok(value) = consteval::value_of(Some(arg), env) else {
                return None;
            };
            match value {
                ConstValue::Str(_) => None,
                ConstValue::Int(i) => {
                    if i > (1_i64 << 31) - 1 {
                        Some(wide(i))
                    } else {
                        Some(TimerArg::Int(i))
                    }
                }
                ConstValue::Float(f) | ConstValue::CFloat(f) => {
                    if !f.is_finite() || !pystr::is_integer(f) {
                        return Some(TimerArg::Float);
                    }
                    let Some(i) = pystr::py_int_from_f64(f) else {
                        return Some(TimerArg::Float);
                    };
                    if i > (1_i64 << 31) - 1 {
                        Some(wide(i))
                    } else {
                        Some(TimerArg::Int(i))
                    }
                }
                ConstValue::BigInt(s) => {
                    if let Ok(v) = s.parse::<i128>()
                        && (env.profile.name == "cr300" || env.profile.name == "cr350")
                        && v.unsigned_abs() < (1_u128 << 32)
                    {
                        Some(TimerArg::Int(-(1_i64 << 31)))
                    } else {
                        Some(TimerArg::Float)
                    }
                }
            }
        }
        Expr::Index(idx) => {
            let upper = pystr::upper(&idx.name);
            if declared.contains(&upper) || env.profile.functions().contains(&upper) {
                Some(TimerArg::Float)
            } else {
                None
            }
        }
        Expr::Unary(u) => {
            let inner = _timer_arg(u.operand.as_deref(), env, declared)?;
            let TimerArg::Int(iv) = inner else {
                return Some(TimerArg::Float);
            };
            let value = match u.op.as_str() {
                "-" => iv.wrapping_neg(),
                "+" => iv,
                _ => !iv,
            };
            Some(TimerArg::Int(intervals::_signed32(value)))
        }
        Expr::Binary(b) => {
            let left = _timer_arg(b.left.as_deref(), env, declared)?;
            let right = _timer_arg(b.right.as_deref(), env, declared)?;
            if left == TimerArg::Float || right == TimerArg::Float || b.op == "/" || b.op == "^" {
                return Some(TimerArg::Float);
            }
            let Ok(value) = consteval::value_of(Some(arg), env) else {
                return None;
            };
            let i = match value {
                ConstValue::Int(i) => i,
                ConstValue::Float(f) | ConstValue::CFloat(f) => pystr::py_int_from_f64(f)?,
                ConstValue::BigInt(s) => {
                    let (neg, digits) = match s.strip_prefix('-') {
                        Some(rest) => (true, rest),
                        None => (false, s.as_str()),
                    };
                    let rem = digits.bytes().fold(0_i64, |acc, b| {
                        if b.is_ascii_digit() {
                            (acc * 10 + i64::from(b - b'0')) & 0xFFFF_FFFF
                        } else {
                            acc
                        }
                    });
                    if neg { -rem } else { rem }
                }
                ConstValue::Str(_) => return None,
            };
            Some(TimerArg::Int(intervals::_signed32(i)))
        }
        _ => None,
    }
}

/// Port of checker/expressions.py::_timeout_wide
#[must_use]
pub fn _timeout_wide(arg: Option<&Expr>, profile: &Profile) -> bool {
    _TIMEOUT_WIDE.with(|wide| {
        intervals::timeout_value_with(arg, None, profile, &wide.borrow())
            == Some(intervals::TimeoutValue::Wide)
    })
}

/// Port of checker/expressions.py::_network_integer_problem
#[must_use]
pub fn _network_integer_problem(
    name: &str,
    arg: Option<&Expr>,
    profile: &Profile,
) -> Option<String> {
    enum NumOrWide {
        Int(i64),
        WideOrFloat,
    }
    let arg = arg?;
    let ctx = _TIMER_CONTEXT.with(|slot| slot.borrow().clone())?;
    if matches!(arg, Expr::String(_)) {
        return Some("network_constint".to_string());
    }
    if let Expr::Name(n) = arg
        && matches!(
            ctx.env.consts.borrow().get(&pystr::upper(&n.name)),
            Some(ConstValue::Str(_))
        )
    {
        return Some("network_constint".to_string());
    }
    let source = if let Expr::Name(n) = arg {
        ctx.const_exprs
            .get(&pystr::upper(&n.name))
            .and_then(|o| o.as_deref())
    } else {
        None
    };
    let mut literal = source;
    while let Some(Expr::Unary(u)) = literal
        && (u.op == "+" || u.op == "-")
    {
        literal = u.operand.as_deref();
    }
    let empty_wide = BTreeSet::new();
    let tv = if let Some(Expr::Number(num)) = literal
        && matches!(num.value, NumberValue::Int(_) | NumberValue::BigInt(_))
    {
        intervals::timeout_value_with(source, None, profile, &empty_wide)
    } else {
        intervals::timeout_value_with(Some(arg), Some(&ctx.env), profile, &empty_wide)
    };
    let val = match tv {
        Some(intervals::TimeoutValue::Wide) => Some(NumOrWide::WideOrFloat),
        Some(intervals::TimeoutValue::Int(i)) => Some(NumOrWide::Int(i)),
        None => match _timer_arg(Some(arg), &ctx.env, &ctx.declared) {
            Some(TimerArg::Float) => Some(NumOrWide::WideOrFloat),
            Some(TimerArg::Int(i)) => Some(NumOrWide::Int(i)),
            None => None,
        },
    };
    let integer = match val {
        Some(NumOrWide::WideOrFloat) => return Some("network_constint".to_string()),
        None if profile.single_pass => return Some("network_constint".to_string()),
        None => return None,
        Some(NumOrWide::Int(i)) => i,
    };
    if name == "HTTPGET" && intervals::_signed32(10_i64.wrapping_mul(integer)) < 0 {
        return Some("Parameter TimeOut cannot be negative".to_string());
    }
    if name == "TCPOPEN" && integer < -1 {
        return Some(format!("Size {integer} cannot be negative"));
    }
    None
}

/// Port of checker/expressions.py::_not_integer_type
#[must_use]
pub fn _not_integer_type(arg: Option<&Expr>) -> bool {
    match arg {
        Some(Expr::Number(n)) => matches!(n.value, NumberValue::Float(_)),
        Some(Expr::String(_)) => true,
        Some(Expr::Name(n)) => pystr::upper(&n.name) == "NAN",
        _ => false,
    }
}

/// Port of checker/expressions.py::_not_constant_integer
#[must_use]
pub fn _not_constant_integer(arg: Option<&Expr>) -> bool {
    match arg {
        Some(Expr::String(_)) => true,
        Some(Expr::Name(n)) => pystr::upper(&n.name) == "NAN",
        Some(Expr::Number(n)) => match n.value {
            NumberValue::Float(_) | NumberValue::BigInt(_) => true,
            NumberValue::Int(i) => !(-(1_i64 << 31)..(1_i64 << 31)).contains(&i),
        },
        _ => false,
    }
}

pub(crate) static _INCOMPLETE_OVERFLOW: LazyLock<OrderedMap<&'static str, usize>> =
    LazyLock::new(|| {
        let mut m = OrderedMap::new();
        m.insert("ABS", 1);
        m.insert("MQTTPUBLISH", 4);
        m
    });

pub(crate) static _ARITY_RANGE: LazyLock<
    OrderedMap<&'static str, OrderedMap<usize, (usize, usize)>>,
> = LazyLock::new(|| {
    let mut m = OrderedMap::new();
    let mut ntp = OrderedMap::new();
    ntp.insert(5, (3, 4));
    ntp.insert(3, (3, 3));
    m.insert("NETWORKTIMEPROTOCOL", ntp);

    let mut gzip = OrderedMap::new();
    gzip.insert(1, (2, 2));
    m.insert("GZIP", gzip);

    let mut ser = OrderedMap::new();
    ser.insert(5, (5, 6));
    ser.insert(6, (5, 8));
    m.insert("SERIALOPEN", ser);

    let mut tmr = OrderedMap::new();
    tmr.insert(3, (3, 3));
    m.insert("TIMER", tmr);
    m
});

pub(crate) static _ARITY_ALLOWED: LazyLock<
    OrderedMap<&'static str, OrderedMap<usize, &'static [usize]>>,
> = LazyLock::new(|| {
    let mut m = OrderedMap::new();

    let mut httpput = OrderedMap::new();
    httpput.insert(5, &[4, 8, 9][..]);
    m.insert("HTTPPUT", httpput);

    let mut ftp = OrderedMap::new();
    ftp.insert(6, &[6, 10, 11][..]);
    m.insert("FTPCLIENT", ftp);

    let mut esend = OrderedMap::new();
    esend.insert(6, &[9, 13, 14][..]);
    m.insert("EMAILSEND", esend);

    let mut erecv = OrderedMap::new();
    erecv.insert(6, &[9, 10, 11, 12, 13][..]);
    m.insert("EMAILRECV", erecv);

    let mut udp = OrderedMap::new();
    udp.insert(6, &[6, 7, 8][..]);
    m.insert("UDPDATAGRAM", udp);

    let mut xml = OrderedMap::new();
    xml.insert(6, &[8][..]);
    m.insert("XMLPARSE", xml);

    let mut hget = OrderedMap::new();
    hget.insert(3, &[3, 4][..]);
    m.insert("HTTPGET", hget);

    let mut uopen = OrderedMap::new();
    uopen.insert(3, &[3, 4][..]);
    m.insert("UDPOPEN", uopen);

    let mut topen = OrderedMap::new();
    topen.insert(3, &[3, 4, 6][..]);
    m.insert("TCPOPEN", topen);

    let mut hpost = OrderedMap::new();
    hpost.insert(5, &[4, 8, 9][..]);
    m.insert("HTTPPOST", hpost);

    let mut csum = OrderedMap::new();
    csum.insert(3, &[3, 4, 6][..]);
    m.insert("CHECKSUM", csum);

    let mut erelay = OrderedMap::new();
    erelay.insert(0, &[4, 5, 9, 10][..]);
    m.insert("EMAILRELAY", erelay);

    m
});

pub(crate) const _ARITY_TRAILING_NEWLINE_LOGGERS: &[&str] = &["cr300", "cr350"];

#[derive(Deserialize)]
struct TypeCodeEntry {
    kind: String,
}

fn deserialize_type_codes<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<OrderedMap<usize, String>, D::Error> {
    struct TypeCodesVisitor;
    impl<'de> Visitor<'de> for TypeCodesVisitor {
        type Value = OrderedMap<usize, String>;
        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("a map of integer type codes to {kind: ...}")
        }
        fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
            let mut out = OrderedMap::new();
            while let Some((k, v)) = map.next_entry::<String, TypeCodeEntry>()? {
                let code = k.parse::<usize>().map_err(de::Error::custom)?;
                out.insert(code, v.kind);
            }
            Ok(out)
        }
    }
    deserializer.deserialize_map(TypeCodesVisitor)
}

static TYPE_CODE_KINDS: LazyLock<OrderedMap<usize, String>> = LazyLock::new(|| {
    let mut de = serde_json::Deserializer::from_str(TYPE_CODES_JSON);
    deserialize_type_codes(&mut de).expect("valid data/type_codes.json")
});

/// Port of checker/expressions.py::_type_code_kinds
#[must_use]
pub fn _type_code_kinds() -> OrderedMap<usize, String> {
    TYPE_CODE_KINDS.clone()
}

/// Port of checker/expressions.py::_position_kinds
#[must_use]
pub fn _position_kinds(
    params: &[i64],
    keyword: &str,
    profile: Option<&Profile>,
) -> OrderedMap<usize, String> {
    let kinds = &*TYPE_CODE_KINDS;
    let kw_upper = pystr::upper(keyword);
    let mut exempt: BTreeSet<usize> = _UNDEFINED_CHECK_EXEMPT_POSITIONS
        .get(kw_upper.as_str())
        .map(|s| s.iter().copied().collect())
        .unwrap_or_default();
    if let Some(prof) = profile
        && _WHOLE_EXPR_ECHO_LOGGERS.contains(&prof.name)
        && let Some(extra) = _EXEMPT_WHOLE_ECHO.get(kw_upper.as_str())
    {
        exempt.extend(extra.iter().copied());
    }
    let mut out = OrderedMap::new();
    for (i, &code) in params.iter().enumerate() {
        if exempt.contains(&i) {
            continue;
        }
        let base = if code >= 100 { code - 100 } else { code };
        let Ok(base_usize) = usize::try_from(base) else {
            continue;
        };
        if let Some(kind) = kinds.get(&base_usize)
            && _RESOLVED_KINDS.contains(&kind.as_str())
        {
            out.insert(i, kind.clone());
        }
    }
    out
}

/// Element returned by `checker/expressions.py::_undefined_names_walk`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UndefinedName {
    Plain(String),
    Dotted(DottedName),
}

impl UndefinedName {
    #[must_use]
    pub fn as_str(&self) -> &str {
        match self {
            Self::Plain(s) => s,
            Self::Dotted(d) => &d.0,
        }
    }

    #[must_use]
    pub fn into_string(self) -> String {
        match self {
            Self::Plain(s) => s,
            Self::Dotted(d) => d.0,
        }
    }
}

/// View of `KnownNames` (or a plain `BTreeSet<String>`) for `_undefined_names`.
pub struct KnownView<'a> {
    pub names: &'a BTreeSet<String>,
    pub tables: &'a BTreeSet<String>,
    pub out_of_scope: &'a BTreeSet<usize>,
    pub dotted_tables: bool,
}

impl<'a> KnownView<'a> {
    #[must_use]
    pub fn from_set(names: &'a BTreeSet<String>) -> Self {
        static EMPTY_STRS: LazyLock<BTreeSet<String>> = LazyLock::new(BTreeSet::new);
        static EMPTY_IDS: LazyLock<BTreeSet<usize>> = LazyLock::new(BTreeSet::new);
        Self {
            names,
            tables: &EMPTY_STRS,
            out_of_scope: &EMPTY_IDS,
            dotted_tables: false,
        }
    }
}

impl<'a> From<&'a super::part2::KnownNames> for KnownView<'a> {
    fn from(known: &'a super::part2::KnownNames) -> Self {
        Self {
            names: &known.names,
            tables: &known.tables,
            out_of_scope: &known.out_of_scope,
            dotted_tables: known.dotted_tables,
        }
    }
}

/// Port of checker/expressions.py::_undefined_names
#[must_use]
pub fn _undefined_names(expr: Option<&Expr>, known: &KnownView<'_>) -> Vec<UndefinedName> {
    let out = _undefined_names_walk(expr, known);
    for (i, n) in out.iter().enumerate() {
        if !matches!(n, UndefinedName::Dotted(_)) {
            let mut res = out[..=i].to_vec();
            res.extend(
                out[i + 1..]
                    .iter()
                    .filter(|m| !matches!(m, UndefinedName::Dotted(_)))
                    .cloned(),
            );
            return res;
        }
    }
    out
}

/// Port of checker/expressions.py::_undefined_names_walk
#[must_use]
pub fn _undefined_names_walk(expr: Option<&Expr>, known: &KnownView<'_>) -> Vec<UndefinedName> {
    let Some(expr) = expr else {
        return Vec::new();
    };
    let expr_addr = std::ptr::from_ref(expr) as usize;
    match expr {
        Expr::Name(n) => {
            if pystr::endswith_any(&n.name, &["."])
                && pystr::len(&n.name) > 1
                && (pystr::isalpha(pystr::slice(&n.name, Some(0), Some(1)))
                    || pystr::slice(&n.name, Some(0), Some(1)) == "_")
            {
                return Vec::new();
            }
            let upper = pystr::upper(&n.name);
            if known.out_of_scope.contains(&expr_addr) {
                return vec![UndefinedName::Plain(n.name.clone())];
            }
            if known.names.contains(&upper) && !known.tables.contains(&upper) {
                Vec::new()
            } else {
                vec![UndefinedName::Plain(n.name.clone())]
            }
        }
        Expr::Unary(u) => _undefined_names_walk(u.operand.as_deref(), known),
        Expr::Binary(b) => {
            let mut out = _undefined_names_walk(b.left.as_deref(), known);
            out.extend(_undefined_names_walk(b.right.as_deref(), known));
            out
        }
        Expr::Index(idx) => {
            let mut head = if known.names.contains(&pystr::upper(&idx.name))
                && !known.out_of_scope.contains(&expr_addr)
            {
                Vec::new()
            } else {
                vec![UndefinedName::Plain(idx.name.clone())]
            };
            for a in &idx.args {
                head.extend(_undefined_names_walk(a.as_deref(), known));
            }
            head
        }
        Expr::Member(m) => {
            let base = m.base.as_deref();
            let mut out = if let Some(base_expr @ Expr::Name(base_name)) = base
                && known.names.contains(&pystr::upper(&base_name.name))
                && !known
                    .out_of_scope
                    .contains(&(std::ptr::from_ref(base_expr) as usize))
            {
                Vec::new()
            } else {
                _undefined_names_walk(base, known)
            };
            if !out.is_empty()
                && let Some(Expr::Name(base_name)) = base
                && known.dotted_tables
            {
                out = vec![UndefinedName::Dotted(DottedName(base_name.name.clone()))];
            }
            if let Some(args) = &m.args {
                for a in args {
                    out.extend(_undefined_names_walk(a.as_deref(), known));
                }
            }
            out
        }
        Expr::ArrayLiteral(_) | Expr::Juxtaposed(_) => {
            let mut out = Vec::new();
            if let Some(items) = expr.array_items() {
                for item in items {
                    out.extend(_undefined_names_walk(item.as_deref(), known));
                }
            }
            out
        }
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profiles;
    use crate::syntax::{Binary, Name, Node, StmtBase, StringLit, Unary};

    fn name(s: &str) -> ExprRef {
        Rc::new(Expr::Name(Name {
            node: Node {
                line: 1,
                ..Default::default()
            },
            name: s.to_string(),
        }))
    }

    fn int_num(v: i64, text: &str) -> ExprRef {
        Rc::new(Expr::Number(Number {
            node: Node {
                line: 1,
                ..Default::default()
            },
            value: NumberValue::Int(v),
            text: text.to_string(),
        }))
    }

    fn float_num(v: f64, text: &str) -> ExprRef {
        Rc::new(Expr::Number(Number {
            node: Node {
                line: 1,
                ..Default::default()
            },
            value: NumberValue::Float(v),
            text: text.to_string(),
        }))
    }

    #[test]
    fn has_dotted_field_matches_python() {
        let m = Expr::Member(Member {
            node: Node {
                line: 1,
                ..Default::default()
            },
            base: Some(name("Status")),
            member: "Battery".to_string(),
            args: None,
        });
        assert!(_has_dotted_field(Some(&m)));
        let u = Expr::Unary(Unary {
            node: Node {
                line: 1,
                ..Default::default()
            },
            op: "-".to_string(),
            operand: Some(Rc::new(m)),
        });
        assert!(_has_dotted_field(Some(&u)));
        assert!(!_has_dotted_field(Some(&*name("X"))));
        assert!(!_has_dotted_field(None));
    }

    #[test]
    fn variable_refs_matches_python() {
        let mut vnames = BTreeSet::new();
        vnames.insert("X".to_string());
        vnames.insert("ARR".to_string());
        let idx = Rc::new(Expr::Index(Index {
            node: Node {
                line: 1,
                ..Default::default()
            },
            name: "arr".to_string(),
            args: vec![Some(name("x"))],
        }));
        let mem = Rc::new(Expr::Member(Member {
            node: Node {
                line: 1,
                ..Default::default()
            },
            base: Some(name("x")),
            member: "f".to_string(),
            args: None,
        }));
        let bin = Expr::Binary(Binary {
            node: Node {
                line: 1,
                ..Default::default()
            },
            op: "+".to_string(),
            left: Some(idx),
            right: Some(mem),
        });
        assert_eq!(_variable_refs(Some(&bin), &vnames), vec!["arr", "x", "x"]);
    }

    #[test]
    fn timer_arg_matches_python() {
        let cr6 = profiles::get("cr6").unwrap();
        let cr300 = profiles::get("cr300").unwrap();
        let env6 = Env::new(cr6);
        env6.define("C_INT", ConstValue::Int(42), None);
        env6.define("C_FLT", ConstValue::Float(1.5), None);
        env6.define("C_BIG", ConstValue::Int(1_i64 << 31), None);
        let env300 = Env::new(cr300);
        env300.define("C_BIG", ConstValue::Int(1_i64 << 31), None);
        let mut decl = BTreeSet::new();
        decl.insert("X".to_string());

        assert_eq!(
            _timer_arg(Some(&*int_num(0xFFFF_FFFF, "&HFFFFFFFF")), &env6, &decl),
            Some(TimerArg::Int(-1))
        );
        assert_eq!(
            _timer_arg(Some(&*int_num(1_i64 << 31, "2147483648")), &env6, &decl),
            Some(TimerArg::Float)
        );
        assert_eq!(
            _timer_arg(Some(&*int_num(1_i64 << 31, "2147483648")), &env300, &decl),
            Some(TimerArg::Int(-(1_i64 << 31)))
        );
        assert_eq!(
            _timer_arg(Some(&*name("c_int")), &env6, &decl),
            Some(TimerArg::Int(42))
        );
        assert_eq!(
            _timer_arg(Some(&*name("c_flt")), &env6, &decl),
            Some(TimerArg::Float)
        );
        assert_eq!(
            _timer_arg(Some(&*name("x")), &env6, &decl),
            Some(TimerArg::Float)
        );
        assert_eq!(
            _timer_arg(Some(&*name("inf")), &env6, &decl),
            Some(TimerArg::Float)
        );
        let div = Expr::Binary(Binary {
            node: Node {
                line: 1,
                ..Default::default()
            },
            op: "/".to_string(),
            left: Some(int_num(4, "4")),
            right: Some(int_num(2, "2")),
        });
        assert_eq!(_timer_arg(Some(&div), &env6, &decl), Some(TimerArg::Float));
        let add = Expr::Binary(Binary {
            node: Node {
                line: 1,
                ..Default::default()
            },
            op: "+".to_string(),
            left: Some(int_num(1, "1")),
            right: Some(name("c_int")),
        });
        assert_eq!(
            _timer_arg(Some(&add), &env6, &decl),
            Some(TimerArg::Int(43))
        );
        let un = Expr::Unary(Unary {
            node: Node {
                line: 1,
                ..Default::default()
            },
            op: "-".to_string(),
            operand: Some(int_num(5, "5")),
        });
        assert_eq!(_timer_arg(Some(&un), &env6, &decl), Some(TimerArg::Int(-5)));
        let idx = Expr::Index(Index {
            node: Node {
                line: 1,
                ..Default::default()
            },
            name: "sin".to_string(),
            args: vec![Some(int_num(1, "1"))],
        });
        assert_eq!(_timer_arg(Some(&idx), &env6, &decl), Some(TimerArg::Float));
    }

    #[test]
    fn not_integer_and_constant_integer_match_python() {
        assert!(_not_integer_type(Some(&*float_num(1.0, "1.0"))));
        assert!(_not_integer_type(Some(&*name("nan"))));
        assert!(!_not_integer_type(Some(&*int_num(1, "1"))));
        assert!(_not_constant_integer(Some(&*int_num(
            1_i64 << 31,
            "2147483648"
        ))));
        assert!(!_not_constant_integer(Some(&*int_num(
            -(1_i64 << 31),
            "-2147483648"
        ))));
    }

    #[test]
    fn type_code_kinds_and_position_kinds_match_python() {
        let cr6 = profiles::get("cr6").unwrap();
        let cr300 = profiles::get("cr300").unwrap();
        let kinds = _type_code_kinds();
        assert_eq!(kinds.get(&1).map(String::as_str), Some("const_expr"));
        assert_eq!(kinds.len(), 59);

        let pk_ap = _position_kinds(&[8, 2, 101], "AddPrecise", Some(cr6));
        assert_eq!(pk_ap.len(), 1);
        assert_eq!(pk_ap.get(&2).map(String::as_str), Some("const_expr"));

        let pk_nf6 = _position_kinds(&[1, 2, 8], "NewFile", Some(cr6));
        assert_eq!(pk_nf6.len(), 2);
        assert_eq!(pk_nf6.get(&0).map(String::as_str), Some("const_expr"));
        assert_eq!(pk_nf6.get(&1).map(String::as_str), Some("expr"));

        let pk_nf300 = _position_kinds(&[1, 2, 8], "NewFile", Some(cr300));
        assert_eq!(pk_nf300.len(), 3);
        assert_eq!(
            pk_nf300.get(&2).map(String::as_str),
            Some("variable_or_array")
        );
    }

    #[test]
    fn undefined_names_matches_python() {
        let mut names = BTreeSet::new();
        names.insert("KNOWN".to_string());
        let mut tables = BTreeSet::new();
        tables.insert("TBL".to_string());
        let oos = BTreeSet::new();
        let view = KnownView {
            names: &names,
            tables: &tables,
            out_of_scope: &oos,
            dotted_tables: true,
        };
        let m1 = Rc::new(Expr::Member(Member {
            node: Node {
                line: 1,
                ..Default::default()
            },
            base: Some(name("Undecl")),
            member: "f".to_string(),
            args: None,
        }));
        let m2 = Rc::new(Expr::Member(Member {
            node: Node {
                line: 1,
                ..Default::default()
            },
            base: Some(name("Undecl2")),
            member: "g".to_string(),
            args: None,
        }));
        let rhs = Rc::new(Expr::Binary(Binary {
            node: Node {
                line: 1,
                ..Default::default()
            },
            op: "+".to_string(),
            left: Some(name("Nope")),
            right: Some(m2),
        }));
        let expr = Expr::Binary(Binary {
            node: Node {
                line: 1,
                ..Default::default()
            },
            op: "+".to_string(),
            left: Some(m1),
            right: Some(rhs),
        });
        assert_eq!(
            _undefined_names(Some(&expr), &view),
            vec![
                UndefinedName::Dotted(DottedName("Undecl".to_string())),
                UndefinedName::Plain("Nope".to_string()),
            ]
        );
        assert_eq!(
            _undefined_names(Some(&*name("T.")), &KnownView::from_set(&names)),
            Vec::<UndefinedName>::new()
        );
    }

    #[test]
    fn set_arity_context_and_network_integer_problem_match_python() {
        let cr6 = profiles::get("cr6").unwrap();
        let mut consts = OrderedMap::new();
        consts.insert("NEG".to_string(), ConstValue::Int(-5));
        consts.insert("STRC".to_string(), ConstValue::Str("abc".to_string()));
        consts.insert("POS".to_string(), ConstValue::Int(10));
        let mut prog = Program::new(
            "test.cr6",
            vec![
                Rc::new(Stmt::ConstDecl(ConstDecl {
                    base: StmtBase::new(1, "test.cr6"),
                    name: "NEG".to_string(),
                    value: Some(int_num(-5, "-5")),
                    ..Default::default()
                })),
                Rc::new(Stmt::ConstDecl(ConstDecl {
                    base: StmtBase::new(2, "test.cr6"),
                    name: "STRC".to_string(),
                    value: Some(Rc::new(Expr::String(StringLit {
                        node: Node {
                            line: 2,
                            ..Default::default()
                        },
                        value: "abc".to_string(),
                    }))),
                    ..Default::default()
                })),
                Rc::new(Stmt::ConstDecl(ConstDecl {
                    base: StmtBase::new(3, "test.cr6"),
                    name: "POS".to_string(),
                    value: Some(int_num(10, "10")),
                    ..Default::default()
                })),
            ],
        );
        prog.consts = consts;
        let prog_ref = Rc::new(prog);
        let mut decl = BTreeSet::new();
        decl.insert("X".to_string());
        set_arity_context(&prog_ref, Some(cr6), Some(decl));

        let str_lit = Expr::String(StringLit {
            node: Node {
                line: 1,
                ..Default::default()
            },
            value: "x".to_string(),
        });
        assert_eq!(
            _network_integer_problem("HTTPGET", Some(&str_lit), cr6).as_deref(),
            Some("network_constint")
        );
        assert_eq!(
            _network_integer_problem("HTTPGET", Some(&*name("strc")), cr6).as_deref(),
            Some("network_constint")
        );
        assert_eq!(
            _network_integer_problem("HTTPGET", Some(&*name("neg")), cr6).as_deref(),
            Some("Parameter TimeOut cannot be negative")
        );
        assert_eq!(
            _network_integer_problem("TCPOPEN", Some(&*name("neg")), cr6).as_deref(),
            Some("Size -5 cannot be negative")
        );
        assert_eq!(
            _network_integer_problem("HTTPGET", Some(&*name("pos")), cr6),
            None
        );
        assert!(!_timeout_wide(Some(&*int_num(10, "10")), cr6));
        assert!(_timeout_wide(
            Some(&*int_num(1_i64 << 32, "4294967296")),
            cr6
        ));
    }
}
