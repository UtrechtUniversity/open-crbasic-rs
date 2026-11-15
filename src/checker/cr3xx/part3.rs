//! Port of `checker/cr3xx.py` (lines 1328-2123).

#![allow(dead_code, unused_variables, unused_imports)]

use std::collections::{BTreeSet, HashMap, HashSet};
use std::rc::Rc;
use std::sync::LazyLock;

use crate::checker::{context, expressions, instr_rules, structure, symbols};
use crate::diagnostics::{DiagRef, Diagnostic, Diagnostics, ShowFile};
use crate::lexer;
use crate::profiles::Profile;
use crate::pyre::{self, Regex};
use crate::pystr;
use crate::syntax::{self, Expr, Program, ProgramRef, Stmt, Unparsed};

// Temporary local copies of constants owned by `part2` / `structure` until cross-part wiring.
const P3_UNRESOLVED_SUFFIX: &str = " is not defined.";
const P3_CONST_FIRST_PARAM: &[i64] = &[1, 5, 38, 39];
const P3_GENERIC_FALLBACK_PREFIX: &str = "Invalid, or out of place expression: ";
const P3_TAIL_REJECTING: &[&str] = &[
    "BEGINPROG",
    "PIPELINEMODE",
    "SEQUENTIALMODE",
    "ENDIF",
    "WEND",
    "OPENINTERVAL",
    "NEXTSCAN",
    "NEXTSUBSCAN",
    "ENDTABLE",
    "FILLSTOP",
    "EXITSCAN",
    "CONTINUESCAN",
    "EXITSUB",
    "EXITDO",
    "EXITFOR",
    "RESTORE",
    "ANGLEDEGREES",
    "SHUTDOWNBEGIN",
    "SHUTDOWNEND",
    "ENDSEQUENCE",
    "EXITFUNCTION",
    "ABORTSCAN",
    "SAMPLEFIELDCAL",
    "WAITTRIGGERSEQUENCE",
    "PULSECOUNTRESET",
    "WEBPAGEEND",
    "DHCPRENEW",
    "RESTART",
    "PRESERVEVARIABLES",
    "APPLYANDRESTARTSEQUENCE",
    "ENDAPPLYANDRESTARTSEQUENCE",
    "TABLEHIDE",
];
const P3_LEAD_OP_NO_OPERAND: &[&str] = &["IS", "MOD"];
const P3_CALL_ADJACENT_EXCLUDED: &[&str] = &[
    "DATA", "DATALONG", "ALIAS", "UNITS", "SUB", "FUNCTION", "IF", "ELSEIF", "WHILE", "UNTIL",
    "SELECT", "CASE", "FOR", "DO", "LOOP",
];
const P3_SELF_CLOSING_FUNCTIONS: &[&str] = &[
    "ABS",
    "ABSLONG",
    "ARRAYLENGTH",
    "TYPEOF",
    "LINENUM",
    "SIGNATURE",
];

static P3_LINE_WORD_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[A-Za-z_][A-Za-z0-9_]*", 0));
static P3_ZERO_TAIL_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\s*([A-Za-z_]\w*)(?=[\s,(])(.*)$", pyre::DOTALL));
static P3_LEAD_OP_WORD_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)(is|mod|and|or|xor|imp|eqv|to)(?![A-Za-z0-9_])",
        pyre::IGNORECASE,
    )
});
static P3_LEAD_OP_SKIP_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)^[ \t]*(?:(?:and|or|xor|imp|eqv|to|not|mod|is)(?![A-Za-z0-9_])[ \t]*)*",
        pyre::IGNORECASE,
    )
});
static P3_INSTRUCTION_PROSE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"\s*([A-Za-z_][A-Za-z0-9_]*)[ \t]+([A-Za-z_][^(]*)$",
        pyre::DOTALL,
    )
});
static P3_TABLE_LINE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\s*([A-Za-z_][A-Za-z0-9_]*)\.", 0));
static P3_SIGNED_TABLE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\s*[-+][ \t]*([A-Za-z_][A-Za-z0-9_]*)\.(?=[A-Za-z_])", 0));

static RE_ALIAS_LEAD: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\s*Alias\b", pyre::IGNORECASE));
static RE_DO_LOOP_LEAD: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\s*(?:DO|LOOP)\b", pyre::IGNORECASE));
static RE_CALL_PAREN_S: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\s*([A-Za-z_]\w*)\s*\((.*)$", pyre::DOTALL));
static RE_QUOTED_LIT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#""[^"]*""#, 0));
static RE_ADJ_WORDS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"[\w"]\s+[\w"]"#, 0));
static RE_WORD_OP_CARG: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\b(?:and|or|xor|eqv|imp|mod|is|to|not)\b",
        pyre::IGNORECASE,
    )
});
static RE_IF_LEAD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\s*if\b", pyre::IGNORECASE));
static RE_WORD_WS_FULL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[\w\s]*", 0));
static RE_WORD_OR_NUM: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[A-Za-z_]\w*|\d+", 0));
static RE_NOT_WS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\s*NOT\s", pyre::IGNORECASE));
static RE_NOT_LEAD: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^\s*NOT\b", pyre::IGNORECASE));
static RE_ASSIGN_OR_NOT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\s*(?:[A-Za-z_]\w*\s*=|NOT\b)", pyre::IGNORECASE));
static RE_NUM_OR_STR_ASSIGN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"\s*(?:\d[\w.]*|-\s*\d[\w.]*|"[^"]*")\s*="#, 0));
static RE_LOGICAL_ASSIGN_LEAD: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"\s*[A-Za-z_]\w*\s+(?:AND|OR|XOR|IMP|EQV)\b[^=]*[\w)]\s*=",
        pyre::IGNORECASE,
    )
});
static RE_LOGICAL_ASSIGN_ANY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b(?:AND|OR|XOR|IMP|EQV)\b[^=]*=", pyre::IGNORECASE));
static RE_CALL_OPEN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\s*([A-Za-z_]\w*)\s*\(([^()]*)", 0));
static RE_JUXT_ARG: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"[\w"]\s+(?!(?:AND|OR|XOR|MOD|IMP|EQV)\b)[A-Za-z_\d"]"#,
        pyre::IGNORECASE,
    )
});
static RE_RHS_CALL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"\s*[A-Za-z_]\w*\s*=\s*([A-Za-z_]\w*)\s*\((.*)$",
        pyre::DOTALL,
    )
});
static RE_UNDEF_WORD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\w+ is not defined\.", 0));
static RE_UNDEF_WORD_CAP: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(\w+) is not defined\.", 0));
static RE_IDENT_WORD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[A-Za-z_]\w*", 0));

fn p3_i64(n: usize) -> i64 {
    i64::try_from(n).unwrap_or(i64::MAX)
}

fn p3_usize(n: i64) -> usize {
    usize::try_from(n.max(0)).unwrap_or(0)
}

fn p3_diag(line: i64, message: &str, file: &str, show_file: bool) -> DiagRef {
    Rc::new(Diagnostic::new(
        "error",
        line,
        message,
        file,
        ShowFile::Bool(show_file),
        0,
        "",
        "",
    ))
}

fn p3_is_cr3xx(profile: &Profile) -> bool {
    matches!(profile.name, "cr300" | "cr350")
}

/// Port of `checker/cr3xx.py::check_unknown_line_words`.
pub fn check_unknown_line_words(_program: &Program, _diags: &mut Diagnostics) {
    todo!("checker/cr3xx.py::check_unknown_line_words (omitted: profile: untyped)")
}

/// Port of `checker/cr3xx.py::check_unknown_line_words`.
pub fn check_unknown_line_words_with(
    program: &ProgramRef,
    profile: &'static Profile,
    diags: &mut Diagnostics,
) {
    if !p3_is_cr3xx(profile) || diags.items.is_empty() {
        return;
    }
    let mut unparsed_lines: HashSet<(i64, String)> = HashSet::new();
    let mut unparsed_text: HashMap<(i64, String, String), &Unparsed> = HashMap::new();
    for s in &program.statements {
        if let Stmt::Unparsed(u) = &**s {
            unparsed_lines.insert((u.base.node.line, u.base.node.file.clone()));
            unparsed_text.insert(
                (
                    u.base.node.line,
                    u.base.node.file.clone(),
                    super::part2::_echo_key(&u.base.text),
                ),
                u,
            );
        }
    }
    if unparsed_lines.is_empty() {
        return;
    }
    let mut known: Option<BTreeSet<String>> = None;
    let mut first_done = false;
    let mut new_items: Vec<DiagRef> = Vec::with_capacity(diags.items.len());
    let mut changed = false;

    for orig_d in &diags.items {
        let mut d = orig_d.clone();
        let d_text_slice = if orig_d.message.len() > P3_GENERIC_FALLBACK_PREFIX.len()
            && orig_d.message.ends_with('.')
            && orig_d.message.starts_with(P3_GENERIC_FALLBACK_PREFIX)
        {
            Some(pystr::slice(
                &orig_d.message,
                Some(p3_i64(pystr::len(P3_GENERIC_FALLBACK_PREFIX))),
                Some(-1),
            ))
        } else {
            None
        };
        let is_unparsed_fallback = d.severity == "error"
            && d_text_slice.is_some_and(|t| {
                unparsed_lines.contains(&(d.line, d.file.clone()))
                    && unparsed_text.contains_key(&(
                        d.line,
                        d.file.clone(),
                        super::part2::_echo_key(t),
                    ))
            });

        if is_unparsed_fallback {
            let text = d_text_slice.unwrap();
            if known.is_none() {
                known = Some(super::part2::prose_known(program));
            }
            if !first_done {
                super::part2::first_declarations(program);
                first_done = true;
            }
            let later = super::part2::declared_later(d.line, &d.file);
            let here: BTreeSet<String> = known
                .as_ref()
                .unwrap()
                .difference(&later)
                .cloned()
                .collect();

            if RE_ALIAS_LEAD.match_(text).is_some()
                && diags.items.iter().any(|e| {
                    e.line == d.line
                        && e.file == d.file
                        && e.message.starts_with("Need '=' in alias definition")
                })
            {
                changed = true;
                continue;
            }

            let table_match = P3_TABLE_LINE_RE.match_(text);
            let signed_match = if table_match.is_some() {
                None
            } else {
                P3_SIGNED_TABLE_RE.match_(text).filter(|m| {
                    let upper = pystr::upper(m.group(1).unwrap_or(""));
                    !symbols::_ALWAYS_KNOWN_SYSTEM_TABLES.contains(&upper.as_str())
                        && !super::part2::prose_known_tables(program).contains(&upper)
                })
            };
            if let Some(table) = table_match.or(signed_match) {
                let tname = table.group(1).unwrap_or("");
                new_items.push(p3_diag(
                    d.line,
                    &format!("Could not find DataTable {tname}."),
                    &d.file,
                    true,
                ));
                changed = true;
                if super::part2::_loose_before_beginprog(program, d.line, &d.file) {
                    new_items.push(d.clone());
                    new_items.push(p3_diag(
                        d.line,
                        "Expr illegally occurs before BeginProg.",
                        &d.file,
                        true,
                    ));
                    continue;
                }
            }

            let bare = pystr::rstrip_chars(text, "\n");
            let cp = structure::condition_part(bare);
            let empty_pass1 = if cp.as_ref().is_some_and(|c| pystr::strip(c).is_empty())
                && !(RE_DO_LOOP_LEAD.match_(bare).is_some()
                    && diags
                        .items
                        .iter()
                        .any(|e| e.severity == "error" && e.line != 0 && e.line < d.line))
            {
                structure::_pass1_scan(bare, None, profile)
                    .filter(|(a, b)| a == "pass1" && (b == "missing_expr" || b == "fallback"))
            } else {
                None
            };
            if let Some((_, kind)) = empty_pass1 {
                let msg = if kind == "missing_expr" {
                    "Invalid or missing expression.".to_owned()
                } else {
                    format!("{P3_GENERIC_FALLBACK_PREFIX}.")
                };
                new_items.push(Rc::new(d.replace(|r| r.message = msg)));
                changed = true;
                continue;
            }

            if let Some(zero) = P3_ZERO_TAIL_RE.match_(bare) {
                let z1 = pystr::upper(zero.group(1).unwrap_or(""));
                if P3_TAIL_REJECTING.contains(&z1.as_str()) {
                    let tok = instr_rules::_next_simple_token(zero.group(2).unwrap_or(""));
                    if !tok.is_empty() {
                        new_items.push(p3_diag(
                            d.line,
                            &format!("Too many parameters:  {tok}."),
                            &d.file,
                            true,
                        ));
                        changed = true;
                        continue;
                    }
                }
            }

            let call = RE_CALL_PAREN_S.match_(bare);
            let call_upper = call
                .as_ref()
                .map(|c| pystr::upper(c.group(1).unwrap_or("")));
            let cinfo = call_upper
                .as_deref()
                .and_then(|u| profile.instructions().get(u));
            if let (Some(call_m), Some(ci), Some(cu)) = (&call, cinfo, call_upper.as_deref()) {
                if !ci.params.is_empty() && !P3_CALL_ADJACENT_EXCLUDED.contains(&cu) {
                    let ctx = context::of(program, Some(profile), None);
                    let vars = ctx.declared_variable_names();
                    let in_table = super::part2::_inside_table(program, d.line, &d.file);
                    if let Some(found) = super::part2::_call_adjacent_arg_messages(
                        call_m.group(2).unwrap_or(""),
                        &here,
                        vars,
                        in_table,
                    ) {
                        for message in found {
                            new_items.push(p3_diag(d.line, &message, &d.file, true));
                        }
                        changed = true;
                        continue;
                    }
                }
            }

            if let (Some(call_m), Some(ci), Some(cu)) = (&call, cinfo, call_upper.as_deref()) {
                if !ci.params.is_empty()
                    && !matches!(pystr::pymod_i64(ci.params[0], 100), Some(6 | 11))
                    && !P3_CALL_ADJACENT_EXCLUDED.contains(&cu)
                {
                    let parts = super::part2::_split_top_commas(call_m.group(2).unwrap_or(""));
                    let mut carg =
                        pystr::rstrip(parts.first().map_or("", String::as_str)).to_owned();
                    let masked = RE_QUOTED_LIT.sub("\"", &carg);
                    if !carg.is_empty()
                        && RE_ADJ_WORDS.search(&masked).is_some()
                        && RE_WORD_OP_CARG.search(&carg).is_none()
                    {
                        if carg.ends_with(')')
                            && carg.matches('(').count() < carg.matches(')').count()
                        {
                            carg = pystr::rstrip(pystr::slice(&carg, None, Some(-1))).to_owned();
                        }
                        for word in super::part2::_leading_unknown_words(&carg, &here) {
                            new_items.push(p3_diag(
                                d.line,
                                &profile.unresolved_symbol_message.format(&word),
                                &d.file,
                                true,
                            ));
                        }
                        new_items.push(p3_diag(
                            d.line,
                            &format!("Incomplete expression: {carg}."),
                            &d.file,
                            true,
                        ));
                        if P3_CONST_FIRST_PARAM.contains(&ci.params[0]) {
                            new_items.push(p3_diag(
                                d.line,
                                &format!("Variable illegal in Parameter: {carg}."),
                                &d.file,
                                true,
                            ));
                        }
                        changed = true;
                        continue;
                    }
                }
            }

            let lead = P3_INSTRUCTION_PROSE_RE.match_(text);
            let lead_upper = lead
                .as_ref()
                .map(|m| pystr::upper(m.group(1).unwrap_or("")));
            let info = lead_upper
                .as_deref()
                .and_then(|u| profile.instructions().get(u));
            if let (Some(lead_m), Some(inf), Some(lu)) = (&lead, info, lead_upper.as_deref()) {
                if !matches!(lu, "DATA" | "DATALONG")
                    && !inf.params.is_empty()
                    && pystr::strip(structure::condition_part(text).as_deref().unwrap_or(""))
                        .is_empty()
                    && structure::inline_if_condition(text)
                        .as_deref()
                        .unwrap_or("")
                        .is_empty()
                {
                    let mut rest =
                        pystr::rstrip_chars(lead_m.group(2).unwrap_or(""), "\n").to_owned();
                    let mut codes = inf.params.clone();
                    if pystr::pymod_i64(codes[0], 100) == Some(11)
                        && !rest.contains(',')
                        && codes.len() > 1
                        && codes[1] < 100
                    {
                        new_items.push(p3_diag(
                            d.line,
                            "Invalid or missing parameter.",
                            &d.file,
                            true,
                        ));
                        changed = true;
                        continue;
                    }
                    if pystr::pymod_i64(codes[0], 100) == Some(6) {
                        let name_tok = instr_rules::_next_simple_token(&rest);
                        if !name_tok.is_empty() {
                            if let Some(idx) = pystr::find(&rest, &name_tok, None, None) {
                                rest = pystr::slice(
                                    &rest,
                                    Some(p3_i64(idx + pystr::len(&name_tok))),
                                    None,
                                )
                                .to_owned();
                            }
                        }
                        codes.remove(0);
                        if codes.is_empty() {
                            let tok = instr_rules::_next_simple_token(&rest);
                            if !tok.is_empty() {
                                new_items.push(p3_diag(
                                    d.line,
                                    &format!("Too many parameters:  {tok}."),
                                    &d.file,
                                    true,
                                ));
                                changed = true;
                                continue;
                            }
                        }
                        rest = pystr::lstrip_chars(&rest, " \t").to_owned();
                    }
                    if !codes.is_empty() && !rest.is_empty() {
                        let op = P3_LEAD_OP_WORD_RE.match_(&rest);
                        let mut words = if op.is_some() {
                            Vec::new()
                        } else {
                            super::part2::_leading_unknown_words(&rest, &here)
                        };
                        let mut after = String::new();
                        if let Some(ref op_m) = op {
                            let op_upper = pystr::upper(op_m.group(1).unwrap_or(""));
                            if !P3_LEAD_OP_NO_OPERAND.contains(&op_upper.as_str()) {
                                after = P3_LEAD_OP_SKIP_RE.sub_n(
                                    "",
                                    pystr::slice(&rest, Some(p3_i64(op_m.end0())), None),
                                    1,
                                );
                                words = if P3_LINE_WORD_RE.match_(&after).is_some() {
                                    super::part2::_leading_unknown_words(&after, &here)
                                        .into_iter()
                                        .take(1)
                                        .collect()
                                } else {
                                    Vec::new()
                                };
                            }
                        }
                        for word in &words {
                            new_items.push(p3_diag(
                                d.line,
                                &profile.unresolved_symbol_message.format(word),
                                &d.file,
                                true,
                            ));
                        }
                        let msg = if op.is_some() {
                            format!("{P3_GENERIC_FALLBACK_PREFIX}{rest}.")
                        } else {
                            format!("Incomplete expression: {rest}.")
                        };
                        new_items.push(p3_diag(d.line, &msg, &d.file, true));
                        if P3_CONST_FIRST_PARAM.contains(&codes[0])
                            && !(op.is_some() && P3_LINE_WORD_RE.match_(&after).is_none())
                        {
                            new_items.push(p3_diag(
                                d.line,
                                &format!("Variable illegal in Parameter: {rest}."),
                                &d.file,
                                true,
                            ));
                        }
                        changed = true;
                        continue;
                    }
                }
            }

            let ctx = context::of(program, Some(profile), None);
            let decl_vars = ctx.declared_variable_names();
            let mut callables = decl_vars.clone();
            for (k, _) in profile.functions().iter() {
                callables.insert(k.to_owned());
            }
            let _ = &callables;
            let fw_start = pystr::len(text) - pystr::len(pystr::lstrip_chars(text, " \t"));
            let fw = P3_LINE_WORD_RE.match_at(text, fw_start);
            let needs_paren = fw.as_ref().is_some_and(|fw_m| {
                let fw_upper = pystr::upper(fw_m.group0());
                let after_fw =
                    pystr::lstrip_chars(pystr::slice(text, Some(p3_i64(fw_m.end0())), None), " \t");
                !profile.instructions().contains(&fw_upper)
                    && !after_fw.starts_with('(')
                    && !after_fw.starts_with('.')
                    && !decl_vars.contains(&fw_upper)
                    && structure::_bare_function_needs_paren(
                        &fw_upper,
                        &structure::part6::functions_for(profile),
                    )
            });

            let mut cond_text =
                structure::condition_part(text).filter(|s| !pystr::strip(s).is_empty());
            if cond_text.is_none() && RE_IF_LEAD.match_(text).is_some() {
                cond_text = structure::inline_if_condition(text).filter(|s| !s.is_empty());
            }
            let mut words =
                super::part2::_leading_unknown_words(cond_text.as_deref().unwrap_or(text), &here);
            if needs_paren {
                if let Some(ref fw_m) = fw {
                    let rest = pystr::slice(text, Some(p3_i64(fw_m.end0())), None);
                    if RE_WORD_WS_FULL.fullmatch(rest).is_some() {
                        let count = p3_usize(
                            profile
                                .functions()
                                .get(&pystr::upper(fw_m.group0()))
                                .and_then(|f| f.nargs)
                                .unwrap_or(1),
                        );
                        words = RE_WORD_OR_NUM
                            .findall(rest)
                            .into_iter()
                            .take(count)
                            .filter(|w| {
                                !w.chars().next().is_some_and(|c| c.is_ascii_digit())
                                    && !here.contains(&pystr::upper(w))
                                    && pystr::upper(w) != "IS"
                            })
                            .map(str::to_owned)
                            .collect();
                    } else {
                        words = super::part2::_leading_unknown_words(rest, &here);
                    }
                }
            } else if RE_NOT_WS.match_(text).is_some() {
                let stripped = RE_NOT_LEAD.sub("", text);
                words = super::part2::_leading_unknown_words(&stripped, &here);
            }
            for word in &words {
                new_items.push(p3_diag(
                    d.line,
                    &profile.unresolved_symbol_message.format(word),
                    &d.file,
                    true,
                ));
                changed = true;
            }
            if needs_paren {
                d = Rc::new(d.replace(|r| r.message = format!("Expression expects ( : {text}.")));
                changed = true;
            }
            if d.message.starts_with(P3_GENERIC_FALLBACK_PREFIX) {
                if let Some(stmt) =
                    unparsed_text.get(&(d.line, d.file.clone(), super::part2::_echo_key(text)))
                {
                    let mut cond = structure::condition_part(&stmt.base.text)
                        .filter(|s| !pystr::strip(s).is_empty());
                    let mut scan_text = stmt.base.text.clone();
                    let mut inline = if cond.is_none() {
                        structure::inline_if_condition(&stmt.base.text).filter(|s| !s.is_empty())
                    } else {
                        None
                    };
                    if let Some(ref inl) = inline {
                        if inl.contains(';') {
                            cond = inline.take();
                        }
                    }
                    if let Some(ref inl) = inline {
                        scan_text = format!("If {inl}Then");
                        let inline_scan = structure::_pass1_scan(&scan_text, None, profile);
                        if inline_scan.as_ref().is_some_and(|(a, b)| {
                            a == "pass1"
                                && matches!(b.as_str(), "incomplete" | "no_matching" | "fallback")
                        }) {
                            cond = Some(inl.clone());
                        }
                    }
                    let should_scan = cond.is_some()
                        || RE_ASSIGN_OR_NOT.match_(&stmt.base.text).is_some()
                        || RE_NUM_OR_STR_ASSIGN.match_(&stmt.base.text).is_some()
                        || RE_LOGICAL_ASSIGN_LEAD.match_(&stmt.base.text).is_some()
                        || RE_LOGICAL_ASSIGN_ANY.search(&stmt.base.text).is_some();
                    let verdict = if should_scan {
                        structure::_pass1_scan(&scan_text, None, profile)
                    } else {
                        None
                    };
                    let mut head =
                        verdict
                            .as_ref()
                            .filter(|(a, _)| a == "pass1")
                            .and_then(|(_, b)| match b.as_str() {
                                "invalid_assignment" => Some("Invalid assignment in expression: "),
                                "incomplete" => Some("Incomplete expression: "),
                                "no_matching" => {
                                    Some("No Matching, or out of place ) in expression: ")
                                }
                                "expects_comma" => Some("Expression expects , : "),
                                _ => None,
                            });
                    let call_open = RE_CALL_OPEN.match_(&stmt.base.text);
                    let juxt = call_open.as_ref().and_then(|cm| {
                        if stmt.reason == "lex error: unterminated string" {
                            return None;
                        }
                        RE_JUXT_ARG.search(cm.group(2).unwrap_or(""))
                    });
                    if head.is_none() {
                        if let (Some(cm), Some(jm)) = (&call_open, &juxt) {
                            let cname = pystr::upper(cm.group(1).unwrap_or(""));
                            if let Some(frec) = profile.functions().get(&cname) {
                                if !decl_vars.contains(&cname) {
                                    let inner = cm.group(2).unwrap_or("");
                                    let k = pystr::count(
                                        pystr::slice(inner, None, Some(p3_i64(jm.start0()) + 1)),
                                        ",",
                                    ) + 1;
                                    let nargs = p3_usize(frec.nargs.unwrap_or(0));
                                    head = Some(if k < nargs {
                                        "Expression expects , : "
                                    } else {
                                        "No Matching, or out of place ) in expression: "
                                    });
                                    if P3_SELF_CLOSING_FUNCTIONS.contains(&cname.as_str()) {
                                        head = Some("Incomplete expression: ");
                                    }
                                }
                            }
                        }
                    }
                    let rhs_call = RE_RHS_CALL.match_(&stmt.base.text);
                    if let (Some((va, vb)), Some(rc)) = (&verdict, &rhs_call) {
                        let rname = pystr::upper(rc.group(1).unwrap_or(""));
                        if va == "pass1"
                            && profile.functions().contains(&rname)
                            && words.is_empty()
                            && (vb == "no_matching"
                                || vb == "expects_comma"
                                || (vb == "incomplete"
                                    && P3_SELF_CLOSING_FUNCTIONS.contains(&rname.as_str())))
                        {
                            let parts = super::part2::_split_args_quoted(rc.group(2).unwrap_or(""))
                                .unwrap_or_default();
                            if let Some(bad) = parts
                                .into_iter()
                                .find(|a| super::part2::_adjacent_operands(a))
                            {
                                for word in
                                    super::part2::_leading_unknown_words(pystr::strip(&bad), &here)
                                {
                                    new_items.push(p3_diag(
                                        d.line,
                                        &profile.unresolved_symbol_message.format(&word),
                                        &d.file,
                                        true,
                                    ));
                                }
                            }
                        }
                    }
                    if head == Some("No Matching, or out of place ) in expression: ")
                        && cond.is_none()
                        && stmt.reason != "lex error: unterminated string"
                    {
                        if let Some(refined) = structure::_unclosed_call_message(
                            profile,
                            &stmt.base.text,
                            stmt.base.at_eol.get(),
                            decl_vars,
                        ) {
                            if !refined.is_empty() {
                                d = Rc::new(d.replace(|r| r.message = refined));
                                changed = true;
                                new_items.push(d);
                                continue;
                            }
                        }
                    }
                    if let Some(ref cond_s) = cond {
                        if head == Some("No Matching, or out of place ) in expression: ") {
                            if let Some(refined) =
                                structure::_unclosed_call_message(profile, cond_s, false, decl_vars)
                            {
                                if refined.starts_with("Expression expects ,") {
                                    head = Some("Expression expects , : ");
                                }
                            }
                        }
                        if head.is_none() && cond_s.contains(';') {
                            head = Some("Invalid symbol in expression: ");
                        }
                        let prefix = head.unwrap_or(P3_GENERIC_FALLBACK_PREFIX);
                        d = Rc::new(d.replace(|r| r.message = format!("{prefix}{cond_s}.")));
                        changed = true;
                    } else if let Some(h) = head {
                        let rest_msg = pystr::slice(
                            &d.message,
                            Some(p3_i64(pystr::len(P3_GENERIC_FALLBACK_PREFIX))),
                            None,
                        )
                        .to_owned();
                        d = Rc::new(d.replace(|r| r.message = format!("{h}{rest_msg}")));
                        changed = true;
                    }
                }
            }
        }
        new_items.push(d);
    }

    if changed {
        diags.items = new_items;
    }
}

fn p3_arity_prefix(kind: &str) -> Option<&'static str> {
    match kind {
        "many" => Some("No Matching, or out of place ) in expression: "),
        "few" => Some("Expression expects , : "),
        "empty" => Some("Invalid, or out of place expression: "),
        "network_constint" => Some("Parameter must be a constant integer: "),
        _ => None,
    }
}

/// Port of `checker/cr3xx.py::_ArityState`.
#[derive(Debug, Default)]
pub struct ArityState {
    pub names: Vec<String>,
    pub kind: Option<String>,
    pub bad: bool,
    pub stop: bool,
}

/// Port of `checker/cr3xx.py::_arity_walk`.
pub fn _arity_walk(_expr: Option<&Expr>, _known: &BTreeSet<String>, _state: &mut ArityState) {
    todo!("checker/cr3xx.py::_arity_walk (omitted: profile: untyped)")
}

/// Port of `checker/cr3xx.py::_arity_walk`.
pub fn _arity_walk_with(
    expr: Option<&Expr>,
    known: &expressions::KnownNames,
    profile: &Profile,
    state: &mut ArityState,
) {
    let Some(expr) = expr else {
        return;
    };
    if state.stop {
        return;
    }
    match expr {
        Expr::Name(n) => {
            let upper = pystr::upper(&n.name);
            if !known.names.contains(&upper) || known.tables.contains(&upper) {
                state.names.push(n.name.clone());
            }
        }
        Expr::Index(idx) => {
            let upper = pystr::upper(&idx.name);
            if !known.names.contains(&upper) {
                state.names.push(idx.name.clone());
            }
            let problem = expressions::_call_arity_problem_with(&idx.name, &idx.args, profile);
            let mut args = idx.args.as_slice();
            let mut stop = false;
            if problem.as_deref() == Some("many") {
                let at = (0..args.len())
                    .find(|&k| {
                        expressions::_call_arity_problem_with(&idx.name, &args[..=k], profile)
                            .as_deref()
                            == Some("many")
                    })
                    .unwrap_or(args.len());
                args = &args[..at];
                stop = true;
                if state.kind.is_none() {
                    state.kind = Some("many".to_owned());
                }
            } else if matches!(
                problem.as_deref(),
                Some("few" | "empty" | "network_constint")
            ) {
                if state.kind.is_none() {
                    state.kind = problem;
                }
            } else if problem.is_some() {
                state.bad = true;
            }
            for a in args {
                _arity_walk_with(a.as_deref(), known, profile, state);
            }
            state.stop = state.stop || stop;
        }
        Expr::Unary(u) => {
            _arity_walk_with(u.operand.as_deref(), known, profile, state);
        }
        Expr::Binary(b) => {
            _arity_walk_with(b.left.as_deref(), known, profile, state);
            _arity_walk_with(b.right.as_deref(), known, profile, state);
        }
        Expr::Member(m) => {
            let base_known = matches!(
                m.base.as_deref(),
                Some(Expr::Name(n)) if known.names.contains(&pystr::upper(&n.name))
            );
            if !base_known {
                _arity_walk_with(m.base.as_deref(), known, profile, state);
            }
            for a in m.args.iter().flatten() {
                _arity_walk_with(a.as_deref(), known, profile, state);
            }
        }
        Expr::ArrayLiteral(arr) => {
            for item in &arr.items {
                _arity_walk_with(item.as_deref(), known, profile, state);
            }
        }
        Expr::Juxtaposed(j) => {
            for item in &j.items {
                _arity_walk_with(item.as_deref(), known, profile, state);
            }
        }
        Expr::Number(_) | Expr::String(_) => {}
    }
}

/// Port of `checker/cr3xx.py::order_names_around_arity`.
pub fn order_names_around_arity(_program: &Program, _diags: &mut Diagnostics) {
    todo!("checker/cr3xx.py::order_names_around_arity (omitted: profile: untyped)")
}

/// Port of `checker/cr3xx.py::order_names_around_arity`.
pub fn order_names_around_arity_with(
    program: &ProgramRef,
    profile: &'static Profile,
    diags: &mut Diagnostics,
) {
    if !p3_is_cr3xx(profile) {
        return;
    }
    let mut per_line: HashMap<(i64, String), usize> = HashMap::new();
    for st in &program.statements {
        let base = st.base();
        *per_line
            .entry((base.node.line, base.node.file.clone()))
            .or_insert(0) += 1;
    }
    let mut known: Option<expressions::KnownNames> = None;
    for stmt in &program.statements {
        let exprs: Vec<Option<&Expr>> = match &**stmt {
            Stmt::Assignment(a) => vec![a.target.as_deref(), a.value.as_deref()],
            Stmt::If(i) => vec![i.cond.as_deref()],
            _ => continue,
        };
        let base = stmt.base();
        let key = (base.node.line, base.node.file.clone());
        if per_line.get(&key).copied().unwrap_or(0) != 1 {
            continue;
        }
        let own: Vec<(usize, DiagRef)> = diags
            .items
            .iter()
            .enumerate()
            .filter(|(_, d)| d.line == key.0 && d.file == key.1 && d.severity == "error")
            .map(|(i, d)| (i, d.clone()))
            .collect();
        if known.is_none() {
            known = Some(
                context::of(program, Some(profile), None)
                    .known_names()
                    .clone(),
            );
        }
        let kn = known.as_ref().unwrap();
        let mut state = ArityState::default();
        for e in exprs {
            _arity_walk_with(e, kn, profile, &mut state);
        }
        let Some(ref kind) = state.kind else {
            continue;
        };
        if state.bad {
            continue;
        }
        let Some(prefix) = p3_arity_prefix(kind) else {
            continue;
        };
        let mut arity: Vec<(usize, DiagRef)> = own
            .iter()
            .filter(|(_, d)| d.message.starts_with(prefix))
            .cloned()
            .collect();
        if arity.is_empty() && kind != "network_constint" {
            if let Stmt::Assignment(a) = &**stmt {
                let kv = expressions::KnownView {
                    names: &kn.names,
                    tables: &kn.tables,
                    out_of_scope: &kn.out_of_scope,
                    dotted_tables: kn.dotted_tables,
                };
                if !own.is_empty()
                    && !expressions::_undefined_names_with(a.target.as_deref(), &kv).is_empty()
                {
                    let msg = expressions::_arity_message_with(
                        kind,
                        &a.base.text,
                        profile,
                        a.base.at_eol.get(),
                        true,
                    );
                    arity = vec![(
                        own[0].0,
                        p3_diag(a.base.node.line, &msg, &a.base.node.file, false),
                    )];
                }
            }
        }
        if arity.len() != 1
            || own.iter().any(|(_, d)| {
                d.message != arity[0].1.message && !d.message.ends_with(P3_UNRESOLVED_SUFFIX)
            })
        {
            continue;
        }
        let mut new_diags: Vec<DiagRef> = state
            .names
            .iter()
            .map(|n| {
                p3_diag(
                    base.node.line,
                    &profile.unresolved_symbol_message.format(n),
                    &base.node.file,
                    true,
                )
            })
            .collect();
        new_diags.push(arity[0].1.clone());
        let first = own[0].0;
        let drop_ids: HashSet<usize> = own.iter().map(|(_, d)| syntax::rc_id(d)).collect();
        let at = first
            - diags.items[..first]
                .iter()
                .filter(|d| drop_ids.contains(&syntax::rc_id(d)))
                .count();
        let mut items: Vec<DiagRef> = diags
            .items
            .iter()
            .filter(|d| !drop_ids.contains(&syntax::rc_id(d)))
            .cloned()
            .collect();
        items.splice(at..at, new_diags);
        diags.items = items;
    }
}

/// Port of `checker/cr3xx.py::order_names_before_call_arity`.
pub fn order_names_before_call_arity(_program: &Program, _diags: &mut Diagnostics) {
    todo!("checker/cr3xx.py::order_names_before_call_arity (omitted: profile: untyped)")
}

/// Port of `checker/cr3xx.py::order_names_before_call_arity`.
pub fn order_names_before_call_arity_with(
    program: &Program,
    profile: &Profile,
    diags: &mut Diagnostics,
) {
    if !p3_is_cr3xx(profile) {
        return;
    }
    let functions: Vec<Regex> = program
        .statements
        .iter()
        .filter_map(|st| match &**st {
            Stmt::SubDef(sd) if sd.keyword == "FUNCTION" => Some(pystr::upper(&sd.name)),
            _ => None,
        })
        .collect::<BTreeSet<String>>()
        .into_iter()
        .map(|f| {
            Regex::new(
                &format!(r"(?i)\b{}\s*\(", pyre::escape(&f)),
                pyre::IGNORECASE,
            )
        })
        .collect();
    if functions.is_empty() {
        return;
    }
    let mut per_line: HashMap<(i64, String), usize> = HashMap::new();
    for st in &program.statements {
        let base = st.base();
        *per_line
            .entry((base.node.line, base.node.file.clone()))
            .or_insert(0) += 1;
    }
    for stmt in &program.statements {
        let Stmt::Assignment(a) = &**stmt else {
            continue;
        };
        let key = (a.base.node.line, a.base.node.file.clone());
        if per_line.get(&key).copied().unwrap_or(0) != 1 {
            continue;
        }
        if !functions.iter().any(|re| re.search(&a.base.text).is_some()) {
            continue;
        }
        let at: Vec<usize> = diags
            .items
            .iter()
            .enumerate()
            .filter(|(_, d)| d.line == key.0 && d.file == key.1)
            .map(|(i, _)| i)
            .collect();
        if at.is_empty() || at.windows(2).any(|w| w[1] != w[0] + 1) {
            continue;
        }
        let own: Vec<DiagRef> = at.iter().map(|&i| diags.items[i].clone()).collect();
        let names: Vec<DiagRef> = own
            .iter()
            .filter(|d| RE_UNDEF_WORD.fullmatch(&d.message).is_some())
            .cloned()
            .collect();
        let rest: Vec<DiagRef> = own.iter().filter(|d| !names.contains(d)).cloned().collect();
        if names.is_empty()
            || rest.is_empty()
            || !rest.iter().all(|d| {
                d.message == "Call argument required."
                    || d.message
                        .starts_with("No Matching, or out of place ) in expression: ")
                    || d.message
                        .starts_with("Invalid, or out of place expression: ")
            })
        {
            continue;
        }
        let mut combined = names;
        combined.extend(rest);
        diags.items.splice(at[0]..=at[at.len() - 1], combined);
    }
}

const P3_CLOSER_ERRORS: &[&str] = &[
    "END IF without IF or END SELECT without SELECT.",
    "NEXT without FOR.",
    "LOOP without DO.",
    "IF or SELECT statement illegally has SCAN without NEXT SCAN.",
    "FOR NEXT statement illegally has SCAN without NEXT SCAN.",
    "LOOP statement illegally has SCAN without NEXT SCAN.",
];

/// Port of `checker/cr3xx.py::_hides_nothing`.
pub fn _hides_nothing() -> bool {
    todo!("checker/cr3xx.py::_hides_nothing (omitted: d: untyped)")
}

/// Port of `checker/cr3xx.py::_hides_nothing`.
pub fn _hides_nothing_with(d: &Diagnostic) -> bool {
    if d.message
        .ends_with(" must be used inside a table declaration.")
    {
        return true;
    }
    if d.message.ends_with("without Sub or nested subs.") {
        return true;
    }
    if d.message == "If without EndIf inside of Sub." {
        return true;
    }
    if d.message
        .starts_with("No Matching, or out of place ) in expression")
        || d.message.starts_with("Expression expects")
    {
        let (_, tail) = pystr::partition(&d.message, ":").pipe_or_whole(&d.message);
        let lower = pystr::lower(tail);
        let masked = RE_QUOTED_LIT.sub_fn(&lower, |m| " ".repeat(pystr::len(m.group0())));
        if let Some(at) = pystr::find(&masked, "public.", None, None) {
            let rparen = pystr::find(&masked, ")", None, None).unwrap_or(pystr::len(&masked));
            return at < rparen;
        }
    }
    false
}

trait PartitionTail<'a> {
    fn pipe_or_whole(self, orig: &'a str) -> (&'a str, &'a str);
}

impl<'a> PartitionTail<'a> for (&'a str, &'a str, &'a str) {
    fn pipe_or_whole(self, orig: &'a str) -> (&'a str, &'a str) {
        if self.1.is_empty() {
            ("", orig)
        } else {
            (self.0, self.2)
        }
    }
}

/// Port of `checker/cr3xx.py::drop_block_closer_errors_after_errors`.
pub fn drop_block_closer_errors_after_errors(_program: &Program, _diags: &mut Diagnostics) {
    todo!("checker/cr3xx.py::drop_block_closer_errors_after_errors (omitted: profile: untyped)")
}

/// Port of `checker/cr3xx.py::drop_block_closer_errors_after_errors`.
pub fn drop_block_closer_errors_after_errors_with(
    _program: &Program,
    profile: &Profile,
    diags: &mut Diagnostics,
) {
    if !p3_is_cr3xx(profile) {
        return;
    }
    let closers: Vec<DiagRef> = diags
        .items
        .iter()
        .filter(|d| d.severity == "error" && P3_CLOSER_ERRORS.contains(&d.message.as_str()))
        .cloned()
        .collect();
    if closers.is_empty() {
        return;
    }
    let lines: HashSet<(i64, String)> = closers.iter().map(|d| (d.line, d.file.clone())).collect();
    let mut ours: HashSet<usize> = closers.iter().map(syntax::rc_id).collect();
    for d in &diags.items {
        if d.severity == "error"
            && lines.contains(&(d.line, d.file.clone()))
            && d.message.ends_with(" illegally occurs before BeginProg.")
        {
            ours.insert(syntax::rc_id(d));
        }
    }
    if !diags.items.iter().any(|d| {
        d.severity == "error" && !ours.contains(&syntax::rc_id(d)) && !_hides_nothing_with(d)
    }) {
        return;
    }
    diags.items.retain(|d| !ours.contains(&syntax::rc_id(d)));
}

/// Port of `checker/cr3xx.py::drop_calltable_error_with_other_errors`.
pub fn drop_calltable_error_with_other_errors(_program: &Program, _diags: &mut Diagnostics) {
    todo!("checker/cr3xx.py::drop_calltable_error_with_other_errors (omitted: profile: untyped)")
}

/// Port of `checker/cr3xx.py::drop_calltable_error_with_other_errors`.
pub fn drop_calltable_error_with_other_errors_with(
    program: &Program,
    profile: &Profile,
    diags: &mut Diagnostics,
) {
    if !p3_is_cr3xx(profile) {
        return;
    }
    let sites: HashSet<(i64, String)> = program
        .statements
        .iter()
        .filter(|st| !matches!(&***st, Stmt::Unparsed(_)))
        .map(|st| {
            let base = st.base();
            (base.node.line, base.node.file.clone())
        })
        .collect();
    let mine: Vec<DiagRef> = diags
        .items
        .iter()
        .filter(|d| {
            d.severity == "error"
                && sites.contains(&(d.line, d.file.clone()))
                && (d.message.starts_with("Could not find DataTable ")
                    || d.message.starts_with("Could not find FileMark table ")
                    || d.message.starts_with("Cannot find DataTable "))
        })
        .cloned()
        .collect();
    if mine.is_empty() {
        return;
    }
    let lines: HashSet<(i64, String)> = mine.iter().map(|d| (d.line, d.file.clone())).collect();
    if diags.items.iter().any(|d| {
        d.severity == "error"
            && !mine.contains(d)
            && !_hides_nothing_with(d)
            && !lines.contains(&(d.line, d.file.clone()))
    }) {
        diags.items.retain(|d| !mine.contains(d));
    }
}

static P3_CALL_KEYWORD_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"\b(?:CASE|DO|FOR|FUNCTION|SCAN|SELECT|SUB|SUBSCAN|WHILE)\s*\(",
        pyre::IGNORECASE,
    )
});

/// Port of `checker/cr3xx.py::order_block_keyword_calls`.
pub fn order_block_keyword_calls(_program: &Program, _diags: &mut Diagnostics) {
    todo!("checker/cr3xx.py::order_block_keyword_calls (omitted: profile: untyped)")
}

/// Port of `checker/cr3xx.py::order_block_keyword_calls`.
pub fn order_block_keyword_calls_with(
    program: &Program,
    profile: &Profile,
    diags: &mut Diagnostics,
) {
    if !p3_is_cr3xx(profile) {
        return;
    }
    for stmt in &program.statements {
        if !matches!(&**stmt, Stmt::Assignment(_) | Stmt::If(_) | Stmt::ElseIf(_)) {
            continue;
        }
        let base = stmt.base();
        let key = (base.node.line, base.node.file.clone());
        let code = RE_QUOTED_LIT.sub_fn(&base.text, |m| " ".repeat(pystr::len(m.group0())));
        let Some(m) = P3_CALL_KEYWORD_RE.search(&code) else {
            continue;
        };
        let marks: Vec<DiagRef> = diags
            .items
            .iter()
            .filter(|d| {
                d.line == key.0
                    && d.file == key.1
                    && d.message.starts_with("Incomplete expression:")
            })
            .cloned()
            .collect();
        if marks.is_empty() {
            continue;
        }
        if diags.items.iter().any(|d| {
            d.line == key.0
                && d.file == key.1
                && d.message
                    .starts_with("Invalid, or out of place expression:")
        }) {
            diags.items.retain(|d| !marks.contains(d));
            continue;
        }
        let before: HashSet<String> = RE_IDENT_WORD
            .findall(pystr::slice(&code, None, Some(p3_i64(m.start0()))))
            .into_iter()
            .map(pystr::upper)
            .collect();
        let mut keep: Vec<DiagRef> = Vec::with_capacity(diags.items.len());
        for d in &diags.items {
            if d.line == key.0 && d.file == key.1 {
                let undef = RE_UNDEF_WORD_CAP.fullmatch(&d.message);
                if d.message.starts_with("Dimensions not declared")
                    || undef
                        .is_some_and(|u| !before.contains(&pystr::upper(u.group(1).unwrap_or(""))))
                {
                    continue;
                }
            }
            keep.push(d.clone());
        }
        let mut group: Vec<DiagRef> = keep
            .iter()
            .filter(|d| d.line == key.0 && d.file == key.1 && !marks.contains(d))
            .cloned()
            .collect();
        group.extend(
            keep.iter()
                .filter(|d| d.line == key.0 && d.file == key.1 && marks.contains(d))
                .cloned(),
        );
        let first = keep
            .iter()
            .position(|d| d.line == key.0 && d.file == key.1)
            .unwrap_or(keep.len());
        let mut rest: Vec<DiagRef> = keep
            .into_iter()
            .filter(|d| !(d.line == key.0 && d.file == key.1))
            .collect();
        rest.splice(first..first, group);
        diags.items = rest;
    }
}

/// Port of `checker/cr3xx.py::fix_then_operand`.
pub fn fix_then_operand(_program: &Program, _diags: &mut Diagnostics) {
    todo!("checker/cr3xx.py::fix_then_operand (omitted: profile: untyped)")
}

/// Port of `checker/cr3xx.py::fix_then_operand`.
pub fn fix_then_operand_with(program: &Program, profile: &Profile, diags: &mut Diagnostics) {
    if !p3_is_cr3xx(profile) {
        return;
    }
    for stmt in &program.statements {
        let cond = match &**stmt {
            Stmt::If(i) => i.cond.as_deref(),
            Stmt::ElseIf(e) => e.cond.as_deref(),
            _ => continue,
        };
        if !structure::_mentions_then(cond) {
            continue;
        }
        let base = stmt.base();
        let Some((va, vb)) = structure::_pass1_scan(&base.text, None, profile) else {
            continue;
        };
        if va != "pass1" {
            continue;
        }
        let target_msg = profile.unresolved_symbol_message.format("Then");
        let hits: Vec<usize> = diags
            .items
            .iter()
            .enumerate()
            .filter(|(_, d)| {
                d.line == base.node.line && d.file == base.node.file && d.message == target_msg
            })
            .map(|(i, _)| i)
            .collect();
        if hits.is_empty() {
            continue;
        }
        let mut cloned = (**stmt).clone();
        match &mut cloned {
            Stmt::If(i) => i.base.at_eol.set(false),
            Stmt::ElseIf(e) => e.base.at_eol.set(false),
            _ => {}
        }
        let message = structure::_pass1_message(profile, &cloned, &vb, None);
        diags.items[hits[0]] = Rc::new(diags.items[hits[0]].replace(|r| r.message = message));
        for &i in hits[1..].iter().rev() {
            diags.items.remove(i);
        }
    }
}

/// Port of `checker/cr3xx.py::order_invalid_assignments`.
pub fn order_invalid_assignments(_program: &Program, _diags: &mut Diagnostics) {
    todo!("checker/cr3xx.py::order_invalid_assignments (omitted: profile: untyped)")
}

/// Port of `checker/cr3xx.py::order_invalid_assignments`.
pub fn order_invalid_assignments_with(
    program: &Program,
    profile: &Profile,
    diags: &mut Diagnostics,
) {
    if !p3_is_cr3xx(profile) {
        return;
    }
    let conditions: HashSet<usize> = program
        .statements
        .iter()
        .filter(|st| matches!(&***st, Stmt::If(_)))
        .map(syntax::stmt_id)
        .collect();
    let markers: Vec<DiagRef> = diags
        .items
        .iter()
        .filter(|d| {
            !conditions.contains(&d.stmt)
                && d.message.starts_with("Invalid assignment in expression:")
        })
        .cloned()
        .collect();
    let mut keys: Vec<(i64, String)> = Vec::new();
    let mut seen: HashSet<(i64, String)> = HashSet::new();
    for d in &markers {
        let k = (d.line, d.file.clone());
        if seen.insert(k.clone()) {
            keys.push(k);
        }
    }
    for key in keys {
        let own: Vec<DiagRef> = diags
            .items
            .iter()
            .filter(|d| d.line == key.0 && d.file == key.1)
            .cloned()
            .collect();
        let marks: Vec<DiagRef> = markers
            .iter()
            .filter(|d| d.line == key.0 && d.file == key.1)
            .cloned()
            .collect();
        let mut group: Vec<DiagRef> = own.into_iter().filter(|d| !marks.contains(d)).collect();
        group.extend(marks);
        let first = diags
            .items
            .iter()
            .position(|d| d.line == key.0 && d.file == key.1)
            .unwrap_or(diags.items.len());
        let mut rest: Vec<DiagRef> = diags
            .items
            .iter()
            .filter(|d| !(d.line == key.0 && d.file == key.1))
            .cloned()
            .collect();
        rest.splice(first..first, group);
        diags.items = rest;
    }
}

pub const _STRING_END_QUOTE: &str = "String without end quote";
pub static _IF_STRING_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\s*If\s+(.*?)\s+Then\s+([A-Za-z_]\w*)\s*(?:\([^()]*\))?\s*=",
        pyre::IGNORECASE,
    )
});

const P3_BINARY_SYMBOLS: &[&str] = &[
    "+", "-", "*", "/", "^", "&", "<", ">", "=", "<>", "<=", ">=", "\\",
];
const P3_WORD_OPERATORS: &[&str] = &["AND", "OR", "XOR", "EQV", "IMP", "MOD", "IS", "TO"];

/// Port of `checker/cr3xx.py::_string_reached`.
pub fn _string_reached(text: &str) -> bool {
    let head = match lexer::tokenize(text, 0) {
        Ok(_) => return false,
        Err(e) => {
            if e.message != "unterminated string"
                || pystr::slice(text, Some(p3_i64(e.col)), Some(p3_i64(e.col) + 1)) != "\""
            {
                return false;
            }
            pystr::slice(text, None, Some(p3_i64(e.col)))
        }
    };
    let Ok(all_toks) = lexer::tokenize(head, 0) else {
        return false;
    };
    let mut expect = true;
    let mut depth: i64 = 0;
    for t in all_toks.into_iter().filter(|t| t.kind != "EOF") {
        let word_op = t.kind == "IDENT" && P3_WORD_OPERATORS.contains(&t.upper.as_str());
        if expect {
            if t.kind == "OP" && matches!(t.text.as_str(), "(" | "-" | "+") {
                if t.text == "(" {
                    depth += 1;
                }
            } else if t.kind == "IDENT" && t.upper == "NOT" {
                // unary NOT keeps expect = true
            } else if matches!(t.kind.as_str(), "IDENT" | "NUMBER" | "STRING") && !word_op {
                expect = false;
            } else {
                return false;
            }
        } else if (t.kind == "OP" && P3_BINARY_SYMBOLS.contains(&t.text.as_str())) || word_op {
            expect = true;
        } else if t.kind == "OP" && t.text == "(" {
            depth += 1;
            expect = true;
        } else if t.kind == "OP" && t.text == ")" {
            depth -= 1;
        } else if t.kind == "OP" && t.text == "," && depth > 0 {
            expect = true;
        } else {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checker::expressions::KnownNames;
    use crate::profiles;
    use crate::syntax::{Assignment, If, Index, Member, Name, Node, Number, NumberValue, StmtBase};

    fn cr300() -> &'static Profile {
        profiles::get("cr300").expect("cr300 profile")
    }

    fn cr6() -> &'static Profile {
        profiles::get("cr6").expect("cr6 profile")
    }

    fn name_expr(s: &str) -> Rc<Expr> {
        Rc::new(Expr::Name(Name {
            node: Node::default(),
            name: s.to_owned(),
        }))
    }

    fn num_expr(v: f64) -> Rc<Expr> {
        Rc::new(Expr::Number(Number {
            node: Node::default(),
            value: NumberValue::Float(v),
            text: v.to_string(),
        }))
    }

    fn make_stmt_base(line: i64, text: &str) -> StmtBase {
        StmtBase {
            node: Node {
                line,
                file: "main.cr300".to_owned(),
            },
            text: text.to_owned(),
            ..StmtBase::default()
        }
    }

    fn make_assign(
        line: i64,
        text: &str,
        target: Option<Rc<Expr>>,
        value: Option<Rc<Expr>>,
    ) -> Rc<Stmt> {
        Rc::new(Stmt::Assignment(Assignment {
            base: make_stmt_base(line, text),
            target,
            value,
            compound: false,
        }))
    }

    #[test]
    fn check_unknown_line_words_early_returns() {
        let mut prog = Program::default();
        prog.statements.push(make_assign(
            1,
            "X = 1",
            Some(name_expr("X")),
            Some(num_expr(1.0)),
        ));
        let prog_ref = Rc::new(prog);
        let mut diags = Diagnostics::new();
        diags.error_basic(1, "Invalid, or out of place expression: Foo.", "main.cr6");
        check_unknown_line_words_with(&prog_ref, cr6(), &mut diags);
        assert_eq!(diags.items.len(), 1);

        let mut diags_empty = Diagnostics::new();
        check_unknown_line_words_with(&prog_ref, cr300(), &mut diags_empty);
        assert_eq!(diags_empty.items.len(), 0);

        check_unknown_line_words_with(&prog_ref, cr300(), &mut diags);
        assert_eq!(diags.items.len(), 1);
    }

    #[test]
    fn arity_walk_stops_at_surplus_and_marks_few() {
        let mut known = KnownNames::default();
        known.names.insert("KNOWN".to_owned());
        known.names.insert("FLOOR".to_owned());
        known.names.insert("ROUND".to_owned());
        known.names.insert("TBL".to_owned());
        known.tables.insert("TBL".to_owned());

        let call_many = Expr::Index(Index {
            node: Node::default(),
            name: "Floor".to_owned(),
            args: vec![Some(name_expr("Nope1")), Some(name_expr("Nope2"))],
        });
        let mut st = ArityState::default();
        _arity_walk_with(Some(&call_many), &known, cr300(), &mut st);
        assert_eq!(st.kind.as_deref(), Some("many"));
        assert!(st.stop);
        assert_eq!(st.names, vec!["Nope1"]);

        let call_few = Expr::Index(Index {
            node: Node::default(),
            name: "Round".to_owned(),
            args: vec![Some(name_expr("Nope1"))],
        });
        let mut st2 = ArityState::default();
        _arity_walk_with(Some(&call_few), &known, cr300(), &mut st2);
        assert_eq!(st2.kind.as_deref(), Some("few"));
        assert!(!st2.stop);
        assert_eq!(st2.names, vec!["Nope1"]);

        let member = Expr::Member(Member {
            node: Node::default(),
            base: Some(name_expr("TBL")),
            member: "x".to_owned(),
            args: Some(vec![Some(name_expr("KNOWN")), Some(name_expr("TBL"))]),
        });
        let mut st3 = ArityState::default();
        _arity_walk_with(Some(&member), &known, cr300(), &mut st3);
        assert_eq!(st3.names, vec!["TBL"]);
    }

    #[test]
    fn order_names_around_arity_reorders_and_synthesizes() {
        let mut prog = Program::default();
        let rhs = Rc::new(Expr::Index(Index {
            node: Node::default(),
            name: "Round".to_owned(),
            args: vec![Some(name_expr("Nope1"))],
        }));
        prog.statements.push(make_assign(
            3,
            "UnkTarget = Round(Nope1)",
            Some(name_expr("UnkTarget")),
            Some(rhs),
        ));
        let prog_ref = Rc::new(prog);
        let mut diags = Diagnostics::new();
        diags.error_basic(3, "UnkTarget is not defined.", "main.cr300");

        order_names_around_arity_with(&prog_ref, cr300(), &mut diags);
        let msgs: Vec<&str> = diags.items.iter().map(|d| d.message.as_str()).collect();
        assert_eq!(
            msgs,
            vec![
                "UnkTarget is not defined.",
                "Nope1 is not defined.",
                "Expression expects , : UnkTarget = Round(Nope1)\n."
            ]
        );
    }

    #[test]
    fn order_names_before_call_arity_moves_names_first() {
        let mut prog = Program::default();
        prog.statements
            .push(Rc::new(Stmt::SubDef(crate::syntax::SubDef {
                base: make_stmt_base(1, "Function F2(a, b)"),
                keyword: "FUNCTION".to_owned(),
                name: "F2".to_owned(),
                params: vec![],
                return_type: None,
                header_errors: vec![],
            })));
        prog.statements.push(make_assign(
            5,
            "A(Z) = F2(1,2,3)",
            Some(name_expr("A")),
            Some(num_expr(1.0)),
        ));
        let mut diags = Diagnostics::new();
        diags.error_basic(5, "Call argument required.", "main.cr300");
        diags.error_basic(5, "Z is not defined.", "main.cr300");
        diags.error_basic(
            5,
            "No Matching, or out of place ) in expression: A(Z) = F2(1,2,3).",
            "main.cr300",
        );

        order_names_before_call_arity_with(&prog, cr300(), &mut diags);
        let msgs: Vec<&str> = diags.items.iter().map(|d| d.message.as_str()).collect();
        assert_eq!(
            msgs,
            vec![
                "Z is not defined.",
                "Call argument required.",
                "No Matching, or out of place ) in expression: A(Z) = F2(1,2,3)."
            ]
        );
    }

    #[test]
    fn hides_nothing_and_drop_block_closer_errors() {
        let d_sample = Diagnostic::new(
            "error",
            1,
            "Sample must be used inside a table declaration.",
            "m",
            ShowFile::Bool(true),
            0,
            "",
            "",
        );
        let d_pub_before = Diagnostic::new(
            "error",
            1,
            "No Matching, or out of place ) in expression: Round(Public.x,1,2)",
            "m",
            ShowFile::Bool(true),
            0,
            "",
            "",
        );
        let d_pub_after = Diagnostic::new(
            "error",
            1,
            "No Matching, or out of place ) in expression: Round(x)+Public.T",
            "m",
            ShowFile::Bool(true),
            0,
            "",
            "",
        );
        assert!(_hides_nothing_with(&d_sample));
        assert!(_hides_nothing_with(&d_pub_before));
        assert!(!_hides_nothing_with(&d_pub_after));

        let prog = Program::default();
        let mut diags = Diagnostics::new();
        diags.error_basic(2, "END IF without IF or END SELECT without SELECT.", "m");
        diags.error_basic(2, "EndIf illegally occurs before BeginProg.", "m");
        diags.items.push(Rc::new(d_sample));
        drop_block_closer_errors_after_errors_with(&prog, cr300(), &mut diags);
        assert_eq!(diags.items.len(), 3);

        diags.error_basic(5, "Foo is not defined.", "m");
        drop_block_closer_errors_after_errors_with(&prog, cr300(), &mut diags);
        let msgs: Vec<&str> = diags.items.iter().map(|d| d.message.as_str()).collect();
        assert_eq!(
            msgs,
            vec![
                "Sample must be used inside a table declaration.",
                "Foo is not defined."
            ]
        );
    }

    #[test]
    fn drop_calltable_error_with_other_errors_respects_same_line_and_other_lines() {
        let mut prog = Program::default();
        prog.statements
            .push(make_assign(2, "X = Tmp.dat + Zed", None, None));
        let mut diags = Diagnostics::new();
        diags.error_basic(2, "Could not find DataTable Tmp.", "main.cr300");
        diags.error_basic(2, "Zed is not defined.", "main.cr300");
        drop_calltable_error_with_other_errors_with(&prog, cr300(), &mut diags);
        assert_eq!(diags.items.len(), 2);

        diags.error_basic(4, "Other is not defined.", "main.cr300");
        drop_calltable_error_with_other_errors_with(&prog, cr300(), &mut diags);
        assert_eq!(diags.items.len(), 2);
        assert_eq!(diags.items[0].message, "Zed is not defined.");
    }

    #[test]
    fn order_block_keyword_calls_filters_and_moves_incomplete_last() {
        let mut prog = Program::default();
        prog.statements
            .push(make_assign(10, "Y = Q + Scan(Zork)", None, None));
        let mut diags = Diagnostics::new();
        diags.error_basic(
            10,
            "Incomplete expression: Y = Q + Scan(Zork).",
            "main.cr300",
        );
        diags.error_basic(10, "Zork is not defined.", "main.cr300");
        diags.error_basic(10, "Dimensions not declared for Scan.", "main.cr300");
        diags.error_basic(10, "Q is not defined.", "main.cr300");

        order_block_keyword_calls_with(&prog, cr300(), &mut diags);
        let msgs: Vec<&str> = diags.items.iter().map(|d| d.message.as_str()).collect();
        assert_eq!(
            msgs,
            vec![
                "Q is not defined.",
                "Incomplete expression: Y = Q + Scan(Zork)."
            ]
        );
    }

    #[test]
    fn order_invalid_assignments_moves_marker_after_names() {
        let mut prog = Program::default();
        let assign = make_assign(4, "Sec = Nope", None, None);
        let if_stmt = Rc::new(Stmt::If(If {
            base: make_stmt_base(8, "If 1 = 2 Then"),
            cond: None,
            inline: None,
            inline_else: None,
        }));
        let assign_id = syntax::stmt_id(&assign);
        let if_id = syntax::stmt_id(&if_stmt);
        prog.statements.push(assign);
        prog.statements.push(if_stmt);

        let mut diags = Diagnostics::new();
        diags.error(
            4,
            "Invalid assignment in expression: Sec = Nope.",
            "main.cr300",
            ShowFile::Bool(true),
            assign_id,
            "",
            "",
        );
        diags.error_basic(4, "Nope is not defined.", "main.cr300");
        diags.error(
            8,
            "Invalid assignment in expression: If 1 = 2 Then.",
            "main.cr300",
            ShowFile::Bool(true),
            if_id,
            "",
            "",
        );
        diags.error_basic(8, "Other is not defined.", "main.cr300");

        order_invalid_assignments_with(&prog, cr300(), &mut diags);
        let msgs: Vec<&str> = diags.items.iter().map(|d| d.message.as_str()).collect();
        assert_eq!(
            msgs,
            vec![
                "Nope is not defined.",
                "Invalid assignment in expression: Sec = Nope.",
                "Invalid assignment in expression: If 1 = 2 Then.",
                "Other is not defined.",
            ]
        );
    }

    #[test]
    fn string_reached_checks_prefix_tokens() {
        assert!(_string_reached(r#"Foo" bar"#));
        assert!(_string_reached(r#"Nope + "abc"#));
        assert!(_string_reached(r#"Len("abc"#));
        assert!(_string_reached(r#"Not "abc"#));
        assert!(!_string_reached(r#"Datalogger</title>")"#));
        assert!(!_string_reached(r#"Foo "ok""#));
        assert!(!_string_reached(r"Foo bar"));
    }
}
