//! Port of `mode.py`.

#![allow(
    clippy::used_underscore_items,
    reason = "port keeps Python's leading-underscore function names"
)]

use std::collections::{HashMap, HashSet};

use crate::consteval::{self, Env};
use crate::profiles::Profile;
use crate::pystr;
use crate::syntax::{ConstValue, Expr, ExprRef, Stmt, StmtRef};

/// Port of `mode.py::_const_value`.
pub fn _const_value() -> Option<f64> {
    todo!("mode.py::_const_value (omitted: expr: untyped, value_of: Callable)")
}

/// Port of `mode.py::_const_value`.
pub fn _const_value_with_env(expr: Option<&Expr>, env: &Env) -> Option<ConstValue> {
    let expr = expr?;
    consteval::value_of(Some(expr), env).ok()
}

/// Port of `mode.py::_numeric_constant`.
pub fn _numeric_constant() -> bool {
    todo!(
        "mode.py::_numeric_constant (omitted: expr: untyped, is_constant: Callable, value_of: Callable)"
    )
}

/// Port of `mode.py::_numeric_constant`.
pub fn _numeric_constant_with_env(expr: Option<&Expr>, env: &Env) -> bool {
    let Some(expr) = expr else {
        return false;
    };
    match expr {
        Expr::Number(_) => true,
        Expr::Name(_) => {
            consteval::is_constant(Some(expr), env)
                && !matches!(
                    _const_value_with_env(Some(expr), env),
                    Some(ConstValue::Str(_))
                )
        }
        Expr::Unary(u) => _numeric_constant_with_env(u.operand.as_deref(), env),
        Expr::Binary(b) => {
            _numeric_constant_with_env(b.left.as_deref(), env)
                && _numeric_constant_with_env(b.right.as_deref(), env)
        }
        _ => false,
    }
}

/// Port of `mode.py::decide`.
pub fn decide(_statements: &[StmtRef], _profile: &Profile) -> String {
    todo!("mode.py::decide (omitted: is_constant: Callable, value_of: Callable)")
}

/// Port of `mode.py::decide`.
pub fn decide_with_env(statements: &[StmtRef], profile: &Profile, env: &Env) -> String {
    struct State {
        sequential: bool,
        pipeline_forced: bool,
        depth: i64,
        in_scan: i64,
        in_slow: i64,
    }

    fn is_num_eq(val: Option<&ConstValue>, target: i32) -> bool {
        match val {
            Some(ConstValue::Int(i)) => *i == i64::from(target),
            Some(ConstValue::Float(f) | ConstValue::CFloat(f)) => *f == f64::from(target),
            _ => false,
        }
    }

    fn visit(
        stmt: &Stmt,
        state: &mut State,
        always_sequential: &HashSet<&str>,
        task_instructions: &HashSet<&str>,
        nonconstant_arg: &HashMap<&str, Vec<usize>>,
        variable_arg: &HashMap<&str, usize>,
        env: &Env,
    ) {
        let kw = _keyword(stmt);
        let args: &[Option<ExprRef>] = match stmt {
            Stmt::Instruction(inst) => &inst.args,
            _ => &[],
        };
        if kw == "SEQUENTIALMODE" {
            state.sequential = true;
        } else if kw == "PIPELINEMODE" {
            state.pipeline_forced = true;
        }
        if !state.sequential && !state.pipeline_forced {
            if always_sequential.contains(kw.as_str()) {
                state.sequential = true;
            } else if task_instructions.contains(kw.as_str())
                && (state.depth != 0 || state.in_scan == 0)
            {
                if kw == "SW12" {
                    state.sequential =
                        is_num_eq(_const_value_with_env(_arg_ref(args, 2), env).as_ref(), 1);
                } else if kw == "DELAY" {
                    let v = _const_value_with_env(_arg_ref(args, 0), env);
                    state.sequential = is_num_eq(v.as_ref(), 0) || is_num_eq(v.as_ref(), 2);
                } else if kw == "SERIALOUTBLOCK" || kw == "SERIALINRECORD" {
                    state.sequential = state.in_slow == 0 || state.depth > 0;
                } else {
                    state.sequential = true;
                }
            } else if let Some(indices) = nonconstant_arg.get(kw.as_str()) {
                state.sequential = indices.iter().any(|&i| {
                    let a = _arg_ref(args, i);
                    a.is_some() && !_numeric_constant_with_env(a, env)
                });
            } else if matches!(kw.as_str(), "FOR" | "IF" | "SELECT" | "WHILE" | "DO") {
                state.depth += 1;
            } else if matches!(kw.as_str(), "NEXT" | "ENDIF" | "END" | "WEND" | "LOOP") {
                let target = _block_word_in(args);
                if kw == "NEXT" && target.as_deref() == Some("SCAN") {
                    state.in_scan -= 1;
                } else if (kw == "NEXT" && target.as_deref() == Some("SUBSCAN"))
                    || (kw == "END" && !matches!(target.as_deref(), Some("IF" | "SELECT")))
                {
                    // pass
                } else {
                    state.depth -= 1;
                }
            }
        }
        if kw == "SLOWSEQUENCE" {
            state.in_slow += 1;
        } else if kw == "SCAN" {
            state.depth = 0;
            state.in_scan += 1;
        } else if kw == "NEXTSCAN" {
            state.in_scan -= 1;
        }
        if let Some(&idx) = variable_arg.get(kw.as_str())
            && !state.pipeline_forced
        {
            let arg = _arg_ref(args, idx);
            if arg.is_none() || !consteval::is_constant(arg, env) {
                state.sequential = true;
            }
        }
        if let Stmt::If(if_stmt) = stmt
            && let Some(inline) = &if_stmt.inline
        {
            for inner in inline {
                visit(
                    inner.as_ref(),
                    state,
                    always_sequential,
                    task_instructions,
                    nonconstant_arg,
                    variable_arg,
                    env,
                );
            }
            if let Some(inline_else) = &if_stmt.inline_else {
                for inner in inline_else {
                    visit(
                        inner.as_ref(),
                        state,
                        always_sequential,
                        task_instructions,
                        nonconstant_arg,
                        variable_arg,
                        env,
                    );
                }
            }
            if !state.sequential && !state.pipeline_forced {
                state.depth -= 1;
            }
        }
    }

    let rules = profile.mode_rules();
    let always_sequential: HashSet<&str> = rules
        .get("always_sequential")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect())
        .unwrap_or_default();
    let task_instructions: HashSet<&str> = rules
        .get("task_instructions")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect())
        .unwrap_or_default();
    let nonconstant_arg: HashMap<&str, Vec<usize>> = rules
        .get("nonconstant_arg")
        .and_then(|v| v.as_object())
        .map(|obj| {
            obj.iter()
                .map(|(k, v)| {
                    let indices = v
                        .as_array()
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|x| {
                                    x.as_f64()
                                        .and_then(pystr::py_int_from_f64)
                                        .and_then(|i| usize::try_from(i).ok())
                                })
                                .collect()
                        })
                        .unwrap_or_default();
                    (k.as_str(), indices)
                })
                .collect()
        })
        .unwrap_or_default();
    let variable_arg: HashMap<&str, usize> = rules
        .get("variable_arg")
        .and_then(|v| v.as_object())
        .map(|obj| {
            obj.iter()
                .filter_map(|(k, v)| {
                    v.as_f64()
                        .and_then(pystr::py_int_from_f64)
                        .and_then(|i| usize::try_from(i).ok())
                        .map(|idx| (k.as_str(), idx))
                })
                .collect()
        })
        .unwrap_or_default();

    let mut state = State {
        sequential: false,
        pipeline_forced: false,
        depth: 0,
        in_scan: 0,
        in_slow: 0,
    };

    for stmt in statements {
        visit(
            stmt.as_ref(),
            &mut state,
            &always_sequential,
            &task_instructions,
            &nonconstant_arg,
            &variable_arg,
            env,
        );
    }

    if state.sequential {
        "Sequential".to_string()
    } else {
        "Pipeline".to_string()
    }
}

/// Port of `mode.py::_keyword`.
pub fn _keyword(stmt: &Stmt) -> String {
    match stmt {
        Stmt::Instruction(inst) => pystr::upper(&inst.keyword),
        Stmt::If(_) => "IF".to_string(),
        Stmt::For(_) => "FOR".to_string(),
        Stmt::DoLoop(dl) => pystr::upper(&dl.keyword),
        _ => "OTHER".to_string(),
    }
}

/// Port of `mode.py::_arg`.
pub fn _arg(_i: i64) {
    todo!("mode.py::_arg (omitted: args: list)")
}

/// Port of `mode.py::_arg`.
pub fn _arg_ref(args: &[Option<ExprRef>], i: usize) -> Option<&Expr> {
    args.get(i).and_then(|e| e.as_deref())
}

/// Port of `mode.py::_block_word`.
pub fn _block_word() -> Option<String> {
    todo!("mode.py::_block_word (omitted: args: list)")
}

/// Port of `mode.py::_block_word`.
pub fn _block_word_in(args: &[Option<ExprRef>]) -> Option<String> {
    match args.first().and_then(|e| e.as_deref()) {
        Some(Expr::Name(n)) => Some(pystr::upper(&n.name)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use super::*;
    use crate::profiles::get;
    use crate::syntax::Instruction;

    fn inst(kw: &str) -> StmtRef {
        Rc::new(Stmt::Instruction(Instruction {
            keyword: kw.to_string(),
            ..Default::default()
        }))
    }

    #[test]
    fn mode_decide_matches_python() {
        let profile = get("cr6").unwrap();
        let env = Env::new(profile);
        let stmts = vec![
            inst("BeginProg"),
            inst("Scan"),
            inst("NextScan"),
            inst("EndProg"),
        ];
        assert_eq!(decide_with_env(&stmts, profile, &env), "Pipeline");

        let stmts2 = vec![
            inst("SequentialMode"),
            inst("BeginProg"),
            inst("Scan"),
            inst("NextScan"),
            inst("EndProg"),
        ];
        assert_eq!(decide_with_env(&stmts2, profile, &env), "Sequential");
    }
}
