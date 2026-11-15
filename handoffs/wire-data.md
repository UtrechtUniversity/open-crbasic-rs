# wire-data

## Current state
All 36 incomplete/full pairs merged in instr_data parts 1–3. All 50 Python functions exist in Python order within their parts. No instr_data todo!() or function definitions ending _with remain. Required one-line Python port docs used.

Checker callers pass actual env/profile/program arguments and consume placement/table-stop results. Unknown CallTable/table-reference stop queries and late reference-line filtering restored. Stage2a hooks removed; real context known names and home functions used directly. Fieldless checks receive env. Query/structure check_end_blocks signatures preserve ProgramRef identity.

Removed duplicate local real-params, inline-walk, approximate known-name and undefined-index implementations in part2; part3 reps conversion uses part2 (Python selects part2's sentinel behavior for huge ints/nonfinite floats). Shared table-output membership uses part2. No _pN_* copies existed. Existing fieldnames insertion clamp/regression test preserved. No AST field change needed.

## Remaining dependencies
- src/checker/instr_data/part1.rs::_unknown_table_references -> _getrecord_short: TODO(wire), tables::build_tables requires an unavailable real BuildTablesHooks adapter. Empty built-table slice retained pending tables wiring; numeric GetRecord short-destination checking cannot run through that caller yet. Full _getrecord_short implementation accepts built tables.
- src/checker/mod.rs duplicate-fields call: TODO(wire), result.tables is passed but table builder must supply duplicate-field bookkeeping. check_duplicate_fields/_dup_entries accept explicit metadata; fallback existing TdfTable fields used when absent. No syntax.rs change required. Building tables itself remains the separate tables task.
- All instr_data entry wiring complete; final release gate passed, exit 0. All 306 baseline files preserved.

## Files touched
- src/checker/instr_data/mod.rs
- src/checker/instr_data/part1.rs
- src/checker/instr_data/part2.rs
- src/checker/instr_data/part3.rs
- src/checker/mod.rs
- src/checker/structure/part1.rs (renamed call)
- src/checker/structure/part4.rs (ProgramRef argument)
- HANDOFF.md

## Observed checks
- Pre-edit cargo check passed (4.55s).
- Post-part cargo check passed: part1 0.54s, part2 0.52s, part3 0.53s.
- cargo fmt --check and cargo clippy --all-targets -- -D warnings passed (Clippy 3.13s; repeated gate 0.06s).
- Debug cargo test --lib: 282 passed, 0 failed (0.25s).
- Final full gate PASSED, exit 0: cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test --release -- --nocapture. Log /tmp/wire-data-release-tests.log. Release lib tests: 282 passed, 0 failed (0.05s). Release chosen per task instruction to avoid debug fixture runtime; full debug fixture replay not run.
- Release fixture replay passed in 92.43s: 63429/77293 cases; 350/613 fully passing files; reference-recording agreement 71524/77293. All 306 checked-in baseline files preserved. User-supplied start 59494/77293, 306 fully passing files: gain +3935 cases/+44 files. No independent before replay.
- Fixture-phase unexpected panic reports: 0 (expected unit-test panic probes occur before fixture phase).
- Golden AST passed: 77903/77903 (9.57s). Golden lines passed: 77903/77903 (8.75s). Doc tests: 0 failed, 1 ignored.
- Compared partial-file totals with previous /tmp/wire-expr-final-tests.log: no previously partial file lost cases. tdf_expressions.json newly partial at 4/20; absent from previous partial listing and the unchanged 306-file baseline, so previous run had zero passing there. No new baseline loss or unexpected panic to repair.
- git diff --check passed. No commits or Git state mutations. syntax.rs, handoffs/ and tests/fixture_baseline.txt untouched.

## Counts
- todo!( occurrences: 0.
- Function definitions ending _with: 0.
- Literal _with occurrences: 38 (13 + 10 + 15): Python's real _with_inline, standard methods, and external unwired-module full APIs; no instr_data implementation sibling remains.

## Final observed gate tail
```text
golden lines passing: 77903/77903
test golden_lines ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 8.75s
Doc-tests opencrbasic
test src/pyre.rs - pyre (line 12) ... ignored
test result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s
all doctests ran in 0.28s; merged doctests compilation took 0.11s
```

## Deviations and limitations
No change to syntax.rs, tests/fixture_baseline.txt or handoffs/. No commits or Git state-changing commands. No unsafe code. Source was unchanged after final release gate started.
The table builder dependencies above remain explicit instead of inventing metadata or broadening into tables wiring. Python-style known-name metadata and undefined-index detection now use the already wired expression implementations; old local approximations removed. Stage2a unit callers now exercise actual home dependencies instead of synthetic hooks. No new tests added.
