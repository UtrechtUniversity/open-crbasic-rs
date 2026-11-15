# struct-2 handoff

Status: incomplete: 19 of 20 functions implemented. The remaining function needs incompatible APIs in another worker's file. Changes are uncommitted.

## Fully ported

`_is_case_else`, `stray_controls_with` (original delegates), `scan_only_outside`, `_subscan_body_time_with`, `subscan_controls_with`, `check_stray_controls_with`, `_has_error_on`, `_before_beginprog_name_with`, `_quote_in_first_arg`, `_control_keyword`, `_table_names`, `_leftover_message_with`, `_constant_assignment_with`, `_no_matching_with`, `_out_of_place_with`, `_function_statement_with`, `_semicolon_assignment`, `_keyword`, `_block_word_with`.

Constants/regex: `SCAN_ONLY`, `_SUBSCAN_ZERO_TIME`, `_SUBSCAN_PLAIN`, `_FALLBACK_PREFIX`, `_CONTROL_COND`, `_END_EXIT_WORDS`, and the four function-local Python patterns cached as LazyLock regexes.

## Remaining / partial

`_scan_placement_walk_with` remains an explicit todo, stopped before Python line 789; the incompatible return is first needed at line 799. No other implementation is partial. Original incomplete-signature wrappers remain todo as instructed, except `stray_controls` can delegate while discarding its result.

## Other-file dependencies

Called stubs: `structure::part5::condition_part(text: &str) -> Option<String>` and `structure::part5::inline_if_condition(text: &str) -> Option<String>`.

The placement walk needs these part1 APIs, currently absent or incompatible; no replacement implementations were added:

- `_track_tables_with(stmt: &Stmt, kw: &str, state: &mut TablePlacementState)`; current `_track_tables` omits state.
- `_calltable_known_with(stmt: &Stmt, state: &TablePlacementState) -> bool`; current function omits state and returns ().
- `_scan_counts_for_table_placement_with(stmt: &Stmt, env: Option<&Env>) -> bool`; current function omits env.
- `_check_calltable_outside_scan_with(stmt: &Instruction, program: &Program, diags: &mut Diagnostics, in_sub: bool, zero_scan: bool, env: Option<&Env>)`; current function omits env.
- `_accept_known_with(stmt: &Stmt, state: &TablePlacementState) -> bool`; current function omits state and returns ().
- `_check_accept_table_with(stmt: &Instruction, program: &Program, diags: &mut Diagnostics, outside: bool, env: Option<&Env>)`; current function omits env.
- `first_misplaced_sub_with(program: &Program) -> Option<(StmtRef, Vec<String>)>`; current function returns ().
- Missing part1 constants: `_DECL_BLOCK_BOUNDS`, `_BODY_BLOCK_STARTS`, `_BODY_BLOCK_ENDS`, `NESTED_SCAN`, `_FORBIDDEN_BEFORE_BEGINPROG`.

`TablePlacementState` needs optional open-table name and insertion-preserving closed-table names; its concrete shared type belongs to part1. Placement-walk dependencies above are not yet called because the walk remains todo.

## Signature problems

Existing public signatures preserved. Full variants:

- `_scan_placement_walk_with(program: &ProgramRef, profile: &'static Profile, diags: &mut Diagnostics) -> Option<(String, i64)>` (ProgramRef preserves context identity).
- `stray_controls_with(program: &Program) -> OrderedMap<usize, (String, bool)>`.
- `_subscan_body_time_with(stmt: &Stmt, profile: &Profile) -> Option<bool>`.
- `subscan_controls_with(program: &Program, profile: &Profile) -> OrderedMap<usize, (String, bool)>`.
- `check_stray_controls_with(program: &Program, profile: &Profile, diags: &mut Diagnostics) -> Option<(String, i64)>`.
- `_before_beginprog_name_with(stmt: &Stmt, kw: &str, profile: &Profile, subs: &BTreeSet<String>) -> Option<String>`.
- `_leftover_message_with(profile: &Profile, stmt: &Stmt) -> String`.
- `_constant_assignment_with(text: &str, profile: &Profile) -> bool`.
- `_no_matching_with(profile: &Profile, text: &str, at_eol: bool) -> String`.
- `_out_of_place_with(profile: &Profile, text: &str, at_eol: bool) -> String`.
- `_function_statement_with(text: &str, profile: &Profile) -> bool`.
- `_block_word_with(args: &[Option<ExprRef>]) -> Option<String>`.

## Last observed gates

Baseline `cargo test --test fixtures -- --nocapture` passed (original stubs were compiled before edits):
```text
fixture files fully passing: 26/613
cases passing: 27110/77293
cases equal to reference stdout and exit (python recording): 71524/77293
test verdict_fixtures ... ok
test result: ok. 1 passed; 0 failed; finished in 313.25s
```

The required gate passed:
```bash
export PATH=/opt/homebrew/opt/rustup/bin:$PATH
cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test
```
Observed results:
```text
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.13s
test result: ok. 253 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.43s
test verdict_fixtures ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 244.60s
test golden_ast ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 101.35s
test golden_lines ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 60.23s
Doc-tests opencrbasic
test result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

The six new in-file regression tests also passed separately:
```text
cargo test --lib checker::structure::part2::tests
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 247 filtered out; finished in 0.02s
```

The explicit after-count `cargo test --test fixtures -- --nocapture` passed:
```text
fixture files fully passing: 26/613
cases passing: 27110/77293
cases equal to reference stdout and exit (python recording): 71524/77293
test verdict_fixtures ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 416.89s
```
Before: **27110/77293**. After: **27110/77293**. No baseline fixture regression.

Logs: `/tmp/struct2-before.log`, `/tmp/struct2-final-gate.log`, `/tmp/struct2-after.log`.

Read-only Python probes observed statement order, block persistence across routine ends, Case Else prefix matching, four-profile nested SubScan behavior, placement stops, semicolon fallback messages, predefined assignments, and Latin-1 quote scanning.

## Deviations

- `_scan_placement_walk` cannot be completed faithfully against the current part1 interfaces within the allowed file scope.
- `_before_beginprog_name_with` uses the exact `SCAN`/`DELAY` values of part1's missing `_FORBIDDEN_BEFORE_BEGINPROG` inline; integration should import the shared constant once available.
- Python AST class tuple `_SUBSCAN_PLAIN` represented as class names, with membership implemented by explicit Stmt variant matching; frozen membership sets use arrays, with no set iteration used for output.
- HANDOFF.md is the explicitly requested exception to the part2.rs-only editing scope. No git operations performed.
