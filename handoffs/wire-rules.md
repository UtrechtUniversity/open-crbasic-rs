# HANDOFF wire-rules (src/checker/instr_rules wiring)

## Done
- All 72 `todo!()` stub / `<name>_with` pairs in instr_rules/part1..part5 merged: stub deleted, `_with` renamed to the Python
  name (doc `/// Port of checker/instr_rules.py::<name>`), Python function order verified against instr_rules.py.
- `todo!(` left in src/checker/instr_rules: 0. `_with` functions left there: 0.
- `RuleFn` (part1, re-exported; mod.rs copy removed) is now `fn(&Instruction, &ProgramRef, &'static Profile, &mut Diagnostics, &Env)`;
  `run_rules(phase, program, profile, diags, env)` dispatches the static RULES table (Python definition order) with env; the
  SDI12/PORTBRIDGE special cases are gone. `check_modemcallback_baud` and `check_matrix_destination` got the 5-arg signature.
- Callers updated: src/checker/mod.rs (env added to ~25 calls), warnings.rs (`_scan_interval_ms(stmt, env)`, check_scan_params/
  check_scan_max_interval/check_data_interval get env), intervals.rs `_scan_step_with`, structure/part1.rs `_scan_counts_for_table_placement_with`,
  structure/part6.rs (`shortfall_message(record, given)`), instr_measure/part1.rs (cdm_null_module / cdm_unsupported_module get env).
- Duplicates dropped: `_p3_shortfall_message`, `_p3_earlier_error_in_stmt`, `_p3_precision_positions` (-> part1/part2),
  `_p4_cdm_type_value`, `_p4_f32_is_2_31`, `_p4_serialin_bad_destination`, `_P4_SERIALIN_DEST_MESSAGE` (-> part5/part2/part3),
  `_p5_cdm_null_module`, `_p5_cdm_named_type`, `_P5_CDM_PORT_MODULES` (-> part4, now pub(super)), `_p5_scan_raw`, `_p5_interval_us`,
  `_p5_us_per_unit`, `_p5_unit_key` (-> part2 `_scan_raw`, `_interval_us`, `_US_PER_UNIT`), part2's local SNMP constants and
  `snmp_components_result` (-> part1). `_p5_cdm_bridge_option_args` renamed `_cdm_bridge_option_args` (home is part5).
- context.rs: `Context::float_names()/literal_names()` call `longconv::float_names_in/literal_names_in(&program.statements)`.

## Deviations
- part5 `check_avw200_scan_time` now uses part2 `_interval_us` (f64) instead of the private exact-int copy: Python's int
  `value*us_per_unit` is exact, f64 differs only above 2^53 us (~285 years). Judged immaterial; the float branch of the old copy is the same arithmetic.
- `_p4_f32_is_2_31(f64)` replaced by `_f32_is_2_31(&ConstValue::Float(x))` (same result for finite and non-finite inputs).
- `thread_local RULES` / `rule` decorator in part1 kept (runtime additions are dispatched after the static table, as before).
- mod.rs allows `clippy::doc_markdown` (the required unbackticked `/// Port of checker/instr_rules.py::name` docs).
- Private `_p3_*`/`_p4_*` Rust helpers that are not copies of a Python function (`_p3_err`, `_p3_find_stmt`, `_p3_known_view`,
  `_p4_known_view`, `_p4_find_instr_stmt`, `_p4_index_value`, `_p4_flat_offset`, `_p3_f64`, `_p3_float32`) remain; `_p3_rule_env` is unused.

## Remaining / caller TODOs (outside instr_rules, not edited)
- structure/part2.rs, structure/part4.rs: not touched (forbidden); they may still call old names - they compile.
- cr3xx/part2.rs (local shims `shortfall_message`, `given_count_of`, `_next_simple_token`, `given_count`, `_required_count`, `_real_params`,
  `_SERIALIN_DEST_MESSAGE`; TODO(integrator-2)) should now call `instr_rules::{shortfall_message, given_count_of, _next_simple_token,
  given_count, _required_count, _real_params, _SERIALIN_DEST_MESSAGE}`.
- instr_data/part2.rs `_real_params_local`, instr_measure/part2.rs `scan_raw_local`, scan_time.rs `scan_raw`: can use `instr_rules::{_real_params,_scan_raw}`.
- expressions/part3.rs:~1617 `matrix_undefined_operands` stub `todo!("wire instr_rules::matrix_undefined_operands_with")`: call
  `instr_rules::matrix_undefined_operands(stmt, program: &ProgramRef, env, known, profile: &'static Profile)` (needs ProgramRef).
- check_spi_open_order/check_pulsecount_channels/check_samplefieldcal/check_return_placement/check_too_many/etc. were already complete and unchanged.

## Gate (observed)
`cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` all green; unit tests 278 passed.
Fixtures: `fixture files fully passing: 53/613`, `cases passing: 34492/77293` (start 28310 / 30 files), equal to reference 71524/77293.
