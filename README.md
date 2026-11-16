# open-crbasic-rs

A Rust implementation of [opencrbasic](https://github.com/UtrechtUniversity/open-crbasic), the open
re-implementation of Campbell Scientific's CRBasic compilers. MIT licensed; not affiliated with
Campbell Scientific.

## Status

The crate ports the reference verdict of opencrbasic 1.2.1 (commit `c547b85`) for CR6, CR1000X,
CR300 and CR350: parse and check, stdout, the message file (`<LOGGER>cmple.xxx`) and exit status.
Measured on 2026-11-16 against that Python commit:

- all 77,293 verdict fixture cases (613 files) give Python's stdout and exit status;
- logical lines and ASTs are equal on all 77,903 golden programs;
- `crbc` gives Python's stdout, exit status and message-file bytes on the station programs under
  all four loggers and on the fuzz reproducers (305 runs, `tools/crbc_compare.py`).

Not ported: TDF output (`-z`), table lists (`-l`, `-c`), `-t`, `-s`, `-u`, `-p`/`-i`, the
opencrbasic checks (`--strict`, `--report`), `#Define`/`#Macro` and the language server.

```bash
cargo run --release --bin crbc -- [--logger cr6|cr1000x|cr300|cr350] program [messagefile]
```

## Development

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
uv run --project ../open-crbasic python tools/pydump.py   # goldens from the Python checkout
cargo test --release                                       # the fixture replay is slow in debug
cargo build --release && python3 tools/crbc_compare.py     # CLI bytes against the Python crbc
uvx pre-commit install   # the same checks before each commit
```

CI (`.github/workflows/ci.yml`) runs them on every push and pull request. Agent and contributor
conventions are in [AGENTS.md](AGENTS.md).
