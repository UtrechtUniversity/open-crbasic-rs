# walk handoff

Status: assigned port complete and required validation green. No git mutations or commits.
Scope: only src/checker/structure/part2.rs and this explicitly requested HANDOFF.md changed.

Fully ported: `_scan_placement_walk_with`, Python structure.py lines 781–849. Uses merged part1 `*_with` APIs and constants, TableState, Rc::ptr_eq for misplaced statements, stmt_id for stray controls, and context::of on the original ProgramRef. Original `_scan_placement_walk` stub untouched. No `_pN_*` helpers added.
Remaining in assigned function: none; stopped after Python line 849. No signature/type/field changes needed.

Read first: AGENTS.md porting rules, HANDOFF.md, handoffs/struct-1.md, handoffs/struct-2.md; then part1.rs, Python walk, and dependency signatures.

Other-file dependencies / integration limits:
- `first_misplaced_sub_with` in part1 still reaches legacy `_block_word()` on END/NEXT statements. Part2's `_block_word_with` exists, but changing its part1 callers is outside scope.
- `_scan_counts_for_table_placement_with` in part1 still calls legacy `instr_rules::scan_split` without env. `scan_split_with` now exists in its owning file. This transitive stub can panic for nonzero Scans; owning-file wiring remains necessary.
- Existing callers, including part1's late-placement adapter, still need separate wiring to the requested ProgramRef/profile/stop-location sibling. No caller changed here.

Validation observed on final source (Cargo PATH set as requested; OPENCRBASIC_PY set in the shell only):
- `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test --lib`: exit 0. Clippy: `Finished dev profile [unoptimized + debuginfo] target(s) in 3.04s`. Library tests: `279 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.24s` (/tmp/cx-walk-lib.log).
- New four-profile regression test passed, covering five cases observed directly from read-only Python: accumulated table diagnostics then stray NextScan, inline diagnostic order and early stop, misplaced Sub priority, declaration-block state, nested literal-zero Scans and SlowSequence. Nested-scan diagnostic kind is checked. CR6/CR1000X return without diagnostics.
- Ran the requested `cargo test --test fixtures -- --nocapture 2>&1 | tail -15` exactly once, with pipefail: exit 0. Observed tail:
  `fixture files fully passing: 30/613`
  `cases passing: 28310/77293`
  `cases equal to reference stdout and exit (python recording): 71524/77293`
  `test verdict_fixtures ... ok`
  `test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 85.60s`
  No live before-count was run in this task; do not attribute differences from earlier handoffs to this unwired sibling.
- Read-only `git diff --check` passed. Git status showed only part2.rs modified and this handoff untracked.

Deviations: no algorithm changes. A local documented clippy::doc_markdown allowance preserves the exact mandated one-line port doc comment. Python probes disabled bytecode writes. Initial clippy findings (doc formatting, collapsible condition, redundant Rc wrapping in the test, empty-collection assertion) were corrected before the green gate.
