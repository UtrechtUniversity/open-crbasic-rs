# HANDOFF rules-3 (checker/instr_rules.py 1539-2311 -> src/checker/instr_rules/part3.rs)

## 1. Fully ported
All 21 Python functions of the range, plus the range's constants (`_SDM_ADDRESS_ARG`, `_AVGRUN_TYPE_ERROR`, `_RUNNING_AVG`, `_INSTANCES_AT` as `_instances_at`, `_CDM_SETTLING_ARG`, `_ILLEGAL_MATRIX`, `_UNDEFINED_MATRIX`, `_SERIALIN_PORTS` as `_serialin_ports`, `_SERIALIN_DEST_MESSAGE`, `_DEBUG_HISTORY_LIMIT`, `_DEBUG_ARG_TYPES` inlined) and the two regexes:
check_precision_variable, check_vibrating_wire_diagnostics, check_sdm_address, _operator_tree, check_avgrun_options, check_option_values, _cdm_delay_overlong, _matrix_shapes, matrix_undefined_operands, _matrix_dimension_errors, check_const_int_args, _strip_parens, _paren_depth_ok, _is_integer_token, check_serialbrk_comport, _serialin_bad_destination, check_serialin_destination, check_serialin_comport, check_serialinrecord, debug_history_oom, check_debug_args.

## 2. Not ported / partial
None.

## 3. Other-file functions needed that are stubs / missing
Called as-is (complete signatures, still `todo!()` in part1/part2): `part1::given_count`, `_required_count`, `_real_params`, `_next_simple_token`; `part2::_cdiv`, `_crem`, `_has_call`, `_stringish`.
Private `todo!()` placeholders in part3 (stub signatures incomplete; replace with the real calls when ported):
- `_p3_shortfall_message(record: &profiles::Instruction, given: i64) -> Option<String>` (part1 `shortfall_message` lacks `record`) - used by check_serialin_comport_with.
- `_p3_earlier_error_in_stmt(program: &Program, stmt: &StmtRef, diags: &Diagnostics, prefixes: &[&str]) -> bool` (part1) - used by check_const_int_args_with (CR6/CR1000X).
- `_p3_precision_positions(kw)`: local copy of part2's constant `_PRECISION_VARIABLE_POSITIONS` (ADDPRECISE/MOVEPRECISE -> {0}); replace by the part2 constant.

## 4. Signature problems in stubs
Every stub that lacked `env` (or other params) kept its signature and gained a `*_with` sibling (`stmt, program: &ProgramRef, profile: &'static Profile, diags, env: &Env`). The original rule-fn stubs (the ones in `mod.rs` RULES) delegate: they rebuild `Rc<Program>` from `&Program`, take env from `context::of(..).env()` and call the `_with`. Replace by the real pipeline env when wiring.
- `_cdm_delay_overlong_with(args: &[Option<ExprRef>], env, profile: &Profile, names, literals) -> i64` (original stub still `todo!()`)
- `_matrix_shapes_with(program: &ProgramRef, env: &Env) -> OrderedMap<String,(i64,i64)>` (stub `todo!()`)
- `matrix_undefined_operands_with(stmt: &Instruction, program: &ProgramRef, env: &Env, known: &BTreeSet<String>, profile: &'static Profile) -> Option<Vec<String>>` (stub `todo!()`)
- `_matrix_dimension_errors_with(option: i64, args: &[Option<ExprRef>], shapes: &OrderedMap<String,(i64,i64)>) -> Vec<String>` (stub `todo!()`)
- `_is_integer_token_with(text: &str, arg: &Expr, env: &Env) -> bool` (stub `todo!()`)
- `debug_history_oom_with(stmt, env: &Env, profile: &'static Profile, program: &ProgramRef) -> bool`; the original `debug_history_oom(stmt, profile, program)` delegates.
- `check_option_values_with(program: &ProgramRef, profile, diags, env)`, `check_const_int_args_with`, `check_serialinrecord_with` likewise.
Complete stubs implemented in place: `_operator_tree`, `_strip_parens`, `_paren_depth_ok`, `_serialin_bad_destination`.

## 5. Gate
`cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test -- --nocapture`: all passed (lib 265 passed, fixtures test ok). `cases passing: 27555/77293` (start 27477).

## 6. Deviations
- Python `bool` constant values never occur in Rust `ConstValue` (TRUE is Int(-1)), so the `isinstance(value, bool)` branches vanish.
- Python code that would raise (int() of inf/nan in `_cdm_delay_overlong`, empty dims in `_matrix_shapes`, missing `statement_scopes` key) returns 0 / uses 1 / uses an empty set instead of crashing.
- `check_serialinrecord`: a BigInt constant (beyond i64) reads as non-constant.
- Statement identity (`id(stmt)`) is found by pointer equality among `program.statements` (top-level only); if absent, check_avgrun_options returns early.
- Delegating stubs rebuild `Rc<Program>` per call (O(n) per statement); fine for wiring-time replacement.
