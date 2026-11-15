//! Port of `checker/expressions.py` (lines 2353-3083).

#![allow(dead_code, unused_variables, unused_imports)]

use std::collections::BTreeSet;
use std::rc::Rc;

use crate::checker::context;
use crate::checker::{cr3xx, intervals, structs, structure};
use crate::diagnostics::{DiagRef, Diagnostic, Diagnostics, ShowFile};
use crate::profiles::Profile;
use crate::pystr;
use crate::syntax::{
    Assignment, Call, Case, ConstDecl, Declaration, Expr, ExprRef, Index, Instruction, Juxtaposed,
    Member, Number, OrderedMap, Program, ProgramRef, Stmt, StmtRef, SubDef, Unparsed, VarDecl,
    stmt_id,
};

const _DATA_KEYWORDS: &[&str] = &["DATA", "DATALONG"];
use super::part1::{_EXTRA_PREDEFINED_SYMBOLS, _WHOLE_EXPR_ECHO_LOGGERS};
use super::part2::_CONTROL_KEYWORDS;
const _ARITY_TRAILING_NEWLINE_LOGGERS: &[&str] = &["cr300", "cr350"];
const _WRONG_TYPE: &str = "Structured Variable assignment illegal, wrong type";

fn arity_min_safe(name: &str) -> Option<i64> {
    match name {
        "FILECOPY" | "FILELIST" | "FILERENAME" | "FORMATFLOAT" | "FORMATLONG" | "IPINFO"
        | "LEFT" | "RIGHT" | "ROUND" | "SECSSINCE1990" | "STRCOMP" | "WATCHDOGTIMER" | "AVE"
        | "PWR" | "ATN2" => Some(2),
        "CHECKSUM" | "FILEOPEN" | "FILEREAD" | "FILEREADLINE" | "FILEWRITE" | "HTTPGET" | "IIF"
        | "MID" | "REPLACE" | "SERIALINBLOCK" | "SMSRECV" | "TCPOPEN" | "UDPOPEN" | "IFTIME"
        | "TIMEINTOINTERVAL" => Some(3),
        "DIALMODEM" | "FINDSPA" | "INSTR" | "MQTTPUBLISH" | "MQTTSUBSCRIBE" | "TIMEISBETWEEN" => {
            Some(4)
        }
        "SERIALOPEN" | "SERIALOUT" => Some(5),
        "EMAILRECV" | "EMAILSEND" | "FTPCLIENT" | "UDPDATAGRAM" | "XMLPARSE" => Some(6),
        "ENCRYPTION" => Some(7),
        "DAYLIGHTSAVING" => Some(8),
        _ => None,
    }
}

fn arity_max_safe(name: &str) -> Option<i64> {
    match name {
        "NEWFIELDCAL" | "PPPCLOSE" | "RND" | "CLOCKCHANGE" => Some(0),
        "ARRAYINDEX" | "ASCII" | "CHECKPORT" | "CHR" | "CHR$" | "COMPORTISACTIVE"
        | "DAYLIGHTSAVINGUS" | "FILECLOSE" | "FILEENCRYPT" | "FILESIZE" | "FILETIME"
        | "FORMATLONGLONG" | "HEX" | "HEXTODEC" | "IPTRACE" | "LEN" | "LOADFIELDCAL"
        | "LOWERCASE" | "LTRIM" | "MQTTUNSUBSCRIBE" | "ROUTE" | "RTRIM" | "SERIALCLOSE"
        | "SERIALFLUSH" | "SERIALINCHK" | "TCPCLOSE" | "TRIM" | "UPPERCASE" | "VOICEKEY"
        | "VOICENUMBER" | "SIN" | "COS" | "LOG" | "LN" | "SQR" | "EXP" | "CSGN" | "INT"
        | "FRAC" | "SGN" | "ATN" | "TAN" | "FIX" | "ASIN" | "ACOS" | "LOG10" | "SINH" | "TANH"
        | "COSH" | "FLOOR" | "CEILING" => Some(1),
        "FILECOPY" | "FILELIST" | "FILERENAME" | "FORMATFLOAT" | "FORMATLONG" | "IPINFO"
        | "LEFT" | "RIGHT" | "ROUND" | "SECSSINCE1990" | "STRCOMP" | "WATCHDOGTIMER" | "PWR"
        | "ATN2" => Some(2),
        "FILEOPEN" | "FILEREAD" | "FILEREADLINE" | "FILEWRITE" | "MID" | "PINGIP" | "REPLACE"
        | "SERIALINBLOCK" | "SMSRECV" | "IIF" | "IFTIME" | "TIMEINTOINTERVAL" => Some(3),
        "FINDSPA" | "INSTR" | "TIMEISBETWEEN" => Some(4),
        "SERIALOUT" => Some(5),
        "ENCRYPTION" => Some(7),
        "DAYLIGHTSAVING" => Some(8),
        _ => None,
    }
}

fn function_const_integer_string_args(name: &str) -> &'static [usize] {
    match name {
        "TIMER" => &[1],
        _ => &[],
    }
}

fn const_integer_overflow_string(name: &str) -> Option<i64> {
    match name {
        "TCPOPEN" => Some(3),
        _ => None,
    }
}

fn const_integer_bad_args(name: &str) -> &'static [usize] {
    match name {
        "SERIALOPEN" => &[3, 4],
        _ => &[],
    }
}

fn network_integer_arg(name: &str) -> Option<usize> {
    match name {
        "HTTPGET" => Some(3),
        "TCPOPEN" => Some(2),
        _ => None,
    }
}

fn const_integer_type_args(name: &str) -> &'static [usize] {
    match name {
        "HTTPPUT" => &[8],
        "FTPCLIENT" => &[10],
        _ => &[],
    }
}

fn const_integer_type_args_always(name: &str) -> &'static [usize] {
    match name {
        "TIMER" => &[2],
        _ => &[],
    }
}

fn incomplete_overflow(name: &str) -> Option<i64> {
    match name {
        "ABS" => Some(1),
        "MQTTPUBLISH" => Some(4),
        _ => None,
    }
}

fn arity_range(name: &str, nargs: i64) -> Option<(usize, usize)> {
    match (name, nargs) {
        ("NETWORKTIMEPROTOCOL", 5) => Some((3, 4)),
        ("NETWORKTIMEPROTOCOL" | "TIMER", 3) => Some((3, 3)),
        ("GZIP", 1) => Some((2, 2)),
        ("SERIALOPEN", 5) => Some((5, 6)),
        ("SERIALOPEN", 6) => Some((5, 8)),
        _ => None,
    }
}

fn arity_allowed(name: &str, nargs: i64) -> Option<&'static [usize]> {
    match (name, nargs) {
        ("HTTPPUT" | "HTTPPOST", 5) => Some(&[4, 8, 9]),
        ("FTPCLIENT", 6) => Some(&[6, 10, 11]),
        ("EMAILSEND", 6) => Some(&[9, 13, 14]),
        ("EMAILRECV", 6) => Some(&[9, 10, 11, 12, 13]),
        ("UDPDATAGRAM", 6) => Some(&[6, 7, 8]),
        ("XMLPARSE", 6) => Some(&[8]),
        ("HTTPGET" | "UDPOPEN", 3) => Some(&[3, 4]),
        ("TCPOPEN" | "CHECKSUM", 3) => Some(&[3, 4, 6]),
        ("EMAILRELAY", 0) => Some(&[4, 5, 9, 10]),
        _ => None,
    }
}

/// Port of checker/expressions.py::check_data_params
pub fn check_data_params(program: &ProgramRef, profile: &'static Profile, diags: &mut Diagnostics) {
    let ctx = context::of(program, Some(profile), None);
    let known = &ctx.known_names().names;
    for stmt in &program.statements {
        match &**stmt {
            Stmt::Instruction(inst)
                if _DATA_KEYWORDS.contains(&pystr::upper(&inst.keyword).as_str()) =>
            {
                _check_data_instruction(inst, known, profile, diags);
            }
            Stmt::Unparsed(unparsed) => {
                _check_unparsed_data(unparsed, diags, profile, program);
            }
            _ => {}
        }
    }
}

/// Port of checker/expressions.py::_data_param_text
pub fn _data_param_text(stmt: &Instruction, i: i64) -> String {
    let mut text = super::part2::_arg_text(stmt, i);
    let last = i64::try_from(stmt.args.len()).unwrap_or(0) - 1;
    if i == last {
        text.push('\n');
    }
    text
}

/// Port of checker/expressions.py::_check_data_instruction
pub fn _check_data_instruction(
    stmt: &Instruction,
    known: &BTreeSet<String>,
    profile: &Profile,
    diags: &mut Diagnostics,
) {
    for (i, arg) in stmt.args.iter().enumerate() {
        let Some(arg) = arg else {
            continue;
        };
        let undefined = super::part1::_undefined_names(
            Some(arg),
            &crate::checker::expressions::KnownView::from_set(known),
        )
        .into_iter()
        .map(crate::checker::expressions::UndefinedName::into_string)
        .collect::<Vec<_>>();
        let resolved_refs = super::part1::_variable_refs(Some(arg), known);
        if undefined.is_empty() && resolved_refs.is_empty() {
            continue;
        }
        let text = _data_param_text(stmt, i64::try_from(i).unwrap_or(0));
        if !undefined.is_empty() {
            if _WHOLE_EXPR_ECHO_LOGGERS.contains(&profile.name) {
                diags.error_basic(
                    stmt.base.node.line,
                    &profile.unresolved_symbol_message.format(&text),
                    &stmt.base.node.file,
                );
            } else {
                for name in undefined {
                    diags.error_basic(
                        stmt.base.node.line,
                        &profile.unresolved_symbol_message.format(&name),
                        &stmt.base.node.file,
                    );
                }
            }
        }
        diags.error_basic(
            stmt.base.node.line,
            &format!("Illegal Data parameter {text}."),
            &stmt.base.node.file,
        );
        return;
    }
}

/// Port of checker/expressions.py::_check_unparsed_data
pub fn _check_unparsed_data(
    stmt: &Unparsed,
    diags: &mut Diagnostics,
    profile: &'static Profile,
    program: &ProgramRef,
) {
    if !stmt
        .reason
        .starts_with("unexpected trailing input starting at")
    {
        return;
    }
    let stripped = pystr::lstrip(&stmt.base.text);
    let upper = pystr::upper(stripped);
    for kw in ["DATALONG", "DATA"] {
        let kw_len = pystr::len(kw);
        let kw_len_i64 = i64::try_from(kw_len).unwrap_or(0);
        if upper.starts_with(kw)
            && pystr::len(stripped) > kw_len
            && pystr::char_at(stripped, kw_len_i64).is_some_and(pystr::is_space_char)
        {
            let tail = pystr::lstrip(pystr::slice(stripped, Some(kw_len_i64), None));
            let text = format!("{tail}\n");
            if _WHOLE_EXPR_ECHO_LOGGERS.contains(&profile.name) {
                diags.error_basic(
                    stmt.base.node.line,
                    &format!("Invalid symbol in expression: {text}."),
                    &stmt.base.node.file,
                );
            } else {
                let known = cr3xx::part2::prose_known_with(program, profile);
                for name in cr3xx::part2::_leading_unknown_words(&text, &known) {
                    diags.error_basic(
                        stmt.base.node.line,
                        &profile.unresolved_symbol_message.format(&name),
                        &stmt.base.node.file,
                    );
                }
                diags.error_basic(
                    stmt.base.node.line,
                    &format!("Incomplete expression: {text}."),
                    &stmt.base.node.file,
                );
            }
            diags.error_basic(
                stmt.base.node.line,
                &format!("Illegal Data parameter {text}."),
                &stmt.base.node.file,
            );
            return;
        }
    }
}

/// Port of checker/expressions.py::_call_arity_problem
pub fn _call_arity_problem(
    name: &str,
    args: &[Option<ExprRef>],
    profile: &Profile,
) -> Option<String> {
    let upper = pystr::upper(name);
    let record = profile.functions().get(&upper)?;
    let n = args.len();
    if upper == "CTYPE" {
        return if args.iter().any(Option::is_none) || n == 2 {
            None
        } else if n == 1 {
            Some("few".to_owned())
        } else if n > 2 {
            Some("many".to_owned())
        } else {
            None
        };
    }
    let nargs = record.nargs?;
    if let Some(option) = network_integer_arg(&upper)
        && option < n
        && super::part1::_TIMER_CONTEXT.with(|slot| slot.borrow().is_some())
        && let Some(problem) =
            super::_network_integer_problem(&upper, args[option].as_deref(), profile)
    {
        return Some(problem);
    }
    if function_const_integer_string_args(&upper)
        .iter()
        .any(|&i| i < args.len() && matches!(args[i].as_deref(), Some(Expr::String(_))))
    {
        return Some("constint".to_owned());
    }
    let integer_args: &[usize] = if upper == "SERIALOPEN" && n == 6 && !profile.single_pass {
        &[3]
    } else {
        const_integer_bad_args(&upper)
    };
    if integer_args
        .iter()
        .any(|&i| i < n && super::_not_constant_integer(args[i].as_deref()))
    {
        return Some("constint".to_owned());
    }
    if super::part1::_TIMEOUT_GATE.with(|slot| *slot.borrow() == Some(false))
        && const_integer_type_args(&upper).iter().any(|&i| {
            i < n
                && (super::_not_integer_type(args[i].as_deref())
                    || super::_timeout_wide(args[i].as_deref(), profile))
        })
    {
        return Some("constint".to_owned());
    }
    if const_integer_type_args_always(&upper)
        .iter()
        .any(|&i| i < n && super::_not_integer_type(args[i].as_deref()))
    {
        return Some("constint".to_owned());
    }
    if upper == "TIMER" && n == 3 {
        let problem = super::part1::_TIMER_CONTEXT.with(|slot| {
            let slot = slot.borrow();
            let ctx = slot.as_ref()?;
            let kinds: Vec<_> = args[1..]
                .iter()
                .map(|a| super::_timer_arg(a.as_deref(), &ctx.env, &ctx.declared))
                .collect();
            if kinds.contains(&Some(super::TimerArg::Float)) {
                return Some("constint".to_owned());
            }
            if let Some(super::TimerArg::Int(value)) = kinds[1]
                && !(0..=4).contains(&value)
            {
                return Some("timer_option".to_owned());
            }
            None
        });
        if problem.is_some() {
            return problem;
        }
    }
    if const_integer_overflow_string(&upper) == Some(nargs)
        && usize::try_from(nargs + 1).ok() == Some(n)
        && matches!(
            args.last().and_then(|a| a.as_deref()),
            Some(Expr::String(_))
        )
    {
        return Some("constint".to_owned());
    }
    if incomplete_overflow(&upper) == Some(nargs) && usize::try_from(nargs + 1).ok() == Some(n) {
        return Some("incomplete".to_owned());
    }
    if n == 0 && (nargs > 0 || record.argspec != 0) {
        return Some("empty".to_owned());
    }
    if let Some(allowed) = arity_allowed(&upper, nargs) {
        if allowed.contains(&n) {
            return None;
        }
        let max_allowed = allowed.iter().copied().max().unwrap_or(0);
        return Some(if n < max_allowed {
            "few".to_owned()
        } else {
            "many".to_owned()
        });
    }
    if let Some((lo, hi)) = arity_range(&upper, nargs) {
        return if n < lo {
            Some("few".to_owned())
        } else if n > hi {
            Some("many".to_owned())
        } else {
            None
        };
    }
    let n_i64 = i64::try_from(n).unwrap_or(i64::MAX);
    if n_i64 < nargs && arity_min_safe(&upper) == Some(nargs) {
        return Some("few".to_owned());
    }
    if n_i64 > nargs && arity_max_safe(&upper) == Some(nargs) {
        return Some("many".to_owned());
    }
    None
}

/// Port of checker/expressions.py::_find_arity_problem
pub fn _find_arity_problem(
    expr: Option<&Expr>,
    profile: &Profile,
    known: Option<&BTreeSet<String>>,
    constants: &BTreeSet<String>,
) -> Option<String> {
    let expr = expr?;
    if let Some(known_set) = known {
        match expr {
            Expr::Name(n) if !known_set.contains(&pystr::upper(&n.name)) => {
                return Some("deferred".to_owned());
            }
            Expr::Index(ix) if !known_set.contains(&pystr::upper(&ix.name)) => {
                return Some("deferred".to_owned());
            }
            _ => {}
        }
    }
    match expr {
        Expr::Index(ix) => {
            if constants.contains(&pystr::upper(&ix.name)) {
                return Some("indexed_const".to_owned());
            }
            let direct = _call_arity_problem(&ix.name, &ix.args, profile);
            if let Some(ref d) = direct
                && let Some(known_set) = known
            {
                let mut read: &[Option<ExprRef>] = &ix.args;
                while d != "few"
                    && !read.is_empty()
                    && !matches!(
                        _call_arity_problem(&ix.name, read, profile).as_deref(),
                        None | Some("few")
                    )
                {
                    read = &read[..read.len() - 1];
                }
                if read.iter().any(|a| {
                    !super::part1::_undefined_names(
                        a.as_deref(),
                        &crate::checker::expressions::KnownView::from_set(known_set),
                    )
                    .is_empty()
                }) {
                    return Some("deferred".to_owned());
                }
            }
            if direct.is_some() {
                return direct;
            }
            for a in &ix.args {
                if let Some(found) = _find_arity_problem(a.as_deref(), profile, known, constants) {
                    return Some(found);
                }
            }
            None
        }
        Expr::Unary(u) => _find_arity_problem(u.operand.as_deref(), profile, known, constants),
        Expr::Binary(b) => _find_arity_problem(b.left.as_deref(), profile, known, constants)
            .or_else(|| _find_arity_problem(b.right.as_deref(), profile, known, constants)),
        Expr::Member(m) => {
            if m.args.as_ref().is_some_and(|args| args.len() > 2) {
                return Some("many_member".to_owned());
            }
            _find_arity_problem(m.base.as_deref(), profile, known, constants)
        }
        Expr::ArrayLiteral(a) => {
            for item in &a.items {
                if let Some(found) = _find_arity_problem(item.as_deref(), profile, known, constants)
                {
                    return Some(found);
                }
            }
            None
        }
        Expr::Juxtaposed(j) => {
            for item in &j.items {
                if let Some(found) = _find_arity_problem(item.as_deref(), profile, known, constants)
                {
                    return Some(found);
                }
            }
            None
        }
        Expr::Number(_) | Expr::Name(_) | Expr::String(_) => None,
    }
}

/// Port of checker/expressions.py::_arity_message
pub fn _arity_message(
    kind: &str,
    text: &str,
    profile: &Profile,
    at_eol: bool,
    whole_statement: bool,
) -> String {
    if kind == "timer_option" {
        return "Legal options are 0..4".to_owned();
    }
    if kind.ends_with(" cannot be negative") {
        return kind.to_owned();
    }
    if matches!(
        kind,
        "constint" | "network_constint" | "incomplete" | "indexed_const"
    ) {
        let verb = if kind == "constint" || kind == "network_constint" {
            "Parameter must be a constant integer: "
        } else {
            "Incomplete expression: "
        };
        let sep = if _ARITY_TRAILING_NEWLINE_LOGGERS.contains(&profile.name) && at_eol {
            "\n"
        } else {
            ""
        };
        return format!("{verb}{text}{sep}.");
    }
    if kind == "empty" {
        let sep = if whole_statement && at_eol && profile.fallback_echo_newline {
            "\n"
        } else {
            ""
        };
        return format!("Invalid, or out of place expression: {text}{sep}.");
    }
    let verb = if kind == "few" {
        "Expression expects , : "
    } else {
        "No Matching, or out of place ) in expression: "
    };
    if _ARITY_TRAILING_NEWLINE_LOGGERS.contains(&profile.name) && at_eol {
        return format!("{verb}{text}\n.");
    }
    format!("{verb}{text}.")
}

/// Port of checker/expressions.py::_is_bare_reference_problem
pub fn _is_bare_reference_problem(
    name: &str,
    profile: &Profile,
    declared_variables: &BTreeSet<String>,
) -> bool {
    let upper = pystr::upper(name);
    if declared_variables.contains(&upper) {
        return false;
    }
    let Some(record) = profile.functions().get(&upper) else {
        return false;
    };
    let Some(nargs) = record.nargs else {
        return false;
    };
    nargs > 0 || record.argspec != 0
}

/// Port of checker/expressions.py::_find_bare_function_ref
pub fn _find_bare_function_ref(
    expr: Option<&Expr>,
    profile: &Profile,
    declared_variables: &BTreeSet<String>,
) -> bool {
    let Some(expr) = expr else {
        return false;
    };
    match expr {
        Expr::Name(n) => _is_bare_reference_problem(&n.name, profile, declared_variables),
        Expr::Index(ix) => ix
            .args
            .iter()
            .any(|a| _find_bare_function_ref(a.as_deref(), profile, declared_variables)),
        Expr::Unary(u) => {
            _find_bare_function_ref(u.operand.as_deref(), profile, declared_variables)
        }
        Expr::Binary(b) => {
            _find_bare_function_ref(b.left.as_deref(), profile, declared_variables)
                || _find_bare_function_ref(b.right.as_deref(), profile, declared_variables)
        }
        Expr::Member(m) => _find_bare_function_ref(m.base.as_deref(), profile, declared_variables),
        Expr::ArrayLiteral(a) => a
            .items
            .iter()
            .any(|item| _find_bare_function_ref(item.as_deref(), profile, declared_variables)),
        Expr::Juxtaposed(j) => j
            .items
            .iter()
            .any(|item| _find_bare_function_ref(item.as_deref(), profile, declared_variables)),
        Expr::Number(_) | Expr::String(_) => false,
    }
}

/// Port of checker/expressions.py::_bare_function_message
pub fn _bare_function_message(
    text: &str,
    profile: &Profile,
    whole_statement: bool,
    at_eol: bool,
) -> String {
    if whole_statement && at_eol && _ARITY_TRAILING_NEWLINE_LOGGERS.contains(&profile.name) {
        return format!("Expression expects ( : {text}\n.");
    }
    format!("Expression expects ( : {text}.")
}

/// Port of checker/expressions.py::_struct_var_names
pub fn _struct_var_names(program: &Program) -> BTreeSet<String> {
    let types = structs::structure_members(program);
    if types.is_empty() {
        return BTreeSet::new();
    }
    structs::variable_kinds(program, &types)
        .iter()
        .filter(|&(_, k)| k.starts_with("STRUCT:"))
        .map(|(n, _)| n.clone())
        .collect()
}

/// Port of checker/expressions.py::_multi_indexed_struct
pub fn _multi_indexed_struct(expr: Option<&Expr>, struct_vars: &BTreeSet<String>) -> bool {
    let Some(expr) = expr else {
        return false;
    };
    match expr {
        Expr::Index(ix) => {
            (struct_vars.contains(&pystr::upper(&ix.name)) && ix.args.len() >= 2)
                || ix
                    .args
                    .iter()
                    .any(|a| _multi_indexed_struct(a.as_deref(), struct_vars))
        }
        Expr::Unary(u) => _multi_indexed_struct(u.operand.as_deref(), struct_vars),
        Expr::Binary(b) => {
            _multi_indexed_struct(b.left.as_deref(), struct_vars)
                || _multi_indexed_struct(b.right.as_deref(), struct_vars)
        }
        Expr::Member(m) => {
            _multi_indexed_struct(m.base.as_deref(), struct_vars)
                || m.args.as_ref().is_some_and(|args| {
                    args.iter()
                        .any(|a| _multi_indexed_struct(a.as_deref(), struct_vars))
                })
        }
        _ => false,
    }
}

/// Port of checker/expressions.py::_hold
pub fn _hold(held: Option<&mut Vec<DiagRef>>, diags: &mut Diagnostics, problem: &str) {
    if problem == "many_member"
        && let Some(last) = diags.items.last_mut()
    {
        let updated = Rc::new(last.replace(|d| "held".clone_into(&mut d.phase)));
        *last = updated.clone();
        if let Some(held) = held {
            held.push(updated);
        }
    }
}

/// Port of checker/expressions.py::check_function_arity
pub fn check_function_arity(
    program: &ProgramRef,
    profile: &'static Profile,
    diags: &mut Diagnostics,
    mut held: Option<&mut Vec<DiagRef>>,
) {
    let ctx = context::of(program, Some(profile), None);
    let mut declared_variables: BTreeSet<String> = ctx.declared_variable_names().clone();
    if profile.single_pass {
        declared_variables.retain(|name| !profile.functions().contains(name));
    }
    let mut known: Option<BTreeSet<String>> = None;
    let mut const_unready_loaded = false;
    let struct_types = structs::structure_members(program);
    let kinds = if struct_types.is_empty() {
        OrderedMap::new()
    } else {
        structs::variable_kinds(program, &struct_types)
    };
    let struct_vars = _struct_var_names(program);
    let sub_names: BTreeSet<String> = program
        .statements
        .iter()
        .filter_map(|st| match &**st {
            Stmt::SubDef(sd) if pystr::upper(&sd.keyword) == "SUB" => Some(pystr::upper(&sd.name)),
            _ => None,
        })
        .collect();
    let start = diags.items.len();
    let mut mine: Vec<DiagRef> = Vec::new();
    let mut table_mine: Vec<DiagRef> = Vec::new();
    let mut in_table = false;
    let mut seen_timeint = false;
    let _timeout_state = TimeoutStateGuard;
    super::part1::_TIMEOUT_WIDE
        .with(|slot| *slot.borrow_mut() = intervals::wide_constants_with(program, profile));
    super::set_arity_context(program, Some(profile), Some(declared_variables.clone()));
    let arity_known = if _WHOLE_EXPR_ECHO_LOGGERS.contains(&profile.name) {
        Some(ctx.known_names().names.clone())
    } else {
        None
    };
    let scopes = ctx.statement_scopes();
    let mut predefined: BTreeSet<String> = profile
        .symbols()
        .iter()
        .map(|(k, _)| k.to_owned())
        .chain(_EXTRA_PREDEFINED_SYMBOLS.iter().map(|s| (*s).to_owned()))
        .collect();
    for &name in profile.declarable_predefined_names {
        if declared_variables.contains(name) {
            predefined.remove(name);
        }
    }
    predefined.retain(|name| {
        !profile.functions().contains(name) && !_CONTROL_KEYWORDS.contains(&name.as_str())
    });
    let mut user_constants: BTreeSet<String> = BTreeSet::new();
    for name in program.consts.keys() {
        if !declared_variables.contains(name) && !profile.functions().contains(name) {
            user_constants.insert(name.clone());
        }
    }
    let all_constants: BTreeSet<String> = predefined.union(&user_constants).cloned().collect();

    let arity = |expr: Option<&Expr>, constants: &BTreeSet<String>| -> Option<String> {
        let problem = _find_arity_problem(expr, profile, arity_known.as_ref(), constants);
        if problem.as_deref() == Some("deferred") {
            None
        } else {
            problem
        }
    };

    let empty_scope = (BTreeSet::new(), BTreeSet::new());
    let empty_set = BTreeSet::new();
    for stmt in &program.statements {
        let (params, pending) = scopes.get(&stmt_id(stmt)).unwrap_or(&empty_scope);
        let mut hidden_constants: BTreeSet<String> = params.clone();
        if profile.single_pass {
            hidden_constants.extend(pending.iter().cloned());
        }
        let constants: BTreeSet<String> = if hidden_constants.is_empty() {
            all_constants.clone()
        } else {
            all_constants
                .iter()
                .filter(|c| !hidden_constants.contains(*c) || predefined.contains(*c))
                .cloned()
                .collect()
        };
        let (gate, next_seen) =
            intervals::timeout_gate_with(seen_timeint, intervals::gate_text_with(stmt).as_deref());
        seen_timeint = next_seen;
        super::part1::_TIMEOUT_GATE.with(|slot| *slot.borrow_mut() = gate);

        match &**stmt {
            Stmt::Instruction(inst) => {
                let kw = pystr::upper(&inst.keyword);
                in_table = kw == "DATATABLE" || (in_table && kw != "ENDTABLE");
                let table_only = crate::checker::instr_data::TABLE_ONLY_INSTRUCTIONS
                    .contains_key(kw.as_str())
                    && !in_table;
                let params_count = profile
                    .instructions()
                    .get(&kw)
                    .map_or(0, |r| r.params.len());
                for (i, arg) in inst.args.iter().enumerate() {
                    let Some(arg) = arg else {
                        continue;
                    };
                    let i_i64 = i64::try_from(i).unwrap_or(0);
                    if !table_only && _multi_indexed_struct(Some(arg), &struct_vars) {
                        diags.error_basic(
                            inst.base.node.line,
                            &_arity_message(
                                "many",
                                &super::part2::_arg_text(inst, i_i64),
                                profile,
                                false,
                                false,
                            ),
                            &inst.base.node.file,
                        );
                        if let Some(last) = diags.items.last().cloned() {
                            mine.push(last.clone());
                            if in_table {
                                table_mine.push(last);
                            }
                        }
                        break;
                    }
                    if kw != "ALIAS"
                        && kw != "UNITS"
                        && _find_bare_function_ref(Some(arg), profile, &declared_variables)
                    {
                        let msg = _bare_function_message(
                            &super::part2::_arg_text(inst, i_i64),
                            profile,
                            false,
                            true,
                        );
                        diags.error_basic(inst.base.node.line, &msg, &inst.base.node.file);
                        break;
                    }
                    let indexed_constants = if kw != "ALIAS" && i < params_count {
                        &constants
                    } else {
                        &empty_set
                    };
                    if let Some(problem) =
                        _find_arity_problem(Some(arg), profile, None, indexed_constants)
                    {
                        if problem == "empty" {
                            diags.error_basic(
                                inst.base.node.line,
                                &_arity_message(
                                    &problem,
                                    &super::part2::_arg_text(inst, i_i64),
                                    profile,
                                    true,
                                    false,
                                ),
                                &inst.base.node.file,
                            );
                            break;
                        }
                        if matches!(
                            problem.as_str(),
                            "many_member" | "constint" | "timer_option" | "indexed_const"
                        ) && table_only
                        {
                            break;
                        }
                        diags.error_basic(
                            inst.base.node.line,
                            &_arity_message(
                                &problem,
                                &super::part2::_arg_text(inst, i_i64),
                                profile,
                                false,
                                false,
                            ),
                            &inst.base.node.file,
                        );
                        _hold(held.as_deref_mut(), diags, &problem);
                        break;
                    }
                }
            }
            Stmt::Call(call) => {
                if call.args.is_empty()
                    && !call.base.text.contains('(')
                    && _is_bare_reference_problem(&call.name, profile, &declared_variables)
                {
                    diags.error_basic(
                        call.base.node.line,
                        &_bare_function_message(
                            &call.base.text,
                            profile,
                            true,
                            call.base.at_eol.get(),
                        ),
                        &call.base.node.file,
                    );
                    continue;
                }
                let tail_slice: [Option<ExprRef>; 1];
                let call_exprs: &[Option<ExprRef>] = if let Some(tail) = &call.tail_expr {
                    tail_slice = [Some(tail.clone())];
                    &tail_slice
                } else {
                    &call.args
                };
                if call_exprs
                    .iter()
                    .any(|a| _find_bare_function_ref(a.as_deref(), profile, &declared_variables))
                {
                    diags.error_basic(
                        call.base.node.line,
                        &_bare_function_message(
                            &call.base.text,
                            profile,
                            true,
                            call.base.at_eol.get(),
                        ),
                        &call.base.node.file,
                    );
                    continue;
                }
                if let Some(bad) = call
                    .args
                    .iter()
                    .position(|a| _multi_indexed_struct(a.as_deref(), &struct_vars))
                {
                    let bad_i64 = i64::try_from(bad).unwrap_or(0);
                    diags.error_basic(
                        call.base.node.line,
                        &_arity_message(
                            "many",
                            &super::part2::_call_arg_text(&call.base.text, bad_i64),
                            profile,
                            false,
                            false,
                        ),
                        &call.base.node.file,
                    );
                    if let Some(last) = diags.items.last().cloned() {
                        mine.push(last);
                    }
                    continue;
                }
                let mut problem = _call_arity_problem(&call.name, &call.args, profile);
                let mut in_arg: Option<usize> = None;
                if problem.is_none() {
                    for (k, a) in call_exprs.iter().enumerate() {
                        problem = arity(a.as_deref(), &constants);
                        if problem.is_some() {
                            if call.tail_expr.is_none()
                                && call.juxtaposed.is_none()
                                && sub_names.contains(&pystr::upper(&call.name))
                            {
                                in_arg = Some(k);
                            }
                            break;
                        }
                    }
                }
                if let Some(problem) = problem {
                    if problem == "constint"
                        && diags.items.iter().any(|d| {
                            d.line == call.base.node.line
                                && d.file == call.base.node.file
                                && d.message
                                    .starts_with("Parameter must be a constant integer:")
                        })
                    {
                        continue;
                    }
                    if !profile.single_pass
                        && pystr::upper(&call.name) == "SERIALOPEN"
                        && !call.args.is_empty()
                        && diags.items.iter().any(|d| {
                            d.line == call.base.node.line
                                && d.file == call.base.node.file
                                && d.message.starts_with("ComPort ")
                        })
                    {
                        continue;
                    }
                    if let Some(k) = in_arg {
                        let k_i64 = i64::try_from(k).unwrap_or(0);
                        let arg_text =
                            pystr::strip(&super::part2::_call_arg_text(&call.base.text, k_i64))
                                .to_owned();
                        diags.error_basic(
                            call.base.node.line,
                            &_arity_message(&problem, &arg_text, profile, false, false),
                            &call.base.node.file,
                        );
                        _hold(held.as_deref_mut(), diags, &problem);
                        continue;
                    }
                    diags.error_basic(
                        call.base.node.line,
                        &_arity_message(
                            &problem,
                            &call.base.text,
                            profile,
                            call.base.at_eol.get(),
                            true,
                        ),
                        &call.base.node.file,
                    );
                    _hold(held.as_deref_mut(), diags, &problem);
                }
            }
            Stmt::If(if_stmt) => {
                let mut problem = arity(if_stmt.cond.as_deref(), &constants);
                let struct_cond = problem.is_none()
                    && _multi_indexed_struct(if_stmt.cond.as_deref(), &struct_vars);
                if problem.is_none() && struct_cond {
                    problem = Some("many".to_owned());
                }
                let text = super::part2::_if_condition_text(stmt);
                if let (Some(problem), Some(text)) = (problem, text) {
                    if problem == "empty"
                        || problem == "timer_option"
                        || problem.ends_with(" cannot be negative")
                    {
                        diags.error_basic(
                            if_stmt.base.node.line,
                            &_arity_message(&problem, &text, profile, true, false),
                            &if_stmt.base.node.file,
                        );
                    } else if problem == "indexed_const" {
                        diags.error_basic(
                            if_stmt.base.node.line,
                            &_arity_message(&problem, &text, profile, false, false),
                            &if_stmt.base.node.file,
                        );
                    } else if problem == "constint" || problem == "network_constint" {
                        diags.error_basic(
                            if_stmt.base.node.line,
                            &format!("Parameter must be a constant integer: {text}."),
                            &if_stmt.base.node.file,
                        );
                    } else {
                        let verb = if problem == "few" {
                            "Expression expects , : "
                        } else {
                            "No Matching, or out of place ) in expression: "
                        };
                        diags.error_basic(
                            if_stmt.base.node.line,
                            &format!("{verb}{text}."),
                            &if_stmt.base.node.file,
                        );
                        _hold(held.as_deref_mut(), diags, &problem);
                        if struct_cond && let Some(last) = diags.items.last().cloned() {
                            mine.push(last);
                        }
                    }
                }
            }
            Stmt::ConstDecl(cd) if cd.value_span.is_some() => {
                let (vs0, vs1) = cd.value_span.unwrap_or((0, 0));
                let problem = arity(cd.value.as_deref(), &constants);
                let text = pystr::slice(&cd.base.text, Some(vs0), Some(vs1));
                if problem.as_deref() == Some("empty") {
                    let tail = pystr::slice(&cd.base.text, Some(vs0), None);
                    diags.error_basic(
                        cd.base.node.line,
                        &format!("Invalid, or out of place expression: {tail}."),
                        &cd.base.node.file,
                    );
                } else if let Some(problem) = problem {
                    let in_range =
                        problem == "timer_option" || problem.ends_with(" cannot be negative");
                    let message = _arity_message(&problem, text, profile, false, false);
                    diags.items.push(Rc::new(Diagnostic::basic(
                        "error",
                        cd.base.node.line,
                        &message,
                        &cd.base.node.file,
                    )));
                    if in_range {
                        if known.is_none() {
                            known = Some(ctx.known_names().names.clone());
                        }
                        if !const_unready_loaded {
                            const_unready_loaded = true;
                        }
                        let const_unready = ctx.not_yet_declared();
                        let unready = const_unready.get(&stmt_id(stmt));
                        let visible: BTreeSet<String> = known
                            .as_ref()
                            .map(|k| {
                                k.iter()
                                    .filter(|n| unready.is_none_or(|u| !u.contains(*n)))
                                    .cloned()
                                    .collect()
                            })
                            .unwrap_or_default();
                        let names = super::part1::_undefined_names(
                            cd.value.as_deref(),
                            &crate::checker::expressions::KnownView::from_set(&visible),
                        )
                        .into_iter()
                        .map(crate::checker::expressions::UndefinedName::into_string)
                        .collect::<Vec<_>>();
                        if !names.is_empty() {
                            super::part3::_report_const_names(cd, &names, profile, diags);
                        }
                    }
                }
            }
            Stmt::Assignment(assign) => {
                if known.is_none() {
                    known = Some(ctx.known_names().names.clone());
                }
                let known_ref = known.as_ref().unwrap_or(&empty_set);
                if !super::part1::_undefined_names(
                    assign.target.as_deref(),
                    &crate::checker::expressions::KnownView::from_set(known_ref),
                )
                .is_empty()
                {
                    continue;
                }
                if _multi_indexed_struct(assign.target.as_deref(), &struct_vars) {
                    diags.error_basic(
                        assign.base.node.line,
                        &structure::part2::_out_of_place(
                            profile,
                            &assign.base.text,
                            assign.base.at_eol.get(),
                        ),
                        &assign.base.node.file,
                    );
                    if let Some(last) = diags.items.last().cloned() {
                        mine.push(last);
                    }
                    continue;
                }
                if let Some(target_message) =
                    _function_target_message(assign, profile, &declared_variables)
                {
                    diags.error_basic(
                        assign.base.node.line,
                        &target_message,
                        &assign.base.node.file,
                    );
                    continue;
                }
                if _find_bare_function_ref(assign.value.as_deref(), profile, &declared_variables) {
                    diags.error_basic(
                        assign.base.node.line,
                        &_bare_function_message(
                            &assign.base.text,
                            profile,
                            true,
                            assign.base.at_eol.get(),
                        ),
                        &assign.base.node.file,
                    );
                    continue;
                }
                let mut problem = arity(assign.value.as_deref(), &constants);
                let struct_value = problem.is_none()
                    && _multi_indexed_struct(assign.value.as_deref(), &struct_vars);
                if struct_value {
                    problem = Some("many".to_owned());
                    if profile.single_pass
                        && !assign.compound
                        && matches!(assign.value.as_deref(), Some(Expr::Index(_)))
                        && structs::operand_kind(assign.target.as_deref(), &kinds, &struct_types)
                            .as_deref()
                            == Some("S")
                    {
                        diags.items.push(Rc::new(Diagnostic::basic(
                            "error",
                            assign.base.node.line,
                            _WRONG_TYPE,
                            &assign.base.node.file,
                        )));
                        if let Some(last) = diags.items.last().cloned() {
                            mine.push(last);
                        }
                    }
                }
                if let Some(problem) = problem {
                    let msg = _arity_message(
                        &problem,
                        &assign.base.text,
                        profile,
                        assign.base.at_eol.get(),
                        true,
                    );
                    diags.items.push(Rc::new(Diagnostic::new(
                        "error",
                        assign.base.node.line,
                        &msg,
                        &assign.base.node.file,
                        ShowFile::Bool(true),
                        stmt_id(stmt),
                        "",
                        "",
                    )));
                    _hold(held.as_deref_mut(), diags, &problem);
                    if struct_value && let Some(last) = diags.items.last().cloned() {
                        mine.push(last);
                    }
                }
            }
            _ => {}
        }
    }
    if !profile.single_pass && !table_mine.is_empty() {
        let outside = mine
            .iter()
            .any(|d| !table_mine.iter().any(|t| Rc::ptr_eq(d, t)));
        let keep: Vec<DiagRef> = if outside {
            Vec::new()
        } else {
            table_mine.iter().take(1).cloned().collect()
        };
        let drop: Vec<DiagRef> = table_mine
            .iter()
            .filter(|d| !keep.iter().any(|k| Rc::ptr_eq(d, k)))
            .cloned()
            .collect();
        let mut tail: Vec<DiagRef> = diags.items.drain(start..).collect();
        tail.retain(|d| !drop.iter().any(|x| Rc::ptr_eq(d, x)));
        diags.items.extend(tail);
        mine.retain(|d| !drop.iter().any(|x| Rc::ptr_eq(d, x)));
    }
    if profile.single_pass
        && diags.items[start..]
            .iter()
            .any(|d| !mine.iter().any(|m| Rc::ptr_eq(d, m)))
    {
        let mut tail: Vec<DiagRef> = diags.items.drain(start..).collect();
        tail.retain(|d| !mine.iter().any(|m| Rc::ptr_eq(d, m)));
        diags.items.extend(tail);
    }
}

/// Port of checker/expressions.py::_function_target_message
pub fn _function_target_message(
    stmt: &Assignment,
    profile: &Profile,
    declared: &BTreeSet<String>,
) -> Option<String> {
    let target = stmt.target.as_deref()?;
    if stmt.compound {
        return None;
    }
    let name = match target {
        Expr::Name(n) => pystr::upper(&n.name),
        Expr::Index(ix) => pystr::upper(&ix.name),
        _ => return None,
    };
    let record = profile.functions().get(&name)?;
    if declared.contains(&name) {
        return None;
    }
    let sep = if profile.fallback_echo_newline && stmt.base.at_eol.get() {
        "\n"
    } else {
        ""
    };
    if let Expr::Name(n) = target {
        if _is_bare_reference_problem(&n.name, profile, declared) {
            return Some(_bare_function_message(
                &stmt.base.text,
                profile,
                true,
                stmt.base.at_eol.get(),
            ));
        }
        return if record.nargs == Some(0) && record.argspec == 0 {
            Some(format!(
                "Invalid assignment in expression: {}{sep}.",
                stmt.base.text
            ))
        } else {
            None
        };
    }
    if let Expr::Index(ix) = target {
        let problem = _call_arity_problem(&ix.name, &ix.args, profile);
        if matches!(problem.as_deref(), Some("few" | "many" | "empty")) {
            return Some(_arity_message(
                problem.as_deref().unwrap_or(""),
                &stmt.base.text,
                profile,
                stmt.base.at_eol.get(),
                true,
            ));
        }
        if problem.is_none()
            && !ix.args.iter().any(Option::is_none)
            && record
                .nargs
                .is_some_and(|n| n > 0 && usize::try_from(n).ok() == Some(ix.args.len()))
        {
            return Some(format!(
                "Invalid assignment in expression: {}{sep}.",
                stmt.base.text
            ));
        }
    }
    None
}

struct TimeoutStateGuard;
impl Drop for TimeoutStateGuard {
    fn drop(&mut self) {
        super::part1::_TIMEOUT_GATE.with(|slot| *slot.borrow_mut() = None);
        super::part1::_TIMEOUT_WIDE.with(|slot| slot.borrow_mut().clear());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_text;
    use crate::profiles;

    #[test]
    fn data_param_text_adds_newline_only_on_last_arg() {
        let cr6 = profiles::get("cr6").expect("cr6 profile");
        let (prog, _) = parse_text(
            "BeginProg\nData 1, Foo, 3\nEndProg\n",
            "<test>",
            None,
            Some(cr6),
        );
        let Stmt::Instruction(inst) = &*prog.statements[1] else {
            panic!("expected Instruction");
        };
        assert_eq!(_data_param_text(inst, 0), "1");
        assert_eq!(_data_param_text(inst, 1), "Foo");
        assert_eq!(_data_param_text(inst, 2), "3\n");
    }

    #[test]
    fn call_arity_problem_matches_python() {
        let cr6 = profiles::get("cr6").expect("cr6 profile");
        let cr300 = profiles::get("cr300").expect("cr300 profile");
        for prof in [cr6, cr300] {
            let (p, _) = parse_text(
                "BeginProg\nX = CTYPE(1)\nX = CTYPE(1, 2)\nX = CTYPE(1, 2, 3)\nEndProg\n",
                "<test>",
                None,
                Some(prof),
            );
            let mut results = Vec::new();
            for st in &p.statements[1..4] {
                let Stmt::Assignment(a) = &**st else {
                    panic!("expected Assignment");
                };
                let Some(Expr::Index(ix)) = a.value.as_deref() else {
                    panic!("expected Index");
                };
                results.push(_call_arity_problem(&ix.name, &ix.args, prof));
            }
            assert_eq!(
                results,
                vec![Some("few".to_owned()), None, Some("many".to_owned())]
            );
        }

        let (p2, _) = parse_text(
            "BeginProg\nX = Sin()\nX = Sin(1,2)\nX = Round(1)\nX = Abs(1,2)\nX = EmailRelay(1,2)\nX = EmailRelay(1,2,3,4)\nX = TCPOpen(\"a\",1,2,\"x\")\nEndProg\n",
            "<test>",
            None,
            Some(cr6),
        );
        let mut results = Vec::new();
        for st in &p2.statements[1..p2.statements.len() - 1] {
            let Stmt::Assignment(a) = &**st else {
                panic!("expected Assignment");
            };
            let Some(Expr::Index(ix)) = a.value.as_deref() else {
                panic!("expected Index");
            };
            results.push(_call_arity_problem(&ix.name, &ix.args, cr6));
        }
        assert_eq!(
            results,
            vec![
                Some("empty".to_owned()),
                Some("many".to_owned()),
                Some("few".to_owned()),
                Some("incomplete".to_owned()),
                Some("few".to_owned()),
                None,
                Some("constint".to_owned()),
            ]
        );
    }

    #[test]
    fn find_arity_problem_handles_constants_members_and_nested_calls() {
        let cr6 = profiles::get("cr6").expect("cr6 profile");
        let (p, _) = parse_text(
            "BeginProg\nX = MyConst(1)\nX = Tb.F(1,2,3)\nX = Randomize(Round(1))\nEndProg\n",
            "<test>",
            None,
            Some(cr6),
        );
        let mut constants = BTreeSet::new();
        constants.insert("MYCONST".to_owned());

        let expr0 = match &*p.statements[1] {
            Stmt::Assignment(a) => a.value.as_deref(),
            _ => unreachable!(),
        };
        assert_eq!(
            _find_arity_problem(expr0, cr6, None, &constants),
            Some("indexed_const".to_owned())
        );

        let expr1 = match &*p.statements[2] {
            Stmt::Assignment(a) => a.value.as_deref(),
            _ => unreachable!(),
        };
        assert_eq!(
            _find_arity_problem(expr1, cr6, None, &constants),
            Some("many_member".to_owned())
        );

        let expr2 = match &*p.statements[3] {
            Stmt::Assignment(a) => a.value.as_deref(),
            _ => unreachable!(),
        };
        assert_eq!(
            _find_arity_problem(expr2, cr6, None, &constants),
            Some("few".to_owned())
        );
    }

    #[test]
    fn arity_message_matches_python() {
        let cr6 = profiles::get("cr6").expect("cr6 profile");
        let cr300 = profiles::get("cr300").expect("cr300 profile");

        assert_eq!(
            _arity_message("few", "Foo(1)", cr6, true, true),
            "Expression expects , : Foo(1)."
        );
        assert_eq!(
            _arity_message("many", "Foo(1)", cr6, true, true),
            "No Matching, or out of place ) in expression: Foo(1)."
        );
        assert_eq!(
            _arity_message("empty", "Foo(1)", cr6, true, true),
            "Invalid, or out of place expression: Foo(1)."
        );
        assert_eq!(
            _arity_message("constint", "Foo(1)", cr6, true, true),
            "Parameter must be a constant integer: Foo(1)."
        );
        assert_eq!(
            _arity_message("incomplete", "Foo(1)", cr6, true, true),
            "Incomplete expression: Foo(1)."
        );
        assert_eq!(
            _arity_message("indexed_const", "Foo(1)", cr6, true, true),
            "Incomplete expression: Foo(1)."
        );
        assert_eq!(
            _arity_message("timer_option", "Foo(1)", cr6, true, true),
            "Legal options are 0..4"
        );
        assert_eq!(
            _arity_message("Scan cannot be negative", "Foo(1)", cr6, true, true),
            "Scan cannot be negative"
        );

        assert_eq!(
            _arity_message("few", "Foo(1)", cr300, true, true),
            "Expression expects , : Foo(1)\n."
        );
        assert_eq!(
            _arity_message("few", "Foo(1)", cr300, false, false),
            "Expression expects , : Foo(1)."
        );
        assert_eq!(
            _arity_message("empty", "Foo(1)", cr300, true, true),
            "Invalid, or out of place expression: Foo(1)\n."
        );
        assert_eq!(
            _arity_message("empty", "Foo(1)", cr300, false, false),
            "Invalid, or out of place expression: Foo(1)."
        );
    }

    #[test]
    fn bare_function_checks_and_messages_match_python() {
        let cr6 = profiles::get("cr6").expect("cr6 profile");
        let cr300 = profiles::get("cr300").expect("cr300 profile");
        let empty = BTreeSet::new();
        let mut declared = BTreeSet::new();
        declared.insert("SIN".to_owned());

        assert!(_is_bare_reference_problem("Sin", cr6, &empty));
        assert!(!_is_bare_reference_problem("Sin", cr6, &declared));
        assert!(!_is_bare_reference_problem("Rnd", cr6, &empty));
        assert!(_is_bare_reference_problem("EmailRelay", cr6, &empty));

        let (p, _) = parse_text(
            "BeginProg\nX = Randomize(Sin)\nEndProg\n",
            "<test>",
            None,
            Some(cr6),
        );
        let expr = match &*p.statements[1] {
            Stmt::Assignment(a) => a.value.as_deref(),
            _ => unreachable!(),
        };
        assert!(_find_bare_function_ref(expr, cr6, &empty));
        assert!(!_find_bare_function_ref(expr, cr6, &declared));

        assert_eq!(
            _bare_function_message("X = Sin", cr6, true, true),
            "Expression expects ( : X = Sin."
        );
        assert_eq!(
            _bare_function_message("X = Sin", cr300, true, true),
            "Expression expects ( : X = Sin\n."
        );
        assert_eq!(
            _bare_function_message("Sin", cr300, false, true),
            "Expression expects ( : Sin."
        );
    }

    #[test]
    fn struct_var_names_and_multi_indexed_struct() {
        let cr6 = profiles::get("cr6").expect("cr6 profile");
        let (p, _) = parse_text(
            "StructureType S\n  v As Float\nEndStructureType\nPublic s1(2) As S\nBeginProg\nX = s1(1, 2).v\nX = s1(1).v(2)\nEndProg\n",
            "<test>",
            None,
            Some(cr6),
        );
        let svars = _struct_var_names(&p);
        assert!(svars.contains("S1"));
        let e0 = match &*p.statements[5] {
            Stmt::Assignment(a) => a.value.as_deref(),
            _ => unreachable!(),
        };
        let e1 = match &*p.statements[6] {
            Stmt::Assignment(a) => a.value.as_deref(),
            _ => unreachable!(),
        };
        assert!(_multi_indexed_struct(e0, &svars));
        assert!(!_multi_indexed_struct(e1, &svars));
    }

    #[test]
    fn hold_marks_many_member_phase_held() {
        let mut diags = Diagnostics::new();
        diags.error_basic(
            1,
            "No Matching, or out of place ) in expression: Tb.X(1,1,1).",
            "<t>",
        );
        let mut held = Vec::new();
        _hold(Some(&mut held), &mut diags, "many_member");
        assert_eq!(diags.items[0].phase, "held");
        assert_eq!(held.len(), 1);
        assert!(Rc::ptr_eq(&held[0], &diags.items[0]));
    }

    #[test]
    fn function_target_message_matches_python() {
        let cr6 = profiles::get("cr6").expect("cr6 profile");
        let cr300 = profiles::get("cr300").expect("cr300 profile");
        let empty = BTreeSet::new();

        for (prof, expected) in [
            (
                cr6,
                vec![
                    Some("Expression expects ( : Sin = 1.".to_owned()),
                    Some("Invalid assignment in expression: Rnd = 2.".to_owned()),
                    Some("Invalid assignment in expression: Sin(X) = 3.".to_owned()),
                    Some("Expression expects , : Round(1) = 2.".to_owned()),
                    Some("Invalid, or out of place expression: Sin() = 3.".to_owned()),
                    None,
                ],
            ),
            (
                cr300,
                vec![
                    Some("Expression expects ( : Sin = 1\n.".to_owned()),
                    Some("Invalid assignment in expression: Rnd = 2\n.".to_owned()),
                    Some("Invalid assignment in expression: Sin(X) = 3\n.".to_owned()),
                    Some("Expression expects , : Round(1) = 2\n.".to_owned()),
                    Some("Invalid, or out of place expression: Sin() = 3\n.".to_owned()),
                    None,
                ],
            ),
        ] {
            let (p, _) = parse_text(
                "BeginProg\nSin = 1\nRnd = 2\nSin(X) = 3\nRound(1) = 2\nSin() = 3\nSin += 1\nEndProg\n",
                "<test>",
                None,
                Some(prof),
            );
            let mut got = Vec::new();
            for st in &p.statements[1..p.statements.len() - 1] {
                let Stmt::Assignment(a) = &**st else {
                    panic!("expected Assignment");
                };
                got.push(_function_target_message(a, prof, &empty));
            }
            assert_eq!(got, expected);
        }
    }
}
