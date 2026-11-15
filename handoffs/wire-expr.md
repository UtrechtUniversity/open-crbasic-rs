# wire-expr

Expression wiring complete; final gate passed. No commits or Git mutations. Scope: expressions parts and required callers.

- Read AGENTS.md, wire-rules and wire-struct handoffs, expression headers/TODOs, P4 notes.
- All 63 incomplete/full entry pairs in parts 1–5 merged; 78 top-level Python functions remain in Python order.
- All expression calls in checker/mod.rs use real profile/context/env/skip arguments. Optional held list is None as in Python; expression entry points return unit.
- Part 3's panicking pending_wiring module removed; APIs connected to expressions, symbols, structs and instr_rules. Matrix operands use the original ProgramRef.
- No `_pN_*` duplicates found in expressions. Duplicate constants, statement-expression helper and empty class methods consolidated.
- Final observed gate: fmt --check, all-target Clippy, full cargo test -- --nocapture all passed (exit 0). Fixtures 59494/77293; 306/613 fully passing files; 282 unit tests passed. See final validation below.
- Starting parity from user/previous handoff: 49517/77293, 163 fully passing files. Actual checked-in baseline lists 53 files. No independent before replay.
- No syntax.rs, handoffs/ or fixture baseline edits.

- Parts 1 and 2 entry pairs renamed; cargo check passed after each (dev target, 1.39s and next check).
- Part 5 dimension/header callers adapted early as part 2 dependencies: real KnownNames, DimsState, DimCounts, profile and statement identity.
- `_undefined_names` plain-set callers currently map the typed results back to strings; KnownNames callers use the metadata view as wiring proceeds.

- Part 3 entry pairs and all panicking pending_wiring bridges replaced. Direct calls to home parts and external full APIs; typed-name conversion helpers retain dotted diagnostic formatting.
- checker/mod.rs now passes profile/env/placement-rejected statements to expression check, following Python.
- Part 3 cargo check passed. Tables `_data_table_fields` still returns empty: real tables build requires unimplemented BuildTablesHooks integration; to document explicitly.

- Part 4 wired and cargo check passed. Restored omitted calls to Timer/network integer/type checks and timeout state; RAII clears timeout gate/wide state. Real known names and CR3xx prose API connected.
- Part 4 checker entry calls use profile and None for optional held list, matching Python.

- All five parts renamed/connected; no expression Python `_with` implementation siblings or `todo!` remain.
- KnownNames::without now takes the real exclusion set; empty constructor stubs removed in favor of real constructors.
- Local repeated constants consolidated to owning expression parts and instr_data table-only map.
- Observed: cargo fmt --check passed; cargo clippy --all-targets -- -D warnings passed (3.13s); cargo test --lib: 281 passed, 0 failed (0.25s).
- Full test suite with uncaptured fixture totals starting now; no fixture replay run before this.

## Audit after replay 1 binary started
- Source now also connects instr_data::check_fieldnames and var_types::check_ctype known-name contexts, and intervals' identical statement-expression copy to expressions::_stmt_exprs.
- Structure's duplicate _CONST_TYPES now imports the expressions home constant (handoff outstanding item resolved).
- Top-level Python function coverage/order audit: all 78 functions present; order correct within every part.
- Replay 1 is testing the pre-audit binary; final-source replay will be required (second and final full replay).

## Remaining dependency / representation limits
- `src/checker/expressions/part2.rs::_data_table_fields`: `TODO(wire)` records that tables::build_tables_with requires a real BuildTablesHooks adapter. No such adapter exists in the crate. Kept the implemented sibling's empty-map fallback (Python also uses an empty map when building fails); DataTable generated-field validation awaits tables wiring. No synthetic builder input added.
- Dimension helpers retain the independent port's Vec<String> result API. Part 3 tags these results Plain for diagnostic formatting; direct undefined-name calls preserve KnownNames dotted/out-of-scope metadata. This is a retained representation boundary, not an AST-field change.
- No `_pN_*` expression copies existed. Empty `__init__` methods and incomplete `without` removed; real Rust constructors/without used.
- Required port docs use the literal one-line form; module allows clippy::doc_markdown, following wire-rules.
- No needed syntax.rs field identified, and syntax.rs is untouched.
- Final-audit fmt and all-target Clippy passed (2.08s); final-audit lib tests 281 passed, 0 failed (0.30s), before the trivial reuse of precomputed placement_rej.

## Files touched (current)
- src/checker/cr3xx/part1.rs
- src/checker/cr3xx/part2.rs
- src/checker/cr3xx/part3.rs
- src/checker/expressions/mod.rs
- src/checker/expressions/part1.rs
- src/checker/expressions/part2.rs
- src/checker/expressions/part3.rs
- src/checker/expressions/part4.rs
- src/checker/expressions/part5.rs
- src/checker/instr_data/part1.rs
- src/checker/instr_data/part2.rs
- src/checker/instr_measure/part1.rs
- src/checker/instr_rules/part1.rs
- src/checker/instr_rules/part2.rs
- src/checker/instr_rules/part3.rs
- src/checker/instr_rules/part4.rs
- src/checker/intervals.rs
- src/checker/mod.rs
- src/checker/structs.rs
- src/checker/structure/part3.rs
- src/checker/structure/part4.rs
- src/checker/structure/part5.rs
- src/checker/structure/part6.rs
- src/checker/structure/part7.rs
- src/checker/structure/part8.rs
- src/checker/var_types.rs
- HANDOFF.md

## Panic found during replay 1
- Two fixture-phase caught panics observed: `range end index 4 out of range for slice of length 3` (alloc::vec::drain).
- A minimal repeated-FieldNames unit reproducer in instr_data::part1::move_fieldnames_symbol_errors produced exactly that panic (observed 0 passed / 1 failed; /tmp/expr-panic-probe.log).
- Python instr_data.py uses `items[at:at] = moved + pair`: insertion clamps when a cached next_at exceeds the now-shorter list after removing another FieldNames' symbol diagnostic. Rust Vec::splice requires an in-bounds range.
- Necessary deviation outside expressions: clamp only the insertion position in instr_data/part1.rs, retaining the Python cached next_at arithmetic. Added the regression test beside its existing owner/move test. This file was already touched for expression caller wiring.
- Observed after fix: fmt --check and Clippy passed (1.03s); full lib tests 282 passed, 0 failed (0.25s). Final replay pending.

## Replay 1 observed (pre-audit binary)
- Full gate exit 0: fmt --check, Clippy, cargo test -- --nocapture.
- Fixtures: 305/613 files fully passing; cases passing 59490/77293; reference recording agreement 71524/77293. Replay 825.44s; all 53 baseline files preserved.
- Unit tests 281 passed; golden AST 77903/77903 (15.79s); golden lines 77903/77903 (10.34s); doc tests 0 failed, 1 ignored.
- Two caught FieldNames range panics; minimal exact reproducer fixed in current source as described above.
- Second and final full replay starting with final source and RUST_BACKTRACE=1 for any remaining unexpected panic. Do not launch a third full replay.

## Precise retained case regressions / CR3xx dependency
Comparison with the previous structure run's /tmp/wire-final-tests.log (read this session) finds two files that were already partial and now have fewer passing cases in replay 1:
- r12_cr3xx_table_field_hidden.json: 16/38 -> 4/38.
- r10_cr3xx_block_quotes.json: 6/18 -> 2/18.
Targeted current-source CLI probes (two cases only, not another corpus replay), saved in /tmp/wire-expr-regression-probes.json:
- cr300-r12_cr3xx_table_field_hidden-0: expected only `line 4: Zork is not defined.`; actual additionally `line 5: Could not find DataTable Nope.`. Python cr3xx.py::drop_calltable_error_with_other_errors removes this table diagnostic. Rust checker/mod.rs still calls cr3xx/part3.rs's incomplete entry (line ~1189), rather than its full sibling.
- cr300-r10_cr3xx_block_quotes-0: both required lines appear, but `Incomplete expression: ...` is before `Nope is not defined.`. Python cr3xx.py::order_block_keyword_calls reorders them. Rust checker/mod.rs still calls cr3xx/part3.rs's incomplete entry (line ~1245), rather than its full sibling.
These are deferred to the explicitly separate CR3xx wiring task; expression behavior was not disabled to regain accidental earlier matches. Their unwired passes are visible as caught todo panics in the two CLI probes' stderr. No expression todo panic occurred there.
Ten files appear as partial in replay 1 but not as partial in the old log: closer_arity.json, closer_public_lexical.json, dims_state.json, r148_sendvariables_destination.json, r14_menupick_plain_const.json, r17_timeout_mode.json, r67_measurement_reps.json, r7_cr3xx_shortfall_order.json, r9_cr3xx_do_if_cond.json, unclosed_structure.json. The old log omits both fully-passing and zero-passing files, so this alone does not establish a regression for those ten. All checked-in baseline files passed.

## Final observed validation (final source)
Command: `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test -- --nocapture`, with backtraces enabled for unexpected panics. Exit 0; log /tmp/wire-expr-final-tests.log.
- Formatting passed. All-target Clippy with warnings denied passed (0.07s).
- Unit tests: 282 passed, 0 failed (0.24s).
- Fixture replay 2/2: 826.87s, test passed; 306/613 files fully passing; **59494/77293 cases passing** (+9977 from supplied start, +143 fully passing files). Reference recording agreement: 71524/77293.
- All 53 checked-in baseline files preserved. Baseline actually lists 53 files; the previous run's 163 fully-passing count is a separate aggregate.
- Unexpected fixture-phase panic reports: **0**. The repeated-FieldNames slice fix is included and its regression test passes.
- Golden AST: 77903/77903, passed (15.67s). Golden lines: 77903/77903, passed (10.45s).
- Doc tests: 0 failed, 1 ignored (0.00s).
- The two documented CR3xx partial-file regressions remain at 4/38 and 2/18 in the final source; they are precise deferred dependencies, not baseline losses.
- No third full fixture replay launched. Two standalone CLI case probes were used to identify the CR3xx differences.

### Final observed gate tail
```text
golden lines passing: 77903/77903
test golden_lines ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 10.45s
Doc-tests opencrbasic
test src/pyre.rs - pyre (line 12) ... ignored
test result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s
all doctests ran in 0.29s; merged doctests compilation took 0.11s
```

## Final counts / audit
- Expressions `todo!(` occurrences: 0.
- Expression function definitions ending `_with`: 0; all 63 pairs merged, all 78 Python top-level functions covered in order.
- Literal `_with` occurrences: 31, from external unwired-module APIs, standard String/map methods, and the tables builder TODO comment. No expression implementation sibling remains.
- Only expression TODO(wire): part2::_data_table_fields / missing real tables builder adapter.
- `git diff --check` passed; no source changes after final replay started. Final handoff documentation updated after gate.
- src/syntax.rs, tests/fixture_baseline.txt and handoffs/ unchanged. No Git state-changing command, commit, or unsafe code introduced.
