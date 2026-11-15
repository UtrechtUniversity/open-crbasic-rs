//! Port of open-crbasic `diagnostics`: diagnostics in CR6Comp.exe's format.
//!
//! `CR6Comp` prints errors as `line N: <message>` and warnings as `Warning: <message>`. An error
//! whose statement came from an included file is printed `<file> line N: <message>` unless its
//! kind is one the real compiler emits after the file was closed (`deferred_kinds`) or
//! `Diagnostic::show_file` says otherwise.
//!
//! The Python `ContextVar`s `_ACTIVE_PHASE` / `_ACTIVE_POSITIONS` are thread-locals with RAII
//! guards (`PhaseGuard`, `PositionsGuard`) that restore the previous value on drop.
//!
//! Diagnostics are immutable and shared as `Rc<Diagnostic>` (`DiagRef`): Python's `id(d)` and
//! `d is m` become `Rc::as_ptr` / `Rc::ptr_eq`, and `dataclasses.replace(d, ...)` becomes
//! `Diagnostic::replace` (which re-runs `post_init`, like Python's `__post_init__`).

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::LazyLock;

use fancy_regex::Regex;

use crate::syntax::LoadProvenance;

/// `(file, line) -> expanded-source statement index` (Python `dict[tuple[str, int], int]`).
pub type Positions = HashMap<(String, i64), i64>;

pub const INVALID_PORT_KINDS: [&str; 2] = ["invalid_port", "invalid_port_comma"];
pub const TABLE_NAME_KINDS: [&str; 3] = [
    "table_name_too_long",
    "table_name_number",
    "table_name_illegal",
];
pub const PLACEMENT_MENU_KINDS: [&str; 2] = ["placement_menu", "placement_menu_recompile"];
pub const COULD_NOT_FIND_KINDS: [&str; 3] = [
    "could_not_find",
    "could_not_find_table",
    "cannot_find_table",
];

/// Message families matched by prefix/regex -> canonical `Diagnostic.kind`. Order matters where a
/// narrower rule precedes a broader one.
pub const DEFERRED_COMMON_SPECS: &[(&str, &str)] = &[
    (
        "before_beginprog",
        r".+ illegally occurs before BeginProg\.$",
    ),
    ("could_not_find_table", r"Could not find DataTable "),
    ("could_not_find", r"Could not find "),
    ("cannot_find_table", r"Cannot find DataTable"),
    (
        "cannot_autoallocate_table",
        r"Cannot autoallocate memory for DataTable",
    ),
    ("duplicate_sub", r"Duplicate Sub "),
    ("endtable_without_datatable", r"EndTable without DataTable"),
    ("empty_table", r"Table \S+ declared with no output fields"),
    (
        "table_interval_uncalled",
        r"Table with DataInterval must be called",
    ),
    (
        "sample_outside_table",
        r"Sample must be used inside a table",
    ),
    ("illegal_matrix", r"Illegal Matrix"),
    ("endif_without_if", r"END IF without IF"),
    ("else_without_if", r"Else with no If"),
    ("loop_without_do", r"LOOP without DO"),
    ("endsub_without_sub", r"EndSub without Sub"),
    ("next_without_for", r"NEXT without FOR"),
    ("nextscan_without_scan", r"NextScan with no Scan"),
    ("nested_scan", r"Scan without NextScan or nested Scans\.$"),
    ("scan_without_nextscan", r"Scan without NextScan"),
    ("missing_nextscan", r"Missing NextScan"),
    ("for_next_illegal", r"FOR NEXT statement illegally has"),
    ("if_select_illegal", r"IF or SELECT statement illegally has"),
    ("loop_illegal", r"LOOP statement illegally has"),
    ("out_of_bounds", r"Variable .* out of bounds"),
    ("file_manage", r"FileManage "),
    ("out_of_memory", r"Out of memory\."),
    ("illegal_semaphore", r"Illegal Semaphore"),
];

pub const DEFERRED_CR6_SPECS: &[(&str, &str)] = &[
    ("case_without_select", r"Case with no Select"),
    (
        "next_subscan_without_subscan",
        r"Next SubScan without SubScan",
    ),
    ("delay_outside_scan", r"Delay not allowed outside"),
    ("scan_invalid", r"Scan \S+ invalid"),
    ("scan_interval_short", r"Scan interval too short"),
    (
        "scan_param",
        r"Scan (Interval|BufferOption|Count) parameter",
    ),
    ("slow_scan_interval", r"Slow Scan interval"),
    ("placement_menu_recompile", r"MenuRecompile must be used"),
    ("menu_pick", r"MenuPick must"),
    ("endmenu_must_be_used", r"EndMenu must be used"),
    ("invalid_channel", r"Invalid channel "),
    ("invalid_port", r"Invalid port "),
    ("invalid_reps", r"Invalid reps "),
    ("invalid_sdm_address", r"Invalid SDM Address"),
    ("illegal_table_name", r"Illegal table name"),
    ("table_name_number", r"Table name .* illegal"),
    ("table_name_too_long", r"DataTable name .* too long"),
    ("dnp_variable", r"DNP Variable must be"),
    ("cant_assign_units", r"Can't assign units to"),
    ("already_assigned_units", r"\S+ already assigned Units"),
    ("paneltemp_channel", r"PanelTemp channel out of range"),
    ("input_channel", r"Input channel"),
    ("reps_cycle_input", r"Reps cycle input"),
    ("spiopen_precede", r"SPIOpen must precede"),
    ("illegal_pakbus_address", r"Illegal PakBus address"),
    ("insufficient", r"Insufficient "),
    ("cpi_address", r"CPI Address must"),
    ("bridge_param", r"Bridge parameter must"),
    ("optional_count_param", r"Optional Count parameter"),
    ("source_dest_types", r"Source and Dest types"),
    ("unable_to_find_dest", r"Unable to find Dest"),
    ("multiply_result_illegal", r"multiply result illegal"),
    ("destination_var_must", r"Destination variable must"),
    ("legal_configuration", r"Legal configuration is"),
    ("sig_length", r"Sig\d+ must be of length"),
    ("number_of_bytes", r"Number of bytes "),
    ("number_of_averages", r"number of averages "),
    (
        "samplefieldcal_without_fieldcal",
        r"SampleFieldCal with no FieldCal",
    ),
    (
        "cdm_pulseport_unsupported",
        r"CDM_PulsePort instruction not supported",
    ),
    ("ch_status_interval", r"CH\d+Status update interval"),
    ("not_settings_field", r".+ is not a Settings Field Name"),
    ("goestable_not_active", r"GoesTable not active"),
    ("missing_dialsequence", r"Missing DialSequence"),
    ("boolean_picklist", r"Boolean picklist must"),
    (
        "must_be_int_const",
        r"\S+ must be declared as a integer Const",
    ),
    (
        "string_source_dest_same",
        r"If source and destination are of type STRING",
    ),
    ("datainterval_non_negative", r"DataInterval TintoInt"),
    ("multiple_beginprogs", r"Multiple BeginProgs"),
    ("sub_endsub_unmatched", r"Sub .* EndSub not matched"),
    ("if_select_without_endif", r"IF or SELECT without EndIf"),
];

pub const DEFERRED_CR3XX_SPECS: &[(&str, &str)] = &[
    ("for_without_next", r"FOR without NEXT"),
    ("scans_gt", r"Scans > "),
    ("dnp_statics", r"DNP\d+Statics and/or"),
    ("destination_var_cannot", r"Destination variable cannot"),
    ("illegal_option", r"Illegal option"),
    ("illegal_dimension", r"Illegal dimension"),
    ("illegal_message_type", r"Illegal message type"),
    ("only_one_datainterval", r"Only one DataInterval allowed"),
    ("settling_time", r"Settling time "),
    ("must_be_length", r"\S+ must be of length"),
    ("invalid_drive", r"Invalid drive or no drive"),
    ("invalid_file_type", r"Invalid file type option"),
    (
        "not_status_settings_field",
        r"\S+ is not a Status or Settings Field Name",
    ),
];

pub const OTHER_KIND_SPECS: &[(&str, &str)] = &[
    ("table_name_illegal", r"DataTable name .* is illegal\.$"),
    ("invalid_port_comma", r"Invalid port"),
    ("wind_subinterval", r"WindVector: non-zero SubInterval"),
    ("invalid_symbol", r"Invalid symbol in expression"),
    ("invalid_expression", r"Invalid, or out of place expression"),
    ("dimensions_not_declared", r"Dimensions not declared"),
    ("sdi12_conflict", r"SDI12"),
    ("fnotch", r"Invalid fnotch value"),
    ("cannot_open_include", r"Cannot open include file "),
    (
        "truncate_warning",
        r"Parameter will truncate to the nearest integer\.$",
    ),
    (
        "placement_menu",
        r".+ must be used within DisplayMenu\.\.EndMenu\.$",
    ),
    (
        "placement_scan",
        r".+ must be used within a Scan\.\.\.NextScan\.$",
    ),
    ("unreached", r".+ never reached at run time\.$"),
    ("unused_variable", r".+ declared but not used\.$"),
    ("not_defined", r".+ is not defined\.$"),
];

/// Compiled `_KIND_RE`: one anchored regex per spec, tried in order. Python joins the specs into
/// one alternation matched at the start; the first alternative that matches wins, which is the
/// same thing. A trailing `$` is Python's (before an optional final newline), translated.
// TODO(P1): switch to pyre
static KIND_REGEXES: LazyLock<Vec<(&'static str, Regex)>> = LazyLock::new(|| {
    [
        DEFERRED_COMMON_SPECS,
        DEFERRED_CR6_SPECS,
        DEFERRED_CR3XX_SPECS,
        OTHER_KIND_SPECS,
    ]
    .into_iter()
    .flatten()
    .map(|&(name, pattern)| {
        let body = pattern
            .strip_suffix('$')
            .map_or_else(|| pattern.to_string(), |p| format!("{p}(?=\\n?\\z)"));
        (
            name,
            Regex::new(&format!("^(?:{body})")).expect("kind pattern compiles"),
        )
    })
    .collect()
});

static DEFERRED_KINDS: LazyLock<[HashSet<&'static str>; 2]> = LazyLock::new(|| {
    let names =
        |specs: &[(&'static str, &str)]| specs.iter().map(|&(n, _)| n).collect::<HashSet<_>>();
    let common = names(DEFERRED_COMMON_SPECS);
    let mut cr6 = common.clone();
    cr6.extend(names(DEFERRED_CR6_SPECS));
    let mut cr3xx = common;
    cr3xx.extend(names(DEFERRED_CR3XX_SPECS));
    cr3xx.extend(["sig_length", "table_name_too_long"]);
    [cr6, cr3xx]
});

/// Python `_DEFERRED_KINDS[drive_prefix]`: kinds the real compiler emits after the included file
/// was closed (no `<file>` prefix).
pub fn deferred_kinds(drive_prefix: bool) -> &'static HashSet<&'static str> {
    &DEFERRED_KINDS[usize::from(drive_prefix)]
}

/// Python `infer_kind`: the canonical `Diagnostic.kind` of a rendered message ("" when none).
pub fn infer_kind(severity: &str, message: &str) -> String {
    if severity == "warning" {
        if message.ends_with(" never reached at run time.") {
            return "unreached".to_string();
        }
        if message.starts_with("Invalid fnotch value") {
            return "fnotch".to_string();
        }
        if message.starts_with("Cannot open include file ") {
            return "cannot_open_include".to_string();
        }
        if message == "Parameter will truncate to the nearest integer." {
            return "truncate_warning".to_string();
        }
        if message.ends_with(" declared but not used.") {
            return "unused_variable".to_string();
        }
    }
    KIND_REGEXES
        .iter()
        .find(|(_, re)| re.is_match(message).unwrap_or(false))
        .map_or_else(String::new, |(name, _)| (*name).to_string())
}

/// Python `_DRIVE_RE.match(s)`: `^(CPU|USR|CRD|USB):` ignoring case; returns the rest of `s`.
// TODO(P1): switch to pyre
fn strip_drive(s: &str) -> Option<&str> {
    let mut chars = s.char_indices();
    let mut head = String::new();
    for _ in 0..3 {
        head.push(chars.next()?.1);
    }
    let (colon_at, colon) = chars.next()?;
    let head = head.to_ascii_uppercase();
    (colon == ':' && matches!(head.as_str(), "CPU" | "USR" | "CRD" | "USB"))
        .then(|| &s[colon_at + 1..])
}

// ---------------------------------------------------------------- ambient state

thread_local! {
    static ACTIVE_POSITIONS: RefCell<Option<Rc<Positions>>> = const { RefCell::new(None) };
    static ACTIVE_PHASE: RefCell<String> = const { RefCell::new(String::new()) };
}

/// The current value of Python's `_ACTIVE_PHASE` (`""` when unset).
pub fn active_phase() -> String {
    ACTIVE_PHASE.with(|p| p.borrow().clone())
}

/// The current value of Python's `_ACTIVE_POSITIONS`.
pub fn active_positions() -> Option<Rc<Positions>> {
    ACTIVE_POSITIONS.with(|p| p.borrow().clone())
}

/// Sets `_ACTIVE_PHASE` until dropped, then restores the previous value (`token = var.set(..)` /
/// `var.reset(token)`).
#[must_use = "the phase is restored when the guard drops"]
pub struct PhaseGuard {
    previous: String,
}

impl PhaseGuard {
    pub fn new(phase: &str) -> Self {
        let previous =
            ACTIVE_PHASE.with(|p| std::mem::replace(&mut *p.borrow_mut(), phase.to_string()));
        Self { previous }
    }
}

impl Drop for PhaseGuard {
    fn drop(&mut self) {
        let previous = std::mem::take(&mut self.previous);
        ACTIVE_PHASE.with(|p| *p.borrow_mut() = previous);
    }
}

/// Sets `_ACTIVE_POSITIONS` until dropped, then restores the previous value.
#[must_use = "the positions are restored when the guard drops"]
pub struct PositionsGuard {
    previous: Option<Rc<Positions>>,
}

impl PositionsGuard {
    pub fn new(positions: Option<Rc<Positions>>) -> Self {
        let previous =
            ACTIVE_POSITIONS.with(|p| std::mem::replace(&mut *p.borrow_mut(), positions));
        Self { previous }
    }
}

impl Drop for PositionsGuard {
    fn drop(&mut self) {
        let previous = self.previous.take();
        ACTIVE_POSITIONS.with(|p| *p.borrow_mut() = previous);
    }
}

// ---------------------------------------------------------------- Diagnostic

/// Python `Diagnostic.show_file: bool | str`: whether an origin outside `main_file` gets a
/// `<file>` prefix. `Bool(true)` always, `Bool(false)` never, `Cr3xx` (`"cr3xx"`) only on
/// CR300/CR350.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShowFile {
    Bool(bool),
    Cr3xx,
}

/// Python `Diagnostic` (frozen dataclass). `stmt`, `kind`, `phase` and `pos` are `compare=False`:
/// the hand-written `PartialEq` ignores them.
#[derive(Debug, Clone)]
pub struct Diagnostic {
    /// `"error"` or `"warning"`.
    pub severity: String,
    /// 0 when the message has no line (e.g. `No BeginProg in Program.`).
    pub line: i64,
    /// Without the `line N: ` / `Warning: ` prefix.
    pub message: String,
    /// Originating file.
    pub file: String,
    pub show_file: ShowFile,
    /// `id()` of the statement that raised it, when tagged (0 otherwise; see `syntax::rc_id`).
    pub stmt: usize,
    /// Canonical message-family identifier.
    pub kind: String,
    /// Checker stage or aside bucket that emitted it.
    pub phase: String,
    /// Expanded-source statement index (-1 when unmapped).
    pub pos: i64,
}

impl PartialEq for Diagnostic {
    fn eq(&self, other: &Self) -> bool {
        self.severity == other.severity
            && self.line == other.line
            && self.message == other.message
            && self.file == other.file
            && self.show_file == other.show_file
    }
}

impl Eq for Diagnostic {}

/// A shared, immutable diagnostic (Python object identity).
pub type DiagRef = Rc<Diagnostic>;

impl Diagnostic {
    /// Python `Diagnostic(severity, line, message, file, show_file, stmt=, kind=, phase=)`,
    /// `__post_init__` included (`pos` starts at -1).
    pub fn new(
        severity: &str,
        line: i64,
        message: &str,
        file: &str,
        show_file: ShowFile,
        stmt: usize,
        kind: &str,
        phase: &str,
    ) -> Self {
        let mut d = Self {
            severity: severity.to_string(),
            line,
            message: message.to_string(),
            file: file.to_string(),
            show_file,
            stmt,
            kind: kind.to_string(),
            phase: phase.to_string(),
            pos: -1,
        };
        d.post_init();
        d
    }

    /// Python `Diagnostic(severity, line, message, file)` with every other field at its default.
    pub fn basic(severity: &str, line: i64, message: &str, file: &str) -> Self {
        Self::new(
            severity,
            line,
            message,
            file,
            ShowFile::Bool(true),
            0,
            "",
            "",
        )
    }

    /// Python `__post_init__`: infer a missing `kind`, take the ambient phase when none was
    /// given, and map `(file, line)` to its position when positions are active.
    pub fn post_init(&mut self) {
        if self.kind.is_empty() {
            self.kind = infer_kind(&self.severity, &self.message);
        }
        if self.phase.is_empty() {
            let phase = active_phase();
            if !phase.is_empty() {
                self.phase = phase;
            }
        }
        if let Some(positions) = active_positions() {
            self.pos = positions
                .get(&(self.file.clone(), self.line))
                .copied()
                .unwrap_or(-1);
        }
    }

    /// Python `dataclasses.replace(d, ...)`: a copy edited by `change`, with `__post_init__` run
    /// again (so an empty `kind` is re-inferred and ambient positions re-applied).
    #[must_use]
    pub fn replace(&self, change: impl FnOnce(&mut Diagnostic)) -> Diagnostic {
        let mut d = self.clone();
        change(&mut d);
        d.post_init();
        d
    }

    /// Python `Diagnostic.cr6comp`.
    pub fn cr6comp(&self, main_file: &str, drive_prefix: bool) -> String {
        if self.severity == "warning" {
            if self.line != 0 {
                return format!("line {}: Warning: {}", self.line, self.message);
            }
            return format!("Warning: {}", self.message);
        }
        if self.line == 0 {
            return self.message.clone();
        }
        let mut prefix = String::new();
        let drive_less = strip_drive(&self.file);
        let mut name = drive_less.unwrap_or(&self.file).to_string();
        let mut show = match self.show_file {
            ShowFile::Cr3xx => drive_prefix,
            ShowFile::Bool(b) => b,
        };
        if show && deferred_kinds(drive_prefix).contains(self.kind.as_str()) {
            show = false;
        }
        // TODO(P1): `to_lowercase` -> pystr (Python `str.lower`)
        if show
            && !name.is_empty()
            && (name.to_lowercase() != main_file.to_lowercase() || drive_less.is_some())
        {
            if drive_prefix {
                name = if drive_less.is_some() {
                    self.file.clone()
                } else {
                    format!("cpu:{}", self.file)
                };
            }
            prefix = format!("<{name}> ");
        }
        format!("{prefix}line {}: {}", self.line, self.message)
    }
}

/// Python `Diagnostics(LoadProvenance)`.
#[derive(Debug, Clone, Default)]
pub struct Diagnostics {
    pub items: Vec<DiagRef>,
    /// Findings gated/merged by the checker resolve step.
    pub asides: Vec<DiagRef>,
    /// Python `LoadProvenance` base class (set by `source.py`).
    pub provenance: LoadProvenance,
}

impl Diagnostics {
    pub fn new() -> Self {
        Self::default()
    }

    /// Python `Diagnostics.error(line, message, file, show_file, stmt, kind=, phase=)`; `stmt` is
    /// the `id()` of the tagged object (`syntax::rc_id`), 0 for none.
    pub fn error(
        &mut self,
        line: i64,
        message: &str,
        file: &str,
        show_file: ShowFile,
        stmt: usize,
        kind: &str,
        phase: &str,
    ) {
        self.items.push(Rc::new(Diagnostic::new(
            "error", line, message, file, show_file, stmt, kind, phase,
        )));
    }

    /// `diags.error(line, message, file)` with every other argument defaulted.
    pub fn error_basic(&mut self, line: i64, message: &str, file: &str) {
        self.error(line, message, file, ShowFile::Bool(true), 0, "", "");
    }

    /// Python `Diagnostics.warning(line, message, file, kind=, phase=)`.
    pub fn warning(&mut self, line: i64, message: &str, file: &str, kind: &str, phase: &str) {
        self.items.push(Rc::new(Diagnostic::new(
            "warning",
            line,
            message,
            file,
            ShowFile::Bool(true),
            0,
            kind,
            phase,
        )));
    }

    /// `diags.warning(line, message, file)` with every other argument defaulted.
    pub fn warning_basic(&mut self, line: i64, message: &str, file: &str) {
        self.warning(line, message, file, "", "");
    }

    /// Python `Diagnostics.trim`: drop trailing diagnostics at or past `cut`.
    pub fn trim(&mut self, cut: Option<usize>) {
        if let Some(cut) = cut {
            self.items.truncate(cut);
        }
    }

    pub fn errors(&self) -> Vec<DiagRef> {
        self.items
            .iter()
            .filter(|d| d.severity == "error")
            .cloned()
            .collect()
    }

    pub fn warnings(&self) -> Vec<DiagRef> {
        self.items
            .iter()
            .filter(|d| d.severity == "warning")
            .cloned()
            .collect()
    }

    /// Python `Diagnostics.__bool__`: true when there is an error.
    pub fn has_errors(&self) -> bool {
        self.items.iter().any(|d| d.severity == "error")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Expected values printed by running Python's diagnostics.py on real fixture messages.
    const KINDS: &[(&str, &str, &str)] = &[
        (
            "error",
            "Invalid channel 1, must use U1..U12.",
            "invalid_channel",
        ),
        ("error", "Nope is not defined.", "not_defined"),
        ("error", "Variable illegal in Parameter: Nope.", ""),
        (
            "error",
            "Invalid, or out of place expression: = lat.",
            "invalid_expression",
        ),
        (
            "error",
            "Can't assign units to Sin, it must be declared Public or referenced as a source in a Data Table.",
            "cant_assign_units",
        ),
        (
            "error",
            "Invalid symbol in expression: Nope1.",
            "invalid_symbol",
        ),
        (
            "error",
            "Battery illegally occurs before BeginProg.",
            "before_beginprog",
        ),
        ("error", "NEXT without FOR.", "next_without_for"),
        (
            "error",
            "END IF without IF or END SELECT without SELECT.",
            "endif_without_if",
        ),
        ("error", "LOOP without DO.", "loop_without_do"),
        (
            "error",
            "EndSub without Sub or nested subs.",
            "endsub_without_sub",
        ),
        (
            "error",
            "IF or SELECT statement illegally has SCAN without NEXT SCAN.",
            "if_select_illegal",
        ),
        (
            "error",
            "FOR NEXT statement illegally has SCAN without NEXT SCAN.",
            "for_next_illegal",
        ),
        (
            "error",
            "LOOP statement illegally has SCAN without NEXT SCAN.",
            "loop_illegal",
        ),
        (
            "error",
            "Delay not allowed outside of Scan..NextScan in PipeLine Mode.",
            "delay_outside_scan",
        ),
        ("warning", "Expr never reached at run time.", "unreached"),
        ("error", "NextScan with no Scan.", "nextscan_without_scan"),
        (
            "error",
            "Invalid port 1, use C1..C4 or U1..U12.",
            "invalid_port",
        ),
        (
            "error",
            "Boolean picklist must be True or False.",
            "boolean_picklist",
        ),
        (
            "error",
            "MenuPick must follow a MenuItem instruction.",
            "menu_pick",
        ),
        ("error", "Variable S out of bounds.", "out_of_bounds"),
        (
            "error",
            "IF or SELECT without EndIf or EndSelect.",
            "if_select_without_endif",
        ),
        (
            "error",
            "Could not find DataTable Hourly.",
            "could_not_find_table",
        ),
        (
            "error",
            "CDM_PulsePort instruction not supported by module NULL.",
            "cdm_pulseport_unsupported",
        ),
        ("warning", "Open detection disabled.", ""),
        ("error", "CPI Address must be 1..120.", "cpi_address"),
        (
            "error",
            "Input channel: 25, out of range (1 - 20).",
            "input_channel",
        ),
        (
            "error",
            "CH400Status update interval must be an integer multiple of 1 second.",
            "ch_status_interval",
        ),
        ("error", "Else with no If.", "else_without_if"),
        ("error", "Case with no Select.", "case_without_select"),
        (
            "error",
            "number of averages 0 and reps 1 in RunningAvg must be > 0.",
            "number_of_averages",
        ),
        (
            "error",
            "Illegal Semaphore: 1..4 are legal, 1 to 3 are general purpose, 4 is reserved (see docs)",
            "illegal_semaphore",
        ),
        ("error", "Illegal message type 1", "illegal_message_type"),
        (
            "error",
            "Illegal PakBus address 7, range 1..4095",
            "illegal_pakbus_address",
        ),
        (
            "error",
            "Legal configuration is 1..3.",
            "legal_configuration",
        ),
        (
            "warning",
            "Invalid fnotch value, must be 50,60 400 or 4000",
            "fnotch",
        ),
        ("error", "Could not find times.", "could_not_find"),
        (
            "error",
            "Insufficient time (1 ms) for measurements from CPI address 1 (needs at least 8 ms).",
            "insufficient",
        ),
        (
            "error",
            "Table name 15Min illegal, starts with a number.",
            "table_name_number",
        ),
        (
            "error",
            "Dimensions not declared: Nope4=Nope(1)",
            "dimensions_not_declared",
        ),
        ("error", "x must be of length >= 32.", "must_be_length"),
        ("error", "FileManage invalid drive: SD:a.", "file_manage"),
        (
            "warning",
            "Cannot open include file  NoSuch.crb.",
            "cannot_open_include",
        ),
        ("error", "SPIOpen must precede SPIWrite.", "spiopen_precede"),
        (
            "error",
            "DNP3Statics and/or DNP3Events are not defined",
            "dnp_statics",
        ),
        (
            "error",
            "Bridge parameter must be an Array(2,3) of type long.",
            "bridge_param",
        ),
        ("error", "Illegal option 2", "illegal_option"),
        ("warning", "X declared but not used.", "unused_variable"),
        ("error", "Illegal Matrix dimensioned", "illegal_matrix"),
        (
            "error",
            "multiply result illegal dimensioned, must be 2,2",
            "multiply_result_illegal",
        ),
        (
            "error",
            "2.5 must be declared as a integer Const.",
            "must_be_int_const",
        ),
        (
            "error",
            "Scan without NextScan or nested Scans.",
            "nested_scan",
        ),
        (
            "error",
            "Scan 0uSec invalid must be at least 1mSec.",
            "scan_invalid",
        ),
        (
            "error",
            "Sample must be used inside a table declaration.",
            "sample_outside_table",
        ),
        ("error", "Invalid port.", "invalid_port_comma"),
        ("error", "Multiple BeginProgs.", "multiple_beginprogs"),
        (
            "error",
            "Cannot autoallocate memory for DataTable T called outside of Scan, e.g., called by Sub.",
            "cannot_autoallocate_table",
        ),
        (
            "error",
            "WindVector: non-zero SubInterval only valid for output modes 0, 2, and 4.",
            "wind_subinterval",
        ),
        ("error", "Out of memory.", "out_of_memory"),
        (
            "error",
            "Table T declared with no output fields.",
            "empty_table",
        ),
        ("error", "Invalid reps 0, must be > 0.", "invalid_reps"),
        (
            "error",
            "DataTable name 1.5 is illegal.",
            "table_name_illegal",
        ),
        ("error", "Duplicate Sub A", "duplicate_sub"),
        (
            "error",
            "Scan Interval parameter must be a non-negative integer",
            "scan_param",
        ),
        (
            "error",
            "Scans > 10mSec must be a multiple of 10mSec.",
            "scans_gt",
        ),
        (
            "error",
            "EndTable without DataTable.",
            "endtable_without_datatable",
        ),
        (
            "error",
            "Scan interval too short for measurement by 499 Sec.",
            "scan_interval_short",
        ),
        (
            "error",
            "DataInterval TintoInt and Interval parameters must be non-negative",
            "datainterval_non_negative",
        ),
        (
            "error",
            "Only one DataInterval allowed within DataTable..EndTable.",
            "only_one_datainterval",
        ),
        (
            "error",
            "Destination variable must be type LONG",
            "destination_var_must",
        ),
        (
            "error",
            "Optional Count parameter must be type long.",
            "optional_count_param",
        ),
        (
            "error",
            "TimerInput must be used within a Scan...NextScan.",
            "placement_scan",
        ),
        (
            "error",
            "MenuRecompile must be used within DisplayMenu..EndMenu.",
            "placement_menu_recompile",
        ),
        (
            "error",
            "MenuItem must be used within DisplayMenu..EndMenu.",
            "placement_menu",
        ),
        ("error", "Missing NextScan.", "missing_nextscan"),
        ("error", "Cannot find DataTable Nope.", "cannot_find_table"),
        (
            "warning",
            "Parameter will truncate to the nearest integer.",
            "truncate_warning",
        ),
        (
            "error",
            "Reps cycle input beyond last channel!",
            "reps_cycle_input",
        ),
        (
            "error",
            "Sub ... EndSub not matched.",
            "sub_endsub_unmatched",
        ),
        ("error", "FOR without NEXT.", "for_without_next"),
        (
            "error",
            "Destination variable cannot be indexed",
            "destination_var_cannot",
        ),
        (
            "error",
            "Next SubScan without SubScan.",
            "next_subscan_without_subscan",
        ),
        ("error", "GoesTable not active.", "goestable_not_active"),
        (
            "error",
            "DataTable name Abcdefghijklmnopqrstu is 1 chars too long.",
            "table_name_too_long",
        ),
        ("error", "Missing DialSequence().", "missing_dialsequence"),
        ("error", "Scan without NextScan.", "scan_without_nextscan"),
        (
            "error",
            "Settling time 1 too short, min time is 20.",
            "settling_time",
        ),
        (
            "error",
            "Number of bytes 100 must be less than input buffer 0",
            "number_of_bytes",
        ),
        (
            "error",
            "SDI12Recorder: channel C3 already in use or unavailable.",
            "sdi12_conflict",
        ),
        ("error", "Illegal dimension 0", "illegal_dimension"),
        (
            "error",
            "Source and Dest types must be the same.",
            "source_dest_types",
        ),
        (
            "error",
            "Invalid SDM Address: 16, must be 0 - 14.",
            "invalid_sdm_address",
        ),
        (
            "error",
            "PanelTemp channel out of range!",
            "paneltemp_channel",
        ),
        (
            "error",
            "Slow Scan interval must be >= 100mSec.",
            "slow_scan_interval",
        ),
        ("error", "Invalid file type option.", "invalid_file_type"),
        (
            "error",
            "Invalid drive or no drive specified in filename CPU:x.",
            "invalid_drive",
        ),
        (
            "error",
            "SampleFieldCal with no FieldCal in program.",
            "samplefieldcal_without_fieldcal",
        ),
        (
            "error",
            "Table with DataInterval must be called within Scan..NextScan.",
            "table_interval_uncalled",
        ),
        (
            "error",
            "PakBusAddress   is not a Settings Field Name.",
            "not_settings_field",
        ),
        (
            "error",
            "Foo is not a Status or Settings Field Name.",
            "not_status_settings_field",
        ),
        (
            "error",
            "If source and destination are of type STRING, bytes per value must be 1",
            "string_source_dest_same",
        ),
    ];

    // (severity, line, message, file, show_file, main_file, drive_prefix, cr6comp)
    #[allow(clippy::type_complexity)]
    const CR6COMP: &[(&str, i64, &str, &str, &str, &str, bool, &str)] = &[
        (
            "error",
            4,
            "Nope is not defined.",
            "main.cr6",
            "true",
            "main.cr6",
            false,
            "line 4: Nope is not defined.",
        ),
        (
            "error",
            4,
            "Nope is not defined.",
            "lib.crb",
            "true",
            "main.cr6",
            false,
            "<lib.crb> line 4: Nope is not defined.",
        ),
        (
            "error",
            4,
            "Nope is not defined.",
            "MAIN.CR6",
            "true",
            "main.cr6",
            false,
            "line 4: Nope is not defined.",
        ),
        (
            "error",
            4,
            "Nope is not defined.",
            "CPU:lib.crb",
            "true",
            "main.cr6",
            false,
            "<lib.crb> line 4: Nope is not defined.",
        ),
        (
            "error",
            4,
            "Nope is not defined.",
            "cpu:main.cr6",
            "true",
            "main.cr6",
            false,
            "<main.cr6> line 4: Nope is not defined.",
        ),
        (
            "error",
            4,
            "Nope is not defined.",
            "lib.crb",
            "cr3xx",
            "main.cr6",
            false,
            "line 4: Nope is not defined.",
        ),
        (
            "error",
            4,
            "Nope is not defined.",
            "lib.crb",
            "false",
            "main.cr6",
            false,
            "line 4: Nope is not defined.",
        ),
        (
            "error",
            4,
            "Could not find DataTable Hourly.",
            "lib.crb",
            "true",
            "main.cr6",
            false,
            "line 4: Could not find DataTable Hourly.",
        ),
        (
            "error",
            4,
            "Table name 15Min illegal, starts with a number.",
            "lib.crb",
            "true",
            "main.cr6",
            false,
            "line 4: Table name 15Min illegal, starts with a number.",
        ),
        (
            "error",
            4,
            "FOR without NEXT.",
            "lib.crb",
            "true",
            "main.cr6",
            false,
            "<lib.crb> line 4: FOR without NEXT.",
        ),
        (
            "error",
            4,
            "DataTable name Abcdefghijklmnopqrstu is 1 chars too long.",
            "lib.crb",
            "true",
            "main.cr6",
            false,
            "line 4: DataTable name Abcdefghijklmnopqrstu is 1 chars too long.",
        ),
        (
            "error",
            4,
            "Sig5 must be of length 4.",
            "lib.crb",
            "true",
            "main.cr6",
            false,
            "line 4: Sig5 must be of length 4.",
        ),
        (
            "error",
            0,
            "No BeginProg in Program.",
            "lib.crb",
            "true",
            "main.cr6",
            false,
            "No BeginProg in Program.",
        ),
        (
            "warning",
            6,
            "Delay never reached at run time.",
            "lib.crb",
            "true",
            "main.cr6",
            false,
            "line 6: Warning: Delay never reached at run time.",
        ),
        (
            "warning",
            0,
            "Table Foo, is declared but never called.",
            "",
            "true",
            "main.cr6",
            false,
            "Warning: Table Foo, is declared but never called.",
        ),
        (
            "error",
            2,
            "Nope is not defined.",
            "",
            "true",
            "main.cr6",
            false,
            "line 2: Nope is not defined.",
        ),
        (
            "error",
            4,
            "Nope is not defined.",
            "main.cr6",
            "true",
            "main.cr6",
            true,
            "line 4: Nope is not defined.",
        ),
        (
            "error",
            4,
            "Nope is not defined.",
            "lib.crb",
            "true",
            "main.cr6",
            true,
            "<cpu:lib.crb> line 4: Nope is not defined.",
        ),
        (
            "error",
            4,
            "Nope is not defined.",
            "MAIN.CR6",
            "true",
            "main.cr6",
            true,
            "line 4: Nope is not defined.",
        ),
        (
            "error",
            4,
            "Nope is not defined.",
            "CPU:lib.crb",
            "true",
            "main.cr6",
            true,
            "<CPU:lib.crb> line 4: Nope is not defined.",
        ),
        (
            "error",
            4,
            "Nope is not defined.",
            "cpu:main.cr6",
            "true",
            "main.cr6",
            true,
            "<cpu:main.cr6> line 4: Nope is not defined.",
        ),
        (
            "error",
            4,
            "Nope is not defined.",
            "lib.crb",
            "cr3xx",
            "main.cr6",
            true,
            "<cpu:lib.crb> line 4: Nope is not defined.",
        ),
        (
            "error",
            4,
            "Nope is not defined.",
            "lib.crb",
            "false",
            "main.cr6",
            true,
            "line 4: Nope is not defined.",
        ),
        (
            "error",
            4,
            "Could not find DataTable Hourly.",
            "lib.crb",
            "true",
            "main.cr6",
            true,
            "line 4: Could not find DataTable Hourly.",
        ),
        (
            "error",
            4,
            "Table name 15Min illegal, starts with a number.",
            "lib.crb",
            "true",
            "main.cr6",
            true,
            "<cpu:lib.crb> line 4: Table name 15Min illegal, starts with a number.",
        ),
        (
            "error",
            4,
            "FOR without NEXT.",
            "lib.crb",
            "true",
            "main.cr6",
            true,
            "line 4: FOR without NEXT.",
        ),
        (
            "error",
            4,
            "DataTable name Abcdefghijklmnopqrstu is 1 chars too long.",
            "lib.crb",
            "true",
            "main.cr6",
            true,
            "line 4: DataTable name Abcdefghijklmnopqrstu is 1 chars too long.",
        ),
        (
            "error",
            4,
            "Sig5 must be of length 4.",
            "lib.crb",
            "true",
            "main.cr6",
            true,
            "line 4: Sig5 must be of length 4.",
        ),
        (
            "error",
            0,
            "No BeginProg in Program.",
            "lib.crb",
            "true",
            "main.cr6",
            true,
            "No BeginProg in Program.",
        ),
        (
            "warning",
            6,
            "Delay never reached at run time.",
            "lib.crb",
            "true",
            "main.cr6",
            true,
            "line 6: Warning: Delay never reached at run time.",
        ),
        (
            "warning",
            0,
            "Table Foo, is declared but never called.",
            "",
            "true",
            "main.cr6",
            true,
            "Warning: Table Foo, is declared but never called.",
        ),
        (
            "error",
            2,
            "Nope is not defined.",
            "",
            "true",
            "main.cr6",
            true,
            "line 2: Nope is not defined.",
        ),
    ];
    fn show(s: &str) -> ShowFile {
        match s {
            "true" => ShowFile::Bool(true),
            "false" => ShowFile::Bool(false),
            _ => ShowFile::Cr3xx,
        }
    }

    #[test]
    fn infer_kind_matches_python_on_fixture_messages() {
        assert!(KINDS.len() >= 25);
        for &(sev, msg, kind) in KINDS {
            assert_eq!(infer_kind(sev, msg), kind, "{sev}: {msg}");
        }
    }

    #[test]
    fn infer_kind_severity_rules() {
        // Observed from Python: the warning-only rules also yield their kind for errors, via the
        // patterns, except those without a pattern.
        assert_eq!(
            Diagnostic::basic("warning", 1, "Open detection disabled.", "").kind,
            ""
        );
        assert_eq!(
            infer_kind("warning", "Foo never reached at run time."),
            "unreached"
        );
        assert_eq!(
            infer_kind("error", "Foo never reached at run time."),
            "unreached"
        );
        assert_eq!(
            infer_kind("warning", "Invalid fnotch value, must be 50"),
            "fnotch"
        );
        assert_eq!(
            infer_kind("warning", "Cannot open include file  X."),
            "cannot_open_include"
        );
        assert_eq!(
            infer_kind("warning", "Parameter will truncate to the nearest integer."),
            "truncate_warning"
        );
        assert_eq!(
            infer_kind("warning", "X declared but not used."),
            "unused_variable"
        );
        assert_eq!(infer_kind("error", ""), "");
    }

    #[test]
    fn cr6comp_matches_python() {
        for &(sev, line, msg, file, sf, main, dp, expected) in CR6COMP {
            let d = Diagnostic::new(sev, line, msg, file, show(sf), 0, "", "");
            assert_eq!(
                d.cr6comp(main, dp),
                expected,
                "{sev} {line} {msg} {file} {sf} {dp}"
            );
        }
    }

    #[test]
    fn equality_ignores_compare_false_fields() {
        // Observed from Python: equal despite stmt/kind/phase/pos; unequal on show_file.
        let a = Diagnostic::basic("error", 1, "a", "f");
        let b = Diagnostic::new("error", 1, "a", "f", ShowFile::Bool(true), 5, "k", "p");
        assert_eq!(a, b);
        assert_ne!(
            a,
            Diagnostic::new("error", 1, "a", "f", ShowFile::Bool(false), 0, "", "")
        );
    }

    #[test]
    fn ambient_phase_and_positions_restore_on_drop() {
        assert_eq!(active_phase(), "");
        {
            let _outer = PhaseGuard::new("early");
            assert_eq!(Diagnostic::basic("error", 3, "zzz", "f").phase, "early");
            {
                let _inner = PhaseGuard::new("late");
                assert_eq!(active_phase(), "late");
            }
            assert_eq!(active_phase(), "early");
            // An explicit phase wins.
            assert_eq!(
                Diagnostic::new("error", 3, "zzz", "f", ShowFile::Bool(true), 0, "", "mine").phase,
                "mine"
            );
        }
        assert_eq!(active_phase(), "");
        assert_eq!(Diagnostic::basic("error", 3, "zzz", "f").phase, "");

        let positions: Positions = HashMap::from([(("f".to_string(), 3), 7)]);
        {
            let _g = PositionsGuard::new(Some(Rc::new(positions)));
            assert_eq!(Diagnostic::basic("error", 3, "zzz", "f").pos, 7);
            assert_eq!(Diagnostic::basic("error", 4, "zzz", "f").pos, -1);
        }
        assert!(active_positions().is_none());
        assert_eq!(Diagnostic::basic("error", 3, "zzz", "f").pos, -1);
    }

    #[test]
    fn replace_reruns_post_init() {
        let d = Diagnostic::basic("error", 1, "x", "f");
        assert_eq!(d.kind, "");
        let r = d.replace(|d| d.message = "Duplicate Sub A".to_string());
        assert_eq!(r.kind, "duplicate_sub");
        // A kind already set is kept.
        let keep = r.replace(|d| d.message = "other".to_string());
        assert_eq!(keep.kind, "duplicate_sub");
    }

    #[test]
    fn diagnostics_collections() {
        let mut ds = Diagnostics::new();
        ds.error_basic(1, "Nope is not defined.", "f");
        ds.warning_basic(2, "X declared but not used.", "f");
        ds.error(3, "e", "f", ShowFile::Cr3xx, 9, "", "p");
        assert_eq!(ds.errors().len(), 2);
        assert_eq!(ds.warnings().len(), 1);
        assert!(ds.has_errors());
        assert_eq!(ds.items[0].kind, "not_defined");
        assert_eq!(ds.items[2].stmt, 9);
        ds.trim(Some(1));
        assert_eq!(ds.items.len(), 1);
        ds.trim(None);
        ds.trim(Some(10));
        assert_eq!(ds.items.len(), 1);
    }

    #[test]
    fn deferred_kind_sets() {
        assert!(deferred_kinds(false).contains("invalid_channel"));
        assert!(!deferred_kinds(true).contains("invalid_channel"));
        assert!(deferred_kinds(true).contains("sig_length"));
        assert!(deferred_kinds(true).contains("for_without_next"));
        assert!(!deferred_kinds(false).contains("for_without_next"));
        assert!(deferred_kinds(false).contains("next_without_for"));
    }
}
