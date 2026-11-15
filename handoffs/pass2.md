Task pass2: COMPLETE; mandatory gate and requested fixture replay green. Edited only src/checker/structure/part4.rs and this root HANDOFF.md. No git commands or commits.

1. Fully ported: check_pass2_with, Python definition 2224 and body 2270-2702, audited line by line to final return at 2702. Existing check_pass2 stub untouched. Fixed OrderedMap consts cloning, Table function-key iteration, Message::format, explicit imports and assigning_clones lint. Preserved early-return cutoff, first-position insertion, statement Rc identity, diagnostic order, supplied-link state and normal-return cutoff clearing.
2. Remaining in assigned function: none. Integration remains outside this task: checker/mod.rs still calls the original check_pass2 stub. No private _p4_* helpers introduced; no existing signature/type/field changed.
3. Dependencies: uses existing full *_with siblings in structure parts 1, 2, 3, 5, 6, 7, 9, plus context, expressions, var_types and instr_rules. The old handoffs/struct-4.md signature blockers have siblings now. Indirect dependencies still need integration in their owners:
   - OBSERVED runtime panic at instr_rules/part2.rs:70 scan_split (env omitted), reached from part1::_scan_counts_for_table_placement_with for Scan(1,Sec,0,0). That helper exists, so this task uses it without duplicating it. Scan(0,...) returns before that dependency.
   - Source inspection: part5 scanner adapters still call part6::_scan_expr, _later_arg_unknown and _bare_function_needs_paren legacy todo stubs, despite available full siblings. Part6::_function_call_scan_with still calls legacy part5::pass1_operands. These indirect paths require owner wiring; not edited here.
   - Existing nested_table_with (outside assigned function) can also encounter these sibling limitations; unchanged.
4. Signature: check_pass2_with(program: &ProgramRef, profile: &'static Profile, diags: &mut Diagnostics, env: Option<Env>, links: Option<&mut CallLinks>) -> Option<i64>. Python annotation also permits a tuple but no path in this function returns one; all stop branches return stmt.line. No signature problem for this body.
5. Validation OBSERVED:
   - Initial clippy failed: 5 wildcard imports and assigning_clones. Corrected; rerun passed (3.15s; /tmp/pass2-clippy2.log).
   - Initial direct tests: 1 passed, 1 failed on the existing scan_split todo (/tmp/pass2-focused.log).
   - Read-only Python probes observed six placement/program-state shapes for all four profiles. The nonzero Scan probe exposed the dependency above; replaced it with zero Scan interval and rechecked all four Python results. Tests cover 24 combinations plus supplied links on early/normal returns.
   - Focused rerun PASSED: 2 tests, 0 failed (/tmp/pass2-focused2.log).
   - Required command cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test --lib PASSED, exit 0. Clippy finished dev in 1.07s; library test result: 279 passed; 0 failed; 0 ignored; finished in 0.21s.
   - Requested fixture command cargo test --test fixtures -- --nocapture 2>&1 | tail -15 run ONCE, with pipefail and OPENCRBASIC_PY supplied only via environment. OBSERVED exit 0; test verdict_fixtures ... ok; 1 passed; 0 failed; finished in 81.20s.
     Exact parity lines:
     fixture files fully passing: 28/613
     cases passing: 28118/77293
     cases equal to reference stdout and exit (python recording): 71524/77293
     No pre-change fixture baseline was run in this task, so no parity improvement claim is made.
6. Deviations: no new Python behavior deviations in this function. Existing sibling stub dependencies and unwired original entry point limit end-to-end validation. Direct tests exercise check_pass2_with itself, while fixture replay exercises current compiler wiring. No Campbell-derived inputs or copied Python evidence comments added.
