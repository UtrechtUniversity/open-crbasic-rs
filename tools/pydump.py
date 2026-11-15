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
LINES_DIR = REPO / "target" / "golden" / "lines"
INCLUDE_OWN_EXT = {"cr6": "CR6", "cr1000x": "CR1X"}
CR3XX_INCLUDE_EXTS = {"cr300": ("CR300", "CR310"), "cr350": ("CR350",)}
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


def serialize_piece(piece) -> dict:
    return {
        "start": piece.start,
        "end": piece.end,
        "file": piece.file,
        "line": piece.line,
        "col": piece.col,
        "occurrence": piece.occurrence,
    }


def serialize_line(ll) -> dict:
    return {
        "text": ll.text,
        "line": ll.line,
        "file": ll.file,
        "section": ll.section,
        "had_trailing_comment": ll.had_trailing_comment,
        "kind": ll.kind,
        "pulls": ll.pulls,
        "unpulled": [serialize_line(u) for u in ll.unpulled] if ll.unpulled is not None else None,
        "bad_init": list(ll.bad_init) if ll.bad_init is not None else None,
        "pieces": [serialize_piece(p) for p in ll.pieces],
        "phys": list(ll.phys) if ll.phys is not None else None,
        "occurrence": ll.occurrence,
        "stops_includes": ll.stops_includes,
    }


def run_lines(program_name: str, source_text: str, files: dict[str, str], logger: str, encoding: str) -> list[dict]:
    from opencrbasic import source as src_mod

    with tempfile.TemporaryDirectory(prefix="pydump-lines-") as tmp:
        root = Path(tmp).resolve()
        for name, text in files.items():
            path = root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(text.encode(encoding))
        program = root / program_name
        program.write_bytes(source_text.encode(encoding))
        own_ext = INCLUDE_OWN_EXT.get(logger)
        pull_init = logger in ("cr6", "cr1000x", "cr300", "cr350")
        cr3xx_exts = CR3XX_INCLUDE_EXTS.get(logger)
        lines, _ = src_mod.load_program(program, own_ext, pull_init, None, cr3xx_exts, None, True, False)
        return [serialize_line(ll) for ll in lines]


def _case_files(case: dict) -> tuple[str, dict[str, str]]:
    program_name = case.get("program_filename", DEFAULT_PROGRAM)
    files: dict[str, str] = {}
    files.update(case.get("files") or {})
    files.update(case.get("siblings") or {})
    if "include" in case:
        files["Lib.crb"] = case["include"]
    return program_name, files


def dump_fixture(path_text: str) -> dict:
    path = Path(path_text)
    data = json.loads(path.read_text(encoding="utf-8"))
    summary = {"fixture": path.name, "written": False, "recorded": 0, "equal": 0, "exceptions": 0,
               "lines_recorded": 0, "skipped": Counter(), "bare_list": False}
    if isinstance(data, list):  # tdf_rules_cases.json: a bare list of non-verdict cases
        summary["bare_list"] = True
        return _finish(summary)

    cases_out = []
    lines_out = []
    for index, case in enumerate(data.get("cases", [])):
        if "source" in case and "logger" in case:
            program_name, files = _case_files(case)
            if not (unsafe_name(program_name) or any(unsafe_name(n) for n in files)):
                lines_out.append({
                    "id": case.get("id", f"{path.stem}-{index}"),
                    "logger": case["logger"],
                    "program_filename": program_name,
                    "source": case["source"],
                    "files": files,
                    "lines": run_lines(program_name, case["source"], files, case["logger"], ENCODING),
                })
                summary["lines_recorded"] += 1

        if not (REQUIRED_KEYS <= case.keys()) or NON_VERDICT_KEYS & case.keys():
            continue
        reason = skip_reason(case)
        program_name, files = _case_files(case)
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

    if lines_out:
        fixtures_lines_dir = LINES_DIR / "fixtures"
        fixtures_lines_dir.mkdir(parents=True, exist_ok=True)
        out_lines = {"fixture": path.name, "encoding": ENCODING, "cases": lines_out}
        (fixtures_lines_dir / path.name).write_text(json.dumps(out_lines, ensure_ascii=False), encoding="utf-8")

    if not cases_out:
        return _finish(summary)
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    out = {"fixture": path.name, "encoding": ENCODING, "cases": cases_out}
    (OUT_DIR / path.name).write_text(json.dumps(out, ensure_ascii=False), encoding="utf-8")
    summary["written"] = True
    return _finish(summary)


def dump_corpus_lines(checkout: Path) -> tuple[int, int]:
    """Dump logical lines for station and fuzz corpus programs into target/golden/lines."""
    LINES_DIR.mkdir(parents=True, exist_ok=True)
    station_dir = checkout / "tests" / "corpus" / "station"
    station_cases = []
    if station_dir.is_dir():
        siblings = {
            p.name: p.read_bytes().decode("latin-1")
            for p in sorted(station_dir.iterdir())
            if p.is_file() and p.suffix.lower() == ".crb"
        }
        for p in sorted(station_dir.iterdir()):
            if not p.is_file() or p.suffix.upper() not in (".CR6", ".CR1X"):
                continue
            logger = "cr1000x" if p.suffix.upper() == ".CR1X" else "cr6"
            source_text = p.read_bytes().decode("latin-1")
            station_cases.append({
                "id": p.name,
                "logger": logger,
                "program_filename": p.name,
                "source": source_text,
                "files": siblings,
                "lines": run_lines(p.name, source_text, siblings, logger, "latin-1"),
            })
    if station_cases:
        out = {"fixture": "station.json", "encoding": "latin-1", "cases": station_cases}
        (LINES_DIR / "station.json").write_text(json.dumps(out, ensure_ascii=False), encoding="utf-8")

    fuzz_dir = checkout / "tests" / "corpus" / "fuzz"
    fuzz_cases = []
    if fuzz_dir.is_dir():
        for logger_dir in sorted(fuzz_dir.iterdir()):
            if not logger_dir.is_dir():
                continue
            logger = logger_dir.name
            for p in sorted(logger_dir.rglob("*.CR6")):
                rel = p.relative_to(fuzz_dir).as_posix()
                source_text = p.read_bytes().decode("latin-1")
                fuzz_cases.append({
                    "id": rel,
                    "logger": logger,
                    "program_filename": p.name,
                    "source": source_text,
                    "files": {},
                    "lines": run_lines(p.name, source_text, {}, logger, "latin-1"),
                })
    if fuzz_cases:
        out = {"fixture": "fuzz.json", "encoding": "latin-1", "cases": fuzz_cases}
        (LINES_DIR / "fuzz.json").write_text(json.dumps(out, ensure_ascii=False), encoding="utf-8")
    return len(station_cases), len(fuzz_cases)


def _finish(summary: dict) -> dict:
    summary["skipped"] = dict(summary["skipped"])
    return summary


def main() -> None:
    os.environ.pop("OPENCRBASIC_STRICT", None)
    checkout = checkout_path()
    check_checkout(checkout)
    if OUT_DIR.exists():
        shutil.rmtree(OUT_DIR)
    if LINES_DIR.exists():
        shutil.rmtree(LINES_DIR)
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
            totals["lines_recorded"] += summary["lines_recorded"]
            skipped.update(summary["skipped"])
    station_count, fuzz_count = dump_corpus_lines(checkout)
    elapsed = time.monotonic() - start

    print(f"checkout: {checkout} at {pinned_commit()}")
    print(f"fixtures scanned: {len(fixtures)}, bare-list files ignored: {totals['bare_list']}")
    print(f"fixtures written: {totals['written']} (files with no verdict case are not written)")
    print(f"cases recorded: {totals['recorded']}")
    print(f"lines recorded: {totals['lines_recorded']} fixture, {station_count} station, {fuzz_count} fuzz")
    print(f"cases skipped: {sum(skipped.values())}")
    for reason, count in sorted(skipped.items()):
        print(f"  {count} skipped: {reason}")
    print(f"recorded cases equal to reference stdout and exit: {totals['equal']}")
    print(f"python internal exceptions recorded as output: {totals['exceptions']}")
    print(f"wall time: {elapsed:.1f}s")


if __name__ == "__main__":
    main()
