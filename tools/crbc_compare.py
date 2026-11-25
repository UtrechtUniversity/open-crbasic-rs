"""Compare the Rust `crbc` with the Python `crbc` on the station and fuzz corpora.

For every station program under each of the four loggers, and every fuzz reproducer under its
recorded logger, both compilers run in a copy of the station directory (so `Include` siblings
resolve). Stdout, exit status and the message-file bytes must be equal.

    cargo build --release
    python3 tools/crbc_compare.py            # exit 1 and a list when anything differs

The Python checkout is `../open-crbasic` or `$OPENCRBASIC_PY`; its `.venv/bin/crbc`
(`.venv/Scripts/crbc.exe` on Windows) is the oracle. Stdout is compared with CRLF read as LF: Python
writes text-mode stdout, which is CRLF on Windows.
"""

import json
import os
import shutil
import subprocess
import sys
import tempfile
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
PY = Path(os.environ.get("OPENCRBASIC_PY", ROOT.parent / "open-crbasic")).resolve()
EXE = ".exe" if os.name == "nt" else ""
RUST = ROOT / "target" / "release" / f"crbc{EXE}"
ORACLE = PY / ".venv" / ("Scripts" if os.name == "nt" else "bin") / f"crbc{EXE}"
STATION = PY / "tests" / "corpus" / "station"
LOGGERS = ("cr6", "cr1000x", "cr300", "cr350")


def run(exe: Path, program: Path, logger: str) -> tuple[int, bytes, bytes]:
    """Exit status, stdout and message-file bytes of one compile in a fresh station copy."""
    with tempfile.TemporaryDirectory() as tmp:
        work = Path(tmp) / "w"
        shutil.copytree(STATION, work)
        shutil.copy(program, work / program.name)
        done = subprocess.run(
            [exe, "--logger", logger, program.name, "msg.xxx"], cwd=work, capture_output=True, timeout=300
        )
        message = work / "msg.xxx"
        stdout = done.stdout.replace(b"\r\n", b"\n")
        return done.returncode, stdout, message.read_bytes() if message.exists() else b"<none>"


def cases() -> list[tuple[Path, str]]:
    out = [
        (p, logger)
        for p in sorted(STATION.iterdir())
        if p.suffix.upper() in (".CR6", ".CR1X")
        for logger in LOGGERS
    ]
    for meta in sorted((PY / "tests" / "corpus" / "fuzz").rglob("*.json")):
        info = json.loads(meta.read_text())
        # (the recorded `file` can be a stale absolute path; the program sits next to its metadata)
        out.append((meta.with_name(Path(info["file"]).name), info["logger"]))
    return out


def compare(case: tuple[Path, str]) -> str | None:
    program, logger = case
    ours, theirs = run(RUST, program, logger), run(ORACLE, program, logger)
    parts = [name for name, a, b in zip(("exit", "stdout", "message file"), ours, theirs) if a != b]
    return f"{program.relative_to(PY)} [{logger}]: {', '.join(parts)} differ" if parts else None


def main() -> int:
    for exe in (RUST, ORACLE):
        if not exe.exists():
            sys.exit(f"missing {exe}")
    todo = cases()
    with ThreadPoolExecutor(os.cpu_count()) as pool:
        failures = [f for f in pool.map(compare, todo) if f]
    print("\n".join(failures))
    print(f"{len(todo) - len(failures)} of {len(todo)} runs equal")
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
