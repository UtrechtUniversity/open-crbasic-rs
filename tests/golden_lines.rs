//! Logical-line golden harness. `tools/pydump.py` records what `source.py` produces for every
//! fixture source, station, and fuzz program under `target/golden/lines`; this test replays each
//! case with `opencrbasic::source::load_program` and asserts 100% equality.

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use opencrbasic::source::load_program;
use serde_json::Value;

const MAX_DIFFS: usize = 5;

fn golden_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("target/golden/lines")
}

fn collect_json(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_json(&path, out);
        } else if path.extension().is_some_and(|x| x == "json") {
            out.push(path);
        }
    }
}

fn encode(text: &str, encoding: &str) -> Result<Vec<u8>, String> {
    match encoding {
        "utf-8" => Ok(text.as_bytes().to_vec()),
        "latin-1" => text
            .chars()
            .map(|c| u8::try_from(u32::from(c)).map_err(|_| format!("{c:?} is not Latin-1")))
            .collect(),
        other => Err(format!("unsupported encoding {other}")),
    }
}

fn text_field<'a>(value: &'a Value, key: &str) -> &'a str {
    value.get(key).and_then(Value::as_str).unwrap_or("")
}

fn write_case(dir: &Path, case: &Value, encoding: &str) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let files = case.get("files").and_then(Value::as_object);
    for (name, text) in files.into_iter().flatten() {
        let path = dir.join(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let bytes = encode(text.as_str().unwrap_or(""), encoding)?;
        fs::write(path, bytes).map_err(|e| e.to_string())?;
    }
    let bytes = encode(text_field(case, "source"), encoding)?;
    fs::write(dir.join(text_field(case, "program_filename")), bytes).map_err(|e| e.to_string())
}

fn run_case(
    case: &Value,
    encoding: &str,
    file_index: usize,
    case_index: usize,
    file: &str,
) -> Result<(), String> {
    let id = case
        .get("id")
        .and_then(Value::as_str)
        .map_or_else(|| format!("{file}-{case_index}"), str::to_owned);
    let expected = case
        .get("lines")
        .ok_or_else(|| format!("{id}: missing lines"))?;
    let dir = std::env::temp_dir().join(format!(
        "opencrbasic-lines-{}-{file_index}-{case_index}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    let setup = write_case(&dir, case, encoding);
    if let Err(e) = setup {
        let _ = fs::remove_dir_all(&dir);
        return Err(format!("{id}: setup failed: {e}"));
    }
    let program = dir.join(text_field(case, "program_filename"));
    let logger = text_field(case, "logger");
    let own_ext = match logger {
        "cr6" => Some("CR6"),
        "cr1000x" => Some("CR1X"),
        _ => None,
    };
    let pull_init = matches!(logger, "cr6" | "cr1000x" | "cr300" | "cr350");
    let cr3xx_exts: Option<&[&str]> = match logger {
        "cr300" => Some(&["CR300", "CR310"]),
        "cr350" => Some(&["CR350"]),
        _ => None,
    };
    let (lines, _) = load_program(
        &program, own_ext, pull_init, None, cr3xx_exts, None, true, false,
    );
    let _ = fs::remove_dir_all(&dir);

    let actual = serde_json::to_value(&lines).map_err(|e| format!("{id}: serialize: {e}"))?;
    if &actual == expected {
        Ok(())
    } else {
        Err(format!(
            "{id}: mismatch\n  expected: {expected}\n  actual:   {actual}"
        ))
    }
}

#[test]
fn golden_lines() {
    let dir = golden_dir();
    if !dir.is_dir() {
        println!(
            "note: no golden files at {}; run tools/pydump.py to record them",
            dir.display()
        );
        return;
    }

    let mut paths = Vec::new();
    collect_json(&dir, &mut paths);
    paths.sort();

    let next = AtomicUsize::new(0);
    let totals = Mutex::new((0usize, 0usize, Vec::<String>::new()));
    let threads = std::thread::available_parallelism().map_or(1, std::num::NonZero::get);
    std::thread::scope(|scope| {
        for _ in 0..threads {
            scope.spawn(|| {
                loop {
                    let file_index = next.fetch_add(1, Ordering::Relaxed);
                    let Some(path) = paths.get(file_index) else {
                        break;
                    };
                    let name = path
                        .strip_prefix(&dir)
                        .unwrap_or(path)
                        .to_string_lossy()
                        .into_owned();
                    let text = fs::read_to_string(path)
                        .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
                    let value: Value = serde_json::from_str(&text)
                        .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
                    let encoding = text_field(&value, "encoding").to_owned();
                    let cases = value
                        .get("cases")
                        .and_then(Value::as_array)
                        .map_or(&[][..], Vec::as_slice);
                    let mut passed = 0usize;
                    let mut file_diffs = Vec::new();
                    for (case_index, case) in cases.iter().enumerate() {
                        match run_case(case, &encoding, file_index, case_index, &name) {
                            Ok(()) => passed += 1,
                            Err(msg) => {
                                if file_diffs.len() < MAX_DIFFS {
                                    file_diffs.push(format!("{name}: {msg}"));
                                }
                            }
                        }
                    }
                    let mut guard = totals.lock().expect("totals lock");
                    guard.0 += passed;
                    guard.1 += cases.len();
                    for d in file_diffs {
                        if guard.2.len() < MAX_DIFFS {
                            guard.2.push(d);
                        }
                    }
                }
            });
        }
    });

    let (passed, total, diffs) = totals.into_inner().expect("totals lock");
    println!("golden lines passing: {passed}/{total}");
    if !diffs.is_empty() {
        let mut msg = String::new();
        for d in diffs {
            let _ = writeln!(msg, "{d}");
        }
        panic!("golden_lines mismatches ({passed}/{total} passed):\n{msg}");
    }
    assert_eq!(passed, total);
}
