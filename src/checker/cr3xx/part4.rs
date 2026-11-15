//! Port of `checker/cr3xx.py` (lines 2124-2364).

#![allow(dead_code, unused_variables, unused_imports)]

use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;
use std::sync::LazyLock;

use super::part1::_UNRESOLVED_SUFFIX;
use super::part2::{
    _GENERIC_FALLBACK_PREFIX, _leading_unknown_words, declared_later, first_declarations,
    prose_known,
};
use super::part3::{_IF_STRING_RE, _STRING_END_QUOTE, _string_reached};
use crate::checker::context;
use crate::checker::expressions::{_index_nodes, _stmt_exprs};
use crate::checker::structure::_unparsed_string_messages;
use crate::diagnostics::{DiagRef, Diagnostic, Diagnostics, ShowFile};
use crate::lexer;
use crate::profiles::Profile;
use crate::pyre;
use crate::pystr;
use crate::syntax::{
    Assignment, Call, Case, ConstDecl, Declaration, Expr, ExprRef, Index, Instruction, Juxtaposed,
    Member, Number, OrderedMap, Program, ProgramRef, Stmt, StmtRef, SubDef, Unparsed, VarDecl,
    ends_routine,
};

static _SUB_P4: LazyLock<pyre::Regex> =
    LazyLock::new(|| pyre::Regex::new(r#"\s*([A-Za-z_]\w*)\s*\([^()"]*\)\s+""#, 0));
static _DECL_P4: LazyLock<pyre::Regex> =
    LazyLock::new(|| pyre::Regex::new(r"(?i)\s*(?:Public|Dim)\b", 0));
static _CALL_PREFIX_P4: LazyLock<pyre::Regex> =
    LazyLock::new(|| pyre::Regex::new(r"\s*([A-Za-z_]\w*)\s*\(", 0));
static _FOR_CONST_RE_P4: LazyLock<pyre::Regex> =
    LazyLock::new(|| pyre::Regex::new(r"(?i)\s*(FOR|CONST)\b", 0));

fn make_error_diag_p4(line: i64, message: &str, file: &str) -> DiagRef {
    Rc::new(Diagnostic::new(
        "error",
        line,
        message,
        file,
        ShowFile::Bool(true),
        0,
        "",
        "",
    ))
}

/// Port of `checker/cr3xx.py::order_unterminated_string`.
pub fn order_unterminated_string(
    program: &ProgramRef,
    profile: &'static Profile,
    diags: &mut Diagnostics,
) {
    if profile.name != "cr300" && profile.name != "cr350" {
        return;
    }
    let declared = context::of(program, Some(profile), None)
        .declared_variable_names()
        .clone();
    let mut known: Option<BTreeSet<String>> = None;
    for stmt in &program.statements {
        let Stmt::Unparsed(u) = stmt.as_ref() else {
            continue;
        };
        if u.reason != "lex error: unterminated string" {
            continue;
        }
        let line = u.base.node.line;
        let file = u.base.node.file.clone();
        let text = &u.base.text;
        let own: Vec<DiagRef> = diags
            .items
            .iter()
            .filter(|d| (d.line, d.file.as_str()) == (line, file.as_str()))
            .cloned()
            .collect();
        let final_diag = own
            .iter()
            .find(|d| {
                d.message.starts_with(_GENERIC_FALLBACK_PREFIX)
                    || d.message.starts_with("Incomplete expression:")
                    || d.message.starts_with("No Matching, or out of place )")
            })
            .cloned();
        let Some(mut final_diag) = final_diag else {
            continue;
        };
        if !own.iter().all(|d| {
            d.message.ends_with(_UNRESOLVED_SUFFIX)
                || d.message == _STRING_END_QUOTE
                || Rc::ptr_eq(d, &final_diag)
        }) {
            continue;
        }
        let sub_match = _SUB_P4.match_(text);
        let sub_declared = sub_match
            .and_then(|m| m.group(1))
            .is_some_and(|g| declared.contains(&pystr::upper(g)));
        if sub_declared || _DECL_P4.match_(text).is_some() {
            continue;
        }
        let if_match = _IF_STRING_RE.match_(text);
        if if_match.is_none() && !_string_reached(text) {
            continue;
        }
        let mut names: Vec<DiagRef> = own
            .iter()
            .filter(|d| d.message.ends_with(_UNRESOLVED_SUFFIX))
            .cloned()
            .collect();
        if names.is_empty()
            && let Some(m) = if_match
        {
            if known.is_none() {
                known = Some(prose_known(program, profile));
            }
            let later = declared_later(&first_declarations(program), line, &file);
            let here: BTreeSet<String> = known
                .as_ref()
                .expect("known initialized")
                .difference(&later)
                .cloned()
                .collect();
            let cond = m.group(1).unwrap_or("");
            let target = m.group(2).unwrap_or("");
            let mut words = _leading_unknown_words(cond, &here);
            if !here.contains(&pystr::upper(target)) {
                words.push(target.to_string());
            }
            names = words
                .into_iter()
                .map(|w| {
                    let msg = profile.unresolved_symbol_message.format(&w);
                    make_error_diag_p4(line, &msg, &file)
                })
                .collect();
        }
        if final_diag.message.starts_with("No Matching") && _CALL_PREFIX_P4.match_(text).is_some() {
            let head = pystr::strip(pystr::lstrip(text).split('(').next().unwrap_or(""));
            if profile.functions().contains(&pystr::upper(head)) {
                let nl = if final_diag.message.ends_with("\n.") {
                    "\n"
                } else {
                    ""
                };
                let msg = format!("{_GENERIC_FALLBACK_PREFIX}{text}{nl}.");
                final_diag = Rc::new(final_diag.replace(|d| d.message = msg));
            }
        }
        let at = diags
            .items
            .iter()
            .position(|d| Rc::ptr_eq(d, &own[0]))
            .unwrap_or(diags.items.len());
        diags
            .items
            .retain(|d| (d.line, d.file.as_str()) != (line, file.as_str()));
        let mut replacement = names;
        replacement.push(make_error_diag_p4(line, _STRING_END_QUOTE, &file));
        replacement.push(final_diag);
        diags.items.splice(at..at, replacement);
    }
}

/// Port of `checker/cr3xx.py::semicolon_headers`.
pub fn semicolon_headers(program: &Program, profile: &Profile, diags: &mut Diagnostics) {
    if profile.name != "cr300" && profile.name != "cr350" {
        return;
    }
    for stmt in &program.statements {
        let Stmt::Unparsed(u) = stmt.as_ref() else {
            continue;
        };
        let text = &u.base.text;
        if !text.contains(';') {
            continue;
        }
        let Some(m) = _FOR_CONST_RE_P4.match_(text) else {
            continue;
        };
        let Ok(raw_toks) = lexer::tokenize(text, 0) else {
            continue;
        };
        let toks: Vec<_> = raw_toks.into_iter().filter(|t| t.kind != "EOF").collect();
        let Some(semi) = toks.iter().position(|t| t.kind == "OP" && t.text == ";") else {
            continue;
        };
        if semi < 2 {
            continue;
        }
        let kw = pystr::upper(m.group(1).unwrap_or(""));
        let echo: String = if kw == "CONST" {
            let Some(eq) = toks.iter().find(|t| t.kind == "OP" && t.text == "=") else {
                continue;
            };
            let slice_from = if toks[semi].col < eq.col {
                toks[semi].col
            } else {
                eq.col + 1
            };
            pystr::lstrip(pystr::slice(
                text,
                Some(i64::try_from(slice_from).unwrap_or(i64::MAX)),
                None,
            ))
            .to_string()
        } else {
            let mut start: Option<usize> = None;
            for t in &toks[..semi] {
                if t.kind == "OP" && t.text == "=" {
                    start = Some(t.col + 1);
                } else if t.kind == "IDENT" && (t.upper == "TO" || t.upper == "STEP") {
                    let after = t.col + t.text.chars().count();
                    let rest =
                        pystr::slice(text, Some(i64::try_from(after).unwrap_or(i64::MAX)), None);
                    let rest_len = rest.chars().count();
                    let stripped_len = pystr::lstrip(rest).chars().count();
                    start = Some(after + rest_len - stripped_len);
                }
            }
            let Some(start) = start else {
                continue;
            };
            let stop = toks[semi..]
                .iter()
                .find(|t| t.kind == "IDENT" && (t.upper == "TO" || t.upper == "STEP"));
            let end = match stop {
                None => text.chars().count(),
                Some(s) => {
                    let has_space = s.col > 0
                        && matches!(
                            pystr::char_at(text, i64::try_from(s.col - 1).unwrap_or(i64::MAX)),
                            Some(' ' | '\t')
                        );
                    s.col - usize::from(has_space)
                }
            };
            pystr::slice(
                text,
                Some(i64::try_from(start).unwrap_or(i64::MAX)),
                Some(i64::try_from(end).unwrap_or(i64::MAX)),
            )
            .to_string()
        };
        let line = u.base.node.line;
        let file = u.base.node.file.clone();
        let Some(at) = diags
            .items
            .iter()
            .position(|d| (d.line, d.file.as_str()) == (line, file.as_str()))
        else {
            continue;
        };
        diags
            .items
            .retain(|d| (d.line, d.file.as_str()) != (line, file.as_str()));
        diags.items.insert(
            at,
            make_error_diag_p4(
                line,
                &format!("Invalid symbol in expression: {echo}."),
                &file,
            ),
        );
    }
}

/// Port of `checker/cr3xx.py::add_string_illegal`.
pub fn add_string_illegal(
    program: &ProgramRef,
    profile: &'static Profile,
    diags: &mut Diagnostics,
) {
    if profile.name != "cr300" && profile.name != "cr350" {
        return;
    }
    for stmt in &program.statements {
        let Stmt::Unparsed(u) = stmt.as_ref() else {
            continue;
        };
        if u.reason == "lex error: unterminated string" {
            continue;
        }
        let line = u.base.node.line;
        let file = u.base.node.file.clone();
        let own: Vec<DiagRef> = diags
            .items
            .iter()
            .filter(|d| (d.line, d.file.as_str()) == (line, file.as_str()))
            .cloned()
            .collect();
        let has_semi = u.base.text.contains(';');
        let final_diag = own
            .iter()
            .find(|d| {
                d.message.starts_with(_GENERIC_FALLBACK_PREFIX)
                    || d.message.starts_with("Incomplete expression:")
                    || (has_semi && d.message.starts_with("Invalid symbol in expression:"))
            })
            .cloned();
        let Some(final_diag) = final_diag else {
            continue;
        };
        if own.iter().any(|d| d.message == "String is illegal here") {
            continue;
        }
        let messages = _unparsed_string_messages(u, program, profile);
        if !messages.is_empty() {
            let at = diags
                .items
                .iter()
                .position(|d| Rc::ptr_eq(d, &final_diag))
                .unwrap_or(diags.items.len());
            let new_diags: Vec<DiagRef> = messages
                .into_iter()
                .map(|m| make_error_diag_p4(line, &m, &file))
                .collect();
            diags.items.splice(at..at, new_diags);
        }
    }
    _order_arith_string_lines(diags);
}

pub const _ARITH_STRING_MESSAGES: [&str; 2] =
    ["Illegal String operand", "Illegal operand of type STRING"];

/// Port of `checker/cr3xx.py::_order_arith_string_lines`.
pub fn _order_arith_string_lines(diags: &mut Diagnostics) {
    let items = &mut diags.items;
    let is_string_msg =
        |msg: &str| _ARITH_STRING_MESSAGES.contains(&msg) || msg == "String is illegal here";
    let mut keys: Vec<(i64, String)> = Vec::new();
    for d in items.iter() {
        if is_string_msg(&d.message) {
            let k = (d.line, d.file.clone());
            if !keys.contains(&k) {
                keys.push(k);
            }
        }
    }
    for key in keys {
        let slots: Vec<usize> = items
            .iter()
            .enumerate()
            .filter(|(_, d)| (d.line, d.file.as_str()) == (key.0, key.1.as_str()))
            .map(|(i, _)| i)
            .collect();
        let own: Vec<DiagRef> = slots.iter().map(|&i| items[i].clone()).collect();
        let arith: Vec<DiagRef> = own
            .iter()
            .filter(|d| _ARITH_STRING_MESSAGES.contains(&d.message.as_str()))
            .cloned()
            .collect();
        let logical: Vec<DiagRef> = own
            .iter()
            .filter(|d| d.message == "String is illegal here")
            .cloned()
            .collect();
        let names: Vec<DiagRef> = own
            .iter()
            .filter(|d| d.message.ends_with(_UNRESOLVED_SUFFIX))
            .cloned()
            .collect();
        if names.is_empty() || (!arith.is_empty() && !logical.is_empty()) {
            continue;
        }
        let rest: Vec<DiagRef> = own
            .iter()
            .filter(|d| {
                !arith.iter().any(|x| Rc::ptr_eq(x, d))
                    && !logical.iter().any(|x| Rc::ptr_eq(x, d))
                    && !names.iter().any(|x| Rc::ptr_eq(x, d))
            })
            .cloned()
            .collect();
        let mut order: Vec<DiagRef> = Vec::with_capacity(own.len());
        order.extend(arith);
        order.extend(names);
        order.extend(logical);
        order.extend(rest);
        let changed = order.len() != own.len()
            || order.iter().zip(own.iter()).any(|(a, b)| !Rc::ptr_eq(a, b));
        if changed {
            for (i, d) in slots.into_iter().zip(order) {
                items[i] = d;
            }
        }
    }
}

pub const _TCPOPEN_HANDLE: &str = "Parameter must be a variable of type long";

/// Port of `checker/cr3xx.py::check_tcpopen_handle`.
pub fn check_tcpopen_handle(
    program: &ProgramRef,
    profile: &'static Profile,
    diags: &mut Diagnostics,
) {
    if profile.name != "cr300" && profile.name != "cr350" {
        return;
    }
    let ctx = context::of(program, Some(profile), None);
    let types = ctx.variable_types(false);
    let known = &ctx.known_names().names;
    let mut params: BTreeMap<String, Option<String>> = BTreeMap::new();
    for stmt in &program.statements {
        if let Stmt::SubDef(sd) = stmt.as_ref() {
            params.clear();
            for p in &sd.params {
                let ty = pystr::upper(p.type_.as_deref().unwrap_or("FLOAT"));
                let val = if p.by_ref.is_none()
                    && p.dims == 0
                    && matches!(ty.as_str(), "LONG" | "BOOLEAN" | "FLOAT" | "STRING")
                {
                    Some(ty)
                } else {
                    None
                };
                params.insert(pystr::upper(&p.name), val);
            }
            continue;
        }
        if ends_routine(stmt.as_ref()) {
            params.clear();
            continue;
        }
        let calls: Vec<Vec<Option<ExprRef>>> = match stmt.as_ref() {
            Stmt::Call(c) if pystr::upper(&c.name) == "TCPOPEN" => vec![c.args.clone()],
            _ => {
                let mut out = Vec::new();
                for e_opt in _stmt_exprs(stmt.as_ref()) {
                    for ix in _index_nodes(e_opt.as_deref()) {
                        if pystr::upper(&ix.name) == "TCPOPEN" {
                            out.push(ix.args.clone());
                        }
                    }
                }
                out
            }
        };
        let node = &stmt.base().node;
        for args in calls {
            if args.len() < 5 {
                continue;
            }
            let Some(arg) = args[4].as_ref() else {
                continue;
            };
            let own_dims = diags.items.iter().any(|d| {
                (d.line, d.file.as_str()) == (node.line, node.file.as_str())
                    && d.message.starts_with("Dimensions not declared")
            });
            let arg_is_unknown_index = matches!(
                arg.as_ref(),
                Expr::Index(ix) if !known.contains(&pystr::upper(&ix.name))
            );
            if own_dims && !arg_is_unknown_index {
                continue;
            }
            if let Some(arg3) = args[3].as_ref() {
                let arg3_unknown = match arg3.as_ref() {
                    Expr::Name(n) => !known.contains(&pystr::upper(&n.name)),
                    Expr::Index(ix) => !known.contains(&pystr::upper(&ix.name)),
                    _ => false,
                };
                if arg3_unknown {
                    continue;
                }
            }
            let bad = match arg.as_ref() {
                Expr::Name(n) => {
                    let up = pystr::upper(&n.name);
                    if let Some(p_ty) = params.get(&up) {
                        let Some(ty) = p_ty else {
                            continue;
                        };
                        ty != "LONG" && ty != "BOOLEAN"
                    } else if !known.contains(&up) {
                        true
                    } else if let Some(ty) = types.get(&up) {
                        ty != "LONG" && ty != "BOOLEAN"
                    } else {
                        continue;
                    }
                }
                Expr::Index(ix) => {
                    let up = pystr::upper(&ix.name);
                    if let Some(p_ty) = params.get(&up) {
                        let Some(ty) = p_ty else {
                            continue;
                        };
                        ty != "LONG" && ty != "BOOLEAN"
                    } else if !known.contains(&up) {
                        true
                    } else if let Some(ty) = types.get(&up) {
                        ty != "LONG" && ty != "BOOLEAN"
                    } else {
                        continue;
                    }
                }
                Expr::Number(_) | Expr::String(_) | Expr::Binary(_) | Expr::Unary(_) => true,
                _ => false,
            };
            if !bad {
                continue;
            }
            let own: Vec<usize> = diags
                .items
                .iter()
                .enumerate()
                .filter(|(_, d)| (d.line, d.file.as_str()) == (node.line, node.file.as_str()))
                .map(|(i, _)| i)
                .collect();
            if own
                .iter()
                .any(|&i| diags.items[i].message == _TCPOPEN_HANDLE)
            {
                continue;
            }
            let names: Vec<usize> = own
                .iter()
                .copied()
                .filter(|&i| diags.items[i].message.ends_with(_UNRESOLVED_SUFFIX))
                .collect();
            let at = if let Some(&last_name) = names.last() {
                last_name + 1
            } else if let Some(&first_own) = own.first() {
                first_own
            } else {
                diags.items.len()
            };
            diags.items.insert(
                at,
                make_error_diag_p4(node.line, _TCPOPEN_HANDLE, &node.file),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profiles;
    use crate::syntax::{Name, Node, Param, StmtBase, StringLit};

    fn cr300_profile() -> &'static Profile {
        profiles::get("cr300").unwrap()
    }

    fn cr6_profile() -> &'static Profile {
        profiles::get("cr6").unwrap()
    }

    fn unparsed_stmt(line: i64, text: &str, reason: &str) -> StmtRef {
        Rc::new(Stmt::Unparsed(Unparsed {
            base: StmtBase {
                node: Node {
                    line,
                    file: "test.cr300".to_string(),
                },
                text: text.to_string(),
                ..StmtBase::default()
            },
            reason: reason.to_string(),
            ..Unparsed::default()
        }))
    }

    #[test]
    fn test_order_arith_string_lines_reorders_arith_and_logical() {
        let mut diags = Diagnostics::new();
        diags
            .items
            .push(make_error_diag_p4(10, "Foo is not defined.", "test.cr300"));
        diags.items.push(make_error_diag_p4(
            10,
            "Illegal String operand",
            "test.cr300",
        ));
        diags.items.push(make_error_diag_p4(
            20,
            "String is illegal here",
            "test.cr300",
        ));
        diags
            .items
            .push(make_error_diag_p4(20, "Bar is not defined.", "test.cr300"));

        _order_arith_string_lines(&mut diags);

        let msgs: Vec<&str> = diags.items.iter().map(|d| d.message.as_str()).collect();
        assert_eq!(
            msgs,
            vec![
                "Illegal String operand",
                "Foo is not defined.",
                "Bar is not defined.",
                "String is illegal here",
            ]
        );
    }

    #[test]
    fn test_semicolon_headers_for_and_const() {
        let mut program = Program::default();
        program
            .statements
            .push(unparsed_stmt(3, "For Y = 1 To 3 Step 1 ;", "parse error"));
        program
            .statements
            .push(unparsed_stmt(4, "Const K = ; 1", "parse error"));
        let mut diags = Diagnostics::new();
        diags.items.push(make_error_diag_p4(
            3,
            "Invalid, or out of place expression: For Y = 1 To 3 Step 1 ;.",
            "test.cr300",
        ));
        diags.items.push(make_error_diag_p4(
            4,
            "Invalid, or out of place expression: Const K = ; 1.",
            "test.cr300",
        ));

        semicolon_headers(&program, cr300_profile(), &mut diags);

        assert_eq!(diags.items.len(), 2);
        assert_eq!(diags.items[0].message, "Invalid symbol in expression: 1 ;.");
        assert_eq!(diags.items[1].message, "Invalid symbol in expression: ; 1.");
    }

    #[test]
    fn test_add_string_illegal_inserts_before_final() {
        let mut program = Program::default();
        program
            .statements
            .push(unparsed_stmt(5, "x = \"a\" and \"b\"", "parse error"));
        let mut diags = Diagnostics::new();
        diags.items.push(make_error_diag_p4(
            5,
            "Invalid, or out of place expression: x = \"a\" and \"b\".",
            "test.cr300",
        ));

        add_string_illegal(&Rc::new(program), cr300_profile(), &mut diags);

        let msgs: Vec<&str> = diags.items.iter().map(|d| d.message.as_str()).collect();
        assert_eq!(
            msgs,
            vec![
                "String is illegal here",
                "Invalid, or out of place expression: x = \"a\" and \"b\".",
            ]
        );
    }

    #[test]
    fn test_order_unterminated_string_reorders_and_skips_cr6() {
        let mut program = Program::default();
        program.statements.push(unparsed_stmt(
            2,
            "Foo \"abc",
            "lex error: unterminated string",
        ));
        let mut diags = Diagnostics::new();
        diags.items.push(make_error_diag_p4(
            2,
            "Invalid, or out of place expression: Foo \"abc.",
            "test.cr300",
        ));
        diags
            .items
            .push(make_error_diag_p4(2, "Foo is not defined.", "test.cr300"));

        let program = Rc::new(program);
        // Skips CR6
        order_unterminated_string(&program, cr6_profile(), &mut diags);
        assert_eq!(diags.items.len(), 2);

        // Reorders on CR300
        order_unterminated_string(&program, cr300_profile(), &mut diags);
        let msgs: Vec<&str> = diags.items.iter().map(|d| d.message.as_str()).collect();
        assert_eq!(
            msgs,
            vec![
                "Foo is not defined.",
                "String without end quote",
                "Invalid, or out of place expression: Foo \"abc.",
            ]
        );
    }
}
