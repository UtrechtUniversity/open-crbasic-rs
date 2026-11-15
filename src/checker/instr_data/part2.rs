//! Port of `checker/instr_data.py` (lines 760-1473).

#![allow(dead_code, unused_variables, unused_imports)]
#![allow(clippy::used_underscore_items)]

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;
use std::sync::LazyLock;

use crate::checker::context::{self, ScopeTables};
use crate::checker::{expressions, symbols};
use crate::consteval::{self, Env};
use crate::diagnostics::{DiagRef, Diagnostic, Diagnostics, ShowFile};
use crate::profiles::Profile;
use crate::syntax::{
    Assignment, Call, Case, ConstDecl, ConstValue, Declaration, Expr, ExprRef, Index, Instruction,
    Juxtaposed, Member, Name, Number, OrderedMap, Program, ProgramRef, Stmt, StmtRef, SubDef,
    Unparsed, VarDecl, ends_routine,
};
use crate::{pyre, pystr};

/// Port of checker/instr_data.py::check_units_targets
pub fn check_units_targets(program: &Program, _profile: &Profile, diags: &mut Diagnostics) {
    let struct_types: BTreeSet<_> = program
        .statements
        .iter()
        .filter_map(|st| match st.as_ref() {
            Stmt::Instruction(st)
                if matches!(
                    pystr::upper(&st.keyword).as_str(),
                    "STRUCTURETYPE" | "OBJECTTYPE"
                ) =>
            {
                match st.args.first()?.as_deref()? {
                    Expr::Name(name) => Some(pystr::upper(&name.name)),
                    _ => None,
                }
            }
            _ => None,
        })
        .collect();
    let mut allowed = BTreeSet::new();
    for st in &program.statements {
        match st.as_ref() {
            Stmt::Declaration(decl)
                if matches!(decl.kind.as_str(), "PUBLIC" | "DIM" | "STATIC") =>
            {
                allowed.extend(
                    decl.vars
                        .borrow()
                        .iter()
                        .filter(|v| v.type_.as_ref().is_none_or(|ty| !struct_types.contains(ty)))
                        .map(|v| pystr::upper(&v.name)),
                );
            }
            Stmt::Instruction(st) if st.keyword.eq_ignore_ascii_case("ALIAS") => {
                allowed.extend(
                    st.args
                        .iter()
                        .skip(1)
                        .filter_map(|arg| match arg.as_deref() {
                            Some(Expr::Name(name)) => Some(pystr::upper(&name.name)),
                            _ => None,
                        }),
                );
            }
            _ => {}
        }
    }

    let mut in_struct = false;
    let mut assigned: Vec<(String, Vec<(&Instruction, &str)>)> = Vec::new();
    let mut indexed = BTreeSet::new();
    let mut invalid_target = |st: &Instruction, name: &str| {
        diags.error_basic(
            st.base.node.line,
            &format!(
                "Can't assign units to {name}, it must be declared Public or referenced as a source in a Data Table."
            ),
            &st.base.node.file,
        );
    };
    for st in &program.statements {
        let Stmt::Instruction(st) = st.as_ref() else {
            continue;
        };
        let kw = pystr::upper(&st.keyword);
        match kw.as_str() {
            "STRUCTURETYPE" | "OBJECTTYPE" => in_struct = true,
            "ENDSTRUCTURETYPE" | "ENDOBJECTTYPE" => in_struct = false,
            _ => {}
        }
        if in_struct || kw != "UNITS" || st.args.is_empty() {
            continue;
        }
        for extra in &st.extra_targets {
            let name = extra.name();
            if !allowed.contains(&pystr::upper(name)) {
                invalid_target(st, name);
            }
        }
        let (name, is_indexed) = match st.args.first().and_then(Option::as_deref) {
            Some(Expr::Name(name)) => (name.name.as_str(), false),
            Some(Expr::Index(index)) => (index.name.as_str(), true),
            _ => continue,
        };
        let key = pystr::upper(name);
        if !allowed.contains(&key) {
            invalid_target(st, name);
        } else if is_indexed {
            indexed.insert(key);
        } else if let Some((_, statements)) = assigned.iter_mut().find(|(k, _)| *k == key) {
            statements.push((st, name));
        } else {
            assigned.push((key, vec![(st, name)]));
        }
    }

    for (key, statements) in assigned {
        if statements.len() < 2 || indexed.contains(&key) {
            continue;
        }
        for (st, name) in &statements {
            let Some(Expr::String(units)) = st.args.get(1).and_then(Option::as_deref) else {
                continue;
            };
            for _ in 1..statements.len() {
                diags.error_basic(
                    st.base.node.line,
                    &format!("{name} already assigned Units {}.", units.value),
                    &st.base.node.file,
                );
            }
        }
    }
}

const _SEQUENCE_OPENERS: &[&str] = &["DIALSEQUENCE", "MODEMHANGUP"];
const _SEQUENCE_CLOSERS: &[(&str, &str, Option<&str>)] = &[
    (
        "ENDDIALSEQUENCE",
        "Missing DialSequence().",
        Some("DIALSEQUENCE"),
    ),
    (
        "ENDMODEMHANGUP",
        "Missing ModemHangup().",
        Some("MODEMHANGUP"),
    ),
    ("ENDVOICE", "EndVoice without Voice", None),
];

fn _lookup_const_name(name: &str, env: &Env) -> Result<ConstValue, consteval::NotConstant> {
    let probe = Expr::Name(Name {
        name: name.to_owned(),
        ..Name::default()
    });
    consteval::value_of(Some(&probe), env)
}

fn _mod_u32_decimal(token: &str) -> Option<u64> {
    let (neg, digits) = if let Some(rest) = token.strip_prefix('-') {
        (true, rest)
    } else if let Some(rest) = token.strip_prefix('+') {
        (false, rest)
    } else {
        (false, token)
    };
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let modulus = 1u64 << 32;
    let mut rem = 0u64;
    for b in digits.bytes() {
        rem = (rem * 10 + u64::from(b - b'0')) % modulus;
    }
    if neg && rem != 0 {
        rem = modulus - rem;
    }
    Some(rem)
}

/// Port of checker/instr_data.py::check_table_and_menu_placement
pub fn check_table_and_menu_placement(
    program: &ProgramRef,
    profile: &'static Profile,
    diags: &mut Diagnostics,
    env: Option<&Env>,
    known_names: Option<&BTreeSet<String>>,
) -> Option<(String, i64)> {
    let mut in_table = false;
    let mut intervals = 0i64;
    let mut has_maxmin = false;
    let mut menus = 0i64;
    let mut submenus = 0i64;
    let mut open_list = false;
    let mut levels: Vec<Vec<String>> = Vec::new();
    let mut table_consts: BTreeSet<String> = BTreeSet::new();
    let mut in_const_table = false;
    for st in &program.statements {
        match st.as_ref() {
            Stmt::Instruction(inst) => {
                let kw = pystr::upper(&inst.keyword);
                if kw == "CONSTTABLE" || kw == "ENDCONSTTABLE" {
                    in_const_table = kw == "CONSTTABLE";
                }
            }
            Stmt::ConstDecl(c) if in_const_table => {
                table_consts.insert(pystr::upper(&c.name));
            }
            _ => {}
        }
    }
    let mut item_const: Option<String> = None;
    let mut item_unpickable = false;
    let mut recompile_invalid = false;
    let has_const_table = program.statements.iter().any(|st| {
        matches!(
            st.as_ref(),
            Stmt::Instruction(inst) if pystr::upper(&inst.keyword) == "CONSTTABLE"
        )
    });
    let mut item_bool = false;
    let mut item_kind: Option<&'static str> = None;
    let mut first_is_const = false;
    let mut sequences: OrderedMap<String, i64> = OrderedMap::new();
    let ctx = context::of(program, Some(profile), None);
    let variables = ctx.declared_variable_names();
    let types = ctx.variable_types(false);
    let mut menu_names_owned: Option<BTreeSet<String>> = None;
    let owned_env = if env.is_none() {
        Some(Env::with_consts(
            profile,
            Rc::new(RefCell::new(program.consts.clone())),
        ))
    } else {
        None
    };
    let env = env.or(owned_env.as_ref()).expect("env initialized");

    let within =
        |st: &Instruction, kw: &str, menus: i64, open_list: bool, diags: &mut Diagnostics| {
            if menus != 0 && open_list {
                return true;
            }
            let instr_name = profile
                .instructions()
                .get(kw)
                .map_or(kw, |r| r.name.as_str());
            let kind = if kw == "MENURECOMPILE" {
                "placement_menu_recompile"
            } else {
                "placement_menu"
            };
            diags.error(
                st.base.node.line,
                &format!("{instr_name} must be used within DisplayMenu..EndMenu."),
                &st.base.node.file,
                ShowFile::Bool(true),
                0,
                kind,
                "",
            );
            false
        };

    for st in &program.statements {
        let Stmt::Instruction(st) = st.as_ref() else {
            continue;
        };
        let kw = pystr::upper(&st.keyword);
        if kw == "DATATABLE" {
            in_table = true;
            intervals = 0;
            has_maxmin = false;
            if !profile.single_pass {
                diags.items.extend(super::part3::_illegal_table_name(st));
            }
        } else if kw == "ENDTABLE" {
            in_table = false;
        } else if in_table && (kw == "MAXIMUM" || kw == "MINIMUM") {
            has_maxmin = true;
        } else if in_table && kw == "SAMPLEMAXMIN" && !has_maxmin {
            if let Some(rec) = profile.instructions().get(&kw) {
                let need = crate::checker::instr_rules::_real_params(rec.params.clone()).len();
                if profile.single_pass
                    && (st.args.len() < need
                        || (need > 0 && st.args[need - 1].is_none())
                        || st.args[need..].iter().any(Option::is_some))
                {
                    continue;
                }
            }
            diags.error_basic(
                st.base.node.line,
                "Maximum() or Minimum() must precede SampleMaxMin().",
                &st.base.node.file,
            );
        } else if kw == "DATAINTERVAL" && in_table {
            intervals += 1;
            if intervals == 2 {
                diags.error_basic(
                    st.base.node.line,
                    "Only one DataInterval allowed within DataTable..EndTable.",
                    &st.base.node.file,
                );
            }
        } else if profile.instructions().get(&kw).is_none() {
        } else if !profile.single_pass
            && let Some((_, message, opener)) = _SEQUENCE_CLOSERS.iter().find(|(k, _, _)| *k == kw)
        {
            if let Some(op) = opener
                && let Some(cnt) = sequences.get(*op).copied()
                && cnt != 0
            {
                sequences.insert((*op).to_owned(), cnt - 1);
            } else {
                diags.error_basic(st.base.node.line, message, &st.base.node.file);
            }
        } else if !profile.single_pass && _SEQUENCE_OPENERS.contains(&kw.as_str()) {
            let cur = sequences.get(&kw).copied().unwrap_or(0);
            sequences.insert(kw, cur + 1);
        } else if kw == "DISPLAYMENU" && menus != 0 {
            diags.error_basic(
                st.base.node.line,
                "DisplayMenu without EndMenu.",
                &st.base.node.file,
            );
        } else if kw == "DISPLAYMENU" {
            menus += 1;
            levels.push(Vec::new());
            open_list = true;
        } else if _MENU_ITEMS.contains(&kw.as_str()) {
            if within(st, &kw, menus, open_list, diags) {
                if let Some(top) = levels.last_mut() {
                    top.push(kw.clone());
                }
                let target = if kw == "MENUITEM" || kw == "MENURECOMPILE" {
                    st.args.get(1).and_then(Option::as_deref)
                } else {
                    None
                };
                item_const = match target {
                    Some(Expr::Name(n)) if kw == "MENUITEM" => {
                        let up = pystr::upper(&n.name);
                        table_consts.contains(&up).then_some(up)
                    }
                    _ => None,
                };
                let constant_name = match target {
                    Some(Expr::Name(n)) => {
                        let up = pystr::upper(&n.name);
                        !variables.contains(&up)
                            && (program.consts.contains_key(&up)
                                || profile.symbols().get(&up).is_some())
                    }
                    _ => false,
                };
                let mut missing_name = false;
                if kw == "MENUITEM"
                    && let Some(Expr::Name(n)) = target
                    && !constant_name
                    && item_const.is_none()
                {
                    let menu_names = match known_names {
                        Some(kn) => kn,
                        None => menu_names_owned.get_or_insert_with(|| {
                            context::of(program, Some(profile), None)
                                .known_names()
                                .names
                                .clone()
                        }),
                    };
                    missing_name = !menu_names.contains(&pystr::upper(&n.name));
                }
                item_unpickable = kw == "MENUITEM"
                    && item_const.is_none()
                    && (constant_name
                        || missing_name
                        || matches!(
                            target,
                            Some(
                                Expr::Number(_)
                                    | Expr::String(_)
                                    | Expr::Unary(_)
                                    | Expr::Binary(_)
                            )
                        ));
                recompile_invalid = false;
                let target_name = match target {
                    Some(Expr::Name(n)) => Some(n.name.as_str()),
                    Some(Expr::Index(idx)) => Some(idx.name.as_str()),
                    _ => None,
                };
                item_bool = target_name
                    .and_then(|n| types.get(&pystr::upper(n)))
                    .map(String::as_str)
                    == Some("BOOLEAN");
                item_kind = None;
                if kw == "MENURECOMPILE" {
                    let kind = target_name
                        .and_then(|n| types.get(&pystr::upper(n)))
                        .map(String::as_str);
                    if matches!(target, Some(Expr::String(_))) || kind == Some("STRING") {
                        diags.error_basic(
                            st.base.node.line,
                            "String type variable not allowed.",
                            &st.base.node.file,
                        );
                        item_unpickable = matches!(target, Some(Expr::String(_)));
                        recompile_invalid = !item_unpickable;
                    } else if constant_name || matches!(target, Some(Expr::Number(_))) {
                        let raw = expressions::_arg_text(st, 1);
                        let txt = pystr::strip(&raw);
                        diags.error_basic(
                            st.base.node.line,
                            &format!("Invalid Variable or Array: {txt}."),
                            &st.base.node.file,
                        );
                        recompile_invalid = true;
                    } else if kind == Some("FLOAT") {
                        item_kind = Some("float");
                    }
                }
                if kw == "MENUITEM" && target_name.is_some() {
                    if let Some(ref ic) = item_const {
                        item_kind = match env.consts.borrow().get(ic) {
                            Some(ConstValue::Str(_)) => Some("string"),
                            Some(ConstValue::Int(_) | ConstValue::BigInt(_)) => Some("int"),
                            Some(ConstValue::Float(f) | ConstValue::CFloat(f))
                                if !f.is_finite() || !pystr::is_integer(*f) =>
                            {
                                Some("float")
                            }
                            _ => None,
                        };
                    } else if let Some(tname) = target_name {
                        item_kind = match types.get(&pystr::upper(tname)).map(String::as_str) {
                            Some("LONG") => Some("int"),
                            Some("FLOAT") => Some("float"),
                            Some("STRING") => Some("string"),
                            _ => None,
                        };
                    }
                }
                first_is_const = false;
                if kw == "SUBMENU" {
                    levels.push(Vec::new());
                    submenus += 1;
                }
            }
        } else if kw == "MENUPICK" {
            if within(st, &kw, menus, open_list, diags) {
                let last_kw = levels.last().and_then(|l| l.last()).map(String::as_str);
                let after_item = last_kw == Some("MENUITEM");
                if !matches!(last_kw, Some("MENUITEM" | "MENURECOMPILE")) {
                    diags.error_basic(
                        st.base.node.line,
                        "MenuPick must follow a MenuItem instruction.",
                        &st.base.node.file,
                    );
                } else if item_unpickable {
                    let msg = if has_const_table {
                        "When using a ConstTable constant in MenuPick, the constant name must be the first parameter."
                    } else {
                        "Value in preceding MenuItem must be a variable or a Const within ConstTable..EndConstTable."
                    };
                    diags.error_basic(st.base.node.line, msg, &st.base.node.file);
                    continue;
                } else if recompile_invalid {
                    continue;
                } else if after_item && item_const.is_some() {
                    let first = st.args.first().and_then(Option::as_deref);
                    first_is_const = matches!(
                        first,
                        Some(Expr::Name(n)) if table_consts.contains(&pystr::upper(&n.name))
                    );
                    if first_is_const {
                        if let Some(top) = levels.last_mut() {
                            top.push(kw.clone());
                        }
                    } else {
                        diags.error_basic(
                            st.base.node.line,
                            "When using a ConstTable constant in MenuPick, the constant name must be the first parameter.",
                            &st.base.node.file,
                        );
                    }
                }
                let bad = st.args.iter().find_map(|a| match a.as_deref() {
                    Some(Expr::Name(n)) if variables.contains(&pystr::upper(&n.name)) => Some(n),
                    _ => None,
                });
                let cur_last = levels.last().and_then(|l| l.last()).map(String::as_str);
                if item_bool && matches!(cur_last, Some("MENUITEM" | "MENURECOMPILE")) {
                    _check_boolean_picklist(st, env, diags);
                } else if (after_item || cur_last == Some("MENURECOMPILE"))
                    && (item_kind.is_some() || item_const.is_some())
                {
                    let wrong_first = item_const.is_some() && !first_is_const;
                    let eff_kind = if wrong_first {
                        Some("float")
                    } else {
                        item_kind
                    };
                    _check_pick_consts(
                        st,
                        eff_kind,
                        env,
                        program,
                        profile,
                        variables,
                        diags,
                        item_const.is_some() && first_is_const,
                        known_names,
                    );
                } else if let Some(bad_name) = bad {
                    diags.error_basic(
                        st.base.node.line,
                        &format!("{} must be declared as a Const.", bad_name.name),
                        &st.base.node.file,
                    );
                }
            }
        } else if kw == "ENDSUBMENU" {
            if submenus == 0 {
                diags.error_basic(
                    st.base.node.line,
                    "EndSubMenu without SubMenu.",
                    &st.base.node.file,
                );
                return Some((st.base.node.file.clone(), st.base.node.line));
            }
            submenus -= 1;
            if within(st, &kw, menus, open_list, diags)
                && levels.pop().is_some_and(|l| l.is_empty())
            {
                diags.error_basic(
                    st.base.node.line,
                    "Empty SubMenu ... EndSubMenu.",
                    &st.base.node.file,
                );
                open_list = false;
            }
        } else if kw == "ENDMENU" && within(st, &kw, menus, open_list, diags) {
            menus -= 1;
            if levels.pop().is_some_and(|l| l.is_empty()) {
                diags.error_basic(
                    st.base.node.line,
                    "Empty DisplayMenu .. EndMenu.",
                    &st.base.node.file,
                );
            }
            if submenus != 0 {
                diags.error_basic(
                    st.base.node.line,
                    "SubMenu with no EndSubMenu.",
                    &st.base.node.file,
                );
            }
        }
    }
    None
}

static _BOOL_NAME_RE: LazyLock<pyre::Regex> =
    LazyLock::new(|| pyre::Regex::new(r"[A-Za-z_][A-Za-z0-9_]*\s*$", 0));
static _BOOL_INT_RE: LazyLock<pyre::Regex> = LazyLock::new(|| pyre::Regex::new(r"[+-]?\d+$", 0));

/// Port of checker/instr_data.py::_boolean_pick_ok
pub fn _boolean_pick_ok(token: &str, env: &Env) -> bool {
    static HEX_RE: LazyLock<pyre::Regex> = LazyLock::new(|| pyre::Regex::new(r"[0-9A-Fa-f]+", 0));
    static BIN_RE: LazyLock<pyre::Regex> = LazyLock::new(|| pyre::Regex::new(r"[01]+", 0));

    let token = token.trim_start_matches([' ', '\t']);
    if _BOOL_NAME_RE.match_(token).is_some() {
        let Ok(value) = _lookup_const_name(pystr::strip(token), env) else {
            return false;
        };
        #[allow(clippy::float_cmp)]
        return match value {
            ConstValue::Int(i) => i == 0 || i == -1,
            ConstValue::Float(f) | ConstValue::CFloat(f) => f == 0.0 || f == -1.0,
            ConstValue::BigInt(_) | ConstValue::Str(_) => false,
        };
    }
    if _BOOL_INT_RE.match_(token).is_some() {
        return _mod_u32_decimal(token).is_some_and(|m| m == 0 || m == (1u64 << 32) - 1);
    }
    let prefix = pystr::upper(pystr::slice(token, None, Some(2)));
    let body = pystr::slice(token, Some(2), None);
    let (digits, radix) = if prefix == "&H" && HEX_RE.fullmatch(body).is_some() {
        (8usize, 16u32)
    } else if prefix == "&B" && BIN_RE.fullmatch(body).is_some() {
        (32usize, 2u32)
    } else {
        return false;
    };
    if pystr::len(body.trim_start_matches('0')) > digits {
        return true;
    }
    u64::from_str_radix(body, radix).is_ok_and(|v| v == 0 || v == 0xFFFF_FFFF)
}

/// Port of checker/instr_data.py::_pick_tokens
pub fn _pick_tokens(stmt: &Instruction) -> Option<Vec<String>> {
    static MENU_PICK: LazyLock<pyre::Regex> =
        LazyLock::new(|| pyre::Regex::new(r"\s*MenuPick\b(.*)$", pyre::I | pyre::S));

    let matched = MENU_PICK.match_(&stmt.base.text)?;
    let mut rest = String::new();
    let mut quoted = false;
    for ch in matched.group(1)?.chars() {
        if ch == '"' {
            quoted = !quoted;
        } else if ch == '\'' && !quoted {
            break;
        }
        rest.push(ch);
    }
    let mut rest = pystr::strip(&rest);
    if rest.starts_with('(') && rest.ends_with(')') {
        rest = pystr::slice(rest, Some(1), Some(-1));
    }
    Some(rest.split(',').map(str::to_owned).collect())
}

/// Port of checker/instr_data.py::_check_boolean_picklist
pub fn _check_boolean_picklist(stmt: &Instruction, env: &Env, diags: &mut Diagnostics) {
    let Some(tokens) = _pick_tokens(stmt) else {
        return;
    };
    let trailing = tokens.len() > 1 && tokens.last().is_some_and(String::is_empty);
    let check_slice = if trailing {
        &tokens[..tokens.len() - 1]
    } else {
        &tokens[..]
    };
    for token in check_slice {
        if !_boolean_pick_ok(token, env) {
            diags.error_basic(
                stmt.base.node.line,
                "Boolean picklist must be True or False.",
                &stmt.base.node.file,
            );
        }
    }
    if trailing {
        diags.error_basic(
            stmt.base.node.line,
            "Unable to parse MenuPick strings.",
            &stmt.base.node.file,
        );
    }
}

static _PICK_INT_RE: LazyLock<pyre::Regex> = LazyLock::new(|| pyre::Regex::new(r"[+-]?\d+$", 0));
static _PICK_FLOAT_RE: LazyLock<pyre::Regex> = LazyLock::new(|| {
    pyre::Regex::new(
        r"[+-]?(\d+\.\d*|\.\d+)([eE][+-]?\d+)?$|[+-]?\d+[eE][+-]?\d+$",
        0,
    )
});
static _PICK_RADIX_RE: LazyLock<pyre::Regex> =
    LazyLock::new(|| pyre::Regex::new(r"&[Hh][0-9A-Fa-f]+$|&[Bb][01]+$", 0));
static _PICK_NAME_RE: LazyLock<pyre::Regex> =
    LazyLock::new(|| pyre::Regex::new(r"[A-Za-z_]\w*$", 0));

/// Port of checker/instr_data.py::_pick_value_bad
pub fn _pick_value_bad(
    token: &str,
    kind: &str,
    env: &Env,
    program: &ProgramRef,
    profile: &'static Profile,
    variables: &BTreeSet<String>,
    known_names: Option<&BTreeSet<String>>,
) -> Option<String> {
    static EXPR_CHARS_RE: LazyLock<pyre::Regex> =
        LazyLock::new(|| pyre::Regex::new(r"[A-Za-z0-9_.&+\-*/ ]+", 0));
    static EXPR_OP_RE: LazyLock<pyre::Regex> =
        LazyLock::new(|| pyre::Regex::new(r"[A-Za-z0-9_.)]\s*[-+*/]", 0));
    static NEG_NAME_RE: LazyLock<pyre::Regex> =
        LazyLock::new(|| pyre::Regex::new(r"-([A-Za-z_]\w*|&[Hh][0-9A-Fa-f]+)", 0));

    let t = token.trim_start_matches([' ', '\t']);
    if t.is_empty() {
        return if kind == "int" {
            Some(String::new())
        } else {
            None
        };
    }
    if t != pystr::rstrip(t) {
        return Some(t.to_owned());
    }
    let mut inner = t;
    if t.starts_with('(') && t.ends_with(')') {
        inner = pystr::slice(t, Some(1), Some(-1));
    }
    if inner.contains('(') || inner.contains(')') || inner.contains(',') {
        return None;
    }
    if inner.starts_with('"')
        && inner.ends_with('"')
        && pystr::len(inner) >= 2
        && inner.matches('"').count() == 2
    {
        return Some(inner.to_owned());
    }
    if _PICK_INT_RE.match_(inner).is_some() || _PICK_RADIX_RE.match_(inner).is_some() {
        return None;
    }
    if _PICK_FLOAT_RE.match_(inner).is_some() {
        return if kind == "int" {
            Some(inner.to_owned())
        } else {
            None
        };
    }
    if _PICK_NAME_RE.match_(inner).is_some() {
        match _lookup_const_name(inner, env) {
            Err(_) => {
                let up = pystr::upper(inner);
                let fallback_kn;
                let kn = if let Some(kn) = known_names {
                    kn
                } else {
                    fallback_kn = context::of(program, Some(profile), None)
                        .known_names()
                        .names
                        .clone();
                    &fallback_kn
                };
                if variables.contains(&up) || !kn.contains(&up) || (kind == "int" && up == "NAN") {
                    return Some(inner.to_owned());
                }
                return None;
            }
            Ok(value) => {
                let bad_int = match value {
                    ConstValue::Str(_) => true,
                    ConstValue::Int(_) | ConstValue::BigInt(_) => false,
                    ConstValue::Float(f) | ConstValue::CFloat(f) => {
                        !f.is_finite() || !pystr::is_integer(f)
                    }
                };
                if kind == "int" && bad_int {
                    return Some(inner.to_owned());
                }
                return None;
            }
        }
    }
    if EXPR_CHARS_RE.fullmatch(inner).is_some() && EXPR_OP_RE.search(inner).is_some() {
        return Some(inner.to_owned());
    }
    if NEG_NAME_RE.fullmatch(inner).is_some() {
        return Some(inner.to_owned());
    }
    None
}

/// Port of checker/instr_data.py::_check_pick_consts
#[allow(clippy::too_many_arguments)]
pub fn _check_pick_consts(
    stmt: &Instruction,
    kind: Option<&str>,
    env: &Env,
    program: &ProgramRef,
    profile: &'static Profile,
    variables: &BTreeSet<String>,
    diags: &mut Diagnostics,
    skip_first: bool,
    known_names: Option<&BTreeSet<String>>,
) {
    let Some(kind) = kind else {
        return;
    };
    if kind == "string" {
        return;
    }
    let Some(tokens) = _pick_tokens(stmt) else {
        return;
    };
    let word = if kind == "int" {
        "integer Const"
    } else {
        "Const"
    };
    let iter = if skip_first && !tokens.is_empty() {
        &tokens[1..]
    } else {
        &tokens[..]
    };
    for token in iter {
        if let Some(bad) =
            _pick_value_bad(token, kind, env, program, profile, variables, known_names)
        {
            diags.error_basic(
                stmt.base.node.line,
                &format!("{bad} must be declared as a {word}."),
                &stmt.base.node.file,
            );
        }
    }
}

const _MENU_ITEMS: &[&str] = &[
    "SUBMENU",
    "MENUITEM",
    "DISPLAYVALUE",
    "MENURECOMPILE",
    "DISPLAYLINE",
];

pub(crate) const _TABLE_OUTPUT: &[&str] = &[
    "TOTALIZE",
    "AVERAGE",
    "MAXIMUM",
    "MINIMUM",
    "SAMPLEMAXMIN",
    "STDDEV",
    "MOMENT",
    "SAMPLE",
    "COVARIANCE",
    "MEDIAN",
];
const _OWN_VARIABLE_CHECK: &[&str] = &["ADDPRECISE", "MOVEPRECISE"];
const _INT32_MIN: i64 = -2_147_483_648;

/// Port of checker/instr_data.py::_destination_expression
pub fn _destination_expression(expr: Option<&Expr>) -> bool {
    let Some(expr) = expr else {
        return false;
    };
    match expr {
        Expr::Number(_) | Expr::String(_) | Expr::Name(_) => true,
        Expr::Unary(u) => _destination_expression(u.operand.as_deref()),
        Expr::Binary(b) => {
            _destination_expression(b.left.as_deref())
                && _destination_expression(b.right.as_deref())
        }
        Expr::Index(_) | Expr::Member(_) | Expr::ArrayLiteral(_) | Expr::Juxtaposed(_) => false,
    }
}

/// Port of checker/instr_data.py::constant_destination
pub fn constant_destination(expr: Option<&Expr>, env: &Env, variables: &BTreeSet<String>) -> bool {
    let Some(expr) = expr else {
        return false;
    };
    if !_destination_expression(Some(expr)) {
        return false;
    }
    let names: BTreeSet<String> = symbols::walk_names(Some(expr))
        .into_iter()
        .map(|n| pystr::upper(&n))
        .collect();
    if names.iter().any(|n| variables.contains(n)) {
        return false;
    }
    match consteval::value_of(Some(expr), env) {
        Ok(val) => !matches!(val, ConstValue::Str(_)),
        Err(_) => names.is_empty() && !matches!(expr, Expr::String(_)),
    }
}

/// Port of checker/instr_data.py::_destination_env
pub fn _destination_env(env: &Env) -> Env {
    let mut consts = OrderedMap::new();
    consts.insert("NAN".to_owned(), ConstValue::Float(f64::NAN));
    consts.insert("INF".to_owned(), ConstValue::Float(f64::INFINITY));
    for (k, v) in env.consts.borrow().iter() {
        consts.insert(k.clone(), v.clone());
    }
    Env {
        profile: env.profile,
        consts: Rc::new(RefCell::new(consts)),
        section: env.section.clone(),
    }
}

/// Port of checker/instr_data.py::_reps_value
pub fn _reps_value(expr: Option<&Expr>, env: &Env) -> Option<i64> {
    if matches!(expr, Some(Expr::String(_))) {
        return Some(_INT32_MIN);
    }
    let value = consteval::value_of(expr, env).ok()?;
    let n = match value {
        ConstValue::Int(i) => i,
        ConstValue::Float(f) | ConstValue::CFloat(f) => {
            if !f.is_finite() {
                return Some(_INT32_MIN);
            }
            pystr::py_int_from_f64(f).unwrap_or(_INT32_MIN)
        }
        ConstValue::BigInt(_) => return Some(_INT32_MIN),
        ConstValue::Str(_) => return None,
    };
    if (-2_147_483_648..2_147_483_648).contains(&n) {
        Some(n)
    } else {
        Some(_INT32_MIN)
    }
}

/// Port of checker/instr_data.py::check_stage2a
pub fn check_stage2a(
    program: &ProgramRef,
    profile: &'static Profile,
    diags: &mut Diagnostics,
    env: &Env,
) -> bool {
    if profile.name != "cr6" && profile.name != "cr1000x" {
        return false;
    }

    super::part3::_check_stage2a(program, profile, diags, env, true)
}

/// Port of checker/instr_data.py::check_table_reps_late
pub fn check_table_reps_late(
    program: &ProgramRef,
    profile: &'static Profile,
    diags: &mut Diagnostics,
    env: &Env,
) {
    if profile.name == "cr300" || profile.name == "cr350" {
        super::part3::_check_stage2a(program, profile, diags, env, false);
    }
}

/// Port of checker/instr_data.py::_report_undefined_source
pub fn _report_undefined_source(
    st: &Instruction,
    record: &crate::profiles::Instruction,
    known: &expressions::KnownNames,
    profile: &Profile,
    diags: &mut Diagnostics,
) -> bool {
    let kinds = expressions::_type_code_kinds();
    let limit = record.params.len().min(st.args.len());
    for (i, &code) in record.params[..limit].iter().enumerate() {
        let Some(arg) = st.args[i].as_deref() else {
            continue;
        };
        let key = usize::try_from(if code >= 100 { code % 100 } else { code }).ok();
        if key.and_then(|k| kinds.get(&k)).map(String::as_str) != Some("deferred_text") {
            continue;
        }
        let names = expressions::_undefined_names(
            Some(arg),
            &crate::checker::expressions::KnownView::from(known),
        )
        .into_iter()
        .map(crate::checker::expressions::UndefinedName::into_string)
        .collect::<Vec<_>>();
        if names.is_empty() {
            continue;
        }
        for name in names {
            diags.error(
                st.base.node.line,
                &profile.unresolved_symbol_message.format(&name),
                &st.base.node.file,
                ShowFile::Bool(false),
                0,
                "",
                "",
            );
        }
        if expressions::_has_undefined_index(st.args[i].as_ref(), known, None, None, &[]) {
            let idx = i64::try_from(i).unwrap_or(0);
            let raw = expressions::_arg_text(st, idx);
            let txt = pystr::strip(&raw);
            diags.error_basic(
                st.base.node.line,
                &expressions::dimensions_message(txt, profile, false, false),
                &st.base.node.file,
            );
        }
        return true;
    }
    false
}

/// Port of checker/instr_data.py::_is_struct_typed
pub fn _is_struct_typed(program: &Program, name: &str) -> bool {
    let types: BTreeSet<String> = program
        .statements
        .iter()
        .filter_map(|st| match st.as_ref() {
            Stmt::Instruction(inst) if inst.keyword.eq_ignore_ascii_case("STRUCTURETYPE") => {
                match inst.args.first().and_then(Option::as_deref) {
                    Some(Expr::Name(n)) => Some(pystr::upper(&n.name)),
                    _ => None,
                }
            }
            _ => None,
        })
        .collect();
    let target = pystr::upper(name);
    program.statements.iter().any(|st| {
        let Stmt::Declaration(d) = st.as_ref() else {
            return false;
        };
        d.vars.borrow().iter().any(|v| {
            pystr::upper(&v.name) == target
                && v.type_
                    .as_deref()
                    .is_some_and(|ty| types.contains(&pystr::upper(ty)))
        })
    })
}

/// Port of checker/instr_data.py::_short_array
pub fn _short_array(
    arg: Option<&Expr>,
    reps: i64,
    dims: &OrderedMap<String, Vec<i64>>,
    env: &Env,
) -> bool {
    let (name, index_args) = match arg {
        Some(Expr::Name(n)) => (n.name.as_str(), None),
        Some(Expr::Index(idx)) => (idx.name.as_str(), Some(&idx.args)),
        _ => return false,
    };
    let Some(shape) = dims.get(&pystr::upper(name)) else {
        return false;
    };
    let mut offset = 0i64;
    if let Some(args) = index_args
        && !args.is_empty()
    {
        if args.len() != 1 || shape.len() != 1 {
            return false;
        }
        let Some(sub) = _reps_value(args[0].as_deref(), env) else {
            return false;
        };
        offset = sub - 1;
    }
    let total: i64 = shape.iter().product();
    offset + reps > total
}

/// Port of checker/instr_data.py::_alias_sources
pub fn _alias_sources(
    program: &Program,
    tables: &dyn Fn(&StmtRef) -> ScopeTables,
    env: &Env,
) -> OrderedMap<String, (String, String, String, i64, i64)> {
    let mut out = OrderedMap::new();
    let mut declared = OrderedMap::new();
    for st in &program.statements {
        if let Stmt::Declaration(d) = st.as_ref() {
            for v in d.vars.borrow().iter() {
                declared.insert(pystr::upper(&v.name), v.name.clone());
            }
        }
    }
    let mut depth = 0i64;
    for st_ref in &program.statements {
        if matches!(st_ref.as_ref(), Stmt::SubDef(_)) {
            depth += 1;
        } else if ends_routine(st_ref.as_ref()) {
            depth -= 1;
        }
        if depth != 0 {
            continue;
        }
        let Stmt::Instruction(st) = st_ref.as_ref() else {
            continue;
        };
        if !st.keyword.eq_ignore_ascii_case("ALIAS") || st.args.len() < 2 {
            continue;
        }
        let (target_name, target_args) = match st.args.first().and_then(Option::as_deref) {
            Some(Expr::Name(n)) => (n.name.as_str(), None),
            Some(Expr::Index(idx)) => (idx.name.as_str(), Some(&idx.args)),
            _ => continue,
        };
        let target_key = pystr::upper(target_name);
        let st_tables = tables(st_ref);
        let Some(shape) = st_tables.dims.get(&target_key) else {
            continue;
        };
        let mut offset = 0i64;
        if let Some(args) = target_args {
            if args.len() != 1 || shape.len() != 1 {
                continue;
            }
            let Some(k) = _reps_value(args[0].as_deref(), env) else {
                continue;
            };
            offset = k - 1;
        }
        let total: i64 = shape.iter().product();
        if env.profile.single_pass
            && st_tables.types.get(&target_key).map(String::as_str) == Some("UINT1")
        {
            continue;
        }
        let backing = declared
            .get(&target_key)
            .map_or(target_name, String::as_str);
        for alias in &st.args[1..] {
            let Some(Expr::Name(alias_name)) = alias.as_deref() else {
                break;
            };
            if !(0 <= offset && offset < total) {
                break;
            }
            let first_name = if offset == 0 {
                backing.to_owned()
            } else {
                alias_name.name.clone()
            };
            out.insert(
                pystr::upper(&alias_name.name),
                (
                    first_name,
                    alias_name.name.clone(),
                    backing.to_owned(),
                    offset,
                    total,
                ),
            );
            offset += 1;
        }
    }
    out
}

pub const WIND_SUBINTERVAL: &str =
    "WindVector: non-zero SubInterval only valid for output modes 0, 2, and 4.";

/// Port of checker/instr_data.py::_wind_options
pub fn _wind_options(
    st: &Instruction,
    reps: Option<i64>,
    env: &Env,
    diags: &mut Diagnostics,
    short_speed: bool,
) -> bool {
    let long_arg = |i: usize| -> Option<i64> {
        let arg = st.args.get(i).and_then(Option::as_deref)?;
        consteval::value_of(Some(arg), env)
            .ok()
            .map(|v| consteval::to_long(&v))
    };
    let sub = long_arg(5);
    let option = long_arg(7);
    let mode = option.map(|o| o & 0xFF);
    if matches!(mode, Some(1 | 3)) && !matches!(sub, None | Some(0)) {
        diags.error(
            st.base.node.line,
            WIND_SUBINTERVAL,
            &st.base.node.file,
            ShowFile::Bool(true),
            0,
            "wind_subinterval",
            "soft",
        );
    }
    if let Some(m) = mode
        && m > 4
    {
        diags.error_basic(
            st.base.node.line,
            "Invalid Output Option in WindVector.",
            &st.base.node.file,
        );
        if reps.is_some_and(|r| r >= 1) {
            if short_speed {
                let name = match st.args.get(1).and_then(Option::as_deref) {
                    Some(Expr::Name(n)) => n.name.as_str(),
                    Some(Expr::Index(idx)) => idx.name.as_str(),
                    _ => "",
                };
                diags.error_basic(
                    st.base.node.line,
                    &format!("Variable {name} out of bounds."),
                    &st.base.node.file,
                );
            }
            diags.error(
                st.base.node.line,
                "Out of memory.",
                &st.base.node.file,
                ShowFile::Bool(true),
                0,
                "out_of_memory",
                "",
            );
            return true;
        }
    }
    if reps == Some(0) {
        diags.error_basic(st.base.node.line, "Illegal Reps 0", &st.base.node.file);
        return true;
    }
    false
}

/// Port of checker/instr_data.py::check_constant_destinations
pub fn check_constant_destinations(
    program: &ProgramRef,
    profile: &'static Profile,
    diags: &mut Diagnostics,
    env: &Env,
) {
    if profile.name != "cr6" && profile.name != "cr1000x" {
        return;
    }
    let env = _destination_env(env);
    let mut variables = context::of(program, Some(profile), None)
        .declared_variable_names()
        .clone();
    let mut in_const_table = false;
    for st in &program.statements {
        match st.as_ref() {
            Stmt::Instruction(inst) => {
                let kw = pystr::upper(&inst.keyword);
                if kw == "CONSTTABLE" || kw == "ENDCONSTTABLE" {
                    in_const_table = kw == "CONSTTABLE";
                }
            }
            Stmt::ConstDecl(c) if in_const_table => {
                variables.insert(pystr::upper(&c.name));
            }
            _ => {}
        }
    }
    let flat = super::part1::_with_inline(&program.statements);
    for st_ref in flat {
        let Stmt::Instruction(st) = st_ref.as_ref() else {
            continue;
        };
        let kw = pystr::upper(&st.keyword);
        let Some(record) = profile.instructions().get(&kw) else {
            continue;
        };
        if _OWN_VARIABLE_CHECK.contains(&kw.as_str()) {
            continue;
        }
        let limit = record.params.len().min(st.args.len());
        for (i, &code) in record.params[..limit].iter().enumerate() {
            if matches!(code, 8 | 27 | 29)
                && constant_destination(st.args[i].as_deref(), &env, &variables)
            {
                let idx = i64::try_from(i).unwrap_or(0);
                let raw = expressions::_arg_text(st, idx);
                let txt = pystr::strip(&raw);
                diags.error_basic(
                    st.base.node.line,
                    &format!("Invalid Variable or Array: {txt}."),
                    &st.base.node.file,
                );
            }
        }
    }
}

/// Port of checker/instr_data.py::drop_unreached_destinations
pub fn drop_unreached_destinations(
    program: &ProgramRef,
    profile: &'static Profile,
    diags: &mut Diagnostics,
    env: &Env,
) {
    if profile.name != "cr6" && profile.name != "cr1000x" {
        return;
    }
    let env = _destination_env(env);
    let variables = context::of(program, Some(profile), None)
        .declared_variable_names()
        .clone();
    let mut drop: Vec<DiagRef> = Vec::new();
    let flat = super::part1::_with_inline(&program.statements);
    for st_ref in flat {
        let Stmt::Instruction(st) = st_ref.as_ref() else {
            continue;
        };
        let kw = pystr::upper(&st.keyword);
        let Some(record) = profile.instructions().get(&kw) else {
            continue;
        };
        let here: Vec<DiagRef> = diags
            .items
            .iter()
            .filter(|d| d.line == st.base.node.line && d.file == st.base.node.file)
            .cloned()
            .collect();
        let texts: Vec<Option<String>> = st
            .args
            .iter()
            .enumerate()
            .map(|(i, a)| {
                a.as_ref().map(|_| {
                    let idx = i64::try_from(i).unwrap_or(0);
                    pystr::strip(&expressions::_arg_text(st, idx)).to_owned()
                })
            })
            .collect();
        let first_bad = texts.iter().enumerate().find_map(|(j, text)| {
            let text = text.as_ref()?;
            let needle = format!("Variable illegal in Parameter: {text}.");
            here.iter().any(|d| d.message == needle).then_some(j)
        });
        let Some(first_bad) = first_bad else {
            continue;
        };
        let limit = record.params.len().min(st.args.len());
        for (i, &code) in record.params[..limit].iter().enumerate() {
            if i > first_bad
                && matches!(code, 8 | 27 | 29)
                && constant_destination(st.args[i].as_deref(), &env, &variables)
                && let Some(txt) = &texts[i]
            {
                let needle = format!("Invalid Variable or Array: {txt}.");
                for d in &here {
                    if d.message == needle {
                        drop.push(Rc::clone(d));
                    }
                }
            }
        }
    }
    diags
        .items
        .retain(|d| !drop.iter().any(|x| Rc::ptr_eq(d, x)));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostics::Diagnostics;
    use crate::profiles;
    use crate::syntax::{
        Binary, ConstValue, Declaration, Expr, Index, Instruction, Name, Number, NumberValue,
        Program, Stmt, StmtBase, StringLit, Unary, VarDecl,
    };

    fn make_inst(line: i64, keyword: &str, text: &str, args: Vec<Option<ExprRef>>) -> StmtRef {
        let mut base = StmtBase {
            text: text.to_owned(),
            ..StmtBase::default()
        };
        base.node.line = line;
        base.node.file = "main.CR6".to_owned();
        Rc::new(Stmt::Instruction(Instruction {
            base,
            keyword: keyword.to_owned(),
            args,
            ..Instruction::default()
        }))
    }

    fn name_expr(name: &str) -> ExprRef {
        Rc::new(Expr::Name(Name {
            name: name.to_owned(),
            ..Name::default()
        }))
    }

    fn int_expr(v: i64) -> ExprRef {
        Rc::new(Expr::Number(Number {
            value: NumberValue::Int(v),
            ..Number::default()
        }))
    }

    fn str_expr(s: &str) -> ExprRef {
        Rc::new(Expr::String(StringLit {
            value: s.to_owned(),
            ..StringLit::default()
        }))
    }

    #[test]
    fn pick_tokens_preserve_trailing_slots_and_split_quoted_commas() {
        let mut stmt = Instruction {
            base: StmtBase {
                text: "  mEnUpIcK (1,\"a,b'c\",) 'comment".to_owned(),
                ..StmtBase::default()
            },
            ..Instruction::default()
        };
        let tokens = _pick_tokens(&stmt).unwrap();
        assert_eq!(tokens, ["1", "\"a", "b'c\"", ""]);

        stmt.base.text = "MenuPicker(1)".to_owned();
        assert_eq!(_pick_tokens(&stmt), None);
    }

    #[test]
    fn boolean_pick_ok_and_check_boolean_picklist() {
        let cr6 = profiles::get("cr6").unwrap();
        let env = Env::new(cr6);
        env.define("MY_TRUE", ConstValue::Int(-1), None);
        env.define("MY_TWO", ConstValue::Int(2), None);

        assert!(_boolean_pick_ok(" True", &env));
        assert!(_boolean_pick_ok("0", &env));
        assert!(_boolean_pick_ok("-1", &env));
        assert!(_boolean_pick_ok("&HFFFFFFFF", &env));
        assert!(_boolean_pick_ok("&B0", &env));
        assert!(_boolean_pick_ok("MY_TRUE", &env));
        assert!(!_boolean_pick_ok("MY_TWO", &env));
        assert!(!_boolean_pick_ok("1", &env));
        assert!(!_boolean_pick_ok("0 ", &env));

        let mut base = StmtBase {
            text: "MenuPick(True, 2,)".to_owned(),
            ..StmtBase::default()
        };
        base.node.line = 10;
        base.node.file = "main.CR6".to_owned();
        let stmt = Instruction {
            base,
            keyword: "MenuPick".to_owned(),
            ..Instruction::default()
        };
        let mut diags = Diagnostics::new();
        _check_boolean_picklist(&stmt, &env, &mut diags);
        let msgs: Vec<&str> = diags.items.iter().map(|d| d.message.as_str()).collect();
        assert_eq!(
            msgs,
            vec![
                "Boolean picklist must be True or False.",
                "Unable to parse MenuPick strings.",
            ]
        );
    }

    #[test]
    fn pick_value_bad_checks_kinds() {
        let cr6 = profiles::get("cr6").unwrap();
        let env = Env::new(cr6);
        env.define("C_INT", ConstValue::Int(5), None);
        env.define("C_FLT", ConstValue::Float(1.5), None);
        let program = Rc::new(Program::new("main.CR6", vec![]));
        let mut vars = BTreeSet::new();
        vars.insert("V".to_owned());
        let mut known = BTreeSet::new();
        known.insert("V".to_owned());
        known.insert("C_INT".to_owned());
        known.insert("C_FLT".to_owned());
        known.insert("NAN".to_owned());

        assert_eq!(
            _pick_value_bad(" 1.5", "int", &env, &program, cr6, &vars, Some(&known)),
            Some("1.5".to_owned())
        );
        assert_eq!(
            _pick_value_bad(" 1.5", "float", &env, &program, cr6, &vars, Some(&known)),
            None
        );
        assert_eq!(
            _pick_value_bad(" C_FLT", "int", &env, &program, cr6, &vars, Some(&known)),
            Some("C_FLT".to_owned())
        );
        assert_eq!(
            _pick_value_bad(" C_INT", "int", &env, &program, cr6, &vars, Some(&known)),
            None
        );
        assert_eq!(
            _pick_value_bad(" V", "float", &env, &program, cr6, &vars, Some(&known)),
            Some("V".to_owned())
        );
        assert_eq!(
            _pick_value_bad(" -A", "float", &env, &program, cr6, &vars, Some(&known)),
            Some("-A".to_owned())
        );
    }

    #[test]
    fn destination_and_reps_helpers() {
        let cr6 = profiles::get("cr6").unwrap();
        let env = Env::new(cr6);
        env.define("C", ConstValue::Int(3), None);
        let mut vars = BTreeSet::new();
        vars.insert("X".to_owned());

        let sum_expr = Expr::Binary(Binary {
            op: "+".to_owned(),
            left: Some(int_expr(1)),
            right: Some(int_expr(2)),
            ..Binary::default()
        });
        assert!(_destination_expression(Some(&sum_expr)));
        assert!(constant_destination(Some(&sum_expr), &env, &vars));

        let var_expr = Expr::Name(Name {
            name: "X".to_owned(),
            ..Name::default()
        });
        assert!(!constant_destination(Some(&var_expr), &env, &vars));

        let dest_env = _destination_env(&env);
        let nan_expr = Expr::Name(Name {
            name: "NaN".to_owned(),
            ..Name::default()
        });
        assert!(constant_destination(Some(&nan_expr), &dest_env, &vars));

        assert_eq!(_reps_value(Some(&str_expr("fuzz")), &env), Some(_INT32_MIN));
        assert_eq!(_reps_value(Some(&int_expr(4)), &env), Some(4));
        assert_eq!(
            _reps_value(Some(&int_expr(999_999_999_999)), &env),
            Some(_INT32_MIN)
        );
    }

    #[test]
    fn struct_typed_and_short_array_and_wind_options() {
        let cr6 = profiles::get("cr6").unwrap();
        let env = Env::new(cr6);
        let program = Program::new(
            "main.CR6",
            vec![
                make_inst(
                    1,
                    "StructureType",
                    "StructureType MyType",
                    vec![Some(name_expr("MyType"))],
                ),
                Rc::new(Stmt::Declaration(Declaration {
                    kind: "PUBLIC".to_owned(),
                    vars: vec![
                        VarDecl {
                            name: "S".to_owned(),
                            type_: Some("MyType".to_owned()),
                            ..VarDecl::default()
                        }
                        .into(),
                    ]
                    .into(),
                    ..Declaration::default()
                })),
            ],
        );
        assert!(_is_struct_typed(&program, "s"));
        assert!(!_is_struct_typed(&program, "other"));

        let mut dims = OrderedMap::new();
        dims.insert("A".to_owned(), vec![5]);
        let idx_expr = Expr::Index(Index {
            name: "A".to_owned(),
            args: vec![Some(int_expr(4))],
            ..Index::default()
        });
        assert!(_short_array(Some(&idx_expr), 3, &dims, &env));
        assert!(!_short_array(Some(&idx_expr), 2, &dims, &env));

        let mut wind_base = StmtBase::default();
        wind_base.node.line = 12;
        wind_base.node.file = "main.CR6".to_owned();
        let wind_st = Instruction {
            base: wind_base,
            keyword: "WindVector".to_owned(),
            args: vec![
                Some(int_expr(1)),
                Some(name_expr("WS")),
                Some(name_expr("WD")),
                Some(name_expr("FP2")),
                Some(int_expr(0)),
                Some(int_expr(10)),
                Some(int_expr(0)),
                Some(int_expr(9)),
            ],
            ..Instruction::default()
        };
        let mut diags = Diagnostics::new();
        assert!(_wind_options(&wind_st, Some(1), &env, &mut diags, true));
        let msgs: Vec<&str> = diags.items.iter().map(|d| d.message.as_str()).collect();
        assert_eq!(
            msgs,
            vec![
                "Invalid Output Option in WindVector.",
                "Variable WS out of bounds.",
                "Out of memory.",
            ]
        );
    }

    #[test]
    fn check_table_and_menu_placement_reports_sequence_and_menu_errors() {
        let cr6 = profiles::get("cr6").unwrap();
        let program = Rc::new(Program::new(
            "main.CR6",
            vec![
                make_inst(1, "EndDialSequence", "EndDialSequence", vec![]),
                make_inst(
                    2,
                    "DataTable",
                    "DataTable(T,1,-1)",
                    vec![Some(name_expr("T")), Some(int_expr(1)), Some(int_expr(-1))],
                ),
                make_inst(3, "DataInterval", "DataInterval(0,1,Min,10)", vec![]),
                make_inst(4, "DataInterval", "DataInterval(0,1,Min,10)", vec![]),
                make_inst(5, "EndTable", "EndTable", vec![]),
                make_inst(6, "DisplayMenu", "DisplayMenu(\"M\",-1)", vec![]),
                make_inst(7, "EndSubMenu", "EndSubMenu", vec![]),
            ],
        ));
        let mut diags = Diagnostics::new();
        let stopped = check_table_and_menu_placement(&program, cr6, &mut diags, None, None);
        assert_eq!(stopped, Some(("main.CR6".to_owned(), 7)));
        let msgs: Vec<&str> = diags.items.iter().map(|d| d.message.as_str()).collect();
        assert_eq!(
            msgs,
            vec![
                "Missing DialSequence().",
                "Only one DataInterval allowed within DataTable..EndTable.",
                "EndSubMenu without SubMenu.",
            ]
        );
    }

    #[test]
    fn alias_sources_and_constant_destinations() {
        let cr6 = profiles::get("cr6").unwrap();
        let env = Env::new(cr6);
        let idx_target = Rc::new(Expr::Index(Index {
            name: "Arr".to_owned(),
            args: vec![Some(int_expr(2))],
            ..Index::default()
        }));
        let program = Rc::new(Program::new(
            "main.CR6",
            vec![
                Rc::new(Stmt::Declaration(Declaration {
                    kind: "PUBLIC".to_owned(),
                    vars: vec![
                        VarDecl {
                            name: "Arr".to_owned(),
                            dims: vec![int_expr(3)],
                            ..VarDecl::default()
                        }
                        .into(),
                    ]
                    .into(),
                    ..Declaration::default()
                })),
                make_inst(
                    2,
                    "Alias",
                    "Alias(Arr(2), B, C)",
                    vec![Some(idx_target), Some(name_expr("B")), Some(name_expr("C"))],
                ),
                make_inst(
                    3,
                    "PanelTemp",
                    "PanelTemp(15000,15000)",
                    vec![Some(int_expr(15000)), Some(int_expr(15000))],
                ),
            ],
        ));
        let mut scope = ScopeTables::default();
        scope.dims.insert("ARR".to_owned(), vec![3]);
        scope.types.insert("ARR".to_owned(), "FLOAT".to_owned());
        let aliases = _alias_sources(&program, &|_| scope.clone(), &env);
        assert_eq!(
            aliases.get("B"),
            Some(&("B".to_owned(), "B".to_owned(), "Arr".to_owned(), 1, 3))
        );
        assert_eq!(
            aliases.get("C"),
            Some(&("C".to_owned(), "C".to_owned(), "Arr".to_owned(), 2, 3))
        );

        let mut diags = Diagnostics::new();
        check_constant_destinations(&program, cr6, &mut diags, &env);
        assert_eq!(diags.items.len(), 1);
        assert!(
            diags.items[0]
                .message
                .starts_with("Invalid Variable or Array:")
        );
    }
}
