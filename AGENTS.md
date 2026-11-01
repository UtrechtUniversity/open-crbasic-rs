# Repository instructions

A Rust implementation of opencrbasic, the open re-implementation of Campbell Scientific's CRBasic
compilers. The Python implementation in
[open-crbasic](https://github.com/UtrechtUniversity/open-crbasic) is the specification: its `docs/`
record the reference compilers' behaviour with evidence, and its tests and fixtures are the
expected results. Match it; where it records a known gap, match the reference compiler instead and
say so.

Priorities: correctness, evidence, simplicity, minimal scope, maintainability.

## Rules

- Never commit Campbell's compilers, help files or example programs, or test inputs derived
  mostly from them (open-crbasic `docs/legal.md`).
- Never report a compile or test result that was not observed.
- No `unsafe` (`Cargo.toml` forbids it). Clippy warnings fail the build.

## Checks

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

## Version control

Never create a merge commit; integrate with rebase. Commit only when asked. Do not use destructive
reset or checkout commands without explicit approval.
