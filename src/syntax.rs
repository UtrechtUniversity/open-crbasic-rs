//! Port of open-crbasic `syntax` (and `provenance`): the CRBasic syntax tree.
//!
//! A program is a flat list of statements, one per logical source line (or per `:`-separated
//! part). Block structure is kept flat, as the original compiler does.
//!
//! Design (contract for the rest of the port):
//! * `Expr` and `Stmt` are enums with one struct per Python class. Python base-class fields
//!   (`Node`, `Stmt`) live in `Node` / `StmtBase`, embedded as the first field of each variant
//!   struct; the enums expose accessors (`line()`, `file()`, `text()`, ...).
//! * Identity: statements, expressions, declarations and parameters are shared as `Rc` (Python
//!   `id(x)` is `rc_id`, `a is b` is `Rc::ptr_eq`, `dataclasses.replace` is `replace_stmt` /
//!   `Program::replaced`). `Clone` of a node clones its `Rc` children, so (as in Python) a
//!   replaced node shares its children with the original.
//! * Python attributes that are not dataclass fields (`getattr(stmt, "grouped", False)`, ...) are
//!   real fields marked `#[serde(skip)]`. Python's `dataclasses.replace` loses them, so
//!   `Stmt::replaced` resets them.
//! * No AST type implements `PartialEq`: Python dataclass equality of nodes is never used by the
//!   spec (inventory in the P0-b report); `Diagnostic` does implement it.
//! * JSON shape: `serde::Serialize` output equals Python `dataclasses.asdict` plus `"type":
//!   "<ClassName>"` on `Expr` / `Stmt` variants. One collision: `ConstDecl.type` is serialized
//!   as `"type_"` (the tag owns `"type"`); the Python dumper must rename it the same way.

use std::borrow::Borrow;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::hash::Hash;
use std::path::PathBuf;
use std::rc::Rc;

use serde::ser::{SerializeMap, SerializeSeq};
use serde::{Serialize, Serializer};

// ---------------------------------------------------------------- shared helpers

/// A shared expression node (Python expression objects are shared and `id()`-keyed).
pub type ExprRef = Rc<Expr>;
/// A shared statement (Python `id(stmt)` is `stmt_id`).
pub type StmtRef = Rc<Stmt>;
/// A shared program.
pub type ProgramRef = Rc<Program>;

/// Python `id(x)` of an `Rc`: its address. Equal for clones of the same `Rc`.
pub fn rc_id<T>(rc: &Rc<T>) -> usize {
    Rc::as_ptr(rc).addr()
}

/// Python `id(stmt)`.
pub fn stmt_id(stmt: &StmtRef) -> usize {
    rc_id(stmt)
}

/// Python `id(expr)`.
pub fn expr_id(expr: &ExprRef) -> usize {
    rc_id(expr)
}

/// Python `id(program)`.
pub fn program_id(program: &ProgramRef) -> usize {
    rc_id(program)
}

/// Python `dataclasses.replace(stmt, ...)`: clone the statement (children stay shared), drop the
/// non-field attributes as Python does, let `change` edit it, and wrap it in a new `Rc`.
pub fn replace_stmt(stmt: &StmtRef, change: impl FnOnce(&mut Stmt)) -> StmtRef {
    let mut new = stmt.replaced();
    change(&mut new);
    Rc::new(new)
}

/// Serialization of `Rc` (serde's `rc` feature is not enabled): implemented for `Rc<T>` and for
/// `Option` / `Vec` of anything serializable this way. Used through `serialize_with`.
pub trait RcSer {
    fn ser<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error>;
}

impl<T: Serialize> RcSer for Rc<T> {
    fn ser<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        (**self).serialize(s)
    }
}

impl<T: RcSer> RcSer for Option<T> {
    fn ser<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            None => s.serialize_none(),
            Some(v) => v.ser(s),
        }
    }
}

impl<T: RcSer> RcSer for Vec<T> {
    fn ser<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut seq = s.serialize_seq(Some(self.len()))?;
        for item in self {
            seq.serialize_element(&RcWrap(item))?;
        }
        seq.end()
    }
}

struct RcWrap<'a, T>(&'a T);

impl<T: RcSer> Serialize for RcWrap<'_, T> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.0.ser(s)
    }
}

fn ser_rc<T: RcSer, S: Serializer>(value: &T, s: S) -> Result<S::Ok, S::Error> {
    value.ser(s)
}

/// A dict that keeps Python's insertion order (replacing a key keeps its position, removing and
/// re-inserting moves it last). Lookups are hashed. Serializes as a JSON object in that order.
#[derive(Debug, Clone)]
pub struct OrderedMap<K, V> {
    entries: Vec<(K, V)>,
    index: HashMap<K, usize>,
}

impl<K, V> Default for OrderedMap<K, V> {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            index: HashMap::new(),
        }
    }
}

impl<K: Hash + Eq + Clone, V> OrderedMap<K, V> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn get<Q>(&self, key: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.index.get(key).map(|&i| &self.entries[i].1)
    }

    pub fn contains_key<Q>(&self, key: &Q) -> bool
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.index.contains_key(key)
    }

    /// `d[key] = value`; returns the previous value.
    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        if let Some(&i) = self.index.get(&key) {
            return Some(std::mem::replace(&mut self.entries[i].1, value));
        }
        self.index.insert(key.clone(), self.entries.len());
        self.entries.push((key, value));
        None
    }

    /// `d.pop(key, None)`.
    pub fn remove<Q>(&mut self, key: &Q) -> Option<V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        let i = self.index.remove(key)?;
        let (_, value) = self.entries.remove(i);
        for slot in self.index.values_mut() {
            if *slot > i {
                *slot -= 1;
            }
        }
        Some(value)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&K, &V)> {
        self.entries.iter().map(|(k, v)| (k, v))
    }

    pub fn keys(&self) -> impl Iterator<Item = &K> {
        self.entries.iter().map(|(k, _)| k)
    }

    pub fn values(&self) -> impl Iterator<Item = &V> {
        self.entries.iter().map(|(_, v)| v)
    }
}

impl<K: Serialize, V: Serialize> Serialize for OrderedMap<K, V> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut map = s.serialize_map(Some(self.entries.len()))?;
        for (k, v) in &self.entries {
            map.serialize_entry(k, v)?;
        }
        map.end()
    }
}

/// Python `str.isspace` for one `char` of Latin-1 text (the characters `str.lstrip()` removes).
// TODO(P1): switch to pystr
fn py_isspace(c: char) -> bool {
    matches!(c, '\t'..='\r' | '\x1c'..='\x1f' | ' ' | '\u{85}' | '\u{a0}')
}

// ---------------------------------------------------------------- constant values

/// Python `type ConstValue = int | float | str`: a folded constant (see `consteval.value_of`).
/// `CFloat` is `consteval.CFloat`, a `float` subclass marking a `Const` known to be float-typed
/// (a plain `Float` is "not established"); Python `bool` is never stored in `consts`.
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum ConstValue {
    Int(i64),
    Float(f64),
    CFloat(f64),
    Str(String),
}

/// Python `Number.value: float | int`.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(untagged)]
pub enum NumberValue {
    Int(i64),
    Float(f64),
}

impl Default for NumberValue {
    fn default() -> Self {
        Self::Int(0)
    }
}

// ---------------------------------------------------------------- nodes

/// Python `Node`: `line` (1-based) and the file it came from.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Node {
    pub line: i64,
    pub file: String,
}

impl Node {
    pub fn new(line: i64, file: &str) -> Self {
        Self {
            line,
            file: file.to_string(),
        }
    }
}

/// Python `COMPOUND_ASSIGNMENT_OPS` (a `frozenset`; order is irrelevant).
pub const COMPOUND_ASSIGNMENT_OPS: [&str; 7] = ["+=", "-=", "*=", "/=", "\\=", "^=", "&="];

// ---------------------------------------------------------------- expressions

/// Python `Number`.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Number {
    #[serde(flatten)]
    pub node: Node,
    pub value: NumberValue,
    /// Source spelling, e.g. `&H1F`, `1.5E3`.
    pub text: String,
}

/// Python `String` (renamed: it would shadow `std::string::String`).
#[derive(Debug, Clone, Default, Serialize)]
pub struct StringLit {
    #[serde(flatten)]
    pub node: Node,
    pub value: String,
}

/// Python `Name`: a bare identifier.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Name {
    #[serde(flatten)]
    pub node: Node,
    pub name: String,
}

/// Python `Index`: `name(i, j, k)`, an array element or a call. `None` marks an empty slot.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Index {
    #[serde(flatten)]
    pub node: Node,
    pub name: String,
    #[serde(serialize_with = "ser_rc")]
    pub args: Vec<Option<ExprRef>>,
}

/// Python `Member`: `Table.Field`, `Status.Battery(1)`, `Struct.member`.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Member {
    #[serde(flatten)]
    pub node: Node,
    #[serde(serialize_with = "ser_rc")]
    pub base: Option<ExprRef>,
    pub member: String,
    #[serde(serialize_with = "ser_rc")]
    pub args: Option<Vec<Option<ExprRef>>>,
}

/// Python `Unary`; `op` is `-`, `+` or `NOT`.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Unary {
    #[serde(flatten)]
    pub node: Node,
    pub op: String,
    #[serde(serialize_with = "ser_rc")]
    pub operand: Option<ExprRef>,
}

/// Python `ArrayLiteral`: `{a, b, c}`.
#[derive(Debug, Clone, Default, Serialize)]
pub struct ArrayLiteral {
    #[serde(flatten)]
    pub node: Node,
    #[serde(serialize_with = "ser_rc")]
    pub items: Vec<Option<ExprRef>>,
}

/// Python `Juxtaposed(ArrayLiteral)`: operands side by side in a declaration dimension. Python
/// code tests `isinstance(e, ArrayLiteral)` and that is true for this class too: use
/// `Expr::is_array_literal` / `Expr::array_items`.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Juxtaposed {
    #[serde(flatten)]
    pub node: Node,
    #[serde(serialize_with = "ser_rc")]
    pub items: Vec<Option<ExprRef>>,
}

/// Python `Binary`; `op` is the upper-case operator text.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Binary {
    #[serde(flatten)]
    pub node: Node,
    pub op: String,
    #[serde(serialize_with = "ser_rc")]
    pub left: Option<ExprRef>,
    #[serde(serialize_with = "ser_rc")]
    pub right: Option<ExprRef>,
}

/// Python `Expr` and its subclasses.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum Expr {
    Number(Number),
    String(StringLit),
    Name(Name),
    Index(Index),
    Member(Member),
    Unary(Unary),
    ArrayLiteral(ArrayLiteral),
    Juxtaposed(Juxtaposed),
    Binary(Binary),
}

impl Expr {
    pub fn node(&self) -> &Node {
        match self {
            Self::Number(e) => &e.node,
            Self::String(e) => &e.node,
            Self::Name(e) => &e.node,
            Self::Index(e) => &e.node,
            Self::Member(e) => &e.node,
            Self::Unary(e) => &e.node,
            Self::ArrayLiteral(e) => &e.node,
            Self::Juxtaposed(e) => &e.node,
            Self::Binary(e) => &e.node,
        }
    }

    pub fn line(&self) -> i64 {
        self.node().line
    }

    pub fn file(&self) -> &str {
        &self.node().file
    }

    /// The Python class name (the `"type"` tag).
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::Number(_) => "Number",
            Self::String(_) => "String",
            Self::Name(_) => "Name",
            Self::Index(_) => "Index",
            Self::Member(_) => "Member",
            Self::Unary(_) => "Unary",
            Self::ArrayLiteral(_) => "ArrayLiteral",
            Self::Juxtaposed(_) => "Juxtaposed",
            Self::Binary(_) => "Binary",
        }
    }

    /// Python `isinstance(e, ArrayLiteral)`: true for `ArrayLiteral` and its subclass `Juxtaposed`.
    pub fn is_array_literal(&self) -> bool {
        matches!(self, Self::ArrayLiteral(_) | Self::Juxtaposed(_))
    }

    /// `e.items` for an `ArrayLiteral` (or `Juxtaposed`), else `None`.
    pub fn array_items(&self) -> Option<&Vec<Option<ExprRef>>> {
        match self {
            Self::ArrayLiteral(e) => Some(&e.items),
            Self::Juxtaposed(e) => Some(&e.items),
            _ => None,
        }
    }

    /// `getattr(e, "name", "")`: `Name` and `Index`.
    pub fn name(&self) -> &str {
        match self {
            Self::Name(e) => &e.name,
            Self::Index(e) => &e.name,
            _ => "",
        }
    }

    /// `getattr(e, "text", "")`: `Number` only.
    pub fn text(&self) -> &str {
        match self {
            Self::Number(e) => &e.text,
            _ => "",
        }
    }

    /// `getattr(e, "args", None) or []`: `Index` and `Member` (the latter when it has any).
    pub fn args(&self) -> &[Option<ExprRef>] {
        match self {
            Self::Index(e) => &e.args,
            Self::Member(e) => e.args.as_deref().unwrap_or(&[]),
            _ => &[],
        }
    }

    /// The child expressions in dataclass field order, like Python's `vars(e).values()` walk
    /// (`parser.py` name collection): `Index.args`, `Member.base` then `args`, `Unary.operand`,
    /// the items of an array literal, `Binary.left` then `right`. Empty slots are skipped.
    pub fn children(&self) -> Vec<&ExprRef> {
        let mut out: Vec<&ExprRef> = Vec::new();
        match self {
            Self::Number(_) | Self::String(_) | Self::Name(_) => {}
            Self::Index(e) => out.extend(e.args.iter().flatten()),
            Self::Member(e) => {
                out.extend(e.base.iter());
                out.extend(e.args.iter().flatten().flatten());
            }
            Self::Unary(e) => out.extend(e.operand.iter()),
            Self::ArrayLiteral(e) => out.extend(e.items.iter().flatten()),
            Self::Juxtaposed(e) => out.extend(e.items.iter().flatten()),
            Self::Binary(e) => {
                out.extend(e.left.iter());
                out.extend(e.right.iter());
            }
        }
        out
    }
}

// ---------------------------------------------------------------- statements

/// Python `Node` + `Stmt` + `_StatementMetadata`: the fields every statement carries.
///
/// The first four are dataclass fields. The rest are Python attributes that are not fields (set
/// by `parser.py` after construction, read with `getattr(..., default)` / `hasattr`); the parser
/// writes them after a statement is already shared, hence `Cell` / `RefCell`.
#[derive(Debug, Clone, Serialize)]
pub struct StmtBase {
    #[serde(flatten)]
    pub node: Node,
    /// The logical line as written (for diagnostics that echo it).
    pub text: String,
    /// `text` (this statement's own `:`-separated segment) reaches the true end of its physical
    /// source line (no `'` comment and no further `:` segment cut it short). A dataclass field,
    /// but `parser.py` rewrites it after construction.
    pub at_eol: Cell<bool>,
    /// Non-field `origin`: (logical line, segment offset); absent when `provenance` is off.
    /// Writer `parser.py:932`, reader `parser.py:610` (`hasattr`).
    #[serde(skip)]
    pub origin: RefCell<Option<(Rc<LogicalLine>, usize)>>,
    /// Non-field `dropped_tail`: tokens dropped by `_close_open_call`; absent = `None`.
    /// Writer `parser.py:1166`, reader `checker/cr3xx.py:250`.
    #[serde(skip)]
    pub dropped_tail: RefCell<Option<Vec<Token>>>,
    /// Non-field `grouped` (default `False`): parsed from a parenthesised group.
    /// Writer `parser.py:1293`, reader `parser.py:930`.
    #[serde(skip)]
    pub grouped: Cell<bool>,
    /// Non-field `hoisted_last`; `None` when the attribute is absent (`hasattr` is false).
    /// Writer `parser.py:606`, reader `checker/warnings.py:336`.
    #[serde(skip)]
    pub hoisted_last: Cell<Option<bool>>,
}

impl StmtBase {
    pub fn new(line: i64, file: &str) -> Self {
        Self {
            node: Node::new(line, file),
            text: String::new(),
            at_eol: Cell::new(true),
            origin: RefCell::new(None),
            dropped_tail: RefCell::new(None),
            grouped: Cell::new(false),
            hoisted_last: Cell::new(None),
        }
    }

    /// Drop the non-field attributes, as `dataclasses.replace` does.
    pub fn reset_adhoc(&mut self) {
        self.origin = RefCell::new(None);
        self.dropped_tail = RefCell::new(None);
        self.grouped = Cell::new(false);
        self.hoisted_last = Cell::new(None);
    }
}

impl Default for StmtBase {
    fn default() -> Self {
        Self::new(0, "")
    }
}

/// Python `VarDecl` (with `_DeclarationMetadata`): one variable of a `Declaration`. Shared as
/// `Rc<VarDecl>` because `checker` keys diagnostics on `id(v)`.
#[derive(Debug, Clone, Default, Serialize)]
pub struct VarDecl {
    pub name: String,
    /// Up to 3.
    #[serde(serialize_with = "ser_rc")]
    pub dims: Vec<ExprRef>,
    /// Upper-case type name (`FLOAT`, ..., or a structure type name).
    #[serde(rename = "type")]
    pub type_: Option<String>,
    /// `As String * n`.
    #[serde(serialize_with = "ser_rc")]
    pub string_size: Option<ExprRef>,
    /// `= value` or `= {a, b, c}`.
    #[serde(serialize_with = "ser_rc")]
    pub init: Option<Vec<Option<ExprRef>>>,
    pub units: Option<String>,
    /// (start, end) columns of the first dimension's own source text within `Declaration.text`.
    pub dim0_span: Option<(i64, i64)>,
    /// A `(` inside the dimension list makes the variable Float whatever its `As` type.
    pub paren_dim: bool,
    /// The dimensions as written when one is a `Juxtaposed`; `dims` then holds 1 in its place.
    #[serde(serialize_with = "ser_rc")]
    pub junk_dims: Vec<ExprRef>,
    /// `As Float() = ...`.
    pub empty_type_init: bool,
    /// Non-field `size_missing`: column just after the `*`, or `None` (also when absent).
    /// Writer `parser.py:2781`, reader `checker/structure.py:1542`.
    #[serde(skip)]
    pub size_missing: Option<i64>,
    /// Non-field `size_junk`: (column of the String size, kind of what follows it), or `None`.
    /// Writer `parser.py:2782`, reader `checker/structure.py:1556`.
    #[serde(skip)]
    pub size_junk: Option<(i64, String)>,
}

impl VarDecl {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            ..Self::default()
        }
    }
}

/// Python `Declaration`: `Public` / `Dim` / `Static` (`kind` is upper-case).
#[derive(Debug, Clone, Default, Serialize)]
pub struct Declaration {
    #[serde(flatten)]
    pub base: StmtBase,
    pub kind: String,
    #[serde(serialize_with = "ser_rc")]
    pub vars: Vec<Rc<VarDecl>>,
}

/// Python `ConstDecl`. `type_` is serialized as `"type_"` (the enum tag owns `"type"`).
#[derive(Debug, Clone, Default, Serialize)]
pub struct ConstDecl {
    #[serde(flatten)]
    pub base: StmtBase,
    pub name: String,
    #[serde(serialize_with = "ser_rc")]
    pub value: Option<ExprRef>,
    #[serde(rename = "type_")]
    pub type_: Option<String>,
    /// (start, end) columns of `value`'s own source text within `Stmt.text`.
    pub value_span: Option<(i64, i64)>,
}

/// Python `Assignment`.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Assignment {
    #[serde(flatten)]
    pub base: StmtBase,
    /// `Name`, `Index` or `Member`.
    #[serde(serialize_with = "ser_rc")]
    pub target: Option<ExprRef>,
    #[serde(serialize_with = "ser_rc")]
    pub value: Option<ExprRef>,
    /// `X += 1`, stored as `X = X + 1`.
    pub compound: bool,
}

/// Python `Instruction`: `Keyword [(] args [)]`, including block keywords (`keyword` upper-case).
#[derive(Debug, Clone, Default, Serialize)]
pub struct Instruction {
    #[serde(flatten)]
    pub base: StmtBase,
    pub keyword: String,
    /// `None` is an empty slot.
    #[serde(serialize_with = "ser_rc")]
    pub args: Vec<Option<ExprRef>>,
    /// (start, end) columns of each argument's own raw source text within `text`.
    pub arg_spans: Vec<Option<(i64, i64)>>,
    /// Non-field `alias_junk` (default `false`): something follows the first name of an `Alias`.
    /// Writer `parser.py:2836`, reader `checker/structure.py:5835`.
    #[serde(skip)]
    pub alias_junk: bool,
    /// Non-field `alias_item_junk` (absent = `None`): per name; the dimension checks skip such
    /// items. Writer `parser.py:2837`, reader `checker/structure.py:5831`.
    #[serde(skip)]
    pub alias_item_junk: Option<Vec<bool>>,
    /// Non-field `extra_targets` (absent = empty): the extra `Units` targets.
    /// Writer `parser.py:2898`, reader `checker/instr_data.py:793`.
    #[serde(skip)]
    pub extra_targets: Vec<ExprRef>,
}

/// Python `If`: `If cond Then` (block form, `inline` is `None`) or `If cond Then stmt [Else stmt]`.
#[derive(Debug, Clone, Default, Serialize)]
pub struct If {
    #[serde(flatten)]
    pub base: StmtBase,
    #[serde(serialize_with = "ser_rc")]
    pub cond: Option<ExprRef>,
    #[serde(serialize_with = "ser_rc")]
    pub inline: Option<Vec<StmtRef>>,
    #[serde(serialize_with = "ser_rc")]
    pub inline_else: Option<Vec<StmtRef>>,
}

/// Python `ElseIf`.
#[derive(Debug, Clone, Default, Serialize)]
pub struct ElseIf {
    #[serde(flatten)]
    pub base: StmtBase,
    #[serde(serialize_with = "ser_rc")]
    pub cond: Option<ExprRef>,
}

/// Python `For`.
#[derive(Debug, Clone, Default, Serialize)]
pub struct For {
    #[serde(flatten)]
    pub base: StmtBase,
    #[serde(serialize_with = "ser_rc")]
    pub var: Option<ExprRef>,
    #[serde(serialize_with = "ser_rc")]
    pub start: Option<ExprRef>,
    #[serde(serialize_with = "ser_rc")]
    pub end: Option<ExprRef>,
    #[serde(serialize_with = "ser_rc")]
    pub step: Option<ExprRef>,
}

/// Python `DoLoop`: `Do [While|Until cond]` or `Loop [While|Until cond]`.
#[derive(Debug, Clone, Default, Serialize)]
pub struct DoLoop {
    #[serde(flatten)]
    pub base: StmtBase,
    /// `DO` or `LOOP`.
    pub keyword: String,
    /// `WHILE` / `UNTIL`.
    pub cond_kind: Option<String>,
    #[serde(serialize_with = "ser_rc")]
    pub cond: Option<ExprRef>,
}

/// Python `CaseItem`: `("value", e)`, `("range", lo, hi)` or `("is", op, e)`. Serialized as the
/// tuple Python's `asdict` produces (a JSON list starting with the tag word).
#[derive(Debug, Clone)]
pub enum CaseItem {
    Value(ExprRef),
    Range(ExprRef, ExprRef),
    Is(String, ExprRef),
}

impl Serialize for CaseItem {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Value(e) => {
                let mut seq = s.serialize_seq(Some(2))?;
                seq.serialize_element("value")?;
                seq.serialize_element(&**e)?;
                seq.end()
            }
            Self::Range(lo, hi) => {
                let mut seq = s.serialize_seq(Some(3))?;
                seq.serialize_element("range")?;
                seq.serialize_element(&**lo)?;
                seq.serialize_element(&**hi)?;
                seq.end()
            }
            Self::Is(op, e) => {
                let mut seq = s.serialize_seq(Some(3))?;
                seq.serialize_element("is")?;
                seq.serialize_element(op)?;
                seq.serialize_element(&**e)?;
                seq.end()
            }
        }
    }
}

/// Python `Case`: `Case a, b To c, Is > d, Else`.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Case {
    #[serde(flatten)]
    pub base: StmtBase,
    pub items: Vec<CaseItem>,
    pub is_else: bool,
}

/// Python `Param`: one parameter of a `SubDef`. Shared as `Rc<Param>` (`id(v)` keys, like
/// `VarDecl`).
#[derive(Debug, Clone, Default, Serialize)]
pub struct Param {
    pub name: String,
    /// `None` when not written.
    pub by_ref: Option<bool>,
    #[serde(rename = "type")]
    pub type_: Option<String>,
    #[serde(serialize_with = "ser_rc")]
    pub string_size: Option<ExprRef>,
    /// `x()` style array parameter.
    pub dims: i64,
    /// `Optional x = 1`.
    #[serde(serialize_with = "ser_rc")]
    pub default: Option<ExprRef>,
    /// Parsed expressions inside `x(...)`, in source order (`compare=False` in Python).
    #[serde(serialize_with = "ser_rc")]
    pub dim_exprs: Vec<ExprRef>,
}

impl Param {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            ..Self::default()
        }
    }
}

/// Python `SubDef`: `Sub name(params)` or `Function name(params) [As type]`.
#[derive(Debug, Clone, Default, Serialize)]
pub struct SubDef {
    #[serde(flatten)]
    pub base: StmtBase,
    /// `SUB` or `FUNCTION`.
    pub keyword: String,
    pub name: String,
    #[serde(serialize_with = "ser_rc")]
    pub params: Vec<Rc<Param>>,
    pub return_type: Option<String>,
    /// Pass-1 messages of a malformed header, in source order (`compare=False` in Python).
    pub header_errors: Vec<String>,
}

impl SubDef {
    /// Python `SubDef.missing_required_params`.
    pub fn missing_required_params(&self, args: &[Option<ExprRef>]) -> usize {
        self.params
            .iter()
            .enumerate()
            .filter(|(i, param)| {
                param.default.is_none() && args.get(*i).is_none_or(Option::is_none)
            })
            .count()
    }
}

/// Python `Call`: `Call name(args)` or a bare `name(args)` / `name args` invocation of a Sub.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Call {
    #[serde(flatten)]
    pub base: StmtBase,
    pub name: String,
    #[serde(serialize_with = "ser_rc")]
    pub args: Vec<Option<ExprRef>>,
    /// Written with `Call`.
    pub explicit: bool,
    /// `S()` followed by ignored trailing text.
    pub tail_ignored: bool,
    /// A `Function` call statement continued as one expression.
    #[serde(serialize_with = "ser_rc")]
    pub tail_expr: Option<ExprRef>,
    /// Operands of a juxtaposed tail: `Fn(1) 1`.
    #[serde(serialize_with = "ser_rc")]
    pub juxtaposed: Option<Vec<ExprRef>>,
    /// Tail starts with `,` or `(`: `Fn(1),Zork`.
    pub juxt_opaque: bool,
}

/// Python `Label`.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Label {
    #[serde(flatten)]
    pub base: StmtBase,
    pub name: String,
}

/// Python `Unparsed`: a line the parser could not handle.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Unparsed {
    #[serde(flatten)]
    pub base: StmtBase,
    pub reason: String,
    /// The expression of a statement the parser took apart but cannot compile.
    #[serde(serialize_with = "ser_rc")]
    pub expr: Option<ExprRef>,
}

/// Python `Stmt` and its subclasses. Always held as `StmtRef`.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
#[allow(
    clippy::large_enum_variant,
    reason = "statements are always behind an Rc; boxing would only add noise"
)]
pub enum Stmt {
    Declaration(Declaration),
    ConstDecl(ConstDecl),
    Assignment(Assignment),
    Instruction(Instruction),
    If(If),
    ElseIf(ElseIf),
    For(For),
    DoLoop(DoLoop),
    Case(Case),
    SubDef(SubDef),
    Call(Call),
    Label(Label),
    Unparsed(Unparsed),
}

macro_rules! stmt_arms {
    ($e:expr, $s:ident => $body:expr) => {
        match $e {
            Stmt::Declaration($s) => $body,
            Stmt::ConstDecl($s) => $body,
            Stmt::Assignment($s) => $body,
            Stmt::Instruction($s) => $body,
            Stmt::If($s) => $body,
            Stmt::ElseIf($s) => $body,
            Stmt::For($s) => $body,
            Stmt::DoLoop($s) => $body,
            Stmt::Case($s) => $body,
            Stmt::SubDef($s) => $body,
            Stmt::Call($s) => $body,
            Stmt::Label($s) => $body,
            Stmt::Unparsed($s) => $body,
        }
    };
}

impl Stmt {
    pub fn base(&self) -> &StmtBase {
        stmt_arms!(self, s => &s.base)
    }

    pub fn base_mut(&mut self) -> &mut StmtBase {
        stmt_arms!(self, s => &mut s.base)
    }

    pub fn line(&self) -> i64 {
        self.base().node.line
    }

    pub fn file(&self) -> &str {
        &self.base().node.file
    }

    pub fn text(&self) -> &str {
        &self.base().text
    }

    pub fn at_eol(&self) -> bool {
        self.base().at_eol.get()
    }

    /// The Python class name (the `"type"` tag).
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::Declaration(_) => "Declaration",
            Self::ConstDecl(_) => "ConstDecl",
            Self::Assignment(_) => "Assignment",
            Self::Instruction(_) => "Instruction",
            Self::If(_) => "If",
            Self::ElseIf(_) => "ElseIf",
            Self::For(_) => "For",
            Self::DoLoop(_) => "DoLoop",
            Self::Case(_) => "Case",
            Self::SubDef(_) => "SubDef",
            Self::Call(_) => "Call",
            Self::Label(_) => "Label",
            Self::Unparsed(_) => "Unparsed",
        }
    }

    /// `getattr(stmt, "keyword", "")`: `Instruction`, `DoLoop`, `SubDef`.
    pub fn keyword(&self) -> &str {
        match self {
            Self::Instruction(s) => &s.keyword,
            Self::DoLoop(s) => &s.keyword,
            Self::SubDef(s) => &s.keyword,
            _ => "",
        }
    }

    /// `getattr(stmt, "name", "")`: `ConstDecl`, `SubDef`, `Call`, `Label`.
    pub fn name(&self) -> &str {
        match self {
            Self::ConstDecl(s) => &s.name,
            Self::SubDef(s) => &s.name,
            Self::Call(s) => &s.name,
            Self::Label(s) => &s.name,
            _ => "",
        }
    }

    /// `getattr(stmt, "args", []) or []`: `Instruction` and `Call`.
    pub fn args(&self) -> &[Option<ExprRef>] {
        match self {
            Self::Instruction(s) => &s.args,
            Self::Call(s) => &s.args,
            _ => &[],
        }
    }

    /// `getattr(stmt, "arg_spans", None)`: `Some` for an `Instruction` only.
    pub fn arg_spans(&self) -> Option<&Vec<Option<(i64, i64)>>> {
        match self {
            Self::Instruction(s) => Some(&s.arg_spans),
            _ => None,
        }
    }

    /// `getattr(stmt, "inline", None)`: `If` only.
    pub fn inline(&self) -> Option<&Vec<StmtRef>> {
        match self {
            Self::If(s) => s.inline.as_ref(),
            _ => None,
        }
    }

    /// `getattr(stmt, "inline_else", None)`: `If` only.
    pub fn inline_else(&self) -> Option<&Vec<StmtRef>> {
        match self {
            Self::If(s) => s.inline_else.as_ref(),
            _ => None,
        }
    }

    /// `(getattr(st, "inline", None) or []) + (getattr(st, "inline_else", None) or [])`.
    pub fn inline_bodies(&self) -> Vec<StmtRef> {
        let mut out = self.inline().cloned().unwrap_or_default();
        out.extend(self.inline_else().cloned().unwrap_or_default());
        out
    }

    /// `getattr(stmt, "reason", "")`: `Unparsed` only.
    pub fn reason(&self) -> &str {
        match self {
            Self::Unparsed(s) => &s.reason,
            _ => "",
        }
    }

    /// Python `dataclasses.replace(stmt)` before the edits: a copy whose children are shared and
    /// whose non-field attributes (`origin`, `dropped_tail`, `grouped`, `hoisted_last`,
    /// `alias_junk`, `alias_item_junk`, `extra_targets`) are gone, as `replace` goes through
    /// `__init__`. Prefer `replace_stmt`.
    #[must_use]
    pub fn replaced(&self) -> Stmt {
        let mut new = self.clone();
        new.base_mut().reset_adhoc();
        if let Stmt::Instruction(i) = &mut new {
            i.alias_junk = false;
            i.alias_item_junk = None;
            i.extra_targets = Vec::new();
        }
        new
    }
}

// ---------------------------------------------------------------- module-level functions

/// Python `ends_routine`: `EndSub`/`EndFunction`, or the two-word `End Sub`/`End Function`
/// (parsed as `END` with the word as its argument).
pub fn ends_routine(stmt: &Stmt) -> bool {
    let Stmt::Instruction(instr) = stmt else {
        return false;
    };
    let kw = instr.keyword.to_uppercase();
    if kw == "ENDSUB" || kw == "ENDFUNCTION" {
        return true;
    }
    kw == "END"
        && matches!(
            instr.args.first(),
            Some(Some(arg)) if matches!(&**arg, Expr::Name(n) if {
                let up = n.name.to_uppercase();
                up == "SUB" || up == "FUNCTION"
            })
        )
}

/// Python `hash_statement`: the `#...` statement that ends the compile, `st` itself or one in an
/// inline `If` body (returned as the same `Rc`, so `id()` identity is kept).
pub fn hash_statement(st: &StmtRef) -> Option<StmtRef> {
    if let Stmt::Unparsed(u) = &**st {
        return u
            .base
            .text
            .trim_start_matches(py_isspace)
            .starts_with('#')
            .then(|| st.clone());
    }
    st.inline_bodies().iter().find_map(hash_statement)
}

/// Python `cut_after_hash`: `st` without the inline statements that follow its `#...` statement
/// (and without the `Else` body when the `#...` is in the `Then` body).
pub fn cut_after_hash(st: &StmtRef) -> StmtRef {
    let Stmt::If(_) = &**st else {
        return st.clone();
    };
    let then = st.inline().cloned().unwrap_or_default();
    let other = st.inline_else().cloned().unwrap_or_default();
    for (body, is_then) in [(then, true), (other, false)] {
        if let Some(k) = body.iter().position(|sub| hash_statement(sub).is_some()) {
            let mut cut: Vec<StmtRef> = body[..k].to_vec();
            cut.push(cut_after_hash(&body[k]));
            return replace_stmt(st, |new| {
                if let Stmt::If(i) = new {
                    if is_then {
                        i.inline = Some(cut);
                        i.inline_else = Some(Vec::new());
                    } else {
                        i.inline_else = Some(cut);
                    }
                }
            });
        }
    }
    st.clone()
}

// ---------------------------------------------------------------- program

/// Python `LoadProvenance` (`provenance.py`): annotations only; loaders populate these when the
/// data exists, so each is `None` until set (Python: attribute absent). Shared by `Program` and
/// `diagnostics::Diagnostics`; `dataclasses.replace` of a `Program` loses all of them.
#[derive(Debug, Clone, Default)]
pub struct LoadProvenance {
    /// `(position, end position)` of each initializer pull.
    pub pull_log: Option<Vec<(Vec<i64>, Vec<i64>)>>,
    /// Included file -> position of its `Include` line.
    pub pull_site: Option<OrderedMap<String, Vec<i64>>>,
    /// Include display name -> the path it resolved to.
    pub include_paths: Option<OrderedMap<String, PathBuf>>,
    /// File -> reported line -> physical spans.
    pub line_map: Option<OrderedMap<String, OrderedMap<i64, Vec<LineSpan>>>>,
    /// Every load of a file, in order.
    pub occurrences: Option<Vec<Occurrence>>,
}

/// Python `Program`.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Program {
    pub name: String,
    #[serde(serialize_with = "ser_rc")]
    pub statements: Vec<StmtRef>,
    /// `EndProg`'s line, else the main file's last physical line.
    pub last_line: i64,
    /// Parser diagnostics kept: those before the first `#...` statement ends.
    pub hash_cut: Option<usize>,
    /// A `#If` still open at the end of the file (before any `EndProg`).
    pub unclosed_if: bool,
    /// Statement count when each reported include error was read.
    pub include_error_at: Vec<usize>,
    /// Name (upper) -> resolved value for every `Const` the parser could fold in an active branch.
    pub consts: OrderedMap<String, ConstValue>,
    /// Python `LoadProvenance` base class.
    #[serde(skip)]
    pub provenance: LoadProvenance,
}

impl Program {
    /// Python `Program(name, statements)`.
    pub fn new(name: &str, statements: Vec<StmtRef>) -> Self {
        Self {
            name: name.to_string(),
            statements,
            ..Self::default()
        }
    }

    /// Python `dataclasses.replace(program, ...)` before the edits: the dataclass fields are
    /// copied (statements stay shared) but `provenance` is gone, as `replace` goes through
    /// `__init__`. Wrap the edited result in `Rc::new`.
    #[must_use]
    pub fn replaced(&self) -> Program {
        Program {
            provenance: LoadProvenance::default(),
            ..self.clone()
        }
    }
}

// ---------------------------------------------------------------- Placeholders until source.rs lands (P1-b)
// Fields exactly as declared in `source.py` / `lexer.py`; P1-b/P1-a replace these with the real
// definitions (and this file then re-exports them).

/// Python `lexer.Token` (frozen dataclass).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Token {
    /// `IDENT` | `NUMBER` | `STRING` | `OP` | `EOF`.
    pub kind: String,
    /// Original spelling.
    pub text: String,
    /// Case-folded form (identifiers/keywords only; same as `text` otherwise).
    pub upper: String,
    /// 0-based offset into the logical line's joined text.
    pub col: i64,
    /// Physical source line (assigned by the caller); default 0.
    pub line: i64,
    /// `NUMBER`: int or float; `STRING`: decoded contents; otherwise `None`.
    pub value: TokenValue,
}

/// Python `Token.value: object`.
#[derive(Debug, Clone, PartialEq, Default, Serialize)]
#[serde(untagged)]
pub enum TokenValue {
    #[default]
    None,
    Int(i64),
    Float(f64),
    Str(String),
}

/// Python `source.Piece` (`NamedTuple`).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Piece {
    pub start: i64,
    pub end: i64,
    pub file: String,
    pub line: i64,
    pub col: i64,
    /// Which load of that file (`Occurrence.id`).
    pub occurrence: i64,
}

/// Python `source.Occurrence` (`NamedTuple`): one load of a file.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Occurrence {
    pub id: i64,
    pub file: String,
    pub path: Option<PathBuf>,
    pub parent: Option<i64>,
    pub site: Option<(i64, i64)>,
}

/// Python `source.LineSpan` (`NamedTuple`): physical lines (first, last) of a logical line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LineSpan {
    pub first: i64,
    pub last: i64,
    pub occurrence: i64,
}

/// Python `source.LogicalLine`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LogicalLine {
    /// Joined, comment-stripped source text.
    pub text: String,
    /// Physical line to report.
    pub line: i64,
    /// Originating file's name.
    pub file: String,
    /// The `Name` argument of the enclosing `IncludeSection()`, if any.
    pub section: Option<String>,
    pub had_trailing_comment: bool,
    /// `code` | `overflow` | `include_begin` | `include_end` | `include_warning` | `include_error`.
    pub kind: String,
    /// CR6/CR1000X initializer pull: physical lines folded into `text`.
    pub pulls: i64,
    pub unpulled: Option<Vec<LogicalLine>>,
    /// (message, variable) reported only when read.
    pub bad_init: Option<(String, String)>,
    pub pieces: Vec<Piece>,
    /// Physical lines (first, last) the statement spans.
    pub phys: Option<(i64, i64)>,
    /// `Occurrence.id` of the load this line belongs to.
    pub occurrence: i64,
    /// An `IncludeSection` extension error.
    pub stops_includes: bool,
}

impl LogicalLine {
    /// Python `LogicalLine(text, line, file)` with every other field at its default.
    pub fn new(text: &str, line: i64, file: &str) -> Self {
        Self {
            text: text.to_string(),
            line,
            file: file.to_string(),
            section: None,
            had_trailing_comment: false,
            kind: "code".to_string(),
            pulls: 0,
            unpulled: None,
            bad_init: None,
            pieces: Vec::new(),
            phys: None,
            occurrence: 0,
            stops_includes: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn name(n: &str) -> ExprRef {
        Rc::new(Expr::Name(Name {
            node: Node::new(1, ""),
            name: n.to_string(),
        }))
    }

    fn number(line: i64, file: &str, value: i64, text: &str) -> ExprRef {
        Rc::new(Expr::Number(Number {
            node: Node::new(line, file),
            value: NumberValue::Int(value),
            text: text.to_string(),
        }))
    }

    fn instr(kw: &str, args: Vec<Option<ExprRef>>) -> StmtRef {
        Rc::new(Stmt::Instruction(Instruction {
            base: StmtBase::new(1, ""),
            keyword: kw.to_string(),
            args,
            ..Default::default()
        }))
    }

    fn unparsed(line: i64, text: &str) -> StmtRef {
        let mut base = StmtBase::new(line, "");
        base.text = text.to_string();
        Rc::new(Stmt::Unparsed(Unparsed {
            base,
            reason: "r".to_string(),
            expr: None,
        }))
    }

    fn if_stmt(inline: Option<Vec<StmtRef>>, inline_else: Option<Vec<StmtRef>>) -> StmtRef {
        Rc::new(Stmt::If(If {
            base: StmtBase::new(5, ""),
            cond: None,
            inline,
            inline_else,
        }))
    }

    fn label(st: &Stmt) -> String {
        match st {
            Stmt::Instruction(i) => i.keyword.clone(),
            other => other.text().to_string(),
        }
    }

    fn json_of<T: Serialize>(v: &T) -> serde_json::Value {
        serde_json::to_value(v).unwrap()
    }

    #[test]
    fn stmt_id_survives_clone_and_changes_on_replace() {
        let st = instr("SCAN", vec![]);
        let alias = st.clone();
        assert_eq!(stmt_id(&st), stmt_id(&alias));
        assert!(Rc::ptr_eq(&st, &alias));
        let replaced = replace_stmt(&st, |s| s.base_mut().at_eol.set(false));
        assert_ne!(stmt_id(&st), stmt_id(&replaced));
        assert!(st.at_eol());
        assert!(!replaced.at_eol());
        assert_ne!(stmt_id(&st), 0);
    }

    #[test]
    fn replace_shares_children_and_drops_adhoc_attributes() {
        let arg = name("x");
        let st = instr("SCAN", vec![Some(arg.clone())]);
        st.base().grouped.set(true);
        st.base().hoisted_last.set(Some(true));
        *st.base().dropped_tail.borrow_mut() = Some(vec![]);
        let new = replace_stmt(&st, |_| {});
        assert!(Rc::ptr_eq(new.args()[0].as_ref().unwrap(), &arg));
        assert!(!new.base().grouped.get());
        assert_eq!(new.base().hoisted_last.get(), None);
        assert!(new.base().dropped_tail.borrow().is_none());
        assert!(st.base().grouped.get());
    }

    #[test]
    fn program_replaced_drops_provenance_keeps_statements() {
        let st = instr("X", vec![]);
        let mut p = Program::new("p", vec![st.clone()]);
        p.provenance.pull_log = Some(vec![]);
        let q = p.replaced();
        assert!(q.provenance.pull_log.is_none());
        assert!(Rc::ptr_eq(&q.statements[0], &st));
        let (a, b) = (Rc::new(p), Rc::new(q));
        assert_ne!(program_id(&a), program_id(&b));
    }

    #[test]
    fn ends_routine_matches_python() {
        // Expected values observed from Python `syntax.ends_routine`.
        assert!(ends_routine(&instr("endsub", vec![])));
        assert!(ends_routine(&instr("EndFunction", vec![])));
        assert!(ends_routine(&instr("END", vec![Some(name("sub"))])));
        assert!(ends_routine(&instr("END", vec![Some(name("Function"))])));
        assert!(!ends_routine(&instr("END", vec![Some(name("if"))])));
        assert!(!ends_routine(&instr("END", vec![])));
        assert!(!ends_routine(&instr(
            "END",
            vec![Some(number(1, "", 1, "1"))]
        )));
        assert!(!ends_routine(&Stmt::Call(Call {
            name: "endsub".into(),
            ..Default::default()
        })));
    }

    #[test]
    fn hash_statement_matches_python() {
        let h = unparsed(3, "  #If X");
        assert!(Rc::ptr_eq(&hash_statement(&h).unwrap(), &h));
        assert!(hash_statement(&unparsed(3, "x #")).is_none());
        assert!(hash_statement(&instr("SCAN", vec![])).is_none());
        let i = if_stmt(
            Some(vec![instr("A", vec![]), h.clone(), instr("B", vec![])]),
            Some(vec![instr("C", vec![])]),
        );
        assert!(Rc::ptr_eq(&hash_statement(&i).unwrap(), &h));
    }

    #[test]
    fn cut_after_hash_matches_python() {
        let h = unparsed(3, "  #If X");
        // `#` in the Then body: later body statements and the whole Else body go.
        let i = if_stmt(
            Some(vec![instr("A", vec![]), h.clone(), instr("B", vec![])]),
            Some(vec![instr("C", vec![])]),
        );
        let c = cut_after_hash(&i);
        assert!(!Rc::ptr_eq(&c, &i));
        let kept: Vec<String> = c.inline().unwrap().iter().map(|s| label(s)).collect();
        assert_eq!(kept, ["A", "  #If X"]);
        assert_eq!(c.inline_else().unwrap().len(), 0);
        assert!(Rc::ptr_eq(&c.inline().unwrap()[1], &h));
        // `#` in the Else body: the Then body is shared untouched.
        let i2 = if_stmt(
            Some(vec![instr("A", vec![])]),
            Some(vec![instr("C", vec![]), h.clone(), instr("D", vec![])]),
        );
        let c2 = cut_after_hash(&i2);
        assert_eq!(c2.inline().unwrap().len(), 1);
        assert!(Rc::ptr_eq(
            &c2.inline().unwrap()[0],
            &i2.inline().unwrap()[0]
        ));
        assert_eq!(c2.inline_else().unwrap().len(), 2);
        // Nothing to cut: the same `Rc` comes back.
        let i3 = if_stmt(Some(vec![instr("A", vec![])]), None);
        assert!(Rc::ptr_eq(&cut_after_hash(&i3), &i3));
        assert!(Rc::ptr_eq(&cut_after_hash(&h), &h));
        // Nested `If`: recursion cuts the inner body too.
        let inner = if_stmt(Some(vec![h.clone(), instr("Z", vec![])]), None);
        let nested = if_stmt(Some(vec![instr("A", vec![]), inner.clone()]), None);
        let cn = cut_after_hash(&nested);
        assert_eq!(cn.inline().unwrap().len(), 2);
        assert_eq!(cn.inline().unwrap()[1].inline().unwrap().len(), 1);
        assert!(!Rc::ptr_eq(&cn.inline().unwrap()[1], &inner));
    }

    #[test]
    fn missing_required_params_matches_python() {
        let sd = SubDef {
            params: vec![
                Rc::new(Param::new("a")),
                Rc::new(Param {
                    default: Some(number(1, "", 1, "1")),
                    ..Param::new("b")
                }),
                Rc::new(Param::new("c")),
            ],
            ..Default::default()
        };
        let x = Some(name("x"));
        assert_eq!(sd.missing_required_params(&[]), 2);
        assert_eq!(sd.missing_required_params(&[None, None, None]), 2);
        assert_eq!(sd.missing_required_params(std::slice::from_ref(&x)), 1);
        assert_eq!(sd.missing_required_params(&[x.clone(), None, x]), 0);
    }

    #[test]
    fn array_literal_helpers_cover_juxtaposed() {
        let lit = Expr::ArrayLiteral(ArrayLiteral {
            items: vec![Some(name("a"))],
            ..Default::default()
        });
        let jux = Expr::Juxtaposed(Juxtaposed {
            items: vec![None],
            ..Default::default()
        });
        assert!(lit.is_array_literal() && jux.is_array_literal());
        assert!(!Expr::Name(Name::default()).is_array_literal());
        assert_eq!(lit.array_items().unwrap().len(), 1);
        assert_eq!(jux.array_items().unwrap().len(), 1);
    }

    #[test]
    fn json_shape_matches_python_asdict() {
        // Expected values printed by Python `dataclasses.asdict` (+ "type").
        let scan = Stmt::Instruction(Instruction {
            base: StmtBase {
                text: "Scan(1,Sec,0,0)".into(),
                ..StmtBase::new(2, "f.cr6")
            },
            keyword: "SCAN".into(),
            args: vec![
                Some(number(2, "f.cr6", 1, "1")),
                None,
                Some(Rc::new(Expr::Name(Name {
                    node: Node::new(2, "f.cr6"),
                    name: "Sec".into(),
                }))),
            ],
            arg_spans: vec![Some((5, 6)), None, Some((8, 10))],
            ..Default::default()
        });
        assert_eq!(
            json_of(&scan),
            json!({"type": "Instruction", "arg_spans": [[5, 6], null, [8, 10]],
                "args": [{"type": "Number", "file": "f.cr6", "line": 2, "text": "1", "value": 1}, null,
                         {"type": "Name", "file": "f.cr6", "line": 2, "name": "Sec"}],
                "at_eol": true, "file": "f.cr6", "keyword": "SCAN", "line": 2, "text": "Scan(1,Sec,0,0)"})
        );
        let decl = Stmt::Declaration(Declaration {
            base: StmtBase {
                text: "Public X".into(),
                ..StmtBase::new(1, "")
            },
            kind: "PUBLIC".into(),
            vars: vec![Rc::new(VarDecl {
                dims: vec![number(1, "", 4, "4")],
                type_: Some("FLOAT".into()),
                dim0_span: Some((9, 10)),
                ..VarDecl::new("X")
            })],
        });
        assert_eq!(
            json_of(&decl),
            json!({"type": "Declaration", "at_eol": true, "file": "", "kind": "PUBLIC", "line": 1, "text": "Public X",
                "vars": [{"dim0_span": [9, 10], "dims": [{"type": "Number", "file": "", "line": 1, "text": "4", "value": 4}],
                          "empty_type_init": false, "init": null, "junk_dims": [], "name": "X", "paren_dim": false,
                          "string_size": null, "type": "FLOAT", "units": null}]})
        );
        let case = Stmt::Case(Case {
            base: StmtBase::new(1, ""),
            items: vec![
                CaseItem::Value(name("a")),
                CaseItem::Range(name("b"), name("c")),
                CaseItem::Is(">".into(), name("d")),
            ],
            is_else: false,
        });
        let n = |s: &str| json!({"type": "Name", "file": "", "line": 1, "name": s});
        assert_eq!(
            json_of(&case),
            json!({"type": "Case", "at_eol": true, "file": "", "is_else": false, "line": 1, "text": "",
                "items": [["value", n("a")], ["range", n("b"), n("c")], ["is", ">", n("d")]]})
        );
        let mut p = Program::new("p", vec![instr("X", vec![])]);
        p.last_line = 3;
        p.consts.insert("A".into(), ConstValue::Int(1));
        p.consts.insert("B".into(), ConstValue::Float(2.5));
        p.consts.insert("C".into(), ConstValue::Str("s".into()));
        assert_eq!(
            json_of(&p),
            json!({"consts": {"A": 1, "B": 2.5, "C": "s"}, "hash_cut": null, "include_error_at": [], "last_line": 3,
                "name": "p", "unclosed_if": false,
                "statements": [{"type": "Instruction", "arg_spans": [], "args": [], "at_eol": true, "file": "",
                                "keyword": "X", "line": 1, "text": ""}]})
        );
        let sub = Stmt::SubDef(SubDef {
            base: StmtBase::new(1, ""),
            keyword: "SUB".into(),
            name: "s".into(),
            params: vec![Rc::new(Param {
                by_ref: Some(true),
                type_: Some("LONG".into()),
                ..Param::new("a")
            })],
            header_errors: vec!["e".into()],
            ..Default::default()
        });
        assert_eq!(
            json_of(&sub),
            json!({"type": "SubDef", "at_eol": true, "file": "", "header_errors": ["e"], "keyword": "SUB", "line": 1,
                "name": "s", "return_type": null, "text": "",
                "params": [{"by_ref": true, "default": null, "dim_exprs": [], "dims": 0, "name": "a",
                            "string_size": null, "type": "LONG"}]})
        );
        let bin = Expr::Binary(Binary {
            node: Node::new(1, ""),
            op: "+".into(),
            left: Some(name("a")),
            right: Some(Rc::new(Expr::Juxtaposed(Juxtaposed {
                node: Node::new(1, ""),
                items: vec![None],
            }))),
        });
        assert_eq!(
            json_of(&bin),
            json!({"type": "Binary", "file": "", "left": n("a"), "line": 1, "op": "+",
                "right": {"type": "Juxtaposed", "file": "", "items": [null], "line": 1}})
        );
    }

    #[test]
    fn ordered_map_keeps_python_dict_order() {
        let mut m: OrderedMap<String, i64> = OrderedMap::new();
        m.insert("b".into(), 1);
        m.insert("a".into(), 2);
        m.insert("b".into(), 3);
        assert_eq!(m.keys().cloned().collect::<Vec<_>>(), ["b", "a"]);
        assert_eq!(m.get("b"), Some(&3));
        assert_eq!(m.remove("b"), Some(3));
        m.insert("b".into(), 4);
        assert_eq!(
            m.iter().map(|(k, v)| (k.as_str(), *v)).collect::<Vec<_>>(),
            [("a", 2), ("b", 4)]
        );
        assert_eq!(m.get("a"), Some(&2));
    }
}
