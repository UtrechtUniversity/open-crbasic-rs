# wire-struct

## Wired

- All nine structure parts: incomplete entry points removed; full siblings renamed to
  Python names; all callers in src/ and tests/ searched and updated. Python top-level
  function order verified in each part. Literal one-line port docs retained.
- checker/mod.rs supplies profiles/environments, receives pipeline/placement results,
  initializes CallLinks positions, consumes pass2's return/cutoff, and uses the same
  links for deferred diagnostics and finalization.
- Part1 late-placement adapter replaced by the actual part2 placement walk.
- Part3 pending_call/pending_result bridges removed; all omitted arguments and three
  result dependencies connected to their home implementations.
- Declaration.vars is RefCell<Vec<Rc<VarDecl>>>. All users updated, including Rc
  serialization. Pass1 retains the shared statement identity when shortening vars.
- Part5 scanner adapters forward tokens/functions/all options to part6; lexer tokens
  now used directly except where parser semicolon helpers require parser tokens.
  Option operands preserve Python _Everything (None) versus finite names (Some).
- External placeholders connected: expression out-of-place messages, expression
  known-name alias dimension checks, CR3xx For-header helpers/scanners, warning
  pipeline reachability. All original Rc identities retained where context needs them.
- Duplicate binary-word/operator, zero-param instruction, tolerant-code and table-output
  membership values use their owning modules. No `_pN_*` copies found in structure.

## Remaining

- Complete: no structure Python function remains unwired; final-source gate passed.
- No new caller TODO(wire) needed so far: existing callers could supply the arguments.
- Other modules still have their own unwired entry points, outside this task. Calls
  from structure use those modules' implemented full APIs where available.
- Existing handoff limitations retained: `_illegal_dim_message` expects the original
  VarDecl identity; part7's _CONST_TYPES remains local because expressions has no
  exported home constant; large-number handling in part8 is unchanged.
- Structure `todo!(` occurrences: 0. Function definitions ending `_with`: 0.
  Literal `_with` occurrences: 113 (100 suffix identifiers, including string methods
  and references to other modules' full APIs); no structure implementation sibling.

## Observed validation

- cargo check passed after integrated parts 1/2, 3 (including shared vars), 4/5,
  6, 7, 8, and 9. Part4 check initially exposed positions type mismatch, fixed
  together with part5 integration before the next successful check.
- cargo fmt --check and cargo clippy --all-targets -- -D warnings passed before
  the first full test run. Initial all-target check found four stale test callers;
  corrected. Latest caller-audit source passed Clippy too (1.74s).
- First full gate PASSED, exit 0 (/tmp/wire-tests.log): 281 unit tests passed;
  fixtures passed in 757.84s, preserving all baseline files; golden AST passed
  (15.62s), golden lines passed (10.24s); doc tests 0 failed, 1 ignored.
  This binary predates the final caller audit; parity total was captured by libtest
  on success, so no aggregate count claimed from that run.
- Final-source gate PASSED, exit 0, using cargo test -- --nocapture to observe parity:
  /tmp/wire-final-tests.log. Final fmt --check and Clippy passed (0.01s).
  Final-source unit tests: 281 passed, 0 failed.
  Final-source fixture replay PASSED in 763.28s:
  fixture files fully passing: 163/613
  cases passing: 49517/77293
  cases equal to reference stdout and exit (python recording): 71524/77293
  test verdict_fixtures ... ok
  test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
  All 53 baseline files preserved.
  Golden AST: 77903/77903; 1 passed, 0 failed, 15.66s.
  Golden lines: 77903/77903; 1 passed, 0 failed, 10.42s.
  Doc tests: 0 passed, 0 failed, 1 ignored, 0.00s.
  No fixture panic from newly connected structure code observed; the only panic
  reports in the uncaptured log were the expected unit panic tests.
- Observed final-source parity: 49517/77293, 163 fully passing files. User-supplied
  start was 34492/77293 with 53 fully passing files. No independent before replay.
- git diff --check passed. No commits or Git state-changing commands performed.
  tests/fixture_baseline.txt and handoffs/ are unchanged (final read-only diff verified).

## Final observed gate tail

```text
golden lines passing: 77903/77903
test golden_lines ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 10.42s
Doc-tests opencrbasic
test src/pyre.rs - pyre (line 12) ... ignored
test result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s
all doctests ran in 0.32s; merged doctests compilation took 0.15s
```

## Files touched

- src/checker/context.rs
- src/checker/cr3xx/part1.rs
- src/checker/cr3xx/part2.rs
- src/checker/cr3xx/part3.rs
- src/checker/ess.rs
- src/checker/expressions/part2.rs
- src/checker/expressions/part3.rs
- src/checker/expressions/part4.rs
- src/checker/expressions/part5.rs
- src/checker/instr_data/part2.rs
- src/checker/instr_data/part3.rs
- src/checker/instr_measure/part1.rs
- src/checker/instr_measure/part2.rs
- src/checker/instr_rules/part1.rs
- src/checker/instr_rules/part2.rs
- src/checker/instr_rules/part3.rs
- src/checker/instr_rules/part4.rs
- src/checker/intervals.rs
- src/checker/mod.rs
- src/checker/storage.rs
- src/checker/structs.rs
- src/checker/structure/mod.rs
- src/checker/structure/part1.rs
- src/checker/structure/part2.rs
- src/checker/structure/part3.rs
- src/checker/structure/part4.rs
- src/checker/structure/part5.rs
- src/checker/structure/part6.rs
- src/checker/structure/part7.rs
- src/checker/structure/part8.rs
- src/checker/structure/part9.rs
- src/checker/symbols.rs
- src/checker/var_types.rs
- src/checker/warnings.rs
- src/parser/decls.rs
- src/parser/driver.rs
- src/parser/stmts.rs
- src/syntax.rs
- src/tables/part1.rs
- src/tables/part2.rs
- HANDOFF.md
