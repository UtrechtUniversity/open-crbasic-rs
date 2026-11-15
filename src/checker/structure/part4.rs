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

/// Port of checker/structure.py::check_end_blocks
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
        let kw = super::part3::_DO_ALIASES
            .get(keyword.as_str())
            .copied()
            .unwrap_or(keyword.as_str());
        let word = if matches!(kw, "END" | "NEXT") {
            _block_word(stmt.args())
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

/// Port of checker/structure.py::CallLinks
#[derive(Debug, Default)]
pub struct CallLinks {
    pub calls: Vec<String>,
    pub registered: BTreeSet<String>,
    pub first: Option<(i64, String, String)>,
    pub positions: OrderedMap<(String, i64), i64>,
    pub cutoff: Option<(String, i64)>,
}

/// Port of checker/structure.py::nested_table
pub fn nested_table(program: &ProgramRef, profile: &'static Profile) -> Option<StmtRef> {
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
                _names_unresolved(
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
                unresolved |= _names_unresolved(
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

/// Port of checker/structure.py::check_pass2
#[allow(
    clippy::too_many_lines,
    clippy::doc_markdown,
    clippy::items_after_statements
)]
pub fn check_pass2(
    program: &ProgramRef,
    profile: &'static Profile,
    diags: &mut Diagnostics,
    env: Option<crate::consteval::Env>,
    links: Option<&mut CallLinks>,
) -> Option<i64> {
    use super::part1::{
        _ALIAS_NOTHING_AFTER_EQ, _ALIAS_WITHOUT_EQ, _BENIGN_REASONS, _BODY_BLOCK_ENDS,
        _BODY_BLOCK_STARTS, _CONST_BAD_NAME, _CONST_BAD_VALUE, _CONST_LEAD, _CONST_WITHOUT_EQ,
        _DECL_BLOCK_BOUNDS, _DO_LOOP_PARAM, _EXTRA_CLOSE_PAREN, _FOR_MISSING,
        _FORBIDDEN_BEFORE_BEGINPROG, _INVALID_AS_CLAUSE, _KNOWN_PARSER_GAP_REASONS,
        _UNCLOSED_DIM_PREFIX, _UNCLOSED_PAREN_EXPR, _UNITS_WITHOUT_EQ, _accept_known,
        _assignment_head, _calltable_known, _check_accept_table, _check_calltable_outside_scan,
        _scan_counts_for_table_placement, _track_tables, NESTED_SCAN, TableState,
        first_misplaced_data_table, first_misplaced_sub,
    };
    use super::part2::{
        _FALLBACK_PREFIX, _before_beginprog_name, _block_word, _control_keyword, _has_error_on,
        _keyword, _leftover_message, _no_matching, _out_of_place, _table_names,
    };
    use super::part3::_slowsequence_resolves;
    use super::part5::{
        _inline_condition_stmt, _known_first_token, _logical_assign_accepted,
        _logical_assign_undefined_messages, _logical_led_string, _pass1_scan, _unclosed_paren_kind,
        _unparsed_string_messages, condition_part, is_hash_statement, pass1_operands,
    };
    use super::part6::{
        _function_symbol_finding, _function_syntax_findings, _instruction_arg_findings,
        _pass1_message, _routine_arg_findings, _unclosed_call_message,
    };
    use super::part7::{
        _bare_sub_link_name, _check_assignment_pass2, _check_call_pass2, _unlinked_call,
        LinkContext,
    };
    use super::part9::_junk_init_findings;
    use crate::checker::{instr_rules, var_types};
    use crate::diagnostics::ShowFile;
    use std::cell::RefCell;

    static SLOW: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s*SlowSequence\b", pyre::I));
    static TARGET: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s*([A-Za-z_]\w*)\s*=", 0));
    static WORD_SPLIT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\W", 0));
    static LEAD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s*([A-Za-z_]\w*)\.", 0));
    static HEAD: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"\s*(IF|WHILE|DO|SELECT)\b", pyre::I));

    let env = env.unwrap_or_else(|| {
        crate::consteval::Env::with_consts(profile, Rc::new(RefCell::new(program.consts.clone())))
    });
    let ctx = context::of(program, Some(profile), Some(env.clone()));
    let subs = ctx.declared_subs();
    let pass1_count = diags.items.len();
    let mut sub_params = OrderedMap::new();
    let mut link_ctx = LinkContext::default();
    for st in &program.statements {
        if let Stmt::SubDef(sub) = st.as_ref() {
            sub_params.insert(pystr::upper(&sub.name), sub.clone());
            if pystr::upper(&sub.keyword) == "SUB" {
                link_ctx.subs.insert(pystr::upper(&sub.name));
            }
        }
    }
    link_ctx.has_routines = !subs.is_empty();
    let mut links = links;
    if let Some(links) = links.as_deref_mut() {
        *link_ctx.links.borrow_mut() = std::mem::take(links);
    }
    let known = ctx.known_names();
    let dims_ds = ctx.dims_state();
    let result = (|| {
        let mut seen_beginprog = false;
        let mut in_body_block = false;
        let mut after_endseq = false;
        let mut in_table = false;
        let mut in_decl_block = false;
        let mut tables = TableState::default();
        let mut in_beginprog = false;
        let mut program_marker_seen = false;
        let declared_here = if matches!(profile.name, "cr300" | "cr350") {
            ctx.declared_names()
                .iter()
                .map(|n| pystr::upper(n))
                .collect()
        } else {
            BTreeSet::new()
        };
        let mut pass2_scans = Vec::new();
        let mut pipeline_setup = false;
        let misplaced = if profile.single_pass {
            None
        } else {
            first_misplaced_sub(program)
        };
        let misplaced_table = if profile.single_pass {
            None
        } else {
            first_misplaced_data_table(program, profile)
        };
        let nested = if profile.single_pass {
            None
        } else {
            nested_table(program, profile)
        };
        if link_ctx.links.borrow().positions.is_empty() {
            for (position, st) in program.statements.iter().enumerate() {
                let key = (st.file().to_owned(), st.line());
                if !link_ctx.links.borrow().positions.contains_key(&key) {
                    link_ctx.links.borrow_mut().positions.insert(
                        key,
                        i64::try_from(position).expect("statement index fits i64"),
                    );
                }
            }
        }
        for stmt in &program.statements {
            link_ctx.links.borrow_mut().cutoff = Some((stmt.file().to_owned(), stmt.line()));
            seen_beginprog |= _keyword(stmt) == "BEGINPROG";
            if matches!(_keyword(stmt).as_str(), "PIPELINEMODE" | "SEQUENTIALMODE") {
                pipeline_setup = _keyword(stmt) == "PIPELINEMODE";
            }
            let before_bp = !seen_beginprog && !profile.single_pass;
            if let Some((st, messages)) = &misplaced
                && Rc::ptr_eq(stmt, st)
            {
                for message in messages {
                    diags.error_basic(stmt.line(), message, stmt.file());
                }
                return Some(stmt.line());
            }
            if misplaced_table
                .as_ref()
                .is_some_and(|st| Rc::ptr_eq(stmt, st))
            {
                diags.error_basic(
                    stmt.line(),
                    "DataTable is declared in the middle of an executing sequence",
                    stmt.file(),
                );
                return Some(stmt.line());
            }
            if nested.as_ref().is_some_and(|st| Rc::ptr_eq(stmt, st)) {
                diags.error_basic(stmt.line(), "DataTable without EndTable.", "");
                return Some(stmt.line());
            }
            if let Stmt::SubDef(sub) = stmt.as_ref() {
                in_body_block = true;
                if pystr::upper(&sub.keyword) == "SUB" {
                    link_ctx
                        .links
                        .borrow_mut()
                        .registered
                        .insert(pystr::upper(&sub.name));
                }
                continue;
            }
            let kw = _keyword(stmt);
            if let Some(bound) = _DECL_BLOCK_BOUNDS.get(&kw.as_str()) {
                in_decl_block = *bound;
            }
            _track_tables(stmt, &kw, &mut tables);
            if kw == "DATATABLE" {
                in_table = true;
            } else if kw == "ENDTABLE" {
                in_table = false;
            } else if in_table
                && matches!(stmt.as_ref(), Stmt::Instruction(_))
                && matches!(profile.name, "cr6" | "cr1000x")
            {
                if let Stmt::Instruction(st) = stmt.as_ref()
                    && let Some(bad) = _undefined_table_source(
                        st,
                        profile,
                        &KnownView {
                            names: &known.names,
                            tables: &known.tables,
                            out_of_scope: &known.out_of_scope,
                            dotted_tables: known.dotted_tables,
                        },
                    )
                {
                    diags.error(
                        stmt.line(),
                        &format!("Invalid symbol in expression: {bad}."),
                        stmt.file(),
                        ShowFile::Bool(false),
                        0,
                        "",
                        "",
                    );
                    return Some(stmt.line());
                }
            } else if kw == "BEGINPROG" {
                if in_beginprog {
                    diags.error_basic(stmt.line(), "Multiple BeginProgs.", stmt.file());
                }
                in_beginprog = true;
            } else if kw == "ENDPROG" {
                in_beginprog = false;
            }
            let ss_resolves = _slowsequence_resolves(
                stmt,
                &declared_here,
                &profile
                    .functions()
                    .iter()
                    .map(|(name, _)| name.to_owned())
                    .collect(),
            );
            if kw == "SLOWSEQUENCE"
                && !program_marker_seen
                && (matches!(profile.name, "cr6" | "cr1000x") || ss_resolves)
                && !diags.items.iter().any(|d| {
                    d.severity == "error" && d.line == stmt.line() && d.file == stmt.file()
                })
            {
                in_beginprog = true;
            }
            if matches!(kw.as_str(), "BEGINPROG" | "ENDPROG" | "SLOWSEQUENCE")
                || (matches!(stmt.as_ref(), Stmt::Unparsed(_))
                    && SLOW.match_(stmt.text()).is_some())
            {
                program_marker_seen = true;
            }
            if kw == "SLOWSEQUENCE" && !pass2_scans.is_empty() && !profile.single_pass {
                diags.error_basic(stmt.line(), "Missing NextScan.", stmt.file());
                return Some(stmt.line());
            }
            if kw == "SCAN" {
                if !pass2_scans.is_empty() && !profile.single_pass {
                    diags.error(
                        stmt.line(),
                        NESTED_SCAN,
                        stmt.file(),
                        ShowFile::Bool(true),
                        0,
                        "nested_scan",
                        "early_scan",
                    );
                }
                pass2_scans.push(_scan_counts_for_table_placement(stmt, Some(&env)));
            } else if kw == "NEXTSCAN"
                || (kw == "NEXT"
                    && _block_word(match stmt.as_ref() {
                        Stmt::Instruction(st) => &st.args,
                        _ => &[],
                    })
                    .as_deref()
                        == Some("SCAN"))
            {
                if !pass2_scans.is_empty() {
                    pass2_scans.pop();
                } else if !profile.single_pass {
                    diags.error_basic(stmt.line(), "NextScan with no Scan.", stmt.file());
                    return Some(stmt.line());
                }
            } else if kw == "CALLTABLE"
                && !profile.single_pass
                && !in_table
                && (!pass2_scans.iter().any(|x| *x) || in_body_block)
                && _calltable_known(stmt, &tables)
            {
                if let Stmt::Instruction(st) = stmt.as_ref() {
                    _check_calltable_outside_scan(
                        st,
                        program,
                        diags,
                        in_body_block,
                        !pass2_scans.is_empty() && !in_body_block,
                        Some(&env),
                    );
                }
            } else if kw == "ACCEPTDATARECORDS"
                && !profile.single_pass
                && !in_table
                && _accept_known(stmt, &tables)
                && let Stmt::Instruction(st) = stmt.as_ref()
            {
                _check_accept_table(
                    st,
                    program,
                    diags,
                    !pass2_scans.iter().any(|x| *x) || in_body_block,
                    &env,
                );
            }
            if _BODY_BLOCK_STARTS.contains(&kw.as_str()) {
                in_body_block = true;
                continue;
            }
            if crate::syntax::ends_routine(stmt) || _BODY_BLOCK_ENDS.contains(&kw.as_str()) {
                in_body_block = false;
                continue;
            }
            if matches!(kw.as_str(), "ENDSEQUENCE" | "SLOWSEQUENCE" | "ENDPROG") {
                after_endseq = kw == "ENDSEQUENCE" && seen_beginprog;
            } else if after_endseq
                && !in_body_block
                && !in_table
                && let Some(display) = _before_beginprog_name(stmt, &kw, profile, subs)
            {
                diags.error_basic(
                    stmt.line(),
                    &format!("{display} illegally occurs after EndSequence."),
                    stmt.file(),
                );
                return Some(stmt.line());
            }
            if matches!(stmt.as_ref(), Stmt::Call(_)) {
                if let Some(stop) = _check_call_pass2(
                    stmt,
                    subs,
                    profile,
                    diags,
                    before_bp && !in_table,
                    in_body_block,
                    Some(&sub_params),
                    Some(known),
                    Some(&link_ctx),
                    Some(dims_ds),
                ) {
                    return Some(stop);
                }
            } else if matches!(stmt.as_ref(), Stmt::Assignment(_)) {
                if let Some(stop) = _check_assignment_pass2(
                    stmt,
                    known,
                    diags,
                    before_bp && !in_table,
                    in_body_block,
                    Some(profile),
                    Some(dims_ds),
                ) {
                    return Some(stop);
                }
            } else if _control_keyword(stmt).is_some()
                && !in_body_block
                && !in_table
                && !seen_beginprog
                && (!profile.single_pass || _has_error_on(stmt, diags))
            {
                let ctrl = _control_keyword(stmt).expect("control keyword");
                let display = profile
                    .instructions()
                    .get(&ctrl)
                    .map_or(ctrl.as_str(), |r| r.name.as_str());
                diags.error_basic(
                    stmt.line(),
                    &format!("{display} illegally occurs before BeginProg."),
                    stmt.file(),
                );
                return Some(stmt.line());
            } else if let Stmt::Instruction(st) = stmt.as_ref()
                && (_FORBIDDEN_BEFORE_BEGINPROG.contains(&kw.as_str())
                    || profile.before_beginprog().contains(&kw))
            {
                if !in_body_block
                    && !in_table
                    && !in_decl_block
                    && before_bp
                    && (kw != "CALLTABLE" || _calltable_known(stmt, &tables))
                    && !(pipeline_setup && matches!(kw.as_str(), "CDM_EXCITEV" | "CDM_MUXSELECT"))
                {
                    let display = profile
                        .instructions()
                        .get(&pystr::upper(&st.keyword))
                        .map_or(st.keyword.as_str(), |r| r.name.as_str());
                    diags.error_basic(
                        stmt.line(),
                        &format!("{display} illegally occurs before BeginProg."),
                        stmt.file(),
                    );
                    return Some(stmt.line());
                }
            } else if let Stmt::Unparsed(st) = stmt.as_ref() {
                if !_routine_arg_findings(st, program, profile).is_empty()
                    || !_function_syntax_findings(stmt, program, profile).is_empty()
                {
                    continue;
                }
                if let Some(message) = _function_symbol_finding(stmt, program, profile) {
                    diags.error_basic(stmt.line(), &message, stmt.file());
                    continue;
                }
                if before_bp
                    && !in_body_block
                    && !in_table
                    && !in_decl_block
                    && _before_beginprog_name(stmt, "UNPARSED", profile, &BTreeSet::new()).is_some()
                {
                    diags.error_basic(
                        stmt.line(),
                        "ESSInitialize illegally occurs before BeginProg.",
                        stmt.file(),
                    );
                    return Some(stmt.line());
                }
                if _KNOWN_PARSER_GAP_REASONS
                    .iter()
                    .any(|r| st.reason.starts_with(r))
                    || _BENIGN_REASONS.contains(&st.reason.as_str())
                    || is_hash_statement(st)
                {
                    continue;
                }
                if let Some(messages) = _logical_assign_undefined_messages(stmt, program, profile) {
                    for message in messages {
                        diags.error(
                            stmt.line(),
                            &message,
                            stmt.file(),
                            ShowFile::Bool(message.ends_with(" is not defined.")),
                            0,
                            "",
                            "",
                        );
                    }
                    continue;
                }
                if _logical_assign_accepted(stmt, program, profile) {
                    let assignment = Stmt::Assignment(Assignment {
                        base: crate::syntax::StmtBase {
                            node: crate::syntax::Node {
                                line: stmt.line(),
                                file: stmt.file().to_owned(),
                            },
                            ..Default::default()
                        },
                        target: Some(Rc::new(Expr::Name(crate::syntax::Name {
                            node: crate::syntax::Node {
                                line: stmt.line(),
                                ..Default::default()
                            },
                            name: "_".to_owned(),
                        }))),
                        value: st.expr.clone(),
                        ..Default::default()
                    });
                    for message in var_types::_string_operand_messages(
                        &mut var_types::Typer::new(program, None, None),
                        &assignment,
                    ) {
                        diags.error_basic(stmt.line(), &message, stmt.file());
                    }
                    continue;
                }
                if let Some(bare) = _bare_sub_link_name(stmt.text(), &link_ctx.subs) {
                    link_ctx.links.borrow_mut().calls.push(bare.clone());
                    if let Some(stop) = _unlinked_call(
                        stmt,
                        &bare,
                        profile,
                        diags,
                        !in_body_block && before_bp && !in_table,
                        &link_ctx,
                    ) {
                        return Some(stop);
                    }
                    continue;
                }
                if diags.items.iter().take(pass1_count).any(|d| {
                    d.line == stmt.line()
                        && d.file == stmt.file()
                        && d.severity == "error"
                        && (d.message.starts_with(_FALLBACK_PREFIX)
                            || d.message.starts_with("Illegal Data parameter"))
                }) {
                    continue;
                }
                if instr_rules::unterminated_string_shape(
                    stmt,
                    profile,
                    subs,
                    Some(pass1_operands(program, profile)),
                    &instr_rules::user_functions(program),
                )
                .is_some()
                {
                    continue;
                }
                if !_junk_init_findings(st, profile).is_empty() {
                    continue;
                }
                if [
                    _UNITS_WITHOUT_EQ,
                    _CONST_WITHOUT_EQ,
                    _ALIAS_WITHOUT_EQ,
                    _FOR_MISSING,
                ]
                .contains(&st.reason.as_str())
                    || [
                        _INVALID_AS_CLAUSE,
                        _CONST_BAD_VALUE,
                        _CONST_BAD_NAME,
                        _DO_LOOP_PARAM,
                        _CONST_LEAD,
                        _ALIAS_NOTHING_AFTER_EQ,
                        _UNCLOSED_DIM_PREFIX,
                    ]
                    .iter()
                    .any(|r| st.reason.starts_with(r))
                {
                    continue;
                }
                let text = stmt.text();
                let at_eol = st.base.at_eol.get();
                if st.reason == _UNCLOSED_PAREN_EXPR {
                    let mut kind = if profile.known_first_word_pass1 {
                        _unclosed_paren_kind(text, &known.names)
                    } else {
                        "pass1".to_owned()
                    };
                    if kind == "pass1" && pystr::rstrip(text).ends_with('(') {
                        "fallback".clone_into(&mut kind);
                    }
                    if kind == "symbol" {
                        diags.error(
                            stmt.line(),
                            &format!("Invalid symbol in expression: {text}."),
                            stmt.file(),
                            ShowFile::Bool(false),
                            0,
                            "",
                            "",
                        );
                    } else if kind == "fallback" {
                        diags.error(
                            stmt.line(),
                            &_out_of_place(profile, text, at_eol),
                            stmt.file(),
                            ShowFile::Bool(false),
                            0,
                            "",
                            "",
                        );
                    } else {
                        if profile.fallback_echo_newline
                            && let Some(target) = TARGET.match_(text)
                            && let Some(name) = target.group(1)
                            && !context::of(program, Some(profile), None)
                                .known_names()
                                .names
                                .contains(&pystr::upper(name))
                        {
                            diags.error(
                                stmt.line(),
                                &profile.unresolved_symbol_message.format(name),
                                stmt.file(),
                                ShowFile::Bool(false),
                                0,
                                "",
                                "",
                            );
                        }
                        if profile.single_pass {
                            for message in _unparsed_string_messages(st, program, profile) {
                                diags.error_basic(stmt.line(), &message, stmt.file());
                            }
                        }
                        let cond = condition_part(text);
                        let operands = pass1_operands(program, profile);
                        let message = cond
                            .as_deref()
                            .filter(|c| !pystr::strip(c).is_empty())
                            .and_then(|c| _unclosed_call_message(profile, c, false, &operands))
                            .filter(|m| !m.is_empty())
                            .or_else(|| {
                                _unclosed_call_message(profile, text, at_eol, &operands)
                                    .filter(|m| !m.is_empty())
                            })
                            .unwrap_or_else(|| _no_matching(profile, text, at_eol));
                        diags.error(
                            stmt.line(),
                            &message,
                            stmt.file(),
                            ShowFile::Bool(false),
                            0,
                            "",
                            "",
                        );
                    }
                } else if st.reason == _EXTRA_CLOSE_PAREN
                    && !_assignment_head(text, program, profile)
                {
                    diags.error_basic(
                        stmt.line(),
                        &_out_of_place(profile, text, at_eol),
                        stmt.file(),
                    );
                } else if st.reason == _EXTRA_CLOSE_PAREN {
                    let sep = if profile.fallback_echo_newline && at_eol {
                        "\n"
                    } else {
                        ""
                    };
                    diags.error(
                        stmt.line(),
                        &format!("Incomplete expression: {text}{sep}."),
                        stmt.file(),
                        ShowFile::Bool(false),
                        0,
                        "",
                        "",
                    );
                } else {
                    let findings = if profile.known_first_word_pass1 {
                        _instruction_arg_findings(
                            text,
                            &pass1_operands(program, profile),
                            profile,
                            context::of(program, Some(profile), None).declared_variable_names(),
                        )
                    } else {
                        vec![]
                    };
                    if let Some(first) = findings.first() {
                        if matches!(first.0, 2 | 6) {
                            diags.error(
                                stmt.line(),
                                &format!("Invalid symbol in expression: {}.", first.2),
                                stmt.file(),
                                ShowFile::Bool(false),
                                0,
                                "",
                                "",
                            );
                            if first.0 == 6 && in_table {
                                return Some(stmt.line());
                            }
                            for (stage, message, _) in findings.iter().skip(1) {
                                if *stage == 5 {
                                    diags.error_basic(stmt.line(), message, stmt.file());
                                }
                            }
                            let ikw = pystr::upper(WORD_SPLIT.split(pystr::lstrip(text), 1)[0]);
                            if let Some(record) = profile.instructions().get(&ikw)
                                && !in_body_block
                                && !in_table
                                && !in_decl_block
                                && before_bp
                                && profile.before_beginprog().contains(&ikw)
                            {
                                diags.error_basic(
                                    stmt.line(),
                                    &format!("{} illegally occurs before BeginProg.", record.name),
                                    stmt.file(),
                                );
                                return Some(stmt.line());
                            }
                        }
                        continue;
                    }
                    let scan_stmt = _inline_condition_stmt(stmt, profile);
                    let verdict = if profile.known_first_word_pass1 {
                        _pass1_scan(
                            scan_stmt.text(),
                            Some(&pass1_operands(program, profile)),
                            profile,
                        )
                    } else {
                        None
                    };
                    if verdict
                        .as_ref()
                        .is_some_and(|(phase, kind)| phase == "pass2" && kind == "symbol")
                    {
                        diags.error(
                            stmt.line(),
                            &_pass1_message(profile, &scan_stmt, "symbol", None),
                            stmt.file(),
                            ShowFile::Cr3xx,
                            0,
                            "",
                            "",
                        );
                    } else if let Some((phase, kind)) = &verdict
                        && phase == "pass1"
                        && (kind != "fallback" || condition_part(text).is_some())
                    {
                        diags.error(
                            stmt.line(),
                            &_pass1_message(profile, stmt, kind, None),
                            stmt.file(),
                            ShowFile::Bool(false),
                            0,
                            "",
                            "",
                        );
                    } else {
                        if verdict
                            .as_ref()
                            .is_some_and(|(phase, kind)| phase == "pass2" && kind == "fallback")
                            || (verdict.is_none()
                                && profile.known_first_word_pass1
                                && _known_first_token(text, &pass1_operands(program, profile)))
                            || (verdict.is_none()
                                && profile.single_pass
                                && _logical_led_string(text))
                        {
                            for message in _unparsed_string_messages(st, program, profile) {
                                diags.error_basic(stmt.line(), &message, stmt.file());
                            }
                        }
                        if let Some(lead) = LEAD.match_(text)
                            && profile.known_first_word_pass1
                            && let Some(name) = lead.group(1)
                            && !context::of(program, Some(profile), None)
                                .system_table_names()
                                .contains(&pystr::upper(name))
                            && !_table_names(program).contains(&pystr::upper(name))
                        {
                            diags.error_basic(
                                stmt.line(),
                                &format!("Could not find {name}."),
                                stmt.file(),
                            );
                        }
                        diags.error(
                            stmt.line(),
                            &_leftover_message(profile, stmt),
                            stmt.file(),
                            ShowFile::Cr3xx,
                            0,
                            "",
                            "",
                        );
                    }
                    if profile.undefined_call_message {
                        continue;
                    }
                }
                if !in_body_block && before_bp && !in_table {
                    let head = HEAD.match_(text);
                    let record = head
                        .as_ref()
                        .and_then(|h| h.group(1))
                        .and_then(|h| profile.instructions().get(&pystr::upper(h)));
                    let display = record
                        .filter(|_| profile.known_first_word_pass1)
                        .map_or("Expr", |r| r.name.as_str());
                    diags.error_basic(
                        stmt.line(),
                        &format!("{display} illegally occurs before BeginProg."),
                        stmt.file(),
                    );
                    return Some(stmt.line());
                }
            }
        }
        link_ctx.links.borrow_mut().cutoff = None;
        None
    })();
    if let Some(links) = links {
        *links = std::mem::take(&mut *link_ctx.links.borrow_mut());
    }
    result
}

/// Port of checker/structure.py::_CLOSER_MESSAGES
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

/// Port of checker/structure.py::_names_unresolved
pub fn _names_unresolved(known: &KnownView<'_>, stmt: &Instruction, sources: bool) -> bool {
    let skip = !sources
        && instr_data::part2::_TABLE_OUTPUT.contains(&pystr::upper(&stmt.keyword).as_str());
    stmt.args.iter().enumerate().any(|(i, arg)| {
        arg.is_some()
            && !(skip && i == 1)
            && !expressions::_undefined_names_with(arg.as_deref(), known).is_empty()
    })
}

/// Port of checker/structure.py::_unparsed_opener
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

/// Port of checker/structure.py::check_block_closers
pub fn check_block_closers(program: &Program, profile: &Profile, diags: &mut Diagnostics) {
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
        let kw = super::part3::_DO_ALIASES
            .get(keyword.as_str())
            .copied()
            .unwrap_or(keyword.as_str());
        let word = if matches!(kw, "END" | "NEXT") {
            _block_word(stmt.args())
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

/// Port of checker/structure.py::_undefined_table_source
pub fn _undefined_table_source(
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

/// Port of checker/structure.py::_leftover_first_name_known
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
    fn pass2_matches_python_placement_and_program_state() {
        for logger in ["cr6", "cr1000x", "cr300", "cr350"] {
            let profile = crate::profiles::get(logger).unwrap();
            for (source, line, message, multi_pass_only) in [
                (
                    "BeginProg\nBeginProg\nEndProg\n",
                    2,
                    "Multiple BeginProgs.",
                    false,
                ),
                (
                    "BeginProg\nNextScan\nNextScan\nEndProg\n",
                    2,
                    "NextScan with no Scan.",
                    true,
                ),
                (
                    "DataTable(T,1,-1)\nSample(1,Nope,FP2)\nEndTable\nBeginProg\nEndProg\n",
                    2,
                    "Invalid symbol in expression: Nope.",
                    true,
                ),
                (
                    "BeginProg\nScan(0,Sec,0,0)\nSlowSequence\nEndProg\n",
                    3,
                    "Missing NextScan.",
                    true,
                ),
                (
                    "SlowSequence\nBeginProg\nEndProg\n",
                    2,
                    "Multiple BeginProgs.",
                    false,
                ),
                (
                    "BeginProg\nEndSequence\nBattery(X)\nEndProg\n",
                    3,
                    "Battery illegally occurs after EndSequence.",
                    false,
                ),
            ] {
                let (program, parse_diags) =
                    crate::parser::parse_text(source, "probe.CR6", None, Some(profile));
                assert!(parse_diags.items.is_empty(), "{logger}: {source:?}");
                let mut diags = Diagnostics::new();
                let mut links = CallLinks::default();
                let stop = check_pass2(&program, profile, &mut diags, None, Some(&mut links));
                let suppressed = multi_pass_only && profile.single_pass;
                let expected_stop =
                    (!suppressed && message != "Multiple BeginProgs.").then_some(line);
                assert_eq!(stop, expected_stop, "{logger}: {source:?}");
                let actual: Vec<_> = diags
                    .items
                    .iter()
                    .map(|d| (d.line, d.message.as_str()))
                    .collect();
                let expected = if suppressed {
                    vec![]
                } else {
                    vec![(line, message)]
                };
                assert_eq!(actual, expected, "{logger}: {source:?}");
                assert_eq!(
                    links.cutoff,
                    expected_stop.map(|line| ("probe.CR6".to_owned(), line))
                );
                assert_eq!(links.positions.len(), program.statements.len());
            }
        }
    }

    #[test]
    fn pass2_preserves_supplied_links_on_early_return_and_completion() {
        let profile = crate::profiles::get("cr6").unwrap();
        for source in [
            "BeginProg\nNextScan\n",
            "Sub S1\nEndSub\nBeginProg\nEndProg\n",
        ] {
            let (program, _) = crate::parser::parse_text(source, "probe.CR6", None, Some(profile));
            let mut links = CallLinks::default();
            links.calls.push("PREVIOUS".to_owned());
            links.registered.insert("PREVIOUS".to_owned());
            links.positions.insert(("earlier.CR6".to_owned(), 7), 0);
            links.first = Some((7, "deferred".to_owned(), "earlier.CR6".to_owned()));
            let mut diags = Diagnostics::new();
            let stop = check_pass2(&program, profile, &mut diags, None, Some(&mut links));
            assert_eq!(links.calls, ["PREVIOUS"]);
            assert!(links.registered.contains("PREVIOUS"));
            assert_eq!(links.positions.len(), 1);
            assert_eq!(
                links.first,
                Some((7, "deferred".to_owned(), "earlier.CR6".to_owned()))
            );
            assert_eq!(
                links.cutoff,
                stop.map(|line| ("probe.CR6".to_owned(), line))
            );
            if stop.is_none() {
                assert!(links.registered.contains("S1"));
            }
        }
    }

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
        assert!(_names_unresolved(&KnownView::from_set(&known), &stmt, true));
        assert!(!_names_unresolved(
            &KnownView::from_set(&known),
            &stmt,
            false
        ));
        assert_eq!(
            _undefined_table_source(
                &stmt,
                crate::profiles::get("cr6").unwrap(),
                &KnownView::from_set(&known),
            ),
            Some("Missing".to_owned())
        );
    }
}
