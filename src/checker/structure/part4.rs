//! Port of `checker/structure.py` (lines 2082-2868).

#![allow(dead_code, unused_variables, unused_imports)]

use std::collections::BTreeSet;
use std::rc::Rc;
use std::sync::LazyLock;

use super::part1::_ends_sub;
use super::part2::{_block_word, _keyword};
use crate::checker::expressions::part1::KnownView;
use crate::checker::{context, expressions, instr_data};
use crate::diagnostics::{DiagRef, Diagnostic, Diagnostics};
use crate::profiles::Profile;
use crate::pyre::{self, Regex};
use crate::syntax::{
    Assignment, Call, Case, ConstDecl, Declaration, Expr, ExprRef, Index, Instruction, Juxtaposed,
    Member, Number, OrderedMap, Program, ProgramRef, Stmt, StmtRef, SubDef, Unparsed, VarDecl,
};
use crate::{lexer, pystr};

/// Port of `checker/structure.py::check_end_blocks`.
pub fn check_end_blocks(program: &Program, diags: &mut Diagnostics) {
    let last_line = if program.last_line != 0 {
        program.last_line
    } else {
        program.statements.last().map_or(1, |st| st.line())
    };
    let mut sub_open = false;
    for st in &program.statements {
        if matches!(st.as_ref(), Stmt::SubDef(_)) {
            sub_open = true;
        } else if _ends_sub(st, &_keyword(st)) {
            sub_open = false;
        }
    }
    if !program
        .statements
        .iter()
        .any(|st| _keyword(st) == "BEGINPROG")
    {
        diags.error_basic(last_line, "No BeginProg in Program.", "");
        if sub_open {
            diags.error_basic(last_line, "Sub ... EndSub not matched.", "");
        }
        return;
    }
    let mut depth = OrderedMap::new();
    for kind in ["SCAN", "FOR", "DO", "IF"] {
        depth.insert(kind, 0_i64);
    }
    let mut in_table = false;
    let mut table_name = None;
    for stmt in &program.statements {
        let keyword = _keyword(stmt);
        let kw = match keyword.as_str() {
            "WHILE" => "DO",
            "WEND" => "LOOP",
            kw => kw,
        };
        // Integration: part2::_block_word still omits its Python `args` parameter.
        let word = if matches!(kw, "END" | "NEXT") {
            _block_word()
        } else {
            None
        };
        let word = word.as_deref();
        let change = if kw == "SCAN" {
            Some(("SCAN", 1))
        } else if kw == "NEXTSCAN" || (kw == "NEXT" && word == Some("SCAN")) {
            Some(("SCAN", -1))
        } else if kw == "FOR" {
            Some(("FOR", 1))
        } else if kw == "NEXT" && !matches!(word, Some("SCAN" | "SUBSCAN")) {
            Some(("FOR", -1))
        } else if kw == "DO" {
            Some(("DO", 1))
        } else if kw == "LOOP" {
            Some(("DO", -1))
        } else if kw == "SELECT"
            || (kw == "IF" && !matches!(stmt.as_ref(), Stmt::If(st) if st.inline.is_some()))
        {
            Some(("IF", 1))
        } else if matches!(kw, "ENDSELECT" | "ENDIF")
            || (kw == "END" && matches!(word, Some("SELECT" | "IF")))
        {
            Some(("IF", -1))
        } else {
            if kw == "DATATABLE" && !in_table {
                in_table = true;
                table_name = match stmt.as_ref() {
                    Stmt::Instruction(st) => {
                        st.args
                            .first()
                            .and_then(Option::as_deref)
                            .and_then(|a| match a {
                                Expr::Name(n) => Some(n.name.clone()),
                                _ => None,
                            })
                    }
                    _ => None,
                };
            } else if kw == "ENDTABLE" {
                in_table = false;
            }
            None
        };
        if let Some((kind, delta)) = change {
            let value = *depth.get(&kind).expect("block counter");
            depth.insert(kind, (value + delta).max(0));
        }
    }
    let mut reported = false;
    for (kind, message) in [
        ("SCAN", "Scan without NextScan."),
        ("FOR", "FOR without NEXT."),
        ("DO", "DO without LOOP."),
        ("IF", "IF or SELECT without EndIf or EndSelect."),
    ] {
        if depth.get(&kind).is_some_and(|n| *n > 0) {
            diags.error_basic(last_line, message, "");
            reported = true;
        }
    }
    if sub_open {
        diags.error_basic(last_line, "Sub ... EndSub not matched.", "");
        return;
    }
    if reported {
        return;
    }
    let mut menus = 0_i64;
    for stmt in &program.statements {
        if let Stmt::Instruction(st) = stmt.as_ref() {
            match pystr::upper(&st.keyword).as_str() {
                "DISPLAYMENU" => menus += 1,
                "ENDMENU" => menus = (menus - 1).max(0),
                _ => (),
            }
        }
    }
    if menus != 0 {
        diags.error_basic(last_line, "DisplayMenu without EndMenu.", "");
        return;
    }
    if in_table
        && let Some(name) = table_name
        && !instr_data::calltable_targets(program).contains(&pystr::upper(&name))
    {
        diags.error_basic(0, "DataTable ... EndTable not matched.", "");
    }
}

/// Port of `checker/structure.py::CallLinks`.
#[derive(Debug, Default)]
pub struct CallLinks {
    pub calls: Vec<String>,
    pub registered: BTreeSet<String>,
    pub first: Option<(i64, String, String)>,
    pub positions: OrderedMap<(String, i64), i64>,
    pub cutoff: Option<(String, i64)>,
}

/// Port of `checker/structure.py::nested_table`.
pub fn nested_table(_program: &Program) -> Option<StmtRef> {
    todo!("checker/structure.py::nested_table (omitted: profile: untyped)")
}

/// Port of `checker/structure.py::nested_table`.
pub fn nested_table_with(program: &ProgramRef, profile: &'static Profile) -> Option<StmtRef> {
    let mut in_table = false;
    let mut closed = false;
    let mut nested = None;
    let mut open_table: Vec<&Instruction> = Vec::new();
    let mut known = None;
    let mut unresolved = false;
    for stmt in &program.statements {
        let kw = _keyword(stmt);
        if kw == "BEGINPROG" {
            break;
        }
        if kw == "DATATABLE" && !in_table {
            in_table = true;
            open_table.clear();
        } else if kw == "DATATABLE" && nested.is_none() {
            nested = Some(stmt.clone());
            let ctx = context::of(program, Some(profile), None);
            known = Some(ctx.known_names().clone());
            unresolved |= open_table.iter().any(|st| {
                let names = known.as_ref().expect("known names");
                _names_unresolved_with(
                    &KnownView {
                        names: &names.names,
                        tables: &names.tables,
                        out_of_scope: &names.out_of_scope,
                        dotted_tables: names.dotted_tables,
                    },
                    st,
                    true,
                )
            });
        } else if kw == "ENDTABLE" {
            in_table = false;
            closed |= nested.is_some();
        } else if in_table && let Stmt::Instruction(st) = stmt.as_ref() {
            if nested.is_none() {
                open_table.push(st);
            } else if !closed && let Some(known) = &known {
                unresolved |= _names_unresolved_with(
                    &KnownView {
                        names: &known.names,
                        tables: &known.tables,
                        out_of_scope: &known.out_of_scope,
                        dotted_tables: known.dotted_tables,
                    },
                    st,
                    false,
                );
            }
        }
    }
    if unresolved { None } else { nested }
}

/// Port of `checker/structure.py::check_pass2`.
pub fn check_pass2(_program: &Program, _diags: &mut Diagnostics, _links: Option<&CallLinks>) {
    // Resume Python line 2270 after dependencies have typed return values and LinkContext fields.
    // In particular first_misplaced_sub currently returns (), not the optional placement finding.
    todo!(
        "checker/structure.py::check_pass2 (omitted: profile: untyped, env: consteval.Env | None)"
    )
}

/// Port of `checker/structure.py::_CLOSER_MESSAGES`.
pub static _CLOSER_MESSAGES: LazyLock<OrderedMap<&'static str, (&'static str, &'static str)>> =
    LazyLock::new(|| {
        let mut messages = OrderedMap::new();
        messages.insert(
            "IF",
            (
                "IF or SELECT statement illegally has SCAN without NEXT SCAN.",
                "END IF without IF or END SELECT without SELECT.",
            ),
        );
        messages.insert(
            "FOR",
            (
                "FOR NEXT statement illegally has SCAN without NEXT SCAN.",
                "NEXT without FOR.",
            ),
        );
        messages.insert(
            "DO",
            (
                "LOOP statement illegally has SCAN without NEXT SCAN.",
                "LOOP without DO.",
            ),
        );
        messages
    });

/// Port of `checker/structure.py::_names_unresolved`.
pub fn _names_unresolved(known: &BTreeSet<String>, stmt: &Instruction, sources: bool) -> bool {
    _names_unresolved_with(&KnownView::from_set(known), stmt, sources)
}

/// Port of `checker/structure.py::_names_unresolved`.
pub fn _names_unresolved_with(known: &KnownView<'_>, stmt: &Instruction, sources: bool) -> bool {
    // instr_data::part2::_TABLE_OUTPUT is private; keep its exact membership here until exposed.
    let skip = !sources
        && matches!(
            pystr::upper(&stmt.keyword).as_str(),
            "TOTALIZE"
                | "AVERAGE"
                | "MAXIMUM"
                | "MINIMUM"
                | "SAMPLEMAXMIN"
                | "STDDEV"
                | "MOMENT"
                | "SAMPLE"
                | "COVARIANCE"
                | "MEDIAN"
        );
    stmt.args.iter().enumerate().any(|(i, arg)| {
        arg.is_some()
            && !(skip && i == 1)
            && !expressions::_undefined_names_with(arg.as_deref(), known).is_empty()
    })
}

/// Port of `checker/structure.py::_unparsed_opener`.
pub fn _unparsed_opener(text: &str) -> Option<String> {
    static IF: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s*IF\b", pyre::I));
    static THEN_BODY: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\bTHEN\s*\S", pyre::I));
    static SELECT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s*SELECT\s+CASE\b", pyre::I));
    static DO: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"\s*(?:DO\s+(?:WHILE|UNTIL)|WHILE)\b", pyre::I));

    if IF.match_(text).is_some() && THEN_BODY.search(text).is_none() {
        Some("IF".to_owned())
    } else if SELECT.match_(text).is_some() {
        Some("SELECT".to_owned())
    } else {
        DO.match_(text).map(|_| "DO".to_owned())
    }
}

/// Port of `checker/structure.py::check_block_closers`.
pub fn check_block_closers(_program: &Program, _diags: &mut Diagnostics) {
    todo!("checker/structure.py::check_block_closers (omitted: profile: untyped)")
}

/// Port of `checker/structure.py::check_block_closers`.
pub fn check_block_closers_with(program: &Program, profile: &Profile, diags: &mut Diagnostics) {
    /// Port of `checker/structure.py::check_block_closers.push`.
    fn push(frames: &mut Vec<String>, depth: &mut usize, kind: &str) {
        if *depth == frames.len() {
            frames.push(kind.to_owned());
        } else if kind == "SCAN" {
            kind.clone_into(&mut frames[*depth]);
        }
        *depth += 1;
    }

    let mut frames: Vec<String> = Vec::new();
    let mut depth = 0;
    let mut open_count = OrderedMap::new();
    for kind in ["IF", "FOR", "DO", "SCAN"] {
        open_count.insert(kind, 0_i64);
    }
    let mut selects = 0_i64;
    let mut subscans = 0_i64;

    for stmt in &program.statements {
        let keyword = _keyword(stmt);
        let kw = match keyword.as_str() {
            "WHILE" => "DO",
            "WEND" => "LOOP",
            kw => kw,
        };
        // Integration: part2::_block_word needs the instruction's args parameter.
        let word = if matches!(kw, "END" | "NEXT") {
            _block_word()
        } else {
            None
        };
        let word = word.as_deref();
        if let Stmt::If(st) = stmt.as_ref()
            && st.inline.is_some()
        {
            if (depth < frames.len() && frames[depth] == "SCAN")
                || st
                    .inline
                    .iter()
                    .flatten()
                    .chain(st.inline_else.iter().flatten())
                    .any(|b| _keyword(b) == "SCAN")
            {
                diags.error_basic(
                    stmt.line(),
                    _CLOSER_MESSAGES.get(&"IF").expect("IF messages").0,
                    stmt.file(),
                );
            }
        } else if kw == "IF" || kw == "SELECT" {
            push(&mut frames, &mut depth, "IF");
            open_count.insert("IF", open_count.get(&"IF").expect("IF counter") + 1);
            selects += i64::from(kw == "SELECT");
        } else if matches!(kw, "FOR" | "DO") {
            push(&mut frames, &mut depth, kw);
            let kind = if kw == "FOR" { "FOR" } else { "DO" };
            open_count.insert(kind, open_count.get(&kind).expect("block counter") + 1);
        } else if let Stmt::Unparsed(st) = stmt.as_ref()
            && let Some(opener) = _unparsed_opener(&st.base.text)
        {
            selects += i64::from(opener == "SELECT");
            let opener = if opener == "DO" { "DO" } else { "IF" };
            push(&mut frames, &mut depth, opener);
            open_count.insert(opener, open_count.get(&opener).expect("opener counter") + 1);
        } else if matches!(kw, "SCAN" | "SUBSCAN") {
            push(&mut frames, &mut depth, "SCAN");
            subscans += i64::from(kw == "SUBSCAN");
        } else if kw == "NEXTSUBSCAN" && subscans == 0 {
            diags.error_basic(stmt.line(), "Next SubScan without SubScan.", stmt.file());
        } else if matches!(kw, "NEXTSCAN" | "NEXTSUBSCAN")
            || (kw == "NEXT" && matches!(word, Some("SCAN" | "SUBSCAN")))
        {
            subscans -= i64::from(kw == "NEXTSUBSCAN" || word == Some("SUBSCAN"));
            if (kw == "NEXTSCAN" || word == Some("SCAN")) && subscans > 0 {
                diags.error_basic(stmt.line(), "SubScan without Next SubScan.", stmt.file());
                subscans = 0;
            }
            if let Some(index) = frames.iter().take(depth).rposition(|kind| kind == "SCAN") {
                frames.remove(index);
                depth -= 1;
            }
        } else if matches!(kw, "ENDIF" | "ENDSELECT" | "NEXT" | "LOOP")
            || (kw == "END" && matches!(word, Some("IF" | "SELECT")))
        {
            let is_select = kw == "ENDSELECT" || (kw == "END" && word == Some("SELECT"));
            if is_select && selects == 0 && profile.single_pass {
                return;
            }
            let kind = match kw {
                "NEXT" => "FOR",
                "LOOP" => "DO",
                _ => "IF",
            };
            let (illegal_scan, without) = _CLOSER_MESSAGES.get(&kind).expect("closer messages");
            if depth > 0 {
                depth -= 1;
                if frames[depth] == "SCAN" {
                    diags.error_basic(stmt.line(), illegal_scan, stmt.file());
                }
            }
            let count = *open_count.get(&kind).expect("closer counter");
            if count == 0 {
                diags.error_basic(stmt.line(), without, stmt.file());
            } else {
                open_count.insert(kind, count - 1);
                selects -= i64::from(is_select);
            }
        }
    }
}

/// Port of `checker/structure.py::_undefined_table_source`.
pub fn _undefined_table_source(_stmt: &Instruction, _known: &BTreeSet<String>) -> Option<String> {
    todo!("checker/structure.py::_undefined_table_source (omitted: profile: untyped)")
}

/// Port of `checker/structure.py::_undefined_table_source`.
pub fn _undefined_table_source_with(
    stmt: &Instruction,
    profile: &Profile,
    known: &KnownView<'_>,
) -> Option<String> {
    let record = profile.instructions().get(&pystr::upper(&stmt.keyword))?;
    let kinds = expressions::_type_code_kinds();
    for (i, code) in record.params.iter().take(stmt.args.len()).enumerate() {
        let arg = &stmt.args[i];
        let key = usize::try_from(if *code >= 100 { code % 100 } else { *code }).ok();
        if key.and_then(|k| kinds.get(&k)).map(String::as_str) != Some("deferred_text")
            || arg.is_none()
        {
            continue;
        }
        if !expressions::_undefined_names_with(arg.as_deref(), known).is_empty() {
            return Some(
                pystr::strip(&expressions::_arg_text(
                    stmt,
                    i64::try_from(i).expect("argument index"),
                ))
                .to_owned(),
            );
        }
    }
    None
}

/// Port of `checker/structure.py::_leftover_first_name_known`.
pub fn _leftover_first_name_known(text: &str, known: &BTreeSet<String>) -> bool {
    let Ok(tokens) = lexer::tokenize(text, 0) else {
        return false;
    };
    tokens
        .iter()
        .find(|token| token.kind == "IDENT")
        .is_none_or(|token| known.contains(&token.upper))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syntax::{Name, Node, NumberValue};

    #[test]
    fn unparsed_opener_matches_python_probes() {
        for (text, expected) in [
            ("If X Then", Some("IF")),
            ("If X Then Y=1", None),
            ("Select Case X", Some("SELECT")),
            ("Do While X", Some("DO")),
            ("While X", Some("DO")),
            ("WhileX", None),
            ("If Æ Then", Some("IF")),
        ] {
            assert_eq!(_unparsed_opener(text).as_deref(), expected, "{text}");
        }
    }

    #[test]
    fn leftover_resolution_only_checks_first_identifier() {
        let known = BTreeSet::from(["X".to_owned()]);
        for (text, expected) in [
            ("(123, \"abc\", X, Y)", true),
            ("(123, \"abc\", Y, X)", false),
            (",200,15000,1.0,0)", true),
            ("\"unclosed", false),
            ("(\"Æ\",X)", true),
        ] {
            assert_eq!(_leftover_first_name_known(text, &known), expected, "{text}");
        }
    }

    #[test]
    fn deferred_source_is_skipped_only_when_requested() {
        let name = |name: &str| {
            Some(Rc::new(Expr::Name(Name {
                node: Node::default(),
                name: name.to_owned(),
            })))
        };
        let stmt = Instruction {
            keyword: "Sample".to_owned(),
            args: vec![
                Some(Rc::new(Expr::Number(Number {
                    node: Node::default(),
                    value: NumberValue::Int(1),
                    text: "1".to_owned(),
                }))),
                name("Missing"),
                name("FP2"),
            ],
            ..Default::default()
        };
        let known = BTreeSet::from(["FP2".to_owned()]);
        assert!(_names_unresolved(&known, &stmt, true));
        assert!(!_names_unresolved(&known, &stmt, false));
        assert_eq!(
            _undefined_table_source_with(
                &stmt,
                crate::profiles::get("cr6").unwrap(),
                &KnownView::from_set(&known),
            ),
            Some("Missing".to_owned())
        );
    }
}
