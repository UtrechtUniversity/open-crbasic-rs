//! Port of `checker/longconv.py`.

#![allow(
    clippy::used_underscore_items,
    reason = "port keeps Python's leading-underscore function names"
)]

use std::collections::BTreeSet;

use crate::consteval::{self, Env};
use crate::profiles::Profile;
use crate::pystr;
use crate::syntax::{ConstValue, Expr, NumberValue, Stmt, StmtRef};

pub const INT_MIN: i64 = -(1_i64 << 31);
const _OPS: &[&str] = &["+", "-", "*", "/", "^", "\\", "MOD"];

/// Port of `checker/longconv.py::float_names`.
pub fn float_names() -> BTreeSet<String> {
    todo!("checker/longconv.py::float_names (omitted: statements: untyped)")
}

/// Port of `checker/longconv.py::float_names`.
pub fn float_names_in(statements: &[StmtRef]) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    for st in statements {
        if let Stmt::ConstDecl(cd) = st.as_ref()
            && _float_typed_with(cd.value.as_deref(), &names)
        {
            names.insert(pystr::upper(&cd.name));
        }
    }
    names
}

/// Port of `checker/longconv.py::literal_names`.
pub fn literal_names() -> BTreeSet<String> {
    todo!("checker/longconv.py::literal_names (omitted: statements: untyped)")
}

/// Port of `checker/longconv.py::literal_names`.
pub fn literal_names_in(statements: &[StmtRef]) -> BTreeSet<String> {
    statements
        .iter()
        .filter_map(|st| match st.as_ref() {
            Stmt::ConstDecl(cd) if _integer_literal(cd.value.as_deref()) => {
                Some(pystr::upper(&cd.name))
            }
            _ => None,
        })
        .collect()
}

/// Port of `checker/longconv.py::_float_typed`.
pub fn _float_typed(_expr: Option<&Expr>) -> bool {
    todo!("checker/longconv.py::_float_typed (omitted: names: untyped)")
}

/// Port of `checker/longconv.py::_float_typed`.
pub fn _float_typed_with(expr: Option<&Expr>, names: &BTreeSet<String>) -> bool {
    let Some(expr) = expr else {
        return false;
    };
    match expr {
        Expr::Number(n) => {
            let text = pystr::upper(&n.text);
            if text.starts_with('&') {
                return false;
            }
            if text.contains('.') || text.contains('E') {
                return true;
            }
            match &n.value {
                NumberValue::Int(i) => !(INT_MIN..=i64::from(i32::MAX)).contains(i),
                NumberValue::Float(f) => !(f64::from(i32::MIN) <= *f && *f < 2_147_483_648.0_f64),
                NumberValue::BigInt(_) => true,
            }
        }
        Expr::Name(n) => names.contains(&pystr::upper(&n.name)),
        Expr::Unary(u) => _float_typed_with(u.operand.as_deref(), names),
        Expr::Binary(b) => {
            b.op == "/"
                || b.op == "^"
                || _float_typed_with(b.left.as_deref(), names)
                || _float_typed_with(b.right.as_deref(), names)
        }
        _ => false,
    }
}

/// Port of `checker/longconv.py::_plain`.
pub fn _plain(expr: Option<&Expr>) -> bool {
    let Some(expr) = expr else {
        return false;
    };
    match expr {
        Expr::Number(_) | Expr::Name(_) => true,
        Expr::Unary(u) => matches!(u.op.as_str(), "-" | "+") && _plain(u.operand.as_deref()),
        Expr::Binary(b) => {
            _OPS.contains(&b.op.as_str()) && _plain(b.left.as_deref()) && _plain(b.right.as_deref())
        }
        _ => false,
    }
}

/// Port of `checker/longconv.py::long_value`.
pub fn long_value(_arg: Option<&Expr>) -> Option<i64> {
    todo!(
        "checker/longconv.py::long_value (omitted: env: consteval.Env, profile: untyped, names: untyped, literals: untyped)"
    )
}

/// Port of `checker/longconv.py::long_value`.
pub fn long_value_with(
    arg: Option<&Expr>,
    env: &Env,
    profile: &Profile,
    names: &BTreeSet<String>,
    literals: &BTreeSet<String>,
) -> Option<i64> {
    let arg = arg?;
    if matches!(arg, Expr::String(_)) {
        return Some(INT_MIN);
    }
    if let Expr::Name(n) = arg {
        let u = pystr::upper(&n.name);
        if (u == "NAN" || u == "INF") && env.consts.borrow().get(&u).is_none() {
            return Some(INT_MIN);
        }
    }
    if !_plain(Some(arg)) {
        return None;
    }
    let value = consteval::value_of(Some(arg), env).ok()?;
    match &value {
        ConstValue::Str(_) => return Some(INT_MIN),
        ConstValue::Float(f) | ConstValue::CFloat(f) if !f.is_finite() => return Some(INT_MIN),
        _ => {}
    }
    if let Expr::Number(n) = arg
        && n.text.starts_with('&')
    {
        let i = match value {
            ConstValue::Int(i) => {
                if i > 0xFFFF_FFFF {
                    return None;
                }
                i
            }
            ConstValue::Float(f) | ConstValue::CFloat(f) => {
                if f > f64::from(u32::MAX) {
                    return None;
                }
                pystr::py_int_from_f64(f)?
            }
            ConstValue::BigInt(_) | ConstValue::Str(_) => return None,
        };
        return Some(if i >= (1_i64 << 31) {
            i - (1_i64 << 32)
        } else {
            i
        });
    }
    if _hex_inside(Some(arg)) {
        return None;
    }
    let is_whole = match &value {
        ConstValue::Int(_) | ConstValue::BigInt(_) => true,
        ConstValue::Float(f) | ConstValue::CFloat(f) => f.fract() == 0.0,
        ConstValue::Str(_) => false,
    };
    if !_float_typed_with(Some(arg), names) && is_whole {
        return match value {
            ConstValue::Int(i) => Some(_wrap(i)),
            ConstValue::Float(f) | ConstValue::CFloat(f) => pystr::py_int_from_f64(f).map(_wrap),
            ConstValue::BigInt(s) => {
                let mut r: i64 = 0;
                for b in s.bytes() {
                    r = (r * 10 + i64::from(b - b'0')) & 0xFFFF_FFFF;
                }
                Some(_wrap(r))
            }
            ConstValue::Str(_) => None,
        };
    }
    let in_range = match &value {
        ConstValue::Int(i) => *i > INT_MIN && *i < (1_i64 << 31),
        ConstValue::Float(f) | ConstValue::CFloat(f) => {
            f64::from(i32::MIN) < *f && *f < 2_147_483_648.0_f64
        }
        ConstValue::BigInt(_) | ConstValue::Str(_) => false,
    };
    if in_range {
        return match value {
            ConstValue::Int(i) => Some(i),
            ConstValue::Float(f) | ConstValue::CFloat(f) => pystr::py_int_from_f64(f),
            ConstValue::BigInt(_) | ConstValue::Str(_) => None,
        };
    }
    if matches!(profile.name, "cr300" | "cr350")
        && (_integer_literal(Some(arg))
            || matches!(arg, Expr::Name(n) if literals.contains(&pystr::upper(&n.name))))
    {
        #[allow(clippy::cast_precision_loss)]
        let fval = match value {
            ConstValue::Int(i) => i as f64,
            ConstValue::Float(f) | ConstValue::CFloat(f) => f,
            ConstValue::BigInt(s) => s.parse::<f64>().unwrap_or(f64::INFINITY),
            ConstValue::Str(_) => return Some(INT_MIN),
        };
        return Some(_float32_long(fval));
    }
    Some(INT_MIN)
}

/// Port of `checker/longconv.py::_integer_literal`.
pub fn _integer_literal(expr: Option<&Expr>) -> bool {
    let Some(mut e) = expr else {
        return false;
    };
    if let Expr::Unary(u) = e
        && u.op == "-"
    {
        let Some(op) = u.operand.as_deref() else {
            return false;
        };
        e = op;
    }
    matches!(
        e,
        Expr::Number(n) if !n.text.is_empty() && n.text.chars().all(|c| c.is_ascii_digit())
    )
}

/// Port of `checker/longconv.py::_hex_inside`.
pub fn _hex_inside(expr: Option<&Expr>) -> bool {
    let Some(expr) = expr else {
        return false;
    };
    match expr {
        Expr::Number(n) => n.text.starts_with('&'),
        Expr::Unary(u) => _hex_inside(u.operand.as_deref()),
        Expr::Binary(b) => _hex_inside(b.left.as_deref()) || _hex_inside(b.right.as_deref()),
        _ => false,
    }
}

/// Port of `checker/longconv.py::_wrap`.
pub fn _wrap(value: i64) -> i64 {
    let v = value & 0xFFFF_FFFF;
    if v >= (1_i64 << 31) {
        v - (1_i64 << 32)
    } else {
        v
    }
}

/// Port of `checker/longconv.py::_float32_long`.
#[allow(clippy::cast_possible_truncation)]
pub fn _float32_long(value: f64) -> i64 {
    let f: f64 = if value.abs() < 3.4e38 {
        f64::from(value as f32)
    } else {
        f64::INFINITY.copysign(value)
    };
    if !(f64::from(i32::MIN) <= f && f <= 2_147_483_648.0_f64) {
        return INT_MIN;
    }
    let i = pystr::py_int_from_f64(f).unwrap_or(INT_MIN);
    i.clamp(INT_MIN + 1, (1_i64 << 31) - 1)
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use super::*;
    use crate::profiles::get;
    use crate::syntax::{Binary, ExprRef, Name, Node, Number, StringLit, Unary};

    fn int_expr(n: i64) -> ExprRef {
        Rc::new(Expr::Number(Number {
            node: Node::default(),
            value: NumberValue::Int(n),
            text: n.to_string(),
        }))
    }

    fn float_expr(f: f64, text: &str) -> ExprRef {
        Rc::new(Expr::Number(Number {
            node: Node::default(),
            value: NumberValue::Float(f),
            text: text.to_string(),
        }))
    }

    #[test]
    fn long_value_matches_python() {
        let cr6 = get("cr6").unwrap();
        let cr300 = get("cr300").unwrap();
        let env = Env::new(cr6);
        let names = BTreeSet::new();
        let literals = BTreeSet::new();

        let mul = Expr::Binary(Binary {
            node: Node::default(),
            op: "*".to_string(),
            left: Some(int_expr(100_000)),
            right: Some(int_expr(100_000)),
        });
        assert_eq!(
            long_value_with(Some(&mul), &env, cr6, &names, &literals),
            Some(1_410_065_408)
        );

        let add = Expr::Binary(Binary {
            node: Node::default(),
            op: "+".to_string(),
            left: Some(int_expr(2_147_483_647)),
            right: Some(int_expr(1)),
        });
        assert_eq!(
            long_value_with(Some(&add), &env, cr6, &names, &literals),
            Some(-2_147_483_648)
        );

        let big = int_expr(2_147_483_648);
        assert_eq!(
            long_value_with(Some(&big), &env, cr6, &names, &literals),
            Some(-2_147_483_648)
        );
        assert_eq!(
            long_value_with(Some(&big), &env, cr300, &names, &literals),
            Some(2_147_483_647)
        );

        let neg_big = Expr::Unary(Unary {
            node: Node::default(),
            op: "-".to_string(),
            operand: Some(int_expr(2_147_483_649)),
        });
        assert_eq!(
            long_value_with(Some(&neg_big), &env, cr300, &names, &literals),
            Some(-2_147_483_647)
        );

        let flt = float_expr(2_147_483_648.0, "2147483648.0");
        assert_eq!(
            long_value_with(Some(&flt), &env, cr300, &names, &literals),
            Some(-2_147_483_648)
        );

        let s = Expr::String(StringLit {
            node: Node::default(),
            value: "abc".to_string(),
        });
        assert_eq!(
            long_value_with(Some(&s), &env, cr6, &names, &literals),
            Some(-2_147_483_648)
        );

        let nan = Expr::Name(Name {
            node: Node::default(),
            name: "NAN".to_string(),
        });
        assert_eq!(
            long_value_with(Some(&nan), &env, cr6, &names, &literals),
            Some(-2_147_483_648)
        );
    }
}
