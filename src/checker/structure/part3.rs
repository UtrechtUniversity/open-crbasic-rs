//! Port of `checker/structure.py` (lines 1294-2081).

#![allow(dead_code, unused_variables, unused_imports, clippy::doc_markdown)]

use std::collections::BTreeSet;
use std::rc::Rc;
use std::sync::LazyLock;

use crate::checker::{context, expressions, symbols};
use crate::consteval::Env;
use crate::syntax::{StmtBase, replace_stmt, stmt_id};
use crate::{pyre, pystr};

use crate::diagnostics::{DiagRef, Diagnostic, Diagnostics};
use crate::profiles::Profile;
use crate::syntax::{
    Assignment, Call, Case, ConstDecl, Declaration, Expr, ExprRef, Index, Instruction, Juxtaposed,
    Member, Number, OrderedMap, Program, ProgramRef, Stmt, StmtRef, SubDef, Unparsed, VarDecl,
};

/// Port of checker/structure.py::check_pass1
pub fn check_pass1(_program: &Program, _diags: &mut Diagnostics) {
    todo!(
        "checker/structure.py::check_pass1 (omitted: profile: untyped, env: consteval.Env | None)"
    )
}

// Omitted arguments stay explicit at call sites until the owning parts expose full signatures.
macro_rules! pending_call {
    ($call:expr; $($omitted:expr),* $(,)?) => {{
        $(let _ = &$omitted;)*
        $call
    }};
}
macro_rules! pending_result {
    ($call:expr; $signature:literal; $($omitted:expr),* $(,)?) => {{
        $(let _ = &$omitted;)*
        $call;
        missing_dependency_result($signature)
    }};
}

fn missing_dependency_result<T>(signature: &str) -> T {
    unimplemented!("{signature}")
}

/// Port of checker/structure.py::check_pass1
#[allow(clippy::if_not_else, clippy::match_same_arms)]
pub fn check_pass1_with(
    program: &ProgramRef,
    profile: &'static Profile,
    diags: &mut Diagnostics,
    env: Option<&Env>,
) {
    use expressions::KnownNames;
    use std::cell::RefCell;

    static SIZE_WORDS: LazyLock<pyre::Regex> =
        LazyLock::new(|| pyre::Regex::new(r"[A-Za-z_]\w*", 0));
    static SIZE_WORD: LazyLock<pyre::Regex> =
        LazyLock::new(|| pyre::Regex::new(r"\s*\d+\s+[A-Za-z_]\w*\s*$", 0));
    static FOR_ASSIGN: LazyLock<pyre::Regex> =
        LazyLock::new(|| pyre::Regex::new(r"\s*For\s+\w+\s*=", pyre::I));
    static TO: LazyLock<pyre::Regex> = LazyLock::new(|| pyre::Regex::new(r"\bTo\b", pyre::I));
    static FOR: LazyLock<pyre::Regex> = LazyLock::new(|| pyre::Regex::new(r"\s*For\s", pyre::I));
    static CONDITION: LazyLock<pyre::Regex> =
        LazyLock::new(|| pyre::Regex::new(r"\s*(?:ELSEIF|IF)\s(.*?)THEN", pyre::I | pyre::S));
    static SELECT: LazyLock<pyre::Regex> =
        LazyLock::new(|| pyre::Regex::new(r"\s*SELECT\b", pyre::I));

    type AliasState = (
        Vec<StmtRef>,
        BTreeSet<String>,
        OrderedMap<String, Vec<Vec<i64>>>,
    );

    let owned_env;
    let env = if let Some(env) = env {
        env
    } else {
        owned_env = Env::with_consts(profile, Rc::new(RefCell::new(program.consts.clone())));
        &owned_env
    };
    let mut scan_depth: i64 = 0;
    let mut cond_depth: i64 = 0;
    let mut select_depth: i64 = 0;
    let mut in_table = false;
    let mut nested_table_line = None;
    let mut suppress_line = None;
    let mut alias_state: AliasState = (Vec::new(), BTreeSet::new(), OrderedMap::new());
    let mut global_alias_state: Option<AliasState> = None;
    let mut alias_parameters: Option<StmtRef> = None;
    let mut alias_parameter_names = BTreeSet::new();
    let mut known: Option<KnownNames> = None;
    let mut operands = BTreeSet::new();
    let mut in_sub = false;
    let mut in_struct = false;
    let mut struct_members = BTreeSet::new();
    let mut structs_seen = BTreeSet::new();
    let mut const_tables: i64 = 0;
    let mut const_open: i64 = 0;
    let mut slow_sequences: i64 = 0;
    let mut seen_endprog = false;
    let mut const_shadow_reported = false;
    let mut header_dims_failed = false;
    let mut declared_kinds: OrderedMap<String, String> = OrderedMap::new();
    let struct_types: BTreeSet<String> = program
        .statements
        .iter()
        .filter_map(|st| {
            if let Stmt::Instruction(i) = st.as_ref()
                && ["STRUCTURETYPE", "OBJECTTYPE"].contains(&pystr::upper(&i.keyword).as_str())
                && let Some(Expr::Name(n)) = i.args.first().and_then(Option::as_deref)
            {
                Some(pystr::upper(&n.name))
            } else {
                None
            }
        })
        .collect();
    let decl_variables: BTreeSet<String> = program
        .statements
        .iter()
        .filter_map(|st| {
            if let Stmt::Declaration(d) = st.as_ref()
                && ["PUBLIC", "DIM", "STATIC"].contains(&d.kind.as_str())
            {
                Some(d)
            } else {
                None
            }
        })
        .flat_map(|d| d.vars.iter().map(|v| pystr::upper(&v.name)))
        .collect();
    // `_CONTROL_KEYWORDS` is private in expressions/part2.rs.
    let control_keywords = [
        "CASE", "DO", "FOR", "FUNCTION", "IF", "SCAN", "SELECT", "SUB", "SUBSCAN", "WHILE",
    ];
    let predefined_constants: BTreeSet<String> = profile
        .symbols()
        .iter()
        .map(|(n, _)| n.to_owned())
        .chain(
            expressions::part1::_EXTRA_PREDEFINED_SYMBOLS
                .iter()
                .map(|s| (*s).to_owned()),
        )
        .filter(|n| {
            !decl_variables.contains(n)
                && !profile.functions().contains(n)
                && !control_keywords.contains(&n.as_str())
        })
        .collect();
    let mut deferred = false;
    let mut all_known: Option<KnownNames> = None;
    let mut seen_consts = BTreeSet::new();
    let mut first_declared = OrderedMap::new();
    let ctx = context::of(program, Some(profile), None);
    let scopes = ctx.statement_scopes();
    let mut alias_spans: OrderedMap<String, Vec<Rc<RefCell<(usize, usize)>>>> = OrderedMap::new();
    let mut routine_spans: Vec<Rc<RefCell<(usize, usize)>>> = Vec::new();
    for (pos, stmt) in program.statements.iter().enumerate() {
        if matches!(stmt.as_ref(), Stmt::SubDef(_)) {
            routine_spans.clear();
        } else if crate::syntax::ends_routine(stmt) {
            for span in &routine_spans {
                span.borrow_mut().1 = pos;
            }
            routine_spans.clear();
        }
        if super::_keyword(stmt) == "ALIAS"
            && let Stmt::Instruction(i) = stmt.as_ref()
        {
            for a in i.args.iter().skip(1).flatten() {
                let name = match a.as_ref() {
                    Expr::Name(n) => Some(&n.name),
                    Expr::Index(i) => Some(&i.name),
                    _ => None,
                };
                if let Some(name) = name {
                    let name = pystr::upper(name);
                    let span = Rc::new(RefCell::new((pos, program.statements.len())));
                    let mut spans = alias_spans.get(&name).cloned().unwrap_or_default();
                    spans.push(span.clone());
                    alias_spans.insert(name, spans);
                    routine_spans.push(span);
                }
            }
        }
        let names: Vec<&str> = match stmt.as_ref() {
            Stmt::ConstDecl(c) => vec![&c.name],
            Stmt::Declaration(d) => d.vars.iter().map(|v| v.name.as_str()).collect(),
            _ => vec![],
        };
        for name in names {
            let name = pystr::upper(name);
            if !first_declared.contains_key(&name) {
                first_declared.insert(name, pos);
            }
        }
    }
    for (pos, stmt) in program.statements.iter().enumerate() {
        let kw = super::_keyword(stmt);
        let args = stmt.args();
        let line = stmt.base().node.line;
        let file = &stmt.base().node.file;
        let text = &stmt.base().text;
        if let Stmt::ConstDecl(c) = stmt.as_ref() {
            seen_consts.insert(pystr::upper(&c.name));
        }
        let fire_suppress = suppress_line == Some(line);
        suppress_line = None;
        if ["END", "EXIT"].contains(&kw.as_str())
            && matches!(stmt.as_ref(), Stmt::Instruction(_))
            && args.len() == 1
            && let Some(Expr::Name(n)) = args[0].as_deref()
            && ![
                "IF", "SELECT", "SUB", "FUNCTION", "PROG", "TABLE", "SCAN", "DO", "FOR",
            ]
            .contains(&pystr::upper(&n.name).as_str())
            && !profile.symbols().contains(&pystr::upper(&n.name))
            && !profile.functions().contains(&pystr::upper(&n.name))
        {
            diags.error_basic(line, &format!("Illegal END or EXIT {}.", n.name), file);
        }
        if let Stmt::SubDef(sub) = stmt.as_ref() {
            // Python saves references to the global collections, not copies.
            if global_alias_state.is_none() {
                global_alias_state = Some(std::mem::take(&mut alias_state));
            } else {
                alias_state = AliasState::default();
            }
            alias_parameter_names = sub
                .params
                .iter()
                .filter(|p| {
                    declared_kinds
                        .get(&pystr::upper(&p.name))
                        .map(String::as_str)
                        != Some("CONST")
                })
                .map(|p| pystr::upper(&p.name))
                .collect();
            if !in_sub
                && declared_kinds
                    .get(&pystr::upper(&sub.name))
                    .map(String::as_str)
                    == Some("CONST")
            {
                pending_call!(super::_report_numeric_sub_name(sub, diags); program.consts.get(&pystr::upper(&sub.name)));
            }
            if known.is_none() {
                known = Some(ctx.known_names().clone());
                operands = pending_call!(super::pass1_operands(program); profile);
            }
            if profile.single_pass && header_dims_failed {
                alias_parameters = Some(Rc::new(Stmt::Declaration(Declaration {
                    base: StmtBase::new(line, file),
                    kind: "DIM".to_owned(),
                    vars: vec![],
                })));
                if !in_sub {
                    pending_call!(super::_check_header_types(text, sub, &structs_seen, diags); profile);
                }
            } else {
                let forward = program
                    .consts
                    .keys()
                    .cloned()
                    .chain(decl_variables.iter().cloned())
                    .filter(|n| !declared_kinds.contains_key(n))
                    .collect();
                let (parameters, failed): (Declaration, bool) = pending_result!(
                    super::_alias_parameter_declaration(sub, &declared_kinds, &known.as_ref().unwrap().names, &forward,
                        if in_sub { None } else { Some(&mut *diags) }, &structs_seen);
                    "part8::_alias_parameter_declaration_with must return (Declaration, bool)"; env, profile);
                alias_parameters = Some(Rc::new(Stmt::Declaration(parameters)));
                header_dims_failed |= failed;
            }
            for message in &sub.header_errors {
                diags
                    .items
                    .push(Rc::new(Diagnostic::basic("error", line, message, file)));
            }
            in_sub = true;
        } else if ["ENDSUB", "ENDFUNCTION"].contains(&kw.as_str())
            || (kw == "END"
                && [Some("SUB"), Some("FUNCTION")]
                    .contains(&pending_call!(super::_block_word(); args).as_deref()))
        {
            in_sub = false;
            if let Some(global) = global_alias_state.take() {
                alias_state = global;
            }
            alias_parameters = None;
            alias_parameter_names.clear();
        }
        if matches!(stmt.as_ref(), Stmt::Declaration(_)) {
            alias_state.0.push(stmt.clone());
        }
        if ["STRUCTURETYPE", "OBJECTTYPE"].contains(&kw.as_str()) {
            if in_struct {
                diags.error_basic(line, "Structure inside Structure", file);
            }
            in_struct = true;
            struct_members.clear();
            if let Some(Expr::Name(n)) = args.first().and_then(Option::as_deref) {
                structs_seen.insert(pystr::upper(&n.name));
            }
        } else if ["ENDSTRUCTURETYPE", "ENDOBJECTTYPE"].contains(&kw.as_str()) {
            if !in_struct {
                diags.error_basic(line, "EndStructure without Structure", file);
            }
            in_struct = false;
            struct_members.clear();
        }
        let mut dup_at = None;
        let mut dup_diags = Diagnostics::new();
        if !in_sub && !in_struct {
            if profile.single_pass
                && matches!(stmt.as_ref(), Stmt::Declaration(d) if ["PUBLIC", "DIM"].contains(&d.kind.as_str()) && !d.vars.is_empty())
            {
                dup_at = pending_call!(super::_check_duplicate_declaration(stmt, &declared_kinds, &mut dup_diags); profile);
            } else {
                dup_at = pending_call!(super::_check_duplicate_declaration(stmt, &declared_kinds, diags); profile);
            }
        } else if in_sub && !in_struct {
            symbols::check_local_name_collision_with_profile(stmt, profile, diags);
            if let Stmt::ConstDecl(c) = stmt.as_ref()
                && let Some(bad) = super::_bad_const_type(c)
            {
                diags.error_basic(line, &format!("{bad} unknown Const type"), file);
            }
            if !seen_endprog {
                for name in super::_local_const_conflicts(stmt, &declared_kinds) {
                    if !(profile.single_pass && const_shadow_reported) {
                        diags.error_basic(
                            line,
                            &format!("{name} is already declared as a CONST."),
                            file,
                        );
                    }
                    const_shadow_reported = true;
                }
                if matches!(stmt.as_ref(), Stmt::Declaration(d) if d.kind == "PUBLIC")
                    || matches!(stmt.as_ref(), Stmt::ConstDecl(_))
                {
                    let mut dup = Diagnostics::new();
                    if let Stmt::ConstDecl(c) = stmt.as_ref() {
                        let name = pystr::upper(&c.name);
                        match declared_kinds.get(&name).map(String::as_str) {
                            Some("VAR" | "ALIAS") => dup.error_basic(
                                line,
                                &format!("{} already declared as a variable.", c.name),
                                file,
                            ),
                            None if !symbols::is_reserved_predefined_name_with_profile(
                                &c.name, profile,
                            ) && !profile.functions().contains(&name) =>
                            {
                                declared_kinds.insert(name, "CONST".to_owned());
                            }
                            _ => {}
                        }
                    } else {
                        pending_call!(super::_check_duplicate_declaration(stmt, &declared_kinds, &mut dup); Option::<&Profile>::None);
                    }
                    for item in dup.items {
                        if item.message.contains("is already declared as a CONST") {
                            if profile.single_pass && const_shadow_reported {
                                continue;
                            }
                            const_shadow_reported = true;
                        }
                        diags.error_basic(item.line, &item.message, &item.file);
                    }
                }
            }
        }
        if in_struct
            && let Stmt::Declaration(d) = stmt.as_ref()
            && d.kind == "STRUCTUREMEMBER"
        {
            struct_members.extend(d.vars.iter().map(|v| pystr::upper(&v.name)));
            pending_call!(super::_check_member_types(d, &structs_seen, diags); profile);
            if known.is_none() {
                known = Some(ctx.known_names().clone());
                operands = pending_call!(super::pass1_operands(program); profile);
            }
            if !d.vars.is_empty()
                && d.vars.iter().any(|v| {
                    v.init.as_ref().is_some_and(|i| !i.is_empty())
                        || !v.dims.is_empty()
                        || v.string_size.is_some()
                })
            {
                let top: BTreeSet<String> = program
                    .statements
                    .iter()
                    .filter_map(|s| match s.as_ref() {
                        Stmt::Declaration(d) if d.kind != "STRUCTUREMEMBER" => Some(d),
                        _ => None,
                    })
                    .flat_map(|d| d.vars.iter().map(|v| pystr::upper(&v.name)))
                    .collect();
                let member_only: BTreeSet<String> = program
                    .statements
                    .iter()
                    .filter_map(|s| match s.as_ref() {
                        Stmt::Declaration(d) if d.kind == "STRUCTUREMEMBER" => Some(d),
                        _ => None,
                    })
                    .flat_map(|d| d.vars.iter().map(|v| pystr::upper(&v.name)))
                    .filter(|n| !top.contains(n) && !program.consts.contains_key(n))
                    .collect();
                let here = known
                    .as_ref()
                    .unwrap()
                    .names
                    .difference(&member_only)
                    .cloned()
                    .collect();
                pending_call!(super::_check_decl_exprs(d, &here, &decl_variables, diags, &struct_types, None, &BTreeSet::new()); profile, Option::<(BTreeSet<String>, BTreeSet<String>)>::None);
            }
        } else if (in_struct || in_sub) && kw == "READONLY" {
            if args.is_empty() {
                diags.error_basic(line, "Missing variable name.", file);
            }
            for a in args.iter().flatten() {
                if let Expr::Name(n) = a.as_ref()
                    && !struct_members.contains(&pystr::upper(&n.name))
                    && !matches!(
                        declared_kinds
                            .get(&pystr::upper(&n.name))
                            .map(String::as_str),
                        Some("VAR" | "ALIAS")
                    )
                {
                    diags.error_basic(line, &format!("{} not yet declared.", n.name), file);
                }
            }
        }
        if let Stmt::Declaration(d) = stmt.as_ref() {
            for v in &d.vars {
                if let Some(size_missing) = v.size_missing {
                    let echo = pystr::slice(text, Some(size_missing + 1), None);
                    let message = if echo.is_empty() || pystr::lstrip(echo).starts_with(',') {
                        "Size of string missing.".to_owned()
                    } else {
                        format!("Invalid, or out of place expression: {echo}.")
                    };
                    if !(profile.single_pass
                        && diags
                            .items
                            .iter()
                            .any(|d| d.message == "Size of string missing."))
                    {
                        diags.error_basic(line, &message, file);
                    }
                    break;
                }
                if let Some((at, kind)) = &v.size_junk {
                    let segment = pystr::slice(text, Some(*at), None);
                    let segment = segment.split(',').next().unwrap_or("");
                    let (echo, assignment) = segment
                        .split_once('=')
                        .map_or((segment, false), |(e, _)| (e, true));
                    if profile.single_pass && kind == "word" {
                        let words: Vec<String> = SIZE_WORDS
                            .finditer(echo)
                            .into_iter()
                            .filter_map(|m| m.group(0).map(str::to_owned))
                            .collect();
                        if words.len() != 1 || SIZE_WORD.match_(echo).is_none() {
                            continue;
                        }
                        diags.error_basic(
                            line,
                            &profile.unresolved_symbol_message.format(&words[0]),
                            file,
                        );
                        let echo = pystr::rstrip(echo);
                        diags.error_basic(line, &format!("Incomplete expression: {echo}."), file);
                        diags.error_basic(
                            line,
                            &format!("Variable illegal in Parameter: {echo}."),
                            file,
                        );
                        break;
                    }
                    let echo = if assignment {
                        echo.to_owned()
                    } else {
                        pystr::rstrip(echo).to_owned()
                    };
                    let message = if kind == "bang" {
                        format!("Incomplete expression: {echo}.")
                    } else {
                        format!("Invalid symbol in expression: {echo}.")
                    };
                    diags.error_basic(line, &message, file);
                    break;
                }
            }
        }
        if profile.known_first_word_pass1
            && matches!(stmt.as_ref(), Stmt::Unparsed(u) if u.reason != "For missing parameter")
            && FOR_ASSIGN.match_(text).is_some()
            && TO.search(text).is_none()
        {
            diags.error_basic(line, "Invalid or missing parameter.", file);
            continue;
        }
        if profile.known_first_word_pass1 && FOR.match_(text).is_some() {
            let found = pending_call!(super::_for_header_findings(text); profile);
            if !found.is_empty() {
                for message in found {
                    diags.error_basic(line, &message, file);
                }
                continue;
            }
        }
        if (profile.known_first_word_pass1 || profile.single_pass)
            && super::_block_keyword_call(stmt)
        {
            let cond = if matches!(stmt.as_ref(), Stmt::If(_) | Stmt::ElseIf(_)) {
                CONDITION.match_(text)
            } else {
                None
            };
            let sep = if cond.is_none() && profile.fallback_echo_newline && stmt.at_eol() {
                "\n"
            } else {
                ""
            };
            let echo = cond.as_ref().and_then(|m| m.group(1)).unwrap_or(text);
            diags.error_basic(line, &format!("Incomplete expression: {echo}{sep}."), file);
        }
        if matches!(stmt.as_ref(), Stmt::Instruction(_)) {
            match kw.as_str() {
                "CONSTTABLE" => {
                    if const_tables != 0 {
                        diags.error_basic(line, "Only one ConstTable allowed.", file);
                    }
                    const_tables += 1;
                    const_open += 1;
                }
                "ENDCONSTTABLE" => {
                    if const_open != 0 {
                        const_open -= 1;
                    } else {
                        diags.error_basic(line, "EndConstTable with no ConstTable.", file);
                    }
                }
                "MQTTPUBLISHCONSTTABLE"
                    if const_open == 0 && profile.instructions().contains(&kw) =>
                {
                    diags.error_basic(
                        line,
                        "MQTTPublishConstTable instruction must be in a ConstTable",
                        file,
                    );
                }
                "ENDPROG" => seen_endprog = true,
                "SLOWSEQUENCE" if !seen_endprog => {
                    slow_sequences += 1;
                    if slow_sequences > 4 {
                        diags.error_basic(line, "Too many Slow Sequences: max = 4.", file);
                    }
                }
                _ => {}
            }
        }
        if let Stmt::Declaration(d) = stmt.as_ref()
            && ["PUBLIC", "DIM", "STATIC"].contains(&d.kind.as_str())
        {
            if d.vars.is_empty() {
                suppress_line = Some(super::_check_declaration(d, diags));
            } else {
                if known.is_none() {
                    known = Some(ctx.known_names().clone());
                    operands = pending_call!(super::pass1_operands(program); profile);
                }
                let mut length_scope: Option<(BTreeSet<String>, BTreeSet<String>)> = None;
                let (params, later) = &scopes[&stmt_id(stmt)];
                let remove: BTreeSet<String> = later
                    .difference(params)
                    .filter(|n| !decl_variables.contains(*n))
                    .cloned()
                    .collect();
                let here = known.as_ref().unwrap().without_names(&remove);
                let indexed_constants: BTreeSet<String> = predefined_constants
                    .iter()
                    .cloned()
                    .chain(
                        seen_consts
                            .iter()
                            .filter(|n| program.consts.contains_key(*n))
                            .cloned(),
                    )
                    .filter(|n| !params.contains(n))
                    .collect();
                let seen: BTreeSet<String> = alias_spans
                    .iter()
                    .filter(|(_, spans)| {
                        spans.iter().any(|s| {
                            let (lo, hi) = *s.borrow();
                            lo < pos && pos <= hi
                        })
                    })
                    .map(|(n, _)| n.clone())
                    .collect();
                if d.vars
                    .iter()
                    .any(|v| v.type_.as_deref() == Some("STRING") && v.string_size.is_some())
                {
                    let later: BTreeSet<String> = first_declared
                        .iter()
                        .filter(|(_, at)| **at > pos)
                        .map(|(n, _)| n.clone())
                        .chain(
                            alias_spans
                                .keys()
                                .filter(|n| {
                                    !seen.contains(*n)
                                        && first_declared.get(*n).is_none_or(|at| *at >= pos)
                                })
                                .cloned(),
                        )
                        .collect();
                    length_scope = Some((
                        known
                            .as_ref()
                            .unwrap()
                            .names
                            .difference(&later)
                            .cloned()
                            .collect(),
                        decl_variables
                            .union(&seen)
                            .filter(|n| !later.contains(*n))
                            .cloned()
                            .chain(alias_parameter_names.iter().cloned())
                            .collect(),
                    ));
                }
                if profile.single_pass {
                    let mut dims_diags = Diagnostics::new();
                    let mut names_diags = Diagnostics::new();
                    let earlier_errors = diags.items.iter().any(|d| d.severity == "error");
                    let dim_at =
                        pending_call!(super::_check_array_dims(d, &mut dims_diags); env, profile);
                    let variables = decl_variables.union(params).cloned().collect();
                    let name_at = pending_call!(super::_check_decl_exprs(d, &here.names, &variables, &mut names_diags, &struct_types, dim_at, &indexed_constants); profile, length_scope, here);
                    let bad_at = name_at.or(dim_at);
                    let own_items = if name_at.is_some() {
                        names_diags.items
                    } else {
                        dims_diags
                            .items
                            .into_iter()
                            .chain(names_diags.items)
                            .collect()
                    };
                    match (dup_at, bad_at) {
                        (None, _) => diags.items.extend(own_items),
                        (Some(dup), bad) if bad.is_none_or(|bad| dup < bad) => {
                            diags.items.extend(dup_diags.items);
                        }
                        (Some(dup), Some(bad)) if dup == bad => {
                            diags.items.extend(
                                own_items
                                    .into_iter()
                                    .filter(|d| !d.message.starts_with("Too many parameters:")),
                            );
                            diags.items.extend(dup_diags.items);
                        }
                        _ => diags.items.extend(own_items),
                    }
                    if let Some(stop) = [bad_at, dup_at].into_iter().flatten().min()
                        && !in_sub
                        && !in_struct
                        && !earlier_errors
                    {
                        for (i, v) in d.vars.iter().enumerate() {
                            let i = i64::try_from(i).expect("statement variable index fits i64");
                            if i <= stop
                                || v.type_.as_ref().is_some_and(|t| struct_types.contains(t))
                            {
                                continue;
                            }
                            if dup_at.is_none_or(|dup| i < dup) {
                                declared_kinds.remove(&pystr::upper(&v.name));
                            }
                        }
                        // Python structure.py:1698 mutates this shared statement in place.
                        if d.vars.iter().enumerate().any(|(i, v)| {
                            i64::try_from(i).expect("statement variable index fits i64") > stop
                                && !v.type_.as_ref().is_some_and(|t| struct_types.contains(t))
                        }) {
                            todo!(
                                "Declaration.vars needs interior mutability for pass1 truncation after stop={stop}"
                            );
                        }
                    }
                } else {
                    pending_call!(super::_check_array_dims(d, diags); env, profile);
                    let variables = decl_variables
                        .iter()
                        .chain(seen.iter())
                        .chain(params.iter())
                        .cloned()
                        .collect();
                    let indexed_constants = indexed_constants.difference(&seen).cloned().collect();
                    pending_call!(super::_check_decl_exprs(d, &here.names, &variables, diags, &struct_types, None, &indexed_constants); profile, length_scope, here);
                }
                pending_call!(super::_check_init_bounds(d, diags); env);
            }
        } else if fire_suppress {
        } else if let Stmt::Unparsed(u) = stmt.as_ref() {
            if !pending_call!(super::_logical_assign_accepted(u, program); profile) {
                let messages = pending_call!(super::_function_syntax_findings(u, program); profile);
                if !messages.is_empty() {
                    for message in messages {
                        diags.error_basic(line, &message, file);
                    }
                } else {
                    let messages = pending_call!(super::_routine_arg_findings(u, program); profile);
                    if !messages.is_empty() {
                        for message in messages {
                            diags.error_basic(line, &message, file);
                        }
                    } else {
                        super::_check_unparsed_pass1_with(u, diags, program, profile);
                        for message in super::_junk_init_findings_with(u, profile) {
                            if message == "Size of string missing."
                                && profile.single_pass
                                && diags.items.iter().any(|d| d.message == message)
                            {
                                continue;
                            }
                            diags.error_basic(line, &message, file);
                        }
                        if profile.known_first_word_pass1 && super::_is_generic_unparsed(u) {
                            if known.is_none() {
                                known = Some(ctx.known_names().clone());
                                operands = pending_call!(super::pass1_operands(program); profile);
                            }
                            let findings: Vec<(i64, String, String)> = pending_result!(super::_instruction_arg_findings(text, &operands);
                                "part6::_instruction_arg_findings_with must return Vec<(i64, String, String)>"; profile, ctx.declared_variable_names());
                            for (stage, kind, arg_text) in &findings {
                                if *stage == 1 {
                                    let mut base = StmtBase::new(line, file);
                                    base.text.clone_from(arg_text);
                                    let echo = Stmt::Unparsed(Unparsed {
                                        base,
                                        ..Unparsed::default()
                                    });
                                    diags.error_basic(line, &pending_call!(super::_pass1_message(&echo, kind, None); profile), file);
                                } else if *stage == 3 || (*stage == 5 && findings[0].0 == 4) {
                                    diags.error_basic(line, kind, file);
                                } else if *stage == 4 {
                                    diags.error_basic(
                                        line,
                                        &format!("Invalid symbol in expression: {arg_text}."),
                                        file,
                                    );
                                }
                            }
                            let mut prefix = program.replaced();
                            prefix.statements = program.statements[..pos].to_vec();
                            let prefix = Rc::new(prefix);
                            let earlier = pending_call!(super::pass1_operands(&prefix); profile);
                            let scan_stmt =
                                pending_call!(super::_inline_condition_stmt(stmt); profile);
                            let mut verdict = if findings.is_empty() {
                                pending_call!(super::_pass1_scan(&scan_stmt.base().text, &earlier); profile)
                            } else {
                                None
                            };
                            let prefix_ctx = context::of(&prefix, Some(profile), None);
                            let later: BTreeSet<String> = ctx
                                .declared_variable_names()
                                .difference(prefix_ctx.declared_variable_names())
                                .cloned()
                                .collect();
                            let here = known
                                .as_ref()
                                .unwrap()
                                .names
                                .difference(&later)
                                .cloned()
                                .collect();
                            if verdict.is_none()
                                && u.reason == crate::parser::helpers::UNCLOSED_PAREN_EXPR
                                && super::_unclosed_paren_kind(text, &here) == "pass1"
                            {
                                verdict = Some((
                                    "pass1".to_owned(),
                                    if pystr::rstrip(text).ends_with('(') {
                                        "fallback"
                                    } else {
                                        "no_matching"
                                    }
                                    .to_owned(),
                                ));
                            }
                            if let Some((stage, kind)) = verdict {
                                if stage == "pass1" {
                                    for message in pending_call!(super::_unparsed_string_messages(u, program); profile)
                                    {
                                        diags.error_basic(line, &message, file);
                                    }
                                    diags.error_basic(line, &pending_call!(super::_pass1_message(&scan_stmt, &kind, Some(earlier.clone())); profile), file);
                                } else if stage == "pass2"
                                    && kind == "symbol"
                                    && SELECT.match_(text).is_some()
                                {
                                    cond_depth += 1;
                                    select_depth += 1;
                                }
                            }
                        }
                    }
                }
            }
        } else if let Stmt::Assignment(a) = stmt.as_ref()
            && let Some(Expr::Member(m)) = a.target.as_deref()
            && let Some(Expr::Name(n)) = m.base.as_deref()
            && pystr::upper(&n.name) == "PUBLIC"
            && !context::of(program, None, None)
                .declared_variable_names()
                .contains(&pystr::upper(&m.member))
        {
            diags.error_basic(
                line,
                &format!("Could not find Public.{}(1).", m.member),
                file,
            );
            diags.error_basic(
                line,
                &pending_call!(super::_pass1_message(stmt, "invalid_assignment", None); profile),
                file,
            );
        } else if let Stmt::Assignment(a) = stmt.as_ref()
            && let Some(Expr::Member(m)) = a.target.as_deref()
            && let Some(Expr::Name(n)) = m.base.as_deref()
            && pystr::upper(&n.name) == "DIM"
        {
            let base = &n.name;
            let message = if profile.single_pass {
                format!("Could not find DataTable {base}.")
            } else {
                format!("Could not find {base}.")
            };
            diags.error_basic(line, &message, file);
            diags.error_basic(
                line,
                &pending_call!(super::_pass1_message(stmt, "invalid_assignment", None); profile),
                file,
            );
        } else if let Stmt::Assignment(a) = stmt.as_ref()
            && (profile.known_first_word_pass1 || profile.single_pass)
            && let Some(Expr::Name(n)) = a.target.as_deref()
            && !profile.instructions().contains(&pystr::upper(&n.name))
            && (seen_consts.contains(&pystr::upper(&n.name))
                || (!context::of(program, None, None)
                    .declared_variable_names()
                    .contains(&pystr::upper(&n.name))
                    && (profile.symbols().contains(&pystr::upper(&n.name))
                        || expressions::part1::_EXTRA_PREDEFINED_SYMBOLS
                            .contains(&pystr::upper(&n.name).as_str())
                        || super::_assigns_user_const(a, program))))
        {
            diags.error_basic(
                line,
                &pending_call!(super::_pass1_message(stmt, "invalid_assignment", None); profile),
                file,
            );
        } else if super::_assigns_const_index(stmt, &seen_consts, &predefined_constants) {
            diags.error_basic(
                line,
                &pending_call!(super::_pass1_message(stmt, "fallback", None); profile),
                file,
            );
        } else if let Stmt::Call(c) = stmt.as_ref()
            && profile.known_first_word_pass1
            && !c.explicit
            && !profile.instructions().contains(&pystr::upper(&c.name))
            && !profile.functions().contains(&pystr::upper(&c.name))
        {
            if known.is_none() {
                known = Some(ctx.known_names().clone());
                operands = pending_call!(super::pass1_operands(program); profile);
            }
            if operands.contains(&pystr::upper(&c.name))
                && !ctx.declared_subs().contains(&pystr::upper(&c.name))
            {
                diags.error_basic(
                    line,
                    &pending_call!(super::_out_of_place(text, stmt.at_eol()); profile),
                    file,
                );
            }
        } else if kw == "ALIAS"
            && let Stmt::Instruction(i) = stmt.as_ref()
        {
            if known.is_none() {
                known = Some(ctx.known_names().clone());
                operands = pending_call!(super::pass1_operands(program); profile);
            }
            let src = i.args.first().and_then(Option::as_deref);
            let src_name = match src {
                Some(Expr::Name(n)) => Some(&n.name),
                Some(Expr::Index(i)) => Some(&i.name),
                _ => None,
            };
            if src_name.is_some_and(|n| alias_state.1.contains(&pystr::upper(n))) {
                diags.error_basic(line, "Cannot Alias an Alias.", file);
            } else if let Some(Expr::Number(n)) = i.args.get(1).and_then(Option::as_deref) {
                diags.error_basic(
                    line,
                    &format!("Variable name {} illegal, starts with a number.", n.text),
                    file,
                );
            } else {
                let remove = scopes[&stmt_id(stmt)]
                    .1
                    .difference(&decl_variables)
                    .cloned()
                    .collect();
                let here = known.as_ref().unwrap().without_names(&remove);
                let variables = decl_variables
                    .union(&alias_parameter_names)
                    .cloned()
                    .collect();
                let check_variables = matches!(src, Some(Expr::Index(i)) if alias_parameter_names.contains(&pystr::upper(&i.name)));
                let indexed_constants = predefined_constants
                    .iter()
                    .cloned()
                    .chain(
                        seen_consts
                            .iter()
                            .filter(|n| program.consts.contains_key(*n))
                            .cloned(),
                    )
                    .filter(|n| !alias_parameter_names.contains(n))
                    .collect();
                if !pending_call!(super::_check_alias_index_names(i, &here.names, &variables, diags, check_variables, &indexed_constants); profile, here)
                {
                    let mut view = program.replaced();
                    view.statements.clone_from(&alias_state.0);
                    if let Some(parameters) = &alias_parameters {
                        view.statements.push(parameters.clone());
                    }
                    let view = Rc::new(view);
                    let view_ctx = context::of(&view, Some(profile), Some(env.clone()));
                    let collisions = pending_call!(super::_alias_name_collisions(i, &declared_kinds, in_sub); profile);
                    let string_slots: OrderedMap<String, (Option<i64>, usize)> = pending_result!(super::_alias_string_slots(&view);
                        "part8::_alias_string_slots_with must return OrderedMap<String, (Option<i64>, usize)>"; env);
                    super::_check_alias_dims_with(
                        i,
                        diags,
                        env,
                        view_ctx.declared_dim0_sizes(),
                        view_ctx.declared_all_dims(),
                        &mut alias_state.1,
                        &mut alias_state.2,
                        Some(&collisions),
                        Some(profile),
                        Some(&string_slots),
                        in_sub,
                    );
                }
            }
        }
        if matches!(stmt.as_ref(), Stmt::If(_))
            && let Some(hashed) = crate::syntax::hash_statement(stmt)
            && let Stmt::Unparsed(u) = hashed.as_ref()
        {
            diags.error_basic(
                u.base.node.line,
                &super::_junk_init_findings_with(u, profile)[0],
                &u.base.node.file,
            );
        }
        if profile.known_first_word_pass1 && matches!(stmt.as_ref(), Stmt::If(_) | Stmt::ElseIf(_))
        {
            if known.is_none() {
                known = Some(ctx.known_names().clone());
                operands = pending_call!(super::pass1_operands(program); profile);
            }
            pending_call!(super::_check_if_pass1(&operands, diags); stmt, profile, program);
            if let Stmt::If(i) = stmt.as_ref() {
                for sub in i
                    .inline
                    .iter()
                    .flatten()
                    .chain(i.inline_else.iter().flatten())
                {
                    let kind = if matches!(sub.as_ref(), Stmt::Assignment(a) if matches!(a.target.as_deref(), Some(Expr::Name(n)) if seen_consts.contains(&pystr::upper(&n.name))))
                    {
                        Some("invalid_assignment")
                    } else if super::_assigns_const_index(sub, &seen_consts, &predefined_constants)
                    {
                        Some("fallback")
                    } else {
                        None
                    };
                    if let Some(kind) = kind {
                        let echo = replace_stmt(sub, |s| s.base().at_eol.set(false));
                        diags.error_basic(
                            sub.base().node.line,
                            &pending_call!(super::_pass1_message(&echo, kind, None); profile),
                            &sub.base().node.file,
                        );
                    }
                }
            }
        } else if profile.single_pass
            && let Stmt::If(i) = stmt.as_ref()
        {
            for sub in i
                .inline
                .iter()
                .flatten()
                .chain(i.inline_else.iter().flatten())
            {
                let messages = if let Stmt::Unparsed(u) = sub.as_ref() {
                    pending_call!(super::_routine_arg_findings(u, program); profile)
                } else {
                    vec![]
                };
                let sub_line = sub.base().node.line;
                let sub_file = &sub.base().node.file;
                if !messages.is_empty() {
                    for message in messages {
                        diags.error_basic(sub_line, &message, sub_file);
                    }
                } else if super::_assigns_const_index(sub, &seen_consts, &predefined_constants) {
                    let echo = replace_stmt(sub, |s| s.base().at_eol.set(false));
                    diags.error_basic(
                        sub_line,
                        &pending_call!(super::_pass1_message(&echo, "fallback", None); profile),
                        sub_file,
                    );
                } else if super::_block_keyword_call(sub) {
                    diags.error_basic(
                        sub_line,
                        &format!("Incomplete expression: {}.", sub.base().text),
                        sub_file,
                    );
                } else if let Stmt::Assignment(a) = sub.as_ref()
                    && let Some(Expr::Name(n)) = a.target.as_deref()
                    && !profile.instructions().contains(&pystr::upper(&n.name))
                    && (seen_consts.contains(&pystr::upper(&n.name))
                        || ((profile.symbols().contains(&pystr::upper(&n.name))
                            || expressions::part1::_EXTRA_PREDEFINED_SYMBOLS
                                .contains(&pystr::upper(&n.name).as_str()))
                            && !ctx
                                .declared_variable_names()
                                .contains(&pystr::upper(&n.name))))
                {
                    let echo = replace_stmt(sub, |s| s.base().at_eol.set(false));
                    diags.error_basic(sub_line, &pending_call!(super::_pass1_message(&echo, "invalid_assignment", None); profile), sub_file);
                }
            }
        }
        if !deferred
            && !profile.single_pass
            && matches!(stmt.as_ref(), Stmt::Assignment(_) | Stmt::Call(_))
        {
            if all_known.is_none() {
                all_known = Some(ctx.known_names().clone());
            }
            let all_known = all_known.as_ref().unwrap();
            let parts: Vec<&ExprRef> = match stmt.as_ref() {
                Stmt::Assignment(a) => a.target.iter().chain(a.value.iter()).collect(),
                Stmt::Call(c) => c.args.iter().flatten().collect(),
                _ => vec![],
            };
            let view = expressions::part1::KnownView {
                names: &all_known.names,
                tables: &all_known.tables,
                dotted_tables: all_known.dotted_tables,
                out_of_scope: &all_known.out_of_scope,
            };
            deferred = parts
                .iter()
                .any(|e| !expressions::_undefined_names_with(Some(e), &view).is_empty())
                || matches!(stmt.as_ref(), Stmt::Call(c) if !c.explicit && !all_known.names.contains(&pystr::upper(&c.name)));
        }
        if let Stmt::If(i) = stmt.as_ref()
            && i.inline.is_some()
            && select_depth == 0
        {
            for sub in i
                .inline
                .iter()
                .flatten()
                .chain(i.inline_else.iter().flatten())
            {
                if let Stmt::Case(c) = sub.as_ref()
                    && !super::_is_case_else(c)
                {
                    diags.error_basic(
                        sub.base().node.line,
                        "Case outside Select.",
                        &sub.base().node.file,
                    );
                }
            }
        }
        if kw == "BEGINPROG" {
        } else if kw == "SCAN" {
            scan_depth += 1;
        } else if kw == "NEXTSCAN"
            || (kw == "NEXT"
                && pending_call!(super::_block_word(); args).as_deref() == Some("SCAN"))
        {
            if scan_depth > 0 {
                scan_depth -= 1;
            }
        } else if kw == "SELECT"
            || (kw == "IF" && !matches!(stmt.as_ref(), Stmt::If(i) if i.inline.is_some()))
        {
            cond_depth += 1;
            select_depth += i64::from(kw == "SELECT");
        } else if kw == "ENDSELECT"
            || (kw == "END"
                && pending_call!(super::_block_word(); args).as_deref() == Some("SELECT"))
        {
            cond_depth = 0.max(cond_depth - 1);
            if select_depth == 0 && !deferred {
                diags.error_basic(line, "EndSelect w/o Select", file);
            }
            select_depth = 0.max(select_depth - 1);
        } else if let Stmt::Case(c) = stmt.as_ref()
            && !super::_is_case_else(c)
            && select_depth == 0
        {
            diags.error_basic(line, "Case outside Select.", file);
        } else if kw == "ENDIF"
            || (kw == "END" && pending_call!(super::_block_word(); args).as_deref() == Some("IF"))
        {
            cond_depth = 0.max(cond_depth - 1);
        } else if kw == "DATATABLE" && !in_table {
            in_table = true;
        } else if kw == "DATATABLE" && in_table && nested_table_line.is_none() {
            nested_table_line = Some(line);
        } else if kw == "ENDTABLE" {
            in_table = false;
        }
    }
    if !diags.errors().is_empty() {
        return;
    }
    if let Some(line) = nested_table_line
        && profile.single_pass
    {
        diags.error_basic(line, "DataTable without EndTable.", "");
    }
}

/// Port of checker/structure.py::_DO_ALIASES
pub static _DO_ALIASES: LazyLock<OrderedMap<&'static str, &'static str>> = LazyLock::new(|| {
    let mut aliases = OrderedMap::new();
    aliases.insert("WHILE", "DO");
    aliases.insert("WEND", "LOOP");
    aliases
});

/// Port of checker/structure.py::_expr_resolves
pub fn _expr_resolves(_e: Option<&Expr>, _declared: &BTreeSet<String>) -> bool {
    todo!("checker/structure.py::_expr_resolves (omitted: functions: untyped)")
}

/// Port of checker/structure.py::_expr_resolves
pub fn _expr_resolves_with(
    e: Option<&Expr>,
    declared: &BTreeSet<String>,
    functions: &BTreeSet<String>,
) -> bool {
    match e {
        None | Some(Expr::Number(_) | Expr::String(_)) => true,
        Some(Expr::Name(n)) => declared.contains(&pystr::upper(&n.name)),
        Some(Expr::Unary(u)) => _expr_resolves_with(u.operand.as_deref(), declared, functions),
        Some(Expr::Binary(b)) => {
            _expr_resolves_with(b.left.as_deref(), declared, functions)
                && _expr_resolves_with(b.right.as_deref(), declared, functions)
        }
        Some(Expr::Index(i)) => {
            functions.contains(&pystr::upper(&i.name))
                && !declared.contains(&pystr::upper(&i.name))
                && i.args
                    .iter()
                    .all(|a| _expr_resolves_with(a.as_deref(), declared, functions))
        }
        _ => false,
    }
}

/// Port of checker/structure.py::_slowsequence_resolves
pub fn _slowsequence_resolves(stmt: &Stmt, declared: &BTreeSet<String>) -> bool {
    _slowsequence_resolves_with(stmt, declared, &BTreeSet::new())
}

/// Port of checker/structure.py::_slowsequence_resolves
pub fn _slowsequence_resolves_with(
    stmt: &Stmt,
    declared: &BTreeSet<String>,
    functions: &BTreeSet<String>,
) -> bool {
    match stmt {
        Stmt::Instruction(i) => i
            .args
            .iter()
            .flatten()
            .all(|a| _expr_resolves_with(Some(a.as_ref()), declared, functions)),
        _ => true,
    }
}

/// Port of checker/structure.py::open_program_at_slowsequence
pub fn open_program_at_slowsequence(program: &Program, profile: &Profile) -> ProgramRef {
    // This legacy signature cannot return the caller's Rc on the unchanged path.
    open_program_at_slowsequence_with(
        &Rc::new(program.clone()),
        crate::profiles::get(profile.name).expect("registered profile"),
    )
}

/// Port of checker/structure.py::open_program_at_slowsequence
pub fn open_program_at_slowsequence_with(
    program: &ProgramRef,
    profile: &'static Profile,
) -> ProgramRef {
    static SLOW: LazyLock<pyre::Regex> =
        LazyLock::new(|| pyre::Regex::new(r"\s*SlowSequence\b", pyre::I));
    let mut found = None;
    for (n, st) in program.statements.iter().enumerate() {
        let kw = super::_keyword(st);
        if kw == "BEGINPROG" || kw == "ENDPROG" {
            return program.clone();
        }
        if matches!(st.as_ref(), Stmt::Unparsed(_)) && SLOW.match_(&st.base().text).is_some() {
            return program.clone();
        }
        if kw == "SLOWSEQUENCE" {
            found = Some((n, st));
            break;
        }
    }
    let Some((n, st)) = found else {
        return program.clone();
    };
    if program
        .statements
        .iter()
        .any(|s| super::_keyword(s) == "BEGINPROG")
    {
        return program.clone();
    }
    let declared = if ["cr300", "cr350"].contains(&profile.name) {
        context::of(program, Some(profile), None)
            .declared_names()
            .iter()
            .map(|x| pystr::upper(x))
            .collect()
    } else {
        BTreeSet::new()
    };
    if ["cr300", "cr350"].contains(&profile.name) {
        let functions = profile
            .functions()
            .iter()
            .map(|(name, _)| name.to_owned())
            .collect();
        if !_slowsequence_resolves_with(st, &declared, &functions) {
            return program.clone();
        }
    }
    let mut base = StmtBase::new(st.base().node.line, &st.base().node.file);
    "BeginProg".clone_into(&mut base.text);
    let begin = Rc::new(Stmt::Instruction(Instruction {
        base,
        keyword: "BEGINPROG".to_owned(),
        ..Instruction::default()
    }));
    let mut result = program.replaced();
    result.statements.insert(n, begin);
    Rc::new(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syntax::{Binary, Name, NumberValue, StringLit, Unary};

    #[test]
    fn expression_and_slowsequence_resolution_match_python() {
        // Observed from the pinned Python implementation with declared={A}, functions={ABS}.
        let name = |text: &str| {
            Rc::new(Expr::Name(Name {
                name: text.to_owned(),
                ..Name::default()
            }))
        };
        let one = Rc::new(Expr::Number(Number {
            value: NumberValue::Int(1),
            text: "1".to_owned(),
            ..Number::default()
        }));
        let index = |text: &str, arg: ExprRef| {
            Rc::new(Expr::Index(Index {
                name: text.to_owned(),
                args: vec![Some(arg)],
                ..Index::default()
            }))
        };
        let binary = |right: ExprRef| {
            Rc::new(Expr::Binary(Binary {
                op: "+".to_owned(),
                left: Some(name("a")),
                right: Some(right),
                ..Binary::default()
            }))
        };
        let exprs = [
            None,
            Some(one.clone()),
            Some(Rc::new(Expr::String(StringLit {
                value: "a".to_owned(),
                ..StringLit::default()
            }))),
            Some(name("a")),
            Some(name("Nope")),
            Some(Rc::new(Expr::Unary(Unary {
                op: "-".to_owned(),
                operand: Some(name("a")),
                ..Unary::default()
            }))),
            Some(binary(one.clone())),
            Some(binary(name("Nope"))),
            Some(index("Abs", one.clone())),
            Some(index("Abs", name("Nope"))),
            Some(index("a", one.clone())),
            Some(Rc::new(Expr::Member(Member {
                base: Some(name("a")),
                member: "x".to_owned(),
                ..Member::default()
            }))),
        ];
        let expected = [
            true, true, true, true, false, true, true, false, true, false, false, false,
        ];
        let declared = BTreeSet::from(["A".to_owned()]);
        let functions = BTreeSet::from(["ABS".to_owned()]);
        for (expr, expected) in exprs.into_iter().zip(expected) {
            assert_eq!(
                _expr_resolves_with(expr.as_deref(), &declared, &functions),
                expected
            );
            let stmt = Stmt::Instruction(Instruction {
                keyword: "SLOWSEQUENCE".to_owned(),
                args: vec![expr],
                ..Instruction::default()
            });
            assert_eq!(
                _slowsequence_resolves_with(&stmt, &declared, &functions),
                expected
            );
        }
        assert!(!_expr_resolves_with(
            Some(&index("Abs", one)),
            &BTreeSet::from(["ABS".to_owned()]),
            &functions,
        ));
        assert!(_slowsequence_resolves(
            &Stmt::Unparsed(Unparsed::default()),
            &BTreeSet::new()
        ));
    }
}
