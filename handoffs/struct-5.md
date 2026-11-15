Task struct-5: ported Python checker/structure.py lines 2869–3659 into src/checker/structure/part5.rs. Work is uncommitted. No git operations performed. HANDOFF.md is the explicitly requested additional file.

Fully ported:

- _unclosed_paren_kind, is_hash_statement, _is_generic_unparsed.
- pass1_operands_with, inline_if_condition, _inline_condition_stmt_with, condition_part.
- _invalid_assignment_with, _semicolons_dropped_with, _pass1_scan_with, _assigns_user_const_with.
- _signed_target_with, _logical_bad_target_with, _logical_assign_eq_with, _known_first_token, _logical_led_string.
- _logical_assign_state_with, including named public_member, members_ok, target_ok and targets_ok functions.
- _logical_assign_accepted_with, _logical_assign_undefined_messages_with, _unparsed_string_messages_with, string_reached_with.
- Everything.__contains__ and __contains___with; all assigned constants and regexes.

Pending/partial: no assigned implementation body remains pending; stopped after Python line 3659. Original incomplete public stubs remain as instructed. The original _assigns_user_const signature also lacks the Rc identities required by Context::enclosing_sub; its implementation is available through _assigns_user_const_with. Scanner-dependent paths remain blocked by the following other-file stubs.

Other-file stubs called, with needed full signatures:

- `part6::_scan_expr`: needs a sibling accepting `(toks: &[Token], i: i64, operands: &BTreeSet<String>, functions: Option<&Table<Function>>, assign: bool, start_operand: bool, unclosed: Option<&str>, cond: bool, to_ok: bool, open_fallback: bool, symbol_unknown: bool) -> Option<(String, String)>`. None functions represents Python's empty dictionary. General Container support can replace the finite operand set when integrating Everything.
- `part6::_later_arg_unknown`: `(args: &[Token], operands: &BTreeSet<String>, functions: &Table<Function>) -> bool`.
- `part6::_bare_function_needs_paren`: `(name: &str, functions: &Table<Function>) -> bool`.

The existing signatures cannot receive all these arguments. Local dependency adapters retain tokens/functions at their call sites and invoke the existing stubs, which still panic. No other-file implementation was duplicated.

Signature problems addressed with full siblings; original public signatures were preserved:

- `pass1_operands_with(&ProgramRef, &'static Profile) -> BTreeSet<String>`.
- `_inline_condition_stmt_with(&StmtRef, &Profile) -> StmtRef`.
- `_invalid_assignment_with(&[Token]) -> bool`.
- `_semicolons_dropped_with(&str, &Profile) -> bool`.
- `_pass1_scan_with(&str, &BTreeSet<String>, &Profile) -> Option<(String, String)>`.
- `_assigns_user_const_with(&StmtRef, &ProgramRef) -> bool`.
- `_signed_target_with(&[Token], &BTreeSet<String>, &Table<Function>, &Profile) -> Option<String>`.
- `_logical_bad_target_with(&[Token], &BTreeSet<String>, &Table<Function>) -> bool`.
- `_logical_assign_eq_with(&[Token], i64) -> Option<i64>`.
- `_logical_assign_state_with(&StmtRef, &ProgramRef, &'static Profile) -> (String, Vec<String>, bool)`; original return type was ().
- `_logical_assign_accepted_with(&StmtRef, &ProgramRef, &'static Profile) -> bool`.
- `_logical_assign_undefined_messages_with(&StmtRef, &ProgramRef, &'static Profile) -> Option<Vec<String>>`.
- `_unparsed_string_messages_with(&Unparsed, &ProgramRef, &'static Profile) -> Vec<String>`.
- `string_reached_with(&str, &BTreeSet<String>, &Profile) -> Option<String>`.
- `Everything.__contains___with<T>(&self, item: T) -> bool`.


Observed validation:

- Before changes: cargo test --test fixtures -- --nocapture passed; cases passing: 27110/77293; 1 passed, 0 failed; 309.37s. Log: /tmp/struct5-baseline.log.
- Final gate passed: cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test -- --nocapture. Clippy finished without warnings (16.10s); 247 unit tests passed; fixtures passed at 27110/77293 (411.51s), unchanged from baseline; golden AST and golden lines each passed 77903/77903. Doc tests completed with one existing ignored test. Logs: /tmp/struct5-final-clippy.log and /tmp/struct5-final2-tests.log.

Observed final output:

```text
Finished `dev` profile [unoptimized + debuginfo] target(s) in 16.10s
cases passing: 27110/77293
golden ast passing: 77903/77903
golden lines passing: 77903/77903
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 28.07s
test result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

- Temporary standalone Rust probes against the final library passed 210 comparisons generated from observed real Python results, covering conditions, signed targets, logical assignments, profile differences, unterminated strings, Rc identity, replacement metadata, parameter shadowing and semicolon handling. Log: /tmp/struct5-probe-results.log. Probe sources and runner: /tmp/struct5_probe_extra.py, /tmp/struct5_probe.rs, /tmp/struct5_run_probe.py. No repository test files added.

Deviations/integration notes:

- Shared constants private to part9/part8 are repeated at use sites with identical values; parser markers are imported.
- parser::tokenize_for_parser converts lexer tokens to the existing syntax::Token type required by parser helpers.
- _invalid_assignment represents Everything by collecting every LHS identifier into its operand set; membership is equivalent for that finite token stream. Scanner dependencies still require integration.
- Identity-sensitive siblings take the original Rc references. Inline-condition replacement uses syntax::replace_stmt to match dataclasses.replace, including dropping non-field metadata. No AST fields or existing public signatures changed.
- Module-level clippy::doc_markdown allowance preserves the required literal one-line port comments. Scoped items_after_statements/too_many_arguments allowances retain the Python nested-function structure. __contains___with needs non_snake_case allowance for the mandated sibling naming convention.

<oai-mem-citation>
<citation_entries>
MEMORY.md:186-186|note=[Prior inline If scope checked against the current Python specification]
</citation_entries>
<rollout_ids>
01a17757-e869-78d3-8cd9-b1a5e1072826
</rollout_ids>
</oai-mem-citation>
