Task struct-6: Python lines 3660-4451 ported in src/checker/structure/part6.rs. All requested gates passed. Work is uncommitted. Only that source file and this explicitly requested handoff are edited. No git operations or commits.

1. Fully ported bodies (integration dependencies below remain): _function_call_scan_with, _function_symbol_finding_with, _function_syntax_findings_with, _routine_arg_findings_with, _instruction_arg_findings_with, _symbol_stage, _later_arg_unknown_with, _scan_expr_with, _bare_function_needs_paren_with, _mentions_then_with, _check_if_pass1_with, _for_header_incomplete_with, _merge_members_with, _for_header_parts_with, _for_header_findings_with, _for_header_message, _block_keyword_call, _unclosed_call_message_with, _assigns_const_index, _pass1_message_with. Constants and all three For regexes are present.

2. Remaining assigned function bodies: none. No Python function body is partially written; reached Python line 4451. Original incomplete-signature stubs remain intentionally unchanged.

3. Called other-file dependencies still stubbed (current signatures):

- structure/part5::pass1_operands(&Program) -> BTreeSet<String>; needs profile: &Profile.
- structure/part5::_pass1_scan(&str, &BTreeSet<String>) -> Option<(String,String)>; needs profile: &Profile.
- structure/part5::condition_part(&str) -> Option<String>.
- structure/part5::inline_if_condition(&str) -> Option<String>.
- structure/part5::_is_generic_unparsed(&Unparsed) -> bool.
- structure/part5::_logical_assign_accepted(&Unparsed, &Program) -> bool; needs profile: &Profile.
- structure/part2::_out_of_place(&str, bool) -> String; needs leading profile: &Profile.
- structure/part2::_no_matching(&str, bool) -> String; needs leading profile: &Profile.
- instr_rules/part1::_next_simple_token(&str) -> String.
- instr_rules/part1::plain_expression_params(Vec<i64>) -> bool.
- instr_rules/part1::_real_params(Vec<i64>) -> Vec<i64>.
- instr_rules/part1::shortfall_message(i64) -> Option<String>; needs leading record: &crate::profiles::Instruction.

Implemented dependencies used: expressions::_type_code_kinds, expressions::_index_nodes_with, lexer::tokenize, pystr and pyre.

4. Signature problems: preserve every existing public signature. Each incomplete stub has a sibling _with with all Python parameters and its actual return type. ScanFunctions = OrderedMap<String, profiles::Function>; ScanVerdict = Option<(String,String)>; FunctionCallScan = (Vec<lexer::Token>, BTreeSet<String>, ScanFunctions, ScanVerdict). Scanner operands: Option<&BTreeSet<String>>; None represents Python _Everything, Some represents a declared-name set. _mentions_then_with accepts Option<&Expr> (the call site passes only expressions). The three user-Function helpers take &StmtRef so single-pass visibility uses Rc::ptr_eq; other Unparsed helpers take &Unparsed. Full signatures added:

```rust
_function_call_scan_with(stmt: &StmtRef, program: &Program, profile: &Profile) -> Option<FunctionCallScan>
_function_symbol_finding_with(stmt: &StmtRef, program: &Program, profile: &Profile) -> Option<String>
_function_syntax_findings_with(stmt: &StmtRef, program: &Program, profile: &Profile) -> Vec<String>
_routine_arg_findings_with(stmt: &Unparsed, program: &Program, profile: &Profile) -> Vec<String>
_instruction_arg_findings_with(text: &str, operands: &BTreeSet<String>, profile: &Profile, variables: &BTreeSet<String>) -> Vec<(i64, String, String)>
_later_arg_unknown_with(args: &[Token], operands: Option<&BTreeSet<String>>, functions: &ScanFunctions) -> bool
_scan_expr_with(toks: &[Token], i: i64, operands: Option<&BTreeSet<String>>, functions: &ScanFunctions, assign: bool, start_operand: bool, unclosed: Option<&str>, cond: bool, to_ok: bool, open_fallback: bool, symbol_unknown: bool) -> ScanVerdict
_bare_function_needs_paren_with(name: &str, functions: &ScanFunctions) -> bool
_mentions_then_with(node: Option<&Expr>) -> bool
_check_if_pass1_with(stmt: &Stmt, operands: &BTreeSet<String>, profile: &Profile, diags: &mut Diagnostics, program: Option<&Program>)
_for_header_incomplete_with(text: &str, profile: &Profile) -> Option<(String, bool)>
_merge_members_with(toks: &[Token]) -> Vec<Token>
_for_header_parts_with(text: &str, profile: &Profile) -> Vec<(String, String)>
_for_header_findings_with(text: &str, profile: &Profile) -> Vec<String>
_unclosed_call_message_with(profile: &Profile, text: &str, at_eol: bool, operands: &BTreeSet<String>) -> Option<String>
_pass1_message_with(profile: &Profile, stmt: &Stmt, kind: &str, operands: Option<&BTreeSet<String>>) -> String
```

Original signatures with complete inputs but missing returns (_merge_members, _for_header_parts) also retain stubs and expose _with.

5. Last observed checks:

- PATH=/opt/homebrew/opt/rustup/bin:$PATH cargo check -q: exit 0, no output.
- Baseline cargo test --test fixtures -- --nocapture: exit 0; cases passing: 27110/77293; fixture files fully passing: 26/613; test result: ok. 1 passed; 0 failed; finished in 314.98s.
- Focused cargo test --lib checker::structure::part6::tests -- --nocapture: exit 0; 2 passed, 0 failed, 247 filtered out. Covers 672 Python-observed scanner comparisons and 56 Python-observed For headers, with parts/incomplete/findings checked.
- cargo fmt -- --check: exit 0, no output.
- Earlier Clippy runs failed on doc_markdown, unnecessary_wraps and if_not_else. Scoped allowances retain the required doc comment form and Python control-flow shape.
- Final gate: cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test -- --nocapture: exit 0. Clippy finished without warnings. Unit tests: 249 passed, 0 failed. Fixtures: 1 passed, 0 failed, 414.52s; post-change cases passing: 27110/77293; fixture files fully passing: 26/613. Both fixture counts unchanged. AST golden: 77903/77903 (120.01s). Source-line golden: 77903/77903 (46.46s). Doctests: 0 failed, 1 ignored. Full test log: /tmp/struct6_gate_tests.log.

Final output tail:
```text
golden lines passing: 77903/77903
test golden_lines ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 46.46s
Doc-tests opencrbasic
test src/pyre.rs - pyre (line 12) ... ignored
test result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s
all doctests ran in 0.63s; merged doctests compilation took 0.48s
```

6. Deviations/integration limits:

- Calls to existing other-file stubs use their existing signatures because editing other files is forbidden. Missing profile/record parameters must be wired by their owners; these paths currently panic in the dependencies, as before.
- Shared _BINARY_WORDS, _START_ERROR_WORDS, _BINARY_OPS and ZERO_PARAM_INSTRUCTIONS constants are absent in their owning Rust parts; exact source values are temporarily local, with an integration TODO. No other-file function was reimplemented.
- User Function metadata uses profiles::Function with only nargs meaningful (other fields zero/None), representing Python's {'nargs': len(params)}.
- For-half identity uses Rc<str> and Rc::ptr_eq. Empty and Latin-1 single-character split strings share Rc values to match observed CPython singleton reuse; longer strings retain separate identities. Python probes confirmed both empty and single-space duplicate-half behavior.

The _mentions_then_with API specializes Python dataclass/list traversal to Option<&Expr>, matching its expression-only call site; Expr::children supplies nested fields and list items. No AST fields or other source files were changed.
