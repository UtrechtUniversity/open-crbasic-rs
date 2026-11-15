//! Port of `checker/cr3xx.py` (lines 769-1327).

#![allow(dead_code, unused_variables, unused_imports)]

use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;
use std::sync::LazyLock;

use crate::checker::context;
use crate::checker::expressions;
use crate::diagnostics::{DiagRef, Diagnostic, Diagnostics, ShowFile};
use crate::profiles::{self, Profile};
use crate::pyre::{self, Regex};
use crate::pystr;
use crate::syntax::{
    Assignment, Call, Case, ConstDecl, Declaration, Expr, ExprRef, Index, Instruction, Juxtaposed,
    Member, Number, OrderedMap, Program, ProgramRef, Stmt, StmtRef, SubDef, Unparsed, VarDecl,
    ends_routine,
};

mod pending_wiring {
    use super::{Instruction, Profile, profiles};

    pub(super) fn to_i64(n: usize) -> i64 {
        i64::try_from(n).unwrap_or(i64::MAX)
    }

    // TODO(integrator-2): import from `cr3xx::part1` once ported.
    pub(super) const _UNRESOLVED_SUFFIX: &str = " is not defined.";
    // TODO(integrator-2): import from `cr3xx::part1` once ported.
    pub(super) const _VARIABLE_ILLEGAL_PREFIX: &str = "Variable illegal in Parameter:";

    // TODO(integrator-2): import from `instr_rules` once ported.
    pub(super) const _SERIALIN_DEST_MESSAGE: &str = "A string variable destination is required";

    // TODO(integrator-2): wire to `instr_rules::shortfall_message_with`.
    pub(super) fn shortfall_message(
        _record: &profiles::Instruction,
        _given: i64,
    ) -> Option<String> {
        todo!("checker/instr_rules.py::shortfall_message")
    }

    // TODO(integrator-2): wire to `instr_rules::given_count_of`.
    pub(super) fn given_count_of(_args: &[String]) -> i64 {
        todo!("checker/instr_rules.py::given_count_of")
    }

    // TODO(integrator-2): wire to `instr_rules::_next_simple_token`.
    pub(super) fn _next_simple_token(_text: &str) -> String {
        todo!("checker/instr_rules.py::_next_simple_token")
    }

    // TODO(integrator-2): wire to `instr_rules::given_count`.
    pub(super) fn given_count(_stmt: &Instruction, _profile: &'static Profile) -> i64 {
        todo!("checker/instr_rules.py::given_count")
    }

    // TODO(integrator-2): wire to `instr_rules::_required_count`.
    pub(super) fn _required_count(_params: &[i64]) -> i64 {
        todo!("checker/instr_rules.py::_required_count")
    }

    // TODO(integrator-2): wire to `instr_rules::_real_params`.
    pub(super) fn _real_params(_params: &[i64]) -> Vec<i64> {
        todo!("checker/instr_rules.py::_real_params")
    }

    pub(super) use crate::checker::structure::{
        _FOR_MISSING, _for_header_incomplete, _for_header_message, _for_header_parts,
    };

    pub(super) fn _for_init_match(text: &str) -> Option<String> {
        crate::checker::structure::part6::_FOR_INIT_RE
            .match_(text)
            .and_then(|m| m.group(1).map(str::to_owned))
    }

    pub(super) fn _for_to_start(text: &str) -> Option<usize> {
        crate::checker::structure::part6::_FOR_TO_RE
            .match_(text)
            .and_then(|m| m.start(1))
    }
}

const _CONST_FIRST_PARAM: [i64; 4] = [1, 5, 38, 39];

const _GENERIC_FALLBACK_PREFIX: &str = "Invalid, or out of place expression: ";
static _LINE_WORD_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[A-Za-z_][A-Za-z0-9_]*", 0));

static _TAIL_REJECTING: LazyLock<BTreeSet<String>> = LazyLock::new(|| {
    "BeginProg PipeLineMode SequentialMode EndIf WEND OpenInterval NEXTSCAN NEXTSUBSCAN ENDTABLE \
     FillStop ExitScan ContinueScan ExitSub ExitDo ExitFor Restore AngleDegrees ShutdownBegin \
     ShutdownEnd EndSequence ExitFunction AbortScan SampleFieldCal WaitTriggerSequence \
     PulseCountReset WebPageEnd TableHide DHCPRenew Restart PreserveVariables \
     ApplyAndRestartSequence EndApplyAndRestartSequence"
        .split_whitespace()
        .map(pystr::upper)
        .collect()
});

static _ZERO_TAIL_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\s*([A-Za-z_]\w*)(?=[\s,(])(.*)$", pyre::S));

static _LEAD_OP_WORD_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)(is|mod|and|or|xor|imp|eqv|to)(?![A-Za-z0-9_])", 0));
const _LEAD_OP_NO_OPERAND: [&str; 2] = ["IS", "MOD"];
static _LEAD_OP_SKIP_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)^[ \t]*(?:(?:and|or|xor|imp|eqv|to|not|mod|is)(?![A-Za-z0-9_])[ \t]*)*",
        0,
    )
});
static _INSTRUCTION_PROSE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"\s*([A-Za-z_][A-Za-z0-9_]*)[ \t]+([A-Za-z_][^(]*)$",
        pyre::S,
    )
});
static _LINE_OPERAND_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"[A-Za-z_][A-Za-z0-9_]*|\d+(?:\.\d+)?\b|"[^"]*""#, 0));

static _LINE_OP_WORD_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"[ \t]+(?i:and|or|xor|eqv|imp|mod|to)[ \t]+(?=[A-Za-z_0-9"])"#,
        0,
    )
});
static _TABLE_LINE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\s*([A-Za-z_][A-Za-z0-9_]*)\.", 0));
static _SIGNED_TABLE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\s*[-+][ \t]*([A-Za-z_][A-Za-z0-9_]*)\.(?=[A-Za-z_])", 0));
static _LINE_OP_RUN_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?:[ \t]*(?:<>|<=|>=|[-+*/^&<>=])[ \t]*)(?=[A-Za-z_0-9"])|(?:<>|<=|>=|[-+*/^&<>=])"#,
        0,
    )
});
static _LINE_WS_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[ \t]+", 0));
static _LINE_PAREN_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[ \t]*\(", 0));

static _AMP_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"&([A-Za-z0-9_]*)", 0));
static _HEXED_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"&[hH][0-9A-Fa-f]+", 0));
static _LEAD_OP_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[ \t]*(?:<>|<=|>=|<<|>>|[=<>])[ \t]*(?=[A-Za-z_])", 0));
static _MINUS_DIGIT_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"-\d", 0));

/// Port of `checker/cr3xx.py::_split_top_commas`.
pub fn _split_top_commas(text: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut depth: i64 = 0;
    let mut cur = String::new();
    for ch in text.chars() {
        depth += i64::from(ch == '(') - i64::from(ch == ')');
        if ch == ',' && depth <= 0 {
            parts.push(std::mem::take(&mut cur));
        } else {
            cur.push(ch);
        }
    }
    parts.push(cur);
    parts
}

/// Port of `checker/cr3xx.py::_inside_table`.
pub fn _inside_table(program: &Program, line: i64, file: &str) -> bool {
    let mut inside = false;
    for st in &program.statements {
        let base = st.base();
        if base.node.line == line && base.node.file == file {
            return inside;
        }
        if let Stmt::Instruction(inst) = st.as_ref() {
            let kw = pystr::upper(&inst.keyword);
            if kw == "DATATABLE" {
                inside = true;
            } else if kw == "ENDTABLE" {
                inside = false;
            }
        }
    }
    false
}

/// Port of `checker/cr3xx.py::_loose_before_beginprog`.
pub fn _loose_before_beginprog(program: &Program, line: i64, file: &str) -> bool {
    let mut depth: i64 = 0;
    for st in &program.statements {
        let base = st.base();
        if base.node.line == line && base.node.file == file {
            return depth == 0;
        }
        let kw = match st.as_ref() {
            Stmt::Instruction(inst) => pystr::upper(&inst.keyword),
            _ => String::new(),
        };
        if kw == "BEGINPROG" {
            return false;
        }
        if matches!(st.as_ref(), Stmt::SubDef(_))
            || matches!(
                kw.as_str(),
                "DATATABLE" | "CONSTTABLE" | "DISPLAYMENU" | "STRUCTURETYPE"
            )
        {
            depth += 1;
        } else if ends_routine(st.as_ref())
            || matches!(
                kw.as_str(),
                "ENDTABLE" | "ENDCONSTTABLE" | "ENDMENU" | "ENDSTRUCTURETYPE"
            )
        {
            depth = (depth - 1).max(0);
        }
    }
    false
}

/// Port of `checker/cr3xx.py::prose_known`.
pub fn prose_known(_program: &Program) -> BTreeSet<String> {
    todo!("checker/cr3xx.py::prose_known (omitted: profile: untyped)")
}

/// Port of `checker/cr3xx.py::prose_known`.
pub fn prose_known_with(program: &ProgramRef, profile: &'static Profile) -> BTreeSet<String> {
    let symbols = profile.symbols();
    let functions = profile.functions();
    let instruction_only: BTreeSet<&str> = profile
        .instructions()
        .iter()
        .map(|(k, _)| k)
        .filter(|k| !symbols.contains(k) && !functions.contains(k))
        .collect();
    let ctx = context::of(program, Some(profile), None);
    let known = ctx.known_names();
    known
        .names
        .iter()
        .filter(|n| !instruction_only.contains(n.as_str()) && !known.tables.contains(*n))
        .cloned()
        .collect()
}

/// Port of `checker/cr3xx.py::prose_known_tables`.
pub fn prose_known_tables(_program: &Program) -> BTreeSet<String> {
    todo!("checker/cr3xx.py::prose_known_tables (omitted: profile: untyped)")
}

/// Port of `checker/cr3xx.py::prose_known_tables`.
pub fn prose_known_tables_with(
    program: &ProgramRef,
    profile: &'static Profile,
) -> BTreeSet<String> {
    let ctx = context::of(program, Some(profile), None);
    ctx.known_names().tables.clone()
}

/// Port of `checker/cr3xx.py::first_declarations`.
pub fn first_declarations(_program: &Program) {
    todo!("checker/cr3xx.py::first_declarations")
}

/// Port of `checker/cr3xx.py::first_declarations`.
pub fn first_declarations_with(program: &Program) -> OrderedMap<String, (String, i64)> {
    let mut first: OrderedMap<String, (String, i64)> = OrderedMap::new();
    for st in &program.statements {
        let base = st.base();
        let declaration_vars = match st.as_ref() {
            Stmt::Declaration(d) => Some(d.vars.borrow()),
            _ => None,
        };
        let mut names: Vec<&str> = Vec::new();
        match st.as_ref() {
            Stmt::Declaration(decl) => {
                for v in declaration_vars.as_ref().unwrap().iter() {
                    names.push(&v.name);
                }
            }
            Stmt::ConstDecl(c) => {
                names.push(&c.name);
            }
            Stmt::Instruction(inst) if pystr::upper(&inst.keyword) == "ALIAS" => {
                for arg in inst.args.iter().skip(1).flatten() {
                    if let Expr::Name(n) = arg.as_ref() {
                        names.push(&n.name);
                    }
                }
            }
            _ => {}
        }
        for n in names {
            let key = pystr::upper(n);
            if !first.contains_key(&key) {
                first.insert(key, (base.node.file.clone(), base.node.line));
            }
        }
    }
    first
}

/// Port of `checker/cr3xx.py::declared_later`.
pub fn declared_later(_line: i64, _file: &str) -> BTreeSet<String> {
    todo!("checker/cr3xx.py::declared_later (omitted: first: dict[str, tuple[str, int]])")
}

/// Port of `checker/cr3xx.py::declared_later`.
pub fn declared_later_with(
    first: &OrderedMap<String, (String, i64)>,
    line: i64,
    file: &str,
) -> BTreeSet<String> {
    first
        .iter()
        .filter(|&(_, (f, l))| f == file && *l > line)
        .map(|(n, _)| n.clone())
        .collect()
}

/// Port of `checker/cr3xx.py::_paren_items_words`.
pub fn _paren_items_words(inner: &str, known: &BTreeSet<String>) -> Vec<String> {
    let mut items = Vec::new();
    let mut depth: i64 = 0;
    let mut cur = String::new();
    for ch in inner.chars() {
        if ch == '(' {
            depth += 1;
        } else if ch == ')' {
            if depth == 0 {
                break;
            }
            depth -= 1;
        }
        if ch == ',' && depth == 0 {
            items.push(std::mem::take(&mut cur));
        } else {
            cur.push(ch);
        }
    }
    items.push(cur);
    if items.len() == 1 && !inner.contains('&') {
        return _leading_unknown_words(inner, known);
    }
    let mut words: Vec<String> = Vec::new();
    for (idx, raw_item) in items.into_iter().take(4).enumerate() {
        let stripped = pystr::lstrip_chars(&raw_item, " \t");
        let mut owned_item: Option<String> = None;
        if let Some(amp) = _AMP_RE.match_(stripped) {
            let hexed = _HEXED_RE.match_(stripped);
            if idx == 3 {
                if hexed.is_some()
                    && let Some(g1) = amp.group(1)
                    && !known.contains(g1)
                {
                    words.push(g1.to_owned());
                }
                continue;
            }
            let Some(hex_m) = hexed else {
                words.push(amp.group0().to_owned());
                continue;
            };
            let rest = pystr::slice(stripped, Some(pending_wiring::to_i64(hex_m.end0())), None);
            owned_item = Some(format!("0{rest}"));
        }
        let item_str = owned_item.as_deref().unwrap_or(stripped);
        let mut peeked = false;
        let found =
            _leading_unknown_words_with(item_str, known, &BTreeSet::new(), Some(&mut peeked));
        if idx == 3 {
            words.extend(found.into_iter().take(1));
        } else {
            words.extend(found);
        }
        if peeked {
            break;
        }
    }
    words
}

/// Port of `checker/cr3xx.py::_leading_unknown_words`.
pub fn _leading_unknown_words(text: &str, known: &BTreeSet<String>) -> Vec<String> {
    _leading_unknown_words_with(text, known, &BTreeSet::new(), None)
}

/// Port of `checker/cr3xx.py::_leading_unknown_words`.
pub fn _leading_unknown_words_with(
    text: &str,
    known: &BTreeSet<String>,
    callables: &BTreeSet<String>,
    mut state_peeked: Option<&mut bool>,
) -> Vec<String> {
    if let Some(lead_op) = _LEAD_OP_RE.match_(text) {
        let word = _LINE_WORD_RE
            .match_at(text, lead_op.end0())
            .expect("lead_op ends before a letter");
        let w = word.group0();
        return if known.contains(&pystr::upper(w)) {
            Vec::new()
        } else {
            vec![w.to_owned()]
        };
    }
    let pos0 = pystr::len(text) - pystr::len(pystr::lstrip_chars(text, " \t"));
    let mut pos = pos0;
    let mut owned_text: Option<String> = None;
    if _MINUS_DIGIT_RE
        .match_(pystr::slice(
            text,
            Some(pending_wiring::to_i64(pos)),
            Some(pending_wiring::to_i64(pos + 2)),
        ))
        .is_some()
    {
        pos += 1;
    } else if pystr::slice(
        text,
        Some(pending_wiring::to_i64(pos)),
        Some(pending_wiring::to_i64(pos + 1)),
    ) == "\""
        && let Some(close) = pystr::find(text, "\"", Some(pending_wiring::to_i64(pos + 1)), None)
        && close > 0
    {
        let prefix = pystr::slice(text, None, Some(pending_wiring::to_i64(pos)));
        let suffix = pystr::slice(text, Some(pending_wiring::to_i64(close + 1)), None);
        owned_text = Some(format!("{prefix}0{suffix}"));
    }
    let text = owned_text.as_deref().unwrap_or(text);
    if _LINE_OPERAND_RE.match_at(text, pos).is_none() {
        return Vec::new();
    }
    if pystr::upper(pystr::slice(
        text,
        Some(pending_wiring::to_i64(pos)),
        Some(pending_wiring::to_i64(pos + 3)),
    )) == "IS "
        || pystr::upper(pystr::slice(text, Some(pending_wiring::to_i64(pos)), None)) == "IS"
    {
        return Vec::new();
    }
    let mut words: Vec<String> = Vec::new();
    let mut extending = true;
    loop {
        let operand = if extending {
            _LINE_OPERAND_RE.match_at(text, pos)
        } else {
            _LINE_WORD_RE.match_at(text, pos)
        };
        let Some(op_m) = operand else {
            break;
        };
        pos = op_m.end0();
        let g0 = op_m.group0();
        let is_lit = g0
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_digit() || c == '"');
        if is_lit {
            // number or string literal
        } else if pystr::slice(
            text,
            Some(pending_wiring::to_i64(pos)),
            Some(pending_wiring::to_i64(pos + 1)),
        ) == "."
        {
            break;
        } else {
            words.push(g0.to_owned());
        }
        if !extending {
            break;
        }
        let paren = if is_lit {
            None
        } else {
            _LINE_PAREN_RE.match_at(text, pos)
        };
        if let Some(paren_m) = paren
            && words.len() == 1
        {
            let w0_up = pystr::upper(&words[0]);
            if !known.contains(&w0_up) || callables.contains(&w0_up) {
                words.extend(_paren_items_words(
                    pystr::slice(text, Some(pending_wiring::to_i64(paren_m.end0())), None),
                    known,
                ));
                break;
            }
        }
        if let Some(op_match) = _LINE_OP_RUN_RE
            .match_at(text, pos)
            .or_else(|| _LINE_OP_WORD_RE.match_at(text, pos))
        {
            pos = op_match.end0();
            continue;
        }
        if let Some(ws_match) = _LINE_WS_RE.match_at(text, pos)
            && _LINE_WORD_RE.match_at(text, ws_match.end0()).is_some()
        {
            pos = ws_match.end0();
            extending = false;
            if let Some(peeked) = state_peeked.as_deref_mut() {
                *peeked = true;
            }
            continue;
        }
        break;
    }
    words
        .into_iter()
        .filter(|w| !known.contains(&pystr::upper(w)))
        .collect()
}

const _CALL_ADJACENT_EXCLUDED: [&str; 15] = [
    "DATA", "DATALONG", "ALIAS", "UNITS", "SUB", "FUNCTION", "IF", "ELSEIF", "WHILE", "UNTIL",
    "SELECT", "CASE", "FOR", "DO", "LOOP",
];
const _ADJACENT_CONST_CODES: [i64; 5] = [1, 5, 20, 38, 39];
const _ADJACENT_EXPR_CODES: [i64; 13] = [2, 8, 24, 25, 26, 27, 28, 29, 33, 40, 41, 47, 48];
static _ADJACENT_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"[\w")]\s+[\w"]"#, 0));
static _ADJACENT_OP_WORD_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(?:and|or|xor|eqv|imp|mod|is|to|not)\b", 0));
static _STRIP_QUOTED_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#""[^"]*""#, 0));

/// Port of `checker/cr3xx.py::_split_args_quoted`.
pub fn _split_args_quoted(text: &str) -> Option<Vec<String>> {
    let mut parts = Vec::new();
    let mut depth: i64 = 0;
    let mut cur = String::new();
    let mut quoted = false;
    for ch in text.chars() {
        if ch == '"' {
            quoted = !quoted;
        } else if !quoted {
            if ch == '(' {
                depth += 1;
            } else if ch == ')' {
                depth -= 1;
                if depth < -1 {
                    return None;
                }
            } else if ch == ',' && depth == 0 {
                parts.push(std::mem::take(&mut cur));
                continue;
            }
        }
        cur.push(ch);
    }
    parts.push(cur);
    if quoted {
        return None;
    }
    Some(parts)
}

/// Port of `checker/cr3xx.py::_adjacent_operands`.
pub fn _adjacent_operands(arg: &str) -> bool {
    let stripped = _STRIP_QUOTED_RE.sub("\"", arg);
    _ADJACENT_RE.search(&stripped).is_some() && _ADJACENT_OP_WORD_RE.search(arg).is_none()
}

const _ADJACENT_TABLE_SOURCES: [&str; 6] = [
    "SAMPLE", "AVERAGE", "MAXIMUM", "MINIMUM", "TOTALIZE", "STDDEV",
];

/// Port of `checker/cr3xx.py::_call_adjacent_arg_messages`.
pub fn _call_adjacent_arg_messages(
    _inner: &str,
    _here: &BTreeSet<String>,
    _variables: &BTreeSet<String>,
    _in_table: bool,
) -> Option<Vec<String>> {
    todo!("checker/cr3xx.py::_call_adjacent_arg_messages (omitted: record: dict, profile: untyped)")
}

/// Port of `checker/cr3xx.py::_call_adjacent_arg_messages`.
pub fn _call_adjacent_arg_messages_with(
    inner: &str,
    record: &profiles::Instruction,
    here: &BTreeSet<String>,
    variables: &BTreeSet<String>,
    profile: &'static Profile,
    in_table: bool,
) -> Option<Vec<String>> {
    let params = &record.params;
    let args = _split_args_quoted(inner)?;
    let mut found: Option<(usize, bool, String)> = None;
    for (k, raw_arg) in args.iter().enumerate() {
        let last = k + 1 == args.len();
        let mut arg = raw_arg.as_str();
        let owned: String;
        if last
            && pystr::rstrip(arg).ends_with(')')
            && arg.matches('(').count() < arg.matches(')').count()
        {
            let r = pystr::rstrip(arg);
            owned = pystr::slice(r, None, Some(-1)).to_owned();
            arg = &owned;
        }
        if _adjacent_operands(arg) {
            found = Some((k, last, arg.to_owned()));
            break;
        }
        if k < params.len() && matches!(params[k] % 100, 6 | 11) {
            continue;
        }
        if pystr::strip(arg).is_empty()
            || arg.matches('(').count() != arg.matches(')').count()
            || !_leading_unknown_words(arg, here).is_empty()
        {
            return None;
        }
    }
    let (k, last, arg) = found?;
    if k >= params.len() {
        return None;
    }
    let code = params[k] % 100;
    if code == 11 {
        if !in_table || !_ADJACENT_TABLE_SOURCES.contains(&pystr::upper(&record.name).as_str()) {
            return None;
        }
        let carg = if last {
            pystr::strip(&arg)
        } else {
            pystr::lstrip(&arg)
        };
        if let Some(short) =
            pending_wiring::shortfall_message(record, pending_wiring::given_count_of(&args))
        {
            return Some(vec![short]);
        }
        let mut out: Vec<String> = _leading_unknown_words(carg, here)
            .into_iter()
            .map(|w| profile.unresolved_symbol_message.format(&w))
            .collect();
        out.push(format!("Incomplete expression: {carg}."));
        return Some(out);
    }
    if !_ADJACENT_CONST_CODES.contains(&code) && !_ADJACENT_EXPR_CODES.contains(&code) {
        return None;
    }
    let carg = if last {
        pystr::strip(&arg)
    } else {
        pystr::lstrip(&arg)
    };
    let mut out: Vec<String> = _leading_unknown_words(carg, here)
        .into_iter()
        .map(|w| profile.unresolved_symbol_message.format(&w))
        .collect();
    out.push(format!("Incomplete expression: {carg}."));
    if _ADJACENT_CONST_CODES.contains(&code) {
        let functions: BTreeSet<String> = profile
            .functions()
            .iter()
            .map(|(n, _)| n.to_owned())
            .collect();
        let chain = _leading_unknown_words_with(carg, &BTreeSet::new(), &functions, None);
        if chain.into_iter().any(|w| {
            let wu = pystr::upper(&w);
            !functions.contains(&wu) && (variables.contains(&wu) || !here.contains(&wu))
        }) {
            out.push(format!("Variable illegal in Parameter: {carg}."));
        }
    } else if !last {
        let rest = args[k + 1..].join(",");
        let token = pending_wiring::_next_simple_token(&rest);
        if !token.is_empty() {
            out.push(format!("Too many parameters:  {token}."));
        }
    }
    Some(out)
}

/// Port of `checker/cr3xx.py::order_email_destination`.
pub fn order_email_destination(_program: &Program, _diags: &mut Diagnostics) {
    todo!("checker/cr3xx.py::order_email_destination (omitted: profile: untyped)")
}

/// Port of `checker/cr3xx.py::order_email_destination`.
pub fn order_email_destination_with(
    _program: &Program,
    profile: &'static Profile,
    diags: &mut Diagnostics,
) {
    if !matches!(profile.name, "cr300" | "cr350") {
        return;
    }
    let items = &mut diags.items;
    for i in 0..items.len() {
        if items[i].message != pending_wiring::_SERIALIN_DEST_MESSAGE {
            continue;
        }
        let d_line = items[i].line;
        let d_file = items[i].file.clone();
        let mut j = i + 1;
        while j < items.len()
            && items[j].line == d_line
            && items[j].file == d_file
            && items[j]
                .message
                .ends_with(pending_wiring::_UNRESOLVED_SUFFIX)
        {
            j += 1;
        }
        if j > i + 1 {
            items[i..j].rotate_left(1);
        }
    }
}

const _SHORTFALL_MESSAGES: [&str; 2] = [
    "Invalid or missing parameter.",
    "Too few parameters in instruction.",
];

/// Port of `checker/cr3xx.py::drop_shortfall_with_names`.
pub fn drop_shortfall_with_names(_program: &Program, _diags: &mut Diagnostics) {
    todo!("checker/cr3xx.py::drop_shortfall_with_names (omitted: profile: untyped)")
}

/// Port of `checker/cr3xx.py::drop_shortfall_with_names`.
pub fn drop_shortfall_with_names_with(
    program: &ProgramRef,
    profile: &'static Profile,
    diags: &mut Diagnostics,
) {
    if !matches!(profile.name, "cr300" | "cr350") {
        return;
    }
    let mut named: BTreeSet<(i64, String)> = diags
        .items
        .iter()
        .filter(|d| {
            d.severity == "error" && d.message.ends_with(pending_wiring::_UNRESOLVED_SUFFIX)
        })
        .map(|d| (d.line, d.file.clone()))
        .collect();
    let illegal: BTreeSet<(i64, String)> = diags
        .items
        .iter()
        .filter(|d| {
            d.severity == "error"
                && d.message
                    .starts_with(pending_wiring::_VARIABLE_ILLEGAL_PREFIX)
        })
        .map(|d| (d.line, d.file.clone()))
        .collect();
    named.extend(illegal.iter().cloned());
    if named.is_empty() {
        return;
    }
    let stmts = &program.statements;
    let ctx = context::of(program, Some(profile), None);
    let known = ctx.known_names();
    let known_view = expressions::part1::KnownView {
        names: &known.names,
        tables: &known.tables,
        out_of_scope: &known.out_of_scope,
        dotted_tables: known.dotted_tables,
    };

    let has_names = |stmt: &Stmt| -> bool {
        let base = stmt.base();
        if matches!(stmt, Stmt::Instruction(_))
            && illegal.contains(&(base.node.line, base.node.file.clone()))
        {
            return true;
        }
        match stmt {
            Stmt::Instruction(inst) => inst.args.iter().any(|e| {
                !expressions::part1::_undefined_names(e.as_deref(), &known_view).is_empty()
            }),
            Stmt::Assignment(assign) => {
                !expressions::part1::_undefined_names(assign.target.as_deref(), &known_view)
                    .is_empty()
                    || !expressions::part1::_undefined_names(assign.value.as_deref(), &known_view)
                        .is_empty()
            }
            _ => false,
        }
    };

    let is_short = |stmt: &Stmt| -> bool {
        let Stmt::Instruction(inst) = stmt else {
            return false;
        };
        let Some(record) = profile.instructions().get(&pystr::upper(&inst.keyword)) else {
            return false;
        };
        pending_wiring::given_count(inst, profile)
            < pending_wiring::_required_count(&pending_wiring::_real_params(&record.params))
    };

    let first = stmts.iter().position(|st| {
        let base = st.base();
        named.contains(&(base.node.line, base.node.file.clone())) && has_names(st.as_ref())
    });
    let with_names: BTreeSet<(i64, String)> = stmts
        .iter()
        .filter(|st| has_names(st.as_ref()))
        .map(|st| {
            let base = st.base();
            (base.node.line, base.node.file.clone())
        })
        .collect();
    let line_only: BTreeSet<(i64, String)> = named.difference(&with_names).cloned().collect();
    let first_key = first.map(|idx| {
        let base = stmts[idx].base();
        (base.node.line, base.node.file.clone())
    });

    let mut seen: BTreeMap<(i64, String), usize> = BTreeMap::new();
    let mut kept = Vec::with_capacity(diags.items.len());
    for d in &diags.items {
        let is_hidden =
            if d.severity != "error" || !_SHORTFALL_MESSAGES.contains(&d.message.as_str()) {
                false
            } else {
                let key = (d.line, d.file.clone());
                let idxs: Vec<usize> = stmts
                    .iter()
                    .enumerate()
                    .filter(|(_, st)| {
                        let base = st.base();
                        base.node.line == key.0 && base.node.file == key.1
                    })
                    .map(|(i, _)| i)
                    .collect();
                if idxs.iter().any(|&i| {
                    matches!(
                        stmts[i].as_ref(),
                        Stmt::Unparsed(u) if u.reason == pending_wiring::_FOR_MISSING
                    )
                }) {
                    false
                } else {
                    let short: Vec<usize> = idxs
                        .iter()
                        .copied()
                        .filter(|&i| is_short(stmts[i].as_ref()))
                        .collect();
                    let nth = *seen.get(&key).unwrap_or(&0);
                    seen.insert(key.clone(), nth + 1);
                    if line_only.contains(&key)
                        || (named.contains(&key)
                            && (idxs.is_empty()
                                || (short.is_empty() && first_key.as_ref() == Some(&key))))
                    {
                        true
                    } else if let Some(first_idx) = first {
                        let cand = if nth < short.len() {
                            short[nth]
                        } else if !short.is_empty() {
                            short[0]
                        } else {
                            idxs[0]
                        };
                        let has_args = match stmts[cand].as_ref() {
                            Stmt::Instruction(inst) => !inst.args.is_empty(),
                            _ => true,
                        };
                        cand >= first_idx && has_args
                    } else {
                        false
                    }
                }
            };
        if !is_hidden {
            kept.push(d.clone());
        }
    }
    diags.items = kept;

    for (i, st) in stmts.iter().enumerate() {
        let is_for_missing = matches!(
            st.as_ref(),
            Stmt::Unparsed(u) if u.reason == pending_wiring::_FOR_MISSING
        );
        if !(is_for_missing || is_short(st.as_ref())) {
            continue;
        }
        let base = st.base();
        let key_line = base.node.line;
        let key_file = &base.node.file;
        if i + 1 < stmts.len() {
            let next_base = stmts[i + 1].base();
            if next_base.node.line == key_line && next_base.node.file == *key_file {
                continue;
            }
        }
        if !stmts[..i].iter().any(|o| {
            let ob = o.base();
            ob.node.line == key_line && ob.node.file == *key_file && has_names(o.as_ref())
        }) {
            continue;
        }
        if stmts.iter().any(|o| {
            let ob = o.base();
            ob.node.line == key_line
                && ob.node.file == *key_file
                && !Rc::ptr_eq(o, st)
                && (is_short(o.as_ref())
                    || matches!(
                        o.as_ref(),
                        Stmt::Unparsed(u) if u.reason == pending_wiring::_FOR_MISSING
                    ))
        }) {
            continue;
        }
        if let Some(moved0) = diags
            .items
            .iter()
            .find(|d| {
                d.line == key_line
                    && d.file == *key_file
                    && _SHORTFALL_MESSAGES.contains(&d.message.as_str())
            })
            .cloned()
            && let Some(at) = diags
                .items
                .iter()
                .rposition(|d| d.line == key_line && d.file == *key_file)
        {
            diags.items.retain(|d| !Rc::ptr_eq(d, &moved0));
            diags.items.insert(at, moved0);
        }
    }
}

static _FOR_PREFIX_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s*For\s", pyre::I));
static _FOR_EQ_TO_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"=(.*?)\sTo\s", pyre::I | pyre::S));

/// Port of `checker/cr3xx.py::check_for_header`.
pub fn check_for_header(_program: &Program, _diags: &mut Diagnostics) {
    todo!("checker/cr3xx.py::check_for_header (omitted: profile: untyped)")
}

/// Port of `checker/cr3xx.py::check_for_header`.
pub fn check_for_header_with(
    program: &ProgramRef,
    profile: &'static Profile,
    diags: &mut Diagnostics,
) {
    if !matches!(profile.name, "cr300" | "cr350") {
        return;
    }
    let mut known: Option<BTreeSet<String>> = None;
    for stmt in &program.statements {
        let base = stmt.base();
        let text = &base.text;
        if _FOR_PREFIX_RE.match_(text).is_none() {
            continue;
        }
        let mut kind = "incomplete".to_owned();
        let found = if let Some(f) = pending_wiring::_for_header_incomplete(text, profile) {
            f
        } else {
            let parts = pending_wiring::_for_header_parts(text, profile);
            if parts.is_empty() {
                continue;
            }
            kind.clone_from(&parts[0].0);
            let init = pending_wiring::_for_init_match(text);
            let is_init = init.as_deref() == Some(parts[0].1.as_str());
            (parts[0].1.clone(), is_init)
        };
        let (echo, is_init) = found;
        let key_line = base.node.line;
        let key_file = &base.node.file;
        let own_ok = diags
            .items
            .iter()
            .filter(|d| d.line == key_line && d.file == *key_file)
            .all(|d| {
                d.message.ends_with(pending_wiring::_UNRESOLVED_SUFFIX)
                    || d.message.starts_with(_GENERIC_FALLBACK_PREFIX)
                    || d.message.starts_with("Incomplete expression:")
                    || d.message.starts_with("No Matching, or out of place )")
            });
        if !own_ok {
            continue;
        }
        let known_set = known.get_or_insert_with(|| prose_known_with(program, profile));
        let later = declared_later_with(&first_declarations_with(program), key_line, key_file);
        let here: BTreeSet<String> = known_set.difference(&later).cloned().collect();
        let start = pending_wiring::_for_to_start(text).expect("_for_header_incomplete matched");
        let eq = _FOR_EQ_TO_RE.search(pystr::slice(
            text,
            None,
            Some(pending_wiring::to_i64(start)),
        ));
        let mut words = if is_init {
            Vec::new()
        } else if let Some(eq_m) = eq
            && let Some(g1) = eq_m.group(1)
        {
            _leading_unknown_words(g1, &here)
        } else {
            Vec::new()
        };
        words.extend(_leading_unknown_words(&echo, &here));
        let at = diags
            .items
            .iter()
            .position(|d| d.line == key_line && d.file == *key_file)
            .unwrap_or(diags.items.len());
        diags
            .items
            .retain(|d| !(d.line == key_line && d.file == *key_file));
        let mut inserted: Vec<DiagRef> = words
            .into_iter()
            .map(|w| {
                Rc::new(Diagnostic::new(
                    "error",
                    key_line,
                    &profile.unresolved_symbol_message.format(&w),
                    key_file,
                    ShowFile::Bool(true),
                    0,
                    "",
                    "",
                ))
            })
            .collect();
        inserted.push(Rc::new(Diagnostic::basic(
            "error",
            key_line,
            &pending_wiring::_for_header_message(&kind, &echo),
            key_file,
        )));
        diags.items.splice(at..at, inserted);
    }
}

/// Port of `checker/cr3xx.py::_echo_key`.
pub fn _echo_key(text: &str) -> String {
    pystr::strip(&text.replace(['(', ')'], "")).to_owned()
}

#[cfg(test)]
mod tests {
    use super::{
        _adjacent_operands, _call_adjacent_arg_messages_with, _echo_key, _inside_table,
        _leading_unknown_words, _loose_before_beginprog, _split_args_quoted, _split_top_commas,
        BTreeSet, ConstDecl, Declaration, Diagnostics, Expr, ExprRef, Instruction, Program, Rc,
        Stmt, StmtRef, VarDecl, declared_later_with, first_declarations_with,
        order_email_destination_with, profiles, prose_known_tables_with, prose_known_with,
    };
    use crate::syntax::{Name, Node, StmtBase};

    fn make_instr(line: i64, kw: &str, args: Vec<Option<ExprRef>>) -> StmtRef {
        Rc::new(Stmt::Instruction(Instruction {
            base: StmtBase::new(line, "test.cr300"),
            keyword: kw.to_string(),
            args,
            ..Default::default()
        }))
    }

    fn make_name(line: i64, name: &str) -> ExprRef {
        Rc::new(Expr::Name(Name {
            node: Node::new(line, "test.cr300"),
            name: name.to_string(),
        }))
    }

    #[test]
    fn split_top_commas_handles_parens() {
        assert_eq!(
            _split_top_commas("a, f(b, c), d"),
            vec!["a", " f(b, c)", " d"]
        );
        assert_eq!(_split_top_commas("single"), vec!["single"]);
    }

    #[test]
    fn inside_table_and_loose_before_beginprog() {
        let stmts = vec![
            make_instr(1, "Public", vec![]),
            make_instr(2, "DataTable", vec![]),
            make_instr(3, "Sample", vec![]),
            make_instr(4, "EndTable", vec![]),
            make_instr(5, "BeginProg", vec![]),
            make_instr(6, "Scan", vec![]),
        ];
        let prog = Program::new("test.cr300", stmts);

        assert!(!_inside_table(&prog, 1, "test.cr300"));
        assert!(!_inside_table(&prog, 2, "test.cr300"));
        assert!(_inside_table(&prog, 3, "test.cr300"));
        assert!(_inside_table(&prog, 4, "test.cr300"));
        assert!(!_inside_table(&prog, 5, "test.cr300"));

        assert!(_loose_before_beginprog(&prog, 1, "test.cr300"));
        assert!(_loose_before_beginprog(&prog, 2, "test.cr300"));
        assert!(!_loose_before_beginprog(&prog, 3, "test.cr300"));
        assert!(!_loose_before_beginprog(&prog, 4, "test.cr300"));
        assert!(_loose_before_beginprog(&prog, 5, "test.cr300"));
        assert!(!_loose_before_beginprog(&prog, 6, "test.cr300"));
    }

    #[test]
    fn first_declarations_and_declared_later() {
        let decl = Rc::new(Stmt::Declaration(Declaration {
            base: StmtBase::new(2, "test.cr300"),
            kind: "Public".to_string(),
            vars: vec![Rc::new(VarDecl {
                name: "Alpha".to_string(),
                ..Default::default()
            })]
            .into(),
        }));
        let cdecl = Rc::new(Stmt::ConstDecl(ConstDecl {
            base: StmtBase::new(4, "test.cr300"),
            name: "Beta".to_string(),
            ..Default::default()
        }));
        let alias = make_instr(
            6,
            "Alias",
            vec![Some(make_name(6, "Alpha")), Some(make_name(6, "Gamma"))],
        );
        let prog = Program::new("test.cr300", vec![decl, cdecl, alias]);
        let first = first_declarations_with(&prog);
        assert_eq!(first.get("ALPHA"), Some(&("test.cr300".to_string(), 2)));
        assert_eq!(first.get("BETA"), Some(&("test.cr300".to_string(), 4)));
        assert_eq!(first.get("GAMMA"), Some(&("test.cr300".to_string(), 6)));

        let later = declared_later_with(&first, 3, "test.cr300");
        assert_eq!(
            later,
            BTreeSet::from(["BETA".to_string(), "GAMMA".to_string()])
        );
    }

    #[test]
    fn prose_known_excludes_instruction_only_and_tables() {
        let prof = profiles::get("cr300").unwrap();
        let dt = make_instr(1, "DataTable", vec![Some(make_name(1, "MyTable"))]);
        let prog = Rc::new(Program::new("test.cr300", vec![dt]));
        let known = prose_known_with(&prog, prof);
        assert!(!known.contains("BATTERY"));
        assert!(known.contains("SCAN"));
        assert!(known.contains("SIN"));
        assert!(!known.contains("MYTABLE"));
        let tables = prose_known_tables_with(&prog, prof);
        assert!(tables.contains("MYTABLE"));
    }

    #[test]
    fn leading_unknown_words_and_paren_items() {
        let known = BTreeSet::from(["IS".to_string(), "TO".to_string(), "DOUBLE".to_string()]);
        assert_eq!(
            _leading_unknown_words("alpha-beta-gamma delta", &known),
            vec!["alpha", "beta", "gamma", "delta"]
        );
        assert_eq!(
            _leading_unknown_words("foo bar baz qux", &known),
            vec!["foo", "bar"]
        );
        assert_eq!(_leading_unknown_words("foo, bar", &known), vec!["foo"]);
        assert_eq!(_leading_unknown_words("Part 1.", &known), vec!["Part"]);
        assert_eq!(_leading_unknown_words("= Year   ", &known), vec!["Year"]);
        assert_eq!(
            _leading_unknown_words("Is foo", &known),
            Vec::<String>::new()
        );
        assert_eq!(_leading_unknown_words("-1 Then", &known), vec!["Then"]);
        assert_eq!(
            _leading_unknown_words("foo and bar baz qux", &known),
            vec!["foo", "bar", "baz"]
        );
        assert_eq!(
            _leading_unknown_words("cv(zz, yy, 6, ww) text", &known),
            vec!["cv", "zz", "yy", "ww"]
        );
        assert_eq!(
            _leading_unknown_words("cv(&h0F, &6, 1, &h1A)", &known),
            vec!["cv", "&6", "h1A"]
        );
    }

    #[test]
    fn split_args_quoted_and_adjacent_operands() {
        assert_eq!(
            _split_args_quoted("a, \"b, c\", f(d, e))"),
            Some(vec![
                "a".to_string(),
                " \"b, c\"".to_string(),
                " f(d, e))".to_string()
            ])
        );
        assert_eq!(_split_args_quoted("\"unclosed"), None);
        assert_eq!(_split_args_quoted("a))"), None);

        assert!(_adjacent_operands("foo bar"));
        assert!(!_adjacent_operands("\"foo bar\""));
        assert!(!_adjacent_operands("foo and bar"));
    }

    #[test]
    fn call_adjacent_arg_messages_const_and_expr() {
        let prof = profiles::get("cr300").unwrap();
        let delay = prof.instructions().get("DELAY").unwrap();
        let here = BTreeSet::from(["X".to_string()]);
        let vars = BTreeSet::from(["X".to_string()]);
        let msgs =
            _call_adjacent_arg_messages_with("1, x y)", delay, &here, &vars, prof, false).unwrap();
        assert_eq!(
            msgs,
            vec!["y is not defined.", "Incomplete expression: x y."]
        );
        let const_msgs =
            _call_adjacent_arg_messages_with("x y)", delay, &here, &vars, prof, false).unwrap();
        assert_eq!(
            const_msgs,
            vec![
                "y is not defined.",
                "Incomplete expression: x y.",
                "Variable illegal in Parameter: x y.",
            ]
        );
    }

    #[test]
    fn order_email_destination_moves_after_unresolved() {
        let prof = profiles::get("cr300").unwrap();
        let prog = Program::new("test.cr300", vec![]);
        let mut diags = Diagnostics::new();
        diags.error_basic(
            10,
            "A string variable destination is required",
            "test.cr300",
        );
        diags.error_basic(10, "Foo is not defined.", "test.cr300");
        diags.error_basic(10, "Bar is not defined.", "test.cr300");
        order_email_destination_with(&prog, prof, &mut diags);
        let msgs: Vec<&str> = diags.items.iter().map(|d| d.message.as_str()).collect();
        assert_eq!(
            msgs,
            vec![
                "Foo is not defined.",
                "Bar is not defined.",
                "A string variable destination is required",
            ]
        );
    }

    #[test]
    fn echo_key_strips_parens_and_whitespace() {
        assert_eq!(_echo_key("  Foo(Bar)  "), "FooBar");
    }
}
