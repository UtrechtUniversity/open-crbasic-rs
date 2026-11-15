//! Port of `checker/instr_rules.py` (lines 1-765).

#![allow(dead_code, unused_variables, unused_imports, clippy::doc_markdown)]

use crate::checker::{context, expressions, structure};
use crate::consteval::Env;
use crate::syntax::stmt_id;
use crate::{pyre, pystr};
use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;
use std::sync::LazyLock;

use crate::diagnostics::{DiagRef, Diagnostic, Diagnostics};
use crate::profiles::Profile;
use crate::syntax::{
    Assignment, Call, Case, ConstDecl, Declaration, Expr, ExprRef, Index, Instruction, Juxtaposed,
    Member, Number, OrderedMap, Program, ProgramRef, Stmt, StmtRef, SubDef, Unparsed, VarDecl,
};

static _THEN_NONEMPTY_RE: LazyLock<pyre::Regex> =
    LazyLock::new(|| pyre::Regex::new(r"(?i)\bThen\b\s*\S", 0));
static _THEN_RE: LazyLock<pyre::Regex> =
    LazyLock::new(|| pyre::Regex::new(r"(?i)\bThen\b\s*(.*)$", 0));
static _ASSIGNMENT_STRING_RE: LazyLock<pyre::Regex> = LazyLock::new(|| {
    pyre::Regex::new(r#"\s*[A-Za-z_][A-Za-z0-9_]*\s*(?:\([^\"]*\))?\s*=\s*\""#, 0)
});
static _EARLIER_ECHO_RE: LazyLock<pyre::Regex> = LazyLock::new(|| {
    pyre::Regex::new(
        r"^(?:Illegal parameter |[^:]*: )|(?:, expecting constant integer)?\.$",
        0,
    )
});
static _ESS_INITIALIZE_RE: LazyLock<pyre::Regex> =
    LazyLock::new(|| pyre::Regex::new(r"(?is)^\s*ESSInitialize\s*\((.*)\)\s*$", 0));

pub type RuleFn = fn(&Instruction, &ProgramRef, &'static Profile, &mut Diagnostics, &Env);
type RuleBuckets = OrderedMap<String, OrderedMap<String, Vec<(Option<bool>, RuleFn)>>>;
thread_local! {
    pub static RULES: RefCell<RuleBuckets> = RefCell::new(OrderedMap::new());
}

/// Port of checker/instr_rules.py::rule
pub fn rule<'a>(
    phase: &'a str,
    keywords: &'a [&'a str],
    single_pass: Option<bool>,
) -> impl Fn(RuleFn) -> RuleFn + 'a {
    move |f| {
        RULES.with(|rules| {
            let mut rules = rules.borrow_mut();
            if !rules.contains_key(phase) {
                rules.insert(phase.to_owned(), OrderedMap::new());
            }
            let mut bucket = rules.get(phase).unwrap().clone();
            for kw in keywords {
                let kw = pystr::upper(kw);
                if !bucket.contains_key(&kw) {
                    bucket.insert(kw.clone(), Vec::new());
                }
                let mut entries = bucket.get(&kw).unwrap().clone();
                entries.push((single_pass, f));
                bucket.insert(kw, entries);
            }
            rules.insert(phase.to_owned(), bucket);
        });
        f
    }
}

/// Port of checker/instr_rules.py::run_rules
pub fn run_rules(
    phase: &str,
    program: &ProgramRef,
    profile: &'static Profile,
    diags: &mut Diagnostics,
    env: &Env,
) {
    let registered = RULES.with(|rules| rules.borrow().get(phase).cloned());
    for st in &program.statements {
        let Stmt::Instruction(stmt) = st.as_ref() else {
            continue;
        };
        let kw = pystr::upper(&stmt.keyword);
        for entry in super::RULES
            .iter()
            .filter(|e| e.phase == phase && e.keywords.contains(&kw.as_str()))
        {
            if entry
                .single_pass
                .is_none_or(|gate| gate == profile.single_pass)
            {
                (entry.func)(stmt, program, profile, diags, env);
            }
        }
        if let Some(entries) = registered.as_ref().and_then(|r| r.get(&kw)) {
            for (gate, f) in entries {
                if gate.is_none_or(|gate| gate == profile.single_pass) {
                    f(stmt, program, profile, diags, env);
                }
            }
        }
    }
}

pub const _TOLERANT_CODES: &[i64] = &[4, 16, 17, 36, 37, 45, 46];
pub static _LEADING_IDENT_RE: LazyLock<pyre::Regex> =
    LazyLock::new(|| pyre::Regex::new(r"\s*([A-Za-z_][A-Za-z0-9_]*)", 0));
pub const _TOO_FEW_PARAMS_KINDS: &[&str] = &["deferred_text", "identifier"];
pub const _DECLARATION_WORDS: &[&str] = &[
    "ALIAS", "CONST", "DIM", "FUNCTION", "PUBLIC", "READONLY", "STATIC", "SUB", "UNITS",
];

/// Port of checker/instr_rules.py::unterminated_string_shape
pub fn unterminated_string_shape(
    stmt: &Stmt,
    profile: &Profile,
    subs: &BTreeSet<String>,
    operands: Option<BTreeSet<String>>,
    functions: &BTreeSet<String>,
) -> Option<String> {
    let Stmt::Unparsed(stmt) = stmt else {
        return None;
    };
    if stmt.reason != "lex error: unterminated string" {
        return None;
    }
    let word = _LEADING_IDENT_RE
        .match_(&stmt.base.text)
        .and_then(|m| m.group(1).map(pystr::upper))
        .unwrap_or_default();
    let text = &stmt.base.text;
    if word == "IF" {
        return _THEN_NONEMPTY_RE.search(text).map(|_| "if".into());
    }
    if word != "CALL"
        && !profile.single_pass
        && let Some(shape) = _function_call_string(text, functions)
    {
        return Some(shape);
    }
    if word == "CALL" || subs.contains(&word) {
        return Some("call".into());
    }
    let record = profile.instructions().get(&word);
    if _DECLARATION_WORDS.contains(&word.as_str()) {
        return None;
    }
    if let Some(record) = record {
        let params = _real_params(record.params.clone());
        let kinds = expressions::_type_code_kinds();
        let kind = params
            .first()
            .and_then(|c| usize::try_from(pystr::pymod_i64(*c, 100).unwrap()).ok())
            .and_then(|b| kinds.get(&b));
        if params.is_empty() || kind.map(String::as_str) == Some("bool_expr") {
            return None;
        }
        if !profile.single_pass
            && let Some(extra) = _past_last_param(text, i64::try_from(params.len()).unwrap())
        {
            return Some(format!("too_many:{extra}"));
        }
        return Some("instruction".into());
    }
    if _ASSIGNMENT_STRING_RE.match_(text).is_some() {
        return Some("assignment".into());
    }
    if let Some(operands) = operands
        && let Some(reached) = structure::string_reached_with(text, &operands, profile)
    {
        return Some(
            if reached == "reached" {
                "assignment"
            } else {
                "many"
            }
            .into(),
        );
    }
    None
}

pub static _FUNCTION_CALL_RE: LazyLock<pyre::Regex> =
    LazyLock::new(|| pyre::Regex::new(r"\s*(?:[A-Za-z_]\w*\s*=\s*)?([A-Za-z_]\w*)\s*\(", 0));

/// Port of checker/instr_rules.py::_function_call_string
pub fn _function_call_string(text: &str, functions: &BTreeSet<String>) -> Option<String> {
    let m = _FUNCTION_CALL_RE.match_(text)?;
    if !functions.contains(&pystr::upper(m.group(1)?)) {
        return None;
    }
    let (mut depth, mut comma, mut i) = (0, false, m.end0() - 1);
    let chars: Vec<char> = text.chars().collect();
    while i < chars.len() {
        let c = chars[i];
        if c == '"' {
            let close = chars
                .iter()
                .enumerate()
                .skip(i + 1)
                .find(|(_, c)| **c == '"')
                .map(|(j, _)| j);
            let Some(close) = close else {
                return Some(if comma { "function_arg" } else { "function" }.into());
            };
            i = close + 1;
            continue;
        }
        depth += i64::from(c == '(');
        depth -= i64::from(c == ')');
        if depth == 0 {
            let rest = pystr::slice(text, Some(i64::try_from(i + 1).unwrap()), None);
            if pystr::count(rest, "\"") != 1 {
                return None;
            }
            let before = pystr::slice(
                rest,
                None,
                Some(i64::try_from(pystr::find(rest, "\"", None, None)?).unwrap()),
            );
            return Some(
                if pystr::strip(before).is_empty() {
                    "function_tail"
                } else {
                    "function"
                }
                .into(),
            );
        }
        comma |= c == ',' && depth == 1;
        i += 1;
    }
    None
}

/// Port of checker/instr_rules.py::user_functions
pub fn user_functions(program: &Program) -> BTreeSet<String> {
    program
        .statements
        .iter()
        .filter_map(|s| match s.as_ref() {
            Stmt::SubDef(s) if pystr::upper(&s.keyword) == "FUNCTION" => {
                Some(pystr::upper(&s.name))
            }
            _ => None,
        })
        .collect()
}

/// Port of checker/instr_rules.py::_past_last_param
pub fn _past_last_param(text: &str, count: i64) -> Option<String> {
    let open_at = pystr::find(text, "(", None, None)?;
    let quote = pystr::find(text, "\"", None, None)?;
    if quote < open_at {
        return None;
    }
    let (mut depth, mut commas) = (0, Vec::new());
    for (i, c) in text.chars().enumerate().take(quote).skip(open_at + 1) {
        depth += i64::from("({".contains(c));
        depth -= i64::from(")}".contains(c));
        if c == ',' && depth == 0 {
            commas.push(i);
        }
    }
    if i64::try_from(commas.len()).unwrap() < count {
        return None;
    }
    let index = if count == 0 {
        commas.len().checked_sub(1)?
    } else {
        usize::try_from(count - 1).ok()?
    };
    let token = _next_simple_token(pystr::slice(
        text,
        Some(i64::try_from(*commas.get(index)? + 1).unwrap()),
        None,
    ));
    if token.is_empty() { None } else { Some(token) }
}

/// Port of checker/instr_rules.py::_check_unterminated_string_param
pub fn _check_unterminated_string_param(
    program: &ProgramRef,
    profile: &'static Profile,
    diags: &mut Diagnostics,
) {
    let ctx = context::of(program, None, None);
    let subs = ctx.declared_subs();
    let operands = structure::pass1_operands_with(program, profile);
    let functions = user_functions(program);
    for stmt in &program.statements {
        let shape =
            unterminated_string_shape(stmt, profile, subs, Some(operands.clone()), &functions);
        match shape.as_deref() {
            Some("instruction") => {
                diags.error_basic(stmt.line(), "Invalid or missing parameter.", stmt.file());
            }
            Some(s) if s.starts_with("too_many:") => {
                diags.error_basic(
                    stmt.line(),
                    &format!("Too many parameters:  {}.", pystr::slice(s, Some(9), None)),
                    stmt.file(),
                );
            }
            Some("call") => {
                diags.error_basic(
                    stmt.line(),
                    "Invalid or malformed function argument.",
                    stmt.file(),
                );
            }
            Some(s @ ("function" | "function_arg" | "function_tail")) => {
                diags.error_basic(stmt.line(), "String without end quote", stmt.file());
                if s == "function_arg" {
                    diags.error_basic(stmt.line(), "Call argument required.", stmt.file());
                }
                let message = if s == "function_tail" {
                    format!("Incomplete expression: {}.", stmt.text())
                } else {
                    structure::_out_of_place_with(profile, stmt.text(), stmt.at_eol())
                };
                diags.error_basic(stmt.line(), &message, stmt.file());
            }
            Some("many") => {
                diags.error_basic(
                    stmt.line(),
                    &structure::_no_matching_with(profile, stmt.text(), stmt.at_eol()),
                    stmt.file(),
                );
            }
            Some("assignment") => {
                diags.error_basic(stmt.line(), "String without end quote", stmt.file());
                diags.error_basic(
                    stmt.line(),
                    &structure::_out_of_place_with(profile, stmt.text(), stmt.at_eol()),
                    stmt.file(),
                );
            }
            Some("if") => {
                let then = _THEN_RE
                    .search(stmt.text())
                    .expect("unterminated_string_shape found Then");
                let rest = then.group(1).unwrap();
                diags.error_basic(stmt.line(), "String without end quote", stmt.file());
                diags.error_basic(
                    stmt.line(),
                    &format!("Invalid, or out of place expression: {rest}:endif."),
                    stmt.file(),
                );
            }
            _ => {}
        }
    }
}

/// Port of checker/instr_rules.py::_real_params
pub fn _real_params(params: Vec<i64>) -> Vec<i64> {
    let kinds = expressions::_type_code_kinds();
    params
        .into_iter()
        .filter(|c| {
            kinds
                .get(&usize::try_from(if *c >= 100 { *c - 100 } else { *c }).unwrap())
                .map(String::as_str)
                != Some("none")
        })
        .collect()
}

/// Port of checker/instr_rules.py::_required_count
#[allow(clippy::needless_pass_by_value)] // Preserve the existing public signature.
pub fn _required_count(params: Vec<i64>) -> i64 {
    i64::try_from(
        params
            .iter()
            .position(|c| *c >= 100)
            .unwrap_or(params.len()),
    )
    .unwrap()
}

/// Port of checker/instr_rules.py::misplaced_returns
pub fn misplaced_returns(program: &Program) -> OrderedMap<usize, bool> {
    let mut found = OrderedMap::new();
    let mut routine = None;
    for st in &context::_with_inline(program).statements {
        match st.as_ref() {
            Stmt::SubDef(s) => routine = Some(pystr::upper(&s.keyword)),
            Stmt::Instruction(s) => {
                let kw = pystr::upper(&s.keyword);
                if ["ENDSUB", "ENDFUNCTION"].contains(&kw.as_str())
                    || (kw == "END"
                        && matches!(s.args.first().and_then(Option::as_deref), Some(Expr::Name(n)) if ["SUB", "FUNCTION"].contains(&pystr::upper(&n.name).as_str())))
                {
                    routine = None;
                } else if kw == "RETURN" && routine.as_deref() != Some("FUNCTION") {
                    found.insert(stmt_id(st), routine.as_deref() == Some("SUB"));
                }
            }
            _ => {}
        }
    }
    found
}

/// Port of checker/instr_rules.py::check_return_placement
pub fn check_return_placement(program: &Program, profile: &Profile, diags: &mut Diagnostics) {
    let misplaced = misplaced_returns(program);
    for st in _return_statements(program) {
        let Some(in_sub) = misplaced.get(&stmt_id(&st)) else {
            continue;
        };
        let Stmt::Instruction(stmt) = st.as_ref() else {
            unreachable!()
        };
        if *in_sub {
            if !stmt.args.is_empty() {
                diags.error_basic(st.line(), "RETURN illegal inside Sub", st.file());
            }
            continue;
        }
        diags.error_basic(st.line(), "Return outside of Function.", st.file());
        if !stmt.args.is_empty()
            && let Some(Some((start, _))) = stmt.arg_spans.first()
        {
            let token = _next_simple_token(pystr::slice(st.text(), Some(*start), None));
            if !token.is_empty() {
                diags.error_basic(
                    st.line(),
                    &format!("Too many parameters:  {token}."),
                    st.file(),
                );
            }
        }
    }
}

/// Port of checker/instr_rules.py::_return_statements
pub fn _return_statements(program: &Program) -> Vec<StmtRef> {
    let mut result = Vec::new();
    for stmt in &program.statements {
        let mut candidates = vec![stmt];
        if let Stmt::If(s) = stmt.as_ref() {
            candidates.extend(s.inline.iter().flatten());
            candidates.extend(s.inline_else.iter().flatten());
        }
        for st in candidates {
            if matches!(st.as_ref(), Stmt::Instruction(s) if pystr::upper(&s.keyword) == "RETURN") {
                result.push(st.clone());
            }
        }
    }
    result
}

/// Port of checker/instr_rules.py::check
pub fn check(program: &ProgramRef, profile: &'static Profile, diags: &mut Diagnostics) {
    let outside = misplaced_returns(program);
    let ctx = context::of(program, Some(profile), None);
    let known = ctx.known_names();
    let kinds = expressions::_type_code_kinds();
    for st in &program.statements {
        let Stmt::Instruction(stmt) = st.as_ref() else {
            continue;
        };
        if outside.get(&stmt_id(st)) == Some(&false) {
            continue;
        }
        let Some(record) = profile.instructions().get(&pystr::upper(&stmt.keyword)) else {
            continue;
        };
        let params = _real_params(record.params.clone());
        let given = given_count(stmt, profile);
        if given >= _required_count(params.clone()) {
            continue;
        }
        let missing_code = params[usize::try_from(given).unwrap()];
        if _TOLERANT_CODES.contains(&missing_code) {
            continue;
        }
        if ["cr6", "cr1000x"].contains(&profile.name) {
            if stmt.args.iter().zip(&params).any(|(a, c)| {
                a.is_some()
                    && ![8, 27, 28, 29].contains(&pystr::pymod_i64(*c, 100).unwrap())
                    && usize::try_from(pystr::pymod_i64(*c, 100).unwrap())
                        .ok()
                        .and_then(|b| kinds.get(&b))
                        .is_some_and(|k| _TOO_MANY_SAFE_KINDS.contains(&k.as_str()))
                    && !expressions::_undefined_names_with(
                        a.as_deref(),
                        &expressions::KnownView {
                            names: &known.names,
                            tables: &known.tables,
                            out_of_scope: &known.out_of_scope,
                            dotted_tables: known.dotted_tables,
                        },
                    )
                    .is_empty()
            }) {
                continue;
            }
            _report_constant_destinations(stmt, record, &params, program, profile, diags);
        }
        if ["cr6", "cr1000x"].contains(&profile.name)
            && _earlier_error_in_stmt(
                program,
                st,
                diags,
                &["Illegal parameter ", "Variable illegal in Parameter"],
            )
        {
            continue;
        }
        let kind = kinds.get(
            &usize::try_from(if missing_code >= 100 {
                missing_code - 100
            } else {
                missing_code
            })
            .unwrap(),
        );
        diags.error_basic(
            st.line(),
            if kind.is_some_and(|k| _TOO_FEW_PARAMS_KINDS.contains(&k.as_str())) {
                "Too few parameters in instruction."
            } else {
                "Invalid or missing parameter."
            },
            st.file(),
        );
    }
    _check_unterminated_string_param(program, profile, diags);
}

/// Port of checker/instr_rules.py::given_count
pub fn given_count(stmt: &Instruction, profile: &Profile) -> i64 {
    i64::try_from(
        stmt.args
            .iter()
            .rposition(Option::is_some)
            .map_or(0, |i| i + 1),
    )
    .unwrap()
}

/// Port of checker/instr_rules.py::given_count_of
#[allow(clippy::needless_pass_by_value)] // Preserve the existing public signature.
pub fn given_count_of(args: Vec<String>) -> i64 {
    i64::try_from(
        args.iter()
            .rposition(|a| !pystr::strip(pystr::rstrip_chars(pystr::strip(a), ")")).is_empty())
            .map_or(0, |i| i + 1),
    )
    .unwrap()
}

/// Port of checker/instr_rules.py::shortfall_message
pub fn shortfall_message(record: &crate::profiles::Instruction, given: i64) -> Option<String> {
    let params = _real_params(record.params.clone());
    if given >= _required_count(params.clone()) {
        return None;
    }
    let code = params[usize::try_from(given).unwrap()];
    if _TOLERANT_CODES.contains(&code) {
        return None;
    }
    let kinds = expressions::_type_code_kinds();
    let kind = kinds.get(&usize::try_from(if code >= 100 { code - 100 } else { code }).unwrap());
    Some(
        if kind.is_some_and(|k| _TOO_FEW_PARAMS_KINDS.contains(&k.as_str())) {
            "Too few parameters in instruction."
        } else {
            "Invalid or missing parameter."
        }
        .into(),
    )
}

/// Port of checker/instr_rules.py::_report_constant_destinations
pub fn _report_constant_destinations(
    stmt: &Instruction,
    record: &crate::profiles::Instruction,
    params: &[i64],
    program: &ProgramRef,
    profile: &'static Profile,
    diags: &mut Diagnostics,
) {
    if super::PRECISION_VARIABLE_KEYWORDS.contains(&pystr::upper(&stmt.keyword).as_str()) {
        let ctx = context::of(program, Some(profile), None);
        let known = ctx.known_names();
        let variables = ctx.declared_variable_names();
        if let Some(Some(arg)) = stmt.args.first()
            && expressions::_undefined_names_with(
                Some(arg),
                &expressions::KnownView {
                    names: &known.names,
                    tables: &known.tables,
                    out_of_scope: &known.out_of_scope,
                    dotted_tables: known.dotted_tables,
                },
            )
            .is_empty()
            && expressions::_variable_refs(Some(arg), variables).is_empty()
        {
            diags.error_basic(
                stmt.base.node.line,
                &format!(
                    "Invalid Variable or Array: {}.",
                    expressions::_arg_text(stmt, 0)
                ),
                &stmt.base.node.file,
            );
        }
    }
}

pub const _TOO_MANY_EXTRA: &[&str] = &[
    "AVERAGE",
    "CALLTABLE",
    "DATATABLE",
    "ESSINITIALIZE",
    "MAXIMUM",
    "MEDIAN",
    "MINIMUM",
    "SAMPLE",
    "SENDTABLEDEF",
    "STDDEV",
    "TOTALIZE",
    "WINDVECTOR",
];
pub const ZERO_PARAM_INSTRUCTIONS: &[&str] = &[
    "ABORTSCAN",
    "ALLOWSPECIALCOMMS",
    "ANGLEDEGREES",
    "BEGINPROG",
    "CARDFLUSH",
    "CONTINUESCAN",
    "DEBUGBREAK",
    "ENDPROCHIPRI",
    "ENDPROG",
    "EXITDO",
    "EXITFOR",
    "EXITSCAN",
    "EXITSUB",
    "LEDOFF",
    "NEXTSCAN",
    "NEXTSUBSCAN",
    "OPENINTERVAL",
    "PIPELINEMODE",
    "PRESERVEVARIABLES",
    "PROCHIPRI",
    "PULSECOUNTRESET",
    "RESTART",
    "RESTORE",
    "TABLEHIDE",
    "WEND",
];
pub const _TOO_MANY_SAFE_KINDS: &[&str] = &[
    "bool_expr",
    "const_expr",
    "const_expr_only",
    "const_int_kw",
    "datatype_kw",
    "end_exit_kw",
    "expr",
    "expr_or_name",
    "expr_pair",
    "expr_special",
    "expr_typed",
    "next_kw",
    "variable_or_array",
];
pub const _TOO_MANY_KINDS: &[&str] = &[
    "bool_expr",
    "const_expr",
    "const_expr_only",
    "const_int_kw",
    "datatype_kw",
    "deferred_text",
    "end_exit_kw",
    "expr",
    "expr_or_name",
    "expr_pair",
    "expr_special",
    "expr_typed",
    "next_kw",
    "variable_or_array",
];
pub const _TOO_MANY_DELIMS: &str = ",:() =\n\t>";

/// Port of checker/instr_rules.py::_next_simple_token
pub fn _next_simple_token(text: &str) -> String {
    text.chars()
        .skip_while(|c| _TOO_MANY_DELIMS.contains(*c))
        .take_while(|c| !_TOO_MANY_DELIMS.contains(*c))
        .collect()
}

/// Port of checker/instr_rules.py::_too_many_eligible
#[allow(clippy::needless_pass_by_value)] // Preserve the existing public signature.
pub fn _too_many_eligible(params: Vec<i64>) -> bool {
    let kinds = expressions::_type_code_kinds();
    !params.is_empty()
        && params.iter().all(|c| {
            let kind = usize::try_from(if *c >= 100 { *c - 100 } else { *c })
                .ok()
                .and_then(|base| kinds.get(&base));
            kind.is_some_and(|k| k == "none" || _TOO_MANY_KINDS.contains(&k.as_str()))
        })
}

pub const _RAW_TEXT_KINDS: &[&str] = &[
    "deferred_text",
    "expr_list",
    "expr_marker",
    "for_header",
    "identifier",
    "raw_line_text",
    "sub_params",
    "type_name",
];

/// Port of checker/instr_rules.py::plain_expression_params
#[allow(clippy::needless_pass_by_value)] // Preserve the existing public signature.
pub fn plain_expression_params(params: Vec<i64>) -> bool {
    let kinds = expressions::_type_code_kinds();
    !params.is_empty()
        && !params.iter().any(|c| {
            usize::try_from(pystr::pymod_i64(*c, 100).unwrap())
                .ok()
                .and_then(|b| kinds.get(&b))
                .is_some_and(|k| _RAW_TEXT_KINDS.contains(&k.as_str()))
        })
}

/// Port of checker/instr_rules.py::check_too_many
pub fn check_too_many(program: &Program, profile: &Profile, diags: &mut Diagnostics) {
    let outside = misplaced_returns(program);
    for st in &program.statements {
        let Stmt::Instruction(stmt) = st.as_ref() else {
            continue;
        };
        if outside.get(&stmt_id(st)) == Some(&false) {
            continue;
        }
        let kw = pystr::upper(&stmt.keyword);
        let Some(record) = profile.instructions().get(&kw) else {
            continue;
        };
        if !(_too_many_eligible(record.params.clone())
            || _TOO_MANY_EXTRA.contains(&kw.as_str())
            || ["GOESDATA", "ARGOSDATA"].contains(&kw.as_str())
            || (["cr6", "cr1000x"].contains(&profile.name)
                && ZERO_PARAM_INSTRUCTIONS.contains(&kw.as_str())))
        {
            continue;
        }
        let max_count = _real_params(record.params.clone()).len();
        if stmt.args.len() <= max_count {
            continue;
        }
        let Some(Some((start, _))) = stmt.arg_spans.get(max_count) else {
            continue;
        };
        let token = _next_simple_token(pystr::slice(st.text(), Some(*start), None));
        if token.is_empty() {
            continue;
        }
        if !profile.single_pass
            && _earlier_error_in_stmt(
                program,
                st,
                diags,
                &["Illegal parameter ", "Variable illegal in Parameter"],
            )
        {
            continue;
        }
        diags.error_basic(
            st.line(),
            &format!("Too many parameters:  {token}."),
            st.file(),
        );
    }
}

/// Port of checker/instr_rules.py::_earlier_error_in_stmt
pub fn _earlier_error_in_stmt(
    program: &Program,
    stmt: &StmtRef,
    diags: &Diagnostics,
    prefixes: &[&str],
) -> bool {
    let here: Vec<_> = diags
        .items
        .iter()
        .filter(|d| {
            d.line == stmt.line()
                && d.file == stmt.file()
                && pystr::startswith_any(&d.message, prefixes)
        })
        .collect();
    if here.is_empty() {
        return false;
    }
    if program
        .statements
        .iter()
        .filter(|o| o.line() == stmt.line() && o.file() == stmt.file())
        .count()
        < 2
    {
        return true;
    }
    for d in here {
        if d.stmt != 0 {
            if d.stmt == stmt_id(stmt) {
                return true;
            }
            continue;
        }
        let echo = _EARLIER_ECHO_RE.sub("", &d.message);
        if stmt.text().contains(pystr::strip(&echo)) {
            return true;
        }
    }
    false
}

/// Port of checker/instr_rules.py::check_essinitialize_too_many
pub fn check_essinitialize_too_many(program: &Program, profile: &Profile, diags: &mut Diagnostics) {
    for st in &program.statements {
        if !matches!(st.as_ref(), Stmt::Unparsed(_)) {
            continue;
        }
        let Some(m) = _ESS_INITIALIZE_RE.match_(st.text()) else {
            continue;
        };
        let body = m.group(1).unwrap();
        let (mut depth, mut quoted, mut split) = (0, false, None);
        for (i, c) in body.chars().enumerate() {
            if c == '"' {
                quoted = !quoted;
            } else if !quoted {
                match c {
                    '(' => depth += 1,
                    ')' => depth -= 1,
                    ',' if depth == 0 => {
                        split = Some(i);
                        break;
                    }
                    _ => {}
                }
            }
        }
        let Some(split) = split else {
            continue;
        };
        let token = _next_simple_token(pystr::slice(
            body,
            Some(i64::try_from(split + 1).unwrap()),
            None,
        ));
        if !token.is_empty() {
            diags.error_basic(
                st.line(),
                &format!("Too many parameters:  {token}."),
                st.file(),
            );
        }
    }
}

pub static _SNMP_SPACED_OID: LazyLock<pyre::Regex> = LazyLock::new(|| {
    pyre::Regex::new(
        r"^\s*SNMPVARIABLE\s*\(\s*[^,]+\s*,\s*(\d[\d.]*\.[\d.]*)[ \t]+[\d.]+(?=[ \t,)])",
        pyre::I,
    )
});
pub static _SNMP_OID_DEST: LazyLock<pyre::Regex> =
    LazyLock::new(|| pyre::Regex::new(r"^\s*SNMPVARIABLE\s*\(\s*(\d+(?:\.\d+)*)\s*,", pyre::I));
pub static _SNMP_TAIL: LazyLock<pyre::Regex> = LazyLock::new(|| {
    pyre::Regex::new(
        r"^\s*SNMPVARIABLE\s*\(\s*[^,]+,\s*\d+(?:\.\d+)*\s*,",
        pyre::I,
    )
});
pub const _SNMP_TYPES: &[&str] = &[
    "COUNTER",
    "FLOAT",
    "GAUGE",
    "INTEGER",
    "OPAQUE",
    "STRING",
    "TIMETICKS",
];
pub static _SNMP_OID_LITERAL: LazyLock<pyre::Regex> = LazyLock::new(|| {
    pyre::Regex::new(r"^\s*SNMPVARIABLE\s*\(\s*[^,]+,\s*([\d.]+)\s*[,)]", pyre::I)
});
pub const _SNMP_FLOOR: [i64; 8] = [1, 3, 6, 1, 2, 1, 1, 7];

/// Port of checker/instr_rules.py::_snmp_components
pub fn _snmp_components(text: &str) -> Option<Vec<Option<i64>>> {
    let mut parts = Vec::new();
    for p in text.split('.') {
        if pystr::len(p) > 5 {
            return None;
        }
        let value = if p.is_empty() {
            None
        } else {
            Some(pystr::parse_int(p)?)
        };
        if value.is_some_and(|v| v > 65535) {
            return None;
        }
        parts.push(value);
    }
    Some(parts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    const OBSERVED: &str = r#"{"text": [["", "", null, [null, null, null]], [" ,:() =\n\t>écho rest", "écho", null, [null, null, null]], ["(1+2),3", "1+2", null, [null, null, null]], ["\"a b\",x", "\"a", null, [null, null, null]], ["F(\"abc", "F", "function", [null, null, null]], ["F(1,\"abc", "F", "function_arg", ["\"abc", null, null]], ["F(G(1,2),\"é", "F", "function_arg", ["\"é", null, null]], ["F(\"x\")\"é", "F", "function_tail", [null, null, null]], ["F(\"x\") + \"é", "F", "function", [null, null, null]], ["N = F(\"a\", \"b", "N", "function_arg", [null, null, null]], ["F(\"x\")", "F", null, [null, null, null]], ["G(\"a", "G", null, [null, null, null]], ["FieldNames(S1,S2,S3\"", "FieldNames", null, ["S2", "S3\"", null]], ["X({1,2},y,\"a", "X", null, ["y", "\"a", null]], ["X(é,ß,\"a", "X", null, ["ß", "\"a", null]]], "params": [[[], [], 0, false, false], [[14], [], 1, true, true], [[32, 11, 108], [11, 108], 2, true, false], [[4], [4], 1, true, true], [[1, 8, 123], [1, 8, 123], 2, false, true], [[6], [6], 1, false, false], [[11], [11], 1, true, false], [[18], [18], 1, false, false], [[45], [45], 1, false, false], [[26, 27, 129], [26, 27, 129], 2, true, true], [[36, 46], [], 2, true, true], [[37], [37], 1, false, false], [[100], [100], 0, false, true], [[999], [999], 0, false, true]], "snmp": [["", [""]], [".", ["", ""]], ["1..3.", [1, "", 3, ""]], ["1.03.65535", [1, 3, 65535]], ["65536", null], ["000001", null], ["0.0", [0, 0]], ["1.65535.2", [1, 65535, 2]]], "programs": [["cr6", "Battery", [[[1, "Invalid or missing parameter."]], [], [], []]], ["cr6", "CallTable", [[[1, "Too few parameters in instruction."]], [], [], []]], ["cr6", "Histogram", [[[1, "Too few parameters in instruction."]], [], [], []]], ["cr6", "Average(1,X,)", [[[1, "Invalid or missing parameter."]], [], [], []]], ["cr6", "Sample(1,X,,)", [[[1, "Invalid or missing parameter."]], [], [], []]], ["cr6", "PortSet(C1,1,1,2)", [[], [], [[1, "Too many parameters:  2."]], []]], ["cr6", "BeginProg junk", [[], [], [[1, "Too many parameters:  junk."]], []]], ["cr6", "Return(1)", [[], [[1, "Return outside of Function."], [1, "Too many parameters:  1."]], [], []]], ["cr6", "Sub S\nReturn\nReturn(1)\nEndSub\nFunction F\nReturn(2)\nEndFunction\nReturn(3)", [[[2, "Invalid or missing parameter."]], [[3, "RETURN illegal inside Sub"], [8, "Return outside of Function."], [8, "Too many parameters:  3."]], [], []]], ["cr6", "If True Then Return(1) Else Return(2)", [[], [[1, "Return outside of Function."], [1, "Too many parameters:  1."], [1, "Return outside of Function."], [1, "Too many parameters:  2."]], [], []]], ["cr6", "HTTPOut(\"é", [[[1, "Invalid or missing parameter."]], [], [], []]], ["cr6", "S = \"é", [[[1, "String without end quote"], [1, "Invalid, or out of place expression: S = \"é."]], [], [], []]], ["cr6", "If X Then S = \"é", [[[1, "String without end quote"], [1, "Invalid, or out of place expression: S = \"é:endif."]], [], [], []]], ["cr6", "Function F\nEndFunction\nN = F(1,\"é", [[[3, "String without end quote"], [3, "Call argument required."], [3, "Invalid, or out of place expression: N = F(1,\"é."]], [], [], []]], ["cr6", "ESSInitialize(1,\"é)", [[], [], [], [[1, "Too many parameters:  \"é."]]]], ["cr6", "Public X\nAddPrecise(1)", [[[2, "Invalid Variable or Array: 1."], [2, "Invalid or missing parameter."]], [], [], []]], ["cr6", "Public X\nSample(1,X,FP2,False,7)", [[], [], [[2, "Too many parameters:  7."]], []]], ["cr6", "Public X\nPortSet(C1,1,x):PortSet(C1,1,1,2)", [[], [], [[2, "Too many parameters:  2."]], []]], ["cr1000x", "Battery", [[[1, "Invalid or missing parameter."]], [], [], []]], ["cr1000x", "CallTable", [[[1, "Too few parameters in instruction."]], [], [], []]], ["cr1000x", "Histogram", [[[1, "Too few parameters in instruction."]], [], [], []]], ["cr1000x", "Average(1,X,)", [[[1, "Invalid or missing parameter."]], [], [], []]], ["cr1000x", "Sample(1,X,,)", [[[1, "Invalid or missing parameter."]], [], [], []]], ["cr1000x", "PortSet(C1,1,1,2)", [[], [], [[1, "Too many parameters:  2."]], []]], ["cr1000x", "BeginProg junk", [[], [], [[1, "Too many parameters:  junk."]], []]], ["cr1000x", "Return(1)", [[], [[1, "Return outside of Function."], [1, "Too many parameters:  1."]], [], []]], ["cr1000x", "Sub S\nReturn\nReturn(1)\nEndSub\nFunction F\nReturn(2)\nEndFunction\nReturn(3)", [[[2, "Invalid or missing parameter."]], [[3, "RETURN illegal inside Sub"], [8, "Return outside of Function."], [8, "Too many parameters:  3."]], [], []]], ["cr1000x", "If True Then Return(1) Else Return(2)", [[], [[1, "Return outside of Function."], [1, "Too many parameters:  1."], [1, "Return outside of Function."], [1, "Too many parameters:  2."]], [], []]], ["cr1000x", "HTTPOut(\"é", [[[1, "Invalid or missing parameter."]], [], [], []]], ["cr1000x", "S = \"é", [[[1, "String without end quote"], [1, "Invalid, or out of place expression: S = \"é."]], [], [], []]], ["cr1000x", "If X Then S = \"é", [[[1, "String without end quote"], [1, "Invalid, or out of place expression: S = \"é:endif."]], [], [], []]], ["cr1000x", "Function F\nEndFunction\nN = F(1,\"é", [[[3, "String without end quote"], [3, "Call argument required."], [3, "Invalid, or out of place expression: N = F(1,\"é."]], [], [], []]], ["cr1000x", "ESSInitialize(1,\"é)", [[], [], [], [[1, "Too many parameters:  \"é."]]]], ["cr1000x", "Public X\nAddPrecise(1)", [[[2, "Invalid Variable or Array: 1."], [2, "Invalid or missing parameter."]], [], [], []]], ["cr1000x", "Public X\nSample(1,X,FP2,False,7)", [[], [], [[2, "Too many parameters:  7."]], []]], ["cr1000x", "Public X\nPortSet(C1,1,x):PortSet(C1,1,1,2)", [[], [], [[2, "Too many parameters:  2."]], []]], ["cr300", "Battery", [[[1, "Invalid or missing parameter."]], [], [], []]], ["cr300", "CallTable", [[[1, "Too few parameters in instruction."]], [], [], []]], ["cr300", "Histogram", [[[1, "Too few parameters in instruction."]], [], [], []]], ["cr300", "Average(1,X,)", [[[1, "Invalid or missing parameter."]], [], [], []]], ["cr300", "Sample(1,X,,)", [[[1, "Invalid or missing parameter."]], [], [], []]], ["cr300", "PortSet(C1,1,1,2)", [[], [], [[1, "Too many parameters:  1."]], []]], ["cr300", "BeginProg junk", [[], [], [], []]], ["cr300", "Return(1)", [[], [[1, "Return outside of Function."], [1, "Too many parameters:  1."]], [], []]], ["cr300", "Sub S\nReturn\nReturn(1)\nEndSub\nFunction F\nReturn(2)\nEndFunction\nReturn(3)", [[[2, "Invalid or missing parameter."]], [[3, "RETURN illegal inside Sub"], [8, "Return outside of Function."], [8, "Too many parameters:  3."]], [], []]], ["cr300", "If True Then Return(1) Else Return(2)", [[], [[1, "Return outside of Function."], [1, "Too many parameters:  1."], [1, "Return outside of Function."], [1, "Too many parameters:  2."]], [], []]], ["cr300", "HTTPOut(\"é", [[[1, "Invalid or missing parameter."]], [], [], []]], ["cr300", "S = \"é", [[[1, "String without end quote"], [1, "Invalid, or out of place expression: S = \"é\n."]], [], [], []]], ["cr300", "If X Then S = \"é", [[[1, "String without end quote"], [1, "Invalid, or out of place expression: S = \"é:endif."]], [], [], []]], ["cr300", "Function F\nEndFunction\nN = F(1,\"é", [[], [], [], []]], ["cr300", "ESSInitialize(1,\"é)", [[], [], [], [[1, "Too many parameters:  \"é."]]]], ["cr300", "Public X\nAddPrecise(1)", [[], [], [], []]], ["cr300", "Public X\nSample(1,X,FP2,False,7)", [[], [], [[2, "Too many parameters:  7."]], []]], ["cr300", "Public X\nPortSet(C1,1,x):PortSet(C1,1,1,2)", [[], [], [[2, "Too many parameters:  x."], [2, "Too many parameters:  1."]], []]], ["cr350", "Battery", [[[1, "Invalid or missing parameter."]], [], [], []]], ["cr350", "CallTable", [[[1, "Too few parameters in instruction."]], [], [], []]], ["cr350", "Histogram", [[[1, "Too few parameters in instruction."]], [], [], []]], ["cr350", "Average(1,X,)", [[[1, "Invalid or missing parameter."]], [], [], []]], ["cr350", "Sample(1,X,,)", [[[1, "Invalid or missing parameter."]], [], [], []]], ["cr350", "PortSet(C1,1,1,2)", [[], [], [[1, "Too many parameters:  1."]], []]], ["cr350", "BeginProg junk", [[], [], [], []]], ["cr350", "Return(1)", [[], [[1, "Return outside of Function."], [1, "Too many parameters:  1."]], [], []]], ["cr350", "Sub S\nReturn\nReturn(1)\nEndSub\nFunction F\nReturn(2)\nEndFunction\nReturn(3)", [[[2, "Invalid or missing parameter."]], [[3, "RETURN illegal inside Sub"], [8, "Return outside of Function."], [8, "Too many parameters:  3."]], [], []]], ["cr350", "If True Then Return(1) Else Return(2)", [[], [[1, "Return outside of Function."], [1, "Too many parameters:  1."], [1, "Return outside of Function."], [1, "Too many parameters:  2."]], [], []]], ["cr350", "HTTPOut(\"é", [[[1, "Invalid or missing parameter."]], [], [], []]], ["cr350", "S = \"é", [[[1, "String without end quote"], [1, "Invalid, or out of place expression: S = \"é\n."]], [], [], []]], ["cr350", "If X Then S = \"é", [[[1, "String without end quote"], [1, "Invalid, or out of place expression: S = \"é:endif."]], [], [], []]], ["cr350", "Function F\nEndFunction\nN = F(1,\"é", [[], [], [], []]], ["cr350", "ESSInitialize(1,\"é)", [[], [], [], [[1, "Too many parameters:  \"é."]]]], ["cr350", "Public X\nAddPrecise(1)", [[], [], [], []]], ["cr350", "Public X\nSample(1,X,FP2,False,7)", [[], [], [[2, "Too many parameters:  7."]], []]], ["cr350", "Public X\nPortSet(C1,1,x):PortSet(C1,1,1,2)", [[], [], [[2, "Too many parameters:  x."], [2, "Too many parameters:  1."]], []]]], "shortfall": [["cr6", "Battery", ["Invalid or missing parameter.", null, null, null, null, null]], ["cr6", "CallTable", ["Too few parameters in instruction.", null, null, null, null, null]], ["cr6", "Histogram", ["Too few parameters in instruction.", "Invalid or missing parameter.", "Invalid or missing parameter.", "Invalid or missing parameter.", "Invalid or missing parameter.", "Invalid or missing parameter."]], ["cr6", "Sample", ["Invalid or missing parameter.", "Too few parameters in instruction.", "Invalid or missing parameter.", null, null, null]], ["cr6", "Do", [null, "Invalid or missing parameter.", null, null, null, null]], ["cr6", "AddPrecise", ["Invalid or missing parameter.", "Invalid or missing parameter.", null, null, null, null]], ["cr6", "EndSub", [null, null, null, null, null, null]], ["cr1000x", "Battery", ["Invalid or missing parameter.", null, null, null, null, null]], ["cr1000x", "CallTable", ["Too few parameters in instruction.", null, null, null, null, null]], ["cr1000x", "Histogram", ["Too few parameters in instruction.", "Invalid or missing parameter.", "Invalid or missing parameter.", "Invalid or missing parameter.", "Invalid or missing parameter.", "Invalid or missing parameter."]], ["cr1000x", "Sample", ["Invalid or missing parameter.", "Too few parameters in instruction.", "Invalid or missing parameter.", null, null, null]], ["cr1000x", "Do", [null, "Invalid or missing parameter.", null, null, null, null]], ["cr1000x", "AddPrecise", ["Invalid or missing parameter.", "Invalid or missing parameter.", null, null, null, null]], ["cr1000x", "EndSub", [null, null, null, null, null, null]], ["cr300", "Battery", ["Invalid or missing parameter.", null, null, null, null, null]], ["cr300", "CallTable", ["Too few parameters in instruction.", null, null, null, null, null]], ["cr300", "Histogram", ["Too few parameters in instruction.", "Invalid or missing parameter.", "Invalid or missing parameter.", "Invalid or missing parameter.", "Invalid or missing parameter.", "Invalid or missing parameter."]], ["cr300", "Sample", ["Invalid or missing parameter.", "Too few parameters in instruction.", "Invalid or missing parameter.", null, null, null]], ["cr300", "Do", [null, "Invalid or missing parameter.", null, null, null, null]], ["cr300", "EndSub", [null, null, null, null, null, null]], ["cr350", "Battery", ["Invalid or missing parameter.", null, null, null, null, null]], ["cr350", "CallTable", ["Too few parameters in instruction.", null, null, null, null, null]], ["cr350", "Histogram", ["Too few parameters in instruction.", "Invalid or missing parameter.", "Invalid or missing parameter.", "Invalid or missing parameter.", "Invalid or missing parameter.", "Invalid or missing parameter."]], ["cr350", "Sample", ["Invalid or missing parameter.", "Too few parameters in instruction.", "Invalid or missing parameter.", null, null, null]], ["cr350", "Do", [null, "Invalid or missing parameter.", null, null, null, null]], ["cr350", "EndSub", [null, null, null, null, null, null]]]}"#;

    #[test]
    fn python_observed_helpers() {
        let data: Value = serde_json::from_str(OBSERVED).unwrap();
        let functions = BTreeSet::from(["F".into()]);
        for row in data["text"].as_array().unwrap() {
            let text = row[0].as_str().unwrap();
            assert_eq!(json!(_next_simple_token(text)), row[1], "token: {text}");
            assert_eq!(
                json!(_function_call_string(text, &functions)),
                row[2],
                "function: {text}"
            );
            let past: Vec<_> = (1..=3).map(|n| _past_last_param(text, n)).collect();
            assert_eq!(json!(past), row[3], "past: {text}");
        }
        for row in data["params"].as_array().unwrap() {
            let params: Vec<i64> = serde_json::from_value(row[0].clone()).unwrap();
            assert_eq!(json!(_real_params(params.clone())), row[1]);
            assert_eq!(json!(_required_count(params.clone())), row[2]);
            assert_eq!(json!(_too_many_eligible(params.clone())), row[3]);
            assert_eq!(json!(plain_expression_params(params)), row[4]);
        }
        for row in data["snmp"].as_array().unwrap() {
            let got = _snmp_components(row[0].as_str().unwrap()).map(|parts| {
                parts
                    .into_iter()
                    .map(|v| v.map_or_else(|| json!(""), |v| json!(v)))
                    .collect::<Vec<_>>()
            });
            assert_eq!(json!(got), row[1]);
        }
        for row in data["shortfall"].as_array().unwrap() {
            let profile = crate::profiles::get(row[0].as_str().unwrap()).unwrap();
            let record = profile
                .instructions()
                .get(&pystr::upper(row[1].as_str().unwrap()))
                .unwrap();
            let got: Vec<_> = (0..6).map(|n| shortfall_message(record, n)).collect();
            assert_eq!(json!(got), row[2]);
        }
    }

    #[test]
    fn python_observed_program_checks() {
        let data: Value = serde_json::from_str(OBSERVED).unwrap();
        for row in data["programs"].as_array().unwrap() {
            let profile = crate::profiles::get(row[0].as_str().unwrap()).unwrap();
            let text = row[1].as_str().unwrap();
            let (program, _) = crate::parser::parse_text(text, "t.cr6", None, Some(profile));
            let mut results = Vec::new();
            for which in 0..4 {
                let mut diags = Diagnostics::new();
                match which {
                    0 => check(&program, profile, &mut diags),
                    1 => check_return_placement(&program, profile, &mut diags),
                    2 => check_too_many(&program, profile, &mut diags),
                    _ => check_essinitialize_too_many(&program, profile, &mut diags),
                }
                results.push(
                    diags
                        .items
                        .iter()
                        .map(|d| json!([d.line, d.message]))
                        .collect::<Vec<_>>(),
                );
            }
            assert_eq!(json!(results), row[2], "{}: {text}", profile.name);
        }
    }

    #[test]
    fn python_observed_dispatch_and_diagnostic_identity() {
        fn a(
            stmt: &Instruction,
            program: &ProgramRef,
            profile: &'static Profile,
            diags: &mut Diagnostics,
            env: &Env,
        ) {
            assert!(std::ptr::eq(profile, env.profile));
            assert!(
                program
                    .statements
                    .iter()
                    .any(|s| matches!(s.as_ref(), Stmt::Instruction(i) if std::ptr::eq(i, stmt)))
            );
            diags.error_basic(
                stmt.base.node.line,
                &format!("A:{}", stmt.keyword),
                &stmt.base.node.file,
            );
        }
        fn b(
            stmt: &Instruction,
            _: &ProgramRef,
            _: &'static Profile,
            diags: &mut Diagnostics,
            _: &Env,
        ) {
            diags.error_basic(
                stmt.base.node.line,
                &format!("B:{}", stmt.keyword),
                &stmt.base.node.file,
            );
        }
        let phase = "rules1_observed_dispatch";
        rule(phase, &["battery"], None)(a);
        rule(phase, &["BATTERY"], Some(true))(b);
        rule(phase, &["Battery"], Some(false))(a);
        for (name, expected) in [
            (
                "cr6",
                json!([
                    [2, "A:BATTERY"],
                    [2, "A:BATTERY"],
                    [3, "A:BATTERY"],
                    [3, "A:BATTERY"]
                ]),
            ),
            (
                "cr300",
                json!([
                    [2, "A:BATTERY"],
                    [2, "B:BATTERY"],
                    [3, "A:BATTERY"],
                    [3, "B:BATTERY"]
                ]),
            ),
        ] {
            let profile = crate::profiles::get(name).unwrap();
            let (program, _) = crate::parser::parse_text(
                "Public X\nBattery(X)\nBattery(X)",
                "t.cr6",
                None,
                Some(profile),
            );
            let mut diags = Diagnostics::new();
            run_rules(phase, &program, profile, &mut diags, &Env::new(profile));
            assert_eq!(
                json!(
                    diags
                        .items
                        .iter()
                        .map(|d| json!([d.line, d.message]))
                        .collect::<Vec<_>>()
                ),
                expected
            );
        }
        let profile = crate::profiles::get("cr6").unwrap();
        let (program, _) = crate::parser::parse_text(
            "PortSet(C1,1,x):PortSet(C1,1,1,2)",
            "t.cr6",
            None,
            Some(profile),
        );
        let observed = [
            (
                "Illegal parameter x.",
                [[true, false], [true, false], [false, true]],
            ),
            (
                "Variable illegal in Parameter: 2.",
                [[false, true], [true, false], [false, true]],
            ),
            (
                "Illegal parameter Missing.",
                [[false, false], [true, false], [false, true]],
            ),
        ];
        for (message, cases) in observed {
            for (tag, expected) in [None, Some(0), Some(1)].into_iter().zip(cases) {
                let mut diags = Diagnostics::new();
                diags.error_basic(1, message, "t.cr6");
                Rc::make_mut(&mut diags.items[0]).stmt =
                    tag.map_or(0, |i| stmt_id(&program.statements[i]));
                let got: Vec<_> = program
                    .statements
                    .iter()
                    .map(|s| {
                        _earlier_error_in_stmt(
                            &program,
                            s,
                            &diags,
                            &["Illegal parameter ", "Variable illegal in Parameter"],
                        )
                    })
                    .collect();
                assert_eq!(got, expected);
            }
        }
    }
}
