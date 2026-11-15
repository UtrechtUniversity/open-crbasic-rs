# wire-cr3xx (src/checker/cr3xx wiring) -- DONE

No commits made.

## Done
- All todo!() stub / `_with` pairs in cr3xx part1..part4 merged (stub deleted, `_with` renamed to the Python name, Python order kept).
- Part 4 wired: `order_unterminated_string`, `add_string_illegal`, `check_tcpopen_handle` now take `(&ProgramRef, &'static Profile, &mut Diagnostics)`
  and use the real `prose_known`/`first_declarations`/`declared_later`/`_leading_unknown_words`/`_string_reached`/`structure::_unparsed_string_messages`/
  `context::of(..).declared_variable_names()/variable_types()/known_names()`/`syntax::ends_routine`/`expressions::{_stmt_exprs,_index_nodes}`.
  `semicolon_headers` takes `(&Program, &Profile, diags)`. checker/mod.rs passes `profile`.
- Bug fixed in part3 `check_unknown_line_words`: the Python passes `callables` (declared names | functions) to `_leading_unknown_words`; the port computed but dropped it.
- Duplicate constants/regexes dropped: part3 `P3_*` copies and part4 `*_P4` copies now use part1/part2/part3 homes (pub(super)).
- Removed the tcpopen unit test that used injected closures (covered by fixtures); other part4 tests rewritten for the real signatures.
- Instruction-rule shims in part1/2/3 already call `instr_rules` homes. No duplicates of cr3xx functions found in other modules.

## Remaining (by design)
- `todo!(` in cr3xx: 0.
- `_with` names left: `_leading_unknown_words_with` (Python function has optional `callables`/`state` args; Rust has no defaults, thin wrapper
  `_leading_unknown_words` kept), `drop_shortfall_with_names` and `drop_calltable_error_with_other_errors` (these are the Python names), `env_with_consts` (helper).
- Caller TODOs: none. No syntax.rs field needed.

## Files touched
src/checker/cr3xx/{mod,part1,part2,part3,part4}.rs, src/checker/mod.rs, src/checker/expressions/part4.rs, src/checker/instr_rules/part3.rs, HANDOFF.md

## Gate (observed)
cargo fmt --check OK; clippy --all-targets -D warnings OK; cargo test --lib 281 passed;
fixtures: `fixture files fully passing: 557/613`, `cases passing: 74821/77293` (start 69819 / 413), `cases equal to reference stdout and exit: 71524/77293`;
golden_ast 77903/77903; golden_lines 77903/77903.
