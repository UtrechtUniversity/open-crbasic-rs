Task rules-2: translated Python checker/instr_rules.py lines 766–1538 into src/checker/instr_rules/part2.rs. Work is uncommitted. Edited only that file and this explicitly mandated handoff. No git operations, branches, commits, or other-file edits.

Fully ported:

- check_modemcallback_baud_with; _SCAN_VALUE_CHECK_LOGGERS; _is_negative_or_invalid_with; check_scan_params_with; _check_slow_scan_interval; _US_PER_UNIT; _scan_min_interval_with; _crem; scan_split_with; _f32_is_2_31_with; _cdiv; check_scan_max_interval_with; _scan_raw_with; _units_key_with; _interval_us_with; _divisible_interval_us_with; check_data_interval_with (including nested table_name); _divisibility_message; _PRECISION_VARIABLE_POSITIONS; _has_call; _stringish; _string_typed_with; check_iif_types_with and its legacy delegate.
- check_snmp_oid_with and check_snmpvariable_too_many have their complete translated algorithms, but paths reaching the dependencies below remain blocked. Legacy check_snmp_oid delegates with Python's default env=None.
- Assigned constants, inline regex patterns, and diagnostic messages are translated. Full source range and relevant sibling handoffs were read before implementation. Requested scan_split_with interface is supplied.

Not yet ported / partial:

- No assigned function remains unstarted; translation reaches Python line 1538.
- SNMP component-dependent execution stops at Python line 782 and the later calls to the same dependency. Local snmp_components_result calls the existing other-file stub, then explicitly stops because its unit return cannot expose Python's tuple. Replace this adapter when the dependency supplies its return value.
- Incomplete legacy signatures retain their original todo! bodies as instructed. Existing env-less callers and the four-argument RuleFn dispatcher require integration with the full interfaces below; callers were not edited.

Other-file stub dependencies called:

- instr_rules/part1::_snmp_components(text: &str) -> () is still a stub. Need _snmp_components_with(text: &str) -> Option<Vec<Option<i64>>>; a None component represents Python's empty string, while the outer None means an unusable OID.
- instr_rules/part1::_next_simple_token(text: &str) -> String is still a stub.
- All other directly called dependencies have implementations; full variants are used where available. No other-file function was reimplemented.

Signature problems / supplied full interfaces:

```rust
check_snmp_oid_with(program: &Program, profile: &Profile, diags: &mut Diagnostics, env: Option<&Env>)
check_modemcallback_baud_with(stmt: &Instruction, program: &Program, profile: &Profile, diags: &mut Diagnostics, env: &Env)
_is_negative_or_invalid_with(value: &ConstValue) -> bool
check_scan_params_with(program: &Program, profile: &Profile, diags: &mut Diagnostics, env: &Env)
_scan_min_interval_with(stmt: &Instruction, diags: &mut Diagnostics, env: &Env) -> Option<(i64, i64)>
scan_split_with(stmt: &Instruction, env: &Env) -> Option<(i64, i64)>
_f32_is_2_31_with(value: &ConstValue) -> bool
check_scan_max_interval_with(program: &Program, profile: &Profile, diags: &mut Diagnostics, env: &Env)
_scan_raw_with(stmt: &Instruction, env: &Env) -> Option<(i64, i64)>
_units_key_with(units_arg: &Expr, env: &Env, truncate: bool) -> Result<Option<ConstValue>, NotConstant>
_interval_us_with(interval_arg: Option<&Expr>, units_arg: Option<&Expr>, env: &Env, truncate_units: bool) -> Option<f64>
_divisible_interval_us_with(interval_arg: Option<&Expr>, units_arg: Option<&Expr>, env: &Env, truncate_units: bool) -> Option<f64>
check_data_interval_with(program: &Program, profile: &Profile, diags: &mut Diagnostics, env: &Env)
_string_typed_with(expr: Option<&Expr>, typer: &Typer, str_consts: &BTreeSet<String>, profile: &Profile) -> Option<bool>
check_iif_types_with(program: &ProgramRef, profile: &'static Profile, diags: &mut Diagnostics)
```

Last observed validation, with /opt/homebrew/opt/rustup/bin on PATH:

- Before: cargo test --test fixtures -- --nocapture passed. Tail: fixture files fully passing: 27/613; cases passing: 27477/77293; cases equal to reference stdout and exit (python recording): 71524/77293; test result: ok. 1 passed; 0 failed; finished in 149.56s. Log: /tmp/rules2-before.log.
- Real read-only Python supplied 172 recorded cases across all four profiles, embedded in the permitted Rust file. Focused matrix passed. The final full test run also passed this matrix. Covers Scan arithmetic and diagnostic ordering, slow scans, table divisibility, modem baud rates, IIF types/omitted arms, and SNMP paths independent of the component helper.
- Final gate cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test -- --nocapture exited 0. Clippy tail: Finished dev profile in 0.02s. Test log: /tmp/rules2-final-source-tests.log.
- Final-source tests: 269 unit tests passed, 0 failed (0.36s); fixtures passed, 27/613 files fully passing, cases passing: 27621/77293 (up 144), equal to reference stdout and exit: 71524/77293, 1 passed/0 failed (92.30s); golden ast passing: 77903/77903 (16.37s); golden lines passing: 77903/77903 (10.62s). Final tail: test result: ok. 0 passed; 0 failed; 1 ignored; finished in 0.00s; all doctests ran in 0.27s; merged doctests compilation took 0.11s.
- Final cargo fmt --check also exited 0. No baseline fixture regression.

Deviations / integration notes:

- Part1's SNMP constants/regexes are absent from its generated Rust file, and var_types::SNMP_RE is private. Their exact Python values/patterns are temporarily local; import them from their owners when exported.
- Original public signatures are unchanged. The full IIF entry accepts the original ProgramRef to preserve context identity. Its legacy wrapper clones Program while sharing statement Rc values; integrators should use the full entry.
- ConstValue has no Python bool variant; shared evaluator normalization is retained. No AST fields were added. Numeric/key helpers handle Rust ConstValue variants; _cdiv derives C truncation from pystr::divmod_i64, avoiding abs(i64::MIN). Membership-only sets use BTreeSet; ordered output maps use OrderedMap.
- Focused modem tests parse their input with CR6 before exercising each target profile's rule because unsupported-profile Rust parsing does not produce Instruction. All Python-recorded expectations remain unchanged.
- Initial compile/lint/test-setup findings were corrected. Narrow lint allowances preserve Python branch order and exact float comparisons. Passing gates do not establish execution of dependency-blocked SNMP paths or integration of env-bearing siblings into existing callers.
