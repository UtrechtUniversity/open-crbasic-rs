# struct-1 handoff

Scope: Python checker/structure.py lines 1–779, read in full before writing. Edited only src/checker/structure/part1.rs and this explicitly required handoff. No git mutations or commits.

Fully ported bodies (full signatures use `_with` where required): constants and regexes; `_assignment_head`, `_track_tables`, `_calltable_known`, `_scan_counts_for_table_placement`, `_check_calltable_outside_scan`, `_accept_known`, `_check_accept_table`, `_ends_sub`, `first_misplaced_sub`, `check_apply_restart_sequence`, `first_misplaced_data_table`, `check_data_table_placement`, `pipeline_task_errors` (including nested `visit`), `check_pipeline_task_placement`, `pipeline_argument_error_first_locations`, `check_slow_sequence_blocks`. These retain calls to unported dependencies; they are not all runnable end to end yet.

Partial: `check_scan_placement_late` (Python 748–779): full post-walk algorithm is implemented in `_with`, but its local `walk` adapter calls the existing unit-returning dependency and then explicitly stops with `todo!`; it cannot recover a stop location from `()`. No remaining source range unread. Current stop: Python line 754, the dependency call; all following Python logic has a Rust body. Validation is complete; remaining work is dependency wiring outside this task’s allowed file.

Other-file dependencies called that remain stubs:

- structure/part2.rs `_keyword(&Stmt) -> String` (signature complete).
- structure/part2.rs `_block_word() -> Option<String>` needs `_block_word_with(args: &[Option<ExprRef>]) -> Option<String>`. Current calls use its existing stub; argument passing is blocked by that signature.
- structure/part2.rs `_scan_placement_walk(&Program, &mut Diagnostics)` needs `_scan_placement_walk_with(program: &Program, profile: &'static Profile, diags: &mut Diagnostics) -> Option<(String, i64)>`. Late-placement adapter explicitly awaits this return value.
- instr_rules/part2.rs `scan_split(&Instruction) -> Option<(i64, i64)>` needs `scan_split_with(stmt: &Instruction, env: &Env) -> Option<(i64, i64)>`. Current call uses the existing stub, so env is not yet passed.

Signature problems in task stubs (existing public signatures preserved):

- `_assignment_head_with(text: &str, program: &ProgramRef, profile: &'static Profile) -> bool`.
- `_track_tables_with(stmt: &Stmt, kw: &str, state: &mut TableState)`; `_calltable_known_with(stmt: &Stmt, state: &TableState) -> bool`; `_accept_known_with(stmt: &Stmt, state: &TableState) -> bool`.
- `_scan_counts_for_table_placement_with(stmt: &Stmt, env: Option<&Env>) -> bool`.
- `_check_calltable_outside_scan_with(stmt: &Instruction, program: &Program, diags: &mut Diagnostics, in_sub: bool, zero_scan: bool, env: Option<&Env>)`.
- `_check_accept_table_with(stmt: &Instruction, program: &Program, diags: &mut Diagnostics, outside: bool, env: &Env)`.
- `first_misplaced_sub_with(program: &Program) -> Option<(StmtRef, Vec<String>)>`; old stub omits return value.
- `check_apply_restart_sequence_with(program: &Program, profile: &Profile, diags: &mut Diagnostics)` (Python profile unused).
- `first_misplaced_data_table_with(program: &Program, profile: &Profile) -> Option<StmtRef>`; `check_data_table_placement_with(program: &Program, profile: &Profile, diags: &mut Diagnostics)`.
- `pipeline_task_errors_with(program: &Program, profile: &'static Profile) -> Vec<(StmtRef, String)>`; `check_pipeline_task_placement_with(program: &Program, profile: &'static Profile, diags: &mut Diagnostics)`.
- `pipeline_argument_error_first_locations_with(program: &Program, profile: &'static Profile) -> BTreeSet<(String, i64)>`.
- `check_slow_sequence_blocks_with(program: &Program, profile: &Profile, diags: &mut Diagnostics)`.
- `check_scan_placement_late_with(program: &Program, profile: &'static Profile, diags: &mut Diagnostics) -> Option<(String, i64)>`.

Last observed gate: passed. Cargo PATH was `/opt/homebrew/opt/rustup/bin:$PATH`.

- `cargo fmt`: passed.
- `cargo clippy --all-targets -- -D warnings`: passed on final source. Tail: `Finished dev profile [unoptimized + debuginfo] target(s) in 12.38s` (/tmp/struct1-final2-clippy.log).
- `cargo test`: initial full run passed (/tmp/struct1-tests.log). Final-source `cargo test -- --nocapture` also exited 0 (/tmp/struct1-final-tests.log): 250 unit tests passed; fixtures passed; golden AST 77903/77903; golden lines 77903/77903. Final tail: `test result: ok. 0 passed; 0 failed; 1 ignored; ... finished in 0.00s` (one ignored documentation example); `all doctests ran in 0.28s; merged doctests compilation took 0.12s`.
- Before edits, `cargo test --test fixtures -- --nocapture`: `cases passing: 27110/77293`, 26/613 fixture files fully passing, test passed in 297.17s (/tmp/struct1-before.log).
- After edits, final uncaptured fixture replay: `cases passing: 27110/77293`, 26/613 fixture files fully passing, test passed in 231.44s. No baseline regression.
- Three new focused tests passed using values observed from the real read-only Python implementation: assignment heads (Latin-1 and nested undeclared names), table closure/name lookup, and literal-zero Scan placement.

Initial compile/lint findings were corrected. Literal port doc comments, nested helper ordering, and Python’s exact float membership equality have narrowly documented lint allowances.

Deviations / integration limits:

- Typed `TableState` models only `open`/`closed` dictionary entries, preserving closed-name insertion order with Vec. Constant sets are membership-only slices; diagnostic-location set is BTreeSet (Python set output is unordered).
- Incomplete old signatures remain stubs unless a correct delegation is possible. The profile unused by Python's apply/restart check is passed a canonical profile by its old wrapper.
- No structural identity caching was introduced; returned statements clone their original Rc. Program prefix uses `replaced()` wrapped in a fresh Rc.
- Stub dependencies above prevent faithful end-to-end execution until their owners port or wire them. No other-file logic was reimplemented.

Resume: port/wire the listed dependencies in their owning tasks, replace the local late-placement `walk` adapter with the full `_scan_placement_walk_with` call, pass statement arguments to `_block_word_with`, pass env to `scan_split_with`, and migrate callers to the full `_with` signatures.
