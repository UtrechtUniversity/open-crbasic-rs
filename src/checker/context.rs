//! Port of `checker/context.py`.

#![allow(dead_code, unused_variables, clippy::used_underscore_items)]

use std::cell::{OnceCell, RefCell};
use std::collections::{BTreeSet, HashMap};
use std::rc::Rc;
use std::sync::LazyLock;

use crate::consteval::{self, Env};
use crate::diagnostics::{Positions, PositionsGuard};
use crate::profiles::Profile;
use crate::pyre;
use crate::pystr;
use crate::syntax::{
    ConstValue, Expr, ExprRef, OrderedMap, Program, ProgramRef, Stmt, StmtRef, ends_routine,
    expr_id, program_id, replace_stmt, stmt_id,
};

use super::expressions::{DimsState, KnownNames};
use super::{expressions, instr_measure, longconv};

// TODO(P3-h): move to `symbols.rs` when `checker/symbols.py` is ported.
const NAME_DECLARING_INSTRUCTIONS: &[&str] =
    &["DATATABLE", "STRUCTURETYPE", "OBJECTTYPE", "CONSTTABLE"];
const ALWAYS_KNOWN_SYSTEM_TABLES: &[&str] = &["STATUS", "PUBLIC", "SETTINGS"];
static SNMP_VARIABLE_RE: LazyLock<pyre::Regex> = LazyLock::new(|| {
    pyre::Regex::new(
        r"^\s*SNMPVariable\s*\(\s*([A-Za-z_][A-Za-z0-9_]*)",
        pyre::IGNORECASE,
    )
});

static BAD_AS_CLAUSE_RE: LazyLock<pyre::Regex> = LazyLock::new(|| {
    pyre::Regex::new(
        r"\s*(?:Public|Dim|Static)\s+(.*)$",
        pyre::IGNORECASE | pyre::DOTALL,
    )
});
static IDENT_RE: LazyLock<pyre::Regex> =
    LazyLock::new(|| pyre::Regex::new(r"\s*([A-Za-z_][A-Za-z0-9_]*)", 0));

/// Port of `checker/context.py::DimCounts`.
#[derive(Debug, Clone, Default)]
pub struct DimCounts {
    pub counts: OrderedMap<String, i64>,
    pub strings: BTreeSet<String>,
}

/// Port of `checker/context.py::ScopeTables`.
#[derive(Debug, Clone, Default)]
pub struct ScopeTables {
    pub dims: OrderedMap<String, Vec<i64>>,
    pub types: OrderedMap<String, String>,
    pub arrays: BTreeSet<String>,
}

/// Port of `checker/context.py::_ScopeBase`.
#[derive(Debug, Clone, Default)]
pub struct ScopeBase {
    pub dims: OrderedMap<String, Vec<i64>>,
    pub arrays: BTreeSet<String>,
    pub array_types: OrderedMap<String, String>,
    pub scalar_types: OrderedMap<String, String>,
}

/// Port of `checker/context.py::_RoutineLayout`.
#[derive(Debug, Clone, Default)]
pub struct RoutineLayout {
    pub owners: HashMap<usize, Option<StmtRef>>,
    pub groups: Vec<(Option<StmtRef>, Vec<StmtRef>)>,
    pub global_stmts: Vec<StmtRef>,
    pub routine_stmts: HashMap<usize, Vec<StmtRef>>,
    pub all_public_in_routines: Vec<(StmtRef, StmtRef)>,
}

/// Local helper mirroring `source.py::_split_top_commas` until `source.rs` lands.
fn split_top_commas(text: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut depth: i64 = 0;
    let mut start: usize = 0;
    let mut in_string = false;
    for (k, ch) in text.chars().enumerate() {
        if ch == '"' {
            in_string = !in_string;
        } else if !in_string {
            if ch == '(' || ch == '{' {
                depth += 1;
            } else if ch == ')' || ch == '}' {
                depth -= 1;
            } else if ch == ',' && depth == 0 {
                parts.push(
                    pystr::slice(text, i64::try_from(start).ok(), i64::try_from(k).ok())
                        .to_string(),
                );
                start = k + 1;
            }
        }
    }
    parts.push(pystr::slice(text, i64::try_from(start).ok(), None).to_string());
    parts
}

/// Local helper mirroring `symbols.py::_walk_names` until `symbols.rs` is ported.
fn walk_names(expr: Option<&Expr>, out: &mut Vec<String>) {
    let Some(e) = expr else {
        return;
    };
    match e {
        Expr::Name(n) => out.push(n.name.clone()),
        Expr::Unary(u) => walk_names(u.operand.as_deref(), out),
        Expr::Binary(b) => {
            walk_names(b.left.as_deref(), out);
            walk_names(b.right.as_deref(), out);
        }
        Expr::Index(ix) => {
            out.push(ix.name.clone());
            for a in &ix.args {
                walk_names(a.as_deref(), out);
            }
        }
        Expr::Member(m) => walk_names(m.base.as_deref(), out),
        Expr::ArrayLiteral(a) => {
            for item in &a.items {
                walk_names(item.as_deref(), out);
            }
        }
        Expr::Juxtaposed(j) => {
            for item in &j.items {
                walk_names(item.as_deref(), out);
            }
        }
        Expr::Number(_) | Expr::String(_) => {}
    }
}

/// Local helper mirroring `symbols.py::_stmt_exprs` until `symbols.rs` is ported.
fn stmt_exprs(stmt: &Stmt) -> Vec<ExprRef> {
    let mut out = Vec::new();
    match stmt {
        Stmt::Declaration(d) => {
            for v in &d.vars {
                out.extend(v.dims.iter().cloned());
                if let Some(ss) = &v.string_size {
                    out.push(ss.clone());
                }
                if let Some(init) = &v.init {
                    out.extend(init.iter().flatten().cloned());
                }
            }
        }
        Stmt::ConstDecl(c) => {
            if let Some(v) = &c.value {
                out.push(v.clone());
            }
        }
        Stmt::Assignment(a) => {
            if let Some(t) = &a.target {
                out.push(t.clone());
            }
            if let Some(v) = &a.value {
                out.push(v.clone());
            }
        }
        Stmt::Instruction(i) => {
            out.extend(i.args.iter().flatten().cloned());
        }
        Stmt::If(i) => {
            if let Some(c) = &i.cond {
                out.push(c.clone());
            }
        }
        Stmt::ElseIf(e) => {
            if let Some(c) = &e.cond {
                out.push(c.clone());
            }
        }
        Stmt::For(f) => {
            for e in [&f.var, &f.start, &f.end, &f.step].into_iter().flatten() {
                out.push(e.clone());
            }
        }
        Stmt::DoLoop(d) => {
            if let Some(c) = &d.cond {
                out.push(c.clone());
            }
        }
        Stmt::Case(c) => {
            for item in &c.items {
                match item {
                    crate::syntax::CaseItem::Value(e) | crate::syntax::CaseItem::Is(_, e) => {
                        out.push(e.clone());
                    }
                    crate::syntax::CaseItem::Range(lo, hi) => {
                        out.push(lo.clone());
                        out.push(hi.clone());
                    }
                }
            }
        }
        Stmt::SubDef(s) => {
            for p in &s.params {
                if let Some(ss) = &p.string_size {
                    out.push(ss.clone());
                }
                if let Some(d) = &p.default {
                    out.push(d.clone());
                }
                out.extend(p.dim_exprs.iter().cloned());
            }
        }
        Stmt::Call(c) => {
            out.extend(c.args.iter().flatten().cloned());
            if let Some(te) = &c.tail_expr {
                out.push(te.clone());
            }
        }
        Stmt::Unparsed(u) => {
            if let Some(e) = &u.expr {
                out.push(e.clone());
            }
        }
        Stmt::Label(_) => {}
    }
    out
}

fn collect_name_refs(expr: &ExprRef, out: &mut Vec<(usize, String)>) {
    match &**expr {
        Expr::Name(n) => out.push((expr_id(expr), n.name.clone())),
        Expr::Index(ix) => {
            out.push((expr_id(expr), ix.name.clone()));
            for a in ix.args.iter().flatten() {
                collect_name_refs(a, out);
            }
        }
        Expr::Member(m) => {
            if let Some(b) = &m.base {
                collect_name_refs(b, out);
            }
            if let Some(args) = &m.args {
                for a in args.iter().flatten() {
                    collect_name_refs(a, out);
                }
            }
        }
        Expr::Unary(u) => {
            if let Some(op) = &u.operand {
                collect_name_refs(op, out);
            }
        }
        Expr::Binary(b) => {
            if let Some(l) = &b.left {
                collect_name_refs(l, out);
            }
            if let Some(r) = &b.right {
                collect_name_refs(r, out);
            }
        }
        Expr::ArrayLiteral(a) => {
            for item in a.items.iter().flatten() {
                collect_name_refs(item, out);
            }
        }
        Expr::Juxtaposed(j) => {
            for item in j.items.iter().flatten() {
                collect_name_refs(item, out);
            }
        }
        Expr::Number(_) | Expr::String(_) => {}
    }
}

/// Port of `checker/context.py::_with_inline`.
pub fn _with_inline(program: &Program) -> ProgramRef {
    let mut statements: Vec<StmtRef> = Vec::new();
    let mut flat_at: Vec<usize> = Vec::new();
    for stmt in &program.statements {
        flat_at.push(statements.len());
        statements.push(stmt.clone());
        if let Stmt::If(i) = &**stmt {
            if let Some(inline) = &i.inline {
                statements.extend(inline.iter().cloned());
            }
            if let Some(inline_else) = &i.inline_else {
                statements.extend(inline_else.iter().cloned());
            }
        }
    }
    flat_at.push(statements.len());
    let mut replaced = program.replaced();
    replaced.statements = statements;
    replaced.include_error_at = program
        .include_error_at
        .iter()
        .map(|&i| flat_at[i.min(flat_at.len() - 1)])
        .collect();
    Rc::new(replaced)
}

/// Port of `checker/context.py::_names_through_bad_as_clause`.
pub fn _names_through_bad_as_clause(stmt: &crate::syntax::Unparsed) -> BTreeSet<String> {
    let Some(m) = BAD_AS_CLAUSE_RE.match_(&stmt.base.text) else {
        return BTreeSet::new();
    };
    let rest = m.group(1).unwrap_or("");
    let bad_raw = stmt
        .reason
        .strip_prefix("Invalid As clause: ")
        .unwrap_or("")
        .trim();
    let bad = pyre::escape(bad_raw);
    let bad_re = if bad.is_empty() {
        None
    } else {
        Some(pyre::Regex::new(&format!(r"\b{bad}\b"), 0))
    };
    let mut out = BTreeSet::new();
    for item in split_top_commas(rest) {
        let Some(name_m) = IDENT_RE.match_(&item) else {
            break;
        };
        if let Some(ident) = name_m.group(1) {
            out.insert(pystr::upper(ident));
        }
        if let Some(ref re) = bad_re {
            let tail = pystr::slice(&item, i64::try_from(name_m.end(0).unwrap_or(0)).ok(), None);
            if re.search(tail).is_some() {
                break;
            }
        }
    }
    out
}

/// Port of `checker/context.py::_declared_names_in_statements`.
pub fn _declared_names_in_statements(statements: &[StmtRef]) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    for stmt in statements {
        match &**stmt {
            Stmt::Declaration(d) => {
                for v in &d.vars {
                    names.insert(pystr::upper(&v.name));
                }
            }
            Stmt::ConstDecl(c) => {
                names.insert(pystr::upper(&c.name));
            }
            Stmt::SubDef(s) => {
                names.insert(pystr::upper(&s.name));
                for p in &s.params {
                    names.insert(pystr::upper(&p.name));
                }
            }
            Stmt::Label(l) => {
                names.insert(pystr::upper(&l.name));
            }
            Stmt::Instruction(i) => {
                let kw = pystr::upper(&i.keyword);
                if NAME_DECLARING_INSTRUCTIONS.contains(&kw.as_str())
                    && let Some(Some(arg0)) = i.args.first()
                    && let Expr::Name(n) = &**arg0
                {
                    names.insert(pystr::upper(&n.name));
                } else if kw == "ALIAS" {
                    for arg in i.args.iter().skip(1).flatten() {
                        match &**arg {
                            Expr::Name(n) => {
                                names.insert(pystr::upper(&n.name));
                            }
                            Expr::Index(ix) if ix.args.len() == 1 => {
                                names.insert(pystr::upper(&ix.name));
                            }
                            _ => {}
                        }
                    }
                }
            }
            _ => {}
        }
        if let Some(m) = SNMP_VARIABLE_RE.match_(stmt.text())
            && let Some(g1) = m.group(1)
        {
            names.insert(pystr::upper(g1));
        }
        if let Stmt::Unparsed(u) = &**stmt
            && u.reason.starts_with("Invalid As clause: ")
        {
            names.extend(_names_through_bad_as_clause(u));
        }
    }
    names
}

/// Port of `checker/context.py::_declared_all_dims`.
pub fn _declared_all_dims(statements: &[StmtRef], env: &Env) -> OrderedMap<String, Vec<i64>> {
    let mut dims = OrderedMap::new();
    for stmt in statements {
        let Stmt::Declaration(d) = &**stmt else {
            continue;
        };
        if !matches!(d.kind.as_str(), "PUBLIC" | "DIM" | "STATIC") {
            continue;
        }
        for v in &d.vars {
            if v.dims.is_empty() {
                dims.insert(pystr::upper(&v.name), vec![1]);
                continue;
            }
            let mut sizes = Vec::new();
            let mut ok = true;
            for dim_expr in &v.dims {
                let Ok(val) = consteval::value_of(Some(dim_expr), env) else {
                    ok = false;
                    break;
                };
                match val {
                    ConstValue::Int(n) => sizes.push(n),
                    ConstValue::Float(f) | ConstValue::CFloat(f) => {
                        let Some(n) = pystr::py_int_from_f64(f) else {
                            ok = false;
                            break;
                        };
                        sizes.push(n);
                    }
                    ConstValue::BigInt(_) | ConstValue::Str(_) => {
                        ok = false;
                        break;
                    }
                }
            }
            if ok {
                dims.insert(pystr::upper(&v.name), sizes);
            }
        }
    }
    dims
}

/// Port of `checker/context.py::_ScopeLookup`.
#[derive(Debug, Clone)]
pub struct ScopeLookup {
    ctx: Rc<Context>,
    scalars: bool,
    parameters: bool,
}

impl ScopeLookup {
    /// Port of `checker/context.py::_ScopeLookup.__init__`.
    pub fn new(ctx: Rc<Context>, scalars: bool, parameters: bool) -> Self {
        Self {
            ctx,
            scalars,
            parameters,
        }
    }

    /// Port of `checker/context.py::_ScopeLookup.for_statement`.
    pub fn for_statement(&self, stmt: &StmtRef) -> ScopeTables {
        self.ctx._lookup_scope(stmt, self.scalars, self.parameters)
    }
}

/// Port of `checker/context.py::Context`.
#[derive(Debug)]
pub struct Context {
    pub program: ProgramRef,
    pub profile: Option<&'static Profile>,
    env_init: RefCell<Option<Env>>,
    variants: RefCell<HashMap<usize, Rc<Context>>>,
    routine_bases: Rc<RefCell<HashMap<usize, ScopeBase>>>,
    scope_cache: Rc<RefCell<HashMap<(usize, bool, bool), ScopeTables>>>,
    scoped_views: RefCell<HashMap<(usize, bool), ProgramRef>>,
    env_cell: OnceCell<Env>,
    flat_cell: OnceCell<ProgramRef>,
    positions_cell: OnceCell<Rc<Positions>>,
    routine_layout_cell: OnceCell<RoutineLayout>,
    declared_subs_cell: OnceCell<BTreeSet<String>>,
    system_table_names_cell: OnceCell<BTreeSet<String>>,
    ess_variable_names_cell: OnceCell<BTreeSet<String>>,
    variable_types_pair_cell: OnceCell<(OrderedMap<String, String>, OrderedMap<String, String>)>,
    const_table_fields_cell: OnceCell<OrderedMap<String, BTreeSet<String>>>,
    declared_variable_names_cell: OnceCell<BTreeSet<String>>,
    declared_names_cell: OnceCell<BTreeSet<String>>,
    statement_scopes_cell: OnceCell<HashMap<usize, (BTreeSet<String>, BTreeSet<String>)>>,
    not_yet_declared_cell: OnceCell<HashMap<usize, BTreeSet<String>>>,
    routine_scopes_cell: OnceCell<HashMap<usize, OrderedMap<String, (String, String)>>>,
    declared_dim0_sizes_cell: OnceCell<OrderedMap<String, i64>>,
    declared_all_dims_cell: OnceCell<OrderedMap<String, Vec<i64>>>,
    dim_counts_pair_cell: OnceCell<(DimCounts, DimCounts)>,
    known_names_cell: OnceCell<KnownNames>,
    dims_state_cell: OnceCell<DimsState>,
    float_names_cell: OnceCell<BTreeSet<String>>,
    literal_names_cell: OnceCell<BTreeSet<String>>,
    string_consts_cell: OnceCell<BTreeSet<String>>,
    alias_names_cell: OnceCell<BTreeSet<String>>,
    serialin_types_map_cell: OnceCell<OrderedMap<String, String>>,
}

impl Context {
    /// Port of `checker/context.py::Context.__init__`.
    pub fn new(program: ProgramRef, profile: Option<&'static Profile>, env: Option<Env>) -> Self {
        Self::with_shared(
            program,
            profile,
            env,
            Rc::new(RefCell::new(HashMap::new())),
            Rc::new(RefCell::new(HashMap::new())),
        )
    }

    fn with_shared(
        program: ProgramRef,
        profile: Option<&'static Profile>,
        env: Option<Env>,
        routine_bases: Rc<RefCell<HashMap<usize, ScopeBase>>>,
        scope_cache: Rc<RefCell<HashMap<(usize, bool, bool), ScopeTables>>>,
    ) -> Self {
        Self {
            program,
            profile,
            env_init: RefCell::new(env),
            variants: RefCell::new(HashMap::new()),
            routine_bases,
            scope_cache,
            scoped_views: RefCell::new(HashMap::new()),
            env_cell: OnceCell::new(),
            flat_cell: OnceCell::new(),
            positions_cell: OnceCell::new(),
            routine_layout_cell: OnceCell::new(),
            declared_subs_cell: OnceCell::new(),
            system_table_names_cell: OnceCell::new(),
            ess_variable_names_cell: OnceCell::new(),
            variable_types_pair_cell: OnceCell::new(),
            const_table_fields_cell: OnceCell::new(),
            declared_variable_names_cell: OnceCell::new(),
            declared_names_cell: OnceCell::new(),
            statement_scopes_cell: OnceCell::new(),
            not_yet_declared_cell: OnceCell::new(),
            routine_scopes_cell: OnceCell::new(),
            declared_dim0_sizes_cell: OnceCell::new(),
            declared_all_dims_cell: OnceCell::new(),
            dim_counts_pair_cell: OnceCell::new(),
            known_names_cell: OnceCell::new(),
            dims_state_cell: OnceCell::new(),
            float_names_cell: OnceCell::new(),
            literal_names_cell: OnceCell::new(),
            string_consts_cell: OnceCell::new(),
            alias_names_cell: OnceCell::new(),
            serialin_types_map_cell: OnceCell::new(),
        }
    }

    /// Port of `checker/context.py::Context.env`.
    pub fn env(&self) -> &Env {
        self.env_cell.get_or_init(|| {
            if let Some(env) = self.env_init.borrow().as_ref() {
                return env.clone();
            }
            let profile = self
                .profile
                .expect("Context::env requires profile when env is None");
            Env::with_consts(profile, Rc::new(RefCell::new(self.program.consts.clone())))
        })
    }

    /// Port of `checker/context.py::Context.flat`.
    pub fn flat(&self) -> &ProgramRef {
        self.flat_cell.get_or_init(|| {
            let flat_prog = _with_inline(&self.program);
            let has_inline_scope_change = self.program.statements.iter().any(|st| {
                let Stmt::If(i) = &**st else {
                    return false;
                };
                i.inline
                    .iter()
                    .flatten()
                    .chain(i.inline_else.iter().flatten())
                    .any(|inl| {
                        matches!(&**inl, Stmt::Declaration(_) | Stmt::SubDef(_))
                            || ends_routine(inl)
                    })
            });
            let flat_ctx = if has_inline_scope_change {
                Context::new(
                    flat_prog.clone(),
                    self.profile,
                    self.env_init.borrow().clone(),
                )
            } else {
                Context::with_shared(
                    flat_prog.clone(),
                    self.profile,
                    self.env_init.borrow().clone(),
                    self.routine_bases.clone(),
                    self.scope_cache.clone(),
                )
            };
            self.variants
                .borrow_mut()
                .insert(program_id(&flat_prog), Rc::new(flat_ctx));
            flat_prog
        })
    }

    /// Port of `checker/context.py::Context.positions`.
    pub fn positions(&self) -> &Rc<Positions> {
        self.positions_cell.get_or_init(|| {
            let mut pos = Positions::new();
            for (index, stmt) in self.flat().statements.iter().enumerate() {
                pos.entry((stmt.file().to_string(), stmt.line()))
                    .or_insert(i64::try_from(index).unwrap_or(i64::MAX));
            }
            Rc::new(pos)
        })
    }

    /// Port of `checker/context.py::Context._routine_layout`.
    pub fn _routine_layout(&self) -> &RoutineLayout {
        self.routine_layout_cell.get_or_init(|| {
            let mut groups: Vec<(Option<StmtRef>, Vec<StmtRef>)> = vec![(None, Vec::new())];
            let mut owners: HashMap<usize, Option<StmtRef>> = HashMap::new();
            let mut global_stmts: Vec<StmtRef> = Vec::new();
            let mut routine_stmts: HashMap<usize, Vec<StmtRef>> = HashMap::new();
            let mut all_public_in_routines: Vec<(StmtRef, StmtRef)> = Vec::new();
            let mut cur: Option<StmtRef> = None;

            for st in &self.program.statements {
                let is_sub = matches!(&**st, Stmt::SubDef(_));
                let is_end = if is_sub { false } else { ends_routine(st) };
                if is_sub {
                    cur = Some(st.clone());
                    groups.push((Some(st.clone()), Vec::new()));
                    routine_stmts.entry(stmt_id(st)).or_default();
                }
                groups
                    .last_mut()
                    .expect("groups non-empty")
                    .1
                    .push(st.clone());
                if is_end {
                    cur = None;
                    groups.push((None, Vec::new()));
                }
                owners.insert(stmt_id(st), cur.clone());
                match &cur {
                    None => global_stmts.push(st.clone()),
                    Some(owner) => {
                        routine_stmts
                            .entry(stmt_id(owner))
                            .or_default()
                            .push(st.clone());
                        if let Stmt::Declaration(d) = &**st
                            && d.kind == "PUBLIC"
                        {
                            all_public_in_routines.push((owner.clone(), st.clone()));
                        }
                    }
                }
            }
            RoutineLayout {
                owners,
                groups,
                global_stmts,
                routine_stmts,
                all_public_in_routines,
            }
        })
    }

    /// Port of `checker/context.py::Context.enclosing_sub`.
    pub fn enclosing_sub(&self, stmt: &StmtRef) -> Option<StmtRef> {
        self._routine_layout()
            .owners
            .get(&stmt_id(stmt))
            .cloned()
            .flatten()
    }

    /// Port of `checker/context.py::Context.declared_subs`.
    pub fn declared_subs(&self) -> &BTreeSet<String> {
        self.declared_subs_cell.get_or_init(|| {
            self.program
                .statements
                .iter()
                .filter_map(|s| match &**s {
                    Stmt::SubDef(sd) => Some(pystr::upper(&sd.name)),
                    _ => None,
                })
                .collect()
        })
    }

    /// Port of `checker/context.py::Context.system_table_names`.
    pub fn system_table_names(&self) -> &BTreeSet<String> {
        self.system_table_names_cell.get_or_init(|| {
            let mut names: BTreeSet<String> = ALWAYS_KNOWN_SYSTEM_TABLES
                .iter()
                .map(|&s| s.to_string())
                .collect();
            if self.program.statements.iter().any(
                |s| matches!(&**s, Stmt::Instruction(i) if pystr::upper(&i.keyword) == "DATATABLE"),
            ) {
                names.insert("DATATABLEINFO".to_string());
            }
            names
        })
    }

    /// Port of `checker/context.py::Context.ess_variable_names`.
    pub fn ess_variable_names(&self) -> &BTreeSet<String> {
        self.ess_variable_names_cell.get_or_init(|| {
            if !self.program.statements.iter().any(|s| {
                matches!(&**s, Stmt::Instruction(i) if pystr::upper(&i.keyword) == "ESSVARIABLES")
            }) {
                return BTreeSet::new();
            }
            let mut names = BTreeSet::new();
            for stmt in &self.program.statements {
                let mut exprs: Vec<&Option<ExprRef>> = Vec::new();
                let mut tmp: Vec<Option<ExprRef>> = Vec::new();
                match &**stmt {
                    Stmt::Assignment(a) => {
                        tmp.push(a.target.clone());
                        tmp.push(a.value.clone());
                    }
                    Stmt::Instruction(i) => {
                        exprs.extend(i.args.iter());
                    }
                    _ => {}
                }
                let mut extracted = Vec::new();
                for e in tmp.iter().flatten() {
                    walk_names(Some(e), &mut extracted);
                }
                for e in exprs.into_iter().flatten() {
                    walk_names(Some(e), &mut extracted);
                }
                for name in extracted {
                    let up = pystr::upper(&name);
                    if up.starts_with("ESS") {
                        names.insert(up);
                    }
                }
            }
            names
        })
    }

    /// Port of `checker/context.py::Context._variable_types_pair`.
    pub fn _variable_types_pair(
        &self,
    ) -> &(OrderedMap<String, String>, OrderedMap<String, String>) {
        self.variable_types_pair_cell.get_or_init(|| {
            let mut first: OrderedMap<String, String> = OrderedMap::new();
            let mut clash: BTreeSet<String> = BTreeSet::new();
            let mut last: OrderedMap<String, String> = OrderedMap::new();
            for st in &self.program.statements {
                if let Stmt::Declaration(d) = &**st
                    && matches!(d.kind.as_str(), "PUBLIC" | "DIM" | "STATIC")
                {
                    for v in &d.vars {
                        let name = pystr::upper(&v.name);
                        let mut kind = pystr::upper(v.type_.as_deref().unwrap_or("FLOAT"));
                        if v.paren_dim {
                            kind = "FLOAT".to_string();
                        }
                        last.insert(name.clone(), kind.clone());
                        if let Some(prev) = first.get(&name) {
                            if prev != &kind {
                                clash.insert(name);
                            }
                        } else {
                            first.insert(name, kind);
                        }
                    }
                }
            }
            for name in &clash {
                first.remove(name);
            }
            (first, last)
        })
    }

    /// Port of `checker/context.py::Context.variable_types`.
    pub fn variable_types(&self, prefer_last: bool) -> OrderedMap<String, String> {
        let (first, last) = self._variable_types_pair();
        if prefer_last {
            last.clone()
        } else {
            first.clone()
        }
    }

    /// Port of `checker/context.py::Context.const_table_fields`.
    pub fn const_table_fields(&self) -> &OrderedMap<String, BTreeSet<String>> {
        self.const_table_fields_cell.get_or_init(|| {
            let mut fields: OrderedMap<String, BTreeSet<String>> = OrderedMap::new();
            let mut current: Option<String> = None;
            for stmt in &self.program.statements {
                match &**stmt {
                    Stmt::Instruction(i) if pystr::upper(&i.keyword) == "CONSTTABLE" => {
                        current = match i.args.first() {
                            Some(Some(arg0)) => match &**arg0 {
                                Expr::Name(n) => Some(pystr::upper(&n.name)),
                                _ => None,
                            },
                            _ => None,
                        };
                        if let Some(ref name) = current
                            && !fields.contains_key(name)
                        {
                            fields.insert(name.clone(), BTreeSet::new());
                        }
                    }
                    Stmt::Instruction(i) if pystr::upper(&i.keyword) == "ENDCONSTTABLE" => {
                        current = None;
                    }
                    Stmt::ConstDecl(c) => {
                        if let Some(ref cur) = current {
                            let mut set = fields.get(cur).cloned().unwrap_or_default();
                            set.insert(pystr::upper(&c.name));
                            fields.insert(cur.clone(), set);
                        }
                    }
                    _ => {}
                }
            }
            fields
        })
    }

    /// Port of `checker/context.py::Context.declared_variable_names`.
    pub fn declared_variable_names(&self) -> &BTreeSet<String> {
        self.declared_variable_names_cell.get_or_init(|| {
            let mut names = BTreeSet::new();
            for stmt in &self.program.statements {
                match &**stmt {
                    Stmt::Declaration(d) => {
                        for v in &d.vars {
                            names.insert(pystr::upper(&v.name));
                        }
                    }
                    Stmt::SubDef(s) => {
                        for p in &s.params {
                            let up = pystr::upper(&p.name);
                            if !self.program.consts.contains_key(&up) {
                                names.insert(up);
                            }
                        }
                    }
                    _ => {}
                }
            }
            names
        })
    }

    /// Port of `checker/context.py::Context.declared_names`.
    pub fn declared_names(&self) -> &BTreeSet<String> {
        self.declared_names_cell
            .get_or_init(|| _declared_names_in_statements(&self.program.statements))
    }

    /// Port of `checker/context.py::Context.statement_scopes`.
    pub fn statement_scopes(&self) -> &HashMap<usize, (BTreeSet<String>, BTreeSet<String>)> {
        self.statement_scopes_cell.get_or_init(|| {
            let profile = self
                .profile
                .expect("Context::statement_scopes requires profile");
            let syms = profile.symbols();
            let consts: BTreeSet<String> = self
                .program
                .statements
                .iter()
                .filter_map(|st| match &**st {
                    Stmt::ConstDecl(c) => {
                        let up = pystr::upper(&c.name);
                        (!syms.contains(&up)).then_some(up)
                    }
                    _ => None,
                })
                .collect();
            let mut pending = consts.clone();
            let mut params = BTreeSet::new();
            let mut out = HashMap::new();
            for st in &self.program.statements {
                if let Stmt::SubDef(sd) = &**st {
                    let declared_consts: BTreeSet<String> =
                        consts.difference(&pending).cloned().collect();
                    params = sd
                        .params
                        .iter()
                        .map(|p| pystr::upper(&p.name))
                        .filter(|n| !declared_consts.contains(n))
                        .collect();
                } else if ends_routine(st) {
                    params = BTreeSet::new();
                } else if let Stmt::ConstDecl(c) = &**st {
                    pending.remove(&pystr::upper(&c.name));
                }
                out.insert(stmt_id(st), (params.clone(), pending.clone()));
            }
            out
        })
    }

    /// Port of `checker/context.py::Context.not_yet_declared`.
    pub fn not_yet_declared(&self) -> &HashMap<usize, BTreeSet<String>> {
        self.not_yet_declared_cell.get_or_init(|| {
            let profile = self
                .profile
                .expect("Context::not_yet_declared requires profile");
            let stmts = &self.program.statements;
            let variables = self.declared_variable_names();
            let mut base: BTreeSet<String> = BTreeSet::new();
            for (k, _) in profile.symbols().iter() {
                base.insert(k.to_string());
            }
            for (k, _) in profile.functions().iter() {
                base.insert(k.to_string());
            }
            for (k, _) in profile.instructions().iter() {
                base.insert(k.to_string());
            }
            let mut seen: BTreeSet<String> = BTreeSet::new();
            let mut out: HashMap<usize, BTreeSet<String>> = HashMap::new();
            let mut consts_after: Vec<BTreeSet<String>> = vec![BTreeSet::new(); stmts.len()];
            let mut pending: BTreeSet<String> = BTreeSet::new();
            for i in (0..stmts.len()).rev() {
                if let Stmt::ConstDecl(c) = &*stmts[i] {
                    pending.insert(pystr::upper(&c.name));
                }
                consts_after[i].clone_from(&pending);
            }
            let mut in_routine = false;
            for (i, st) in stmts.iter().enumerate() {
                if matches!(&**st, Stmt::SubDef(_)) {
                    in_routine = true;
                } else if ends_routine(st) {
                    in_routine = false;
                } else if in_routine {
                } else if let Stmt::ConstDecl(c) = &**st {
                    let mut later: BTreeSet<String> = consts_after[i].clone();
                    for v in variables {
                        if !seen.contains(v) {
                            later.insert(v.clone());
                        }
                    }
                    later.retain(|n| !seen.contains(n) && !base.contains(n));
                    out.insert(stmt_id(st), later);
                    seen.insert(pystr::upper(&c.name));
                } else if let Stmt::Declaration(d) = &**st {
                    for v in &d.vars {
                        seen.insert(pystr::upper(&v.name));
                    }
                }
            }
            out
        })
    }

    /// Port of `checker/context.py::Context.routine_scopes`.
    pub fn routine_scopes(&self) -> &HashMap<usize, OrderedMap<String, (String, String)>> {
        self.routine_scopes_cell.get_or_init(|| {
            fn declarations(stmts: &[StmtRef]) -> OrderedMap<String, (String, String)> {
                let mut out: OrderedMap<String, (String, String)> = OrderedMap::new();
                let mut clash: BTreeSet<String> = BTreeSet::new();
                for st in stmts {
                    if let Stmt::Declaration(d) = &**st
                        && matches!(d.kind.as_str(), "PUBLIC" | "DIM" | "STATIC")
                    {
                        for v in &d.vars {
                            let kind = if v.paren_dim {
                                "FLOAT".to_string()
                            } else {
                                pystr::upper(v.type_.as_deref().unwrap_or("FLOAT"))
                            };
                            let key = pystr::upper(&v.name);
                            if let Some((_, prev_kind)) = out.get(&key) {
                                if prev_kind != &kind {
                                    clash.insert(key);
                                }
                            } else {
                                out.insert(key, (v.name.clone(), kind));
                            }
                        }
                    }
                }
                for key in &clash {
                    out.remove(key);
                }
                out
            }

            let groups = &self._routine_layout().groups;
            let glob_stmts: Vec<StmtRef> = groups
                .iter()
                .filter(|(r, _)| r.is_none())
                .flat_map(|(_, s)| s.iter().cloned())
                .collect();
            let glob = declarations(&glob_stmts);
            let mut scopes: HashMap<usize, OrderedMap<String, (String, String)>> = HashMap::new();
            for (routine, stmts) in groups {
                let scope = match routine {
                    None => glob.clone(),
                    Some(r) => {
                        let mut sc = glob.clone();
                        for (k, v) in declarations(stmts).iter() {
                            sc.insert(k.clone(), v.clone());
                        }
                        if let Stmt::SubDef(sd) = &**r {
                            for prm in &sd.params {
                                sc.insert(
                                    pystr::upper(&prm.name),
                                    (
                                        prm.name.clone(),
                                        pystr::upper(prm.type_.as_deref().unwrap_or("FLOAT")),
                                    ),
                                );
                            }
                            if pystr::upper(&sd.keyword) == "FUNCTION" {
                                let rkey = pystr::upper(&sd.name);
                                if !sc.contains_key(&rkey) {
                                    sc.insert(rkey, (sd.name.clone(), String::new()));
                                }
                            }
                        }
                        sc
                    }
                };
                for st in stmts {
                    scopes.insert(stmt_id(st), scope.clone());
                }
            }
            scopes
        })
    }

    /// Port of `checker/context.py::Context.declared_dim0_sizes`.
    pub fn declared_dim0_sizes(&self) -> &OrderedMap<String, i64> {
        self.declared_dim0_sizes_cell.get_or_init(|| {
            let mut sizes = OrderedMap::new();
            for stmt in &self.program.statements {
                let Stmt::Declaration(d) = &**stmt else {
                    continue;
                };
                if !matches!(d.kind.as_str(), "PUBLIC" | "DIM" | "STATIC") {
                    continue;
                }
                for v in &d.vars {
                    if v.dims.is_empty() {
                        sizes.insert(pystr::upper(&v.name), 1);
                        continue;
                    }
                    let Ok(val) = consteval::value_of(Some(&v.dims[0]), self.env()) else {
                        continue;
                    };
                    match val {
                        ConstValue::Int(n) => {
                            sizes.insert(pystr::upper(&v.name), n);
                        }
                        ConstValue::Float(f) | ConstValue::CFloat(f) => {
                            if let Some(n) = pystr::py_int_from_f64(f) {
                                sizes.insert(pystr::upper(&v.name), n);
                            }
                        }
                        ConstValue::BigInt(_) | ConstValue::Str(_) => {}
                    }
                }
            }
            sizes
        })
    }

    /// Port of `checker/context.py::Context.declared_all_dims`.
    pub fn declared_all_dims(&self) -> &OrderedMap<String, Vec<i64>> {
        self.declared_all_dims_cell
            .get_or_init(|| _declared_all_dims(&self.program.statements, self.env()))
    }

    /// Port of `checker/context.py::Context._dim_counts_pair`.
    pub fn _dim_counts_pair(&self) -> &(DimCounts, DimCounts) {
        self.dim_counts_pair_cell.get_or_init(|| {
            let build = |strings: bool| -> DimCounts {
                let mut counts: OrderedMap<String, i64> = OrderedMap::new();
                let mut ambiguous: BTreeSet<String> = BTreeSet::new();
                let mut strings_seen: BTreeSet<String> = BTreeSet::new();
                for stmt in &self.program.statements {
                    if let Stmt::Declaration(d) = &**stmt
                        && matches!(d.kind.as_str(), "PUBLIC" | "DIM" | "STATIC")
                    {
                        for v in &d.vars {
                            let name = pystr::upper(&v.name);
                            let is_str = pystr::upper(v.type_.as_deref().unwrap_or("")) == "STRING";
                            if is_str {
                                strings_seen.insert(name.clone());
                            }
                            let empty_paren_pat = format!(
                                r"\b{}\s*\(\s*\){}",
                                pyre::escape(&v.name),
                                if strings { r"\s*As\b" } else { "" }
                            );
                            let empty_paren_re =
                                pyre::Regex::new(&empty_paren_pat, pyre::IGNORECASE);
                            if (is_str && !strings) || empty_paren_re.search(stmt.text()).is_some()
                            {
                                ambiguous.insert(name);
                            } else {
                                let ndims = i64::try_from(v.dims.len()).unwrap_or(i64::MAX);
                                if let Some(&prev) = counts.get(&name) {
                                    if prev != ndims {
                                        ambiguous.insert(name);
                                    }
                                } else {
                                    counts.insert(name, ndims);
                                }
                            }
                        }
                    }
                }
                for k in &ambiguous {
                    counts.remove(k);
                }
                DimCounts {
                    counts,
                    strings: strings_seen,
                }
            };
            (build(false), build(true))
        })
    }

    /// Port of `checker/context.py::Context.dim_counts`.
    pub fn dim_counts(&self, strings: bool) -> &DimCounts {
        let pair = self._dim_counts_pair();
        if strings { &pair.1 } else { &pair.0 }
    }

    /// Port of `checker/context.py::Context.scoped_view`.
    pub fn scoped_view(&self, stmt: &StmtRef, publics: bool) -> ProgramRef {
        let layout = self._routine_layout();
        let owner = layout.owners.get(&stmt_id(stmt)).cloned().flatten();
        let owner_id = owner.as_ref().map_or(0, stmt_id);
        let cache_key = (owner_id, publics);
        if let Some(cached) = self.scoped_views.borrow().get(&cache_key) {
            return cached.clone();
        }
        let mut stmts: Vec<StmtRef> = layout.global_stmts.clone();
        if let Some(ref own) = owner
            && let Some(rstmts) = layout.routine_stmts.get(&stmt_id(own))
        {
            stmts.extend(rstmts.iter().cloned());
        }
        if publics {
            let mut prefix: Vec<StmtRef> = layout
                .all_public_in_routines
                .iter()
                .filter(|(r, _)| !owner.as_ref().is_some_and(|o| Rc::ptr_eq(r, o)))
                .map(|(_, st)| st.clone())
                .collect();
            prefix.extend(stmts);
            stmts = prefix;
        }
        let view = Rc::new(Program::new("", stmts));
        self.scoped_views
            .borrow_mut()
            .insert(cache_key, view.clone());
        view
    }

    /// Port of `checker/context.py::Context._build_scope`.
    pub fn _build_scope(&self, routine: Option<&StmtRef>) -> ScopeBase {
        let layout = self._routine_layout();
        let mut stmts = layout.global_stmts.clone();
        if let Some(r) = routine
            && let Some(rstmts) = layout.routine_stmts.get(&stmt_id(r))
        {
            stmts.extend(rstmts.iter().cloned());
        }
        let mut dims = _declared_all_dims(&stmts, self.env());
        let mut array_types = OrderedMap::new();
        let mut scalar_types = OrderedMap::new();
        let mut arrays = BTreeSet::new();
        for st in &stmts {
            if let Stmt::Declaration(d) = &**st
                && matches!(d.kind.as_str(), "PUBLIC" | "DIM")
            {
                for v in &d.vars {
                    let name = pystr::upper(&v.name);
                    let kind = if v.paren_dim {
                        "FLOAT".to_string()
                    } else {
                        pystr::upper(v.type_.as_deref().unwrap_or("FLOAT"))
                    };
                    scalar_types.insert(name.clone(), kind.clone());
                    if !v.dims.is_empty() {
                        array_types.insert(name.clone(), kind);
                        arrays.insert(name);
                    }
                }
            }
        }
        if let Some(r) = routine
            && let Stmt::SubDef(sd) = &**r
        {
            for prm in &sd.params {
                let n = pystr::upper(&prm.name);
                dims.remove(&n);
                array_types.remove(&n);
                scalar_types.remove(&n);
                arrays.remove(&n);
            }
        }
        ScopeBase {
            dims,
            arrays,
            array_types,
            scalar_types,
        }
    }

    /// Port of `checker/context.py::Context._lookup_scope`.
    pub fn _lookup_scope(&self, stmt: &StmtRef, scalars: bool, parameters: bool) -> ScopeTables {
        let routine = self
            ._routine_layout()
            .owners
            .get(&stmt_id(stmt))
            .cloned()
            .flatten();
        let routine_id = routine.as_ref().map_or(0, stmt_id);
        let key = (routine_id, scalars, parameters);
        if let Some(cached) = self.scope_cache.borrow().get(&key) {
            return cached.clone();
        }
        let base = {
            let mut bases = self.routine_bases.borrow_mut();
            bases
                .entry(routine_id)
                .or_insert_with(|| self._build_scope(routine.as_ref()))
                .clone()
        };
        let base_types = if scalars {
            &base.scalar_types
        } else {
            &base.array_types
        };
        let params = match &routine {
            Some(r) => match &**r {
                Stmt::SubDef(sd) if !sd.params.is_empty() => Some(&sd.params),
                _ => None,
            },
            None => None,
        };
        let table = if let Some(params) = params.filter(|_| parameters) {
            let mut dims = base.dims;
            let mut types = base_types.clone();
            for prm in params {
                let n = pystr::upper(&prm.name);
                let mut shape: Vec<i64> = Vec::new();
                let mut ok = true;
                if prm.dim_exprs.is_empty() {
                    shape.push(1);
                } else {
                    for d in &prm.dim_exprs {
                        let Ok(val) = consteval::value_of(Some(d), self.env()) else {
                            ok = false;
                            break;
                        };
                        let num_ok = match val {
                            ConstValue::Int(i) => {
                                if i >= 1 {
                                    shape.push(i);
                                    true
                                } else {
                                    false
                                }
                            }
                            ConstValue::Float(f) | ConstValue::CFloat(f) => {
                                if (1.0..f64::INFINITY).contains(&f) {
                                    if let Some(n) = pystr::py_int_from_f64(f) {
                                        shape.push(n);
                                        true
                                    } else {
                                        false
                                    }
                                } else {
                                    false
                                }
                            }
                            ConstValue::BigInt(_) | ConstValue::Str(_) => false,
                        };
                        if !num_ok {
                            ok = false;
                            break;
                        }
                    }
                }
                if ok {
                    dims.insert(n.clone(), shape);
                    types.insert(n, pystr::upper(prm.type_.as_deref().unwrap_or("FLOAT")));
                }
            }
            ScopeTables {
                dims,
                types,
                arrays: base.arrays,
            }
        } else {
            ScopeTables {
                dims: base.dims,
                types: base_types.clone(),
                arrays: base.arrays,
            }
        };
        self.scope_cache.borrow_mut().insert(key, table.clone());
        table
    }

    /// Port of `checker/context.py::Context.scope_tables`.
    pub fn scope_tables(self: &Rc<Self>, scalars: bool, parameters: bool) -> ScopeLookup {
        ScopeLookup::new(self.clone(), scalars, parameters)
    }

    /// Port of `checker/context.py::Context.out_of_scope_nodes`.
    pub fn out_of_scope_nodes(&self, resolvable: &BTreeSet<String>) -> BTreeSet<usize> {
        let mut locals: HashMap<usize, BTreeSet<String>> = HashMap::new();
        let mut owners: Vec<(StmtRef, Option<StmtRef>)> = Vec::new();
        let mut outer_stmts: Vec<StmtRef> = Vec::new();
        for (routine, stmts) in &self._routine_layout().groups {
            if let Some(r) = routine {
                let rid = stmt_id(r);
                let local_names = locals.entry(rid).or_insert_with(|| match &**r {
                    Stmt::SubDef(sd) => sd.params.iter().map(|p| pystr::upper(&p.name)).collect(),
                    _ => BTreeSet::new(),
                });
                for st in stmts {
                    owners.push((st.clone(), Some(r.clone())));
                    if Rc::ptr_eq(st, r) {
                        if let Stmt::SubDef(sd) = &**r
                            && pystr::upper(&sd.keyword) != "SUB"
                        {
                            let no_params = replace_stmt(r, |s| {
                                if let Stmt::SubDef(sub) = s {
                                    sub.params.clear();
                                }
                            });
                            outer_stmts.push(no_params);
                        }
                    } else if let Stmt::Declaration(d) = &**st
                        && d.kind == "DIM"
                    {
                        for v in &d.vars {
                            local_names.insert(pystr::upper(&v.name));
                        }
                    } else {
                        outer_stmts.push(st.clone());
                    }
                }
            } else {
                for st in stmts {
                    owners.push((st.clone(), None));
                    outer_stmts.push(st.clone());
                }
            }
        }
        let mut every = BTreeSet::new();
        for set in locals.values() {
            every.extend(set.iter().cloned());
        }
        if every.is_empty() {
            return BTreeSet::new();
        }
        let outer = _declared_names_in_statements(&outer_stmts);
        let hidden_all: BTreeSet<String> = every
            .difference(&outer)
            .filter(|n| !resolvable.contains(*n))
            .cloned()
            .collect();
        if hidden_all.is_empty() {
            return BTreeSet::new();
        }
        let mut out = BTreeSet::new();
        for (st, r) in &owners {
            if let Some(rout) = r
                && Rc::ptr_eq(rout, st)
            {
                continue;
            }
            let local_r = r.as_ref().and_then(|rout| locals.get(&stmt_id(rout)));
            for e in stmt_exprs(st) {
                let mut refs = Vec::new();
                collect_name_refs(&e, &mut refs);
                for (nid, name) in refs {
                    let up = pystr::upper(&name);
                    if hidden_all.contains(&up) && !local_r.is_some_and(|l| l.contains(&up)) {
                        out.insert(nid);
                    }
                }
            }
        }
        out
    }

    /// Port of `checker/context.py::Context.known_names`.
    pub fn known_names(&self) -> &KnownNames {
        self.known_names_cell
            .get_or_init(|| expressions::_build_known_names(self))
    }

    /// Port of `checker/context.py::Context.dims_state`.
    pub fn dims_state(&self) -> &DimsState {
        self.dims_state_cell
            .get_or_init(|| expressions::_build_dims_state(self))
    }

    /// Port of `checker/context.py::Context.float_names`.
    pub fn float_names(&self) -> &BTreeSet<String> {
        self.float_names_cell
            .get_or_init(|| longconv::float_names_in(&self.program.statements))
    }

    /// Port of `checker/context.py::Context.literal_names`.
    pub fn literal_names(&self) -> &BTreeSet<String> {
        self.literal_names_cell
            .get_or_init(|| longconv::literal_names_in(&self.program.statements))
    }

    /// Port of `checker/context.py::Context.string_consts`.
    pub fn string_consts(&self) -> &BTreeSet<String> {
        self.string_consts_cell.get_or_init(|| {
            let mut out = BTreeSet::new();
            for st in &self.program.statements {
                if let Stmt::ConstDecl(c) = &**st
                    && _stringish_const(c.value.as_deref(), &out)
                {
                    out.insert(pystr::upper(&c.name));
                }
            }
            out
        })
    }

    /// Port of `checker/context.py::Context.alias_names`.
    pub fn alias_names(&self) -> &BTreeSet<String> {
        self.alias_names_cell.get_or_init(|| {
            let mut out = BTreeSet::new();
            for st in &self.program.statements {
                if let Stmt::Instruction(i) = &**st
                    && pystr::upper(&i.keyword) == "ALIAS"
                {
                    for a in i.args.iter().skip(1).flatten() {
                        if let Expr::Name(n) = &**a {
                            out.insert(pystr::upper(&n.name));
                        }
                    }
                }
            }
            out
        })
    }

    /// Port of `checker/context.py::Context._serialin_types_map`.
    pub fn _serialin_types_map(&self) -> &OrderedMap<String, String> {
        self.serialin_types_map_cell.get_or_init(|| {
            let profile = self
                .profile
                .expect("Context::_serialin_types_map requires profile");
            let mut types = self.variable_types(false);
            for stmt in &self.program.statements {
                match &**stmt {
                    Stmt::ConstDecl(c) => {
                        types.insert(pystr::upper(&c.name), "CONST".to_string());
                    }
                    Stmt::Instruction(i) if pystr::upper(&i.keyword) == "ALIAS" => {
                        if let Some(Some(arg0)) = i.args.first() {
                            let src_name = match &**arg0 {
                                Expr::Name(n) => Some(pystr::upper(&n.name)),
                                Expr::Index(ix) => Some(pystr::upper(&ix.name)),
                                _ => None,
                            };
                            if let Some(sname) = src_name
                                && let Some(kind) = types.get(&sname).cloned()
                            {
                                for alias in i.args.iter().skip(1).flatten() {
                                    if let Expr::Name(n) = &**alias {
                                        types.insert(pystr::upper(&n.name), kind.clone());
                                    }
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            for (name, _) in profile.functions().iter() {
                if !types.contains_key(name) {
                    types.insert(name.to_string(), "FUNCTION".to_string());
                }
            }
            for (name, _) in profile.symbols().iter() {
                if !types.contains_key(name) {
                    types.insert(name.to_string(), "CONST".to_string());
                }
            }
            types
        })
    }

    /// Port of `checker/context.py::Context.serialin_types`.
    pub fn serialin_types(&self) -> OrderedMap<String, String> {
        self._serialin_types_map().clone()
    }

    /// Port of `checker/context.py::Context.measurement_aliases`.
    pub fn measurement_aliases(self: &Rc<Self>) {
        instr_measure::_build_measurement_aliases(&self.program);
    }
}

/// Port of `checker/context.py::_stringish_const`.
pub fn _stringish_const(expr: Option<&Expr>, str_consts: &BTreeSet<String>) -> bool {
    let Some(e) = expr else {
        return false;
    };
    match e {
        Expr::String(_) => true,
        Expr::Name(n) => str_consts.contains(&pystr::upper(&n.name)),
        Expr::Binary(b) if b.op == "&" || b.op == "+" => {
            _stringish_const(b.left.as_deref(), str_consts)
                || _stringish_const(b.right.as_deref(), str_consts)
        }
        _ => false,
    }
}

thread_local! {
    static CURRENT: RefCell<Option<Rc<Context>>> = const { RefCell::new(None) };
}

/// RAII guard restoring `_CURRENT` and `_ACTIVE_POSITIONS` on drop.
#[must_use = "the active checker context is restored when the guard drops"]
pub struct ContextGuard {
    prev: Option<Rc<Context>>,
    _pos_guard: PositionsGuard,
}

impl Drop for ContextGuard {
    fn drop(&mut self) {
        let prev = self.prev.take();
        CURRENT.with(|c| *c.borrow_mut() = prev);
    }
}

/// Port of `checker/context.py::activate`.
pub fn activate(ctx: Rc<Context>) -> ContextGuard {
    let pos_guard = PositionsGuard::new(Some(ctx.positions().clone()));
    let prev = CURRENT.with(|c| c.borrow_mut().replace(ctx));
    ContextGuard {
        prev,
        _pos_guard: pos_guard,
    }
}

fn env_matches(a: Option<&Env>, b: Option<&Env>) -> bool {
    match (a, b) {
        (None, _) | (_, None) => true,
        (Some(ea), Some(eb)) => {
            ea.profile.name == eb.profile.name
                && Rc::ptr_eq(&ea.consts, &eb.consts)
                && ea.section == eb.section
        }
    }
}

/// Port of `checker/context.py::of`.
pub fn of(
    program: &ProgramRef,
    profile: Option<&'static Profile>,
    env: Option<Env>,
) -> Rc<Context> {
    let active = CURRENT.with(|c| c.borrow().clone());
    let Some(active) = active else {
        return Rc::new(Context::new(program.clone(), profile, env));
    };
    let same_profile =
        profile.is_none() || profile.map(|p| p.name) == active.profile.map(|p| p.name);
    if Rc::ptr_eq(program, &active.program)
        && same_profile
        && env_matches(env.as_ref(), active.env_init.borrow().as_ref())
    {
        if env.is_some() && active.env_init.borrow().is_none() {
            *active.env_init.borrow_mut() = env;
        }
        return active;
    }
    let eff_profile = profile.or(active.profile);
    let effenv_init = if env.is_none()
        && active.env_init.borrow().is_some()
        && program.consts.len() == active.program.consts.len()
    {
        active.env_init.borrow().clone()
    } else {
        env
    };
    let key = program_id(program);
    if let Some(cached) = active.variants.borrow().get(&key).cloned() {
        let cached_profile_ok =
            profile.is_none() || cached.profile.map(|p| p.name) == eff_profile.map(|p| p.name);
        if Rc::ptr_eq(&cached.program, program)
            && cached_profile_ok
            && env_matches(effenv_init.as_ref(), cached.env_init.borrow().as_ref())
        {
            if effenv_init.is_some() && cached.env_init.borrow().is_none() {
                *cached.env_init.borrow_mut() = effenv_init;
            }
            return cached;
        }
    }
    let variant = Rc::new(Context::new(program.clone(), eff_profile, effenv_init));
    active.variants.borrow_mut().insert(key, variant.clone());
    variant
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostics;
    use crate::profiles;
    use crate::syntax::{
        ConstDecl, Declaration, If, Instruction, Name, Node, Number, NumberValue, StmtBase,
        StringLit, SubDef, Unparsed, VarDecl,
    };

    fn make_program(stmts: Vec<StmtRef>) -> ProgramRef {
        Rc::new(Program::new("test.cr6", stmts))
    }

    fn make_instr(line: i64, kw: &str) -> StmtRef {
        Rc::new(Stmt::Instruction(Instruction {
            base: StmtBase::new(line, "test.cr6"),
            keyword: kw.to_string(),
            ..Default::default()
        }))
    }

    #[test]
    fn of_returns_active_for_same_rc_and_variant_for_replaced_program() {
        let prof = profiles::get("cr6").unwrap();
        let prog = make_program(vec![make_instr(1, "BEGINPROG"), make_instr(2, "ENDPROG")]);
        let ctx = Rc::new(Context::new(prog.clone(), Some(prof), None));

        // Before activate, `of` returns a fresh standalone context.
        let standalone = of(&prog, Some(prof), None);
        assert!(!Rc::ptr_eq(&standalone, &ctx));

        {
            let _guard = activate(ctx.clone());
            // Same Rc returns the active context.
            let got = of(&prog, Some(prof), None);
            assert!(Rc::ptr_eq(&got, &ctx));
            assert!(diagnostics::active_positions().is_some());

            // Replaced program returns a cached variant.
            let replaced = Rc::new(prog.replaced());
            let v1 = of(&replaced, Some(prof), None);
            assert!(!Rc::ptr_eq(&v1, &ctx));
            assert!(Rc::ptr_eq(&v1.program, &replaced));

            // Second lookup on the same replaced Rc returns the same cached variant.
            let v2 = of(&replaced, Some(prof), None);
            assert!(Rc::ptr_eq(&v1, &v2));
        }

        // Guard dropped: no active context or positions.
        assert!(diagnostics::active_positions().is_none());
        let after = of(&prog, Some(prof), None);
        assert!(!Rc::ptr_eq(&after, &ctx));
    }

    #[test]
    fn flat_flattens_inline_if_and_updates_positions() {
        let prof = profiles::get("cr6").unwrap();
        let inner = make_instr(1, "CALL");
        let if_stmt = Rc::new(Stmt::If(If {
            base: StmtBase::new(1, "test.cr6"),
            cond: None,
            inline: Some(vec![inner.clone()]),
            inline_else: None,
        }));
        let tail = make_instr(2, "ENDPROG");
        let prog = make_program(vec![if_stmt, tail]);
        let ctx = Rc::new(Context::new(prog.clone(), Some(prof), None));

        let flat = ctx.flat();
        assert_eq!(flat.statements.len(), 3);
        assert!(Rc::ptr_eq(&flat.statements[1], &inner));
        assert_eq!(ctx.positions().get(&("test.cr6".to_string(), 1)), Some(&0));
        assert_eq!(ctx.positions().get(&("test.cr6".to_string(), 2)), Some(&2));

        // Flat program is registered as a variant on `ctx`.
        let _guard = activate(ctx.clone());
        let flat_ctx = of(flat, Some(prof), None);
        assert!(Rc::ptr_eq(&flat_ctx.program, flat));
    }

    #[test]
    fn scope_tables_and_declared_dims_work() {
        let prof = profiles::get("cr6").unwrap();
        let decl = Rc::new(Stmt::Declaration(Declaration {
            base: StmtBase::new(1, "test.cr6"),
            kind: "PUBLIC".to_string(),
            vars: vec![Rc::new(VarDecl {
                dims: vec![Rc::new(Expr::Number(Number {
                    node: Node::new(1, "test.cr6"),
                    value: NumberValue::Int(4),
                    text: "4".to_string(),
                }))],
                type_: Some("LONG".to_string()),
                ..VarDecl::new("Arr")
            })],
        }));
        let end = make_instr(2, "ENDPROG");
        let prog = make_program(vec![decl, end.clone()]);
        let ctx = Rc::new(Context::new(prog, Some(prof), None));

        assert_eq!(ctx.declared_all_dims().get("ARR"), Some(&vec![4]));
        let lookup = ctx.scope_tables(false, false);
        let st = lookup.for_statement(&end);
        assert!(st.arrays.contains("ARR"));
        assert_eq!(st.types.get("ARR").map(String::as_str), Some("LONG"));
    }

    #[test]
    fn string_consts_and_bad_as_clause() {
        let c1 = Rc::new(Stmt::ConstDecl(ConstDecl {
            base: StmtBase::new(1, "test.cr6"),
            name: "S1".to_string(),
            value: Some(Rc::new(Expr::String(StringLit {
                node: Node::new(1, "test.cr6"),
                value: "hi".to_string(),
            }))),
            ..Default::default()
        }));
        let c2 = Rc::new(Stmt::ConstDecl(ConstDecl {
            base: StmtBase::new(2, "test.cr6"),
            name: "S2".to_string(),
            value: Some(Rc::new(Expr::Name(Name {
                node: Node::new(2, "test.cr6"),
                name: "S1".to_string(),
            }))),
            ..Default::default()
        }));
        let bad = Rc::new(Stmt::Unparsed(Unparsed {
            base: StmtBase {
                node: Node::new(3, "test.cr6"),
                text: "Public Q, Flag A s Boolean, After".to_string(),
                ..Default::default()
            },
            reason: "Invalid As clause: A".to_string(),
            expr: None,
        }));
        let sub = Rc::new(Stmt::SubDef(SubDef {
            base: StmtBase::new(4, "test.cr6"),
            keyword: "SUB".to_string(),
            name: "MySub".to_string(),
            ..Default::default()
        }));
        let prog = make_program(vec![c1, c2, bad, sub]);
        let ctx = Context::new(prog, None, None);
        assert!(ctx.string_consts().contains("S1"));
        assert!(ctx.string_consts().contains("S2"));
        assert!(ctx.declared_names().contains("Q"));
        assert!(ctx.declared_names().contains("FLAG"));
        assert!(!ctx.declared_names().contains("AFTER"));
        assert!(ctx.declared_subs().contains("MYSUB"));
    }
}
