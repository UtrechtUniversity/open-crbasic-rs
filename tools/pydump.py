"""Record what the Python opencrbasic compiler outputs for every verdict fixture case.

Writes one JSON file per fixture to target/golden/verdict/ under this repository. The Rust
harness (tests/fixtures.rs) replays the recorded program files and compares its own output.

Run from this repository with:
    OPENCRBASIC_PY=<checkout> uv run --project <checkout> python tools/pydump.py
"""

import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import time
from collections import Counter
from multiprocessing import Pool
from pathlib import Path

ENCODING = "utf-8"
DEFAULT_PROGRAM = "t.CR6"
REPO = Path(__file__).resolve().parent.parent
OUT_DIR = REPO / "target" / "golden" / "verdict"
# Keys that make a case a non-verdict observation (its expected output is not a compile verdict).
NON_VERDICT_KEYS = {"expected_tdf", "expected_times", "expected_sizes", "expected_files", "args", "lst", "tdf"}
REQUIRED_KEYS = {"source", "logger", "expected_stdout", "expected_exit"}
# Keys that change how the test consuming them sets up the program (see the notes in the task).
# `name`, `batch`, `body`, `code`, `not_modelled` and the expected_* extras do not change the setup
# of their consumers (the consuming tests only read the verdict, or record the case as xfail).
HANDLED_KEYS = {
    "id",
    "siblings",  # tests/test_include_validation.py: written next to the program, utf-8
    "files",  # tests/test_r66_include_stop.py: written next to the program, utf-8
    "include",  # tests/test_source_buffer.py: written as Lib.crb next to the program
    "program_filename",  # tests/test_serialin_destination.py and the name-collision tests
    "name",  # display only, no consumer reads it for the verdict
    "batch",  # no consumer reads it; the verdict uses program_filename only
    "body",  # tests/test_checks_m3.py: test ids only
    "code",  # tests/test_checks_m3.py: checks.run expectation, verdict unaffected
    "not_modelled",  # tests/test_const_ranges.py: xfail marker; the actual output is still recorded
    "expected_interval_lines",  # tests/test_interval_units.py: extra assertions only
    "expected_test_records",  # tests/test_interval_units.py: extra assertions only
}


def checkout_path() -> Path:
    env = os.environ.get("OPENCRBASIC_PY")
    return Path(env).resolve() if env else (REPO.parent / "open-crbasic").resolve()


def pinned_commit() -> str:
    for line in (REPO / "PORT_PROGRESS.md").read_text(encoding="utf-8").splitlines():
        if line.startswith("Spec:"):
            match = re.search(r"`([0-9a-f]{7,40})`", line)
            if match:
                return match.group(1)
    sys.exit("PORT_PROGRESS.md: no 'Spec:' line with a pinned commit")


def check_checkout(checkout: Path) -> None:
    pinned = pinned_commit()
    result = subprocess.run(
        ["git", "-C", str(checkout), "rev-parse", "--short", "HEAD"],
        capture_output=True,
        text=True,
        check=False,
    )
    if result.returncode != 0:
        sys.exit(f"{checkout} is not a git checkout: {result.stderr.strip()}")
    head = result.stdout.strip()
    if head != pinned:
        sys.exit(f"{checkout} is at {head}, but PORT_PROGRESS.md pins the spec at {pinned}")


def skip_reason(case: dict) -> str | None:
    for key in case:
        if key not in HANDLED_KEYS and key not in REQUIRED_KEYS:
            return f"unhandled key {key}"
    return None


def unsafe_name(name: str) -> bool:
    path = Path(name)
    return path.is_absolute() or ".." in path.parts or not name


def run_compile(program_name: str, source: str, files: dict, logger: str) -> tuple[int, str, bool]:
    """Returns (exit, stdout, raised). Internal errors are the compiler's output (OPENCRBASIC_STRICT unset)."""
    from opencrbasic import profiles
    from opencrbasic.compiler import compile_file

    with tempfile.TemporaryDirectory(prefix="pydump-") as tmp:
        root = Path(tmp).resolve()
        for name, text in files.items():
            path = root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(text.encode(ENCODING))
        program = root / program_name
        program.write_bytes(source.encode(ENCODING))
        try:
            result = compile_file(program, profiles.get(logger))
        except Exception as exc:  # the case is recorded as the failure it produced
            return 1, f"<pydump exception: {type(exc).__name__}: {exc}>", True
        return (0 if result.ok else 1), "\n".join(result.result_lines()), False


def dump_fixture(path_text: str) -> dict:
    path = Path(path_text)
    data = json.loads(path.read_text(encoding="utf-8"))
    summary = {"fixture": path.name, "written": False, "recorded": 0, "equal": 0, "exceptions": 0,
               "skipped": Counter(), "bare_list": False}
    if isinstance(data, list):  # tdf_rules_cases.json: a bare list of non-verdict cases
        summary["bare_list"] = True
        return _finish(summary)

    cases_out = []
    for index, case in enumerate(data.get("cases", [])):
        if not (REQUIRED_KEYS <= case.keys()) or NON_VERDICT_KEYS & case.keys():
            continue
        reason = skip_reason(case)
        program_name = case.get("program_filename", DEFAULT_PROGRAM)
        files: dict[str, str] = {}
        files.update(case.get("files") or {})
        files.update(case.get("siblings") or {})
        if "include" in case:
            files["Lib.crb"] = case["include"]
        if reason is None and (unsafe_name(program_name) or any(unsafe_name(n) for n in files)):
            reason = "unsafe file name"
        if reason is not None:
            summary["skipped"][reason] += 1
            continue
        exit_code, stdout, raised = run_compile(program_name, case["source"], files, case["logger"])
        if raised:
            summary["exceptions"] += 1
        reference_stdout, reference_exit = case["expected_stdout"], case["expected_exit"]
        summary["recorded"] += 1
        if stdout == reference_stdout and exit_code == reference_exit:
            summary["equal"] += 1
        cases_out.append({
            "id": case.get("id", f"{path.stem}-{index}"),
            "logger": case["logger"],
            "program_filename": program_name,
            "source": case["source"],
            "files": files,
            "exit": exit_code,
            "stdout": stdout,
            "reference_stdout": reference_stdout,
            "reference_exit": reference_exit,
        })

    if not cases_out:
        return _finish(summary)
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    out = {"fixture": path.name, "encoding": ENCODING, "cases": cases_out}
    (OUT_DIR / path.name).write_text(json.dumps(out, ensure_ascii=False), encoding="utf-8")
    summary["written"] = True
    return _finish(summary)


def _finish(summary: dict) -> dict:
    summary["skipped"] = dict(summary["skipped"])
    return summary


def main() -> None:
    os.environ.pop("OPENCRBASIC_STRICT", None)
    checkout = checkout_path()
    check_checkout(checkout)
    if OUT_DIR.exists():
        shutil.rmtree(OUT_DIR)
    fixtures = sorted(str(p) for p in (checkout / "tests" / "fixtures").glob("*.json"))

    start = time.monotonic()
    totals = Counter()
    skipped: Counter = Counter()
    with Pool(processes=os.cpu_count()) as pool:
        for summary in pool.imap_unordered(dump_fixture, fixtures, chunksize=1):
            totals["bare_list"] += summary["bare_list"]
            totals["written"] += summary["written"]
            totals["recorded"] += summary["recorded"]
            totals["equal"] += summary["equal"]
            totals["exceptions"] += summary["exceptions"]
            skipped.update(summary["skipped"])
    elapsed = time.monotonic() - start

    print(f"checkout: {checkout} at {pinned_commit()}")
    print(f"fixtures scanned: {len(fixtures)}, bare-list files ignored: {totals['bare_list']}")
    print(f"fixtures written: {totals['written']} (files with no verdict case are not written)")
    print(f"cases recorded: {totals['recorded']}")
    print(f"cases skipped: {sum(skipped.values())}")
    for reason, count in sorted(skipped.items()):
        print(f"  {count} skipped: {reason}")
    print(f"recorded cases equal to reference stdout and exit: {totals['equal']}")
    print(f"python internal exceptions recorded as output: {totals['exceptions']}")
    print(f"wall time: {elapsed:.1f}s")


if __name__ == "__main__":
    main()
