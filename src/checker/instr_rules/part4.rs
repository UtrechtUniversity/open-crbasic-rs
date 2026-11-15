//! Port of `checker/instr_rules.py` (lines 2312-3084).

#![allow(dead_code, unused_variables, unused_imports)]
#![allow(clippy::used_underscore_items)]

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::rc::Rc;
use std::sync::LazyLock;

use crate::checker::{context, expressions, intervals, longconv, var_types};
use crate::consteval::{self, Env};
use crate::diagnostics::{Diagnostic, Diagnostics, ShowFile};
use crate::profiles::{self, Profile};
use crate::syntax::{
    self, ConstValue, Expr, ExprRef, Instruction, OrderedMap, Program, ProgramRef, Stmt, StmtRef,
};
use crate::{pyre, pystr};

use super::part2::_f32_is_2_31;
use super::part3::{_SERIALIN_DEST_MESSAGE, _serialin_bad_destination};
use super::part5::_cdm_type_value;

const _EMAIL_FUNCTIONS: &[&str] = &["EMAILRECV", "EMAILSEND"];
fn _p4_find_instr_stmt<'a>(program: &'a Program, target: &Instruction) -> Option<&'a StmtRef> {
    program.statements.iter().find(|s| match s.as_ref() {
        Stmt::Instruction(instr) => std::ptr::eq(instr, target),
        _ => false,
    })
}

fn _p4_known_view(known: &expressions::KnownNames) -> expressions::KnownView<'_> {
    expressions::KnownView {
        names: &known.names,
        tables: &known.tables,
        out_of_scope: &known.out_of_scope,
        dotted_tables: known.dotted_tables,
    }
}

/// Port of checker/instr_rules.py::check_emailrecv_destination
pub fn check_emailrecv_destination(
    program: &ProgramRef,
    profile: &'static Profile,
    diags: &mut Diagnostics,
) {
    if !profile.functions().contains("EMAILRECV") {
        return;
    }
    let ctx = context::of(program, Some(profile), None);
    let types = ctx.serialin_types();
    let known = ctx.known_names();
    let known_view = _p4_known_view(known);

    let bad = |a: &Expr| -> bool {
        if _serialin_bad_destination(a, &types).unwrap_or(false) {
            return true;
        }
        if profile.single_pass {
            let name = match a {
                Expr::Name(n) => Some(pystr::upper(&n.name)),
                Expr::Index(ix) => Some(pystr::upper(&ix.name)),
                _ => None,
            };
            if let Some(upper) = name {
                return !known.names.contains(&upper) && !known.tables.contains(&upper);
            }
        }
        false
    };

    for stmt in &program.statements {
        if let Stmt::Assignment(assign) = stmt.as_ref()
            && !expressions::_undefined_names(assign.target.as_deref(), &known_view).is_empty()
        {
            continue;
        }
        let (args, fname): (&[Option<ExprRef>], String) = match stmt.as_ref() {
            Stmt::Assignment(assign) => match assign.value.as_deref() {
                Some(Expr::Index(ix))
                    if _EMAIL_FUNCTIONS.contains(&pystr::upper(&ix.name).as_str()) =>
                {
                    (&ix.args, pystr::upper(&ix.name))
                }
                Some(Expr::Name(n))
                    if _EMAIL_FUNCTIONS.contains(&pystr::upper(&n.name).as_str()) =>
                {
                    (&[], pystr::upper(&n.name))
                }
                _ => {
                    let nested: Vec<&crate::syntax::Index> =
                        expressions::_index_nodes(assign.value.as_deref())
                            .into_iter()
                            .filter(|n| {
                                _EMAIL_FUNCTIONS.contains(&pystr::upper(&n.name).as_str())
                                    && n.args.len() >= 9
                            })
                            .collect();
                    let Some(first) = nested.first() else {
                        continue;
                    };
                    (&first.args, pystr::upper(&first.name))
                }
            },
            Stmt::Call(call) if _EMAIL_FUNCTIONS.contains(&pystr::upper(&call.name).as_str()) => {
                (&call.args, pystr::upper(&call.name))
            }
            _ => continue,
        };
        let start = 8.min(args.len());
        let end = (if fname == "EMAILRECV" { 12 } else { 9 }).min(args.len());
        let tail = &args[start..end];
        if tail.iter().any(Option::is_none) {
            continue;
        }
        if !tail.is_empty()
            && !tail.iter().any(|a| {
                let Some(expr) = a.as_deref() else {
                    return false;
                };
                !matches!(expr, Expr::Binary(_) | Expr::Unary(_)) && bad(expr)
            })
        {
            continue;
        }
        let key = (stmt.line(), stmt.file());
        let mut at = diags
            .items
            .iter()
            .position(|d| (d.line, d.file.as_str()) == key)
            .unwrap_or(diags.items.len());
        while profile.single_pass
            && at < diags.items.len()
            && (diags.items[at].line, diags.items[at].file.as_str()) == key
            && diags.items[at].message.ends_with(" is not defined.")
        {
            at += 1;
        }
        diags.items.insert(
            at,
            Rc::new(Diagnostic::basic(
                "error",
                stmt.line(),
                _SERIALIN_DEST_MESSAGE,
                stmt.file(),
            )),
        );
    }
}

/// Port of checker/instr_rules.py::check_tcpopen_handle_type
pub fn check_tcpopen_handle_type(
    program: &ProgramRef,
    profile: &'static Profile,
    diags: &mut Diagnostics,
) {
    if !matches!(profile.name, "cr6" | "cr1000x") {
        return;
    }
    let types = context::of(program, Some(profile), None).variable_types(false);
    for stmt in &program.statements {
        let calls: Vec<Vec<Option<ExprRef>>> = match stmt.as_ref() {
            Stmt::Call(call) if pystr::upper(&call.name) == "TCPOPEN" => vec![call.args.clone()],
            _ => expressions::_stmt_exprs(stmt)
                .into_iter()
                .flat_map(|e| {
                    expressions::_index_nodes(e.as_deref())
                        .into_iter()
                        .filter(|ix| pystr::upper(&ix.name) == "TCPOPEN")
                        .map(|ix| ix.args.clone())
                        .collect::<Vec<_>>()
                })
                .collect(),
        };
        for args in calls.into_iter().take(1) {
            if args.len() < 5 {
                continue;
            }
            let Some(arg) = args[4].as_deref() else {
                continue;
            };
            let bad = match arg {
                Expr::Name(n) => {
                    let kind = types.get(&pystr::upper(&n.name));
                    kind.is_some_and(|k| k != "LONG" && k != "BOOLEAN")
                }
                Expr::Index(ix) => {
                    let kind = types.get(&pystr::upper(&ix.name));
                    kind.is_some_and(|k| k != "LONG" && k != "BOOLEAN")
                }
                Expr::Number(_) | Expr::String(_) | Expr::Binary(_) | Expr::Unary(_) => true,
                _ => false,
            };
            if bad {
                let key = (stmt.line(), stmt.file());
                let at = diags
                    .items
                    .iter()
                    .position(|d| (d.line, d.file.as_str()) == key)
                    .unwrap_or(diags.items.len());
                diags.items.insert(
                    at,
                    Rc::new(Diagnostic::basic(
                        "error",
                        stmt.line(),
                        "Parameter must be a variable of type long",
                        stmt.file(),
                    )),
                );
            }
        }
    }
}

/// Port of checker/instr_rules.py::check_tcpopen_connections
pub fn check_tcpopen_connections(
    program: &ProgramRef,
    profile: &'static Profile,
    diags: &mut Diagnostics,
    env: &Env,
) {
    static INT_OR_HEX: LazyLock<pyre::Regex> =
        LazyLock::new(|| pyre::Regex::new(r"\d+|&[Hh][0-9A-Fa-f]+", 0));

    let ctx = context::of(program, Some(profile), Some(env.clone()));
    let types = ctx.variable_types(false);
    let dims = ctx.declared_all_dims();
    let mut declared: BTreeMap<String, i64> = BTreeMap::new();
    for st in &program.statements {
        if let Stmt::Declaration(decl) = st.as_ref() {
            for v in decl.vars.borrow().iter() {
                let upper = pystr::upper(&v.name);
                *declared.entry(upper).or_insert(0) += 1;
            }
        }
    }
    let busy: BTreeSet<(i64, String)> = diags
        .items
        .iter()
        .map(|d| (d.line, d.file.clone()))
        .collect();
    for stmt in &ctx.flat().statements {
        let calls: Vec<Vec<Option<ExprRef>>> = match stmt.as_ref() {
            Stmt::Call(call) if pystr::upper(&call.name) == "TCPOPEN" => vec![call.args.clone()],
            _ => expressions::_stmt_exprs(stmt)
                .into_iter()
                .flat_map(|e| {
                    expressions::_index_nodes(e.as_deref())
                        .into_iter()
                        .filter(|ix| pystr::upper(&ix.name) == "TCPOPEN")
                        .map(|ix| ix.args.clone())
                        .collect::<Vec<_>>()
                })
                .collect(),
        };
        for args in calls.into_iter().take(1) {
            if !(5..=7).contains(&args.len())
                || args[4].is_none()
                || (args.len() > 5 && args[5].is_none())
            {
                continue;
            }
            let handle = args[4].as_deref();
            let name = match handle {
                Some(Expr::Name(n)) => Some(pystr::upper(&n.name)),
                Some(Expr::Index(ix)) => Some(pystr::upper(&ix.name)),
                _ => None,
            };
            let Some(name) = name else {
                continue;
            };
            if declared.get(&name).copied() != Some(1)
                || types.get(&name).map(String::as_str) != Some("LONG")
                || !dims.contains_key(&name)
            {
                continue;
            }
            let count = if args.len() > 5 {
                match consteval::value_of(args[5].as_deref(), env) {
                    Ok(v) => v,
                    Err(_) => continue,
                }
            } else {
                ConstValue::Int(0)
            };
            let arg5 = args.get(5).and_then(Option::as_deref);
            let lit = match arg5 {
                Some(Expr::Unary(u)) if u.op == "-" || u.op == "+" => u.operand.as_deref(),
                other => other,
            };
            let size: i64 = dims.get(&name).map_or(1, |d| d.iter().copied().product());
            if let Some(Expr::Number(num)) = lit
                && INT_OR_HEX.fullmatch(&num.text).is_none()
                && arg5.is_some_and(|a| std::ptr::eq(lit.unwrap(), a))
                && !busy.contains(&(stmt.line(), stmt.file().to_string()))
            {
                if !profile.single_pass {
                    diags.error(
                        stmt.line(),
                        &format!("Max number of concurrent connections must be 1..{size}"),
                        stmt.file(),
                        ShowFile::Bool(true),
                        syntax::rc_id(stmt),
                        "",
                        "",
                    );
                }
                let sep = if profile.fallback_echo_newline && stmt.at_eol() {
                    "\n"
                } else {
                    ""
                };
                diags.error(
                    stmt.line(),
                    &format!(
                        "Parameter must be a constant integer: {}{sep}.",
                        stmt.text()
                    ),
                    stmt.file(),
                    ShowFile::Bool(true),
                    syntax::rc_id(stmt),
                    "",
                    "",
                );
                continue;
            }
            if profile.single_pass {
                continue;
            }
            let count_int = match count {
                ConstValue::Int(i) => Some(i),
                ConstValue::Float(f) | ConstValue::CFloat(f) if pystr::is_integer(f) => {
                    pystr::py_int_from_f64(f)
                }
                _ => None,
            };
            let Some(count_int) = count_int else {
                continue;
            };
            if (1..=size).contains(&count_int)
                || busy.contains(&(stmt.line(), stmt.file().to_string()))
            {
                continue;
            }
            diags.error(
                stmt.line(),
                &format!("Max number of concurrent connections must be 1..{size}"),
                stmt.file(),
                ShowFile::Bool(true),
                syntax::rc_id(stmt),
                "",
                "",
            );
        }
    }
}

fn _pulse_channel_regex(profile_name: &str) -> Option<&'static pyre::Regex> {
    static CR6: LazyLock<pyre::Regex> =
        LazyLock::new(|| pyre::Regex::new(r"C[1-4]|U(?:[1-9]|1[0-2])", 0));
    static CR1000X: LazyLock<pyre::Regex> = LazyLock::new(|| pyre::Regex::new(r"C[1-8]|P[12]", 0));
    match profile_name {
        "cr6" => Some(&CR6),
        "cr1000x" => Some(&CR1000X),
        _ => None,
    }
}

/// Port of `checker/instr_rules.py::check_pulsecount_channels`.
pub fn check_pulsecount_channels(program: &Program, profile: &Profile, diags: &mut Diagnostics) {
    static NEXT_SCAN_RE: LazyLock<pyre::Regex> =
        LazyLock::new(|| pyre::Regex::new(r"(?i)\s*Next\s+Scan\b", 0));

    let Some(channel) = _pulse_channel_regex(profile.name) else {
        return;
    };
    let mut used: Option<BTreeSet<String>> = None;
    for stmt in &program.statements {
        let Stmt::Instruction(instr) = stmt.as_ref() else {
            continue;
        };
        let kw = pystr::upper(&instr.keyword);
        if kw == "SCAN" {
            used = Some(BTreeSet::new());
        } else if kw == "NEXTSCAN" || (kw == "NEXT" && NEXT_SCAN_RE.match_(stmt.text()).is_some()) {
            used = None;
        } else if kw == "PULSECOUNT"
            && let Some(used_set) = used.as_mut()
            && instr.args.len() > 2
            && let Some(Expr::Name(n)) = instr.args[2].as_deref()
        {
            let name = pystr::upper(&n.name);
            if channel.fullmatch(&name).is_some() {
                if used_set.contains(&name) {
                    if profile.name == "cr1000x" && name.starts_with('C') {
                        diags.error_basic(
                            stmt.line(),
                            &format!("Pulsecount: channel {name} already in use or unavailable."),
                            stmt.file(),
                        );
                    }
                    diags.error_basic(
                        stmt.line(),
                        &format!("{name} already used as PulseCount in this scan."),
                        stmt.file(),
                    );
                }
                used_set.insert(name);
            }
        }
    }
}

/// Port of checker/instr_rules.py::check_sdi12_nan_fill
pub fn check_sdi12_nan_fill(
    st: &Instruction,
    program: &ProgramRef,
    profile: &'static Profile,
    diags: &mut Diagnostics,
    env: &Env,
) {
    if !matches!(profile.name, "cr6" | "cr1000x") || st.args.len() <= 6 {
        return;
    }
    let Some(dest) = st.args[0].as_deref() else {
        return;
    };
    let dest_name = match dest {
        Expr::Name(n) => &n.name,
        Expr::Index(ix) => &ix.name,
        _ => return,
    };
    let Some(st_stmt) = _p4_find_instr_stmt(program, st) else {
        return;
    };
    let ctx = context::of(program, Some(profile), Some(env.clone()));
    let tables = ctx.scope_tables(true, false);
    let scope = tables.for_statement(st_stmt);
    let Some(kind) = scope.types.get(&pystr::upper(dest_name)) else {
        return;
    };
    if kind.starts_with("STRING") {
        return;
    }
    let shape = scope
        .dims
        .get(&pystr::upper(dest_name))
        .cloned()
        .unwrap_or_else(|| vec![1]);
    if let Expr::Index(ix) = dest {
        let indexed = ix.args.iter().any(Option::is_some);
        if (indexed && ix.args.len() != shape.len()) || shape == [1] {
            return;
        }
    }
    let offset = crate::checker::instr_measure::_flat_offset(dest, &shape, Some(env));
    let total: i64 = shape.iter().copied().product();
    let Ok(fill) = consteval::value_of(st.args[6].as_deref(), env) else {
        return;
    };
    let fill_int = match fill {
        ConstValue::Int(i) => i,
        ConstValue::Float(f) | ConstValue::CFloat(f) if pystr::is_integer(f) => {
            let Some(i) = pystr::py_int_from_f64(f) else {
                return;
            };
            i
        }
        _ => return,
    };
    let Some(offset) = offset else {
        return;
    };
    if offset >= total || !(-(1_i64 << 31)..(1_i64 << 31)).contains(&fill_int) {
        return;
    }
    let room = total - offset;
    if !(-1..=room).contains(&fill_int) {
        diags.error_basic(
            st.base.node.line,
            &format!("Illegal NAN fill locations, legal is -1 or 1..{room}(room in the destination array)"),
            &st.base.node.file,
        );
    }
}

fn _serial_terminal_map(profile_name: &str) -> Option<&'static [(i64, i64)]> {
    const CR6: &[(i64, i64)] = &[
        (9, 101),
        (10, 103),
        (11, 201),
        (12, 203),
        (13, 205),
        (14, 207),
        (15, 209),
        (16, 211),
    ];
    const CR1000X: &[(i64, i64)] = &[(9, 101), (10, 103), (11, 105), (12, 107)];
    match profile_name {
        "cr6" => Some(CR6),
        "cr1000x" => Some(CR1000X),
        _ => None,
    }
}

fn _io_terminals(profile_name: &str) -> BTreeSet<i64> {
    match profile_name {
        "cr6" => (101..105).chain(201..213).collect(),
        "cr1000x" => (101..109).collect(),
        _ => BTreeSet::new(),
    }
}

fn _sdi12_terminals(profile_name: &str) -> BTreeSet<i64> {
    match profile_name {
        "cr6" => [101, 103, 201, 203, 205, 207, 209, 211]
            .into_iter()
            .collect(),
        "cr1000x" => [101, 103, 105, 107].into_iter().collect(),
        _ => BTreeSet::new(),
    }
}

fn _terminal_role(kw: &str) -> Option<(usize, &'static str, &'static str)> {
    match kw {
        "SDI12RECORDER" => Some((1, "sdi12", "SDI12Recorder")),
        "SDI12SENSORSETUP" => Some((1, "sdi12", "SDI12SensorSetup")),
        "PORTSET" => Some((0, "output", "PortSet")),
        "PULSECOUNT" => Some((2, "pulse", "Pulsecount")),
        _ => None,
    }
}

/// Port of checker/instr_rules.py::check_terminal_conflicts
pub fn check_terminal_conflicts(
    program: &Program,
    profile: &Profile,
    diags: &mut Diagnostics,
    env: &Env,
) {
    let Some(serial_map) = _serial_terminal_map(profile.name) else {
        return;
    };
    let io = _io_terminals(profile.name);
    let sdi12 = _sdi12_terminals(profile.name);

    let value = |arg: Option<&Expr>| -> Option<i64> {
        let v = consteval::value_of(arg, env).ok()?;
        match v {
            ConstValue::Int(i) => Some(i),
            ConstValue::Float(f) | ConstValue::CFloat(f)
                if f.is_finite() && pystr::is_integer(f) =>
            {
                pystr::py_int_from_f64(f)
            }
            _ => None,
        }
    };
    let term_name = |t: i64| -> String {
        let prefix = if t < 200 { 'C' } else { 'U' };
        format!("{prefix}{}", t % 100)
    };

    let mut owner: BTreeMap<i64, String> = BTreeMap::new();
    let mut sdi_kind: BTreeMap<i64, String> = BTreeMap::new();
    for st in &program.statements {
        if let Stmt::Call(call) = st.as_ref()
            && pystr::upper(&call.name) == "SERIALOPEN"
            && !call.explicit
            && !call.args.is_empty()
            && let Some(port) = value(call.args[0].as_deref())
            && let Some((_, t)) = serial_map.iter().find(|&&(p, _)| p == port)
        {
            owner.insert(*t, "serial".to_string());
            owner.insert(*t + 1, "serial".to_string());
        }
    }
    for st in &program.statements {
        let Stmt::Instruction(instr) = st.as_ref() else {
            continue;
        };
        let kw = pystr::upper(&instr.keyword);
        let (claims, label): (Vec<(i64, &'static str)>, &'static str) =
            if kw == "WRITEIO" && !instr.args.is_empty() {
                let Some(mask) = value(instr.args[0].as_deref()) else {
                    continue;
                };
                let count = io.iter().filter(|&&t| t < 200).count();
                let mut out = Vec::new();
                for b in 0..count {
                    if ((mask >> b) & 1) != 0 {
                        out.push((101 + i64::try_from(b).unwrap_or(0), "output"));
                    }
                }
                (out, "WriteIO")
            } else if let Some((pos, role, lbl)) = _terminal_role(&kw)
                && instr.args.len() > pos
            {
                let Some(t) = value(instr.args[pos].as_deref()) else {
                    continue;
                };
                let valid_set = if role == "sdi12" { &sdi12 } else { &io };
                if !valid_set.contains(&t) {
                    continue;
                }
                (vec![(t, role)], lbl)
            } else {
                continue;
            };

        for (t, role) in claims {
            let existing = owner.entry(t).or_insert_with(|| role.to_string());
            if existing != role {
                let kind = if label.starts_with("SDI12") {
                    "sdi12_conflict"
                } else {
                    ""
                };
                diags.error(
                    st.line(),
                    &format!(
                        "{label}: channel {} already in use or unavailable.",
                        term_name(t)
                    ),
                    st.file(),
                    ShowFile::Bool(true),
                    0,
                    kind,
                    "",
                );
            }
            if role == "sdi12" {
                let prev = sdi_kind.entry(t).or_insert_with(|| kw.clone());
                if prev != &kw {
                    diags.error_basic(
                        st.line(),
                        "SDI Recorder and Sensor cannot use the same port.",
                        st.file(),
                    );
                }
            }
        }
    }
}

/// Port of `checker/instr_rules.py::_port_number`.
pub fn _port_number(arg: Option<&Expr>) -> Option<i64> {
    static PORT_NAME: LazyLock<pyre::Regex> = LazyLock::new(|| pyre::Regex::new(r"([CU])(\d+)", 0));

    match arg? {
        Expr::Name(name) => {
            let upper = pystr::upper(&name.name);
            let parsed = PORT_NAME.fullmatch(&upper)?;
            let base = if parsed.group(1)? == "C" { 100 } else { 200 };
            // Any overflow remains outside the supported port sets.
            Some(
                pystr::parse_int(parsed.group(2)?)
                    .unwrap_or(i64::MAX)
                    .saturating_add(base),
            )
        }
        Expr::Number(number) => match &number.value {
            crate::syntax::NumberValue::Int(value) => Some(*value),
            crate::syntax::NumberValue::Float(value) if pystr::is_integer(*value) => {
                pystr::py_int_from_f64(*value)
            }
            crate::syntax::NumberValue::BigInt(value) => {
                pystr::py_int_from_f64(pystr::parse_float(value)?)
            }
            crate::syntax::NumberValue::Float(_) => None,
        },
        _ => None,
    }
}

/// Port of `checker/instr_rules.py::check_spi_open_order`.
pub fn check_spi_open_order(program: &Program, profile: &Profile, diags: &mut Diagnostics) {
    let valid: &[i64] = match profile.name {
        "cr6" => &[101, 201, 205, 209],
        "cr1000x" => &[101, 105],
        _ => return,
    };
    let mut opened = BTreeSet::new();
    for stmt in &program.statements {
        let Stmt::Instruction(instr) = stmt.as_ref() else {
            continue;
        };
        let kw = pystr::upper(&instr.keyword);
        if kw == "SPIOPEN" {
            let Some(port) = _port_number(instr.args.first().and_then(Option::as_deref)) else {
                return;
            };
            if port < 100 && profile.name == "cr6" {
                return;
            }
            if valid.contains(&port) {
                opened.insert(port);
            }
        } else if matches!(kw.as_str(), "SPIREAD" | "SPIWRITE")
            && let [port, _, _] = instr.args.as_slice()
            && let Some(port) = _port_number(port.as_deref())
            && valid.contains(&port)
            && !opened.contains(&port)
        {
            let name = if kw == "SPIREAD" {
                "SPIRead"
            } else {
                "SPIWrite"
            };
            diags.error_basic(
                stmt.line(),
                &format!("SPIOpen must precede {name}."),
                stmt.file(),
            );
        }
    }
}

const _DNP_BOOL: &[&str] = &["BOOLEAN"];
const _DNP_LONG: &[&str] = &["LONG"];
const _DNP_ANALOG: &[&str] = &["LONG", "FLOAT"];
const _DNP_STRING: &[&str] = &["STRING"];
const _DNP_KNOWN_TYPES: &[&str] = &["BOOLEAN", "LONG", "FLOAT", "STRING"];

struct DnpGroupSpec {
    base: Option<i64>,
    is_event: bool,
    allowed: &'static [&'static str],
    type_name: &'static str,
    top: Option<i64>,
}

fn _dnp_group_spec(group: i64) -> Option<DnpGroupSpec> {
    let spec = match group {
        1 | 10 => DnpGroupSpec {
            base: None,
            is_event: false,
            allowed: _DNP_BOOL,
            type_name: "Boolean",
            top: Some(2),
        },
        2 => DnpGroupSpec {
            base: Some(1),
            is_event: true,
            allowed: _DNP_BOOL,
            type_name: "Boolean",
            top: Some(3),
        },
        12 => DnpGroupSpec {
            base: Some(10),
            is_event: false,
            allowed: _DNP_BOOL,
            type_name: "Boolean",
            top: Some(1),
        },
        20 => DnpGroupSpec {
            base: None,
            is_event: false,
            allowed: _DNP_LONG,
            type_name: "Long",
            top: Some(8),
        },
        21 => DnpGroupSpec {
            base: Some(20),
            is_event: false,
            allowed: _DNP_LONG,
            type_name: "Long",
            top: Some(12),
        },
        22 => DnpGroupSpec {
            base: Some(20),
            is_event: true,
            allowed: _DNP_LONG,
            type_name: "Long",
            top: Some(8),
        },
        23 => DnpGroupSpec {
            base: Some(21),
            is_event: true,
            allowed: _DNP_LONG,
            type_name: "Long",
            top: Some(8),
        },
        30 => DnpGroupSpec {
            base: None,
            is_event: false,
            allowed: _DNP_ANALOG,
            type_name: "Long",
            top: Some(5),
        },
        31 => DnpGroupSpec {
            base: Some(30),
            is_event: false,
            allowed: _DNP_ANALOG,
            type_name: "Long",
            top: Some(7),
        },
        32 => DnpGroupSpec {
            base: Some(30),
            is_event: true,
            allowed: _DNP_ANALOG,
            type_name: "Long",
            top: Some(7),
        },
        33 => DnpGroupSpec {
            base: Some(31),
            is_event: true,
            allowed: _DNP_ANALOG,
            type_name: "Long",
            top: Some(7),
        },
        40 => DnpGroupSpec {
            base: None,
            is_event: false,
            allowed: _DNP_ANALOG,
            type_name: "Long",
            top: Some(3),
        },
        41 => DnpGroupSpec {
            base: Some(40),
            is_event: false,
            allowed: _DNP_ANALOG,
            type_name: "Long",
            top: Some(3),
        },
        110 => DnpGroupSpec {
            base: None,
            is_event: false,
            allowed: _DNP_STRING,
            type_name: "String",
            top: None,
        },
        _ => return None,
    };
    Some(spec)
}

pub struct DnpConv<'a> {
    pub env: Option<&'a Env>,
    pub profile: &'static Profile,
    pub names: &'a BTreeSet<String>,
    pub literals: &'a BTreeSet<String>,
}

/// Port of checker/instr_rules.py::_dnp_long
pub fn _dnp_long(arg: Option<&Expr>, conv: &DnpConv<'_>, wrap16: bool) -> Option<i64> {
    let env = conv.env?;
    let arg = arg?;
    let number =
        longconv::long_value_with(Some(arg), env, conv.profile, conv.names, conv.literals)?;
    if wrap16 {
        Some(((number + 32768) & 0xFFFF) - 32768)
    } else {
        Some(number)
    }
}

/// Port of checker/instr_rules.py::_dnp_source
pub fn _dnp_source(
    stmt: &Instruction,
    conv: &DnpConv<'_>,
    dims: &OrderedMap<String, Vec<i64>>,
) -> (Vec<String>, Option<i64>) {
    let Some(src) = stmt.args.first().and_then(Option::as_deref) else {
        return (Vec::new(), None);
    };
    let (src_name, ix_args) = match src {
        Expr::Name(n) => (&n.name, None),
        Expr::Index(ix) => (&ix.name, Some(&ix.args)),
        _ => return (Vec::new(), None),
    };
    let Some(shape) = dims.get(&pystr::upper(src_name)) else {
        return (Vec::new(), None);
    };
    let total: i64 = shape.iter().copied().product();
    let Some(ix_args) = ix_args else {
        return (Vec::new(), if shape == &[1] { Some(1) } else { None });
    };
    if ix_args.is_empty() {
        return (Vec::new(), Some(total));
    }
    if ix_args.len() > shape.len() {
        return (Vec::new(), None);
    }
    let Some(env) = conv.env else {
        return (Vec::new(), None);
    };
    let mut values = Vec::with_capacity(ix_args.len());
    for a in ix_args {
        let a_ref = a.as_deref();
        let number = match a_ref {
            Some(Expr::Unary(u)) if u.op == "-" || u.op == "+" => u.operand.as_deref(),
            other => other,
        };
        if let Some(Expr::Name(n)) = number
            && matches!(pystr::upper(&n.name).as_str(), "TRUE" | "FALSE")
        {
            return (Vec::new(), None);
        }
        let simple = matches!(number, Some(Expr::Number(_)))
            || matches!(number, Some(Expr::Name(n)) if env.consts.borrow().contains_key(&pystr::upper(&n.name)));
        let value = if simple {
            consteval::value_of(a_ref, env).ok()
        } else {
            None
        };
        let int_val = match value {
            Some(ConstValue::Int(i)) => Some(i),
            Some(ConstValue::Float(f) | ConstValue::CFloat(f)) => pystr::py_int_from_f64(f),
            _ => None,
        };
        let Some(v) = int_val else {
            return (
                vec!["Cannot use a variable index reference.".to_string()],
                None,
            );
        };
        values.push(v);
    }
    let last = values.len() - 1;
    if values[..last].iter().any(|&v| v < 0) {
        return (
            vec!["Cannot use a variable index reference.".to_string()],
            None,
        );
    }
    if values[last] < 0 {
        values[last] = -values[last];
    }
    if values
        .iter()
        .zip(shape.iter())
        .any(|(&v, &s)| !(1..=s).contains(&v))
    {
        return (
            vec![
                format!("Variable {src_name} out of bounds."),
                "Cannot use a variable index reference.".to_string(),
            ],
            None,
        );
    }
    if values.len() != shape.len() {
        return (Vec::new(), None);
    }
    let mut offset = 0_i64;
    for (&v, &n) in values.iter().zip(shape.iter()) {
        offset = offset * n + (v - 1);
    }
    (Vec::new(), Some(total - offset))
}

const _DNP_FLAG_BINARY_OPS: &[&str] = &[
    "+", "-", "*", "\\", "MOD", "&", "<<", ">>", "AND", "OR", "XOR", "IMP", "EQV", "=", "<>", "<",
    ">", "<=", ">=",
];
const _DNP_FLAG_TYPE: &str = "Flag paramter must be of type LONG";

/// Port of checker/instr_rules.py::_dnp_long_flag
pub fn _dnp_long_flag(
    arg: Option<&Expr>,
    typer: &var_types::Typer,
    env: &Env,
    profile: &'static Profile,
    routine_names: &BTreeSet<String>,
) -> Option<bool> {
    let arg = arg?;
    if let Some(literal) = intervals::timeout_value_with(Some(arg), None, profile, &BTreeSet::new())
    {
        return Some(matches!(literal, intervals::TimeoutValue::Int(_)));
    }
    if let Expr::Name(n) = arg {
        let name = pystr::upper(&n.name);
        let const_val = env.consts.borrow().get(&name).cloned();
        if let Some(value) = const_val {
            if matches!(value, ConstValue::Str(_)) {
                return Some(false);
            }
            if typer
                .consts
                .get(&name)
                .is_some_and(|c| c.type_.as_deref() == Some("LONG"))
            {
                return Some(true);
            }
            match value {
                ConstValue::Float(f) | ConstValue::CFloat(f) => {
                    if !f.is_finite() || !pystr::is_integer(f) {
                        return Some(false);
                    }
                    if f < -2_147_483_648.0_f64 {
                        return Some(_f32_is_2_31(&ConstValue::Float(-f)));
                    }
                    let mag = f.abs();
                    let Some(mag_int) = pystr::py_int_from_f64(mag) else {
                        return Some(false);
                    };
                    let sign = if f < 0.0 { -1 } else { 1 };
                    let converted =
                        intervals::_decimal_with(mag_int.unsigned_abs(), sign, profile, true);
                    return Some(matches!(converted, Some(intervals::TimeoutValue::Int(_))));
                }
                ConstValue::Int(i) => {
                    if i < longconv::INT_MIN {
                        #[allow(clippy::cast_precision_loss)]
                        let neg = -(i as f64);
                        return Some(_f32_is_2_31(&ConstValue::Float(neg)));
                    }
                    let sign = if i < 0 { -1 } else { 1 };
                    let converted = intervals::_decimal_with(i.unsigned_abs(), sign, profile, true);
                    return Some(matches!(converted, Some(intervals::TimeoutValue::Int(_))));
                }
                ConstValue::BigInt(s) => {
                    let Some(f) = pystr::parse_float(&s) else {
                        return Some(false);
                    };
                    if !f.is_finite() {
                        return Some(false);
                    }
                    if f < -2_147_483_648.0_f64 {
                        return Some(_f32_is_2_31(&ConstValue::Float(-f)));
                    }
                    return Some(false);
                }
                ConstValue::Str(_) => unreachable!(),
            }
        }
        if matches!(name.as_str(), "INF" | "NAN") {
            return Some(false);
        }
    }
    if let Expr::Unary(u) = arg {
        return _dnp_long_flag(u.operand.as_deref(), typer, env, profile, routine_names);
    }
    if let Expr::Binary(b) = arg {
        if matches!(b.op.as_str(), "/" | "^") {
            return Some(false);
        }
        if _DNP_FLAG_BINARY_OPS.contains(&b.op.as_str()) {
            let left = _dnp_long_flag(b.left.as_deref(), typer, env, profile, routine_names);
            let right = _dnp_long_flag(b.right.as_deref(), typer, env, profile, routine_names);
            return if left == Some(false) || right == Some(false) {
                Some(false)
            } else if left.is_none() || right.is_none() {
                None
            } else {
                Some(true)
            };
        }
    }
    if let Expr::Index(ix) = arg
        && typer.variable(Some(arg)).is_none()
    {
        let name = pystr::upper(&ix.name);
        if routine_names.contains(&name) {
            return Some(
                typer
                    .function_types
                    .get(&name)
                    .map_or("FLOAT", String::as_str)
                    == "LONG",
            );
        }
        if name == "INT" {
            return Some(false);
        }
        if name == "CTYPE"
            && ix.args.len() == 2
            && let Some(Expr::Name(k)) = ix.args[1].as_deref()
        {
            let kind = pystr::upper(&k.name);
            return match kind.as_str() {
                "LONG" => Some(true),
                "FLOAT" | "STRING" => Some(false),
                _ => None,
            };
        }
    }
    let kind = typer.of(Some(arg), 0)?;
    Some(kind == "LONG" || kind == "BOOLEAN")
}

#[derive(Clone, Debug)]
pub struct DnpStmtInfo {
    pub group: Option<i64>,
    pub before: Vec<String>,
    pub after: Vec<String>,
    pub stops_early: bool,
    pub fails_late: bool,
}

/// Port of checker/instr_rules.py::_dnp_statement
pub fn _dnp_statement(
    stmt: &Instruction,
    conv: &DnpConv<'_>,
    dims: &OrderedMap<String, Vec<i64>>,
    kind: Option<&str>,
    long_flag: Option<bool>,
) -> DnpStmtInfo {
    let group = if stmt.args.len() >= 3 {
        _dnp_long(stmt.args[2].as_deref(), conv, true)
    } else {
        None
    };
    let Some(group) = group else {
        return DnpStmtInfo {
            group: None,
            before: Vec::new(),
            after: Vec::new(),
            stops_early: false,
            fails_late: false,
        };
    };
    let full = stmt.args.len() == 8;
    let (source_messages, left) = if conv.env.is_some() {
        _dnp_source(stmt, conv, dims)
    } else {
        (Vec::new(), None)
    };
    if !source_messages.is_empty() {
        return DnpStmtInfo {
            group: Some(group),
            before: source_messages,
            after: Vec::new(),
            stops_early: true,
            fails_late: false,
        };
    }
    let before = Vec::new();
    let mut after = Vec::new();
    let swath = if full {
        _dnp_long(stmt.args[1].as_deref(), conv, false)
    } else {
        None
    };
    let limit = if full && (kind != Some("STRING") || group == 110) {
        left
    } else {
        None
    };
    if let (Some(swath), Some(limit)) = (swath, limit)
        && !(1..=limit).contains(&swath)
    {
        return DnpStmtInfo {
            group: Some(group),
            before: vec![format!("Swath parameter illegal, must be 1..{limit}")],
            after: Vec::new(),
            stops_early: true,
            fails_late: false,
        };
    }
    let variation = if full {
        _dnp_long(stmt.args[3].as_deref(), conv, true)
    } else {
        None
    };
    if let Some(v) = variation
        && v < 1
    {
        return DnpStmtInfo {
            group: Some(group),
            before: vec!["Variation must be greater than 0.".to_string()],
            after: Vec::new(),
            stops_early: true,
            fails_late: false,
        };
    }
    let Some(spec) = _dnp_group_spec(group) else {
        return DnpStmtInfo {
            group: Some(group),
            before: vec![format!("Illegal DNP3 group {group}.")],
            after: Vec::new(),
            stops_early: true,
            fails_late: false,
        };
    };
    if let Some(k) = kind
        && _DNP_KNOWN_TYPES.contains(&k)
        && !spec.allowed.contains(&k)
    {
        after.push(format!(
            "For DNP3 group {group} variable must be of type {}.",
            spec.type_name
        ));
    }
    if let (Some(v), Some(top)) = (variation, spec.top)
        && v > top
    {
        after.push(format!(
            "Illegal Variation {v}, max for group {group} is {top}."
        ));
    }
    if !full {
        return DnpStmtInfo {
            group: Some(group),
            before,
            after,
            stops_early: false,
            fails_late: false,
        };
    }
    if let Some(klass) = _dnp_long(stmt.args[4].as_deref(), conv, false) {
        if (1..=3).contains(&klass) && !spec.is_event {
            after.push("Static Data Class: must be 0".to_string());
            return DnpStmtInfo {
                group: Some(group),
                before,
                after,
                stops_early: false,
                fails_late: true,
            };
        }
        if !(0..=3).contains(&klass) {
            after.push("Illegal Class: must be 0..3".to_string());
        }
    }
    if long_flag == Some(false) {
        after.push(_DNP_FLAG_TYPE.to_string());
    }
    if let Some(events) = _dnp_long(stmt.args[7].as_deref(), conv, false) {
        if spec.is_event && events == 0 {
            after.push("Event size must be non-zero to hold events".to_string());
        } else if !spec.is_event && events != 0 {
            after.push(format!("{events} Events allocated for a non-Event object"));
        }
    }
    DnpStmtInfo {
        group: Some(group),
        before,
        after,
        stops_early: false,
        fails_late: false,
    }
}

/// Port of checker/instr_rules.py::check_dnp_update
pub fn check_dnp_update(
    program: &ProgramRef,
    profile: &'static Profile,
    diags: &mut Diagnostics,
    env: Option<&Env>,
) {
    let mut defined = false;
    let mut failed_bases: Vec<i64> = Vec::new();
    let mut assigned: BTreeSet<i64> = BTreeSet::new();
    let ctx = context::of(program, Some(profile), env.cloned());
    let types = ctx.variable_types(false);
    let tables = env.map(|_| ctx.scope_tables(false, false));
    let float_names = longconv::float_names_in(&program.statements);
    let literal_names = longconv::literal_names_in(&program.statements);
    let conv = DnpConv {
        env,
        profile,
        names: &float_names,
        literals: &literal_names,
    };
    let mut info: HashMap<usize, DnpStmtInfo> = HashMap::new();
    let mut typer: Option<var_types::Typer> = None;
    let mut known: Option<&expressions::KnownNames> = None;
    let routine_names: BTreeSet<String> = program
        .statements
        .iter()
        .filter_map(|st| match st.as_ref() {
            Stmt::SubDef(sub) if sub.keyword == "FUNCTION" => Some(pystr::upper(&sub.name)),
            _ => None,
        })
        .collect();
    let empty_dims: OrderedMap<String, Vec<i64>> = OrderedMap::new();

    for st in &program.statements {
        if let Stmt::Instruction(instr) = st.as_ref()
            && pystr::upper(&instr.keyword) == "DNPVARIABLE"
        {
            let src = instr.args.first().and_then(Option::as_deref);
            let kind = match src {
                Some(Expr::Name(n)) => types.get(&pystr::upper(&n.name)).map(String::as_str),
                Some(Expr::Index(ix)) => types.get(&pystr::upper(&ix.name)).map(String::as_str),
                _ => None,
            };
            let dims_map = tables.as_ref().map(|t| t.for_statement(st).dims);
            let dims_ref = dims_map.as_ref().unwrap_or(&empty_dims);
            let mut long_flag = None;
            if instr.args.len() == 8
                && let Some(env_ref) = env
            {
                if typer.is_none() {
                    typer = Some(var_types::Typer::new(program, None, Some(profile)));
                    known = Some(ctx.known_names());
                }
                let flag = instr.args[5].as_deref();
                if let Some(k) = known
                    && expressions::_undefined_names(flag, &_p4_known_view(k)).is_empty()
                    && let Some(ref t) = typer
                {
                    long_flag = _dnp_long_flag(flag, t, env_ref, profile, &routine_names);
                }
            }
            info.insert(
                syntax::rc_id(st),
                _dnp_statement(instr, &conv, dims_ref, kind, long_flag),
            );
        }
    }

    let every_group: BTreeSet<i64> = info
        .values()
        .filter_map(|i| {
            if !(i.stops_early || i.fails_late) && i.group != Some(110) {
                i.group
            } else {
                None
            }
        })
        .collect();

    for stmt in &program.statements {
        if let Stmt::Instruction(instr) = stmt.as_ref() {
            let kw = pystr::upper(&instr.keyword);
            if kw == "DNPVARIABLE" {
                let entry = &info[&syntax::rc_id(stmt)];
                let Some(group) = entry.group else {
                    defined = true;
                    continue;
                };
                for text in &entry.before {
                    diags.error_basic(stmt.line(), text, stmt.file());
                }
                if entry.stops_early {
                    continue;
                }
                if let Some(spec) = _dnp_group_spec(group)
                    && let Some(base) = spec.base
                    && !assigned.contains(&base)
                {
                    diags.error_basic(
                        stmt.line(),
                        &format!("Must assign group {base} before using group {group}."),
                        stmt.file(),
                    );
                    failed_bases.push(base);
                    defined = true;
                    continue;
                }
                for text in &entry.after {
                    diags.error_basic(stmt.line(), text, stmt.file());
                }
                if entry.fails_late {
                    continue;
                }
                assigned.insert(group);
                if group != 110 {
                    defined = true;
                }
            } else if kw == "DNPUPDATE"
                && (!defined || failed_bases.iter().any(|b| !every_group.contains(b)))
            {
                diags.error_basic(
                    stmt.line(),
                    "DNP3Statics and/or DNP3Events are not defined",
                    stmt.file(),
                );
            }
        }
    }
}

/// Port of checker/instr_rules.py::check_portbridge_array
pub fn check_portbridge_array(
    stmt: &Instruction,
    program: &ProgramRef,
    profile: &'static Profile,
    diags: &mut Diagnostics,
    env: &Env,
) {
    if stmt.args.len() != 2 {
        return;
    }
    let Some(stmt_ref) = _p4_find_instr_stmt(program, stmt) else {
        return;
    };
    let ctx = context::of(program, Some(profile), Some(env.clone()));
    let types = ctx.variable_types(false);
    let tables = ctx.scope_tables(false, false);
    let dims = tables.for_statement(stmt_ref).dims;
    let Some(arg) = stmt.args[1].as_deref() else {
        return;
    };
    let bad = match arg {
        Expr::Number(_) => true,
        Expr::Name(n) => {
            let upper = pystr::upper(&n.name);
            let Some(kind) = types.get(&upper) else {
                return;
            };
            let Some(shape) = dims.get(&upper) else {
                return;
            };
            kind != "LONG" || shape.len() < 2 || shape[..2] != [2, 3]
        }
        _ => return,
    };
    if bad {
        diags.error_basic(
            stmt.base.node.line,
            "Bridge parameter must be an Array(2,3) of type long.",
            &stmt.base.node.file,
        );
    }
}

const _CDM_ADDRESS_INSTRUCTIONS: &[&str] = &[
    "CDM_BATTERY",
    "CDM_VOLTSE",
    "CDM_VOLTDIFF",
    "CDM_TCDIFF",
    "CDM_TCSE",
    "CDM_PANELTEMP",
    "CDM_BRHALF",
    "CDM_EXCITEV",
    "CDM_EXCITEI",
    "CDM_SW12",
    "CDM_SW5",
    "CDM_SWPOWER",
    "CDM_DELAY",
    "CDM_PULSEPORT",
    "CDM_MUXSELECT",
    "CDM_THERM107",
    "CDM_RESISTANCE",
    "CDM_ACPOWER",
    "CDM_CURRENTDIFF",
    "CDM_BRFULL6W",
    "CDM_BRHALF3W",
    "CDM_TCCOMP",
    "CDM_PERIODAVG",
];

pub(super) fn _is_cdm_named_type(kind: i64) -> bool {
    (100..106).contains(&kind)
        || (200..=202).contains(&kind)
        || (220..232).contains(&kind)
        || matches!(kind, 260 | 261)
}

const _CDM_ANALOG: &str = "Analog measurement";

pub(super) fn _cdm_null_module(kw: &str) -> Option<&'static [&'static str]> {
    match kw {
        "CDM_VOLTSE" | "CDM_VOLTDIFF" | "CDM_TCSE" | "CDM_PANELTEMP" | "CDM_BRFULL6W"
        | "CDM_BRHALF3W" | "CDM_THERM107" | "CDM_RESISTANCE" => Some(&[_CDM_ANALOG]),
        "CDM_BATTERY" => Some(&["CDM_Battery"]),
        "CDM_DELAY" => Some(&["CDM_Delay"]),
        "CDM_PULSEPORT" => Some(&["CDM_PulsePort"]),
        "CDM_TCCOMP" => Some(&["CDM_TCComp"]),
        "CDM_ACPOWER" => Some(&["CDM_ACPower"]),
        "CDM_PERIODAVG" => Some(&["CDM_PeriodAvg"]),
        "CDM_EXCITEV" | "CDM_EXCITEI" => Some(&["CDM_Excite"]),
        "CDM_MUXSELECT" => Some(&["CDM_MuxSel"]),
        "CDM_SW12" | "CDM_SW5" => Some(&["SW"]),
        "CDM_SWPOWER" => Some(&["CDM_SWPower", "SW"]),
        "CDM_CURRENTDIFF" => Some(&["CDM_CurrentDiff", _CDM_ANALOG]),
        _ => None,
    }
}

fn _cdm_module_name(kind: i64) -> Option<&'static str> {
    match kind {
        100 => Some("EC100"),
        101 => Some("CSAT3B"),
        102 => Some("TGA300"),
        103 => Some("IRGASON2"),
        104 => Some("EC155UNH"),
        105 => Some("IRGASONHP"),
        200 => Some("CDM-VW300"),
        201 => Some("CDM-VW305"),
        202 => Some("VWIRE305"),
        224 => Some("TEMP120"),
        225 => Some("VOLT408V1"),
        226 => Some("VOLT408V10"),
        227 => Some("VOLT408V60"),
        228 => Some("TEMP408J"),
        229 => Some("TEMP408K"),
        230 => Some("TEMP408T"),
        231 => Some("CURRENT408"),
        260 => Some("SPECTRUM103"),
        261 => Some("SPECTRUM109"),
        _ => None,
    }
}

const _CDM_PORT_INSTRUCTIONS: &[&str] = &["CDM_SW12", "CDM_SW5", "CDM_PULSEPORT", "CDM_MUXSELECT"];
pub(super) const _CDM_PORT_MODULES: &[i64] = &[220, 221, 222, 223];

/// Port of checker/instr_rules.py::cdm_unsupported_module
pub fn cdm_unsupported_module(stmt: &Instruction, env: &Env) -> Option<String> {
    let kw = pystr::upper(&stmt.keyword);
    if stmt.args.is_empty()
        || (!_CDM_PORT_INSTRUCTIONS.contains(&kw.as_str()) && kw != "CDM_PANELTEMP")
    {
        return None;
    }
    let val = consteval::value_of(stmt.args[0].as_deref(), env).ok()?;
    let kind = _cdm_type_value(&val)?;
    let supported = if kw == "CDM_PANELTEMP" {
        (220..232).contains(&kind)
    } else {
        _CDM_PORT_MODULES.contains(&kind)
    };
    if supported {
        None
    } else {
        _cdm_module_name(kind).map(str::to_string)
    }
}

/// Port of checker/instr_rules.py::_delay_option
pub fn _delay_option(stmt: &Instruction, env: &Env) -> Option<ConstValue> {
    if stmt.args.len() <= 2 {
        return None;
    }
    consteval::value_of(stmt.args[2].as_deref(), env).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_text;
    use crate::profiles;

    fn parse_prog(src: &str, profile: &'static Profile) -> ProgramRef {
        parse_text(src, "test.crb", None, Some(profile)).0
    }

    fn env_for(prog: &ProgramRef, profile: &'static Profile) -> Env {
        context::of(prog, Some(profile), None).env().clone()
    }

    #[test]
    fn test_emailrecv_destination_flags_non_string() {
        let src = "\
Public X
BeginProg
  EmailRecv(\"a\", \"b\", \"c\", \"d\", \"e\", \"f\", \"g\", \"h\", X)
EndProg
";
        let profile = profiles::get("cr6").unwrap();
        let prog = parse_prog(src, profile);
        let mut diags = Diagnostics::new();
        check_emailrecv_destination(&prog, profile, &mut diags);
        assert_eq!(diags.items.len(), 1);
        assert_eq!(diags.items[0].message, _SERIALIN_DEST_MESSAGE);
    }

    #[test]
    fn test_tcpopen_handle_type_and_connections() {
        let src = "\
Public F As Float
Public H(2) As Long
Public r1, r2
BeginProg
  r1 = TCPOpen(\"\", 3002, 1000, 6000, F, 1)
  r2 = TCPOpen(\"\", 3002, 1000, 6000, H, 5)
EndProg
";
        let profile = profiles::get("cr6").unwrap();
        let prog = parse_prog(src, profile);
        let env = env_for(&prog, profile);
        let mut diags = Diagnostics::new();
        check_tcpopen_handle_type(&prog, profile, &mut diags);
        assert_eq!(diags.items.len(), 1);
        assert_eq!(
            diags.items[0].message,
            "Parameter must be a variable of type long"
        );

        let mut diags2 = Diagnostics::new();
        check_tcpopen_connections(&prog, profile, &mut diags2, &env);
        assert_eq!(diags2.items.len(), 1);
        assert_eq!(
            diags2.items[0].message,
            "Max number of concurrent connections must be 1..2"
        );
    }

    #[test]
    fn test_pulsecount_channels_duplicate_in_scan() {
        let src = "\
Public P(2)
BeginProg
  Scan(1, Sec, 0, 0)
    PulseCount(P(1), 1, C1, 0, 0, 1.0, 0)
    PulseCount(P(2), 1, C1, 0, 0, 1.0, 0)
  NextScan
EndProg
";
        let profile = profiles::get("cr1000x").unwrap();
        let prog = parse_prog(src, profile);
        let mut diags = Diagnostics::new();
        check_pulsecount_channels(&prog, profile, &mut diags);
        assert_eq!(diags.items.len(), 2);
        assert_eq!(
            diags.items[0].message,
            "Pulsecount: channel C1 already in use or unavailable."
        );
        assert_eq!(
            diags.items[1].message,
            "C1 already used as PulseCount in this scan."
        );
    }

    #[test]
    fn test_sdi12_nan_fill_out_of_range() {
        let src = "\
Public Dest(3)
BeginProg
  Scan(1, Sec, 0, 0)
    SDI12Recorder(Dest(2), C1, 0, \"M!\", 1.0, 0, 5)
  NextScan
EndProg
";
        let profile = profiles::get("cr6").unwrap();
        let prog = parse_prog(src, profile);
        let env = env_for(&prog, profile);
        let mut diags = Diagnostics::new();
        let Stmt::Instruction(instr) = prog.statements[3].as_ref() else {
            panic!("expected SDI12Recorder instruction");
        };
        check_sdi12_nan_fill(instr, &prog, profile, &mut diags, &env);
        assert_eq!(diags.items.len(), 1);
        assert_eq!(
            diags.items[0].message,
            "Illegal NAN fill locations, legal is -1 or 1..2(room in the destination array)"
        );
    }

    #[test]
    fn test_terminal_conflicts_serial_and_sdi12() {
        let src = "\
Public D(2)
BeginProg
  SerialOpen(ComC1, 9600, 0, 0, 100)
  Scan(1, Sec, 0, 0)
    SDI12Recorder(D, C1, 0, \"M!\", 1.0, 0)
  NextScan
EndProg
";
        let profile = profiles::get("cr6").unwrap();
        let prog = parse_prog(src, profile);
        let env = env_for(&prog, profile);
        let mut diags = Diagnostics::new();
        check_terminal_conflicts(&prog, profile, &mut diags, &env);
        assert_eq!(diags.items.len(), 1);
        assert_eq!(
            diags.items[0].message,
            "SDI12Recorder: channel C1 already in use or unavailable."
        );
        assert_eq!(diags.items[0].kind, "sdi12_conflict");
    }

    #[test]
    fn test_dnp_update_and_portbridge_and_cdm() {
        let src = "\
Public B As Boolean
Public PB(2, 2) As Long
BeginProg
  DNPUpdate(0, 0)
  DNPVariable(B, 1, 2, 1, 0, 1.5, 0, 0)
  PortBridge(ComC1, PB)
  CDM_SW12(100, 1, 1, 1)
  CDM_Delay(0, 1, 0)
EndProg
";
        let profile = profiles::get("cr6").unwrap();
        let prog = parse_prog(src, profile);
        let env = env_for(&prog, profile);

        let mut dnp_diags = Diagnostics::new();
        check_dnp_update(&prog, profile, &mut dnp_diags, Some(&env));
        assert!(
            dnp_diags
                .items
                .iter()
                .any(|d| d.message == "DNP3Statics and/or DNP3Events are not defined")
        );
        assert!(
            dnp_diags
                .items
                .iter()
                .any(|d| d.message == "Must assign group 1 before using group 2.")
        );

        let mut pb_diags = Diagnostics::new();
        let Stmt::Instruction(pb_instr) = prog.statements[5].as_ref() else {
            panic!("expected PortBridge");
        };
        check_portbridge_array(pb_instr, &prog, profile, &mut pb_diags, &env);
        assert_eq!(pb_diags.items.len(), 1);
        assert_eq!(
            pb_diags.items[0].message,
            "Bridge parameter must be an Array(2,3) of type long."
        );

        let Stmt::Instruction(cdm_instr) = prog.statements[6].as_ref() else {
            panic!("expected CDM_SW12");
        };
        assert_eq!(
            cdm_unsupported_module(cdm_instr, &env),
            Some("EC100".to_string())
        );

        let Stmt::Instruction(delay_instr) = prog.statements[7].as_ref() else {
            panic!("expected CDM_Delay");
        };
        assert!(matches!(
            _delay_option(delay_instr, &env),
            Some(ConstValue::Int(0))
        ));
    }
}
