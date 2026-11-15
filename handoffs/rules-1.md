Task rules-1: checker/instr_rules.py lines 1–765 ported into src/checker/instr_rules/part1.rs. Work is uncommitted. Edited only part1.rs and this explicitly requested HANDOFF.md. No git operations, commits, branches, AST changes, or signature changes in other files.

1. Fully ported

rule_with, run_rules_with, unterminated_string_shape, _function_call_string, user_functions, _past_last_param, _check_unterminated_string_param_with, _real_params, _required_count, misplaced_returns_with, check_return_placement, _return_statements_with, check_with, given_count, given_count_of, shortfall_message_with, _report_constant_destinations_with, _next_simple_token, _too_many_eligible, plain_expression_params, check_too_many, _earlier_error_in_stmt_with, check_essinitialize_too_many, _snmp_components_with.

All assigned constants and regexes are present in source order. Regex patterns and diagnostic strings match Python. Inline Return identity uses original Rc statement pointers. Shared context and KnownNames metadata are preserved.

2. Not yet ported / partial

No assigned function body remains unstarted or partial; translated through Python line 765. Legacy entry points with incomplete inputs/returns intentionally remain todo: rule, run_rules, _check_unterminated_string_param, check, shortfall_message, _report_constant_destinations, _earlier_error_in_stmt. Their full implementations are the _with siblings below. Legacy unit-return misplaced_returns, _return_statements and _snmp_components delegate and discard results; use their _with siblings to obtain values.

All required gates passed after the final change. No assigned implementation remains pending. Runtime paths through other-file stubs remain blocked by their owners; passing gates do not establish complete checker integration.

3. Other-file functions called that remain stubs

The existing parent RULES registry invokes these legacy callbacks:
- instr_rules/part2: check_modemcallback_baud.
- instr_rules/part3: check_precision_variable, check_vibrating_wire_diagnostics, check_sdm_address, check_avgrun_options, check_serialbrk_comport, check_serialin_destination, check_serialin_comport, check_debug_args.
- instr_rules/part5: check_matrix_destination.

Their current callable signatures are fn(&Instruction, &Program, &Profile, &mut Diagnostics). Full signatures needed for env-aware registration:
```rust
fn(stmt: &Instruction, program: &ProgramRef, profile: &'static Profile,
   diags: &mut Diagnostics, env: &Env)
```
They are called through existing signatures because no full siblings exist. Implemented part4::check_sdi12_nan_fill_with and check_portbridge_array_with receive the original ProgramRef, profile and env. Other direct dependencies used here are implemented; none was reimplemented locally. The existing parent PRECISION_VARIABLE_KEYWORDS supplies the precision-destination membership set.

4. Signature problems and full interfaces

All existing public signatures are preserved. New full interfaces:
```rust
pub type RuleFn = fn(&Instruction, &ProgramRef, &'static Profile, &mut Diagnostics, &Env);
rule_with<'a>(phase: &'a str, keywords: &'a [&'a str], single_pass: Option<bool>)
    -> impl Fn(RuleFn) -> RuleFn + 'a
run_rules_with(phase: &str, program: &ProgramRef, profile: &'static Profile,
    diags: &mut Diagnostics, env: &Env)
_check_unterminated_string_param_with(program: &ProgramRef, profile: &'static Profile,
    diags: &mut Diagnostics)
misplaced_returns_with(program: &Program) -> OrderedMap<usize, bool>
_return_statements_with(program: &Program) -> Vec<StmtRef>
check_with(program: &ProgramRef, profile: &'static Profile, diags: &mut Diagnostics)
shortfall_message_with(record: &crate::profiles::Instruction, given: i64) -> Option<String>
_report_constant_destinations_with(stmt: &Instruction, record: &crate::profiles::Instruction,
    params: &[i64], program: &ProgramRef, profile: &'static Profile, diags: &mut Diagnostics)
_earlier_error_in_stmt_with(program: &Program, stmt: &StmtRef, diags: &Diagnostics,
    prefixes: &[&str]) -> bool
_snmp_components_with(text: &str) -> Option<Vec<Option<i64>>>
```
rule lacks keywords/decorator return; run_rules lacks env; misplaced_returns, _return_statements and _snmp_components lack returns; shortfall_message lacks record; _report_constant_destinations lacks record/params/program; _earlier_error_in_stmt lacks stmt/prefixes. check and _check_unterminated_string_param require owning ProgramRef and static Profile for context::of and pass1_operands_with; borrowed legacy inputs cannot recover original program identity. The new RuleFn is part1::RuleFn; the parent module's existing legacy RuleFn remains unchanged.

The APIs requested by sibling handoffs are supplied: _real_params(Vec<i64>) -> Vec<i64>, _required_count(Vec<i64>) -> i64, _next_simple_token(&str) -> String, plain_expression_params(Vec<i64>) -> bool, and shortfall_message_with(record, given).

5. Last observed validation

- Baseline: PATH=/opt/homebrew/opt/rustup/bin:$PATH cargo test --test fixtures -- --nocapture: exit 0; fixture files fully passing: 27/613; cases passing: 27477/77293; cases equal to reference stdout and exit (python recording): 71524/77293; test verdict_fixtures ... ok; 1 passed, 0 failed; finished in 141.82s. Log: /tmp/rules1-baseline.log.
- cargo check -q: exit 0, no output.
- Real Python read-only probes supplied expected outputs embedded in three Rust tests: 72 profile/program rows, 26 shortfall rows, text/parameter/SNMP matrices, rule ordering and profile gates, and nine statement diagnostic ownership cases. Initial focused run: 2 passed, 0 failed, 268 filtered out, finished in 0.58s. All three final tests passed in the full suite.
- First full gate: cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test -- --nocapture: exit 0. Clippy finished in 17.36s; 271 unit tests passed; fixtures passed with 28/613 files fully passing and cases passing: 28040/77293 (baseline 27477/77293), finished in 118.16s. AST golden: 77903/77903, 33.93s. Source-line golden: 77903/77903, 17.40s. Doctests: 0 failed, 1 ignored. Log: /tmp/rules1-gate-tests.log.
- Final gate after numeric-helper cleanup: PATH=/opt/homebrew/opt/rustup/bin:$PATH for cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test -- --nocapture: exit 0. Formatting and Clippy passed (3.34s); unit tests: 271 passed, 0 failed, 0.56s. Fixtures: 28/613 files fully passing; cases passing: 28040/77293, up 563 from baseline 27477/77293; 1 passed, 0 failed, 76.44s. AST golden: 77903/77903, 28.72s. Source-line golden: 77903/77903, 16.93s. Doctests: 0 failed, 1 ignored. Log: /tmp/rules1-final-gate-tests.log.
- Earlier compile/Clippy failures were corrected before the passing checks: unavailable get_mut/from_known APIs, redundant raw-string hashes, underscore parameter names, semicolons, preserved Vec signatures, and mutation through Rc in a test.


Final gate output tail:
```text
golden lines passing: 77903/77903
test golden_lines ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 16.93s
Doc-tests opencrbasic
test src/pyre.rs - pyre (line 12) ... ignored
test result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s
all doctests ran in 0.51s; merged doctests compilation took 0.34s
```

6. Deviations / integration limits

- _snmp_components_with represents Python's mixed integer/empty-string tuple as Vec<Option<i64>>. Inner None is the empty string and remains distinct from integer zero; outer None means rejected components.
- part1::RULES stores runtime decorator additions in a thread-local insertion-ordered map. Dispatch also uses the existing parent module static registrations in their existing order. This accommodates the already-created parent registry without editing another file. Legacy stub callbacks cannot receive env until their owners add full siblings; the two implemented part4 callbacks already receive env.
- Context-backed siblings require original Rc program ownership and a static profile reference, following existing context APIs. No synthetic program identity or structural-equality cache is introduced. misplaced_returns uses implemented context::_with_inline to obtain the same flattened statement sequence with original Rc identities.
- Scoped Clippy allowances preserve required one-line doc comments and existing by-value Vec public signatures.
