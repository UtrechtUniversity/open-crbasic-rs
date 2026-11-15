//! Port of `checker/context.py`.

#![allow(dead_code, unused_variables, unused_imports)]

use std::collections::BTreeSet;
use std::rc::Rc;

use crate::diagnostics::{DiagRef, Diagnostic, Diagnostics};
use crate::profiles::Profile;
use crate::syntax::{
    Assignment, Call, Case, ConstDecl, Declaration, Expr, ExprRef, Index, Instruction, Juxtaposed,
    Member, Number, OrderedMap, Program, ProgramRef, Stmt, StmtRef, SubDef, Unparsed, VarDecl,
};

/// Port of `checker/context.py::DimCounts`.
#[derive(Debug, Default)]
pub struct DimCounts {}

/// Port of `checker/context.py::ScopeTables`.
#[derive(Debug, Default)]
pub struct ScopeTables {}

impl ScopeTables {
    /// Port of `checker/context.py::ScopeTables::__init__`.
    pub fn __init__(
        &self,
        _dims: &OrderedMap<String, Vec<i64>>,
        _types: &OrderedMap<String, String>,
        _arrays: &BTreeSet<String>,
    ) {
        todo!("checker/context.py::ScopeTables::__init__")
    }
}

/// Port of `checker/context.py::_ScopeBase`.
#[derive(Debug, Default)]
pub struct ScopeBase {}

/// Port of `checker/context.py::_RoutineLayout`.
#[derive(Debug, Default)]
pub struct RoutineLayout {}

/// Port of `checker/context.py::_with_inline`.
pub fn _with_inline(_program: &Program) -> ProgramRef {
    todo!("checker/context.py::_with_inline")
}

/// Port of `checker/context.py::_names_through_bad_as_clause`.
pub fn _names_through_bad_as_clause(_stmt: &Unparsed) -> BTreeSet<String> {
    todo!("checker/context.py::_names_through_bad_as_clause")
}

/// Port of `checker/context.py::_declared_names_in_statements`.
pub fn _declared_names_in_statements(_statements: &[StmtRef]) -> BTreeSet<String> {
    todo!("checker/context.py::_declared_names_in_statements")
}

/// Port of `checker/context.py::_declared_all_dims`.
pub fn _declared_all_dims(_statements: &[StmtRef]) -> OrderedMap<String, Vec<i64>> {
    todo!("checker/context.py::_declared_all_dims (omitted: env: consteval.Env)")
}

/// Port of `checker/context.py::_ScopeLookup`.
#[derive(Debug, Default)]
pub struct ScopeLookup {}

impl ScopeLookup {
    /// Port of `checker/context.py::_ScopeLookup::__init__`.
    pub fn __init__(&self, _ctx: &Context, _scalars: bool, _parameters: bool) {
        todo!("checker/context.py::_ScopeLookup::__init__")
    }

    /// Port of `checker/context.py::_ScopeLookup::__call__`.
    pub fn __call__(&self, _stmt: &Stmt) -> ScopeTables {
        todo!("checker/context.py::_ScopeLookup::__call__")
    }

    /// Port of `checker/context.py::_ScopeLookup::for_statement`.
    pub fn for_statement(&self, _stmt: &Stmt) -> ScopeTables {
        todo!("checker/context.py::_ScopeLookup::for_statement")
    }
}

/// Port of `checker/context.py::Context`.
#[derive(Debug, Default)]
pub struct Context {}

impl Context {
    /// Port of `checker/context.py::Context::__init__`.
    pub fn __init__(&self, _program: &Program, _profile: Option<&Profile>) {
        todo!(
            "checker/context.py::Context::__init__ (omitted: env: consteval.Env | None, _routine_bases: dict[int, _ScopeBase] | None, _scope_cache: dict[tuple[int, bool, bool], ScopeTables] | None)"
        )
    }

    /// Port of `checker/context.py::Context::env`.
    pub fn env(&self) {
        todo!("checker/context.py::Context::env")
    }

    /// Port of `checker/context.py::Context::flat`.
    pub fn flat(&self) -> ProgramRef {
        todo!("checker/context.py::Context::flat")
    }

    /// Port of `checker/context.py::Context::positions`.
    pub fn positions(&self) {
        todo!("checker/context.py::Context::positions")
    }

    /// Port of `checker/context.py::Context::_routine_layout`.
    pub fn _routine_layout(&self) {
        todo!("checker/context.py::Context::_routine_layout")
    }

    /// Port of `checker/context.py::Context::enclosing_sub`.
    pub fn enclosing_sub(&self, _stmt: &Stmt) -> Option<&SubDef> {
        todo!("checker/context.py::Context::enclosing_sub")
    }

    /// Port of `checker/context.py::Context::declared_subs`.
    pub fn declared_subs(&self) -> BTreeSet<String> {
        todo!("checker/context.py::Context::declared_subs")
    }

    /// Port of `checker/context.py::Context::system_table_names`.
    pub fn system_table_names(&self) -> BTreeSet<String> {
        todo!("checker/context.py::Context::system_table_names")
    }

    /// Port of `checker/context.py::Context::ess_variable_names`.
    pub fn ess_variable_names(&self) -> BTreeSet<String> {
        todo!("checker/context.py::Context::ess_variable_names")
    }

    /// Port of `checker/context.py::Context::_variable_types_pair`.
    pub fn _variable_types_pair(&self) {
        todo!("checker/context.py::Context::_variable_types_pair")
    }

    /// Port of `checker/context.py::Context::variable_types`.
    pub fn variable_types(&self, _prefer_last: bool) -> OrderedMap<String, String> {
        todo!("checker/context.py::Context::variable_types")
    }

    /// Port of `checker/context.py::Context::const_table_fields`.
    pub fn const_table_fields(&self) {
        todo!("checker/context.py::Context::const_table_fields")
    }

    /// Port of `checker/context.py::Context::declared_variable_names`.
    pub fn declared_variable_names(&self) -> BTreeSet<String> {
        todo!("checker/context.py::Context::declared_variable_names")
    }

    /// Port of `checker/context.py::Context::declared_names`.
    pub fn declared_names(&self) -> BTreeSet<String> {
        todo!("checker/context.py::Context::declared_names")
    }

    /// Port of `checker/context.py::Context::statement_scopes`.
    pub fn statement_scopes(&self) {
        todo!("checker/context.py::Context::statement_scopes")
    }

    /// Port of `checker/context.py::Context::not_yet_declared`.
    pub fn not_yet_declared(&self) {
        todo!("checker/context.py::Context::not_yet_declared")
    }

    /// Port of `checker/context.py::Context::routine_scopes`.
    pub fn routine_scopes(&self) {
        todo!("checker/context.py::Context::routine_scopes")
    }

    /// Port of `checker/context.py::Context::declared_dim0_sizes`.
    pub fn declared_dim0_sizes(&self) -> OrderedMap<String, i64> {
        todo!("checker/context.py::Context::declared_dim0_sizes")
    }

    /// Port of `checker/context.py::Context::declared_all_dims`.
    pub fn declared_all_dims(&self) -> OrderedMap<String, Vec<i64>> {
        todo!("checker/context.py::Context::declared_all_dims")
    }

    /// Port of `checker/context.py::Context::_dim_counts_pair`.
    pub fn _dim_counts_pair(&self) {
        todo!("checker/context.py::Context::_dim_counts_pair")
    }

    /// Port of `checker/context.py::Context::dim_counts`.
    pub fn dim_counts(&self, _strings: bool) -> DimCounts {
        todo!("checker/context.py::Context::dim_counts")
    }

    /// Port of `checker/context.py::Context::scoped_view`.
    pub fn scoped_view(&self, _stmt: &Stmt, _publics: bool) -> ProgramRef {
        todo!("checker/context.py::Context::scoped_view")
    }

    /// Port of `checker/context.py::Context::_build_scope`.
    pub fn _build_scope(&self, _routine: Option<&SubDef>) {
        todo!("checker/context.py::Context::_build_scope")
    }

    /// Port of `checker/context.py::Context::_lookup_scope`.
    pub fn _lookup_scope(&self, _stmt: &Stmt, _scalars: bool, _parameters: bool) -> ScopeTables {
        todo!("checker/context.py::Context::_lookup_scope")
    }

    /// Port of `checker/context.py::Context::scope_tables`.
    pub fn scope_tables(&self, _scalars: bool, _parameters: bool) {
        todo!("checker/context.py::Context::scope_tables")
    }

    /// Port of `checker/context.py::Context::out_of_scope_nodes`.
    pub fn out_of_scope_nodes(&self, _resolvable: &BTreeSet<String>) -> BTreeSet<usize> {
        todo!("checker/context.py::Context::out_of_scope_nodes")
    }

    /// Port of `checker/context.py::Context::known_names`.
    pub fn known_names(&self) -> crate::checker::expressions::KnownNames {
        todo!("checker/context.py::Context::known_names")
    }

    /// Port of `checker/context.py::Context::dims_state`.
    pub fn dims_state(&self) -> crate::checker::expressions::DimsState {
        todo!("checker/context.py::Context::dims_state")
    }

    /// Port of `checker/context.py::Context::float_names`.
    pub fn float_names(&self) -> BTreeSet<String> {
        todo!("checker/context.py::Context::float_names")
    }

    /// Port of `checker/context.py::Context::literal_names`.
    pub fn literal_names(&self) -> BTreeSet<String> {
        todo!("checker/context.py::Context::literal_names")
    }

    /// Port of `checker/context.py::Context::string_consts`.
    pub fn string_consts(&self) -> BTreeSet<String> {
        todo!("checker/context.py::Context::string_consts")
    }

    /// Port of `checker/context.py::Context::alias_names`.
    pub fn alias_names(&self) -> BTreeSet<String> {
        todo!("checker/context.py::Context::alias_names")
    }

    /// Port of `checker/context.py::Context::_serialin_types_map`.
    pub fn _serialin_types_map(&self) -> OrderedMap<String, String> {
        todo!("checker/context.py::Context::_serialin_types_map")
    }

    /// Port of `checker/context.py::Context::serialin_types`.
    pub fn serialin_types(&self) -> OrderedMap<String, String> {
        todo!("checker/context.py::Context::serialin_types")
    }

    /// Port of `checker/context.py::Context::measurement_aliases`.
    pub fn measurement_aliases(&self) {
        todo!("checker/context.py::Context::measurement_aliases")
    }
}

/// Port of `checker/context.py::_stringish_const`.
pub fn _stringish_const(_expr: Option<&Expr>, _str_consts: &BTreeSet<String>) -> bool {
    todo!("checker/context.py::_stringish_const")
}

/// Port of `checker/context.py::activate`.
pub fn activate(_ctx: &Context) {
    todo!("checker/context.py::activate")
}

/// Port of `checker/context.py::of`.
pub fn of(_program: &Program, _profile: Option<&Profile>) -> Rc<Context> {
    todo!("checker/context.py::of (omitted: env: consteval.Env | None)")
}
