//! Port of `checker/symbols.py`.

#![allow(dead_code, unused_variables, unused_imports)]

use std::collections::BTreeSet;
use std::rc::Rc;
use std::sync::LazyLock;

use crate::diagnostics::{DiagRef, Diagnostic, Diagnostics, ShowFile};
use crate::profiles::Profile;
use crate::pyre;
use crate::pystr;
use crate::syntax::{
    Assignment, Call, Case, CaseItem, ConstDecl, Declaration, Expr, ExprRef, Index, Instruction,
    Juxtaposed, Member, Number, OrderedMap, Program, ProgramRef, Stmt, StmtRef, SubDef, Unparsed,
    VarDecl,
};

pub const _NAME_DECLARING_INSTRUCTIONS: &[&str] =
    &["DATATABLE", "STRUCTURETYPE", "OBJECTTYPE", "CONSTTABLE"];

pub const _ALWAYS_KNOWN_SYSTEM_TABLES: &[&str] = &["STATUS", "PUBLIC", "SETTINGS"];

pub static _SNMP_VARIABLE_RE: LazyLock<pyre::Regex> =
    LazyLock::new(|| pyre::Regex::new(r"(?i)\s*SNMPVariable\s*\(\s*([A-Za-z_][A-Za-z0-9_]*)", 0));

/// Port of `checker/symbols.py::_walk_names`.
pub fn _walk_names(_expr: Option<&Expr>) {
    todo!("checker/symbols.py::_walk_names (signature returns () instead of Vec<String>)")
}

/// Port of `checker/symbols.py::_walk_names`.
pub fn walk_names(expr: Option<&Expr>) -> Vec<String> {
    let mut out = Vec::new();
    let Some(expr) = expr else {
        return out;
    };
    match expr {
        Expr::Name(n) => out.push(n.name.clone()),
        Expr::Unary(u) => out.extend(walk_names(u.operand.as_deref())),
        Expr::Binary(b) => {
            out.extend(walk_names(b.left.as_deref()));
            out.extend(walk_names(b.right.as_deref()));
        }
        Expr::Index(idx) => {
            out.push(idx.name.clone());
            for a in &idx.args {
                out.extend(walk_names(a.as_deref()));
            }
        }
        Expr::Member(m) => out.extend(walk_names(m.base.as_deref())),
        Expr::ArrayLiteral(al) => {
            for item in &al.items {
                out.extend(walk_names(item.as_deref()));
            }
        }
        Expr::Number(_) | Expr::String(_) | Expr::Juxtaposed(_) => {}
    }
    out
}

/// Port of `checker/symbols.py::_stmt_exprs`.
pub fn _stmt_exprs(_stmt: &Stmt) {
    todo!("checker/symbols.py::_stmt_exprs (signature returns () instead of Vec<Option<ExprRef>>)")
}

/// Port of `checker/symbols.py::_stmt_exprs`.
pub fn stmt_exprs(stmt: &Stmt) -> Vec<Option<ExprRef>> {
    let mut out = Vec::new();
    match stmt {
        Stmt::Declaration(d) => {
            for v in d.vars.borrow().iter() {
                out.extend(v.dims.iter().cloned().map(Some));
                if let Some(init) = &v.init {
                    out.extend(init.iter().cloned());
                }
                out.push(v.string_size.clone());
            }
        }
        Stmt::ConstDecl(c) => out.push(c.value.clone()),
        Stmt::Assignment(a) => {
            out.push(a.target.clone());
            out.push(a.value.clone());
        }
        Stmt::Instruction(i) => out.extend(i.args.iter().cloned()),
        Stmt::Call(c) => out.extend(c.args.iter().cloned()),
        Stmt::If(i) => {
            out.push(i.cond.clone());
            if let Some(inline) = &i.inline {
                for s2 in inline {
                    out.extend(stmt_exprs(s2));
                }
            }
            if let Some(inline_else) = &i.inline_else {
                for s2 in inline_else {
                    out.extend(stmt_exprs(s2));
                }
            }
        }
        Stmt::ElseIf(e) => out.push(e.cond.clone()),
        Stmt::DoLoop(d) => out.push(d.cond.clone()),
        Stmt::For(f) => {
            out.push(f.var.clone());
            out.push(f.start.clone());
            out.push(f.end.clone());
            out.push(f.step.clone());
        }
        Stmt::Case(c) => {
            for item in &c.items {
                match item {
                    CaseItem::Value(e) | CaseItem::Is(_, e) => out.push(Some(Rc::clone(e))),
                    CaseItem::Range(a, b) => {
                        out.push(Some(Rc::clone(a)));
                        out.push(Some(Rc::clone(b)));
                    }
                }
            }
        }
        Stmt::SubDef(s) => {
            for p in &s.params {
                out.push(p.default.clone());
                out.push(p.string_size.clone());
            }
        }
        _ => {}
    }
    out
}

/// Port of `checker/symbols.py::_name_nodes`.
pub fn _name_nodes(_expr: Option<&Expr>) {
    todo!("checker/symbols.py::_name_nodes (signature returns () instead of Vec<ExprRef>)")
}

/// Port of `checker/symbols.py::_name_nodes`.
pub fn name_nodes(expr: Option<&ExprRef>) -> Vec<ExprRef> {
    let mut out = Vec::new();
    let Some(expr) = expr else {
        return out;
    };
    match expr.as_ref() {
        Expr::Name(_) => out.push(Rc::clone(expr)),
        Expr::Index(idx) => {
            out.push(Rc::clone(expr));
            for a in &idx.args {
                out.extend(name_nodes(a.as_ref()));
            }
        }
        Expr::Unary(u) => out.extend(name_nodes(u.operand.as_ref())),
        Expr::Binary(b) => {
            out.extend(name_nodes(b.left.as_ref()));
            out.extend(name_nodes(b.right.as_ref()));
        }
        Expr::Member(m) => {
            out.extend(name_nodes(m.base.as_ref()));
            if let Some(args) = &m.args {
                for a in args {
                    out.extend(name_nodes(a.as_ref()));
                }
            }
        }
        Expr::ArrayLiteral(al) => {
            for item in &al.items {
                out.extend(name_nodes(item.as_ref()));
            }
        }
        Expr::Number(_) | Expr::String(_) | Expr::Juxtaposed(_) => {}
    }
    out
}

/// Port of `checker/symbols.py::is_reserved_predefined_name`.
pub fn is_reserved_predefined_name(_name: &str) -> bool {
    todo!("checker/symbols.py::is_reserved_predefined_name (omitted: profile: untyped)")
}

/// Port of `checker/symbols.py::is_reserved_predefined_name`.
pub fn is_reserved_predefined_name_with_profile(name: &str, profile: &Profile) -> bool {
    let key = pystr::upper(name);
    profile.symbols().contains(&key) && !profile.declarable_predefined_names.contains(&key.as_str())
}

/// Port of `checker/symbols.py::report_predefined_name_collision`.
pub fn report_predefined_name_collision(
    _stmt: &Stmt,
    _name: &str,
    _diags: &mut Diagnostics,
) -> bool {
    todo!("checker/symbols.py::report_predefined_name_collision (omitted: profile: untyped)")
}

/// Port of `checker/symbols.py::report_predefined_name_collision`.
pub fn report_predefined_name_collision_with_profile(
    stmt: &Stmt,
    name: &str,
    profile: &Profile,
    diags: &mut Diagnostics,
) -> bool {
    if !is_reserved_predefined_name_with_profile(name, profile) {
        return false;
    }
    let node = &stmt.base().node;
    diags.error(
        node.line,
        &format!("{name} is a predefined CONST."),
        &node.file,
        ShowFile::Bool(true),
        0,
        "",
        "",
    );
    true
}

/// Port of `checker/symbols.py::report_function_name_collision`.
pub fn report_function_name_collision(_stmt: &Stmt, _name: &str, _diags: &mut Diagnostics) -> bool {
    todo!("checker/symbols.py::report_function_name_collision (omitted: profile: untyped)")
}

/// Port of `checker/symbols.py::report_function_name_collision`.
pub fn report_function_name_collision_with_profile(
    stmt: &Stmt,
    name: &str,
    profile: &Profile,
    diags: &mut Diagnostics,
) -> bool {
    if !profile.functions().contains(&pystr::upper(name)) {
        return false;
    }
    let node = &stmt.base().node;
    diags.error(
        node.line,
        &format!("{name} already defined as an internal function."),
        &node.file,
        ShowFile::Bool(true),
        0,
        "",
        "",
    );
    true
}

/// Port of `checker/symbols.py::check_local_name_collision`.
pub fn check_local_name_collision(_stmt: &Stmt, _diags: &mut Diagnostics) {
    todo!("checker/symbols.py::check_local_name_collision (omitted: profile: untyped)")
}

/// Port of `checker/symbols.py::check_local_name_collision`.
pub fn check_local_name_collision_with_profile(
    stmt: &Stmt,
    profile: &Profile,
    diags: &mut Diagnostics,
) {
    let declaration_vars = match stmt {
        Stmt::Declaration(d) => Some(d.vars.borrow()),
        _ => None,
    };
    let (names, shadows_functions): (Vec<&str>, bool) = match stmt {
        Stmt::ConstDecl(c) => (vec![c.name.as_str()], false),
        Stmt::Declaration(d) if d.kind == "PUBLIC" || d.kind == "DIM" => (
            declaration_vars
                .as_ref()
                .unwrap()
                .iter()
                .map(|v| v.name.as_str())
                .collect(),
            d.kind == "DIM",
        ),
        Stmt::SubDef(s) => (s.params.iter().map(|p| p.name.as_str()).collect(), true),
        _ => return,
    };
    let node = &stmt.base().node;
    for (index, name) in names.iter().enumerate() {
        let mut collision = !shadows_functions
            && report_function_name_collision_with_profile(stmt, name, profile, diags);
        collision =
            collision || report_predefined_name_collision_with_profile(stmt, name, profile, diags);
        if collision && profile.single_pass {
            if index + 2 < names.len() {
                diags.error(
                    node.line,
                    &format!("Too many parameters:  {}.", names[index + 2]),
                    &node.file,
                    ShowFile::Bool(true),
                    0,
                    "",
                    "",
                );
            }
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profiles::get as get_profile;
    use crate::syntax::{Binary, Name, Node, StmtBase, Unary};

    #[test]
    fn test_walk_names_and_name_nodes() {
        let x: ExprRef = Rc::new(Expr::Name(Name {
            node: Node::default(),
            name: "x".to_string(),
        }));
        let y: ExprRef = Rc::new(Expr::Name(Name {
            node: Node::default(),
            name: "y".to_string(),
        }));
        let idx: ExprRef = Rc::new(Expr::Index(Index {
            node: Node::default(),
            name: "arr".to_string(),
            args: vec![Some(Rc::clone(&x))],
        }));
        let bin: ExprRef = Rc::new(Expr::Binary(Binary {
            node: Node::default(),
            op: "+".to_string(),
            left: Some(Rc::clone(&idx)),
            right: Some(Rc::new(Expr::Unary(Unary {
                node: Node::default(),
                op: "-".to_string(),
                operand: Some(Rc::clone(&y)),
            }))),
        }));
        assert_eq!(walk_names(Some(&bin)), vec!["arr", "x", "y"]);
        let nodes = name_nodes(Some(&bin));
        assert_eq!(nodes.len(), 3);
    }

    #[test]
    fn test_local_name_collision() {
        let profile = get_profile("CR6").expect("CR6 profile");
        let stmt = Stmt::ConstDecl(ConstDecl {
            base: StmtBase::new(10, ""),
            name: "Sin".to_string(),
            type_: None,
            value: None,
            value_span: None,
        });
        let mut diags = Diagnostics::default();
        check_local_name_collision_with_profile(&stmt, profile, &mut diags);
        assert_eq!(diags.items.len(), 1);
        assert_eq!(
            diags.items[0].message,
            "Sin already defined as an internal function."
        );
    }
}
