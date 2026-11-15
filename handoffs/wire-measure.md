# wire-measure

Complete. No commits or Git state-changing commands. All work is in this worktree.

## Wired
- Read AGENTS.md porting rules, every handoffs/wire-*.md, PORT_PROGRESS.md P4, module headers/TODOs and Python implementations/call sites.
- Merged 49 incomplete/full API pairs (28 part1, 17 part2, 4 part3, including three convenience wrappers). All 54 Python top-level functions present in Python order within each part with the required one-line docs.
- Every checker/mod.rs measurement call supplies actual program/profile/env/flat arguments as checker/__init__.py does. These entry points return unit. Warnings' sequential checks receive their real env.
- Connected part1's delay, CS616, excitation, sequential, timed-SDM offset and FFT offset calls to full home APIs; preserves ProgramRef identity and Python's Some(env)/None offset distinction.
- Removed part2's constant/int/index/flat-offset/delay-option/task-memory/scan copies and duplicate constants. Calls use instr_measure home helpers and instr_rules::_scan_raw. Python home behavior decides differing copies (notably finite float int conversion).
- Removed part3's byte/constant/int copies. Removed instr_rules/part4's duplicate _p4_index_value/_p4_flat_offset; SDI12 fill uses instr_measure::_flat_offset. These copies agree on accepted Name/Index inputs; Python home's constant guard decides.
- Replaced part1's identical inline flatten copy with instr_data::_with_inline, as Python does.
- Context.measurement_aliases now caches the implemented lookup in OnceCell and supplies real scalar scope tables/env; _measurement_aliases consumes it. Lookup retains per-statement ScopeTables keyed by original stmt_id, avoiding an Rc cycle from Context -> cached lookup -> ScopeLookup -> Context.
- Part3 uses actual long-conversion name sets, measurement aliases and overrun functions, CDM skip predicates, and AVW200 trailing-token diagnostics.

## Remaining / caller TODOs
None for this module. No new caller TODO(wire) was necessary; all arguments were available. No needed syntax.rs field identified.

Counts in instr_measure: todo!( occurrences 0; function definitions ending _with 0. Literal _with occurrences 20 (part1 10, part2 6, part3 4), from external full APIs, Python's real _with_inline and standard string/collection methods. Additional Rust-only data lookup/nested-closure helpers remain; they are not duplicate Python functions.

## Files touched
- src/checker/instr_measure/mod.rs
- src/checker/instr_measure/part1.rs
- src/checker/instr_measure/part2.rs
- src/checker/instr_measure/part3.rs
- src/checker/context.rs
- src/checker/instr_rules/part4.rs
- src/checker/mod.rs
- src/checker/warnings.rs
- HANDOFF.md

## Observed validation
Initial cargo check passed (4.39s). Intermediate cargo checks passed after connection (0.95s), deduplication (0.49s), part3 connection (0.45s), cache ownership fix (1.68s), inline reuse (0.92s).

First required full gate passed, exit 0, /tmp/wire-measure-replay1.log: fmt and strict all-target Clippy green; lib 282 passed (0.24s); release fixtures 69819/77293 cases, 413/613 files (91.05s); golden AST 77903/77903 (9.67s), golden lines 77903/77903 (8.77s). Predates cycle fix and identical inline-helper reuse.

FINAL SOURCE required gate PASSED, observed exit 0, /tmp/wire-measure-final.log:
`cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test --lib && cargo test --release --test fixtures --test golden_ast --test golden_lines -- --nocapture`
- Formatting passed; strict all-target Clippy passed (1.39s).
- Debug library tests: 282 passed, 0 failed (0.25s).
- Release fixtures passed (91.43s): cases passing 69819/77293; files fully passing 413/613; reference-recording agreement 71524/77293.
- All 350 checked-in baseline entries retained. tests/fixture_baseline.txt unchanged.
- Gain from supplied start and observed previous /tmp/wire-data-release-tests.log (63429/77293, 350 files): +6390 cases, +63 fully passing files. No independent pre-edit replay.
- Compared partial-file totals against that previous log: no previously partial file lost cases and no newly partial files. No fixture panic reports in either replay.
- Golden AST: 77903/77903, passed (9.61s).
- Golden lines: 77903/77903, passed (8.84s).
- No source edits after final gate started. git diff --check passed; read-only diff confirms syntax.rs, handoffs/ and tests/fixture_baseline.txt untouched. No unsafe introduced.

Final observed gate tail:
```text
golden lines passing: 77903/77903
test golden_lines ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 8.84s
```

## Deviations and retained limits
Part3 contained placeholder builder/destination data rather than Python values: restored CSAT3B/monitor, HydraProbe, GOESGPS, RectPolar, CS7500/LI7200/LI7700, TGA, EC100, Realtime and per-rep families from Python. Missing alias/token/CDM connections were restored at the same time. These are necessary corrections to incomplete port connections exposed by wiring; no existing baseline or partial-file matches were lost. Rust-only p3_* data lookup helpers represent Python dictionaries and nested closures and remain. Existing i64/large-integer and saturating-product representation limits retained; the replay exposed no panic or lost cases requiring changes there. No tests added or baseline edits; existing unit callers were updated to the renamed real APIs.
