//! Port of `preprocess.py`: `-p FILE` / `-i FILE`: `CR6Comp`'s preprocessed-source output.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use crate::diagnostics::Diagnostics;
use crate::parser::helpers::{_DIRECTIVE_RE, _split_text_segments, TokStream};
use crate::parser::{Parser, tokenize_for_parser};
use crate::profiles::Profile;
use crate::pyre::{IGNORECASE, Regex};
use crate::pystr;
use crate::sources::{DiskSource, SourceProvider};
use crate::syntax::Token;

// Mirror of private `source.rs` regexes/helpers so `source.rs` stays untouched.
static INCLUDE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"^\s*Include\s*\(?\s*"([^"]*)"\s*\)?\s*$"#, IGNORECASE));
static INCLUDE_SECTION_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"^\s*IncludeSection\s*\(\s*"([^"]*)"\s*,\s*"([^"]*)"\s*(?:,.*)?\)\s*$"#,
        IGNORECASE,
    )
});
static DRIVE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(CPU|USR|CRD|USB):", IGNORECASE));

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

fn is_continuation(line: &str) -> (bool, &str) {
    let t = pystr::rstrip_chars(line, " \t");
    if let Some(rest) = t.strip_suffix('_')
        && (rest.is_empty() || rest.ends_with([' ', '\t']))
    {
        return (true, pystr::rstrip_chars(rest, " \t"));
    }
    (false, line)
}

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

fn resolve_include_path(spec: &str, directory: Option<&Path>) -> Option<PathBuf> {
    let name = DRIVE_RE.sub("", pystr::strip(spec));
    let mut dir = directory?.to_path_buf();
    if name.is_empty() || name.starts_with(['/', '\\']) {
        return None;
    }
    let source = DiskSource;
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
        dir = child_directory(&dir, part, &source)?;
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

/// Port of `preprocess.py::_split_physical`.
fn split_physical(raw: &str) -> Vec<String> {
    let normalized = raw.replace("\r\n", "\n").replace('\r', "\n");
    let mut parts: Vec<String> = normalized.split('\n').map(str::to_owned).collect();
    if parts.last().is_some_and(String::is_empty) {
        parts.pop();
    }
    parts
}

/// Port of `preprocess.py::_is_continuation_raw`.
fn is_continuation_raw(stripped: &str) -> (bool, &str) {
    let t = pystr::rstrip_chars(stripped, " \t");
    if let Some(rest) = t.strip_suffix('_')
        && (rest.is_empty() || rest.ends_with([' ', '\t']))
    {
        return (true, rest);
    }
    (false, stripped)
}

/// Port of `preprocess.py::_Group`.
struct Group {
    raw: String,
    eval: String,
    line: i64,
}

/// Port of `preprocess.py::_read_groups`.
#[allow(clippy::cast_possible_wrap)]
fn read_groups(physical: &[String]) -> Vec<Group> {
    let mut groups = Vec::new();
    let mut i = 0usize;
    let n = physical.len();
    while i < n {
        let mut raw_parts: Vec<String> = Vec::new();
        let mut eval_text = strip_comment(&physical[i]).to_owned();
        loop {
            let (cont, eval_prefix_ref) = is_continuation(&eval_text);
            if !cont || i + 1 >= n {
                raw_parts.push(physical[i].clone());
                if cont {
                    let n = eval_prefix_ref.len();
                    eval_text.truncate(n);
                }
                break;
            }
            let eval_prefix = eval_prefix_ref.to_owned();
            let (_, raw_prefix) = is_continuation_raw(strip_comment(&physical[i]));
            raw_parts.push(raw_prefix.to_owned());
            i += 1;
            let next_stripped = pystr::lstrip_chars(strip_comment(&physical[i]), " \t");
            eval_text = format!("{eval_prefix} {next_stripped}");
        }
        groups.push(Group {
            raw: raw_parts.concat(),
            eval: eval_text,
            line: (i + 1) as i64,
        });
        i += 1;
    }
    groups
}

/// Port of `preprocess.py::_fold_consts_and_check_endprog`.
fn fold_consts_and_check_endprog(
    eval_text: &str,
    line: i64,
    filename: &str,
    parser_instance: &mut Parser,
) -> bool {
    for seg in _split_text_segments(eval_text) {
        if pystr::strip(&seg).is_empty() {
            continue;
        }
        let Ok(tokens) = tokenize_for_parser(&seg, line) else {
            continue;
        };
        let real: Vec<&Token> = tokens.iter().filter(|t| t.kind != "EOF").collect();
        if real.is_empty() || real[0].kind != "IDENT" {
            continue;
        }
        if real[0].upper == "CONST" {
            let mut ts = TokStream::new(tokens, seg);
            let _ = parser_instance._parse_const(&mut ts, line, filename);
        } else if real[0].upper == "ENDPROG" {
            return true;
        }
    }
    false
}

struct StopAt {
    at: (String, i64),
    hit: bool,
}

/// Port of `preprocess.py::_walk_file`.
fn walk_file(
    groups: &[Group],
    filename: &str,
    section: Option<&str>,
    directory: Option<&Path>,
    parser_instance: &mut Parser,
    expand_includes: bool,
    in_include: bool,
    seen: &HashSet<String>,
    mut stop: Option<&mut StopAt>,
) -> Vec<String> {
    let mut out = Vec::new();
    for g in groups {
        if parser_instance.done {
            break;
        }
        parser_instance.env.section = section.map(str::to_owned);
        if let Some(m) = _DIRECTIVE_RE.match_(&g.eval) {
            let mut kind = pystr::upper(m.group(1).unwrap_or(""));
            if kind == "IF_NO_REMOVE" {
                "IF".clone_into(&mut kind);
            }
            let rest = m.group(2).unwrap_or("");
            parser_instance._handle_directive(&kind, rest, g.line, filename);
            continue;
        }
        if !parser_instance._active() {
            continue;
        }
        if let Some(ref mut s) = stop
            && s.at == (pystr::lower(filename), g.line)
        {
            out.push(g.raw.clone());
            s.hit = true;
            parser_instance.done = true;
            break;
        }
        if fold_consts_and_check_endprog(&g.eval, g.line, filename, parser_instance) {
            out.push(g.raw.clone());
            parser_instance.done = true;
            break;
        }
        let m_sec = INCLUDE_SECTION_RE.match_(&g.eval);
        let m_inc = if m_sec.is_some() {
            None
        } else {
            INCLUDE_RE.match_(&g.eval)
        };
        if (m_sec.is_some() || m_inc.is_some()) && !expand_includes {
            let spec = if let Some(ref m) = m_sec {
                m.group(2).unwrap_or("")
            } else if let Some(ref m) = m_inc {
                m.group(1).unwrap_or("")
            } else {
                ""
            };
            out.push(g.raw.clone());
            let Some(target) = resolve_include_path(spec, directory) else {
                parser_instance.done = true;
                break;
            };
            let key = fs::canonicalize(&target)
                .unwrap_or_else(|_| target.clone())
                .to_string_lossy()
                .into_owned();
            if !(seen.contains(&key) || m_inc.is_some() && in_include) {
                let Ok(data) = fs::read(&target) else {
                    continue;
                };
                let child = read_groups(&split_physical(&pystr::decode_latin1(&data)));
                let child_name = target
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                let child_section = m_sec.as_ref().and_then(|m| m.group(1));
                let mut child_seen = seen.clone();
                child_seen.insert(key);
                walk_file(
                    &child,
                    &child_name,
                    child_section,
                    target.parent(),
                    parser_instance,
                    false,
                    in_include || m_inc.is_some(),
                    &child_seen,
                    stop.as_deref_mut(),
                );
                if stop.as_ref().is_some_and(|s| s.hit) {
                    break;
                }
                parser_instance.done = false;
            }
            continue;
        }
        if (m_sec.is_some() || m_inc.is_some()) && expand_includes {
            let (spec, new_section, new_in_include) = if let Some(ref m) = m_sec {
                (m.group(2).unwrap_or(""), m.group(1), in_include)
            } else {
                if in_include {
                    out.push(g.raw.clone());
                    continue;
                }
                let m = m_inc.as_ref().expect("m_sec or m_inc");
                (m.group(1).unwrap_or(""), None, true)
            };
            let Some(target) = resolve_include_path(spec, directory) else {
                out.push(g.raw.clone());
                continue;
            };
            let key = fs::canonicalize(&target)
                .unwrap_or_else(|_| target.clone())
                .to_string_lossy()
                .into_owned();
            if seen.contains(&key) {
                out.push(g.raw.clone());
                continue;
            }
            let Ok(data) = fs::read(&target) else {
                continue;
            };
            let child_groups = read_groups(&split_physical(&pystr::decode_latin1(&data)));
            let child_name = target
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            let mut child_seen = seen.clone();
            child_seen.insert(key);
            out.extend(walk_file(
                &child_groups,
                &child_name,
                new_section,
                target.parent(),
                parser_instance,
                expand_includes,
                new_in_include,
                &child_seen,
                stop.as_deref_mut(),
            ));
            if parser_instance.done {
                break;
            }
            continue;
        }
        out.push(g.raw.clone());
    }
    out
}

/// Port of `preprocess.py::preprocess`.
pub fn preprocess(
    path: &Path,
    profile: &'static Profile,
    expand_includes: bool,
    stop: Option<(&str, i64)>,
) -> std::io::Result<Vec<u8>> {
    let data = fs::read(path)?;
    let groups = read_groups(&split_physical(&pystr::decode_latin1(&data)));
    let mut parser_instance = Parser::new(profile, Diagnostics::new(), true);
    let file_name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let key = fs::canonicalize(path)
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .into_owned();
    let mut seen = HashSet::new();
    seen.insert(key);
    let mut stop_state = stop.map(|(f, l)| StopAt {
        at: (pystr::lower(f), l),
        hit: false,
    });
    let lines = walk_file(
        &groups,
        &file_name,
        None,
        path.parent(),
        &mut parser_instance,
        expand_includes,
        false,
        &seen,
        stop_state.as_mut(),
    );
    let mut text = lines.join("\r\n");
    text.push_str("\r\n");
    Ok(pystr::encode_latin1_replace(&text))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profiles;

    fn with_temp_dir<F: FnOnce(&Path)>(name: &str, f: F) {
        let dir = std::env::temp_dir().join(format!(
            "opencrbasic-preprocess-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create temp dir");
        f(&dir);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn continuation_comments_and_endprog_match_python() {
        with_temp_dir("basic", |dir| {
            let main = dir.join("t.CR6");
            let src = "Const A = 1 ' keep comment\n\nPublic X _ ' drop cont comment\n  As Long\n#If A = 1 Then\nPublic Kept\n#Else\nPublic Dropped\n#EndIf\nEndProg\nPublic AfterEndProg\n";
            fs::write(&main, src.as_bytes()).expect("write main");
            let profile = profiles::get("cr6").expect("cr6 profile");
            let out = preprocess(&main, profile, false, None).expect("preprocess");
            let expected = b"Const A = 1 ' keep comment\r\n\r\nPublic X   As Long\r\nPublic Kept\r\nEndProg\r\n";
            assert_eq!(out, expected);
        });
    }

    #[test]
    fn include_p_vs_i_and_includesection_const_match_python() {
        with_temp_dir("includes", |dir| {
            let lib = dir.join("Lib.crb");
            let lib_src = "#If Section = \"Rail\" Then\nConst FLAG = 1\n#Else\nConst FLAG = 0\n#EndIf\nPublic FromLib\n";
            fs::write(&lib, lib_src.as_bytes()).expect("write lib");

            let main = dir.join("t.CR6");
            let main_src = "IncludeSection(\"Rail\", \"CPU:Lib.crb\")\n#If FLAG = 1 Then\nPublic Yes\n#Else\nPublic No\n#EndIf\nEndProg\n";
            fs::write(&main, main_src.as_bytes()).expect("write main");

            let profile = profiles::get("cr6").expect("cr6 profile");
            let out_p = preprocess(&main, profile, false, None).expect("preprocess -p");
            assert_eq!(
                out_p,
                b"IncludeSection(\"Rail\", \"CPU:Lib.crb\")\r\nPublic Yes\r\nEndProg\r\n"
            );

            let out_i = preprocess(&main, profile, true, None).expect("preprocess -i");
            assert_eq!(
                out_i,
                b"Const FLAG = 1\r\nPublic FromLib\r\nPublic Yes\r\nEndProg\r\n"
            );
        });
    }

    #[test]
    fn missing_include_and_stop_match_python() {
        with_temp_dir("stop", |dir| {
            let main = dir.join("t.CR6");
            let main_src = "Public A\nInclude \"CPU:Missing.crb\"\nPublic AfterMissing\nEndProg\n";
            fs::write(&main, main_src.as_bytes()).expect("write main");

            let profile = profiles::get("cr6").expect("cr6 profile");
            let out_p = preprocess(&main, profile, false, None).expect("preprocess -p");
            assert_eq!(out_p, b"Public A\r\nInclude \"CPU:Missing.crb\"\r\n");

            let out_i = preprocess(&main, profile, true, None).expect("preprocess -i");
            assert_eq!(
                out_i,
                b"Public A\r\nInclude \"CPU:Missing.crb\"\r\nPublic AfterMissing\r\nEndProg\r\n"
            );

            let out_stop =
                preprocess(&main, profile, true, Some(("t.cr6", 3))).expect("preprocess stop");
            assert_eq!(
                out_stop,
                b"Public A\r\nInclude \"CPU:Missing.crb\"\r\nPublic AfterMissing\r\n"
            );
        });
    }
}
