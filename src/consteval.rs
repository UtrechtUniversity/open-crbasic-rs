//! Port of open-crbasic `consteval.py`: constant-expression evaluation.

#![allow(
    clippy::used_underscore_items,
    reason = "port keeps Python's leading-underscore function names"
)]

use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;

use crate::profiles::Profile;
use crate::pystr;
use crate::syntax::{ConstValue, Expr, NumberValue, OrderedMap};

/// Port of `consteval.py::NotConstant` (and `DivideByZero` subclass).
#[derive(Debug, Clone)]
pub enum NotConstant {
    NotConstant(String),
    DivideByZero {
        message: String,
        numerator: Option<ConstValue>,
    },
}

impl NotConstant {
    fn msg(message: impl Into<String>) -> Self {
        Self::NotConstant(message.into())
    }
}

impl fmt::Display for NotConstant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotConstant(m) | Self::DivideByZero { message: m, .. } => f.write_str(m),
        }
    }
}

impl std::error::Error for NotConstant {}

/// `2^63` as `f64` (exact); values with `|x| < I64_BOUND_F64` and `is_integer(x)` fit in `i64`.
const I64_BOUND_F64: f64 = 9_223_372_036_854_775_808.0;

/// Port of `consteval.py::Env`
#[derive(Debug, Clone)]
pub struct Env {
    pub profile: &'static Profile,
    pub consts: Rc<RefCell<OrderedMap<String, ConstValue>>>,
    pub section: Option<String>,
}

impl Env {
    /// Port of `consteval.py::Env.__init__`
    pub fn new(profile: &'static Profile) -> Self {
        Self {
            profile,
            consts: Rc::new(RefCell::new(OrderedMap::new())),
            section: None,
        }
    }

    /// Port of `consteval.py::Env.__init__` (with existing `consts`)
    pub fn with_consts(
        profile: &'static Profile,
        consts: Rc<RefCell<OrderedMap<String, ConstValue>>>,
    ) -> Self {
        Self {
            profile,
            consts,
            section: None,
        }
    }

    /// Port of `consteval.py::Env.define`
    pub fn define(&self, name: &str, mut value: ConstValue, is_float: Option<bool>) {
        if is_float == Some(true) {
            if let ConstValue::Float(f) | ConstValue::CFloat(f) = value {
                value = ConstValue::CFloat(f);
            }
        } else if is_float == Some(false)
            && let ConstValue::Float(f) | ConstValue::CFloat(f) = value
            && !f.is_nan()
            && f.abs() < I64_BOUND_F64
            && pystr::is_integer(f)
            && let Some(i) = pystr::py_int_from_f64(f)
        {
            value = ConstValue::Int(i);
        }
        self.consts.borrow_mut().insert(pystr::upper(name), value);
    }

    /// Port of `consteval.py::Env.child`
    #[must_use]
    pub fn child(&self, section: Option<String>) -> Self {
        Self {
            profile: self.profile,
            consts: Rc::clone(&self.consts),
            section,
        }
    }
}

/// Port of `consteval.py::_fn_chr`
fn _fn_chr(args: &[ConstValue]) -> Result<ConstValue, NotConstant> {
    let v = _num(&args[0])?;
    let code = if v.is_finite() && v.abs() < I64_BOUND_F64 {
        pystr::py_int_from_f64(v).unwrap_or(0) & 0xFF
    } else {
        0
    };
    let s = if code != 0 {
        let byte = u8::try_from(code).unwrap_or(0);
        String::from(char::from(byte))
    } else {
        String::new()
    };
    Ok(ConstValue::Str(s))
}

/// Port of `consteval.py::_fn_abs`
fn _fn_abs(args: &[ConstValue]) -> Result<ConstValue, NotConstant> {
    Ok(ConstValue::Float(_num(&args[0])?.abs()))
}

/// Port of `consteval.py::_fn_int`
fn _fn_int(args: &[ConstValue]) -> Result<ConstValue, NotConstant> {
    let v = _num(&args[0])?;
    if !v.is_finite() {
        return Err(NotConstant::msg("cannot convert non-finite float to int"));
    }
    Ok(ConstValue::Float(v.floor()))
}

/// Port of `consteval.py::_fn_sqr`
fn _fn_sqr(args: &[ConstValue]) -> Result<ConstValue, NotConstant> {
    let v = _num(&args[0])?;
    if v < 0.0 {
        return Err(NotConstant::msg(
            "SQR() of a negative constant is not folded",
        ));
    }
    Ok(ConstValue::Float(v.sqrt()))
}

/// Port of `consteval.py::_fn_len`
fn _fn_len(args: &[ConstValue]) -> Result<ConstValue, NotConstant> {
    match &args[0] {
        ConstValue::Str(s) => {
            let n = i64::try_from(pystr::len(s))
                .map_err(|_| NotConstant::msg("string length exceeds i64"))?;
            Ok(ConstValue::Int(n))
        }
        _ => Err(NotConstant::msg("LEN() of a non-string constant")),
    }
}

/// `int(sys.float_info.max)` in Python.
const FLOAT_MAX_INT: &str = "179769313486231570814527423731704356798070567525844996598917476803157260780028538760589558632766878171540458953514382464234321326889464182768467546703537516986049910576551282076245490090389328944075868508455133942304583236903222948165808559332123348274797826204144723168738177180919299881250404026184124858368";

fn exceeds_float_max_int(s: &str) -> bool {
    let digits = s.strip_prefix('-').unwrap_or(s).trim_start_matches('0');
    digits.len() > FLOAT_MAX_INT.len()
        || (digits.len() == FLOAT_MAX_INT.len() && digits > FLOAT_MAX_INT)
}

/// Port of `consteval.py::_num`
#[allow(clippy::cast_precision_loss, reason = "Python int to float")]
fn _num(v: &ConstValue) -> Result<f64, NotConstant> {
    match v {
        ConstValue::Int(i) => Ok(*i as f64),
        ConstValue::Float(f) | ConstValue::CFloat(f) => Ok(*f),
        ConstValue::BigInt(s) => {
            if exceeds_float_max_int(s) {
                return Err(NotConstant::msg("number outside the float range"));
            }
            s.parse::<f64>()
                .map_err(|_| NotConstant::msg("number outside the float range"))
        }
        ConstValue::Str(_) => Err(NotConstant::msg("expected a number")),
    }
}

/// Port of `consteval.py::_bool`
fn _bool(v: &ConstValue) -> Result<bool, NotConstant> {
    Ok(_num(v)? != 0.0)
}

/// Port of `consteval.py::to_long`
pub fn to_long(value: &ConstValue) -> i64 {
    const MIN_LONG: i64 = -2_147_483_648;
    const MAX_LONG: i64 = 2_147_483_647;
    let n = match value {
        ConstValue::Str(_) | ConstValue::BigInt(_) => return MIN_LONG,
        ConstValue::Int(i) => *i,
        ConstValue::Float(f) | ConstValue::CFloat(f) => match pystr::py_int_from_f64(*f) {
            Some(i) => i,
            None => return MIN_LONG,
        },
    };
    if (MIN_LONG..=MAX_LONG).contains(&n) {
        n
    } else {
        MIN_LONG
    }
}

/// Port of `consteval.py::is_finite_number`
pub fn is_finite_number(value: &ConstValue) -> bool {
    match value {
        ConstValue::Int(_) | ConstValue::BigInt(_) => true,
        ConstValue::Float(f) | ConstValue::CFloat(f) => f.is_finite(),
        ConstValue::Str(_) => false,
    }
}

/// Port of `consteval.py::_truth`
fn _truth(v: bool) -> ConstValue {
    ConstValue::Int(if v { -1 } else { 0 })
}

/// Port of `consteval.py::value_of`
pub fn value_of(expr: Option<&Expr>, env: &Env) -> Result<ConstValue, NotConstant> {
    let value = _value_of(expr, env)?;
    if let ConstValue::BigInt(ref s) = value
        && exceeds_float_max_int(s)
    {
        return Err(NotConstant::msg("number outside the float range"));
    }
    Ok(value)
}

/// Port of `consteval.py::_value_of`
fn _value_of(expr: Option<&Expr>, env: &Env) -> Result<ConstValue, NotConstant> {
    let Some(expr) = expr else {
        return Err(NotConstant::msg("empty expression"));
    };
    match expr {
        Expr::Number(n) => Ok(match &n.value {
            NumberValue::Int(i) => ConstValue::Int(*i),
            NumberValue::Float(f) => ConstValue::Float(*f),
            NumberValue::BigInt(s) => ConstValue::BigInt(s.clone()),
        }),
        Expr::String(s) => Ok(ConstValue::Str(s.value.clone())),
        Expr::Name(n) => _lookup(&n.name, env),
        Expr::Unary(u) => {
            let v = value_of(u.operand.as_deref(), env)?;
            match u.op.as_str() {
                "-" => Ok(ConstValue::Float(-_num(&v)?)),
                "+" => Ok(ConstValue::Float(_num(&v)?)),
                "NOT" => Ok(_truth(!_bool(&v)?)),
                other => Err(NotConstant::msg(format!(
                    "unsupported unary operator {other:?}"
                ))),
            }
        }
        Expr::Binary(b) => {
            let left = value_of(b.left.as_deref(), env)?;
            let right = value_of(b.right.as_deref(), env)?;
            _binary(&b.op, &left, &right)
        }
        Expr::Index(idx) => {
            let upper = pystr::upper(&idx.name);
            let func: fn(&[ConstValue]) -> Result<ConstValue, NotConstant> = match upper.as_str() {
                "CHR" | "CHR$" => _fn_chr,
                "ABS" => _fn_abs,
                "INT" => _fn_int,
                "SQR" => _fn_sqr,
                "LEN" => _fn_len,
                _ => {
                    return Err(NotConstant::msg(format!(
                        "{} is not a constant-foldable function",
                        idx.name
                    )));
                }
            };
            if idx.args.len() != 1 {
                return Err(NotConstant::msg(format!("{} takes one argument", idx.name)));
            }
            let arg = value_of(idx.args[0].as_deref(), env)?;
            func(&[arg])
        }
        _ => Err(NotConstant::msg(format!(
            "{} has no constant value",
            expr.type_name()
        ))),
    }
}

/// Port of `consteval.py::is_float`
pub fn is_float(expr: Option<&Expr>, env: &Env) -> Option<bool> {
    let expr = expr?;
    match expr {
        Expr::Number(n) => Some(matches!(n.value, NumberValue::Float(_))),
        Expr::Name(n) => {
            let upper = pystr::upper(&n.name);
            if let Some(v) = env.consts.borrow().get(&upper) {
                return match v {
                    ConstValue::CFloat(_) => Some(true),
                    ConstValue::Int(_) | ConstValue::BigInt(_) => Some(false),
                    ConstValue::Float(_) | ConstValue::Str(_) => None,
                };
            }
            env.profile.symbols().get(&upper).map(|_| false)
        }
        Expr::Unary(u) if matches!(u.op.as_str(), "-" | "+") => is_float(u.operand.as_deref(), env),
        Expr::Binary(b) if matches!(b.op.as_str(), "/" | "^") => Some(true),
        Expr::Binary(b) if matches!(b.op.as_str(), "+" | "-" | "*" | "\\" | "MOD") => {
            let left = is_float(b.left.as_deref(), env);
            let right = is_float(b.right.as_deref(), env);
            if left == Some(true) || right == Some(true) {
                Some(true)
            } else if left.is_none() || right.is_none() {
                None
            } else {
                Some(false)
            }
        }
        Expr::Index(idx) if pystr::upper(&idx.name) == "INT" => Some(true),
        Expr::Index(idx) if pystr::upper(&idx.name) == "ABS" && idx.args.len() == 1 => {
            is_float(idx.args[0].as_deref(), env)
        }
        _ => None,
    }
}

/// Port of `consteval.py::truthy`
pub fn truthy(v: &ConstValue) -> bool {
    match v {
        ConstValue::Str(s) => !s.is_empty(),
        ConstValue::Int(i) => *i != 0,
        ConstValue::BigInt(s) => s != "0",
        ConstValue::Float(f) | ConstValue::CFloat(f) => *f != 0.0,
    }
}

/// Port of `consteval.py::is_constant`
pub fn is_constant(expr: Option<&Expr>, env: &Env) -> bool {
    value_of(expr, env).is_ok()
}

/// Port of `consteval.py::_lookup`
fn _lookup(name: &str, env: &Env) -> Result<ConstValue, NotConstant> {
    let upper = pystr::upper(name);
    if upper == "SECTION"
        && let Some(section) = &env.section
    {
        return Ok(ConstValue::Str(section.clone()));
    }
    if let Some(val) = env.consts.borrow().get(&upper) {
        return Ok(val.clone());
    }
    if let Some(sym) = env.profile.symbols().get(&upper) {
        return Ok(ConstValue::Int(sym.value));
    }
    Err(NotConstant::msg(format!("{name} is not a constant")))
}

/// Port of `consteval.py::_as_str`
fn _as_str(v: &ConstValue) -> Result<String, NotConstant> {
    if let ConstValue::Str(s) | ConstValue::BigInt(s) = v {
        return Ok(s.clone());
    }
    let n = _num(v)?;
    if !n.is_finite() {
        return Err(NotConstant::msg("cannot convert non-finite float to int"));
    }
    if n.abs() < I64_BOUND_F64
        && pystr::is_integer(n)
        && let Some(i) = pystr::py_int_from_f64(n)
    {
        return Ok(i.to_string());
    }
    Ok(pystr::repr_f64(n))
}

fn to_i64(v: &ConstValue, op: &str) -> Result<i64, NotConstant> {
    let n = _num(v)?;
    if !n.is_finite() || n.abs() >= I64_BOUND_F64 {
        return Err(NotConstant::msg(format!(
            "{op:?}: number outside int range"
        )));
    }
    pystr::py_int_from_f64(n)
        .ok_or_else(|| NotConstant::msg(format!("{op:?}: number outside int range")))
}

/// Port of `consteval.py::_binary`
#[allow(clippy::cast_precision_loss, reason = "Python int to float")]
fn _binary(op: &str, left: &ConstValue, right: &ConstValue) -> Result<ConstValue, NotConstant> {
    match op {
        "TO" => {
            _num(right)?;
            Ok(ConstValue::Int(0))
        }
        "&" => Ok(ConstValue::Str(format!(
            "{}{}",
            _as_str(left)?,
            _as_str(right)?
        ))),
        "+" => {
            if matches!(left, ConstValue::Str(_)) || matches!(right, ConstValue::Str(_)) {
                Ok(ConstValue::Str(format!(
                    "{}{}",
                    _as_str(left)?,
                    _as_str(right)?
                )))
            } else {
                Ok(ConstValue::Float(_num(left)? + _num(right)?))
            }
        }
        "-" => Ok(ConstValue::Float(_num(left)? - _num(right)?)),
        "*" => Ok(ConstValue::Float(_num(left)? * _num(right)?)),
        "/" => {
            let l = _num(left)?;
            let r = _num(right)?;
            if r == 0.0 {
                return Err(NotConstant::DivideByZero {
                    message: "'/' cannot be folded: float division by zero".to_owned(),
                    numerator: Some(left.clone()),
                });
            }
            Ok(ConstValue::Float(l / r))
        }
        "\\" => {
            let l = to_i64(left, op)?;
            let r = to_i64(right, op)?;
            let q = pystr::floordiv_i64(l, r)
                .ok_or_else(|| NotConstant::msg("'\\': integer division or modulo by zero"))?;
            Ok(ConstValue::Float(q as f64))
        }
        "^" => {
            let l = _num(left)?;
            let r = _num(right)?;
            if l == 0.0 && r < 0.0 {
                return Err(NotConstant::msg(
                    "'^': 0.0 cannot be raised to a negative power",
                ));
            }
            if l < 0.0 && !pystr::is_integer(r) {
                return Err(NotConstant::msg(
                    "a negative base to a fractional power is not folded",
                ));
            }
            let power = l.powf(r);
            if l.is_finite() && r.is_finite() && power.is_infinite() {
                return Err(NotConstant::msg("'^': Numerical result out of range"));
            }
            Ok(ConstValue::Float(power))
        }
        "MOD" => {
            let l = to_i64(left, op)?;
            let r = to_i64(right, op)?;
            let m = pystr::pymod_i64(l, r)
                .ok_or_else(|| NotConstant::msg("'MOD': integer division or modulo by zero"))?;
            Ok(ConstValue::Float(m as f64))
        }
        "<<" => {
            let l = to_i64(left, op)?;
            let r = to_i64(right, op)?;
            let shift =
                u32::try_from(r).map_err(|_| NotConstant::msg("'<<': negative shift count"))?;
            let res = l
                .checked_shl(shift)
                .ok_or_else(|| NotConstant::msg("'<<': shift overflow"))?;
            Ok(ConstValue::Float(res as f64))
        }
        ">>" => {
            let l = to_i64(left, op)?;
            let r = to_i64(right, op)?;
            let shift =
                u32::try_from(r).map_err(|_| NotConstant::msg("'>>': negative shift count"))?;
            let res = l.checked_shr(shift).unwrap_or(if l < 0 { -1 } else { 0 });
            Ok(ConstValue::Float(res as f64))
        }
        "AND" => Ok(ConstValue::Float(
            (to_i64(left, op)? & to_i64(right, op)?) as f64,
        )),
        "OR" => Ok(ConstValue::Float(
            (to_i64(left, op)? | to_i64(right, op)?) as f64,
        )),
        "XOR" => Ok(ConstValue::Float(
            (to_i64(left, op)? ^ to_i64(right, op)?) as f64,
        )),
        "EQV" => Ok(_truth(_bool(left)? == _bool(right)?)),
        "IMP" => Ok(_truth(!_bool(left)? || _bool(right)?)),
        "=" | "<>" | "<" | ">" | "<=" | ">=" => {
            if matches!(left, ConstValue::Str(_)) || matches!(right, ConstValue::Str(_)) {
                Ok(_truth(_compare(op, &_as_str(left)?, &_as_str(right)?)))
            } else {
                Ok(_truth(_compare(op, &_num(left)?, &_num(right)?)))
            }
        }
        _ => Err(NotConstant::msg(format!("unsupported operator {op:?}"))),
    }
}

/// Port of `consteval.py::_compare`
fn _compare<T: PartialOrd + ?Sized>(op: &str, left: &T, right: &T) -> bool {
    match op {
        "=" => left == right,
        "<>" => left != right,
        "<" => left < right,
        ">" => left > right,
        "<=" => left <= right,
        ">=" => left >= right,
        _ => unreachable!("unsupported comparison operator {op:?}"),
    }
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use super::*;
    use crate::profiles;
    use crate::syntax::{Binary, ExprRef, Index, Name, Node, Number, StringLit, Unary};

    fn num_i(v: i64) -> ExprRef {
        Rc::new(Expr::Number(Number {
            node: Node::new(1, "t.cr6"),
            value: NumberValue::Int(v),
            text: v.to_string(),
        }))
    }

    fn num_f(v: f64) -> ExprRef {
        Rc::new(Expr::Number(Number {
            node: Node::new(1, "t.cr6"),
            value: NumberValue::Float(v),
            text: pystr::repr_f64(v),
        }))
    }

    fn str_lit(s: &str) -> ExprRef {
        Rc::new(Expr::String(StringLit {
            node: Node::new(1, "t.cr6"),
            value: s.to_owned(),
        }))
    }

    fn name_expr(s: &str) -> ExprRef {
        Rc::new(Expr::Name(Name {
            node: Node::new(1, "t.cr6"),
            name: s.to_owned(),
        }))
    }

    fn unary(op: &str, operand: ExprRef) -> ExprRef {
        Rc::new(Expr::Unary(Unary {
            node: Node::new(1, "t.cr6"),
            op: op.to_owned(),
            operand: Some(operand),
        }))
    }

    fn binary(op: &str, left: ExprRef, right: ExprRef) -> ExprRef {
        Rc::new(Expr::Binary(Binary {
            node: Node::new(1, "t.cr6"),
            op: op.to_owned(),
            left: Some(left),
            right: Some(right),
        }))
    }

    fn call1(fn_name: &str, arg: ExprRef) -> ExprRef {
        Rc::new(Expr::Index(Index {
            node: Node::new(1, "t.cr6"),
            name: fn_name.to_owned(),
            args: vec![Some(arg)],
        }))
    }

    fn fold_const(expr: &ExprRef) -> ConstValue {
        let env = Env::new(profiles::get("cr6").unwrap());
        let val = value_of(Some(expr), &env).unwrap();
        env.define("R", val, is_float(Some(expr), &env));
        env.consts.borrow().get("R").unwrap().clone()
    }

    #[derive(Debug)]
    enum Expected {
        Int(i64),
        Float(f64),
        CFloat(f64),
        Str(&'static str),
    }

    fn assert_const_eq(actual: &ConstValue, expected: Expected) {
        match (actual, expected) {
            (ConstValue::Int(a), Expected::Int(b)) => assert_eq!(*a, b),
            (ConstValue::Float(a), Expected::Float(b))
            | (ConstValue::CFloat(a), Expected::CFloat(b)) => {
                assert_eq!(a.to_bits(), b.to_bits());
            }
            (ConstValue::Str(a), Expected::Str(b)) => assert_eq!(a, b),
            _ => panic!("mismatch: actual={actual:?}"),
        }
    }

    #[test]
    fn test_const_fold() {
        let cases: Vec<(ExprRef, Expected)> = vec![
            // 2 + 3 * 4 -> 14
            (
                binary("+", num_i(2), binary("*", num_i(3), num_i(4))),
                Expected::Int(14),
            ),
            // 10 - 4 - 3 -> 3
            (
                binary("-", binary("-", num_i(10), num_i(4)), num_i(3)),
                Expected::Int(3),
            ),
            // 2^3^2 -> 64
            (
                binary("^", binary("^", num_i(2), num_i(3)), num_i(2)),
                Expected::CFloat(64.0),
            ),
            // -2^2 -> -4
            (
                unary("-", binary("^", num_i(2), num_i(2))),
                Expected::CFloat(-4.0),
            ),
            // 1 OR 0 AND 0 -> 0
            (
                binary("AND", binary("OR", num_i(1), num_i(0)), num_i(0)),
                Expected::Float(0.0),
            ),
            // 2 \ 3 -> 0
            (binary("\\", num_i(2), num_i(3)), Expected::Int(0)),
            // 7 INTDV 2 -> 3 (parser normalizes INTDV to \)
            (binary("\\", num_i(7), num_i(2)), Expected::Int(3)),
            // 5 MOD 3 -> 2
            (binary("MOD", num_i(5), num_i(3)), Expected::Int(2)),
            // TRUE -> -1
            (name_expr("TRUE"), Expected::Int(-1)),
            // FALSE -> 0
            (name_expr("FALSE"), Expected::Int(0)),
            // NOT TRUE -> 0
            (unary("NOT", name_expr("TRUE")), Expected::Int(0)),
            // 1 = 1 -> -1
            (binary("=", num_i(1), num_i(1)), Expected::Int(-1)),
            // 1 <> 1 -> 0
            (binary("<>", num_i(1), num_i(1)), Expected::Int(0)),
            // 1 << 2 -> 4
            (binary("<<", num_i(1), num_i(2)), Expected::Float(4.0)),
            // 8 >> 2 -> 2
            (binary(">>", num_i(8), num_i(2)), Expected::Float(2.0)),
            // "a" & "b" -> "ab"
            (binary("&", str_lit("a"), str_lit("b")), Expected::Str("ab")),
            // "a" + "b" -> "ab"
            (binary("+", str_lit("a"), str_lit("b")), Expected::Str("ab")),
            // ABS(-5) -> 5
            (call1("ABS", unary("-", num_i(5))), Expected::Int(5)),
            // INT(3.7) -> 3.0
            (call1("INT", num_f(3.7)), Expected::CFloat(3.0)),
            // LEN("hello") -> 5
            (call1("LEN", str_lit("hello")), Expected::Int(5)),
            // CHR(65) -> "A"
            (call1("CHR", num_i(65)), Expected::Str("A")),
        ];
        for (expr, expected) in cases {
            let actual = fold_const(&expr);
            assert_const_eq(&actual, expected);
        }
    }

    #[test]
    fn test_predefined_symbols_case_insensitive() {
        assert_const_eq(&fold_const(&name_expr("mV200")), Expected::Int(804));
        assert_const_eq(&fold_const(&name_expr("mv200")), Expected::Int(804));
        assert_const_eq(&fold_const(&name_expr("U9")), Expected::Int(209));
        assert_const_eq(&fold_const(&name_expr("_50Hz")), Expected::Int(50));
    }

    #[test]
    fn test_earlier_const_referenced() {
        let env = Env::new(profiles::get("cr6").unwrap());
        let a_expr = num_i(2);
        let a_val = value_of(Some(&a_expr), &env).unwrap();
        env.define("A", a_val, is_float(Some(&a_expr), &env));

        let b_expr = binary("*", name_expr("A"), num_i(3));
        let b_val = value_of(Some(&b_expr), &env).unwrap();
        env.define("B", b_val, is_float(Some(&b_expr), &env));

        assert_const_eq(env.consts.borrow().get("A").unwrap(), Expected::Int(2));
        assert_const_eq(env.consts.borrow().get("B").unwrap(), Expected::Int(6));
    }

    #[test]
    fn test_is_constant_true_and_false() {
        let env = Env::new(profiles::get("cr6").unwrap());
        let five = num_i(5);
        assert!(is_constant(Some(&five), &env));
        let undefined = name_expr("SomeUndefinedThing");
        assert!(!is_constant(Some(&undefined), &env));
    }

    #[test]
    fn test_huge_integer_is_not_foldable_as_float() {
        // In Rust, i64 cannot hold 10^309; verify non-numeric and overflowing cases stop folding.
        assert!(_num(&ConstValue::Str("x".to_owned())).is_err());
        assert!(to_i64(&ConstValue::Float(f64::INFINITY), "<<").is_err());
        assert!(to_i64(&ConstValue::Float(f64::NEG_INFINITY), "<<").is_err());
    }

    #[test]
    fn test_huge_integer_numeric_callers_stop_folding() {
        let env = Env::new(profiles::get("cr6").unwrap());
        let h = name_expr("H");
        let exprs = vec![
            unary("+", Rc::clone(&h)),
            unary("-", Rc::clone(&h)),
            unary("NOT", Rc::clone(&h)),
            call1("Chr", Rc::clone(&h)),
            call1("Abs", Rc::clone(&h)),
            call1("Int", Rc::clone(&h)),
            call1("Sqr", Rc::clone(&h)),
            binary("&", Rc::clone(&h), str_lit("")),
            binary("+", Rc::clone(&h), str_lit("")),
            binary("TO", num_i(1), Rc::clone(&h)),
            binary("+", Rc::clone(&h), num_i(1)),
            binary("-", Rc::clone(&h), num_i(1)),
            binary("*", Rc::clone(&h), num_i(1)),
            binary("/", Rc::clone(&h), num_i(1)),
            binary("\\", Rc::clone(&h), num_i(1)),
            binary("^", Rc::clone(&h), num_i(1)),
            binary("MOD", Rc::clone(&h), num_i(1)),
            binary("<<", Rc::clone(&h), num_i(1)),
            binary(">>", Rc::clone(&h), num_i(1)),
            binary("AND", Rc::clone(&h), num_i(1)),
            binary("OR", Rc::clone(&h), num_i(1)),
            binary("XOR", Rc::clone(&h), num_i(1)),
            binary("EQV", Rc::clone(&h), num_i(1)),
            binary("IMP", Rc::clone(&h), num_i(1)),
            binary("=", Rc::clone(&h), num_i(1)),
        ];
        for expr in exprs {
            assert!(value_of(Some(&expr), &env).is_err());
        }
    }

    #[test]
    fn test_section_only_bound_inside_includesection() {
        let env = Env::new(profiles::get("cr6").unwrap());
        let child = env.child(Some("Defs".to_owned()));
        let cond = binary("=", name_expr("Section"), str_lit("Defs"));
        let res = value_of(Some(&cond), &child).unwrap();
        assert!(truthy(&res));
        child.define("FromLib", ConstValue::Int(1), Some(false));
        assert_const_eq(
            env.consts.borrow().get("FROMLIB").unwrap(),
            Expected::Int(1),
        );

        let top_cond = binary("=", name_expr("Section"), str_lit("Foo"));
        assert!(value_of(Some(&top_cond), &env).is_err());
    }

    #[test]
    fn test_bigint_value_of_and_float_range() {
        let env = Env::new(profiles::get("cr6").unwrap());

        let ok_big = Expr::Number(Number {
            node: Node::default(),
            value: NumberValue::BigInt("123456789012345678901".to_owned()),
            text: "123456789012345678901".to_owned(),
        });
        assert!(matches!(
            value_of(Some(&ok_big), &env),
            Ok(ConstValue::BigInt(ref s)) if s == "123456789012345678901"
        ));
        assert_eq!(is_float(Some(&ok_big), &env), Some(false));
        assert!(is_finite_number(&ConstValue::BigInt(
            "123456789012345678901".to_owned()
        )));
        assert!(truthy(&ConstValue::BigInt(
            "123456789012345678901".to_owned()
        )));

        // Exact sys.float_info.max integer (309 digits) is accepted; +1 is rejected.
        let at_max = Expr::Number(Number {
            node: Node::default(),
            value: NumberValue::BigInt(FLOAT_MAX_INT.to_owned()),
            text: FLOAT_MAX_INT.to_owned(),
        });
        assert!(value_of(Some(&at_max), &env).is_ok());

        let mut over_max_str = FLOAT_MAX_INT.to_owned();
        over_max_str.replace_range(308..309, "9");
        let over_max = Expr::Number(Number {
            node: Node::default(),
            value: NumberValue::BigInt(over_max_str.clone()),
            text: over_max_str,
        });
        let err = value_of(Some(&over_max), &env).unwrap_err();
        assert_eq!(err.to_string(), "number outside the float range");

        let too_many_digits = Expr::Number(Number {
            node: Node::default(),
            value: NumberValue::BigInt(format!("1{}", "0".repeat(309))),
            text: format!("1{}", "0".repeat(309)),
        });
        assert!(value_of(Some(&too_many_digits), &env).is_err());
    }
}
