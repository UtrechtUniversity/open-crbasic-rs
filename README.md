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

## Install

```bash
cargo install opencrbasic        # builds the `crbc` binary (Rust 1.85 or newer)
```

Or download a prebuilt `crbc` for Linux, macOS or Windows from the
[GitHub Releases](https://github.com/UtrechtUniversity/open-crbasic-rs/releases).

## Usage

```bash
crbc [options] program [messagefile]
```

| Option | Effect |
| --- | --- |
| `--logger {cr6,cr1000x,cr300,cr350}` | Override the logger inferred from the file extension |
| `--no-message-file` | Write the verdict to stdout only |
| `-v` | Print version information before compiling |
| `-d` | Print version information and exit |
| `-m` | Accepted for compiler compatibility; ignored |
| `--help` | Print usage |

`crbc` prints the verdict on stdout and, unless `--no-message-file` is given, writes it to
`messagefile` (default `<LOGGER>cmple.xxx`, e.g. `CR6cmple.xxx`, in the current directory). The exit
status is 0 when the program compiles and 1 otherwise.

### In VS Code

The [CRBasic extension](https://github.com/UtrechtUniversity/open-crbasic-vsix) builds programs with
this `crbc` when `opencrbasic.compiler` is `rust` (Logger: Build and Show Compiler Output). It runs
`crbc [--logger L] --no-message-file <program>` on the saved file. It looks for `crbc` in Cargo's
bin directory (where `cargo install` puts it), else on `PATH`; set `opencrbasic.crbcPath` when the
Python `crbc` comes first on `PATH`. The two print the same version line. Diagnostics, hover and
the other editor features still come from the Python language server.

## Library

The crate also exposes the compiler as a library; the modules are public, but the interface is the
direct port of the Python functions and may change before 1.0.

```rust
use std::path::Path;
use opencrbasic::{compiler, profiles};

let profile = profiles::get("cr6")?;
let result = compiler::compile_file(
    Path::new("program.cr6"), Some(profile), None, false, None, true, false, false,
);
println!("{}", result.result_lines(false).join("\n"));
assert!(result.ok());
```

## Limitations

Python opencrbasic is the reference specification; where the two differ, Python is right (no
differences are currently known). This port covers only the verdict path (parse, check, stdout,
message file, exit status). Not ported: TDF output (`-z`), table lists (`-l`, `-c`), `-t`, `-s`,
`-u`, `-p`/`-i`, the opencrbasic checks (`--strict`, `--report`), `#Define`/`#Macro` and the
language server. Other loggers than CR6, CR1000X, CR300 and CR350 are not supported.

## Licence and affiliation

MIT, see [LICENSE](LICENSE). This is an independent project and is not affiliated with, endorsed by
or sponsored by Campbell Scientific. CRBasic is their product name.

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

## Releasing

Bump `version` in `Cargo.toml` and push to `main`. When CI passes and no GitHub release for that
version exists, it builds `crbc` for Linux, macOS (arm64, x86_64) and Windows, publishes the crate to
crates.io through trusted publishing (OIDC, environment `crates-io`; no token is stored) and creates
release `v<version>` with those archives and the `.crate`.
