//! Port of `checker/ess.py`.

#![allow(
    clippy::used_underscore_items,
    reason = "port keeps Python's leading-underscore function names"
)]

use std::collections::HashMap;

use crate::consteval::{self, Env};
use crate::diagnostics::{Diagnostics, ShowFile};
use crate::profiles::{EssVariable, Profile};
use crate::pystr;
use crate::syntax::{ConstValue, Expr, Program, Stmt};

/// Port of `checker/ess.py::_table`.
pub fn _table(_data_dir: &str) {
    // Table data is embedded per-profile via `Profile::ess_variables()`.
}

/// Port of `checker/ess.py::_table`.
pub fn _table_for_profile(profile: &Profile) -> &'static [EssVariable] {
    profile.ess_variables()
}

/// Port of `checker/ess.py::_const_int`.
pub fn _const_int(_expr: Option<&Expr>) -> Option<i64> {
    todo!("checker/ess.py::_const_int (omitted: env: consteval.Env)")
}

/// Port of `checker/ess.py::_const_int`.
pub fn _const_int_with_env(expr: Option<&Expr>, env: &Env) -> Option<i64> {
    let expr = expr?;
    if !consteval::is_constant(Some(expr), env) {
        return None;
    }
    let value = consteval::value_of(Some(expr), env).ok()?;
    match value {
        ConstValue::Int(i) => Some(i),
        ConstValue::Float(f) | ConstValue::CFloat(f) => {
            if !f.is_finite() {
                return None;
            }
            pystr::py_int_from_f64(f)
        }
        ConstValue::Str(_) => None,
    }
}

/// Port of `checker/ess.py::check`.
pub fn check(_program: &Program, _diags: &mut Diagnostics) {
    todo!("checker/ess.py::check (omitted: profile: untyped, env: consteval.Env)")
}

/// Port of `checker/ess.py::check`.
pub fn check_with_env(program: &Program, profile: &Profile, diags: &mut Diagnostics, env: &Env) {
    let table = _table_for_profile(profile);
    if table.is_empty() {
        return;
    }
    let mut declared: HashMap<String, (String, String, Option<i64>, Option<i64>)> = HashMap::new();
    for stmt in &program.statements {
        match stmt.as_ref() {
            Stmt::Declaration(decl)
                if matches!(decl.kind.as_str(), "PUBLIC" | "DIM" | "STATIC") =>
            {
                for v in &decl.vars {
                    let mut total: Option<i64> = Some(1);
                    for d in &v.dims {
                        let n = _const_int_with_env(Some(d.as_ref()), env);
                        total = match (total, n) {
                            (Some(t), Some(n)) => t.checked_mul(n),
                            _ => None,
                        };
                    }
                    let kind = pystr::upper(v.type_.as_deref().unwrap_or("FLOAT"));
                    let size = if let Some(ss) = &v.string_size {
                        _const_int_with_env(Some(ss.as_ref()), env)
                    } else {
                        Some(24)
                    };
                    declared
                        .entry(pystr::upper(&v.name))
                        .or_insert_with(|| (v.name.clone(), kind, total, size));
                }
            }
            Stmt::Instruction(inst) if pystr::upper(&inst.keyword) == "ESSVARIABLES" => {
                for row in table {
                    if row.count == 0 {
                        continue;
                    }
                    let required = if row.type_ == "STRING" {
                        "STRING"
                    } else {
                        "LONG"
                    };
                    let key = pystr::upper(&row.name);
                    if let Some((spelling, kind, total, size)) = declared.get(&key) {
                        if kind != required {
                            let req_cap = if required == "STRING" {
                                "String"
                            } else {
                                "Long"
                            };
                            diags.error(
                                inst.base.node.line,
                                &format!("ESS variable {spelling} must be declared as {req_cap}"),
                                &inst.base.node.file,
                                ShowFile::Bool(true),
                                0,
                                "",
                                "",
                            );
                        } else if required == "STRING" && size.is_some_and(|s| s != 255) {
                            diags.error(
                                inst.base.node.line,
                                "ESS variable of type string must be of size 255",
                                &inst.base.node.file,
                                ShowFile::Bool(true),
                                0,
                                "",
                                "",
                            );
                        } else if total.is_some_and(|t| t != row.count) {
                            diags.error(
                                inst.base.node.line,
                                &format!(
                                    "ESS variable {spelling} must be dimensioned as {}",
                                    row.count
                                ),
                                &inst.base.node.file,
                                ShowFile::Bool(true),
                                0,
                                "",
                                "",
                            );
                        }
                    } else {
                        let total = row.count * if row.type_ == "LONG_ARRAY16" { 16 } else { 1 };
                        declared.insert(
                            key,
                            (
                                row.name.clone(),
                                required.to_string(),
                                Some(total),
                                Some(255),
                            ),
                        );
                    }
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use super::*;
    use crate::profiles::get;
    use crate::syntax::{
        Declaration, ExprRef, Instruction, Node, Number, NumberValue, StmtBase, VarDecl,
    };

    fn num(n: i64) -> ExprRef {
        Rc::new(Expr::Number(Number {
            node: Node::default(),
            value: NumberValue::Int(n),
            text: n.to_string(),
        }))
    }

    #[test]
    fn ess_checks_match_python() {
        let profile = get("cr6").unwrap();
        let decl = |line: i64, name: &str, ty: &str, dims: Vec<ExprRef>, size: Option<ExprRef>| {
            Rc::new(Stmt::Declaration(Declaration {
                base: StmtBase {
                    node: Node {
                        line,
                        file: "test.cr6".to_string(),
                    },
                    ..Default::default()
                },
                kind: "PUBLIC".to_string(),
                vars: vec![Rc::new(VarDecl {
                    name: name.to_string(),
                    type_: Some(ty.to_string()),
                    dims,
                    string_size: size,
                    ..Default::default()
                })],
            }))
        };
        let ess_stmt = Rc::new(Stmt::Instruction(Instruction {
            base: StmtBase {
                node: Node {
                    line: 4,
                    file: "test.cr6".to_string(),
                },
                ..Default::default()
            },
            keyword: "ESSVariables".to_string(),
            ..Default::default()
        }));
        let str_var = profile
            .ess_variables()
            .iter()
            .find(|r| r.type_ == "STRING" && r.count > 0)
            .unwrap();
        let prog = Program {
            statements: vec![
                decl(1, "essTypeofStation", "Float", vec![], None),
                decl(2, &str_var.name, "String", vec![], Some(num(10))),
                ess_stmt,
            ],
            ..Default::default()
        };
        let mut diags = Diagnostics::new();
        let env = Env::new(profile);
        check_with_env(&prog, profile, &mut diags, &env);
        let msgs: Vec<&str> = diags.items.iter().map(|d| d.message.as_str()).collect();
        assert_eq!(
            msgs,
            vec![
                "ESS variable essTypeofStation must be declared as Long",
                "ESS variable of type string must be of size 255",
            ]
        );
    }
}
