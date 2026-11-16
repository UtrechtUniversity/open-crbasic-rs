//! Port of open-crbasic `profiles.py`: the four logger profiles and their data tables.
//!
//! The JSON tables under `data/` are embedded with `include_str!` and parsed on first use, one
//! table at a time (Python's `cached_property`). There is no fallback between directories: a
//! profile reads only its own `data_dir`; a missing optional file means the Python default
//! (empty set / `{}` / `{"tasks": {}}`), not the top-level file. CR6 uses `data/` itself.
//!
//! Raw dict tables are kept as [`Json`], whose objects keep file order (`serde_json`'s map does not).

use std::collections::{HashMap, HashSet};
use std::fmt::{self, Write as _};
use std::path::Path;
use std::sync::OnceLock;

use serde::Deserialize;
use serde::de::{self, Deserializer, MapAccess, SeqAccess, Visitor};

/// Stand-in for `Infinity` while parsing: `serde_json` rejects the token, Python's `json` accepts it.
const INF_SENTINEL: f64 = 1.5e308;

/// A JSON value whose objects keep their file order (Python dict insertion order).
#[derive(Debug, Clone, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    Int(i64),
    /// Includes the non-standard `Infinity`/`-Infinity` that Python's `json` accepts.
    Float(f64),
    String(String),
    Array(Vec<Json>),
    Object(Vec<(String, Json)>),
}

impl Json {
    /// Python `dict.get(key)`: the last of any duplicate keys wins.
    pub fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Object(o) => o.iter().rev().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Json::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&[Json]> {
        match self {
            Json::Array(a) => Some(a),
            _ => None,
        }
    }

    /// The `(key, value)` pairs in file order.
    pub fn as_object(&self) -> Option<&[(String, Json)]> {
        match self {
            Json::Object(o) => Some(o),
            _ => None,
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Json::Int(n) => Some(*n),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            #[allow(clippy::cast_precision_loss, reason = "Python int to float")]
            Json::Int(n) => Some(*n as f64),
            Json::Float(f) => Some(*f),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Json::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// Number of elements of an array, entries of an object, or 0 for a scalar.
    pub fn len(&self) -> usize {
        match self {
            Json::Array(a) => a.len(),
            Json::Object(o) => o.len(),
            _ => 0,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl<'de> Deserialize<'de> for Json {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = Json;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("any JSON value")
            }
            fn visit_unit<E>(self) -> Result<Json, E> {
                Ok(Json::Null)
            }
            fn visit_bool<E>(self, v: bool) -> Result<Json, E> {
                Ok(Json::Bool(v))
            }
            fn visit_i64<E>(self, v: i64) -> Result<Json, E> {
                Ok(Json::Int(v))
            }
            fn visit_u64<E: de::Error>(self, v: u64) -> Result<Json, E> {
                i64::try_from(v)
                    .map(Json::Int)
                    .map_err(|_| E::custom("integer out of range"))
            }
            fn visit_f64<E>(self, v: f64) -> Result<Json, E> {
                Ok(Json::Float(if v == INF_SENTINEL {
                    f64::INFINITY
                } else if v == -INF_SENTINEL {
                    f64::NEG_INFINITY
                } else {
                    v
                }))
            }
            fn visit_str<E>(self, v: &str) -> Result<Json, E> {
                Ok(Json::String(v.to_owned()))
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut a: A) -> Result<Json, A::Error> {
                let mut out = Vec::new();
                while let Some(v) = a.next_element()? {
                    out.push(v);
                }
                Ok(Json::Array(out))
            }
            fn visit_map<A: MapAccess<'de>>(self, mut a: A) -> Result<Json, A::Error> {
                let mut out = Vec::new();
                while let Some(kv) = a.next_entry()? {
                    out.push(kv);
                }
                Ok(Json::Object(out))
            }
        }
        d.deserialize_any(V)
    }
}

/// An insertion-ordered dict keyed by upper-cased name (`{r["name"].upper(): r ...}`).
#[derive(Debug)]
pub struct Table<T> {
    rows: Vec<(String, T)>,
    index: HashMap<String, usize>,
}

impl<T> Table<T> {
    /// Python dict semantics for a repeated key: first position, last value.
    fn new(rows: Vec<(String, T)>) -> Self {
        let mut index: HashMap<String, usize> = HashMap::new();
        let mut out: Vec<(String, T)> = Vec::new();
        for (k, v) in rows {
            if let Some(&i) = index.get(&k) {
                out[i].1 = v;
            } else {
                index.insert(k.clone(), out.len());
                out.push((k, v));
            }
        }
        Table { rows: out, index }
    }

    /// Lookup by the already upper-cased key.
    pub fn get(&self, key: &str) -> Option<&T> {
        self.index.get(key).map(|&i| &self.rows[i].1)
    }

    pub fn contains(&self, key: &str) -> bool {
        self.index.contains_key(key)
    }

    /// `(upper-cased name, record)` in file order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &T)> {
        self.rows.iter().map(|(k, v)| (k.as_str(), v))
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
}

/// A row of `instructions.json`; `id` is `index + 1` as in the Python property.
#[derive(Debug, Clone, Deserialize)]
pub struct Instruction {
    pub index: i64,
    pub name: String,
    pub params: Vec<i64>,
    pub compile_fn: String,
    pub runtime_fn: Option<String>,
    #[serde(default)]
    pub id: i64,
}

/// A row of `functions.json`; `nargs`/`argtypes` are absent on some rows.
#[derive(Debug, Clone, Deserialize)]
pub struct Function {
    pub name: String,
    pub code: i64,
    pub kind: i64,
    pub argspec: i64,
    pub nargs: Option<i64>,
    pub argtypes: Option<Vec<i64>>,
}

/// A row of `symbols.json`.
#[derive(Debug, Clone, Deserialize)]
pub struct Symbol {
    pub name: String,
    pub value: i64,
    pub fvalue: f64,
    pub extra: i64,
}

/// A row of `ess_variables.json` (`checker/ess.py::_table`), in file order.
#[derive(Debug, Clone, Deserialize)]
pub struct EssVariable {
    pub name: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub count: i64,
}

/// An undeclared name used as `Name.member` in an expression (CR300/CR350 read it as a table).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DottedName(pub String);

/// The `name=` argument of `Message::format`: Python passes a `str` or its subclass `DottedName`.
pub trait NameArg {
    fn text(&self) -> &str;
    fn is_dotted(&self) -> bool {
        false
    }
}

impl NameArg for str {
    fn text(&self) -> &str {
        self
    }
}

impl NameArg for String {
    fn text(&self) -> &str {
        self
    }
}

impl NameArg for DottedName {
    fn text(&self) -> &str {
        &self.0
    }
    fn is_dotted(&self) -> bool {
        true
    }
}

/// A message template with a `{name}` placeholder (Python `str.format(name=...)`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Message {
    /// A plain `str` template.
    Plain(&'static str),
    /// `DottedAwareMessage`: a dotted name yields `Could not find DataTable <name>.`.
    DottedAware(&'static str),
}

impl Message {
    pub fn template(&self) -> &'static str {
        match self {
            Message::Plain(t) | Message::DottedAware(t) => t,
        }
    }

    /// `template.format(name=name)`; a name that is a Python `DottedName` takes the dotted wording.
    pub fn format<N: NameArg + ?Sized>(&self, name: &N) -> String {
        match self {
            Message::DottedAware(_) if name.is_dotted() => {
                format!("Could not find DataTable {}.", name.text())
            }
            _ => format_name(self.template(), name.text()),
        }
    }

    /// `template.format(name=name)` for a `DottedName`.
    pub fn format_dotted(&self, name: &DottedName) -> String {
        match self {
            Message::Plain(t) => format_name(t, &name.0),
            Message::DottedAware(_) => format!("Could not find DataTable {}.", name.0),
        }
    }
}

/// `template.format(name=name)` for the templates in this module (only `{name}` occurs).
pub fn format_name(template: &str, name: &str) -> String {
    template.replace("{name}", name)
}

/// One lazily parsed table per `cached_property`.
struct Cache {
    instructions: OnceLock<Table<Instruction>>,
    functions: OnceLock<Table<Function>>,
    symbols: OnceLock<Table<Symbol>>,
    status_members: OnceLock<HashSet<String>>,
    mode_rules: OnceLock<Json>,
    pipeline_tasks: OnceLock<Json>,
    terminals: OnceLock<Json>,
    fnotch: OnceLock<Json>,
    measure: OnceLock<Json>,
    settings_fields: OnceLock<Json>,
    before_beginprog: OnceLock<HashSet<String>>,
    background_calibration: OnceLock<Json>,
    ess_variables: OnceLock<Vec<EssVariable>>,
}

impl Cache {
    const fn new() -> Self {
        Cache {
            instructions: OnceLock::new(),
            functions: OnceLock::new(),
            symbols: OnceLock::new(),
            status_members: OnceLock::new(),
            mode_rules: OnceLock::new(),
            pipeline_tasks: OnceLock::new(),
            terminals: OnceLock::new(),
            fnotch: OnceLock::new(),
            measure: OnceLock::new(),
            settings_fields: OnceLock::new(),
            before_beginprog: OnceLock::new(),
            background_calibration: OnceLock::new(),
            ess_variables: OnceLock::new(),
        }
    }
}

static CACHES: [Cache; 4] = [const { Cache::new() }; 4];

/// The embedded text of `data/<dir>/<name>.json`, `None` when that directory has no such file.
fn source(dir: &str, name: &str) -> Option<&'static str> {
    match (dir, name) {
        ("", "background_calibration") => Some(include_str!("../data/background_calibration.json")),
        ("", "before_beginprog") => Some(include_str!("../data/before_beginprog.json")),
        ("cr1000x", "background_calibration") => {
            Some(include_str!("../data/cr1000x/background_calibration.json"))
        }
        ("cr1000x", "before_beginprog") => {
            Some(include_str!("../data/cr1000x/before_beginprog.json"))
        }
        ("cr1000x", "ess_variables") => Some(include_str!("../data/cr1000x/ess_variables.json")),
        ("cr1000x", "fnotch") => Some(include_str!("../data/cr1000x/fnotch.json")),
        ("cr1000x", "functions") => Some(include_str!("../data/cr1000x/functions.json")),
        ("cr1000x", "instructions") => Some(include_str!("../data/cr1000x/instructions.json")),
        ("cr1000x", "measure") => Some(include_str!("../data/cr1000x/measure.json")),
        ("cr1000x", "mode") => Some(include_str!("../data/cr1000x/mode.json")),
        ("cr1000x", "pipeline_tasks") => Some(include_str!("../data/cr1000x/pipeline_tasks.json")),
        ("cr1000x", "settings_fields") => {
            Some(include_str!("../data/cr1000x/settings_fields.json"))
        }
        ("cr1000x", "status_members") => Some(include_str!("../data/cr1000x/status_members.json")),
        ("cr1000x", "symbols") => Some(include_str!("../data/cr1000x/symbols.json")),
        ("cr1000x", "terminals") => Some(include_str!("../data/cr1000x/terminals.json")),
        ("cr300", "before_beginprog") => Some(include_str!("../data/cr300/before_beginprog.json")),
        ("cr300", "ess_variables") => Some(include_str!("../data/cr300/ess_variables.json")),
        ("cr300", "fnotch") => Some(include_str!("../data/cr300/fnotch.json")),
        ("cr300", "functions") => Some(include_str!("../data/cr300/functions.json")),
        ("cr300", "instructions") => Some(include_str!("../data/cr300/instructions.json")),
        ("cr300", "measure") => Some(include_str!("../data/cr300/measure.json")),
        ("cr300", "mode") => Some(include_str!("../data/cr300/mode.json")),
        ("cr300", "settings_fields") => Some(include_str!("../data/cr300/settings_fields.json")),
        ("cr300", "status_members") => Some(include_str!("../data/cr300/status_members.json")),
        ("cr300", "symbols") => Some(include_str!("../data/cr300/symbols.json")),
        ("cr300", "terminals") => Some(include_str!("../data/cr300/terminals.json")),
        ("cr350", "before_beginprog") => Some(include_str!("../data/cr350/before_beginprog.json")),
        ("cr350", "ess_variables") => Some(include_str!("../data/cr350/ess_variables.json")),
        ("cr350", "fnotch") => Some(include_str!("../data/cr350/fnotch.json")),
        ("cr350", "functions") => Some(include_str!("../data/cr350/functions.json")),
        ("cr350", "instructions") => Some(include_str!("../data/cr350/instructions.json")),
        ("cr350", "measure") => Some(include_str!("../data/cr350/measure.json")),
        ("cr350", "mode") => Some(include_str!("../data/cr350/mode.json")),
        ("cr350", "settings_fields") => Some(include_str!("../data/cr350/settings_fields.json")),
        ("cr350", "status_members") => Some(include_str!("../data/cr350/status_members.json")),
        ("cr350", "symbols") => Some(include_str!("../data/cr350/symbols.json")),
        ("cr350", "terminals") => Some(include_str!("../data/cr350/terminals.json")),
        ("", "ess_variables") => Some(include_str!("../data/ess_variables.json")),
        ("", "fnotch") => Some(include_str!("../data/fnotch.json")),
        ("", "functions") => Some(include_str!("../data/functions.json")),
        ("", "instructions") => Some(include_str!("../data/instructions.json")),
        ("", "measure") => Some(include_str!("../data/measure.json")),
        ("", "mode") => Some(include_str!("../data/mode.json")),
        ("", "pipeline_tasks") => Some(include_str!("../data/pipeline_tasks.json")),
        ("", "settings_fields") => Some(include_str!("../data/settings_fields.json")),
        ("", "status_members") => Some(include_str!("../data/status_members.json")),
        ("", "symbols") => Some(include_str!("../data/symbols.json")),
        ("", "terminals") => Some(include_str!("../data/terminals.json")),
        _ => None,
    }
}

fn parse<T: for<'de> Deserialize<'de>>(dir: &str, name: &str, text: &str) -> T {
    // Only `data/cr300/terminals.json` contains `-Infinity` (as a bare number token).
    let text = text.replace("Infinity", "1.5e308");
    serde_json::from_str(&text)
        .unwrap_or_else(|e| panic!("embedded data/{dir}/{name}.json does not parse: {e}"))
}

/// `Profile._load(name)`: a required table.
fn load<T: for<'de> Deserialize<'de>>(dir: &str, name: &str) -> T {
    let text = source(dir, name).unwrap_or_else(|| panic!("data/{dir}/{name}.json is missing"));
    parse(dir, name, text)
}

/// An optional table: the Python default (given as JSON text) when the file does not exist.
fn load_or<T: for<'de> Deserialize<'de>>(dir: &str, name: &str, default: &str) -> T {
    parse(dir, name, source(dir, name).unwrap_or(default))
}

fn table<T>(rows: Vec<T>, name: impl Fn(&T) -> &str) -> Table<T> {
    Table::new(
        rows.into_iter()
            .map(|r| (name(&r).to_ascii_uppercase(), r))
            .collect(),
    )
}

/// Logger profile: the per-model data and conventions the compiler is parameterised by.
#[derive(Debug)]
pub struct Profile {
    /// `"cr6"`, `"cr1000x"`, ...
    pub name: &'static str,
    /// Logger name as the exe prints it.
    pub display: &'static str,
    /// OS version string of the reference compiler.
    pub version: &'static str,
    /// Build date string of the reference compiler.
    pub date: &'static str,
    /// Directory of the tables, relative to the repository root (`data`, `data/cr300`, ...).
    pub data_dir: &'static str,
    /// True: "Compiled in <X>Mode."; False: "Compiled OK."
    pub reports_mode: bool,
    /// Reference compiler file name.
    pub exe_name: &'static str,
    /// Wording of an unresolved predefined-symbol name at a checked expression position.
    pub unresolved_symbol_message: Message,
    pub early_warnings_gated: bool,
    pub undefined_call_message: bool,
    pub fallback_echo_newline: bool,
    pub known_first_word_pass1: bool,
    pub include_drive_prefix: bool,
    pub include_warning_first: bool,
    pub single_pass: bool,
    /// Identifier exceptions to the predefined-CONST namespace rule (a `frozenset`).
    pub declarable_predefined_names: &'static [&'static str],
}

impl Profile {
    fn dir(&self) -> &'static str {
        self.data_dir
            .strip_prefix("data")
            .unwrap_or("")
            .trim_start_matches('/')
    }

    fn cache(&self) -> &'static Cache {
        let i = match self.name {
            "cr6" => 0,
            "cr1000x" => 1,
            "cr300" => 2,
            _ => 3,
        };
        &CACHES[i]
    }

    /// Upper-case name -> record (with `id` = table index + 1).
    pub fn instructions(&self) -> &'static Table<Instruction> {
        self.cache().instructions.get_or_init(|| {
            let mut rows: Vec<Instruction> = load(self.dir(), "instructions");
            for r in &mut rows {
                r.id = r.index + 1;
            }
            table(rows, |r| &r.name)
        })
    }

    pub fn functions(&self) -> &'static Table<Function> {
        self.cache()
            .functions
            .get_or_init(|| table(load(self.dir(), "functions"), |r: &Function| &r.name))
    }

    pub fn symbols(&self) -> &'static Table<Symbol> {
        self.cache()
            .symbols
            .get_or_init(|| table(load(self.dir(), "symbols"), |r: &Symbol| &r.name))
    }

    /// `Status.<name>` read names as written in `status_members.json`; empty: unchecked.
    pub fn status_members(&self) -> &'static HashSet<String> {
        self.cache().status_members.get_or_init(|| {
            #[derive(Deserialize)]
            struct F {
                names: Vec<String>,
            }
            let f: F = load_or(self.dir(), "status_members", r#"{"names":[]}"#);
            f.names.into_iter().collect()
        })
    }

    /// Sequential/PipelineMode trigger lists (`mode.json`, required).
    pub fn mode_rules(&self) -> &'static Json {
        self.cache()
            .mode_rules
            .get_or_init(|| load(self.dir(), "mode"))
    }

    /// `PipeLine` Mode placement rules; `{"tasks": {}}` when the logger has no file.
    pub fn pipeline_tasks(&self) -> &'static Json {
        self.cache()
            .pipeline_tasks
            .get_or_init(|| load_or(self.dir(), "pipeline_tasks", r#"{"tasks":{}}"#))
    }

    /// Channel-argument range checks; `{"channels": {}, "instructions": {}}` when absent.
    pub fn terminals(&self) -> &'static Json {
        self.cache().terminals.get_or_init(|| {
            load_or(
                self.dir(),
                "terminals",
                r#"{"channels":{},"instructions":{}}"#,
            )
        })
    }

    /// `fnotch` argument check; `{}` when absent.
    pub fn fnotch(&self) -> &'static Json {
        self.cache()
            .fnotch
            .get_or_init(|| load_or(self.dir(), "fnotch", "{}"))
    }

    /// Value-set/value-range checks; `{}` when absent.
    pub fn measure(&self) -> &'static Json {
        self.cache()
            .measure
            .get_or_init(|| load_or(self.dir(), "measure", "{}"))
    }

    /// `SetSetting`/`SetStatus` field-name lists (`settings_fields.json`, required).
    pub fn settings_fields(&self) -> &'static Json {
        self.cache()
            .settings_fields
            .get_or_init(|| load(self.dir(), "settings_fields"))
    }

    /// Instructions rejected before `BeginProg`, as written in the file.
    pub fn before_beginprog(&self) -> &'static HashSet<String> {
        self.cache().before_beginprog.get_or_init(|| {
            #[derive(Deserialize)]
            struct F {
                instructions: Vec<String>,
            }
            let f: F = load_or(self.dir(), "before_beginprog", r#"{"instructions":[]}"#);
            f.instructions.into_iter().collect()
        })
    }

    /// `Scan` interval threshold for the background-calibration warning; `{}` when absent.
    pub fn background_calibration(&self) -> &'static Json {
        self.cache()
            .background_calibration
            .get_or_init(|| load_or(self.dir(), "background_calibration", "{}"))
    }

    /// Rows of `ess_variables.json` in file order (`checker/ess.py::_table`); empty when absent.
    pub fn ess_variables(&self) -> &'static [EssVariable] {
        self.cache()
            .ess_variables
            .get_or_init(|| load_or(self.dir(), "ess_variables", "[]"))
    }
}

const DECLARABLE_NUMERIC_TYPES: &[&str] = &[
    "FLOAT", "LONG", "BOOLEAN", "IEEE8", "IEEE4", "BOOL8", "UINT1", "UINT2", "UINT4", "DOUBLE",
];

/// The four profiles in `PROFILES` order.
pub static PROFILES: [Profile; 4] = [
    Profile {
        name: "cr6",
        display: "CR6",
        version: "CR6.14.2.0",
        date: "03/04/2025",
        data_dir: "data",
        reports_mode: true,
        exe_name: "CR6Comp.exe",
        unresolved_symbol_message: Message::Plain("Invalid symbol in expression: {name}."),
        early_warnings_gated: false,
        undefined_call_message: false,
        fallback_echo_newline: false,
        known_first_word_pass1: true,
        include_drive_prefix: false,
        include_warning_first: true,
        single_pass: false,
        declarable_predefined_names: DECLARABLE_NUMERIC_TYPES,
    },
    Profile {
        name: "cr1000x",
        display: "CR1000X",
        version: "CR1000X.8.2.0",
        date: "03/04/2025",
        data_dir: "data/cr1000x",
        reports_mode: true,
        exe_name: "cr1000xcomp.exe",
        unresolved_symbol_message: Message::Plain("Invalid symbol in expression: {name}."),
        early_warnings_gated: false,
        undefined_call_message: false,
        fallback_echo_newline: false,
        known_first_word_pass1: true,
        include_drive_prefix: false,
        include_warning_first: true,
        single_pass: false,
        declarable_predefined_names: DECLARABLE_NUMERIC_TYPES,
    },
    Profile {
        name: "cr300",
        display: "CR300",
        version: "CR300.Std.11.02",
        date: "02/04/2025",
        data_dir: "data/cr300",
        reports_mode: false,
        exe_name: "cr300comp.exe",
        unresolved_symbol_message: Message::DottedAware("{name} is not defined."),
        early_warnings_gated: true,
        undefined_call_message: true,
        fallback_echo_newline: true,
        known_first_word_pass1: false,
        include_drive_prefix: true,
        include_warning_first: false,
        single_pass: true,
        declarable_predefined_names: &[],
    },
    Profile {
        name: "cr350",
        display: "CR350",
        version: "CR350.Std.1.07",
        date: "02/04/2025",
        data_dir: "data/cr350",
        reports_mode: false,
        exe_name: "cr350comp.exe",
        unresolved_symbol_message: Message::DottedAware("{name} is not defined."),
        early_warnings_gated: true,
        undefined_call_message: true,
        fallback_echo_newline: true,
        known_first_word_pass1: false,
        include_drive_prefix: true,
        include_warning_first: false,
        single_pass: true,
        declarable_predefined_names: &[],
    },
];

/// `EXTENSIONS`: lower-case extension -> profile name.
pub const EXTENSIONS: [(&str, &str); 4] = [
    (".cr6", "cr6"),
    (".cr1x", "cr1000x"),
    (".cr300", "cr300"),
    (".cr350", "cr350"),
];

/// The `ValueError` of `profiles.get`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownLogger(pub String);

impl fmt::Display for UnknownLogger {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for UnknownLogger {}

/// Python `repr()` of a str.
fn py_repr(s: &str) -> String {
    let quote = if s.contains('\'') && !s.contains('"') {
        '"'
    } else {
        '\''
    };
    let mut out = String::from(quote);
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c == quote => {
                out.push('\\');
                out.push(c);
            }
            c if c.is_control() => {
                let _ = write!(out, "\\x{:02x}", u32::from(c));
            }
            c => out.push(c),
        }
    }
    out.push(quote);
    out
}

/// Port of `profiles.py::get`
pub fn get(name: &str) -> Result<&'static Profile, UnknownLogger> {
    let lower = name.to_lowercase();
    PROFILES.iter().find(|p| p.name == lower).ok_or_else(|| {
        let names: Vec<&str> = PROFILES.iter().map(|p| p.name).collect();
        UnknownLogger(format!(
            "unknown logger {}; choose from {}",
            py_repr(name),
            names.join(", ")
        ))
    })
}

/// Python `PurePath.suffix` of the final component.
fn suffix(path: &Path) -> String {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    match name.rfind('.') {
        Some(i) if i > 0 && i + 1 < name.len() => name[i..].to_owned(),
        _ => String::new(),
    }
}

/// Port of `profiles.py::for_file` with `default="cr6"`: the profile implied by the extension.
pub fn for_file(path: &Path) -> &'static Profile {
    for_file_or(path, "cr6").expect("cr6 is a known logger")
}

/// Port of `profiles.py::for_file` with an explicit `default` logger name.
pub fn for_file_or(path: &Path, default: &str) -> Result<&'static Profile, UnknownLogger> {
    let ext = suffix(path).to_lowercase();
    let name = EXTENSIONS
        .iter()
        .find(|(e, _)| *e == ext)
        .map_or(default, |(_, n)| n);
    get(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn keys(j: &Json, key: &str) -> Vec<String> {
        let o = j.get(key).and_then(Json::as_object).expect("object");
        o.iter().map(|(k, _)| k.clone()).collect()
    }

    // name, display, version, date, reports_mode, exe, instructions, functions, symbols,
    // status_members, before_beginprog, settings, status, tasks, channels, terminal instructions,
    // fnotch keys, measure keys, background_calibration keys, task_instructions (Python, observed)
    const COUNTS: [(&str, [usize; 14]); 4] = [
        (
            "cr6",
            [410, 134, 150, 314, 95, 257, 55, 59, 10, 37, 4, 13, 2, 64],
        ),
        (
            "cr1000x",
            [403, 134, 145, 313, 134, 259, 52, 63, 13, 35, 4, 10, 2, 61],
        ),
        (
            "cr300",
            [246, 127, 97, 297, 38, 209, 40, 0, 8, 17, 3, 4, 0, 64],
        ),
        (
            "cr350",
            [247, 127, 95, 298, 40, 232, 50, 0, 7, 17, 3, 6, 0, 64],
        ),
    ];

    #[test]
    fn table_sizes_match_python() {
        for (name, c) in COUNTS {
            let p = get(name).unwrap();
            let got = [
                p.instructions().len(),
                p.functions().len(),
                p.symbols().len(),
                p.status_members().len(),
                p.before_beginprog().len(),
                p.settings_fields().get("settings").unwrap().len(),
                p.settings_fields().get("status").unwrap().len(),
                p.pipeline_tasks().get("tasks").unwrap().len(),
                p.terminals().get("channels").unwrap().len(),
                p.terminals().get("instructions").unwrap().len(),
                p.fnotch().len(),
                p.measure().len(),
                p.background_calibration().len(),
                p.mode_rules().get("task_instructions").unwrap().len(),
            ];
            assert_eq!(got, c, "{name}");
            assert_eq!(p.ess_variables().len(), 217, "{name}");
        }
    }

    #[test]
    fn scalars() {
        let p = get("cr6").unwrap();
        assert_eq!(
            (p.display, p.version, p.date, p.exe_name),
            ("CR6", "CR6.14.2.0", "03/04/2025", "CR6Comp.exe")
        );
        assert!(p.reports_mode && p.include_warning_first && p.known_first_word_pass1);
        assert!(!p.single_pass && !p.early_warnings_gated && !p.include_drive_prefix);
        assert_eq!(
            p.unresolved_symbol_message.template(),
            "Invalid symbol in expression: {name}."
        );
        assert_eq!(p.declarable_predefined_names.len(), 10);
        let p = get("cr1000x").unwrap();
        assert_eq!(
            (p.version, p.exe_name, p.data_dir),
            ("CR1000X.8.2.0", "cr1000xcomp.exe", "data/cr1000x")
        );
        for n in ["cr300", "cr350"] {
            let p = get(n).unwrap();
            assert_eq!(p.date, "02/04/2025");
            assert!(!p.reports_mode && p.single_pass && p.early_warnings_gated);
            assert!(p.undefined_call_message && p.fallback_echo_newline && p.include_drive_prefix);
            assert!(!p.known_first_word_pass1 && !p.include_warning_first);
            assert_eq!(p.declarable_predefined_names.len(), 0);
            assert_eq!(
                p.unresolved_symbol_message.template(),
                "{name} is not defined."
            );
        }
        assert_eq!(get("cr300").unwrap().version, "CR300.Std.11.02");
        assert_eq!(get("cr350").unwrap().version, "CR350.Std.1.07");
    }

    #[test]
    fn entries_and_order() {
        for (name, first, last, last_id) in [
            ("cr6", "SLOWSEQUENCE", "SYSINFO", 410),
            ("cr1000x", "SLOWSEQUENCE", "SYSINFO", 403),
            ("cr300", "SLOWSEQUENCE", "MQTTPUBLISHCONSTTABLE", 246),
            ("cr350", "SLOWSEQUENCE", "MQTTPUBLISHCONSTTABLE", 247),
        ] {
            let p = get(name).unwrap();
            let names: Vec<&str> = p.instructions().iter().map(|(k, _)| k).collect();
            assert_eq!((names[0], *names.last().unwrap()), (first, last), "{name}");
            let r = p.instructions().get(last).unwrap();
            assert_eq!(r.id, last_id);
            assert_eq!(p.instructions().get(first).unwrap().params, [108]);
            assert!(p.instructions().get("slowsequence").is_none());
            assert_eq!(p.functions().get("NOT").unwrap().kind, 267);
            let w = p.symbols().get("WHILE").unwrap();
            assert_eq!((w.name.as_str(), w.value, w.extra), ("While", 1, 0));
            assert!((w.fvalue - 1.0).abs() < f64::EPSILON);
            assert_eq!(p.symbols().get("OSVERSION").unwrap().value, 0);
        }
        let p = get("cr6").unwrap();
        let i = p.instructions().get("SYSINFO").unwrap();
        assert_eq!(
            (i.index, i.compile_fn.as_str(), i.runtime_fn.as_deref()),
            (409, "0x406654", None)
        );
        let f = p.functions().get("JSONVALUE").unwrap();
        assert_eq!(
            (f.code, f.kind, f.argspec, f.nargs, f.argtypes.as_deref()),
            (104, 309, 209, Some(1), Some(&[1][..]))
        );
        assert_eq!(p.functions().get("NOT").unwrap().nargs, None);
        assert_eq!(p.symbols().get("OSVERSION").unwrap().extra, 6_684_656);
        assert_eq!(
            get("cr300")
                .unwrap()
                .symbols()
                .get("OSVERSION")
                .unwrap()
                .extra,
            6_923_592
        );
        assert_eq!(
            get("cr350")
                .unwrap()
                .functions()
                .get("MQTTUNSUBSCRIBE")
                .unwrap()
                .code,
            101
        );
        assert_eq!(p.ess_variables()[0].name, "essTypeofStation");
        assert_eq!(p.ess_variables()[0].type_, "LONG");
    }

    #[test]
    fn raw_tables() {
        let p = get("cr6").unwrap();
        assert_eq!(
            p.background_calibration()
                .get("threshold_ms")
                .unwrap()
                .as_i64(),
            Some(8)
        );
        assert_eq!(
            p.background_calibration()
                .get("busy_slack")
                .unwrap()
                .as_i64(),
            Some(23)
        );
        assert_eq!(
            get("cr1000x")
                .unwrap()
                .background_calibration()
                .get("threshold_ms")
                .unwrap()
                .as_i64(),
            Some(2)
        );
        assert!(get("cr300").unwrap().background_calibration().is_empty());
        assert_eq!(
            &keys(p.terminals(), "channels")[..2],
            ["u_terminal", "u_terminal_se"]
        );
        assert_eq!(
            &keys(get("cr1000x").unwrap().terminals(), "channels")[..2],
            ["current_chan_cr1000x", "diff_chan"]
        );
        assert_eq!(
            &keys(get("cr300").unwrap().terminals(), "channels")[..2],
            ["diff_chan", "se_chan"]
        );
        assert_eq!(
            &keys(p.pipeline_tasks(), "tasks")[..2],
            ["PANELTEMP", "BATTERY"]
        );
        assert_eq!(
            keys(get("cr300").unwrap().pipeline_tasks(), "tasks"),
            Vec::<String>::new()
        );
        assert_eq!(keys(p.mode_rules(), "nonconstant_arg")[0], "CDM_EXCITEI");
        assert_eq!(
            p.mode_rules()
                .get("nonconstant_arg")
                .unwrap()
                .get("CDM_EXCITEI")
                .unwrap()
                .as_array()
                .unwrap()[0]
                .as_i64(),
            Some(3)
        );
        assert!((p.fnotch().get("min_hz").unwrap().as_f64().unwrap() - 1.430_525).abs() < 1e-12);
        assert!(p.fnotch().get("valid").is_none());
        let valid: Vec<i64> = get("cr300")
            .unwrap()
            .fnotch()
            .get("valid")
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .filter_map(Json::as_i64)
            .collect();
        assert_eq!(valid, [50, 60, 400, 4000]);
        let sw: Vec<i64> = p
            .measure()
            .get("sw12_ports")
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .filter_map(Json::as_i64)
            .collect();
        assert_eq!(sw, [1, 2, 501, 502]);
        assert!(get("cr300").unwrap().measure().get("sw12_ports").is_none());
        let sf = p
            .settings_fields()
            .get("settings")
            .unwrap()
            .as_array()
            .unwrap();
        assert_eq!(
            (sf[0].as_str(), sf[1].as_str()),
            (Some("StationName"), Some("Security"))
        );
        assert_eq!(
            p.settings_fields()
                .get("trim_trailing_blanks")
                .unwrap()
                .as_bool(),
            Some(true)
        );
        assert_eq!(
            get("cr300")
                .unwrap()
                .settings_fields()
                .get("trim_trailing_blanks")
                .unwrap()
                .as_bool(),
            Some(false)
        );
        assert!(p.status_members().contains("BATTERY") && p.status_members().contains("BAUDRATE"));
        assert!(
            p.before_beginprog().contains("ARGOSERROR") && p.before_beginprog().contains("AVGRUN")
        );
        assert!(
            get("cr1000x")
                .unwrap()
                .before_beginprog()
                .contains("ACPOWER")
        );
    }

    #[test]
    fn get_and_for_file() {
        assert_eq!(get("CR1000X").unwrap().name, "cr1000x");
        assert_eq!(
            get("nope").unwrap_err().0,
            "unknown logger 'nope'; choose from cr6, cr1000x, cr300, cr350"
        );
        assert_eq!(
            get("it's").unwrap_err().0,
            "unknown logger \"it's\"; choose from cr6, cr1000x, cr300, cr350"
        );
        for (f, want) in [
            ("a.CR6", "cr6"),
            ("a.CR1X", "cr1000x"),
            ("dir/a.CR300", "cr300"),
            ("a.CR350", "cr350"),
            ("a.cr6", "cr6"),
            ("a.cr350", "cr350"),
            ("a.txt", "cr6"),
            ("a", "cr6"),
            (".cr300", "cr6"),
        ] {
            assert_eq!(for_file(Path::new(f)).name, want, "{f}");
        }
        assert_eq!(
            for_file_or(Path::new("a.txt"), "cr300").unwrap().name,
            "cr300"
        );
        assert!(for_file_or(Path::new("a.txt"), "x").is_err());
    }

    #[test]
    fn dotted_name_message() {
        let m = get("cr300").unwrap().unresolved_symbol_message;
        assert_eq!(m.format("Foo"), "Foo is not defined.");
        assert_eq!(
            m.format_dotted(&DottedName("Foo.Bar".into())),
            "Could not find DataTable Foo.Bar."
        );
        let m = get("cr6").unwrap().unresolved_symbol_message;
        assert_eq!(m.format("Foo"), "Invalid symbol in expression: Foo.");
        assert_eq!(
            m.format_dotted(&DottedName("Foo.Bar".into())),
            "Invalid symbol in expression: Foo.Bar."
        );
        assert_eq!(format_name("{name} x", "A"), "A x");
    }

    fn files(dir: &Path, out: &mut Vec<PathBuf>) {
        for e in std::fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                files(&p, out);
            } else {
                out.push(p);
            }
        }
    }

    #[test]
    fn data_is_identical_to_python_copy() {
        let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let mut roots: Vec<PathBuf> = std::env::var_os("OPENCRBASIC_PY")
            .map(PathBuf::from)
            .into_iter()
            .collect();
        roots.push(manifest.join("../open-crbasic"));
        let Some(py) = roots
            .iter()
            .map(|r| r.join("src/opencrbasic/data"))
            .find(|d| d.is_dir())
        else {
            eprintln!("skipped: no Python checkout (set OPENCRBASIC_PY)");
            return;
        };
        let mut mine = Vec::new();
        files(&manifest.join("data"), &mut mine);
        assert_eq!(mine.len(), 50);
        for f in mine {
            let rel = f.strip_prefix(manifest.join("data")).unwrap();
            assert_eq!(
                std::fs::read(&f).unwrap(),
                std::fs::read(py.join(rel)).unwrap(),
                "{}",
                rel.display()
            );
        }
    }
}
