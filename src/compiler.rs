//! Port of `compiler.py`.

#![allow(dead_code, unused_variables, clippy::used_underscore_items)]

use std::any::Any;
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};
use std::path::Path;

use crate::diagnostics::{DiagRef, Diagnostic, Diagnostics};
use crate::profiles::{self, Profile};
use crate::pystr;
use crate::sources::SourceProvider;
use crate::syntax::{OrderedMap, ProgramRef};
use crate::tdf::TdfTable;

const _MESSAGE_BUFFER_LIMIT: usize = 4096;
const _IMPLEMENTATION_FAILURE_MESSAGE: &str = "Internal error: unable to compile program.";

fn panic_message(payload: &(dyn Any + Send)) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "panic".to_string()
    }
}

/// Port of `compiler.py::_no_line_kind`.
pub fn _no_line_kind(d: &Diagnostic) -> i64 {
    i64::from(!d.message.ends_with("declared but not used."))
}

/// Port of `compiler.py::_bounded_by_message_buffer`.
pub fn _bounded_by_message_buffer(lines: Vec<String>) -> Vec<String> {
    let mut used = 0;
    let mut out = Vec::new();
    for line in lines {
        let cost = pystr::encode_latin1_replace(&line).len() + 2;
        if used + cost > _MESSAGE_BUFFER_LIMIT {
            break;
        }
        used += cost;
        out.push(line);
    }
    out
}

/// Port of `compiler.py::CompileResult`.
#[derive(Debug, Clone)]
pub struct CompileResult {
    pub program: String,
    pub profile: &'static Profile,
    pub diagnostics: Diagnostics,
    pub mode: Option<String>,
    pub tables: Vec<TdfTable>,
    pub table_list: Option<String>,
    /// Attributes Python hangs on each `TdfTable` (`_dup_fields`, `_interval_display`, `_storage`).
    pub table_extras: crate::tables::TableExtras,
    pub signature: Option<i64>,
    pub file_order: OrderedMap<String, i64>,
    pub stmt_lines: OrderedMap<String, Vec<(i64, i64)>>,
    pub implementation_exceptions: Vec<String>,
    pub program_ast: Option<ProgramRef>,
    pub pass1_errors: Option<bool>,
    pub findings: Vec<String>,
    pub suppressed: Vec<String>,
}

impl Default for CompileResult {
    fn default() -> Self {
        Self::new(
            "",
            profiles::get("cr6").expect("cr6 is a known logger"),
            Diagnostics::new(),
        )
    }
}

impl CompileResult {
    pub fn new(program: &str, profile: &'static Profile, diagnostics: Diagnostics) -> Self {
        Self {
            program: program.to_string(),
            profile,
            diagnostics,
            mode: None,
            tables: Vec::new(),
            table_list: None,
            table_extras: crate::tables::TableExtras::default(),
            signature: None,
            file_order: OrderedMap::new(),
            stmt_lines: OrderedMap::new(),
            implementation_exceptions: Vec::new(),
            program_ast: None,
            pass1_errors: None,
            findings: Vec::new(),
            suppressed: Vec::new(),
        }
    }

    /// Port of `compiler.py::CompileResult::record_implementation_failure`.
    pub fn record_implementation_failure(&mut self, where_: &str, exc: &str) {
        self.implementation_exceptions
            .push(format!("{where_}: {exc}"));
        if !self
            .diagnostics
            .errors()
            .iter()
            .any(|d| d.message == _IMPLEMENTATION_FAILURE_MESSAGE)
        {
            self.diagnostics
                .error_basic(0, _IMPLEMENTATION_FAILURE_MESSAGE, "");
        }
    }

    /// Port of `compiler.py::CompileResult::_position`.
    pub fn _position(&self, d: &Diagnostic) -> (i64, i64) {
        let file = if d.file.is_empty() {
            &self.program
        } else {
            &d.file
        };
        let Some(lines) = self.stmt_lines.get(file) else {
            return (*self.file_order.get(file).unwrap_or(&0), d.line);
        };
        if lines.is_empty() {
            return (*self.file_order.get(file).unwrap_or(&0), d.line);
        }
        let i = lines.partition_point(|&(l, idx)| (l, idx) < (d.line, -1));
        let index = if i < lines.len() {
            lines[i].1
        } else {
            lines[lines.len() - 1].1 + 1
        };
        (index, d.line)
    }

    /// Port of `compiler.py::CompileResult::ok`.
    pub fn ok(&self) -> bool {
        !self.diagnostics.has_errors() && self.implementation_exceptions.is_empty()
    }

    /// Port of `compiler.py::CompileResult::_sort_key`.
    pub fn _sort_key(&self, d: &Diagnostic) -> (bool, i64, i64, i64) {
        if d.line == 0 {
            return (true, _no_line_kind(d), 0, 0);
        }
        let file = if d.file.is_empty() {
            &self.program
        } else {
            &d.file
        };
        if self.profile.include_warning_first
            && file == &self.program
            && d.severity == "warning"
            && d.message.starts_with("Cannot open include file ")
        {
            return (false, -1, d.line, 0);
        }
        if d.severity == "warning"
            && d.message == "Parameter will truncate to the nearest integer."
            && !self.diagnostics.has_errors()
        {
            return (false, -1, d.line, 0);
        }
        let pos = self._position(d);
        let third = if d.message.ends_with("never reached at run time.") {
            2
        } else {
            i64::from(d.message.ends_with("illegally occurs before BeginProg."))
        };
        (false, pos.0, pos.1, third)
    }

    /// Port of `compiler.py::CompileResult::result_lines`.
    pub fn result_lines(&self, show_signature: bool) -> Vec<String> {
        let mut ordered = self.diagnostics.items.clone();
        ordered.sort_by_key(|d| self._sort_key(d));
        let drive = self.profile.include_drive_prefix;
        if !self.ok() {
            let rendered_all: Vec<String> = ordered
                .iter()
                .map(|d| d.cr6comp(&self.program, drive))
                .collect();
            let mut rendered = _bounded_by_message_buffer(rendered_all);
            if let Some(last) = rendered.last_mut() {
                *last = pystr::rstrip_chars(last, "\n").to_string();
            }
            let mut out = vec![
                format!("{} -- Compile Failed!", self.program),
                String::new(),
            ];
            out.extend(rendered);
            return out;
        }
        let mut lines = if self.profile.reports_mode {
            let mode = self
                .mode
                .as_deref()
                .filter(|s| !s.is_empty())
                .unwrap_or("Sequential");
            vec![format!("{} -- Compiled in {}Mode.", self.program, mode)]
        } else {
            vec![format!("{} -- Compiled OK.", self.program)]
        };
        if show_signature && let Some(sig) = self.signature {
            lines.push(format!("ProgSignature =  {sig}."));
        }
        for d in ordered {
            if d.severity == "warning" {
                lines.push(d.cr6comp(&self.program, drive));
            }
        }
        lines
    }
}

/// Port of `compiler.py::dropped_message`.
pub fn dropped_message(result: &CompileResult) -> Option<DiagRef> {
    if result.ok() {
        return None;
    }
    let mut ordered = result.diagnostics.items.clone();
    ordered.sort_by_key(|d| result._sort_key(d));
    let drive = result.profile.include_drive_prefix;
    let lines: Vec<String> = ordered
        .iter()
        .map(|d| d.cr6comp(&result.program, drive))
        .collect();
    let kept = _bounded_by_message_buffer(lines).len();
    ordered.get(kept).cloned()
}

/// Port of `compiler.py::compile_file`.
#[allow(clippy::too_many_arguments)]
pub fn compile_file(
    path: &Path,
    profile: Option<&'static Profile>,
    display_name: Option<&str>,
    warn_unused: bool,
    source: Option<&dyn SourceProvider>,
    signature: bool,
    provenance: bool,
    macros: bool,
) -> CompileResult {
    let profile = profile.unwrap_or_else(|| profiles::for_file(path));
    let name = display_name.map_or_else(
        || {
            path.file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default()
        },
        str::to_string,
    );
    let mut result = _compile_once(path, profile, &name, None, source, provenance, macros);
    if profile.single_pass {
        let mut limit: Option<Vec<i64>> = None;
        for _ in 0..16 {
            let Some(ref ast) = result.program_ast else {
                break;
            };
            let log: Vec<(Vec<i64>, Vec<i64>)> =
                ast.provenance.pull_log.clone().unwrap_or_default();
            let site: OrderedMap<String, Vec<i64>> =
                ast.provenance.pull_site.clone().unwrap_or_default();
            let errs: Vec<Vec<i64>> = result
                .diagnostics
                .items
                .iter()
                .filter(|d| {
                    d.severity == "error"
                        && d.line != 0
                        && !d.message.ends_with("illegally occurs before BeginProg.")
                })
                .map(|d| {
                    let mut v = site.get(&d.file).cloned().unwrap_or_default();
                    v.push(d.line);
                    v
                })
                .collect();
            let bad = log
                .into_iter()
                .find(|(_start, end)| errs.iter().any(|e| e < end))
                .map(|(start, _end)| start);
            let Some(bad) = bad else {
                break;
            };
            if limit.as_ref().is_some_and(|l| &bad >= l) {
                break;
            }
            limit = Some(bad);
            result = _compile_once(
                path,
                profile,
                &name,
                limit.as_deref(),
                source,
                provenance,
                macros,
            );
        }
    }
    if result.program_ast.is_none() {
        return result;
    }
    _finish(&mut result, path, warn_unused, source, signature)
}

/// Port of `compiler.py::_compile_once`.
pub fn _compile_once(
    path: &Path,
    profile: &'static Profile,
    name: &str,
    pull_limit: Option<&[i64]>,
    source: Option<&dyn SourceProvider>,
    provenance: bool,
    macros: bool,
) -> CompileResult {
    let parsed = catch_unwind(AssertUnwindSafe(|| {
        crate::parser::parse_file(path, Some(profile), pull_limit, source, provenance, macros)
    }));
    let (program, diags) = match parsed {
        Ok(Ok(pair)) => pair,
        Ok(Err(exc)) => {
            let mut diags = Diagnostics::new();
            diags.error_basic(0, "Internal error: unable to parse program.", "");
            let mut result = CompileResult::new(name, profile, diags);
            result
                .implementation_exceptions
                .push(format!("parser.parse_file: {exc}"));
            return result;
        }
        Err(payload) => {
            let msg = panic_message(&*payload);
            if std::env::var("OPENCRBASIC_STRICT").as_deref() == Ok("1") {
                resume_unwind(payload);
            }
            let mut diags = Diagnostics::new();
            diags.error_basic(0, "Internal error: unable to parse program.", "");
            let mut result = CompileResult::new(name, profile, diags);
            result
                .implementation_exceptions
                .push(format!("parser.parse_file: {msg}"));
            return result;
        }
    };

    let mut file_order: OrderedMap<String, i64> = OrderedMap::new();
    let mut stmt_lines_map: std::collections::HashMap<String, Vec<(i64, i64)>> =
        std::collections::HashMap::new();
    for (index, stmt) in program.statements.iter().enumerate() {
        let sfile = stmt.file();
        let file = if sfile.is_empty() { name } else { sfile };
        if !file_order.contains_key(file) {
            let rank = i64::try_from(file_order.len()).unwrap_or(i64::MAX);
            file_order.insert(file.to_string(), rank);
        }
        let idx = i64::try_from(index).unwrap_or(i64::MAX);
        stmt_lines_map
            .entry(file.to_string())
            .or_default()
            .push((stmt.line(), idx));
    }
    let mut stmt_lines: OrderedMap<String, Vec<(i64, i64)>> = OrderedMap::new();
    for file in file_order.keys() {
        if let Some(mut lines) = stmt_lines_map.remove(file) {
            lines.sort_unstable();
            stmt_lines.insert(file.clone(), lines);
        }
    }

    let mut result = CompileResult::new(name, profile, diags);
    result.file_order = file_order;
    result.stmt_lines = stmt_lines;
    result.program_ast = Some(program.clone());

    let file_name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let fname_len = pystr::len(&file_name);
    if fname_len + 1 > 60 {
        let excess = i64::try_from(fname_len - 59).unwrap_or(i64::MAX);
        result.diagnostics.error_basic(
            0,
            &format!("Program name exceeds maximum allowable characters by {excess}."),
            "",
        );
    }

    let mut diags_tmp = std::mem::take(&mut result.diagnostics);
    let check_res = catch_unwind(AssertUnwindSafe(|| {
        crate::checker::check(&program, profile, &mut diags_tmp, &mut result);
    }));
    result.diagnostics = diags_tmp;
    if let Err(payload) = check_res {
        let msg = panic_message(&*payload);
        result.record_implementation_failure("checker.check", &msg);
        if std::env::var("OPENCRBASIC_STRICT").as_deref() == Ok("1") {
            resume_unwind(payload);
        }
    }

    if program.unclosed_if && !result.diagnostics.has_errors() {
        result.diagnostics.items.clear();
        result.diagnostics.error_basic(0, "#If with no #Endif", "");
    }
    result
}

/// Port of `compiler.py::_finish`.
pub fn _finish(
    result: &mut CompileResult,
    _path: &Path,
    warn_unused: bool,
    _source: Option<&dyn SourceProvider>,
    signature: bool,
) -> CompileResult {
    if warn_unused && result.program_ast.is_some() && !result.diagnostics.has_errors() {
        // TODO(wire): `checker/unused.py::check(program, diags)` (the `-u` warnings): `checker::unused` is not ported yet.
    }
    if !signature {
        return result.clone();
    }
    // TODO(wire): `result.signature = signature.program_signature(path, source)`: `signature` is not ported yet.
    result.clone()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostics::ShowFile;

    #[test]
    fn result_lines_ok_modes_and_signature() {
        let cr6 = profiles::get("cr6").unwrap();
        let mut res = CompileResult::new("test.CR6", cr6, Diagnostics::new());
        res.mode = Some("Sequential".to_string());
        res.signature = Some(12345);
        assert_eq!(
            res.result_lines(false),
            vec!["test.CR6 -- Compiled in SequentialMode."]
        );
        assert_eq!(
            res.result_lines(true),
            vec![
                "test.CR6 -- Compiled in SequentialMode.",
                "ProgSignature =  12345."
            ]
        );

        let cr300 = profiles::get("cr300").unwrap();
        let res300 = CompileResult::new("test.CR300", cr300, Diagnostics::new());
        assert_eq!(
            res300.result_lines(false),
            vec!["test.CR300 -- Compiled OK."]
        );
    }

    #[test]
    fn sort_key_ordering_matches_python() {
        let cr6 = profiles::get("cr6").unwrap();
        let mut diags = Diagnostics::new();
        // No-line warning (table declared but never called)
        diags.warning(0, "DataTable Foo declared but never called.", "", "", "");
        // No-line unused warning (-u)
        diags.warning(0, "Variable X declared but not used.", "", "", "");
        // Reachability warning on line 5
        diags.warning(5, "X = 1 never reached at run time.", "", "", "");
        // Error on line 5
        diags.error(5, "Bad expr.", "", ShowFile::Bool(true), 0, "", "");
        // Error on line 2
        diags.error(2, "First err.", "", ShowFile::Bool(true), 0, "", "");

        let res = CompileResult::new("prog.CR6", cr6, diags);
        assert_eq!(
            res.result_lines(false),
            vec![
                "prog.CR6 -- Compile Failed!",
                "",
                "line 2: First err.",
                "line 5: Bad expr.",
                "line 5: Warning: X = 1 never reached at run time.",
                "Warning: Variable X declared but not used.",
                "Warning: DataTable Foo declared but never called.",
            ]
        );
    }

    #[test]
    fn message_buffer_4096_truncation_and_dropped_message() {
        let cr6 = profiles::get("cr6").unwrap();
        let mut diags = Diagnostics::new();
        // Each line is "line N: " + 52 chars = ~60 bytes + 2 CRLF = 62 bytes.
        // 67 * 62 = 4154 > 4096, so some messages must be dropped.
        let payload = "x".repeat(52);
        for i in 1..=80 {
            diags.error_basic(i, &payload, "");
        }
        let res = CompileResult::new("buf.CR6", cr6, diags);
        let lines = res.result_lines(false);
        let body = &lines[2..];
        let total_bytes: usize = body
            .iter()
            .map(|l| pystr::encode_latin1_replace(l).len() + 2)
            .sum();
        assert!(total_bytes <= 4096);
        let dropped = dropped_message(&res).expect("should drop overflow");
        assert_eq!(dropped.line, i64::try_from(body.len() + 1).unwrap());
    }

    #[test]
    fn record_implementation_failure_adds_single_error() {
        let cr6 = profiles::get("cr6").unwrap();
        let mut res = CompileResult::new("fail.CR6", cr6, Diagnostics::new());
        res.record_implementation_failure("checker.check", "boom");
        res.record_implementation_failure("checker.other", "boom2");
        assert!(!res.ok());
        assert_eq!(res.implementation_exceptions.len(), 2);
        assert_eq!(res.diagnostics.errors().len(), 1);
        assert_eq!(
            res.diagnostics.errors()[0].message,
            _IMPLEMENTATION_FAILURE_MESSAGE
        );
    }

    #[test]
    fn position_uses_stmt_lines_bisect() {
        let cr6 = profiles::get("cr6").unwrap();
        let mut res = CompileResult::new("main.CR6", cr6, Diagnostics::new());
        res.file_order.insert("main.CR6".to_string(), 0);
        res.file_order.insert("lib.crb".to_string(), 1);
        res.stmt_lines
            .insert("main.CR6".to_string(), vec![(1, 0), (10, 3)]);
        res.stmt_lines
            .insert("lib.crb".to_string(), vec![(2, 1), (5, 2)]);

        let d_lib = Diagnostic::basic("error", 4, "err in lib", "lib.crb");
        let d_main = Diagnostic::basic("error", 10, "err in main", "main.CR6");
        let d_after = Diagnostic::basic("error", 20, "err after", "main.CR6");
        assert_eq!(res._position(&d_lib), (2, 4));
        assert_eq!(res._position(&d_main), (3, 10));
        assert_eq!(res._position(&d_after), (4, 20));
    }
}
