//! Port of `checker/instr_rules.py` (lines 3085-3737).

#![allow(dead_code, unused_variables, unused_imports)]
#![allow(clippy::used_underscore_items)]

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;
use std::sync::LazyLock;

use crate::checker::instr_rules::part2::{_US_PER_UNIT, _interval_us, _scan_raw};
use crate::checker::instr_rules::part4::{
    _CDM_PORT_MODULES, _cdm_null_module, _delay_option, _dnp_long, _is_cdm_named_type, DnpConv,
    cdm_unsupported_module,
};
use crate::checker::{context, longconv};
use crate::consteval::{self, Env};
use crate::diagnostics::{DiagRef, Diagnostic, Diagnostics};
use crate::profiles::{Json, Profile};
use crate::syntax::{
    Assignment, Call, Case, ConstDecl, ConstValue, Declaration, Expr, ExprRef, Index, Instruction,
    Juxtaposed, Member, Number, NumberValue, OrderedMap, Program, ProgramRef, Stmt, StmtRef,
    SubDef, Unparsed, VarDecl,
};
use crate::{mode, pyre, pystr};

fn arg_at(args: &[Option<ExprRef>], i: usize) -> Option<&Expr> {
    args.get(i).and_then(|a| a.as_deref())
}

/// Port of checker/instr_rules.py::check_fieldcal_variable_args
pub fn check_fieldcal_variable_args(
    program: &ProgramRef,
    profile: &'static Profile,
    diags: &mut Diagnostics,
    env: Option<&Env>,
) {
    let ctx = context::of(program, Some(profile), env.cloned());
    let types = ctx.variable_types(false);
    let dims = if env.is_some() {
        Some(ctx.declared_all_dims())
    } else {
        None
    };
    let float_names = longconv::float_names_in(&program.statements);
    let literal_names = longconv::literal_names_in(&program.statements);
    let conv = DnpConv {
        env,
        profile,
        names: &float_names,
        literals: &literal_names,
    };
    let number = |arg: Option<&Expr>| {
        if env.is_some() {
            _dnp_long(arg, &conv, false)
        } else {
            None
        }
    };

    let mut size_reported: BTreeSet<String> = BTreeSet::new();
    for stmt in &program.statements {
        let Stmt::Instruction(stmt) = stmt.as_ref() else {
            continue;
        };
        if stmt.args.is_empty() {
            continue;
        }
        let keyword = pystr::upper(&stmt.keyword);
        if keyword != "FIELDCAL" && keyword != "FIELDCALSTRAIN" {
            continue;
        }
        let args = &stmt.args;
        let line = stmt.base.node.line;
        let file = &stmt.base.node.file;
        let before = diags.items.len();
        let mut option = number(arg_at(args, 0));
        if option.is_none() {
            option = match arg_at(args, 0) {
                Some(Expr::Number(n)) => match n.value {
                    NumberValue::Int(v) => Some(v),
                    NumberValue::BigInt(_) => Some(i64::MAX),
                    NumberValue::Float(_) => None,
                },
                _ => None,
            };
        }
        let Some(option) = option else {
            continue;
        };
        if keyword == "FIELDCALSTRAIN" {
            if !((10..=44).contains(&option) && option % 10 <= 4) {
                diags.error_basic(line, "Invalid function option.", file);
            }
            continue;
        }
        if !(0..=4).contains(&option) {
            diags.error_basic(line, "Invalid function option.", file);
            continue;
        }
        // a MeasVar or Reps error (CR300/CR350 then print no Mult/Offset line)
        let mut earlier = false;
        let meas = args.len() > 1 && _fieldcal_string_subscript(arg_at(args, 1));
        if meas {
            earlier = true;
            diags.error_basic(
                line,
                "FieldCal MeasVar must be a declared public variable that can be resolved during compilation.",
                file,
            );
        }
        let reps = if args.len() > 2 {
            number(arg_at(args, 2))
        } else {
            None
        };
        if let Some(reps) = reps
            && reps != 1
            && reps != 2
            && !meas
        {
            earlier = true;
            diags.error_basic(line, "Illegal Reps: must be 1 or 2.", file);
        }
        for (index, label, options) in [
            (3_usize, "Mult", &[2_i64, 3][..]),
            (4, "Offset", &[0, 1, 2, 4][..]),
        ] {
            if options.contains(&option)
                && index < args.len()
                && _fieldcal_bad_variable(arg_at(args, index))
                && !(earlier && profile.single_pass)
            {
                diags.error_basic(
                    line,
                    &format!(
                        "FieldCal {label} must be a declared variable that can be resolved during compilation."
                    ),
                    file,
                );
                earlier = true;
            }
        }
        if args.len() > 5 && _fieldcal_string_subscript(arg_at(args, 5)) {
            diags.error_basic(
                line,
                "FieldCal Mode must be a declared public variable that can be resolved during compilation.",
                file,
            );
        }
        if (1..=3).contains(&option) && args.len() > 6 {
            let known = arg_at(args, 6);
            let shape = match (dims, known) {
                (Some(dims), Some(Expr::Index(ix))) => dims.get(&pystr::upper(&ix.name)),
                _ => None,
            };
            // (printed after `Variable K out of bounds.`)
            let outside = match (shape, known) {
                (Some(shape), Some(Expr::Index(ix))) => {
                    !shape.is_empty()
                        && shape.len() == 1
                        && ix.args.len() == 1
                        && match ix.args[0].as_deref() {
                            Some(Expr::Number(n)) => match n.value {
                                NumberValue::Int(v) => !(1..=shape[0]).contains(&v),
                                NumberValue::BigInt(_) => true,
                                NumberValue::Float(_) => false,
                            },
                            _ => false,
                        }
                }
                _ => false,
            };
            let bad = outside
                || _fieldcal_bad_variable(known)
                || matches!(known, Some(Expr::Unary(_)))
                || match known {
                    Some(Expr::Name(n)) => {
                        types.get(&pystr::upper(&n.name)).map(String::as_str) == Some("STRING")
                    }
                    Some(Expr::Index(ix)) => {
                        types.get(&pystr::upper(&ix.name)).map(String::as_str) == Some("STRING")
                    }
                    _ => false,
                };
            if bad {
                diags.error_basic(
                    line,
                    "FieldCal KnownVar must be a declared public variable that can be resolved during compilation.",
                    file,
                );
            }
        }
        if (1..=3).contains(&option)
            && args.len() > 6
            && let Some(dims) = dims
            && diags.items.len() == before
        {
            let known = arg_at(args, 6);
            let measure = arg_at(args, 1);
            let mut sizes: Vec<Option<i64>> = Vec::new();
            for var in [known, measure] {
                let size = match var {
                    Some(Expr::Name(n)) => Some(&n.name),
                    Some(Expr::Index(ix)) => Some(&ix.name),
                    _ => None,
                }
                .and_then(|name| {
                    let upper = pystr::upper(name);
                    let shape = dims.get(&upper)?;
                    if shape.len() > 1 || types.get(&upper).map(String::as_str) == Some("STRING") {
                        None
                    } else {
                        shape.first().copied()
                    }
                });
                sizes.push(size);
            }
            if sizes.contains(&None) || sizes[0] == sizes[1] {
                continue;
            }
            let name_of = |e: Option<&Expr>| match e {
                Some(Expr::Name(n)) => n.name.clone(),
                Some(Expr::Index(ix)) => ix.name.clone(),
                _ => String::new(),
            };
            let text = format!(
                "{} and {} must be declared the same size.",
                name_of(known),
                name_of(measure)
            );
            if !size_reported.contains(&text) {
                // (printed once per program, oracle 2026-10-12)
                size_reported.insert(text.clone());
                diags.error_basic(line, &text, file);
            }
        }
    }
}

/// Port of checker/instr_rules.py::_fieldcal_string_subscript
pub fn _fieldcal_string_subscript(arg: Option<&Expr>) -> bool {
    match arg {
        Some(Expr::Index(ix)) => ix
            .args
            .iter()
            .any(|a| matches!(a.as_deref(), Some(Expr::String(_)))),
        _ => false,
    }
}

/// Port of `checker/instr_rules.py::check_matrix_destination`.
pub fn check_matrix_destination(
    stmt: &Instruction,
    _program: &ProgramRef,
    profile: &'static Profile,
    diags: &mut Diagnostics,
    _env: &Env,
) {
    if !profile.single_pass || stmt.args.len() < 4 {
        return;
    }
    let constant = match stmt.args[3].as_deref() {
        Some(Expr::Number(_) | Expr::String(_) | Expr::Binary(_)) => true,
        Some(Expr::Unary(u)) => matches!(u.operand.as_deref(), Some(Expr::Number(_))),
        _ => false,
    };
    if constant {
        diags.error_basic(
            stmt.base.node.line,
            "Illegal destination variable type, expecting Float.",
            &stmt.base.node.file,
        );
    }
}

/// Port of checker/instr_rules.py::_fieldcal_bad_variable
pub fn _fieldcal_bad_variable(arg: Option<&Expr>) -> bool {
    match arg {
        Some(Expr::String(_) | Expr::Binary(_)) => true,
        Some(Expr::Index(ix)) => ix
            .args
            .iter()
            .any(|a| matches!(a.as_deref(), Some(Expr::String(_)))),
        Some(Expr::Number(n)) => match n.value {
            NumberValue::Int(v) => v >= 0,
            NumberValue::Float(f) => f >= 0.0,
            NumberValue::BigInt(_) => true,
        },
        _ => false,
    }
}

/// Port of `checker/instr_rules.py::check_samplefieldcal`.
pub fn check_samplefieldcal(program: &Program, profile: &Profile, diags: &mut Diagnostics) {
    let mut instructions = program
        .statements
        .iter()
        .filter_map(|stmt| match stmt.as_ref() {
            Stmt::Instruction(instr) => Some(instr),
            _ => None,
        });
    if !profile.instructions().contains("SAMPLEFIELDCAL")
        || instructions.clone().any(|instr| {
            instr.keyword.eq_ignore_ascii_case("FIELDCAL")
                || instr.keyword.eq_ignore_ascii_case("FIELDCALSTRAIN")
        })
    {
        return;
    }
    if let Some(instr) =
        instructions.find(|instr| instr.keyword.eq_ignore_ascii_case("SAMPLEFIELDCAL"))
    {
        diags.error_basic(
            instr.base.node.line,
            "SampleFieldCal with no FieldCal in program.",
            &instr.base.node.file,
        );
    }
}

/// Port of checker/instr_rules.py::_cdm_type_value
pub fn _cdm_type_value(kind: &ConstValue) -> Option<i64> {
    match kind {
        ConstValue::BigInt(_) => None,
        ConstValue::Str(_) => Some(0),
        ConstValue::Int(i) => (i.abs() < (1_i64 << 31)).then_some(*i),
        ConstValue::Float(f) | ConstValue::CFloat(f) => {
            if f.is_finite() && f.abs() < 2_147_483_648.0 {
                pystr::py_int_from_f64(*f)
            } else {
                None
            }
        }
    }
}

/// Port of checker/instr_rules.py::cdm_null_module
pub fn cdm_null_module(stmt: &Instruction, env: &Env) -> bool {
    let kw = pystr::upper(&stmt.keyword);
    if _cdm_null_module(&kw).is_none() || stmt.args.is_empty() {
        return false;
    }
    let Ok(value) = consteval::value_of(stmt.args[0].as_deref(), env) else {
        return false;
    };
    let Some(kind) = _cdm_type_value(&value) else {
        return false;
    };
    (0..=65536).contains(&kind)
        && !_is_cdm_named_type(kind)
        && (kw != "CDM_DELAY"
            || match _delay_option(stmt, env) {
                Some(ConstValue::Int(0)) => true,
                Some(ConstValue::Float(f) | ConstValue::CFloat(f)) => f == 0.0,
                _ => false,
            })
}

fn _cdm_bridge_option_args(kw: &str) -> Option<(usize, usize)> {
    match kw {
        "CDM_BRHALF3W" => Some((10, 8)),
        "CDM_BRFULL6W" => Some((12, 9)),
        _ => None,
    }
}

/// Port of checker/instr_rules.py::_check_cdm_bridge_options
pub fn _check_cdm_bridge_options(
    stmt: &Instruction,
    positions: (usize, usize),
    profile: &Profile,
    diags: &mut Diagnostics,
    env: &Env,
    names: &BTreeSet<String>,
    literals: &BTreeSet<String>,
) {
    let value = |pos: usize| -> Option<i64> {
        if pos >= stmt.args.len() {
            return None;
        }
        let arg = stmt.args[pos].as_deref();
        if let Some(Expr::Name(n)) = arg
            && pystr::upper(&n.name) == "INF"
            && !env.consts.borrow().contains_key("INF")
        {
            return Some((1_i64 << 31) - 1);
        }
        longconv::long_value_with(arg, env, profile, names, literals)
    };
    let settling = value(positions.0);
    let excitation = value(positions.1);
    let line = stmt.base.node.line;
    let file = &stmt.base.node.file;
    if let Some(settling) = settling
        && settling != 0
        && !(100..=100_000).contains(&settling)
    {
        diags.error_basic(
            line,
            &format!("Settling time: {settling}, out of range (100 - 100000, or 0 for default)."),
            file,
        );
    }
    if let Some(excitation) = excitation {
        let magnitude = if excitation == longconv::INT_MIN {
            excitation
        } else {
            excitation.abs()
        };
        if !(20..=5000).contains(&magnitude) {
            diags.error_basic(
                line,
                &format!("Excitation magnitude: {magnitude}, out of range (20 - 5000)."),
                file,
            );
        }
    }
}

/// Port of checker/instr_rules.py::_check_cdm_mux_options
pub fn _check_cdm_mux_options(
    stmt: &Instruction,
    module: i64,
    profile: &Profile,
    diags: &mut Diagnostics,
    env: &Env,
    names: &BTreeSet<String>,
    literals: &BTreeSet<String>,
) {
    if stmt.args.len() < 5 {
        return;
    }
    let conv = |i: usize| {
        longconv::long_value_with(stmt.args[i].as_deref(), env, profile, names, literals)
    };
    let (mut clock, mut reset, width) = (conv(2), conv(3), conv(4));
    let last_port = if module == 220 || module == 222 { 2 } else { 4 };
    let line = stmt.base.node.line;
    let file = &stmt.base.node.file;
    if let Some(c) = clock
        && !(1..=last_port).contains(&c)
    {
        diags.error_basic(line, "Unrecognized port!", file);
        clock = Some(1);
    }
    if let Some(r) = reset
        && !(1..=last_port).contains(&r)
    {
        diags.error_basic(line, "Unrecognized port!", file);
        reset = Some(2);
    }
    if clock.is_some() && clock == reset {
        diags.error_basic(line, "Clock and Reset ports must be unique!", file);
    }
    if let Some(width) = width
        && (width < 1 || longconv::_wrap(1000 * width) > 100_000)
    {
        diags.error_basic(
            line,
            &format!("MuxSelect ClockPW: {width}, out of range (1 - 100)."),
            file,
        );
    }
}

/// Port of checker/instr_rules.py::check_cdm_address_types
pub fn check_cdm_address_types(
    program: &ProgramRef,
    profile: &'static Profile,
    diags: &mut Diagnostics,
    env: &Env,
) {
    const ADDRESS_INSTRUCTIONS: &[&str] = &[
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
    if profile.name != "cr6" && profile.name != "cr1000x" {
        return;
    }
    let mut first: OrderedMap<i64, f64> = OrderedMap::new();
    let mut conversion: Option<(BTreeSet<String>, BTreeSet<String>)> = None;
    for stmt in &program.statements {
        let Stmt::Instruction(stmt) = stmt.as_ref() else {
            continue;
        };
        let kw = pystr::upper(&stmt.keyword);
        if !ADDRESS_INSTRUCTIONS.contains(&kw.as_str()) || stmt.args.len() < 2 {
            continue;
        }
        let line = stmt.base.node.line;
        let file = &stmt.base.node.file;
        let Ok(address) = consteval::value_of(stmt.args[1].as_deref(), env) else {
            continue;
        };
        // The address converts as C's `(long)`; outside 1..120 (a string, `TRUE`, overflow) is
        // `CPI Address must be 1..120.` and the statement then registers as address 1 (oracle).
        let mut address = consteval::to_long(&address);
        if !(1..=120).contains(&address) {
            if kw != "CDM_PERIODAVG" {
                diags.error_basic(line, "CPI Address must be 1..120.", file);
            }
            address = 1;
        }
        let Ok(kind) = consteval::value_of(stmt.args[0].as_deref(), env) else {
            continue;
        };
        // a string or fraction only gets the module line (its clash behaviour is not probed)
        // ponytail: nan/inf kind crashes Python (`int(kind)`); here it takes this branch
        let numeric: Option<f64> = match &kind {
            ConstValue::Str(_) => None,
            ConstValue::Int(i) =>
            {
                #[allow(clippy::cast_precision_loss)]
                Some(*i as f64)
            }
            ConstValue::Float(f) | ConstValue::CFloat(f) => {
                (f.is_finite() && f.fract() == 0.0).then_some(*f)
            }
            ConstValue::BigInt(s) => Some(s.parse::<f64>().unwrap_or(f64::INFINITY)),
        };
        let Some(numeric) = numeric else {
            if cdm_null_module(stmt, env) {
                for name in _cdm_null_module(&kw).unwrap_or(&[]) {
                    diags.error_basic(
                        line,
                        &format!("{name} instruction not supported by module NULL."),
                        file,
                    );
                }
            }
            continue;
        };
        let prior = if let Some(p) = first.get(&address) {
            *p
        } else {
            first.insert(address, numeric);
            numeric
        };
        let kind_int = match &kind {
            ConstValue::Int(i) => Some(*i),
            ConstValue::Float(f) | ConstValue::CFloat(f) => pystr::py_int_from_f64(*f),
            _ => None,
        };
        #[allow(clippy::float_cmp)]
        let clash = prior != numeric;
        if clash {
            diags.error_basic(
                line,
                &format!("Address {address} already used with a different type CDM."),
                file,
            );
        } else if cdm_null_module(stmt, env) {
            for name in _cdm_null_module(&kw).unwrap_or(&[]) {
                diags.error_basic(
                    line,
                    &format!("{name} instruction not supported by module NULL."),
                    file,
                );
            }
        } else if let Some(module) = cdm_unsupported_module(stmt, env) {
            for name in _cdm_null_module(&kw).unwrap_or(&[]) {
                diags.error_basic(
                    line,
                    &format!("{name} instruction not supported by module {module}."),
                    file,
                );
            }
        } else if let Some(kind) = kind_int
            && _CDM_PORT_MODULES.contains(&kind)
            && (_cdm_bridge_option_args(&kw).is_some() || kw == "CDM_MUXSELECT")
        {
            if conversion.is_none() {
                conversion = Some((
                    longconv::float_names_in(&program.statements),
                    longconv::literal_names_in(&program.statements),
                ));
            }
            let (names, literals) = conversion.as_ref().expect("set above");
            if kw == "CDM_MUXSELECT" {
                _check_cdm_mux_options(stmt, kind, profile, diags, env, names, literals);
            } else if let Some(positions) = _cdm_bridge_option_args(&kw) {
                _check_cdm_bridge_options(stmt, positions, profile, diags, env, names, literals);
            }
        }
    }
}

const _EPI_FILTERS: &[&str] = &["CDM_BRIDGEFILT", "CDM_FFTFILT"];
const _EPI_RATES_MS: &[i64] = &[1, 2, 5, 10, 20, 50, 100, 200, 500, 1000];
const _FFT_SAMPLE_PERIODS: &[i64] = &[1, 2, 5, 10, 20, 50, 100, 200, 500, 1000, 2000, 5000, 10000];

/// Port of checker/instr_rules.py::check_epi_scan_buffers
pub fn check_epi_scan_buffers(
    program: &ProgramRef,
    profile: &'static Profile,
    diags: &mut Diagnostics,
    env: &Env,
) {
    static NEXT_SUBSCAN: LazyLock<pyre::Regex> =
        LazyLock::new(|| pyre::Regex::new(r"(?i)\s*Next\s+SubScan\b", 0));
    static NEXT_SCAN: LazyLock<pyre::Regex> =
        LazyLock::new(|| pyre::Regex::new(r"(?i)\s*Next\s+Scan\b", 0));
    if profile.name != "cr6" && profile.name != "cr1000x" {
        return;
    }
    let stmts: Vec<&Instruction> = program
        .statements
        .iter()
        .filter_map(|st| match st.as_ref() {
            Stmt::Instruction(i) => Some(i),
            _ => None,
        })
        .collect();
    if !stmts
        .iter()
        .any(|st| _EPI_FILTERS.contains(&pystr::upper(&st.keyword).as_str()))
    {
        return;
    }
    let float_names = longconv::float_names_in(&program.statements);
    let literal_names = longconv::literal_names_in(&program.statements);
    let names = (&float_names, &literal_names);
    let execution = mode::decide_with_env(&program.statements, profile, env);
    if execution != "Pipeline" {
        if execution == "Sequential" {
            for st in &stmts {
                if pystr::upper(&st.keyword) == "CDM_FFTFILT" {
                    diags.error_basic(
                        st.base.node.line,
                        "CDM_FFTFilt not allowed in Sequential Mode.",
                        &st.base.node.file,
                    );
                }
            }
        }
        return;
    }
    let mut slow = false;
    let mut us: Option<i64> = None;
    let mut minimum: Option<i64> = None;
    let mut subscan: i64 = 0;
    for st in stmts {
        let kw = pystr::upper(&st.keyword);
        let line = st.base.node.line;
        let file = &st.base.node.file;
        if kw == "SLOWSEQUENCE" {
            slow = true;
        } else if kw == "SCAN" {
            us = None;
            minimum = None;
            subscan = 0;
            let raw = if slow { None } else { _scan_raw(st, env) };
            if let Some((value, unit)) = raw {
                let total = value.saturating_mul(_US_PER_UNIT.get(&unit).copied().unwrap_or(1));
                if total >= 1000 {
                    us = Some(total);
                    let buffers = if st.args.len() > 2 {
                        _const_int(st.args[2].as_deref(), env)
                    } else {
                        None
                    };
                    if let Some(buffers) = buffers
                        && total <= 500_000
                        && buffers < std::cmp::min(100, 2_000_000 / total)
                    {
                        minimum = Some(std::cmp::min(100, 2_000_000 / total));
                    }
                }
            }
        } else if kw == "SUBSCAN" {
            subscan += 1;
        } else if (kw == "NEXT" && NEXT_SUBSCAN.match_(&st.base.text).is_some())
            || kw == "NEXTSUBSCAN"
        {
            subscan = std::cmp::max(0, subscan - 1);
        } else if (kw == "NEXT" && NEXT_SCAN.match_(&st.base.text).is_some()) || kw == "NEXTSCAN" {
            us = None;
            minimum = None;
        } else if _EPI_FILTERS.contains(&kw.as_str())
            && let Some(us) = us
        {
            if let Some(minimum) = minimum {
                diags.error_basic(
                    line,
                    &format!("Insufficient Scan buffers to read EPI data, minimum is {minimum}."),
                    file,
                );
            }
            if subscan == 0 && kw == "CDM_BRIDGEFILT" {
                if !_EPI_RATES_MS.iter().any(|ms| ms * 1000 == us) {
                    diags.error_basic(line, "Invalid Output Rate.", file);
                }
                if us > 1_000_000 {
                    diags.error_basic(
                        line,
                        "Maximum Scan interval for CDM_BridgeFilt is 1 Sec.",
                        file,
                    );
                }
            }
            if subscan == 0 && kw == "CDM_FFTFILT" {
                _check_cdm_fft(st, us, profile, env, names, diags);
            }
        }
    }
}

/// Port of checker/instr_rules.py::_check_cdm_fft
pub fn _check_cdm_fft(
    st: &Instruction,
    us: i64,
    profile: &Profile,
    env: &Env,
    names: (&BTreeSet<String>, &BTreeSet<String>),
    diags: &mut Diagnostics,
) {
    if st.args.len() != 17 {
        return;
    }
    let arg = |pos: usize| {
        longconv::long_value_with(st.args[pos].as_deref(), env, profile, names.0, names.1)
    };
    if !matches!(arg(0), Some(260 | 261)) {
        return;
    }
    let (rate, length) = (arg(9), arg(10));
    let (Some(rate), Some(length)) = (rate, length) else {
        return;
    };
    if rate <= 0 || !(32..=65536).contains(&length) {
        return;
    }
    let line = st.base.node.line;
    let file = &st.base.node.file;

    let required_us = 100 * (length * 10000 / rate);
    if us < required_us {
        diags.error_basic(
            line,
            &format!(
                "Scan interval must be at least {required_us} usec for FFT length of {length}."
            ),
            file,
        );
    }

    let (reference, bins, low, high) = (arg(13), arg(14), arg(15), arg(16));
    if let (Some(reference), Some(bins), Some(low), Some(high)) = (reference, bins, low, high) {
        let half_length = length / 2;
        if reference != 0 && !(1..=12).contains(&bins) {
            diags.error_basic(line, "Illegal spectral binning.", file);
        }
        if high < low {
            diags.error_basic(line, "ILow must be less than IHigh.", file);
        }
        if bins > 1 && (matches!(arg(12), Some(0 | 2)) || bins > half_length) {
            diags.error_basic(line, "Illegal spectral binning.", file);
        }
        let mut maximum: Option<i64> = None;
        if reference == 0 && bins >= 0 {
            maximum = Some(half_length / std::cmp::max(1, bins));
        } else if 0 < reference.abs()
            && reference.abs() <= 32767
            && (500..=10000).contains(&rate)
            && (0..=13).contains(&bins)
        {
            // The observed conversion of the nonzero-reference calculation on the probed range;
            // outside it the bound differs and is not modelled (docs/instructions.md, `Fref`).
            maximum = Some(longconv::INT_MIN);
        }
        if let Some(maximum) = maximum
            && high > maximum
        {
            diags.error_basic(line, &format!("Maximum IHigh is {maximum}."), file);
        }
    }

    if !_FFT_SAMPLE_PERIODS.contains(&(10000 / rate)) {
        diags.error_basic(line, "Invalid Output Rate.", file);
    }
}

/// Port of checker/instr_rules.py::_const_int
pub fn _const_int(arg: Option<&Expr>, env: &Env) -> Option<i64> {
    arg?;
    if !consteval::is_constant(arg, env) {
        return None;
    }
    match consteval::value_of(arg, env).ok()? {
        ConstValue::Int(i) => Some(i),
        ConstValue::Float(f) | ConstValue::CFloat(f) => pystr::py_int_from_f64(f),
        // ponytail: a BigInt does not fit i64; callers only compare it against small bounds
        ConstValue::BigInt(_) | ConstValue::Str(_) => None,
    }
}

/// Port of checker/instr_rules.py::check_avw200_scan_time
pub fn check_avw200_scan_time(
    program: &ProgramRef,
    profile: &'static Profile,
    diags: &mut Diagnostics,
    env: &Env,
) {
    static NEXT_SCAN: LazyLock<pyre::Regex> =
        LazyLock::new(|| pyre::Regex::new(r"(?i)\s*Next\s+Scan\b", 0));
    if (profile.name != "cr6" && profile.name != "cr1000x")
        || !profile.instructions().contains("AVW200")
    {
        return;
    }
    let stmts: Vec<&StmtRef> = program
        .statements
        .iter()
        .filter(|st| {
            matches!(
                st.as_ref(),
                Stmt::Instruction(_) | Stmt::If(_) | Stmt::For(_) | Stmt::DoLoop(_) | Stmt::Case(_)
            )
        })
        .collect();
    let is_avw = |st: &StmtRef| match st.as_ref() {
        Stmt::Instruction(i) => pystr::upper(&i.keyword) == "AVW200",
        _ => false,
    };
    if !stmts.iter().any(|st| is_avw(st)) {
        return;
    }
    if mode::decide_with_env(&program.statements, profile, env) != "Pipeline" {
        return;
    }
    // the open main-sequence Scan statement
    let mut scan: Option<&Instruction> = None;
    let mut slow = false;
    // (scan, [AVW200 calls])
    let mut sums: Vec<(&Instruction, Vec<&Instruction>)> = Vec::new();
    for st in &stmts {
        let Stmt::Instruction(st) = st.as_ref() else {
            if scan.is_some() {
                // AVW200 in a conditional/loop has its own, unmodelled message
                return;
            }
            continue;
        };
        let kw = pystr::upper(&st.keyword);
        if (kw == "SELECT" || kw == "WHILE") && scan.is_some() {
            return;
        }
        if kw == "SLOWSEQUENCE" {
            slow = true;
        } else if kw == "SCAN" {
            scan = if slow { None } else { Some(st) };
            if scan.is_some() {
                sums.push((st, Vec::new()));
            }
        } else if kw == "NEXTSCAN" || (kw == "NEXT" && NEXT_SCAN.match_(&st.base.text).is_some()) {
            scan = None;
        } else if kw == "SUBSCAN" && scan.is_some() {
            return;
        } else if kw == "AVW200" {
            if scan.is_none() {
                return;
            }
            sums.last_mut().expect("scan is open").1.push(st);
        }
    }
    let holders: Vec<&(&Instruction, Vec<&Instruction>)> =
        sums.iter().filter(|e| !e.1.is_empty()).collect();
    if holders.len() != 1
        || sums.iter().map(|e| e.1.len()).sum::<usize>()
            != stmts.iter().filter(|st| is_avw(st)).count()
    {
        return;
    }
    let (scan_stmt, calls) = holders[0];
    let Some(interval_us) = _interval_us(
        arg_at(&scan_stmt.args, 0),
        arg_at(&scan_stmt.args, 1),
        env,
        false,
    ) else {
        return;
    };
    if pystr::pymod_f64(interval_us, 1_000_000.0).is_none_or(|m| m != 0.0) || interval_us <= 0.0 {
        return;
    }
    let Some(interval) =
        pystr::floordiv_f64(interval_us, 1_000_000.0).and_then(pystr::py_int_from_f64)
    else {
        return;
    };
    let mut total: i64 = 0;
    for st in calls {
        let reps = if st.args.len() > 7 {
            match consteval::value_of(st.args[7].as_deref(), env) {
                Ok(v) => Some(v),
                Err(_) => return,
            }
        } else {
            None
        };
        let reps = match reps {
            Some(ConstValue::Int(i)) if (1..=16).contains(&i) => i,
            Some(ConstValue::Float(f) | ConstValue::CFloat(f))
                if f.is_finite() && f.fract() == 0.0 && (1.0..=16.0).contains(&f) =>
            {
                match pystr::py_int_from_f64(f) {
                    Some(i) => i,
                    None => return,
                }
            }
            _ => return,
        };
        // AVWChan / MuxChannel: outside 1..32 the compiler adds `Bad parameter(s)`
        for position in [5_usize, 6] {
            let Some(slot) = st.args.get(position) else {
                return;
            };
            let Ok(channel) = consteval::value_of(slot.as_deref(), env) else {
                return;
            };
            let ok = match channel {
                ConstValue::Int(i) => (1..=32).contains(&i),
                ConstValue::Float(f) | ConstValue::CFloat(f) => (1.0..=32.0).contains(&f),
                _ => false,
            };
            if !ok {
                return;
            }
        }
        total += reps;
        if 2 * total > interval {
            diags.error_basic(
                st.base.node.line,
                &format!(
                    "Scan interval {interval} secs is too small; AVW200 measurement time is {} secs (2 secs per sensor).",
                    2 * total
                ),
                &st.base.node.file,
            );
        }
    }
}

/// Port of checker/instr_rules.py::check_mqtt_publish_payload
pub fn check_mqtt_publish_payload(
    program: &ProgramRef,
    profile: &'static Profile,
    diags: &mut Diagnostics,
) {
    if profile.name != "cr300" && profile.name != "cr350" {
        return;
    }
    let ctx = context::of(program, Some(profile), None);
    let types = ctx.serialin_types();
    let string_consts = ctx.string_consts();
    for stmt in &program.statements {
        let Stmt::Assignment(stmt) = stmt.as_ref() else {
            continue;
        };
        let Some(Expr::Index(call)) = stmt.value.as_deref() else {
            continue;
        };
        if pystr::upper(&call.name) != "MQTTPUBLISH" || call.args.len() < 2 {
            continue;
        }
        let payload = call.args[1].as_deref();
        if matches!(payload, Some(Expr::String(_))) {
            continue;
        }
        if let Some(Expr::Name(n)) = payload {
            let upper = pystr::upper(&n.name);
            if string_consts.contains(&upper)
                || types.get(&upper).map(String::as_str) == Some("STRING")
            {
                continue;
            }
        }
        if matches!(payload, Some(Expr::Number(_) | Expr::Name(_)))
            || _numeric_expression(payload, &types)
        {
            diags.error_basic(
                stmt.base.node.line,
                "A string or string variable is required",
                &stmt.base.node.file,
            );
        }
    }
}

/// Port of checker/instr_rules.py::_numeric_expression
pub fn _numeric_expression(expr: Option<&Expr>, types: &OrderedMap<String, String>) -> bool {
    match expr {
        Some(Expr::Number(_)) => true,
        Some(Expr::Name(n)) => !matches!(
            types.get(&pystr::upper(&n.name)).map(String::as_str),
            None | Some("STRING")
        ),
        Some(Expr::Unary(u)) => u.op == "-" && _numeric_expression(u.operand.as_deref(), types),
        Some(Expr::Binary(b)) => {
            matches!(b.op.as_str(), "+" | "-" | "*" | "/" | "^")
                && _numeric_expression(b.left.as_deref(), types)
                && _numeric_expression(b.right.as_deref(), types)
        }
        _ => false,
    }
}

/// Port of checker/instr_rules.py::_constant_name_text
pub fn _constant_name_text(arg: Option<&Expr>, env: &Env, depth: i64) -> Option<String> {
    let arg = arg?;
    if depth > 16 {
        return None;
    }
    match arg {
        Expr::String(s) => Some(s.value.clone()),
        Expr::Name(n) => match env.consts.borrow().get(&pystr::upper(&n.name)) {
            Some(ConstValue::Str(s)) => Some(s.clone()),
            _ => None,
        },
        Expr::Binary(b) if b.op == "+" => {
            let left = _constant_name_text(b.left.as_deref(), env, depth + 1);
            let right = _constant_name_text(b.right.as_deref(), env, depth + 1);
            Some(left? + &right?)
        }
        _ => None,
    }
}

/// Port of checker/instr_rules.py::_check_port_index
pub fn _check_port_index(
    stmt: &Instruction,
    key: &str,
    index: Option<&str>,
    cfg: Option<&Json>,
    env: &Env,
    profile: &Profile,
    found: &mut Diagnostics,
) {
    static INT_RE: LazyLock<pyre::Regex> = LazyLock::new(|| pyre::Regex::new(r"0|-?[1-9]\d*", 0));
    static ALPHA_RE: LazyLock<pyre::Regex> = LazyLock::new(|| pyre::Regex::new(r"[A-Za-z]", 0));
    static IDENT_RE: LazyLock<pyre::Regex> = LazyLock::new(|| pyre::Regex::new(r"[A-Za-z]\w*", 0));
    let Some(cfg) = cfg else {
        return;
    };
    if cfg.is_empty() {
        return;
    }
    let (Some(index), Some(limit)) = (index, cfg.get(key)) else {
        return;
    };
    if !index.ends_with(')') {
        return;
    }
    // only the first word counts: `COMC1 x` is COMC1, `C  OMC1` and `COMC1+1` are unknown, so 0
    // (oracle, round 55)
    let index = pystr::split_ws(pystr::slice(index, None, Some(-1)), None)
        .first()
        .copied()
        .unwrap_or("");
    let value = if INT_RE.fullmatch(index).is_some() {
        // ponytail: an index beyond i64 does not parse here (Python prints the big int)
        let Some(v) = pystr::parse_int(index) else {
            return;
        };
        v
    } else if ALPHA_RE.match_(index).is_some()
        && !env.consts.borrow().contains_key(&pystr::upper(index))
    {
        let symbol = if IDENT_RE.fullmatch(index).is_some() {
            profile.symbols().get(&pystr::upper(index))
        } else {
            None
        };
        symbol.map_or(0, |s| s.value)
    } else {
        return;
    };
    let Some(limit) = limit.as_i64() else {
        return;
    };
    if !(1..=limit).contains(&value) {
        found.error_basic(
            stmt.base.node.line,
            &format!("Illegal dimension {value}"),
            &stmt.base.node.file,
        );
    }
}

/// Port of checker/instr_rules.py::check_settings_fields
pub fn check_settings_fields(
    program: &ProgramRef,
    profile: &'static Profile,
    diags: &mut Diagnostics,
    found: Option<&mut Diagnostics>,
    env: Option<&Env>,
) {
    let own_env = Env::with_consts(profile, Rc::new(RefCell::new(program.consts.clone())));
    let env = env.unwrap_or(&own_env);
    let fields = profile.settings_fields();
    let trim = fields
        .get("trim_trailing_blanks")
        .and_then(Json::as_bool)
        .unwrap_or(false);
    let names_of = |key: &str| -> BTreeSet<String> {
        fields
            .get(key)
            .and_then(Json::as_array)
            .unwrap_or(&[])
            .iter()
            .filter_map(Json::as_str)
            .map(pystr::upper)
            .collect()
    };
    let settings = names_of("settings");
    let both: BTreeSet<String> = settings.union(&names_of("status")).cloned().collect();
    let mut stop: Option<(String, i64)> = None;
    for d in &diags.items {
        if d.severity == "error"
            && d.kind == "could_not_find_table"
            && d.line != 0
            && stop.as_ref().is_none_or(|s| d.line < s.1)
        {
            stop = Some((d.file.clone(), d.line));
        }
    }
    let target = match found {
        Some(f) => f,
        None => diags,
    };
    let ctx = context::of(program, None, None);
    let const_tables = ctx.const_table_fields();
    for stmt in &program.statements {
        let Stmt::Instruction(stmt) = stmt.as_ref() else {
            continue;
        };
        let kw = pystr::upper(&stmt.keyword);
        if kw == "ENDPROG" {
            break;
        }
        if stmt.args.is_empty() {
            continue;
        }
        let label = match kw.as_str() {
            "SETSETTING" => "Settings",
            "SETSTATUS" => "Status or Settings",
            _ => continue,
        };
        if !profile.instructions().contains(&kw) {
            continue;
        }
        let line = stmt.base.node.line;
        let file = &stmt.base.node.file;
        if let Some((stop_file, stop_line)) = &stop
            && file == stop_file
            && line > *stop_line
        {
            continue;
        }
        let Some(name) = _constant_name_text(stmt.args[0].as_deref(), env, 0) else {
            // a number, variable or other non-string-constant name
            // (docs/checker.md#settings-name-must-be-constant-round-36)
            if stmt.args[0].is_some() {
                target.error_basic(line, "Status Field name must be constant.", file);
            }
            continue;
        };
        if name.contains('.') {
            let (table, field) = name.split_once('.').expect("contains '.'");
            let field_name = pystr::strip(field.split('(').next().unwrap_or(""));
            match const_tables.get(&pystr::upper(pystr::strip(table))) {
                None => target.error_basic(line, &format!("Cannot find {table}"), file),
                Some(fields_for_table) => {
                    let upper = pystr::upper(field_name);
                    let apply = upper == "APPLYANDRESTART" && !fields_for_table.is_empty();
                    if !fields_for_table.contains(&upper) && !apply {
                        target.error_basic(line, &format!("Cannot find {field_name}"), file);
                    }
                }
            }
            continue;
        }
        let (name, index) = if let Some((head, tail)) = name.split_once('(') {
            if head.is_empty() {
                continue;
            }
            (head.to_string(), Some(tail.to_string()))
        } else {
            (name, None)
        };
        let key = pystr::upper(if trim {
            pystr::rstrip_chars(&name, " \t")
        } else {
            &name
        });
        if (if kw == "SETSTATUS" { &both } else { &settings }).contains(&key) {
            _check_port_index(
                stmt,
                &key,
                index.as_deref(),
                fields.get("index_size"),
                env,
                profile,
                target,
            );
            continue;
        }
        let shown = if name.is_empty() {
            fields
                .get("empty_name")
                .and_then(Json::as_str)
                .unwrap_or("")
        } else {
            &name
        };
        target.error_basic(line, &format!("{shown} is not a {label} Field Name."), file);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_text;
    use crate::profiles;

    fn run(
        src: &str,
        profile: &'static Profile,
        f: impl Fn(&ProgramRef, &Env, &mut Diagnostics),
    ) -> Vec<(i64, String)> {
        let prog = parse_text(src, "t.crb", None, Some(profile)).0;
        let env = Env::with_consts(profile, Rc::new(RefCell::new(prog.consts.clone())));
        let mut diags = Diagnostics::new();
        f(&prog, &env, &mut diags);
        diags
            .items
            .iter()
            .map(|d| (d.line, d.message.clone()))
            .collect()
    }

    // Expected values observed from the Python implementation.
    #[test]
    fn fieldcal_matches_python() {
        let src = "Public FA(2), K(3), M(2), CM(2), S As String * 10, a
Public X
BeginProg
  FieldCal(0,M(),1,CM(),1,0,K(),1,1)
  FieldCal(2,M(),3,\"x\",1.5,0,K(),1,1)
  FieldCal(1,M(),1,CM(),CM(\"a\"),0,K(),1,1)
  FieldCal(7,M(),1,CM(),1,0,K(),1,1)
  FieldCal(2,M(),1,CM(),CM(1),0,K(5),1,1)
EndProg
";
        let profile = profiles::get("cr1000x").unwrap();
        let got = run(src, profile, |p, e, d| {
            check_fieldcal_variable_args(p, profile, d, Some(e));
        });
        let off =
            "FieldCal Offset must be a declared variable that can be resolved during compilation.";
        let mult =
            "FieldCal Mult must be a declared variable that can be resolved during compilation.";
        let want = vec![
            (4, off),
            (5, "Illegal Reps: must be 1 or 2."),
            (5, mult),
            (5, off),
            (6, off),
            (7, "Invalid function option."),
            (
                8,
                "FieldCal KnownVar must be a declared public variable that can be resolved during compilation.",
            ),
        ];
        let want: Vec<(i64, String)> = want.into_iter().map(|(l, m)| (l, m.to_string())).collect();
        assert_eq!(got, want);
    }

    #[test]
    fn settings_and_mqtt_match_python() {
        let src = "Public X, S As String * 20
BeginProg
  SetSetting(\"StationName\", \"x\")
  SetSetting(\"Bogus\", \"x\")
  SetSetting(\"Baudrate(9)\", \"x\")
  SetSetting(5, \"x\")
  SetStatus(\"Foo.Bar\", 1)
  SetSetting(\"Security(2)\", 1)
EndProg
";
        let profile = profiles::get("cr1000x").unwrap();
        let got = run(src, profile, |p, _, d| {
            check_settings_fields(p, profile, d, None, None);
        });
        let want = [
            (4, "Bogus is not a Settings Field Name."),
            (6, "Status Field name must be constant."),
            (7, "Cannot find Foo"),
        ];
        assert_eq!(got, want.map(|(l, m)| (l, m.to_string())));

        let src = "Public X, S As String * 20, R
BeginProg
  R = MQTTPublish(1,X,\"a\",1,1,1)
  R = MQTTPublish(1,S,\"a\",1,1,1)
  R = MQTTPublish(1,X+1,\"a\",1,1,1)
  R = MQTTPublish(1,5,\"a\",1,1,1)
EndProg
";
        let profile = profiles::get("cr300").unwrap();
        let got = run(src, profile, |p, _, d| {
            check_mqtt_publish_payload(p, profile, d);
        });
        let lines: Vec<i64> = got.iter().map(|g| g.0).collect();
        assert_eq!(lines, [3, 5, 6]);
    }

    #[test]
    fn epi_and_cdm_match_python() {
        let src = "Public X(2), R(3)
BeginProg
  Scan(1,mSec,0,0)
    CDM_BridgeFilt(R(),1,224,1,1,1,1,1,1,1,1,1,1)
  NextScan
  Scan(20,mSec,3,0)
    CDM_BridgeFilt(R(),1,224,1,1,1,1,1,1,1,1,1,1)
  NextScan
EndProg
";
        let profile = profiles::get("cr6").unwrap();
        let got = run(src, profile, |p, e, d| {
            check_epi_scan_buffers(p, profile, d, e);
        });
        let m = "Insufficient Scan buffers to read EPI data, minimum is 100.".to_string();
        assert_eq!(got, [(4, m.clone()), (7, m)]);

        let src = "Public X(2), R(3)
BeginProg
  CDM_VoltSE(R(),1,100,1,1,1,1,1,1,1,1)
  CDM_VoltSE(R(),1,99,1,1,1,1,1,1,1,1)
  CDM_VoltSE(R(),1,100,500,1,1,1,1,1,1,1)
  CDM_MuxSelect(1,220,1,5,1,10)
  CDM_MuxSelect(2,220,1,1,1,0)
  CDM_BRHALF3W(R(),1,220,3,1,1,1,1,50,1,1,1)
EndProg
";
        let got = run(src, profile, |p, e, d| {
            check_cdm_address_types(p, profile, d, e);
        });
        let want = [
            (6, "CPI Address must be 1..120."),
            (6, "CDM_MuxSel instruction not supported by module NULL."),
            (7, "CPI Address must be 1..120."),
            (7, "Address 1 already used with a different type CDM."),
        ];
        assert_eq!(got, want.map(|(l, m)| (l, m.to_string())));
    }
}
