//! Port of `checker/structs.py`.

#![allow(dead_code, unused_variables, unused_imports)]

use std::collections::BTreeSet;
use std::rc::Rc;

use crate::diagnostics::{DiagRef, Diagnostic, Diagnostics, ShowFile};
use crate::profiles::Profile;
use crate::pystr;
use crate::syntax::{
    Assignment, Call, Case, ConstDecl, Declaration, Expr, ExprRef, Index, Instruction, Juxtaposed,
    Member, Number, NumberValue, OrderedMap, Program, ProgramRef, Stmt, StmtRef, SubDef, Unparsed,
    VarDecl,
};

use super::{context, expressions, structure};

const _SCALAR_TYPES: &[&str] = &["FLOAT", "LONG", "BOOLEAN", "STRING", "DOUBLE"];
const _WRONG_TYPE: &str = "Structured Variable assignment illegal, wrong type";
const _ILLEGAL: &str = "Illegal structure type assignment";

fn unmodelled() -> Vec<(String, String, i64)> {
    vec![("?".to_string(), String::new(), 0)]
}

/// Port of `checker/structs.py::_structure_members`.
pub fn _structure_members(program: &Program) -> OrderedMap<String, OrderedMap<String, String>> {
    let mut result: OrderedMap<String, OrderedMap<String, String>> = OrderedMap::new();
    let mut current: Option<String> = None;
    for st in &program.statements {
        match st.as_ref() {
            Stmt::Instruction(inst)
                if pystr::upper(&inst.keyword) == "STRUCTURETYPE" && !inst.args.is_empty() =>
            {
                if let Some(Some(Expr::Name(n))) = inst.args.first().map(|a| a.as_deref()) {
                    let key = pystr::upper(&n.name);
                    if !result.contains_key(&key) {
                        result.insert(key.clone(), OrderedMap::new());
                    }
                    current = Some(key);
                }
            }
            Stmt::Instruction(inst) if pystr::upper(&inst.keyword) == "ENDSTRUCTURETYPE" => {
                current = None;
            }
            Stmt::Declaration(decl) if current.is_some() && decl.kind == "STRUCTUREMEMBER" => {
                if let Some(ref cur) = current {
                    let mut map = result.get(cur).cloned().unwrap_or_default();
                    for v in decl.vars.borrow().iter() {
                        let ty = v.type_.as_deref().unwrap_or("FLOAT");
                        map.insert(pystr::upper(&v.name), pystr::upper(ty));
                    }
                    result.insert(cur.clone(), map);
                }
            }
            _ => {}
        }
    }
    result
}

/// Port of `checker/structs.py::_variable_kinds`.
pub fn _variable_kinds(
    program: &Program,
    structs: &OrderedMap<String, OrderedMap<String, String>>,
) -> OrderedMap<String, String> {
    let mut kinds: OrderedMap<String, String> = OrderedMap::new();
    let mut clash: BTreeSet<String> = BTreeSet::new();
    for st in &program.statements {
        let Stmt::Declaration(decl) = st.as_ref() else {
            continue;
        };
        if decl.kind != "PUBLIC" && decl.kind != "DIM" && decl.kind != "STATIC" {
            continue;
        }
        for v in decl.vars.borrow().iter() {
            let mut kind = pystr::upper(v.type_.as_deref().unwrap_or("FLOAT"));
            if v.paren_dim {
                kind = "FLOAT".to_string();
            }
            if structs.contains_key(&kind) {
                kind = format!("STRUCT:{kind}");
            } else if !_SCALAR_TYPES.contains(&kind.as_str()) {
                clash.insert(pystr::upper(&v.name));
                continue;
            }
            let vname = pystr::upper(&v.name);
            if let Some(existing) = kinds.get(&vname) {
                if *existing != kind {
                    clash.insert(vname);
                }
            } else {
                kinds.insert(vname, kind);
            }
        }
    }
    let mut out = OrderedMap::new();
    for (k, v) in kinds.iter() {
        if !clash.contains(k) {
            out.insert(k.clone(), v.clone());
        }
    }
    out
}

/// Port of `checker/structs.py::_names`.
pub fn _names(expr: Option<&Expr>) -> Vec<String> {
    let Some(expr) = expr else {
        return Vec::new();
    };
    match expr {
        Expr::Number(_) | Expr::String(_) => Vec::new(),
        Expr::Name(n) => vec![pystr::upper(&n.name)],
        Expr::Index(idx) => {
            let mut out = vec![pystr::upper(&idx.name)];
            for a in &idx.args {
                out.extend(_names(a.as_deref()));
            }
            out
        }
        Expr::Unary(u) => _names(u.operand.as_deref()),
        Expr::Binary(b) => {
            let mut out = _names(b.left.as_deref());
            out.extend(_names(b.right.as_deref()));
            out
        }
        _ => vec!["?".to_string()],
    }
}

/// Port of `checker/structs.py::_member_chain`.
pub fn _member_chain(expr: &Member) -> Option<(&Expr, Vec<String>)> {
    let mut path = vec![expr.member.clone()];
    let mut node: Option<&Expr> = expr.base.as_deref();
    while let Some(Expr::Member(m)) = node {
        if m.args.as_ref().is_some_and(|a| !a.is_empty()) {
            return None;
        }
        path.push(m.member.clone());
        node = m.base.as_deref();
    }
    match node {
        Some(n @ (Expr::Name(_) | Expr::Index(_))) => {
            path.reverse();
            Some((n, path))
        }
        _ => None,
    }
}

/// Port of `checker/structs.py::_member_errors`.
pub fn _member_errors(
    expr: Option<&Expr>,
    kinds: &OrderedMap<String, String>,
    structs: &OrderedMap<String, OrderedMap<String, String>>,
) -> Vec<(String, String, i64)> {
    let Some(expr) = expr else {
        return Vec::new();
    };
    match expr {
        Expr::Member(m) => {
            let chain = _member_chain(m);
            let root = chain.as_ref().map(|(r, _)| *r);
            let root_name = match root {
                Some(Expr::Name(n)) => Some(n.name.as_str()),
                Some(Expr::Index(idx)) => Some(idx.name.as_str()),
                _ => None,
            };
            let kind = root_name.and_then(|rn| kinds.get(&pystr::upper(rn)));
            if chain.is_none() || kind.is_some_and(|k| !k.starts_with("STRUCT:")) {
                return unmodelled();
            }
            let Some(kind) = kind else {
                return Vec::new();
            };
            let (root, path) = chain.unwrap();
            let struct_name = &kind[7..];
            let first = structs
                .get(struct_name)
                .and_then(|s| s.get(&pystr::upper(&path[0])));
            let mut index: i64 = 1;
            if let Some(args) = &m.args {
                if !args.is_empty() {
                    let a = if args.len() == 1 {
                        args[0].as_deref()
                    } else {
                        None
                    };
                    if let Some(Expr::Number(Number {
                        value: NumberValue::Int(val),
                        ..
                    })) = a
                    {
                        index = *val;
                    } else {
                        return unmodelled();
                    }
                }
            }
            let mut found = if first.is_none()
                || (path.len() > 1 && first.is_some_and(|f| _SCALAR_TYPES.contains(&f.as_str())))
            {
                vec![(root_name.unwrap_or("").to_string(), path.join("."), index)]
            } else if first.is_some_and(|f| structs.contains_key(f)) || path.len() > 1 {
                return unmodelled();
            } else {
                Vec::new()
            };
            if let Expr::Index(idx) = root {
                for a in &idx.args {
                    found.extend(_member_errors(a.as_deref(), kinds, structs));
                }
            }
            found
        }
        Expr::Index(idx) => {
            let mut out = Vec::new();
            for a in &idx.args {
                out.extend(_member_errors(a.as_deref(), kinds, structs));
            }
            out
        }
        Expr::Unary(u) => _member_errors(u.operand.as_deref(), kinds, structs),
        Expr::Binary(b) => {
            let mut out = _member_errors(b.left.as_deref(), kinds, structs);
            out.extend(_member_errors(b.right.as_deref(), kinds, structs));
            out
        }
        _ => Vec::new(),
    }
}

/// Port of `checker/structs.py::check_members`.
pub fn check_members(
    program: &ProgramRef,
    profile: &'static Profile,
    diags: &mut Diagnostics,
    flat: &Program,
) {
    let structs = _structure_members(program);
    if structs.is_empty() {
        return;
    }
    let mut kinds = _variable_kinds(program, &structs);
    let mut declared: BTreeSet<String> = BTreeSet::new();
    for st in &program.statements {
        if let Stmt::Declaration(d) = st.as_ref() {
            for v in d.vars.borrow().iter() {
                declared.insert(pystr::upper(&v.name));
            }
        }
    }
    for st in &program.statements {
        if let Stmt::SubDef(sd) = st.as_ref() {
            for p in &sd.params {
                if let Some(ty) = &p.type_ {
                    let ty_up = pystr::upper(ty);
                    let pname_up = pystr::upper(&p.name);
                    if structs.contains_key(&ty_up)
                        && !declared.contains(&pname_up)
                        && !kinds.contains_key(&pname_up)
                    {
                        kinds.insert(pname_up, format!("STRUCT:{ty_up}"));
                    }
                }
            }
        }
    }
    let ctx = context::of(program, Some(profile), None);
    let known = crate::checker::expressions::KnownView::from(ctx.known_names());
    for st in &flat.statements {
        let Stmt::Assignment(assign) = st.as_ref() else {
            continue;
        };
        if assign.compound
            || !expressions::part1::_undefined_names(assign.target.as_deref(), &known).is_empty()
            || !expressions::part1::_undefined_names(assign.value.as_deref(), &known).is_empty()
        {
            continue;
        }
        let left = _member_errors(assign.target.as_deref(), &kinds, &structs);
        let right = _member_errors(assign.value.as_deref(), &kinds, &structs);
        if left.iter().chain(right.iter()).any(|e| e.0 == "?")
            || (left.is_empty() && right.is_empty())
        {
            continue;
        }
        let node = &assign.base.node;
        if !profile.single_pass {
            for (base, path, _) in left.iter().chain(right.iter()) {
                diags.error(
                    node.line,
                    &format!("In structure {base}, could not find .{path}"),
                    &node.file,
                    ShowFile::Bool(true),
                    0,
                    "",
                    "",
                );
            }
        } else if !left.is_empty() {
            let (base, path, index) = &left[0];
            let sep = if profile.fallback_echo_newline && assign.base.at_eol.get() {
                "\n"
            } else {
                ""
            };
            diags.error(
                node.line,
                &format!("Could not find {base}.{path}({index})."),
                &node.file,
                ShowFile::Bool(true),
                0,
                "",
                "",
            );
            diags.error(
                node.line,
                &format!(
                    "Invalid assignment in expression: {}{sep}.",
                    assign.base.text
                ),
                &node.file,
                ShowFile::Bool(true),
                0,
                "",
                "",
            );
        } else {
            for (base, path, index) in &right {
                diags.error(
                    node.line,
                    &format!("Could not find {base}.{path}({index})."),
                    &node.file,
                    ShowFile::Bool(true),
                    0,
                    "",
                    "",
                );
            }
        }
    }
}

/// Port of `checker/structs.py::_operand_kind`.
pub fn _operand_kind(
    expr: Option<&Expr>,
    kinds: &OrderedMap<String, String>,
    structs: &OrderedMap<String, OrderedMap<String, String>>,
) -> Option<String> {
    let expr = expr?;
    match expr {
        Expr::Number(_) | Expr::String(_) => Some("N".to_string()),
        Expr::Name(n) => {
            let kind = kinds.get(&pystr::upper(&n.name))?;
            if kind.starts_with("STRUCT:") {
                Some("S".to_string())
            } else {
                Some("N".to_string())
            }
        }
        Expr::Index(idx) => {
            let kind = kinds.get(&pystr::upper(&idx.name))?;
            if !kind.starts_with("STRUCT:") {
                Some("N".to_string())
            } else if idx.args.is_empty() {
                Some("SA()".to_string())
            } else {
                Some("S".to_string())
            }
        }
        Expr::Member(m) => {
            let chain = _member_chain(m)?;
            let root_name = match chain.0 {
                Expr::Name(n) => &n.name,
                Expr::Index(idx) => &idx.name,
                _ => return None,
            };
            let kind = kinds.get(&pystr::upper(root_name));
            let members = kind
                .filter(|k| k.starts_with("STRUCT:"))
                .and_then(|k| structs.get(&k[7..]));
            if chain.1.len() == 1 {
                if let Some(m_map) = members {
                    if let Some(ty) = m_map.get(&pystr::upper(&chain.1[0])) {
                        if _SCALAR_TYPES.contains(&ty.as_str()) {
                            return Some("M".to_string());
                        }
                    }
                }
            }
            None
        }
        Expr::Unary(_) | Expr::Binary(_) => {
            let names = _names(Some(expr));
            if names.iter().any(|n| n == "?" || !kinds.contains_key(n)) {
                return None;
            }
            if matches!(expr, Expr::Binary(_))
                && names
                    .iter()
                    .any(|n| kinds.get(n).is_some_and(|k| k.starts_with("STRUCT:")))
            {
                return None;
            }
            Some("N".to_string())
        }
        _ => None,
    }
}

/// Port of `checker/structs.py::check_struct_assignment`.
pub fn check_struct_assignment(
    program: &Program,
    profile: &Profile,
    diags: &mut Diagnostics,
    flat: &Program,
) {
    if !profile.single_pass {
        return;
    }
    let structs = _structure_members(program);
    if structs.is_empty() {
        return;
    }
    let kinds = _variable_kinds(program, &structs);
    for st in &flat.statements {
        let Stmt::Assignment(assign) = st.as_ref() else {
            continue;
        };
        if assign.compound {
            continue;
        }
        if !matches!(
            assign.target.as_deref(),
            Some(Expr::Name(_) | Expr::Index(_))
        ) {
            continue;
        }
        let left = _operand_kind(assign.target.as_deref(), &kinds, &structs);
        let right = _operand_kind(assign.value.as_deref(), &kinds, &structs);
        let (Some(left), Some(right)) = (left, right) else {
            continue;
        };
        let messages: Vec<Option<&str>> = match (left.as_str(), right.as_str()) {
            ("SA()", _) => vec![Some(_WRONG_TYPE), None],
            ("S", "N") => vec![Some(_WRONG_TYPE)],
            ("S", "SA()") => vec![None],
            ("N", "S") => vec![Some(_ILLEGAL)],
            ("N", "SA()") => vec![Some(_ILLEGAL), None],
            _ => continue,
        };
        let node = &assign.base.node;
        for m in messages {
            let msg = match m {
                Some(s) => s.to_string(),
                None => structure::part2::_out_of_place(
                    profile,
                    &assign.base.text,
                    assign.base.at_eol.get(),
                ),
            };
            diags.error(node.line, &msg, &node.file, ShowFile::Bool(true), 0, "", "");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profiles::get as get_profile;
    use crate::syntax::{Name, Node, StmtBase};

    #[test]
    fn test_names_and_operand_kind() {
        let mut structs = OrderedMap::new();
        let mut pt = OrderedMap::new();
        pt.insert("X".to_string(), "FLOAT".to_string());
        structs.insert("PT".to_string(), pt);

        let mut kinds = OrderedMap::new();
        kinds.insert("P".to_string(), "STRUCT:PT".to_string());
        kinds.insert("V".to_string(), "FLOAT".to_string());

        let p_expr = Expr::Name(Name {
            node: Node::default(),
            name: "p".to_string(),
        });
        let v_expr = Expr::Name(Name {
            node: Node::default(),
            name: "v".to_string(),
        });
        assert_eq!(
            _operand_kind(Some(&p_expr), &kinds, &structs).as_deref(),
            Some("S")
        );
        assert_eq!(
            _operand_kind(Some(&v_expr), &kinds, &structs).as_deref(),
            Some("N")
        );
    }
}
