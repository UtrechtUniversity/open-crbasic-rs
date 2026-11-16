//! Port of `source.py`: physical lines -> logical lines, with comments stripped,
//! line continuation joined, and `Include`/`IncludeSection` expanded.

use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use crate::diagnostics::Diagnostics;
use crate::pyre::{IGNORECASE, Regex};
use crate::pystr;
use crate::sources::{DiskSource, SourceProvider};
use crate::syntax::OrderedMap;
pub use crate::syntax::{LineSpan, LogicalLine, Occurrence, Piece};

static INCLUDE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"^\s*Include\s*\(?\s*"([^"]*)"\s*\)?\s*$"#, IGNORECASE));
static INCLUDE_SECTION_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"^\s*IncludeSection\s*\(\s*"([^"]*)"\s*,\s*"([^"]*)"\s*(?:,.*)?\)\s*$"#,
        IGNORECASE,
    )
});
static NESTED_INCLUDE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^\s*(?:Include(?:Section)?(?=[\s(=])|Include\s*$)",
        IGNORECASE,
    )
});
static DRIVE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(CPU|USR|CRD|USB):", IGNORECASE));
static DECL_START_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*(?:Public|Dim)\b", IGNORECASE));
static MASK_STRINGS_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#""[^"]*""#, 0));
static IDENT_PAREN_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s*(\w+)\s*(\(?)", 0));

/// Port of `source.py::_clip`.
fn clip(pieces: &[Piece], n: i64) -> Vec<Piece> {
    let mut out = Vec::new();
    for p in pieces {
        if p.start >= n {
            break;
        }
        if p.end <= n {
            out.push(p.clone());
        } else {
            let mut clipped = p.clone();
            clipped.end = n;
            out.push(clipped);
        }
    }
    out
}

/// Port of `source.py::_split_physical_lines`.
fn split_physical_lines(raw: &str) -> Vec<String> {
    raw.replace("\r\n", "\n")
        .split('\n')
        .map(str::to_owned)
        .collect()
}

/// Port of `source.py::_strip_comment`.
fn strip_comment(line: &str) -> &str {
    let mut in_string = false;
    let mut it = line.char_indices().peekable();
    while let Some((i, ch)) = it.next() {
        if in_string {
            if ch == '"' {
                if let Some(&(_, '"')) = it.peek() {
                    it.next();
                    continue;
                }
                in_string = false;
            }
            continue;
        }
        if ch == '"' {
            in_string = true;
            continue;
        }
        if ch == '\'' {
            return &line[..i];
        }
    }
    line
}

/// Port of `source.py::_is_continuation`.
fn is_continuation(line: &str) -> (bool, &str) {
    let t = pystr::rstrip_chars(line, " \t");
    if let Some(rest) = t.strip_suffix('_')
        && (rest.is_empty() || rest.ends_with([' ', '\t']))
    {
        return (true, pystr::rstrip_chars(rest, " \t"));
    }
    (false, line)
}

/// Port of `source.py::_resolve_include_path`.
fn resolve_include_path(
    spec: &str,
    directory: Option<&Path>,
    source: Option<&dyn SourceProvider>,
) -> Option<PathBuf> {
    let name = DRIVE_RE.sub("", pystr::strip(spec));
    let mut dir = directory?.to_path_buf();
    if name.is_empty() || name.starts_with(['/', '\\']) {
        return None;
    }
    let default_source = DiskSource;
    let source = source.unwrap_or(&default_source);
    let normalized = name.replace('\\', "/");
    let split_parts: Vec<&str> = normalized.split('/').collect();
    let (leaf, folders) = split_parts.split_last()?;
    let mut parts: Vec<&str> = Vec::new();
    for &part in folders {
        if part.is_empty() || part == "." {
            continue;
        }
        if part.contains(':') {
            return None;
        }
        if part == ".." {
            if parts.is_empty() {
                let abs = std::path::absolute(&dir).ok()?;
                dir = abs.parent().unwrap_or(&abs).to_path_buf();
            } else {
                parts.pop();
            }
        } else {
            parts.push(part);
        }
    }
    for part in parts {
        dir = child_directory(&dir, part, source)?;
    }
    if matches!(*leaf, "" | "." | "..") || leaf.contains(':') {
        return None;
    }
    let names = source.list_dir(&dir)?;
    let leaf_lower = pystr::lower(leaf);
    for entry in names {
        if pystr::lower(&entry) == leaf_lower {
            return Some(dir.join(entry));
        }
    }
    None
}

/// Port of `source.py::_child_directory`.
fn child_directory(directory: &Path, part: &str, source: &dyn SourceProvider) -> Option<PathBuf> {
    let exact = directory.join(part);
    if source.list_dir(&exact).is_some() {
        return Some(exact);
    }
    let part_lower = pystr::lower(part);
    if let Some(subdirs) = source.list_subdirs(directory) {
        for entry in subdirs {
            if pystr::lower(&entry) == part_lower {
                return Some(directory.join(entry));
            }
        }
    }
    None
}

/// Port of `source.py::_mask_strings`.
fn mask_strings(text: &str) -> String {
    MASK_STRINGS_RE.sub_fn(text, |m| "s".repeat(pystr::len(m.group0())))
}

/// Port of `source.py::_split_top_commas`.
fn split_top_commas(text: &str) -> Vec<String> {
    let masked = mask_strings(text);
    let mut parts = Vec::new();
    let mut depth = 0i64;
    let mut start_byte = 0usize;
    for ((byte_idx, _), ch) in text.char_indices().zip(masked.chars()) {
        if ch == '(' || ch == '{' {
            depth += 1;
        } else if ch == ')' || ch == '}' {
            depth -= 1;
        } else if ch == ',' && depth == 0 {
            parts.push(text[start_byte..byte_idx].to_owned());
            start_byte = byte_idx + ch.len_utf8();
        }
    }
    parts.push(text[start_byte..].to_owned());
    parts
}

enum TraceEdit {
    Cut(i64),
    Add {
        gap: i64,
        index: usize,
        col: i64,
        length: i64,
    },
}

/// Port of `source.py::_pull_decl_initializer`.
#[allow(clippy::cast_possible_wrap)]
fn pull_decl_initializer(
    text: &str,
    physical: &[String],
    mut i: usize,
    mut trace: Option<&mut Vec<TraceEdit>>,
) -> (String, usize, usize, Option<(String, String)>) {
    let mut text = text.to_owned();
    let mut pulls = 0usize;
    let n = physical.len();

    let next_text = |idx: usize| -> Option<(&str, &str)> {
        if idx >= n {
            return None;
        }
        let raw = &physical[idx];
        let stripped = strip_comment(raw);
        if pystr::strip(stripped).is_empty() {
            None
        } else {
            Some((raw.as_str(), stripped))
        }
    };

    let masked_owned = mask_strings(&text);
    let masked = pystr::rstrip(&masked_owned);
    if masked.ends_with('=') && !masked.ends_with("<=") && !masked.ends_with(">=") {
        let Some((raw, stripped)) = next_text(i + 1) else {
            return (text, i, 0, None);
        };
        let head = pystr::rstrip(pystr::slice(pystr::rstrip(&text), None, Some(-1))).to_owned();
        if let Some(ref mut t) = trace {
            t.push(TraceEdit::Cut(pystr::len(&head) as i64));
        }
        let mut body = pystr::lstrip_chars(stripped, " \t").to_owned();
        let names = split_top_commas(&head);
        let mut first = names.last().cloned().unwrap_or_default();
        if names.len() == 1 {
            first = DECL_START_RE.sub_n("", &first, 1);
        }
        let m = IDENT_PAREN_RE.match_(&first);
        let masked_body_owned = mask_strings(&body);
        let masked_body = pystr::rstrip(&masked_body_owned);
        let unbal = pystr::count(&body, "{") > pystr::count(&body, "}")
            && (masked_body.ends_with(',') || masked_body.ends_with('{'));
        if body.starts_with('{')
            && let Some(ref mat) = m
            && (!mat.group(2).unwrap_or("").is_empty() || unbal)
        {
            let seg = split_top_commas(&body);
            let mut echo = seg[0].clone();
            if seg.len() == 1 && pystr::len(stripped) == pystr::len(raw) {
                echo.push('\n');
            }
            let kind = if pystr::strip(&seg[0]) == "{" {
                "Invalid, or out of place expression"
            } else {
                "Incomplete expression"
            };
            let var = mat.group(1).unwrap_or("").to_owned();
            return (head, i + 1, 1, Some((format!("{kind}: {echo}."), var)));
        }
        if !body.starts_with('{') {
            body = split_top_commas(&body)
                .into_iter()
                .next()
                .unwrap_or_default();
        }
        if let Some(ref mut t) = trace {
            let col =
                (pystr::len(stripped) - pystr::len(pystr::lstrip_chars(stripped, " \t"))) as i64;
            t.push(TraceEdit::Add {
                gap: 3,
                index: i + 1,
                col,
                length: pystr::len(&body) as i64,
            });
        }
        return (format!("{head} = {body}"), i + 1, 1, None);
    }

    loop {
        let m_owned = mask_strings(&text);
        let m = pystr::rstrip(&m_owned);
        let after = match m.rfind('=') {
            Some(pos) => &m[pos + 1..],
            None => "",
        };
        if !(pystr::count(after, "{") > pystr::count(after, "}")
            && (m.ends_with(',') || m.ends_with('{')))
        {
            break;
        }
        let Some((_, nxt_stripped)) = next_text(i + pulls + 1) else {
            break;
        };
        let nxt_l = pystr::lstrip_chars(nxt_stripped, " \t");
        if let Some(ref mut t) = trace {
            t.push(TraceEdit::Cut(pystr::len(pystr::rstrip(&text)) as i64));
            let col = (pystr::len(nxt_stripped) - pystr::len(nxt_l)) as i64;
            t.push(TraceEdit::Add {
                gap: 1,
                index: i + pulls + 1,
                col,
                length: pystr::len(nxt_l) as i64,
            });
        }
        text = format!("{} {nxt_l}", pystr::rstrip(&text));
        pulls += 1;
    }
    i += pulls;
    (text, i, pulls, None)
}

/// Port of `source.py::bad_include_extension`.
pub fn bad_include_extension(
    spec: &str,
    own_ext: Option<&str>,
    cr3xx_exts: Option<&[&str]>,
) -> bool {
    if let Some(exts) = cr3xx_exts {
        let ext = if spec.contains('.') {
            pystr::upper(pystr::rpartition(spec, ".").2)
        } else {
            String::new()
        };
        let ext_len = pystr::len(&ext);
        return (ext_len == 3 && ext != "CRB")
            || (ext_len == 5 && ext.starts_with("CR3") && !exts.contains(&ext.as_str()));
    }
    if let Some(own) = own_ext {
        let ext = pystr::upper(pystr::rpartition(spec, ".").2);
        return !(ext.starts_with("CRB") || ext.starts_with(own));
    }
    false
}

/// Port of `source.py::_Loader`.
struct Loader<'a> {
    provenance: bool,
    source: &'a dyn SourceProvider,
    includes: OrderedMap<String, PathBuf>,
    directory: Option<PathBuf>,
    pull_init: bool,
    pull_limit: Option<Vec<i64>>,
    pull_log: Vec<(Vec<i64>, Vec<i64>)>,
    site: OrderedMap<String, Vec<i64>>,
    #[allow(dead_code)]
    diags: &'a mut Diagnostics,
    own_ext: Option<String>,
    cr3xx_exts: Option<Vec<String>>,
    files: Vec<Vec<u8>>,
    occurrences: Vec<Occurrence>,
    cur: Option<i64>,
    path: Option<PathBuf>,
    site_phys: Option<(i64, i64)>,
}

impl<'a> Loader<'a> {
    /// Port of `source.py::_Loader.__init__`.
    fn new(
        directory: Option<PathBuf>,
        diags: &'a mut Diagnostics,
        own_ext: Option<&str>,
        pull_init: bool,
        cr3xx_exts: Option<&[&str]>,
        source: &'a dyn SourceProvider,
        provenance: bool,
        _macros: bool,
    ) -> Self {
        // macros are out of scope; take the no-macros path.
        Self {
            provenance,
            source,
            includes: OrderedMap::new(),
            directory,
            pull_init,
            pull_limit: None,
            pull_log: Vec::new(),
            site: OrderedMap::new(),
            diags,
            own_ext: own_ext.map(str::to_owned),
            cr3xx_exts: cr3xx_exts.map(|s| s.iter().map(|&x| x.to_owned()).collect()),
            files: Vec::new(),
            occurrences: Vec::new(),
            cur: None,
            path: None,
            site_phys: None,
        }
    }

    /// Port of `source.py::_Loader.load_file`.
    fn load_file(
        &mut self,
        path: &Path,
        section: Option<&str>,
        in_include: bool,
        display: Option<&str>,
    ) -> Vec<LogicalLine> {
        let Some(data) = self.source.read(path) else {
            return Vec::new();
        };
        self.files.push(data.clone());
        let saved = self.cur;
        self.path = Some(path.to_path_buf());
        let fallback_name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let filename = display.unwrap_or(&fallback_name);
        let decoded = pystr::decode_latin1(&data);
        let lines = self.load_text(&decoded, filename, section, in_include);
        self.cur = saved;
        lines
    }

    /// Port of `source.py::_Loader.load_text`.
    #[allow(clippy::cast_possible_wrap)]
    fn load_text(
        &mut self,
        raw: &str,
        filename: &str,
        section: Option<&str>,
        in_include: bool,
    ) -> Vec<LogicalLine> {
        let physical = split_physical_lines(raw);
        let prov = self.provenance;
        let occ = self.occurrences.len() as i64;
        let parent = self.cur;
        let site = if parent.is_some() {
            self.site_phys
        } else {
            None
        };
        self.occurrences.push(Occurrence {
            id: occ,
            file: filename.to_owned(),
            path: self.path.take(),
            parent,
            site,
        });
        self.cur = Some(occ);

        let mut out: Vec<LogicalLine> = Vec::new();
        let mut i = 0usize;
        let n = physical.len();
        let real_n = if n > 1 && physical[n - 1].is_empty() {
            n - 1
        } else {
            n
        };
        let mut shift = 0i64;
        while i < n {
            let start_line = (i + 1) as i64;
            let mut raw_line = &physical[i];
            let mut buffer_text = raw_line.clone();
            if pystr::len(&buffer_text) + usize::from(i + 1 < n) > 510 {
                let mut ll = LogicalLine::new("", start_line - shift, filename);
                ll.section = section.map(str::to_owned);
                "overflow".clone_into(&mut ll.kind);
                ll.phys = Some((start_line, start_line));
                ll.occurrence = occ;
                out.push(ll);
                break;
            }
            let mut text = strip_comment(raw_line).to_owned();
            let mut had_comment = text.len() != raw_line.len();
            let mut pieces = if !text.is_empty() && prov {
                vec![Piece {
                    start: 0,
                    end: pystr::len(&text) as i64,
                    file: filename.to_owned(),
                    line: start_line,
                    col: 0,
                    occurrence: occ,
                }]
            } else {
                Vec::new()
            };
            let mut last_line = start_line;
            loop {
                let (cont, prefix_ref) = is_continuation(&text);
                let prefix = prefix_ref.to_owned();
                if cont && i + 1 >= real_n {
                    text.clear();
                    pieces.clear();
                    break;
                }
                if !cont || i + 1 >= n {
                    if cont {
                        text = prefix;
                    }
                    if prov {
                        pieces = clip(&pieces, pystr::len(&text) as i64);
                    }
                    break;
                }
                let stripped_buf = strip_comment(&buffer_text);
                let trimmed_buf = pystr::rstrip_chars(stripped_buf, " \t");
                buffer_text = pystr::slice(trimmed_buf, None, Some(-1)).to_owned();
                i += 1;
                last_line = (i + 1) as i64;
                raw_line = &physical[i];
                buffer_text.push_str(raw_line);
                if pystr::len(&buffer_text) + usize::from(i + 1 < n) > 510 {
                    let mut ll = LogicalLine::new("", last_line - shift, filename);
                    ll.section = section.map(str::to_owned);
                    "overflow".clone_into(&mut ll.kind);
                    ll.phys = Some((start_line, last_line));
                    ll.occurrence = occ;
                    out.push(ll);
                    return out;
                }
                let stripped = strip_comment(raw_line);
                had_comment = stripped.len() != raw_line.len();
                let (s_cont, s_prefix) = is_continuation(stripped);
                let lone =
                    pystr::strip(&prefix).is_empty() && s_cont && pystr::strip(s_prefix).is_empty();
                let joined = pystr::len(pystr::rstrip_chars(&text, " \t")) as i64 - 1;
                text = if lone {
                    stripped.to_owned()
                } else {
                    let head = pystr::slice(pystr::rstrip_chars(&text, " \t"), None, Some(-1));
                    format!("{head}{stripped}")
                };
                pieces = if lone || !prov {
                    Vec::new()
                } else {
                    clip(&pieces, joined)
                };
                if !stripped.is_empty() && prov {
                    pieces.push(Piece {
                        start: if lone { 0 } else { joined },
                        end: pystr::len(&text) as i64,
                        file: filename.to_owned(),
                        line: last_line,
                        col: 0,
                        occurrence: occ,
                    });
                }
            }
            if pystr::strip(&text).is_empty() {
                i += 1;
                continue;
            }
            // macros are out of scope; take the no-macros path.
            let mut pulls = 0usize;
            let mut bad = None;
            let orig = text.clone();
            let orig_pieces = pieces.clone();
            let mut pos = self.site.get(filename).cloned().unwrap_or_default();
            pos.push(start_line - shift);
            if self.pull_init
                && DECL_START_RE.match_(&text).is_some()
                && self
                    .pull_limit
                    .as_ref()
                    .is_none_or(|lim| pos.as_slice() < lim.as_slice())
            {
                let mut trace = Vec::new();
                let res = pull_decl_initializer(&text, &physical, i, Some(&mut trace));
                text = res.0;
                i = res.1;
                pulls = res.2;
                bad = res.3;
                let mut size = pystr::len(&orig) as i64;
                if prov {
                    for edit in trace {
                        match edit {
                            TraceEdit::Cut(cut) => {
                                pieces = clip(&pieces, cut);
                                size = cut;
                            }
                            TraceEdit::Add {
                                gap,
                                index,
                                col,
                                length,
                            } => {
                                pieces.push(Piece {
                                    start: size + gap,
                                    end: size + gap + length,
                                    file: filename.to_owned(),
                                    line: (index + 1) as i64,
                                    col,
                                    occurrence: occ,
                                });
                                size += gap + length;
                            }
                        }
                    }
                }
                if pulls > 0 {
                    let mut end_pos = self.site.get(filename).cloned().unwrap_or_default();
                    end_pos.push(last_line - shift);
                    self.pull_log.push((pos, end_pos));
                    had_comment = strip_comment(&physical[i]).len() != physical[i].len();
                }
            }
            self.site_phys = Some((start_line, (i + 1) as i64));
            let mut new_lines = self.expand_or_keep(
                &text,
                last_line - shift,
                filename,
                section,
                in_include,
                had_comment,
            );
            for made in &mut new_lines {
                if made.phys.is_none() {
                    made.phys = Some((start_line, (i + 1) as i64));
                    made.occurrence = occ;
                }
            }
            if new_lines.len() == 1 && new_lines[0].kind == "code" {
                new_lines[0].pieces = pieces;
            }
            if pulls > 0 && new_lines.len() == 1 && new_lines[0].kind == "code" {
                let base = last_line - shift - (pulls as i64);
                new_lines[0].pulls = pulls as i64;
                new_lines[0].bad_init = bad;
                let mut unpulled = Vec::with_capacity(1 + pulls);
                let mut first_ll = LogicalLine::new(&orig, base, filename);
                first_ll.section = section.map(str::to_owned);
                first_ll.pieces = orig_pieces;
                first_ll.phys = Some((start_line, last_line));
                first_ll.occurrence = occ;
                unpulled.push(first_ll);
                let last_line_usize = usize::try_from(last_line).unwrap_or(0);
                for k in 1..=pulls {
                    let phys_idx = last_line_usize - 1 + k;
                    let phys_line = &physical[phys_idx];
                    let mut ll =
                        LogicalLine::new(strip_comment(phys_line), base + (k as i64), filename);
                    ll.section = section.map(str::to_owned);
                    ll.pieces = if prov {
                        Self::whole(phys_line, filename, last_line + (k as i64), occ)
                    } else {
                        Vec::new()
                    };
                    ll.phys = Some((last_line + (k as i64), last_line + (k as i64)));
                    ll.occurrence = occ;
                    unpulled.push(ll);
                }
                new_lines[0].unpulled = Some(unpulled);
            }
            out.extend(new_lines);
            shift += pulls as i64;
            i += 1;
        }
        out
    }

    /// Port of `source.py::_Loader._whole`.
    #[allow(clippy::cast_possible_wrap)]
    fn whole(raw: &str, filename: &str, line: i64, occ: i64) -> Vec<Piece> {
        let n = pystr::len(strip_comment(raw)) as i64;
        if n > 0 {
            vec![Piece {
                start: 0,
                end: n,
                file: filename.to_owned(),
                line,
                col: 0,
                occurrence: occ,
            }]
        } else {
            Vec::new()
        }
    }

    /// Port of `source.py::_Loader._expand_or_keep`.
    fn expand_or_keep(
        &mut self,
        text: &str,
        line: i64,
        filename: &str,
        section: Option<&str>,
        in_include: bool,
        had_comment: bool,
    ) -> Vec<LogicalLine> {
        if in_include && NESTED_INCLUDE_RE.match_(text).is_some() {
            return vec![Self::nested_include(line, filename, section)];
        }
        if let Some(m) = INCLUDE_SECTION_RE.match_(text) {
            let name = m.group(1).unwrap_or("").to_owned();
            let spec = m.group(2).unwrap_or("").to_owned();
            if self.bad_extension(&spec) {
                let mut err = Self::extension_error(&spec, line, filename, section);
                err.stops_includes = true;
                return vec![err];
            }
            let target = if self.unloadable(&spec) {
                None
            } else {
                resolve_include_path(&spec, self.directory.as_deref(), Some(self.source))
            };
            let Some(target) = target else {
                return vec![Self::missing(&spec, line, filename, section)];
            };
            let spec_trimmed = pystr::strip(&spec);
            if !self.site.contains_key(spec_trimmed) {
                let mut pos = self.site.get(filename).cloned().unwrap_or_default();
                pos.push(line);
                self.site.insert(spec_trimmed.to_owned(), pos);
            }
            if !self.includes.contains_key(spec_trimmed) {
                self.includes
                    .insert(spec_trimmed.to_owned(), target.clone());
            }
            let included = self.load_file(&target, Some(&name), true, Some(spec_trimmed));
            return Self::include_lines(included, line, filename, section);
        }
        if let Some(m) = INCLUDE_RE.match_(text) {
            let spec = m.group(1).unwrap_or("").to_owned();
            if self.bad_extension(&spec) {
                return vec![Self::extension_error(&spec, line, filename, section)];
            }
            let target = if self.unloadable(&spec) {
                None
            } else {
                resolve_include_path(&spec, self.directory.as_deref(), Some(self.source))
            };
            let Some(target) = target else {
                return vec![Self::missing(&spec, line, filename, section)];
            };
            let spec_trimmed = pystr::strip(&spec);
            if !self.site.contains_key(spec_trimmed) {
                let mut pos = self.site.get(filename).cloned().unwrap_or_default();
                pos.push(line);
                self.site.insert(spec_trimmed.to_owned(), pos);
            }
            if !self.includes.contains_key(spec_trimmed) {
                self.includes
                    .insert(spec_trimmed.to_owned(), target.clone());
            }
            let included = self.load_file(&target, None, true, Some(spec_trimmed));
            return Self::include_lines(included, line, filename, section);
        }
        let mut ll = LogicalLine::new(text, line, filename);
        ll.section = section.map(str::to_owned);
        ll.had_trailing_comment = had_comment;
        vec![ll]
    }

    /// Port of `source.py::_Loader._bad_extension`.
    fn bad_extension(&self, spec: &str) -> bool {
        let cr3xx_refs: Option<Vec<&str>> = self
            .cr3xx_exts
            .as_ref()
            .map(|v| v.iter().map(String::as_str).collect());
        bad_include_extension(spec, self.own_ext.as_deref(), cr3xx_refs.as_deref())
    }

    /// Port of `source.py::_Loader._unloadable`.
    fn unloadable(&self, spec: &str) -> bool {
        let ext = if spec.contains('.') {
            pystr::upper(pystr::rpartition(spec, ".").2)
        } else {
            String::new()
        };
        if let Some(ref exts) = self.cr3xx_exts {
            return ext != "CRB" && !exts.contains(&ext);
        }
        false
    }

    /// Port of `source.py::_Loader._nested_include`.
    fn nested_include(line: i64, filename: &str, section: Option<&str>) -> LogicalLine {
        let mut ll = LogicalLine::new(
            "Include file within an Include file is illegal.",
            line,
            filename,
        );
        ll.section = section.map(str::to_owned);
        "include_error".clone_into(&mut ll.kind);
        ll
    }

    /// Port of `source.py::_Loader._extension_error`.
    fn extension_error(
        spec: &str,
        line: i64,
        filename: &str,
        section: Option<&str>,
    ) -> LogicalLine {
        let mut ll = LogicalLine::new(
            &format!("Include file {spec} must have valid program file extension."),
            line,
            filename,
        );
        ll.section = section.map(str::to_owned);
        "include_error".clone_into(&mut ll.kind);
        ll
    }

    /// Port of `source.py::_Loader._missing`.
    fn missing(spec: &str, line: i64, filename: &str, section: Option<&str>) -> LogicalLine {
        let mut ll = LogicalLine::new(
            &format!("Cannot open include file  {spec}."),
            line,
            filename,
        );
        ll.section = section.map(str::to_owned);
        "include_warning".clone_into(&mut ll.kind);
        ll
    }

    /// Port of `source.py::_Loader._include_lines`.
    fn include_lines(
        included: Vec<LogicalLine>,
        line: i64,
        filename: &str,
        section: Option<&str>,
    ) -> Vec<LogicalLine> {
        let mut begin = LogicalLine::new("", line, filename);
        begin.section = section.map(str::to_owned);
        "include_begin".clone_into(&mut begin.kind);
        let mut end = LogicalLine::new("", line, filename);
        end.section = section.map(str::to_owned);
        "include_end".clone_into(&mut end.kind);
        let mut out = Vec::with_capacity(2 + included.len());
        out.push(begin);
        out.extend(included);
        out.push(end);
        out
    }
}

/// Port of `source.py::load_program`.
pub fn load_program(
    path: &Path,
    own_ext: Option<&str>,
    pull_init: bool,
    pull_limit: Option<&[i64]>,
    cr3xx_exts: Option<&[&str]>,
    source: Option<&dyn SourceProvider>,
    provenance: bool,
    macros: bool,
) -> (Vec<LogicalLine>, Diagnostics) {
    let mut diags = Diagnostics::new();
    let default_source = DiskSource;
    let src = source.unwrap_or(&default_source);
    let mut loader = Loader::new(
        // (Python: `Path("x.CR6").parent` is `.`, Rust's is the empty path, which is no directory)
        path.parent().map(|p| {
            if p.as_os_str().is_empty() {
                PathBuf::from(".")
            } else {
                p.to_path_buf()
            }
        }),
        &mut diags,
        own_ext,
        pull_init,
        cr3xx_exts,
        src,
        provenance,
        macros,
    );
    loader.pull_limit = pull_limit.map(<[i64]>::to_vec);
    let lines = loader.load_file(path, None, false, None);
    // macros are out of scope; skip loader.macros.finish().
    let pull_log = loader.pull_log;
    let site = loader.site;
    let includes = loader.includes;
    let occurrences = loader.occurrences;
    diags.provenance.pull_log = Some(pull_log);
    diags.provenance.pull_site = Some(site);
    diags.provenance.include_paths = Some(includes);
    diags.provenance.occurrences = Some(occurrences);
    (lines, diags)
}

/// Port of `source.py::load_text`.
pub fn load_text(
    text: &str,
    name: &str,
    directory: Option<&Path>,
    own_ext: Option<&str>,
    pull_init: bool,
    cr3xx_exts: Option<&[&str]>,
) -> (Vec<LogicalLine>, Diagnostics) {
    let mut diags = Diagnostics::new();
    let default_source = DiskSource;
    let mut loader = Loader::new(
        directory.map(Path::to_path_buf),
        &mut diags,
        own_ext,
        pull_init,
        cr3xx_exts,
        &default_source,
        true,
        false,
    );
    let lines = loader.load_text(text, name, None, false);
    let occurrences = loader.occurrences;
    diags.provenance.occurrences = Some(occurrences);
    (lines, diags)
}
