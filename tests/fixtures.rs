//! Verdict golden harness. `tools/pydump.py` records what the Python compiler prints for every
//! verdict fixture case under `target/golden/verdict`; this test replays each case and compares
//! `(exit, stdout)` with the recording. Files listed in `tests/fixture_baseline.txt` must pass.
//!
//! Env: `FIXTURE_ONLY=<substring>` runs only matching fixture files and prints their failing
//! cases; `FIXTURE_WRITE_BASELINE=1` rewrites the baseline with the fully passing files.

use opencrbasic::{compiler, profiles};
use serde_json::Value;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

const MAX_DIFFS: usize = 5;

fn golden_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("target/golden/verdict")
}

fn baseline_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixture_baseline.txt")
}

fn compile_case(dir: &Path, program: &str, logger: &str) -> Result<(i32, String), String> {
    let profile = profiles::get(logger).map_err(|error| error.to_string())?;
    let result = compiler::compile_file(&dir.join(program), Some(profile), None, false, true);
    Ok((
        i32::from(!result.ok()),
        result.result_lines(false).join("\n"),
    ))
}

/// Encodes program and sibling text the way the recording was written.
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

struct Outcome {
    id: String,
    passed: bool,
    expected: (i32, String),
    actual: Option<(i32, String)>,
    error: Option<String>,
    matches_reference: bool,
}

struct FileResult {
    name: String,
    outcomes: Vec<Outcome>,
}

impl FileResult {
    fn passed(&self) -> usize {
        self.outcomes.iter().filter(|o| o.passed).count()
    }
}

fn text_field<'a>(value: &'a Value, key: &str) -> &'a str {
    value.get(key).and_then(Value::as_str).unwrap_or("")
}

fn exit_field(value: &Value, key: &str) -> i32 {
    value
        .get(key)
        .and_then(Value::as_i64)
        .and_then(|n| i32::try_from(n).ok())
        .unwrap_or(i32::MIN)
}

/// Writes the case's program and sibling files into `dir`.
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

/// Runs one case in a fresh temporary directory, which is removed afterwards.
fn run_case(case: &Value, encoding: &str, index: usize, file: &str) -> Outcome {
    let id = case
        .get("id")
        .and_then(Value::as_str)
        .map_or_else(|| format!("{file}-{index}"), str::to_owned);
    let expected = (
        exit_field(case, "exit"),
        text_field(case, "stdout").to_owned(),
    );
    let reference = (
        exit_field(case, "reference_exit"),
        text_field(case, "reference_stdout").to_owned(),
    );
    let matches_reference = expected == reference;
    let dir = std::env::temp_dir().join(format!(
        "opencrbasic-fixture-{}-{file}-{index}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    let (actual, error) = match write_case(&dir, case, encoding).and_then(|()| {
        compile_case(
            &dir,
            text_field(case, "program_filename"),
            text_field(case, "logger"),
        )
    }) {
        Ok(verdict) => (Some(verdict), None),
        Err(e) => (None, Some(e)),
    };
    let _ = fs::remove_dir_all(&dir);
    let passed = error.is_none() && actual.as_ref() == Some(&expected);
    Outcome {
        id,
        passed,
        expected,
        actual,
        error,
        matches_reference,
    }
}

fn run_file(path: &Path) -> FileResult {
    let name = path
        .file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
    let text = fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let value: Value =
        serde_json::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let encoding = text_field(&value, "encoding").to_owned();
    let cases = value
        .get("cases")
        .and_then(Value::as_array)
        .map_or(&[][..], Vec::as_slice);
    let outcomes = cases
        .iter()
        .enumerate()
        .map(|(index, case)| run_case(case, &encoding, index, &name))
        .collect();
    FileResult { name, outcomes }
}

/// Line-by-line diff of expected and actual stdout, plus the exit codes.
fn describe(outcome: &Outcome) -> String {
    let mut out = format!("  case {}\n", outcome.id);
    if let Some(error) = &outcome.error {
        let _ = writeln!(out, "    setup failed: {error}");
        return out;
    }
    let Some((actual_exit, actual_stdout)) = &outcome.actual else {
        out += "    no compiler verdict\n";
        return out;
    };
    let (exit, stdout) = &outcome.expected;
    if exit != actual_exit {
        let _ = writeln!(out, "    exit: expected {exit}, actual {actual_exit}");
    }
    let expected_lines: Vec<&str> = stdout.lines().collect();
    let actual_lines: Vec<&str> = actual_stdout.lines().collect();
    for i in 0..expected_lines.len().max(actual_lines.len()) {
        let e = expected_lines.get(i).copied();
        let a = actual_lines.get(i).copied();
        if e != a {
            let _ = writeln!(
                out,
                "    line {}:\n      -{}\n      +{}",
                i + 1,
                e.unwrap_or(""),
                a.unwrap_or("")
            );
        }
    }
    out
}

fn read_baseline() -> Vec<String> {
    fs::read_to_string(baseline_path())
        .unwrap_or_default()
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(str::to_owned)
        .collect()
}

#[test]
fn verdict_fixtures() {
    let dir = golden_dir();
    if !dir.is_dir() {
        println!(
            "note: no golden files at {}; run tools/pydump.py to record them",
            dir.display()
        );
        return;
    }
    let only = std::env::var("FIXTURE_ONLY").ok();
    let write_baseline = std::env::var("FIXTURE_WRITE_BASELINE").is_ok_and(|v| v == "1");

    let mut paths: Vec<PathBuf> = fs::read_dir(&dir)
        .expect("read golden directory")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .filter(|p| {
            only.as_ref()
                .is_none_or(|needle| p.to_string_lossy().contains(needle.as_str()))
        })
        .collect();
    paths.sort();

    // The fixture runner is its own test process. Avoid capturing a panic report for each
    // unfinished port function the checker catches across the corpus.
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let unfinished = info
            .payload()
            .downcast_ref::<String>()
            .map(String::as_str)
            .or_else(|| info.payload().downcast_ref::<&str>().copied())
            .is_some_and(|message| message.starts_with("not yet implemented: "));
        if !unfinished {
            default_hook(info);
        }
    }));

    let next = AtomicUsize::new(0);
    let results = Mutex::new(Vec::new());
    let threads = std::thread::available_parallelism().map_or(1, std::num::NonZero::get);
    std::thread::scope(|scope| {
        for _ in 0..threads {
            scope.spawn(|| {
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    let Some(path) = paths.get(i) else { break };
                    let result = run_file(path);
                    results.lock().expect("results lock").push(result);
                }
            });
        }
    });
    let mut results = results.into_inner().expect("results lock");
    results.sort_by(|a, b| a.name.cmp(&b.name));

    let baseline = read_baseline();
    let mut failures = String::new();
    let mut fully_passing = Vec::new();
    let (mut cases_passing, mut cases_total, mut cases_reference) = (0, 0, 0);
    for result in &results {
        let total = result.outcomes.len();
        let passed = result.passed();
        cases_passing += passed;
        cases_total += total;
        cases_reference += result
            .outcomes
            .iter()
            .filter(|o| o.matches_reference)
            .count();
        let fully = passed == total;
        if fully {
            fully_passing.push(result.name.clone());
        } else if passed > 0 {
            println!("{} {passed}/{total}", result.name);
        }
        let listed = baseline.contains(&result.name);
        if !fully && (listed || only.is_some()) {
            let mut shown = format!("{} {passed}/{total}\n", result.name);
            for outcome in result.outcomes.iter().filter(|o| !o.passed).take(MAX_DIFFS) {
                shown += &describe(outcome);
            }
            if listed && only.is_none() && !write_baseline {
                failures += &shown;
            } else {
                print!("{shown}");
            }
        }
    }
    for name in &baseline {
        if !results.iter().any(|r| &r.name == name) && only.is_none() {
            let _ = writeln!(
                failures,
                "{name}: listed in the baseline but has no golden file"
            );
        }
    }

    println!(
        "fixture files fully passing: {}/{}\ncases passing: {cases_passing}/{cases_total}\n\
         cases equal to reference stdout and exit (python recording): {cases_reference}/{cases_total}",
        fully_passing.len(),
        results.len()
    );

    if write_baseline {
        let mut text = String::from(
            "# Fixture files that must fully pass (tests/fixtures.rs). Regenerate with\n\
             # FIXTURE_WRITE_BASELINE=1 cargo test --test fixtures after tools/pydump.py.\n",
        );
        for name in &fully_passing {
            text += name;
            text.push('\n');
        }
        fs::write(baseline_path(), text).expect("write baseline");
        println!("baseline written: {} files", fully_passing.len());
        return;
    }
    assert!(
        failures.is_empty(),
        "baseline fixture files are not passing:\n{failures}"
    );
}
