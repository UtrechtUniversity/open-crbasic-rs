Task struct-8: all assigned function bodies translated; final validation in progress; non-finite conversion failure paths also translated. Work remains uncommitted. No git operations performed.

Fully ported bodies: _group_end, _group_items, _report_decl_expr_with, _report_juxtaposed_dim_with, _check_member_types_with, _check_header_types_with, _check_init_bounds_with, _adjacent_literals, _check_decl_exprs_with, _check_alias_index_names_with, _alias_flat_offset, _alias_string_slots_with, _alias_parameter_declaration_with, _alias_name_collisions_with; constants and regexes from Python 5252–5848. Original incomplete public stubs remain unchanged. No assigned function is partial (translation reaches Python line 5848); integration depends on the following other-file stubs.

Other-file stub dependencies:
- structure/part7::_split_top(text: &str, sep: &str) -> Vec<String>, called by grouping and declaration checks.
- structure/part7::_illegal_dim_message(stmt: &Stmt) -> String, called by parameter dimension recovery; Python also has optional v=None, unused here.
Implemented dependencies used: expressions/part1::_undefined_names_with, _variable_refs, expressions/part4::_find_arity_problem_with, structure/part9::_alias_name_lead_with, symbols::is_reserved_predefined_name_with_profile, consteval, lexer.

Signature problems: all generated signatures except _group_end, _group_items, _adjacent_literals, _alias_flat_offset omit arguments and/or return types. Full implementations are named _with; profile, expr, env, tag, length_scope and return values are supplied. KnownView preserves undefined-name metadata. Full siblings (Names = BTreeSet<String>, Kinds = OrderedMap<String, String>, Scope = Option<(Names, Names)>):

```rust
_report_decl_expr_with(expr: Option<&Expr>, echo: &str, known: &KnownView<'_>, variables: &Names, profile: &Profile, stmt: &Stmt, diags: &mut Diagnostics, allow_variable: bool, tag: usize, indexed_constants: &Names) -> bool
_report_juxtaposed_dim_with(expr: &Juxtaposed, echo: &str, known: &KnownView<'_>, variables: &Names, profile: &Profile, stmt: &Stmt, diags: &mut Diagnostics) -> bool
_check_member_types_with(stmt: &Declaration, structs_seen: &Names, profile: &Profile, diags: &mut Diagnostics)
_check_header_types_with(text: &str, stmt: &SubDef, structs_seen: &Names, profile: &Profile, diags: &mut Diagnostics) -> bool
_check_init_bounds_with(stmt: &Declaration, env: &Env, diags: &mut Diagnostics)
_check_decl_exprs_with(stmt: &Declaration, known: &KnownView<'_>, variables: &Names, profile: &Profile, diags: &mut Diagnostics, struct_types: &Names, stop_at: Option<i64>, length_scope: &Scope, indexed_constants: &Names) -> Option<i64>
_check_alias_index_names_with(stmt: &Instruction, known: &KnownView<'_>, variables: &Names, profile: &Profile, diags: &mut Diagnostics, check_variables: bool, indexed_constants: &Names) -> bool
_alias_string_slots_with(program: &Program, env: &Env) -> OrderedMap<String, (Option<i64>, usize)>
_alias_parameter_declaration_with(routine: &SubDef, env: &Env, declared: &Kinds, known: &Names, forward: &Names, profile: &Profile, diags: Option<&mut Diagnostics>, structs_seen: &Names) -> (Declaration, bool)
_alias_name_collisions_with(stmt: &Instruction, declared: &Kinds, profile: Option<&Profile>, in_routine: bool) -> OrderedMap<usize, String>
```

For alias collisions, pass Some(profile) for the profile-bearing call requested by handoffs/struct-3.md; None preserves Python's falsy-profile branch. Diagnostic tags are Rc pointer ids via rc_id.

Last observed validation:

```sh
PATH=/opt/homebrew/opt/rustup/bin:$PATH cargo test --test fixtures -- --nocapture
PATH=/opt/homebrew/opt/rustup/bin:$PATH cargo fmt
PATH=/opt/homebrew/opt/rustup/bin:$PATH cargo clippy --all-targets -- -D warnings
PATH=/opt/homebrew/opt/rustup/bin:$PATH cargo test -- --nocapture
```

- Before fixture replay passed: cases passing: 27477/77293; fully passing files: 27/613; 1 passed; 0 failed; 288.25s. Log: /tmp/struct8-before.log.
- Final cargo fmt passed. Final Clippy passed: Finished dev profile in 1.01s. Log: /tmp/struct8-gate-clippy.log.
- Final full test gate is running. Observed so far: 266 unit tests passed, 0 failed, 0.23s; all four focused part8 tests passed. Final fixture replay passed: cases passing: 27477/77293; fully passing files: 27/613; 1 passed; 0 failed; 226.90s. Before/after N: 27477 -> 27477. Golden AST/line replay is pending. Log: /tmp/struct8-gate-tests.log.
- Earlier full test runs passed (before the final non-finite error-path change), with fixtures unchanged at 27477/77293, golden ASTs 77903/77903 and golden lines 77903/77903. These are not substituted for the pending final-source gate.
- Read-only Python probes supplied focused expected results for Latin-1 grouping, adjacent literals, flat offsets, two-profile diagnostic order, indexed constants, bounds warnings, large numeric dimensions, String slots, and non-finite conversion exceptions. The final source includes four focused tests covering these outcomes.

Deviations / integration notes:
- HANDOFF.md is the explicit requested exception to the one-file edit scope.
- Legacy incomplete stubs retain todo!() as requested; callers must use full siblings during integration.
- Shared private constants _MAX_ARRAY_DIMS=3 and _MAX_DIM_VALUE=2147483647 are used as identical local literals.
- Initializer element-count products saturate at i64::MAX rather than overflow; this preserves comparison to representable initializer lengths. Numeric dimensions outside i64 use signed bounds as comparison sentinels, preserving positive/negative dimension decisions. Read-only Python probes confirmed 10**20 and 1e20 produce no bounds warning, while their negatives produce one. Those cases are added to the focused bounds test.
- Python raises OverflowError/ValueError for non-finite dimensions in initializer bounds checks; Rust preserves the uncaught failure via panic, since the existing public return type is (). A focused test checks both observed failure paths. Parameter dimension evaluation still catches non-finite conversion failure, as Python does.
- Invalid group opening characters return None instead of raising Python KeyError; all specified callers provide '(' or '{'.
