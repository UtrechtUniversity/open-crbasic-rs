//! Port of `checker/structure.py` (lines 780-1293).

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;
use std::sync::LazyLock;

use super::part1::{
    _BODY_BLOCK_ENDS, _BODY_BLOCK_STARTS, _DECL_BLOCK_BOUNDS, _accept_known_with,
    _calltable_known_with, _check_accept_table_with, _check_calltable_outside_scan_with,
    _scan_counts_for_table_placement_with, _track_tables_with, NESTED_SCAN, TableState,
    first_misplaced_sub_with,
};
use crate::checker::context;
use crate::consteval::Env;
use crate::diagnostics::{Diagnostics, ShowFile};
use crate::profiles::Profile;
use crate::syntax::{Case, Expr, ExprRef, NumberValue, OrderedMap, Program, Stmt, stmt_id};
use crate::{lexer, pyre, pystr};

/// Port of `checker/structure.py::_scan_placement_walk`.
pub fn _scan_placement_walk(_program: &Program, _diags: &mut Diagnostics) {
    todo!("checker/structure.py::_scan_placement_walk (omitted: profile: untyped)")
}

/// Port of checker/structure.py::_scan_placement_walk
#[allow(
    clippy::doc_markdown,
    reason = "AGENTS.md requires literal one-line port comments"
)]
pub fn _scan_placement_walk_with(
    program: &crate::syntax::ProgramRef,
    profile: &'static Profile,
    diags: &mut Diagnostics,
) -> Option<(String, i64)> {
    if !profile.single_pass {
        return None;
    }
    let env = Env::with_consts(profile, Rc::new(RefCell::new(program.consts.clone())));
    let (mut in_table, mut in_body, mut in_decl, mut seen_beginprog) = (false, false, false, false);
    let mut scans = Vec::new();
    let mut tables = TableState::default();
    let ctx = context::of(program, None, None);
    let subs = ctx.declared_subs();
    let mut stray = stray_controls_with(program);
    for (k, m) in scan_only_outside(program).iter() {
        stray.insert(*k, (m.clone(), true));
    }
    for (k, m) in subscan_controls_with(program, profile).iter() {
        stray.insert(*k, m.clone());
    }
    let misplaced = first_misplaced_sub_with(program);
    for stmt in &program.statements {
        if let Some((misplaced_stmt, messages)) = &misplaced
            && Rc::ptr_eq(stmt, misplaced_stmt)
        {
            for message in messages {
                diags.error_basic(stmt.line(), message, stmt.file());
            }
            return Some((stmt.file().to_owned(), stmt.line()));
        }
        let mut items = vec![stmt.clone()];
        if matches!(stmt.as_ref(), Stmt::If(_)) {
            items.extend(stmt.inline_bodies());
        }
        for item in items {
            if let Some((message, stops)) = stray.get(&stmt_id(&item)) {
                diags.error_basic(item.line(), message, item.file());
                if *stops {
                    return Some((item.file().to_owned(), item.line()));
                }
            }
        }
        seen_beginprog = seen_beginprog || _keyword(stmt) == "BEGINPROG";
        if matches!(stmt.as_ref(), Stmt::SubDef(_)) {
            in_body = true;
            continue;
        }
        let kw = _keyword(stmt);
        in_decl = _DECL_BLOCK_BOUNDS
            .get(kw.as_str())
            .copied()
            .unwrap_or(in_decl);
        _track_tables_with(stmt, &kw, &mut tables);
        if kw == "DATATABLE" {
            in_table = true;
        } else if kw == "ENDTABLE" {
            in_table = false;
        } else if kw == "SLOWSEQUENCE" && !scans.is_empty() {
            diags.error_basic(stmt.line(), "Missing NextScan.", stmt.file());
            return Some((stmt.file().to_owned(), stmt.line()));
        } else if kw == "SCAN" {
            if !scans.is_empty() {
                diags.error(
                    stmt.line(),
                    NESTED_SCAN,
                    stmt.file(),
                    ShowFile::Bool(true),
                    0,
                    "nested_scan",
                    "",
                );
            }
            scans.push(_scan_counts_for_table_placement_with(stmt, Some(&env)));
        } else if kw == "NEXTSCAN"
            || kw == "NEXT" && _block_word_with(stmt.args()).as_deref() == Some("SCAN")
        {
            if scans.is_empty() {
                diags.error_basic(stmt.line(), "NextScan with no Scan.", stmt.file());
                return Some((stmt.file().to_owned(), stmt.line()));
            }
            scans.pop();
        } else if kw == "CALLTABLE"
            && !in_table
            && (!scans.iter().any(|scan| *scan) || in_body)
            && _calltable_known_with(stmt, &tables)
        {
            if let Stmt::Instruction(inst) = stmt.as_ref() {
                _check_calltable_outside_scan_with(
                    inst,
                    program,
                    diags,
                    in_body,
                    !scans.is_empty() && !in_body,
                    Some(&env),
                );
            }
        } else if kw == "ACCEPTDATARECORDS" && !in_table && _accept_known_with(stmt, &tables) {
            if let Stmt::Instruction(inst) = stmt.as_ref() {
                _check_accept_table_with(
                    inst,
                    program,
                    diags,
                    !scans.iter().any(|scan| *scan) || in_body,
                    &env,
                );
            }
        }
        if _BODY_BLOCK_STARTS.contains(&kw.as_str()) {
            in_body = true;
        } else if crate::syntax::ends_routine(stmt) || _BODY_BLOCK_ENDS.contains(&kw.as_str()) {
            in_body = false;
        } else if !(seen_beginprog || in_body || in_table || in_decl)
            && (kw != "CALLTABLE" || _calltable_known_with(stmt, &tables))
            && let Some(display) = _before_beginprog_name_with(stmt, &kw, profile, subs)
            && !display.is_empty()
        {
            diags.error_basic(
                stmt.line(),
                &format!("{display} illegally occurs before BeginProg."),
                stmt.file(),
            );
            return Some((stmt.file().to_owned(), stmt.line()));
        }
    }
    None
}

static CASE_ELSE_RE: LazyLock<pyre::Regex> =
    LazyLock::new(|| pyre::Regex::new(r"\s*case\s+else", pyre::I));

/// Port of `checker/structure.py::_is_case_else`.
pub fn _is_case_else(stmt: &Case) -> bool {
    stmt.is_else || CASE_ELSE_RE.match_(&stmt.base.text).is_some()
}

static SELECT_RE: LazyLock<pyre::Regex> =
    LazyLock::new(|| pyre::Regex::new(r"\s*SELECT\b", pyre::I));

/// Port of `checker/structure.py::stray_controls`.
pub fn stray_controls(program: &Program) {
    let _ = stray_controls_with(program);
}

/// Port of `checker/structure.py::stray_controls`.
pub fn stray_controls_with(program: &Program) -> OrderedMap<usize, (String, bool)> {
    let mut found = OrderedMap::new();
    let mut elses = Vec::new();
    let mut opens = Vec::new();
    let (mut do_loops, mut for_loops, mut scans) = (0_i64, 0_i64, 0_i64);
    let mut in_sub = false;
    let mut seen_beginprog = false;
    for stmt in &program.statements {
        let kw = _keyword(stmt);
        let word = if matches!(kw.as_str(), "END" | "NEXT" | "EXIT") {
            _block_word_with(stmt.args())
        } else {
            None
        };
        let word = word.as_deref();
        let (mut message, mut stops) = (None, false);
        seen_beginprog |= kw == "BEGINPROG";
        if matches!(stmt.as_ref(), Stmt::SubDef(_)) {
            if !opens.is_empty() && !in_sub {
                message = Some("Sub declaration inside If..EndIf.".to_owned());
            }
            in_sub = true;
        } else if matches!(kw.as_str(), "ENDSUB" | "ENDFUNCTION")
            || kw == "END" && matches!(word, Some("SUB" | "FUNCTION"))
        {
            if !in_sub {
                message = Some("EndSub without Sub or nested subs.".to_owned());
            } else if !opens.is_empty() {
                message = Some("If without EndIf inside of Sub.".to_owned());
            }
            in_sub = false;
        } else if kw == "SELECT"
            || kw == "IF" && !matches!(stmt.as_ref(), Stmt::If(s) if s.inline.is_some())
            || matches!(stmt.as_ref(), Stmt::Unparsed(s) if SELECT_RE.match_(&s.base.text).is_some())
        {
            elses.push(false);
            opens.push(if kw == "IF" { "IF" } else { "SELECT" });
        } else if (kw == "ENDSELECT" || kw == "END" && word == Some("SELECT"))
            && !in_sub
            && !seen_beginprog
            && opens.contains(&"SELECT")
        {
            message = Some("EndSelect illegally occurs before BeginProg.".to_owned());
            stops = true;
        } else if matches!(kw.as_str(), "ENDIF" | "ENDSELECT")
            || kw == "END" && matches!(word, Some("IF" | "SELECT"))
        {
            elses.pop();
            opens.pop();
        } else if kw == "ELSE" || matches!(stmt.as_ref(), Stmt::Case(s) if _is_case_else(s)) {
            if let Some(last) = elses.last_mut() {
                if *last {
                    message = Some("If statement with duplicate Else.".to_owned());
                } else {
                    *last = true;
                }
            } else {
                message = Some("Else with no If.".to_owned());
                stops = true;
            }
        } else if (kw == "ELSEIF" || matches!(stmt.as_ref(), Stmt::Case(_))) && elses.is_empty() {
            message = Some("Case with no Select.".to_owned());
            stops = true;
        } else if matches!(kw.as_str(), "DO" | "FOR" | "WHILE") {
            if kw == "FOR" {
                for_loops += 1;
            } else {
                do_loops += 1;
            }
        } else if matches!(kw.as_str(), "LOOP" | "WEND") {
            do_loops = (do_loops - 1).max(0);
        } else if kw == "NEXT" && !matches!(word, Some("SCAN" | "SUBSCAN")) {
            for_loops = (for_loops - 1).max(0);
        } else if kw == "SCAN" {
            scans += 1;
        } else if kw == "NEXTSCAN" || kw == "NEXT" && word == Some("SCAN") {
            scans = (scans - 1).max(0);
        } else if kw == "EXIT"
            && matches!(word, Some("DO" | "FOR"))
            && (if word == Some("DO") {
                do_loops
            } else {
                for_loops
            }) == 0
        {
            let word = word.unwrap();
            message = Some(format!("Exit {} without {word}.", pystr::capitalize(word)));
        } else if kw == "EXIT" && matches!(word, Some("SUB" | "FUNCTION")) && !in_sub {
            message = Some("Exit Sub without Sub or nested subs.".to_owned());
        } else if matches!(kw.as_str(), "EXITSCAN" | "CONTINUESCAN") && scans == 0 {
            message = Some(format!(
                "{} not within a Scan.",
                if kw == "EXITSCAN" {
                    "ExitScan"
                } else {
                    "ContinueScan"
                }
            ));
        }
        if let Some(message) = message {
            found.insert(stmt_id(stmt), (message, stops));
        }
        if let Stmt::If(s) = stmt.as_ref()
            && s.inline.is_some()
            && (seen_beginprog || in_sub)
        {
            for sub in s
                .inline
                .iter()
                .flatten()
                .chain(s.inline_else.iter().flatten())
            {
                let sub_kw = _keyword(sub);
                let sub_word = if matches!(sub_kw.as_str(), "EXIT" | "END") {
                    _block_word_with(sub.args())
                } else {
                    None
                };
                let sub_word = sub_word.as_deref();
                let message = if sub_kw == "EXIT"
                    && matches!(sub_word, Some("DO" | "FOR"))
                    && (if sub_word == Some("DO") {
                        do_loops
                    } else {
                        for_loops
                    }) == 0
                {
                    let word = sub_word.unwrap();
                    Some(format!("Exit {} without {word}.", pystr::capitalize(word)))
                } else if sub_kw == "EXIT"
                    && matches!(sub_word, Some("SUB" | "FUNCTION"))
                    && !in_sub
                {
                    Some("Exit Sub without Sub or nested subs.".to_owned())
                } else if !in_sub
                    && (matches!(sub_kw.as_str(), "ENDSUB" | "ENDFUNCTION")
                        || sub_kw == "END" && matches!(sub_word, Some("SUB" | "FUNCTION")))
                {
                    Some("EndSub without Sub or nested subs.".to_owned())
                } else {
                    None
                };
                if let Some(message) = message {
                    found.insert(stmt_id(sub), (message, false));
                }
            }
        }
    }
    found
}

pub static SCAN_ONLY: LazyLock<OrderedMap<&'static str, &'static str>> = LazyLock::new(|| {
    let mut found = OrderedMap::new();
    for (kw, name) in [
        ("PULSECOUNT", "PulseCount"),
        ("QUADRATURE", "Quadrature"),
        ("TIMERINPUT", "TimerInput"),
    ] {
        found.insert(kw, name);
    }
    found
});

/// Port of `checker/structure.py::scan_only_outside`.
pub fn scan_only_outside(program: &Program) -> OrderedMap<usize, String> {
    let mut found = OrderedMap::new();
    let mut scans = 0_i64;
    let mut statements = Vec::new();
    for stmt in &program.statements {
        statements.push(stmt.clone());
        if matches!(stmt.as_ref(), Stmt::If(_)) {
            statements.extend(stmt.inline_bodies());
        }
    }
    for stmt in statements {
        let kw = _keyword(&stmt);
        if kw == "SCAN" {
            scans += 1;
        } else if kw == "NEXTSCAN"
            || kw == "NEXT" && _block_word_with(stmt.args()).as_deref() == Some("SCAN")
        {
            scans = (scans - 1).max(0);
        } else if let Some(name) = SCAN_ONLY.get(kw.as_str())
            && matches!(stmt.as_ref(), Stmt::Instruction(_))
            && scans == 0
        {
            found.insert(
                stmt_id(&stmt),
                format!("{name} must be used within a Scan...NextScan."),
            );
        }
    }
    found
}

pub const _SUBSCAN_ZERO_TIME: [&str; 13] = [
    "CALLTABLE",
    "SERIALIN",
    "ELSE",
    "ENDIF",
    "SELECT",
    "ENDSELECT",
    "NEXT",
    "END",
    "EXIT",
    "EXITSCAN",
    "CONTINUESCAN",
    "WHILE",
    "WEND",
];

pub const _SUBSCAN_PLAIN: [&str; 9] = [
    "Assignment",
    "Declaration",
    "ConstDecl",
    "Label",
    "If",
    "ElseIf",
    "Case",
    "DoLoop",
    "For",
];

/// Port of `checker/structure.py::_subscan_body_time`.
pub fn _subscan_body_time(_stmt: &Stmt) -> Option<bool> {
    todo!("checker/structure.py::_subscan_body_time (omitted: profile: untyped)")
}

/// Port of `checker/structure.py::_subscan_body_time`.
pub fn _subscan_body_time_with(stmt: &Stmt, profile: &Profile) -> Option<bool> {
    if matches!(
        stmt,
        Stmt::Assignment(_)
            | Stmt::Declaration(_)
            | Stmt::ConstDecl(_)
            | Stmt::Label(_)
            | Stmt::If(_)
            | Stmt::ElseIf(_)
            | Stmt::Case(_)
            | Stmt::DoLoop(_)
            | Stmt::For(_)
    ) {
        return Some(false);
    }
    let Stmt::Instruction(s) = stmt else {
        return None;
    };
    let kw = pystr::upper(&s.keyword);
    if _SUBSCAN_ZERO_TIME.contains(&kw.as_str()) || kw == "BATTERY" && profile.name == "cr6" {
        return Some(false);
    }
    if kw == "DELAY"
        && let Some(Some(arg)) = s.args.first()
        && let Expr::Number(n) = arg.as_ref()
        && match &n.value {
            NumberValue::Int(v) => *v != 0,
            NumberValue::Float(v) => *v != 0.0,
            NumberValue::BigInt(_) => true,
        }
    {
        return Some(false);
    }
    None
}

/// Port of `checker/structure.py::subscan_controls`.
pub fn subscan_controls(_program: &Program) {
    todo!("checker/structure.py::subscan_controls (omitted: profile: untyped)")
}

/// Port of `checker/structure.py::subscan_controls`.
pub fn subscan_controls_with(
    program: &Program,
    profile: &Profile,
) -> OrderedMap<usize, (String, bool)> {
    let mut found = OrderedMap::new();
    let mut scans = 0_i64;
    let mut in_sub = false;
    let mut frames = Vec::new();
    let mut flat = Vec::new();
    for stmt in &program.statements {
        flat.push(stmt.clone());
        if matches!(stmt.as_ref(), Stmt::If(_)) {
            flat.extend(stmt.inline_bodies());
        }
    }
    for stmt in flat {
        let kw = _keyword(&stmt);
        let word = if matches!(kw.as_str(), "END" | "NEXT") {
            _block_word_with(stmt.args())
        } else {
            None
        };
        if matches!(stmt.as_ref(), Stmt::SubDef(_)) {
            in_sub = true;
        } else if crate::syntax::ends_routine(&stmt) {
            in_sub = false;
        } else if kw == "SCAN" {
            scans += 1;
        } else if kw == "NEXTSCAN" || kw == "NEXT" && word.as_deref() == Some("SCAN") {
            scans = (scans - 1).max(0);
            frames.clear();
        } else if kw == "SUBSCAN" && matches!(stmt.as_ref(), Stmt::Instruction(_)) {
            if in_sub || scans == 0 {
                found.insert(
                    stmt_id(&stmt),
                    ("SubScan used outside of Scan .. NextScan.".to_owned(), true),
                );
                return found;
            }
            if !frames.is_empty() {
                found.insert(stmt_id(&stmt), ("Cannot Nest SubScans.".to_owned(), false));
            }
            frames.push(true);
        } else if kw == "NEXTSUBSCAN" || kw == "NEXT" && word.as_deref() == Some("SUBSCAN") {
            if frames.pop() == Some(true) && !profile.single_pass {
                found.insert(
                    stmt_id(&stmt),
                    ("No Measurements in SubScan.".to_owned(), false),
                );
            }
        } else if let Some(last) = frames.last_mut()
            && *last
            && _subscan_body_time_with(&stmt, profile).is_none()
        {
            *last = false;
        }
    }
    found
}

/// Port of `checker/structure.py::check_stray_controls`.
pub fn check_stray_controls(_program: &Program, _diags: &mut Diagnostics) -> Option<(String, i64)> {
    todo!("checker/structure.py::check_stray_controls (omitted: profile: untyped)")
}

/// Port of `checker/structure.py::check_stray_controls`.
pub fn check_stray_controls_with(
    program: &Program,
    profile: &Profile,
    diags: &mut Diagnostics,
) -> Option<(String, i64)> {
    if profile.single_pass {
        return None;
    }
    let mut found = stray_controls_with(program);
    for (k, m) in scan_only_outside(program).iter() {
        found.insert(*k, (m.clone(), true));
    }
    for (k, m) in subscan_controls_with(program, profile).iter() {
        found.insert(*k, m.clone());
    }
    let mut statements = Vec::new();
    for stmt in &program.statements {
        statements.push(stmt.clone());
        if matches!(stmt.as_ref(), Stmt::If(_)) {
            statements.extend(stmt.inline_bodies());
        }
    }
    for stmt in statements {
        if let Some((message, stops)) = found.get(&stmt_id(&stmt)) {
            let kind = if message.ends_with("must be used within a Scan...NextScan.") {
                "placement_scan"
            } else {
                ""
            };
            diags.error(
                stmt.line(),
                message,
                stmt.file(),
                ShowFile::Bool(true),
                0,
                kind,
                "",
            );
            if *stops {
                return Some((stmt.file().to_owned(), stmt.line()));
            }
        }
    }
    None
}

/// Port of `checker/structure.py::_has_error_on`.
pub fn _has_error_on(stmt: &Stmt, diags: &mut Diagnostics) -> bool {
    diags
        .items
        .iter()
        .any(|d| d.severity == "error" && d.line == stmt.line() && d.file == stmt.file())
}

/// Port of `checker/structure.py::_before_beginprog_name`.
pub fn _before_beginprog_name(_stmt: &Stmt, _kw: &str, _subs: &BTreeSet<String>) -> Option<String> {
    todo!("checker/structure.py::_before_beginprog_name (omitted: profile: untyped)")
}

static ESS_INITIALIZE_RE: LazyLock<pyre::Regex> =
    LazyLock::new(|| pyre::Regex::new(r"(?i)\s*ESSInitialize\s*\(", 0));

/// Port of `checker/structure.py::_before_beginprog_name`.
pub fn _before_beginprog_name_with(
    stmt: &Stmt,
    kw: &str,
    profile: &Profile,
    subs: &BTreeSet<String>,
) -> Option<String> {
    let control = _control_keyword(stmt);
    // _FORBIDDEN_BEFORE_BEGINPROG lives in part1.py; the Rust part1 currently omits it.
    if control.is_some()
        || matches!(stmt, Stmt::Instruction(_))
            && (matches!(kw, "SCAN" | "DELAY") || profile.before_beginprog().contains(kw))
    {
        let key = control.as_deref().unwrap_or(kw);
        if let Some(record) = profile.instructions().get(key) {
            return Some(record.name.clone());
        }
        return Some(control.unwrap_or_else(|| {
            if matches!(stmt, Stmt::Instruction(_)) {
                stmt.keyword().to_owned()
            } else {
                kw.to_owned()
            }
        }));
    }
    if matches!(stmt, Stmt::Unparsed(_))
        && kw == "UNPARSED"
        && ESS_INITIALIZE_RE.match_(stmt.text()).is_some()
        && _quote_in_first_arg(stmt.text())
    {
        return Some("ESSInitialize".to_owned());
    }
    if matches!(stmt, Stmt::Assignment(_)) {
        return Some("Expr".to_owned());
    }
    if let Stmt::Call(s) = stmt {
        let name = pystr::upper(&s.name);
        return if subs.contains(&name) {
            Some("Call".to_owned())
        } else if profile.functions().contains(&name) {
            Some("Expr".to_owned())
        } else {
            None
        };
    }
    None
}

/// Port of `checker/structure.py::_quote_in_first_arg`.
pub fn _quote_in_first_arg(text: &str) -> bool {
    let mut i = pystr::find(text, "(", None, None).map_or(0, |i| i + 1);
    while i > 0 && i < pystr::len(text) {
        if pystr::char_at(text, i64::try_from(i).unwrap()) == Some('"') {
            let Some(j) = pystr::find(text, "\"", Some(i64::try_from(i + 1).unwrap()), None) else {
                return true;
            };
            i = j;
        } else if pystr::char_at(text, i64::try_from(i).unwrap()) == Some(',') {
            return false;
        }
        i += 1;
    }
    false
}

/// Port of `checker/structure.py::_control_keyword`.
pub fn _control_keyword(stmt: &Stmt) -> Option<String> {
    match stmt {
        Stmt::If(_) => Some("IF".to_owned()),
        Stmt::For(_) => Some("FOR".to_owned()),
        Stmt::DoLoop(s) => Some(pystr::upper(&s.keyword)),
        Stmt::Instruction(s) => {
            let kw = pystr::upper(&s.keyword);
            if matches!(kw.as_str(), "SELECT" | "ENDIF")
                || kw == "NEXT"
                    && !matches!(
                        _block_word_with(&s.args).as_deref(),
                        Some("SCAN" | "SUBSCAN")
                    )
            {
                Some(kw)
            } else {
                None
            }
        }
        _ => None,
    }
}

pub const _FALLBACK_PREFIX: &str = "Invalid, or out of place expression:";

/// Port of `checker/structure.py::_table_names`.
pub fn _table_names(program: &Program) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    for stmt in &program.statements {
        if let Stmt::Instruction(s) = stmt.as_ref()
            && matches!(
                pystr::upper(&s.keyword).as_str(),
                "DATATABLE" | "CONSTTABLE"
            )
            && let Some(Some(arg)) = s.args.first()
            && let Expr::Name(n) = arg.as_ref()
        {
            names.insert(pystr::upper(&n.name));
        }
    }
    names
}

pub static _CONTROL_COND: LazyLock<pyre::Regex> = LazyLock::new(|| {
    pyre::Regex::new(
        r"\s*(?:IF|ELSEIF|DO\s+(?:WHILE|UNTIL)|LOOP\s+(?:WHILE|UNTIL))\b",
        pyre::I,
    )
});

/// Port of `checker/structure.py::_leftover_message`.
pub fn _leftover_message(_stmt: &Stmt) -> String {
    todo!("checker/structure.py::_leftover_message (omitted: profile: untyped)")
}

/// Port of `checker/structure.py::_leftover_message`.
pub fn _leftover_message_with(profile: &Profile, stmt: &Stmt) -> String {
    let text = pystr::strip(stmt.text());
    if !profile.single_pass
        && _CONTROL_COND.match_(stmt.text()).is_some()
        && let Some(cond) = super::part5::condition_part(stmt.text())
            .or_else(|| super::part5::inline_if_condition(stmt.text()))
    {
        return format!("{_FALLBACK_PREFIX} {cond}.");
    }
    if !(profile.fallback_echo_newline && text.starts_with('(')) {
        return _out_of_place_with(profile, stmt.text(), stmt.at_eol());
    }
    let mut depth = 0_i64;
    for (i, ch) in text.chars().enumerate() {
        depth += i64::from(ch == '(') - i64::from(ch == ')');
        if depth == 0 {
            let inner = pystr::slice(text, Some(1), Some(i64::try_from(i).unwrap()));
            if _constant_assignment_with(inner, profile) {
                return format!("Invalid assignment in expression: {inner}.");
            }
            return format!("{_FALLBACK_PREFIX} {inner}.");
        }
    }
    _out_of_place_with(profile, pystr::slice(text, Some(1), None), stmt.at_eol())
}

/// Port of `checker/structure.py::_constant_assignment`.
pub fn _constant_assignment(_text: &str) -> bool {
    todo!("checker/structure.py::_constant_assignment (omitted: profile: untyped)")
}

/// Port of `checker/structure.py::_constant_assignment`.
pub fn _constant_assignment_with(text: &str, profile: &Profile) -> bool {
    let Ok(toks) = lexer::tokenize(text, 0) else {
        return false;
    };
    let toks: Vec<_> = toks.into_iter().filter(|t| t.kind != "EOF").collect();
    if toks.len() < 3 || !(toks[1].kind == "OP" && toks[1].text == "=") {
        return false;
    }
    let first = &toks[0];
    matches!(first.kind.as_str(), "NUMBER" | "STRING")
        || first.kind == "IDENT"
            && !profile.instructions().contains(&first.upper)
            && (profile.symbols().contains(&first.upper)
                || crate::checker::expressions::part1::_EXTRA_PREDEFINED_SYMBOLS
                    .contains(&first.upper.as_str()))
}

/// Port of `checker/structure.py::_no_matching`.
pub fn _no_matching(_text: &str, _at_eol: bool) -> String {
    todo!("checker/structure.py::_no_matching (omitted: profile: untyped)")
}

/// Port of `checker/structure.py::_no_matching`.
pub fn _no_matching_with(profile: &Profile, text: &str, at_eol: bool) -> String {
    let sep = if profile.fallback_echo_newline && at_eol {
        "\n"
    } else {
        ""
    };
    format!("No Matching, or out of place ) in expression: {text}{sep}.")
}

/// Port of `checker/structure.py::_out_of_place`.
pub fn _out_of_place(_text: &str, _at_eol: bool) -> String {
    todo!("checker/structure.py::_out_of_place (omitted: profile: untyped)")
}

/// Port of `checker/structure.py::_out_of_place`.
pub fn _out_of_place_with(profile: &Profile, text: &str, at_eol: bool) -> String {
    let sep = if profile.fallback_echo_newline && at_eol {
        "\n"
    } else {
        ""
    };
    if profile.fallback_echo_newline
        && text.contains(';')
        && (_semicolon_assignment(text) || _function_statement_with(text, profile))
    {
        return format!("Invalid symbol in expression: {text}{sep}.");
    }
    format!("Invalid, or out of place expression: {text}{sep}.")
}

/// Port of `checker/structure.py::_function_statement`.
pub fn _function_statement(_text: &str) -> bool {
    todo!("checker/structure.py::_function_statement (omitted: profile: untyped)")
}

static FUNCTION_STATEMENT_RE: LazyLock<pyre::Regex> =
    LazyLock::new(|| pyre::Regex::new(r"\s*([A-Za-z_]\w*)\s*\(", 0));

/// Port of `checker/structure.py::_function_statement`.
pub fn _function_statement_with(text: &str, profile: &Profile) -> bool {
    FUNCTION_STATEMENT_RE
        .match_(text)
        .and_then(|m| m.group(1))
        .is_some_and(|name| {
            let name = pystr::upper(name);
            profile.functions().contains(&name) && !profile.instructions().contains(&name)
        })
}

/// Port of `checker/structure.py::_semicolon_assignment`.
pub fn _semicolon_assignment(text: &str) -> bool {
    let Ok(toks) = crate::parser::tokenize_for_parser(text, 0) else {
        return false;
    };
    let toks: Vec<_> = toks.into_iter().filter(|t| t.kind != "EOF").collect();
    if toks.is_empty() || toks[0].kind == "OP" && toks[0].text == ";" {
        return false;
    }
    let mut depth = 0_i64;
    for t in &toks {
        if t.kind == "OP" && "()".contains(&t.text) {
            depth += if t.text == "(" { 1 } else { -1 };
        } else if t.kind == "OP" && t.text == ";" {
            return false;
        } else if depth == 0
            && t.kind == "OP"
            && (t.text == "=" || crate::syntax::COMPOUND_ASSIGNMENT_OPS.contains(&t.text.as_str()))
        {
            break;
        }
    }
    toks.iter().any(|t| t.kind == "OP" && t.text == ";")
        && crate::parser::helpers::_is_assignment(
            &toks
                .into_iter()
                .filter(|t| !(t.kind == "OP" && t.text == ";"))
                .collect::<Vec<_>>(),
        )
}

/// Port of `checker/structure.py::_keyword`.
pub fn _keyword(stmt: &Stmt) -> String {
    match stmt {
        Stmt::Instruction(s) => pystr::upper(&s.keyword),
        Stmt::If(_) => "IF".to_owned(),
        Stmt::For(_) => "FOR".to_owned(),
        Stmt::DoLoop(s) => pystr::upper(&s.keyword),
        _ => pystr::upper(stmt.type_name()),
    }
}

pub const _END_EXIT_WORDS: [&str; 9] = [
    "IF", "SELECT", "SUB", "FUNCTION", "PROG", "TABLE", "SCAN", "DO", "FOR",
];

/// Port of `checker/structure.py::_block_word`.
pub fn _block_word() -> Option<String> {
    todo!("checker/structure.py::_block_word (omitted: args: list)")
}

/// Port of `checker/structure.py::_block_word`.
pub fn _block_word_with(args: &[Option<ExprRef>]) -> Option<String> {
    match args.first()?.as_deref()? {
        Expr::Name(n) => Some(pystr::upper(&n.name)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{parser, profiles};

    #[test]
    fn placement_walk_matches_python_stops_and_diagnostic_order() {
        let cases: &[(&str, i64, &[(i64, &str)])] = &[
            (
                "DataTable(T,1,-1)\nDataInterval(0,1,Sec,10)\nEndTable\nBeginProg\nCallTable(T)\nNextScan\nElse\nEndProg",
                6,
                &[
                    (
                        5,
                        "Table with DataInterval must be called within Scan..NextScan.",
                    ),
                    (
                        5,
                        "Cannot autoallocate memory for DataTable T called outside of Scan, e.g., called by Sub.",
                    ),
                    (6, "NextScan with no Scan."),
                ],
            ),
            (
                "BeginProg\nIf 1 Then Exit Do Else Exit For\nElse\nNextScan\nEndProg",
                3,
                &[
                    (2, "Exit Do without DO."),
                    (2, "Exit For without FOR."),
                    (3, "Else with no If."),
                ],
            ),
            (
                "BeginProg\nSub A\nElse\nEndSub\nEndProg",
                2,
                &[(2, "Sub is declared in the middle of an executing sequence")],
            ),
            (
                "WebPageBegin\nX=1\nWebPageEnd\nX=2\nBeginProg\nEndProg",
                4,
                &[(4, "Expr illegally occurs before BeginProg.")],
            ),
            (
                "BeginProg\nScan(0,Sec,0,0)\nScan(0,Sec,0,0)\nSlowSequence\nEndProg",
                4,
                &[
                    (3, "Scan without NextScan or nested Scans."),
                    (4, "Missing NextScan."),
                ],
            ),
        ];
        for name in ["cr6", "cr1000x", "cr300", "cr350"] {
            let profile = profiles::get(name).unwrap();
            for (source, line, expected) in cases {
                let (program, _) = parser::parse_text(source, "t.cr6", None, Some(profile));
                let mut diags = Diagnostics::new();
                let stop = _scan_placement_walk_with(&program, profile, &mut diags);
                if profile.single_pass {
                    assert_eq!(stop, Some(("t.cr6".to_owned(), *line)), "{name}: {source}");
                    let actual: Vec<_> = diags
                        .items
                        .iter()
                        .map(|d| (d.line, d.message.as_str()))
                        .collect();
                    assert_eq!(actual, *expected, "{name}: {source}");
                    for diag in &diags.items {
                        if diag.message == NESTED_SCAN {
                            assert_eq!(diag.kind, "nested_scan");
                        }
                    }
                } else {
                    assert_eq!(stop, None);
                    assert_eq!(diags.items.len(), 0);
                }
            }
        }
    }

    fn located(
        program: &Program,
        found: &OrderedMap<usize, (String, bool)>,
    ) -> Vec<(i64, String, bool)> {
        let mut flat = Vec::new();
        for stmt in &program.statements {
            flat.push(stmt.clone());
            flat.extend(stmt.inline_bodies());
        }
        found
            .iter()
            .map(|(id, (message, stops))| {
                let stmt = flat.iter().find(|s| stmt_id(s) == *id).unwrap();
                (stmt.line(), message.clone(), *stops)
            })
            .collect()
    }

    #[test]
    fn stray_blocks_keep_identity_and_inline_order() {
        let source = "Sub A\nIf 1 Then\nEndSub\nSub B\nEndSub\nEndIf\nBeginProg\nIf 1 Then Exit Do Else Exit For\nExit Sub\nEndProg";
        for name in ["cr6", "cr1000x", "cr300", "cr350"] {
            let (p, _) =
                parser::parse_text(source, "t.cr6", None, Some(profiles::get(name).unwrap()));
            let actual = located(&p, &stray_controls_with(&p));
            let expected: Vec<_> = [
                (3, "If without EndIf inside of Sub."),
                (4, "Sub declaration inside If..EndIf."),
                (5, "If without EndIf inside of Sub."),
                (8, "Exit Do without DO."),
                (8, "Exit For without FOR."),
                (9, "Exit Sub without Sub or nested subs."),
            ]
            .into_iter()
            .map(|(line, m)| (line, m.to_owned(), false))
            .collect();
            assert_eq!(actual, expected, "{name}");
        }
    }

    #[test]
    fn case_else_prefix_and_mismatched_closer() {
        let source = "BeginProg\nSelect Case X\nCase Elsewhere\nElse\nEndIf\nCase 1\nElse\nEndProg";
        for name in ["cr6", "cr1000x", "cr300", "cr350"] {
            let (p, _) =
                parser::parse_text(source, "t.cr6", None, Some(profiles::get(name).unwrap()));
            assert_eq!(
                located(&p, &stray_controls_with(&p)),
                vec![
                    (4, "If statement with duplicate Else.".to_owned(), false),
                    (6, "Case with no Select.".to_owned(), true),
                    (7, "Else with no If.".to_owned(), true),
                ],
                "{name}"
            );
        }
    }

    #[test]
    fn nested_zero_time_subscans_are_profile_specific() {
        let source = "BeginProg\nScan(1,Sec,0,0)\nSubScan(1,mSec,1)\nSubScan(1,mSec,1)\nDelay(1,1,mSec)\nNextSubScan\nNextSubScan\nNextScan\nEndProg";
        for name in ["cr6", "cr1000x", "cr300", "cr350"] {
            let profile = profiles::get(name).unwrap();
            let (p, _) = parser::parse_text(source, "t.cr6", None, Some(profile));
            let mut expected = vec![(4, "Cannot Nest SubScans.".to_owned(), false)];
            if !profile.single_pass {
                expected.push((6, "No Measurements in SubScan.".to_owned(), false));
                expected.push((7, "No Measurements in SubScan.".to_owned(), false));
            }
            assert_eq!(
                located(&p, &subscan_controls_with(&p, profile)),
                expected,
                "{name}"
            );
        }
    }

    #[test]
    fn scan_only_diagnostics_stop_in_statement_order() {
        let source = "BeginProg\nIf 1 Then PulseCount(X,1,1,0,0,1,0)\nExitScan\nSubScan(1,mSec,1)\nContinueScan\nEndProg";
        for name in ["cr6", "cr1000x", "cr300", "cr350"] {
            let profile = profiles::get(name).unwrap();
            let (p, _) = parser::parse_text(source, "t.cr6", None, Some(profile));
            assert_eq!(
                located(&p, &subscan_controls_with(&p, profile)),
                vec![(
                    4,
                    "SubScan used outside of Scan .. NextScan.".to_owned(),
                    true
                ),]
            );
            let scans = scan_only_outside(&p);
            assert_eq!(scans.len(), 1);
            assert_eq!(
                scans.values().next().unwrap(),
                "PulseCount must be used within a Scan...NextScan."
            );
            let mut diags = Diagnostics::new();
            let stop = check_stray_controls_with(&p, profile, &mut diags);
            if profile.single_pass {
                assert_eq!(stop, None);
                assert_eq!(diags.items.len(), 0);
            } else {
                assert_eq!(stop, Some(("t.cr6".to_owned(), 2)));
                assert_eq!(diags.items.len(), 1);
                assert_eq!(diags.items[0].kind, "placement_scan");
            }
        }
    }

    #[test]
    fn semicolons_quotes_and_latin1_indices() {
        for (text, expected) in [
            ("X=1;", true),
            ("X;=1", false),
            (";X=1", false),
            ("X(1;)=2", false),
            ("X(1)=2;", true),
            ("X += 1;", true),
        ] {
            assert_eq!(_semicolon_assignment(text), expected, "{text}");
        }
        for (text, expected) in [
            ("Word(\"a,\"b\")", true),
            ("Word(\"a", true),
            ("Word(é,\"x", false),
            ("Word(é,\"x\",z)", false),
            ("Word(é,z,\"x", false),
        ] {
            assert_eq!(_quote_in_first_arg(text), expected, "{text}");
        }
    }

    #[test]
    fn fallback_echo_and_predefined_assignment() {
        for name in ["cr6", "cr300"] {
            let p = profiles::get(name).unwrap();
            for (text, expected) in [
                ("Double = Z", true),
                ("Sec = 1", true),
                ("NAN = 1", true),
                ("Battery = 1", false),
            ] {
                assert_eq!(
                    _constant_assignment_with(text, p),
                    expected,
                    "{name}: {text}"
                );
            }
            assert_eq!(
                _out_of_place_with(p, "X=1;", true),
                if name == "cr6" {
                    "Invalid, or out of place expression: X=1;."
                } else {
                    "Invalid symbol in expression: X=1;\n."
                }
            );
            assert_eq!(
                _no_matching_with(p, "X=1;", true),
                if name == "cr6" {
                    "No Matching, or out of place ) in expression: X=1;."
                } else {
                    "No Matching, or out of place ) in expression: X=1;\n."
                }
            );
            assert_eq!(
                _out_of_place_with(p, "Timer(1 ;,Sec,0)", true),
                if name == "cr6" {
                    "Invalid, or out of place expression: Timer(1 ;,Sec,0)."
                } else {
                    "Invalid symbol in expression: Timer(1 ;,Sec,0)\n."
                }
            );
        }
    }
}
