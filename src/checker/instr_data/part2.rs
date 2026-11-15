//! Port of `checker/instr_data.py` (lines 760-1473).

#![allow(dead_code, unused_variables, unused_imports)]

use std::collections::BTreeSet;
use std::rc::Rc;

use crate::diagnostics::{DiagRef, Diagnostic, Diagnostics};
use crate::profiles::Profile;
use crate::pystr;
use crate::syntax::{
    Assignment, Call, Case, ConstDecl, Declaration, Expr, ExprRef, Index, Instruction, Juxtaposed,
    Member, Number, OrderedMap, Program, ProgramRef, Stmt, StmtRef, SubDef, Unparsed, VarDecl,
};

/// Port of `checker/instr_data.py::check_units_targets`.
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

/// Port of `checker/instr_data.py::check_table_and_menu_placement`.
pub fn check_table_and_menu_placement(
    _program: &Program,
    _profile: &Profile,
    _diags: &mut Diagnostics,
) -> Option<(String, i64)> {
    todo!("checker/instr_data.py::check_table_and_menu_placement (omitted: env: untyped)")
}

/// Port of `checker/instr_data.py::_boolean_pick_ok`.
pub fn _boolean_pick_ok(_token: &str) -> bool {
    todo!("checker/instr_data.py::_boolean_pick_ok (omitted: env: untyped)")
}

/// Port of `checker/instr_data.py::_pick_tokens`.
pub fn _pick_tokens(_stmt: &Instruction) -> Option<Vec<String>> {
    todo!("checker/instr_data.py::_pick_tokens")
}

/// Port of `checker/instr_data.py::_check_boolean_picklist`.
pub fn _check_boolean_picklist(_stmt: &Instruction, _diags: &mut Diagnostics) {
    todo!("checker/instr_data.py::_check_boolean_picklist (omitted: env: untyped)")
}

/// Port of `checker/instr_data.py::_pick_value_bad`.
pub fn _pick_value_bad(_token: &str, _kind: &str) -> Option<String> {
    todo!(
        "checker/instr_data.py::_pick_value_bad (omitted: env: untyped, program: untyped, profile: untyped, variables: untyped)"
    )
}

/// Port of `checker/instr_data.py::_check_pick_consts`.
pub fn _check_pick_consts() {
    todo!(
        "checker/instr_data.py::_check_pick_consts (omitted: stmt: untyped, kind: untyped, env: untyped, program: untyped, profile: untyped, variables: untyped, diags: untyped, skip_first: untyped)"
    )
}

/// Port of `checker/instr_data.py::_destination_expression`.
pub fn _destination_expression(_expr: Option<&Expr>) -> bool {
    todo!("checker/instr_data.py::_destination_expression")
}

/// Port of `checker/instr_data.py::constant_destination`.
pub fn constant_destination() -> bool {
    todo!(
        "checker/instr_data.py::constant_destination (omitted: expr: untyped, env: untyped, variables: untyped)"
    )
}

/// Port of `checker/instr_data.py::_destination_env`.
pub fn _destination_env() {
    todo!("checker/instr_data.py::_destination_env (omitted: env: consteval.Env)")
}

/// Port of `checker/instr_data.py::_reps_value`.
pub fn _reps_value() -> Option<i64> {
    todo!("checker/instr_data.py::_reps_value (omitted: expr: untyped, env: untyped)")
}

/// Port of `checker/instr_data.py::check_stage2a`.
pub fn check_stage2a(_program: &Program, _profile: &Profile, _diags: &mut Diagnostics) -> bool {
    todo!("checker/instr_data.py::check_stage2a (omitted: env: untyped)")
}

/// Port of `checker/instr_data.py::check_table_reps_late`.
pub fn check_table_reps_late(_program: &Program, _profile: &Profile, _diags: &mut Diagnostics) {
    todo!("checker/instr_data.py::check_table_reps_late (omitted: env: untyped)")
}

/// Port of `checker/instr_data.py::_report_undefined_source`.
pub fn _report_undefined_source(
    _st: &Instruction,
    _known: &BTreeSet<String>,
    _profile: &Profile,
    _diags: &mut Diagnostics,
) -> bool {
    todo!("checker/instr_data.py::_report_undefined_source (omitted: record: dict)")
}

/// Port of `checker/instr_data.py::_is_struct_typed`.
pub fn _is_struct_typed(_program: &Program, _name: &str) -> bool {
    todo!("checker/instr_data.py::_is_struct_typed")
}

/// Port of `checker/instr_data.py::_short_array`.
pub fn _short_array(_reps: i64) -> bool {
    todo!("checker/instr_data.py::_short_array (omitted: arg: untyped, dims: dict, env: untyped)")
}

/// Port of `checker/instr_data.py::_alias_sources`.
pub fn _alias_sources(_program: &Program) {
    todo!("checker/instr_data.py::_alias_sources (omitted: tables: untyped, env: untyped)")
}

/// Port of `checker/instr_data.py::_wind_options`.
pub fn _wind_options(
    _st: &Instruction,
    _reps: Option<i64>,
    _diags: &mut Diagnostics,
    _short_speed: bool,
) -> bool {
    todo!("checker/instr_data.py::_wind_options (omitted: env: untyped)")
}

/// Port of `checker/instr_data.py::check_constant_destinations`.
pub fn check_constant_destinations(
    _program: &Program,
    _profile: &Profile,
    _diags: &mut Diagnostics,
) {
    todo!("checker/instr_data.py::check_constant_destinations (omitted: env: untyped)")
}

/// Port of `checker/instr_data.py::drop_unreached_destinations`.
pub fn drop_unreached_destinations(
    _program: &Program,
    _profile: &Profile,
    _diags: &mut Diagnostics,
) {
    todo!("checker/instr_data.py::drop_unreached_destinations (omitted: env: untyped)")
}
