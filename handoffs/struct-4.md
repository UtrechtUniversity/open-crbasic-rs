Task struct-4: INCOMPLETE; work is uncommitted. Read AGENTS.md and all Python lines 2082-2868 before writing. Edited only src/checker/structure/part4.rs and this explicitly requested handoff; no git commands used.

1. Fully ported: CallLinks fields; _CLOSER_MESSAGES; _names_unresolved (plus _with accepting KnownView); _undefined_table_source_with; existing _unparsed_opener and _leftover_first_name_known verified against the Python source. Implementations written for check_end_blocks, nested_table_with and check_block_closers_with, subject to the dependencies below.

2. Not ported: check_pass2 (Python definition 2224, executable body starts 2270); original todo retained. Other incomplete-signature entry points retain original todos as instructed: nested_table, check_block_closers, _undefined_table_source. The new _with variants implement their full parameters. Next work is check_pass2, starting at Python 2270; it cannot faithfully consume several current other-file stub signatures.

3. Other-file stubs actually called:

- structure/part1::_ends_sub(&Stmt, &str) -> bool: still todo, signature complete.
- structure/part2::_keyword(&Stmt) -> String: still todo, signature complete.
- structure/part2::_block_word() -> Option<String>: still todo; NEED _block_word_with(args: &[Option<ExprRef>]) -> Option<String>. Current calls use existing stub per task instructions, so two-word block terminators need integration after that sibling exists.

All called context, expressions, lexer and instr_data functions are implemented. In particular instr_data::calltable_targets is implemented.

4. Signature problems:

- nested_table needs (program: &ProgramRef, profile: &'static Profile) -> Option<StmtRef>; supplied nested_table_with. ProgramRef and static profile are required by context::of without breaking identity.
- check_block_closers needs (program: &Program, profile: &Profile, diags: &mut Diagnostics); supplied check_block_closers_with.
- _undefined_table_source needs (stmt: &Instruction, profile: &Profile, known: &KnownView<'_>) -> Option<String>; supplied _undefined_table_source_with. KnownView preserves KnownNames table/scope semantics.
- check_pass2 needs (program: &ProgramRef, profile: &'static Profile, diags: &mut Diagnostics, env: Option<crate::consteval::Env>, links: Option<&mut CallLinks>) -> Option<i64> for the actual body. Its Python annotation also permits a (String, i64) stop. Original stub omits profile/env/return and takes immutable links.
- Blocking check_pass2 dependencies: first_misplaced_sub(&Program) currently returns (), needs Option<(StmtRef, Vec<String>)>; _track_tables lacks mutable table state; _calltable_known and _accept_known return () instead of bool and lack table state; _instruction_arg_findings returns () instead of Vec<(i64, String, String)>; LinkContext in part7 has no fields, needs subs, has_routines and mutable CallLinks access. Needed table interfaces are _track_tables_with(&Stmt, &str, &mut TableState), _calltable_known_with(&Stmt, &TableState) -> bool, and _accept_known_with(&Stmt, &TableState) -> bool; TableState must represent the current open name and closed-name set. Needed argument interface is _instruction_arg_findings_with(&str, &BTreeSet<String>, &Profile, &BTreeSet<String>) -> Vec<(i64, String, String)>. These have no _with siblings in this worktree. Their owners must supply these; no outside files were edited.

5. Last observed gates (PATH=/opt/homebrew/opt/rustup/bin:$PATH):

- cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test: PASSED (/tmp/struct4-gate.log). 250 unit tests passed; fixtures passed in 315.41s; golden_ast passed in 124.02s; golden_lines passed in 84.57s. Final tail:
    Doc-tests opencrbasic
    test src/pyre.rs - pyre (line 12) ... ignored
    test result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s
- After normalizing CallLinks.positions to i64, cargo fmt and cargo clippy --all-targets -- -D warnings passed again (Finished dev profile in 10.70s; /tmp/struct4-final-clippy.log), and cargo test --lib passed again (250 passed; 0 failed; finished in 4.00s; /tmp/struct4-final-unit.log).
- Before: cargo test --test fixtures -- --nocapture PASSED; cases passing: 27110/77293; 1 passed; 0 failed; finished in 313.97s (/tmp/struct4-before.log).
- After: cargo test --test fixtures -- --nocapture PASSED; cases passing: 27114/77293; fixture files fully passing: 26/613; 1 passed; 0 failed; finished in 356.93s (/tmp/struct4-after.log). Increase: 4 cases; no baseline regression.

Three focused tests use observed read-only Python probes: opener recognition, first-name-only leftover resolution including Latin-1 text, and Sample source skipping/echo.

6. Deviations: _TABLE_OUTPUT is private in instr_data/part2.rs, so _names_unresolved_with currently mirrors its exact ten keyword members in matches!; expose that constant and import it during integration. WHILE/DO and WEND/LOOP mapping is inlined because _DO_ALIASES has no Rust definition. _block_word arguments cannot yet be passed to its generated stub, as explained above. No Python evidence comments copied; no AST fields added. check_pass2 remains explicitly unimplemented rather than silently dropping its behavior.
