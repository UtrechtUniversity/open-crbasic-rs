# struct-3 handoff

The port remains partial at the dependency-result and AST-mutation blockers listed below. Work is uncommitted. Implementation edits are confined to `src/checker/structure/part3.rs`; this handoff is the task's explicitly required additional file. No Git commands were run.

## Fully ported

- `_DO_ALIASES` (Python 2020), insertion-ordered.
- `_expr_resolves_with` (2023–2040), including recursive resolution and function/variable shadowing. Original incomplete `_expr_resolves` stub retained.
- `_slowsequence_resolves_with` (2043–2051); original signature delegates with the Python default empty function set.
- `open_program_at_slowsequence_with` (2054–2081), retaining the original program Rc on unchanged paths. Original signature delegates. Runtime execution still depends on the other-file `_keyword` stub.
- Added a focused Rust test covering 12 expression/SlowSequence shapes, function shadowing, and a non-instruction statement, using outcomes observed from the real Python code read-only.

## Remaining / exact stop points

No assigned function remains unstarted. `check_pass1_with` control flow is translated through the end of the function (1296–2017), including all regexes and diagnostic strings. Original incomplete `check_pass1` stub retained. The full sibling has explicit unresolved dependency-result bridges at Python 1422 (`_alias_parameter_declaration`), 1737 (`_instruction_arg_findings`), and 1876 (`_alias_string_slots`): their current Rust stubs return `()`, making faithful use of their outputs impossible. Each bridge calls the existing stub and then an explicit unimplemented result adapter; it never fabricates a result.

Python 1698 truncates the variable list of an existing shared declaration. Rust has `Declaration.vars: Vec<Rc<VarDecl>>` inside immutable `Rc<Stmt>`. The branch remains an explicit `todo!()` when variables actually need removal. The integrator must supply an identity-preserving interior-mutation API/type change in `syntax.rs`; no field was added here. Earlier `declared_kinds` removals are translated. This function is not fully usable until its other-file dependencies and these blockers are resolved.

## Other-file stub dependencies and required signatures

All entries below are still stubs at inspection. Types are Rust; `Names = BTreeSet<String>`, `Kinds = OrderedMap<String, String>`, `Scope = Option<(Names, Names)>`, `Slots = OrderedMap<String, (Option<i64>, usize)>`. `KnownView` is `expressions::part1::KnownView<'_>` and preserves KnownNames metadata where Python passes a KnownNames object; plain sets can use `KnownView::from_set`. Profile arguments may use `&Profile` unless context requires `&'static Profile`. Add full siblings in owning files without changing legacy signatures.

```rust
// structure/part2.rs
_is_case_else(stmt: &Case) -> bool
_out_of_place(profile: &Profile, text: &str, at_eol: bool) -> String
_keyword(stmt: &Stmt) -> String
_block_word(args: &[Option<ExprRef>]) -> Option<String>

// structure/part5.rs
_unclosed_paren_kind(text: &str, known: &Names) -> String
_is_generic_unparsed(stmt: &Unparsed) -> bool
pass1_operands(program: &Program, profile: &Profile) -> Names
_inline_condition_stmt(stmt: &Stmt, profile: &Profile) -> StmtRef
_pass1_scan(text: &str, operands: &Names, profile: &Profile) -> Option<(String, String)>
_assigns_user_const(stmt: &Assignment, program: &Program) -> bool
_logical_assign_accepted(stmt: &Unparsed, program: &Program, profile: &Profile) -> bool
_unparsed_string_messages(stmt: &Unparsed, program: &Program, profile: &Profile) -> Vec<String>

// structure/part6.rs
_function_syntax_findings(stmt: &Unparsed, program: &Program, profile: &Profile) -> Vec<String>
_routine_arg_findings(stmt: &Unparsed, program: &Program, profile: &Profile) -> Vec<String>
_instruction_arg_findings(text: &str, operands: &Names, profile: &Profile, variables: &Names) -> Vec<(i64, String, String)>
_check_if_pass1(stmt: &Stmt, operands: &Names, profile: &Profile, diags: &mut Diagnostics, program: Option<&Program>)
_for_header_findings(text: &str, profile: &Profile) -> Vec<String>
_block_keyword_call(stmt: &Stmt) -> bool
_assigns_const_index(stmt: &Stmt, consts: &Names, predefined: &Names) -> bool
_pass1_message(profile: &Profile, stmt: &Stmt, kind: &str, operands: Option<&Names>) -> String

// structure/part7.rs
_check_declaration(stmt: &Declaration, diags: &mut Diagnostics) -> i64
_report_numeric_sub_name(stmt: &SubDef, value: Option<&ConstValue>, diags: &mut Diagnostics)
_bad_const_type(stmt: &ConstDecl) -> Option<String>
_local_const_conflicts(stmt: &Stmt, declared: &Kinds) -> Vec<String>
_check_duplicate_declaration(stmt: &Stmt, declared: &mut Kinds, diags: &mut Diagnostics, profile: Option<&Profile>) -> Option<i64>
_check_array_dims(stmt: &Declaration, diags: &mut Diagnostics, env: &Env, profile: Option<&Profile>) -> Option<i64>

// structure/part8.rs
_check_member_types(stmt: &Declaration, structs_seen: &Names, profile: &Profile, diags: &mut Diagnostics)
_check_header_types(text: &str, stmt: &SubDef, structs_seen: &Names, profile: &Profile, diags: &mut Diagnostics) -> bool
_check_init_bounds(stmt: &Declaration, env: &Env, diags: &mut Diagnostics)
_check_decl_exprs(stmt: &Declaration, known: &KnownView, variables: &Names, profile: &Profile, diags: &mut Diagnostics, struct_types: &Names, stop_at: Option<i64>, length_scope: &Scope, indexed_constants: &Names) -> Option<i64>
_check_alias_index_names(stmt: &Instruction, known: &KnownView, variables: &Names, profile: &Profile, diags: &mut Diagnostics, check_variables: bool, indexed_constants: &Names) -> bool
_alias_string_slots(program: &Program, env: &Env) -> Slots
_alias_parameter_declaration(routine: &SubDef, env: &Env, declared: &Kinds, known: &Names, forward: &Names, profile: &Profile, diags: Option<&mut Diagnostics>, structs_seen: &Names) -> (Declaration, bool)
_alias_name_collisions(stmt: &Instruction, declared: &Kinds, profile: &Profile, in_routine: bool) -> OrderedMap<usize, String>
```

Implemented dependencies used: context APIs; symbols `check_local_name_collision_with_profile` and `is_reserved_predefined_name_with_profile`; expressions `_undefined_names_with`; structure/part9 `_check_alias_dims_with`, `_junk_init_findings_with`, `_check_unparsed_pass1_with`.

## Assigned signature problems

- `check_pass1_with(program: &ProgramRef, profile: &'static Profile, diags: &mut Diagnostics, env: Option<&Env>)` supplies omitted profile/env and preserves program identity.
- `_expr_resolves_with(e: Option<&Expr>, declared: &Names, functions: &Names)` supplies omitted function membership.
- `_slowsequence_resolves_with(stmt: &Stmt, declared: &Names, functions: &Names)` supplies omitted function membership.
- `open_program_at_slowsequence_with(program: &ProgramRef, profile: &'static Profile) -> ProgramRef` supplies Rc ownership for identity and the profile lifetime required by context.

## Last observed validation

Final gate passed, exit 0, on the finished source:

```sh
PATH=/opt/homebrew/opt/rustup/bin:$PATH cargo fmt
PATH=/opt/homebrew/opt/rustup/bin:$PATH cargo clippy --all-targets -- -D warnings
PATH=/opt/homebrew/opt/rustup/bin:$PATH cargo test -- --nocapture
```

Observed output:

```text
Finished dev profile ... in 0.05s
248 passed; 0 failed
fixture files fully passing: 26/613
cases passing: 27110/77293
cases equal to reference stdout and exit (python recording): 71524/77293
test verdict_fixtures ... ok
1 passed; 0 failed; ... finished in 414.33s
golden ast passing: 77903/77903
test golden_ast ... ok
1 passed; 0 failed; ... finished in 61.21s
golden lines passing: 77903/77903
test golden_lines ... ok
1 passed; 0 failed; ... finished in 28.75s
Doc-tests: 0 passed; 0 failed; 1 ignored
```

- Fixture before/after N: **27110 → 27110**, out of 77293. Before command: `cargo test --test fixtures -- --nocapture` (passed, 198.15s); after count observed in the full test run with `--nocapture` above. No baseline regression.
- Final `cargo fmt --check` also passed, exit 0.
- Logs: `/tmp/cx-struct-3-before.log`, `/tmp/cx-struct-3-gate-clippy.log`, `/tmp/cx-struct-3-gate-tests.log`.
- Read-only Python probe observed expression and SlowSequence matrix `[True, True, True, True, False, True, True, False, True, False, False, False]`; function shadowing `False`; non-instruction SlowSequence resolution `True`. The focused Rust test passed against these results.
- Passing gates do not demonstrate end-to-end operation of `check_pass1_with`: callers still use legacy stubs, and its listed dependencies remain unresolved.

## Deviations / integration notes

- `pending_call!` invokes existing incomplete stubs and records every unavailable argument after `;`. These arguments are not forwarded until the owning files expose full signatures. Replace these sites with full siblings during integration, particularly the mutable declared-kinds argument, KnownNames metadata, environment, profile, and String-length scope. `pending_result!` additionally marks the three missing return types above.
- Shared declaration mutation is explicitly blocked as described above; no substitute AST identity or unsafe mutation was introduced.
- The legacy opener accepts `&Program`, so it must clone into a new Rc; callers requiring Python identity must use the `_with` entry point. The legacy profile is resolved to its registered static counterpart for context.
- `_CONTROL_KEYWORDS` is private in expressions, and `_END_EXIT_WORDS`/`_FOR_MISSING` are absent from the generated structure/part2 constants. Their exact values are retained locally at use sites; no outside-file code was implemented. `_EXTRA_PREDEFINED_SYMBOLS` and parser `UNCLOSED_PAREN_EXPR` are imported from their owners.
- Doc comments keep the mandated one-line form; local Clippy allowances retain that spelling and Python branch order. No evidence comments were copied.
