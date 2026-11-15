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

## Porting rules

The port is a faithful transliteration of open-crbasic at the commit pinned in
`PORT_PROGRESS.md`. State and task list: `PORT_PROGRESS.md`. Cargo is at
`/opt/homebrew/opt/rustup/bin` (add it to `PATH`).

- One Python function becomes one Rust function with the same name, in the same order, in the
  mirrored file (`src/opencrbasic/checker/structure.py` -> `src/checker/structure*.rs`). Its doc
  comment is one line: `/// Port of checker/structure.py::check_pass1`. Do not copy the Python
  evidence comments; the Python file is their home.
- AST: `enum Expr`, `enum Stmt` with a shared metadata struct; `isinstance` becomes `match`. Every
  ad-hoc Python attribute (`getattr(x, "name", default)`) is an explicit field in `syntax.rs`.
  Do not add fields; report the need.
- Identity: statements and the program are `Rc<..>`. Python `id(x)` is `Rc::as_ptr`, `a is b` is
  `Rc::ptr_eq`, `dataclasses.replace` builds a new `Rc`. Never key a cache on structural equality
  where Python keys on `id()`.
- Ambient state: Python `ContextVar`s are `thread_local!` with RAII guards; `cached_property` is a
  `OnceCell` field.
- Text is Latin-1: one source byte is one `char`. Never byte-slice a `String`; use `pystr`
  (Python slicing with clamping and negative indices, `char_at`, `upper`, `strip`, ...).
- Regex only through `pyre` (`match_`, `search`, `fullmatch`, `sub`, `finditer`, `split`, with
  Python anchoring). Copy pattern strings verbatim; compile once in a `LazyLock`.
- Numbers: Python `int` is `i64`, `float` is `f64`. `int()`, `round()`, `//`, `%` and float
  formatting go through `pystr` helpers, never a bare `as` cast or `{}`.
- A Python `except` that swallows becomes an explicit `Option`/`Result` path. Indexing that can
  raise in Python and is caught must not panic uncaught in Rust.
- Python dicts and sets iterate in insertion order and output depends on it: keep insertion
  order (a `Vec` of pairs, or a map plus an order `Vec`), never `HashMap` iteration order.
- Match diagnostics by `kind` where Python does, by message text where Python still does.
- Never edit a file outside your task's list. Report a needed signature or type change; do not
  make it. Never report a test or gate result you did not observe.
