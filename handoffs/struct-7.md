Task struct-7: Python checker/structure.py lines 4452–5251 translated into src/checker/structure/part7.rs. Work is uncommitted. Read AGENTS.md, the full assigned Python range, the two requested example ports, and relevant sibling handoffs. Edited only part7.rs and this explicitly required HANDOFF.md. No git operations.

Fully ported:
- check_missing_param_followup_with; _optional_header_error; _check_call_args; _report_call_arg_names_with.
- LinkContext; _call_link_name; _bare_sub_link_name_with; _unlinked_call_with; _LINK_TOLERATED; _link_step_runs_with; commit_deferred_sub_error_with; finalize_call_links_with.
- _check_call_pass2_with; _over_indexed_target_with; _check_assignment_pass2_with.
- _check_declaration; _first_dim_text; _report_numeric_sub_name_with; _CONST_TYPES; _bad_const_type; _local_const_conflicts; _check_duplicate_declaration_with.
- _illegal_dim_message_with (legacy entry delegates with v=None); _check_array_dims_with; _check_var_dims_with; _split_top.
- All assigned regex patterns, constants and diagnostic strings are translated in source order. Tests remain inside the permitted source file.

Not yet ported / partial:
- No assigned implementation remains unstarted; translated through Python line 5251.
- Incomplete legacy signatures retain their todo!() bodies as instructed. Integrators must use the full siblings below. Existing callers were not edited.
- Runtime paths reaching the four other-file stubs below remain blocked by those owners' implementations. Passing gates do not establish full checker integration.

Other-file functions called that are still stubs (signatures already usable):
```rust
// checker/instr_rules/part1.rs
_real_params(params: Vec<i64>) -> Vec<i64>
_required_count(params: Vec<i64>) -> i64
// checker/structure/part8.rs
_group_end(text: &str, open_at: i64) -> Option<i64>
_group_items(text: &str, open_at: i64) -> Option<Vec<String>>
```
Implemented dependencies are called through their full variants where available: context, expressions parts 1/2/4, symbols, ess, and structure parts 1/2/4. No other-file function was reimplemented.

Signature problems and supplied full interfaces:
Aliases below are explanatory only: Names=BTreeSet<String>, Kinds=OrderedMap<String,String>.
```rust
check_missing_param_followup_with(program: &ProgramRef, profile: &'static Profile, diags: &mut Diagnostics)
_report_call_arg_names_with(stmt_ref: &StmtRef, known: &KnownNames, profile: &Profile, diags: &mut Diagnostics, is_function: bool, ds: Option<&DimsState>)
_bare_sub_link_name_with(text: &str, subs: &Names) -> Option<String>
_unlinked_call_with(stmt: &Stmt, link: &str, profile: &Profile, diags: &mut Diagnostics, before_beginprog: bool, ctx: &LinkContext) -> Option<i64>
_link_step_runs_with(profile: &Profile, diags: &Diagnostics, pass1_errors: bool) -> bool
commit_deferred_sub_error_with(links: &CallLinks, profile: &Profile, diags: &mut Diagnostics)
finalize_call_links_with(links: &CallLinks, profile: &Profile, diags: &mut Diagnostics, pass1_errors: bool)
_check_call_pass2_with(stmt_ref: &StmtRef, subs: &Names, profile: &Profile, diags: &mut Diagnostics, before_bp: bool, in_body_block: bool, sub_params: Option<&OrderedMap<String,SubDef>>, known: Option<&KnownNames>, link_ctx: Option<&LinkContext>, ds: Option<&DimsState>) -> Option<i64>
_over_indexed_target_with(target: &Index, ds: Option<&DimsState>) -> bool
_check_assignment_pass2_with(stmt_ref: &StmtRef, known: &KnownNames, diags: &mut Diagnostics, before_bp: bool, in_body_block: bool, profile: Option<&Profile>, ds: Option<&DimsState>) -> Option<i64>
_report_numeric_sub_name_with(stmt: &SubDef, value: Option<&ConstValue>, diags: &mut Diagnostics)
_check_duplicate_declaration_with(stmt: &Stmt, declared: &mut Kinds, diags: &mut Diagnostics, profile: Option<&Profile>) -> Option<i64>
_illegal_dim_message_with(stmt: &Stmt, v: Option<&VarDecl>) -> String
_check_array_dims_with(stmt: &Declaration, diags: &mut Diagnostics, env: &Env, profile: Option<&Profile>) -> Option<i64>
_check_var_dims_with(stmt: &Declaration, v: &VarDecl, diags: &mut Diagnostics, env: &Env)
```
- Original public parameter and return types are unchanged. Full call/assignment siblings require the original Rc statement so DimsState.before uses stmt_id rather than the address of a clone.
- KnownNames retains tables, dotted_tables and out_of_scope metadata; a plain set can be wrapped with empty metadata.
- LinkContext supplies public subs: Names, has_routines: bool, links: Rc<RefCell<CallLinks>>. Share the original links Rc when constructing the context; do not copy link state.
- The interfaces requested by struct-3 and the LinkContext fields requested by struct-4 are supplied. No syntax.rs fields or other signatures were changed.

Last observed validation:
- Baseline: PATH=/opt/homebrew/opt/rustup/bin:$PATH cargo test --test fixtures -- --nocapture: exit 0; fixture files fully passing: 27/613; cases passing: 27477/77293; cases equal to reference stdout and exit (python recording): 71524/77293; test verdict_fixtures ... ok; 1 passed, 0 failed; finished in 287.74s.
- Read-only real-Python probes supplied 196 expectations embedded in two focused Rust tests. PATH=/opt/homebrew/opt/rustup/bin:$PATH cargo test --lib checker::structure::part7::tests -- --nocapture: exit 0; 2 passed, 0 failed, 262 filtered out; finished in 0.28s. Covers all four profiles for dimensions, assignments, call argument names/counts, duplicate declarations and call placement; also link-name regexes, Latin-1 splitting, Const types/conflicts and numeric routine names.
- Earlier focused test reached the existing part8::_group_items todo!(). Its 12 dependency-blocked cases were replaced with declared-function cases that exercise the implemented tail paths. No dependency implementation was copied into part7.
- Latest full gate is running serially: cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test -- --nocapture. fmt and Clippy passed, exit 0 (Finished dev profile in 4.02s); tests are still running. Output: /tmp/cx-struct-7-gate-tests.log. No after-fixture count observed yet.
- One earlier Clippy attempt failed on redundant raw-string hashes in test data; corrected before the latest passing Clippy run.

Deviations / integration notes:
- Full siblings add the omitted profile, env, ds, value, subs and context parameters, mutable declared-kind state, and identity-preserving Rc statement inputs. Legacy incomplete entry points remain stubs by task convention.
- _CONST_TYPES has the exact Python values locally because expressions does not yet export that constant. Missing instr_rules::_TOLERANT_CODES is represented by its exact seven Python values at the membership check. Integrators should import these from their owners when exposed.
- Dynamic call-head regexes use a LazyLock-protected pattern cache; each exact escaped-name pattern is compiled once through pyre. Other regexes are LazyLock statics.
- _illegal_dim_message_with locates v by its original VarDecl pointer. All internal callers pass the original Rc-backed variable; callers must preserve that invariant. Python uses list.index, which can also accept a structurally equal copy; supporting external copies would need a shared AST equality API, absent here.
- Link state uses Rc<RefCell<CallLinks>> to retain Python's shared mutable object. Program and statement identities are retained in full APIs; no AST field additions or unsafe code.
