# HANDOFF rules-5 (instr_rules.py 3085-3737 -> src/checker/instr_rules/part5.rs)

Only `src/checker/instr_rules/part5.rs` edited (plus this file). Nothing committed.

## 1. Fully ported
Every function of the range. Original stubs kept for incomplete signatures; the real code is in the `*_with` sibling:
- `check_fieldcal_variable_args_with(&ProgramRef, &'static Profile, &mut Diagnostics, Option<&Env>)`
- `_fieldcal_string_subscript_with(Option<&Expr>)`, `_fieldcal_bad_variable_with(Option<&Expr>)`
- `check_matrix_destination` (in place; signature complete, `env` is unused by Python; RULES table unchanged)
- `check_samplefieldcal` (already implemented in place, checked against Python)
- `_cdm_type_value_with(&ConstValue)`, `cdm_null_module_with(&Instruction, &Env)`
- `_check_cdm_bridge_options_with(stmt, (usize, usize), profile, diags, env, names, literals)`
- `_check_cdm_mux_options_with(stmt, module: i64, profile, diags, env, names, literals)`
- `check_cdm_address_types_with(&ProgramRef, &'static Profile, diags, &Env)`
- `check_epi_scan_buffers_with(&ProgramRef, &'static Profile, diags, &Env)`
- `_check_cdm_fft_with(st, us: i64, profile, env, names: (&BTreeSet, &BTreeSet), diags)`
- `_const_int_with(Option<&Expr>, &Env)`
- `check_avw200_scan_time_with(&ProgramRef, &'static Profile, diags, &Env)`
- `check_mqtt_publish_payload_with(&ProgramRef, &'static Profile, diags)`
- `_numeric_expression_with(Option<&Expr>, &OrderedMap<String,String>)`
- `_constant_name_text_with(Option<&Expr>, &Env, depth)`
- `_check_port_index_with(stmt, key, index, cfg: Option<&Json>, env, profile, found)`
- `check_settings_fields_with(&ProgramRef, &'static Profile, diags, found: Option<&mut Diagnostics>, env: Option<&Env>)`

## 2. Not ported / partial
None. The original incomplete stubs (`check_fieldcal_variable_args`, `check_cdm_address_types`, `check_epi_scan_buffers`, `check_avw200_scan_time`, `check_mqtt_publish_payload`, `check_settings_fields`, and the helpers above) still `todo!()`; their signatures cannot carry `&ProgramRef`/`&'static Profile`/`Env`, so callers must use `_with`.

## 3. Other-file functions needed that are stubs / missing
- part2.rs `_scan_raw(stmt, env)` and `_interval_us(interval_arg, units_arg, env, truncate_units)`: no `_with` exists yet, so part5 holds private copies `_p5_scan_raw` and `_p5_interval_us` (truncate_units=false only), plus `_p5_us_per_unit`. Replace with `_scan_raw_with(&Instruction, &Env) -> Option<(i64, i64)>` and `_interval_us_with(Option<&Expr>, Option<&Expr>, &Env, bool)` when part2 has them.
- part4.rs keeps `_cdm_null_module`, `_is_cdm_named_type`, `_CDM_PORT_MODULES`, `_cdm_module_name`, `_p4_cdm_type_value` private; part5 has copies (`_p5_cdm_null_module`, `_p5_cdm_named_type`, `_P5_CDM_PORT_MODULES`). Making the part4 ones `pub` would let part5 drop them.
- `context::Context::float_names()` / `literal_names()` call `longconv::float_names()` / `literal_names()`, which are `todo!()` stubs (they need the statements). I used `longconv::float_names_in(&program.statements)` / `literal_names_in` instead. Context's two cached getters should call the `_in` variants.
- `mode::decide_with_env` and `consteval` used as is; no Python `except Exception` swallow around `mode.decide` (Rust has no exception path).

## 4. Stub signature problems (correct Rust signatures)
RULES table (mod.rs) needs `RuleFn = fn(&Instruction,&Program,&Profile,&mut Diagnostics)`, but the Python rules `check_matrix_destination` and the `check_*` functions called from elsewhere need `env`. Callers with an env must call the `_with` versions listed in section 1. The stubs `_scan_raw`, `_units_key`, `_interval_us` in part2 miss `env` as noted.

## 5. Last gate result
Unit tests: `cargo test --lib instr_rules::part5` -> 3 passed (fieldcal, settings, mqtt, epi, cdm outputs compared with values observed from the Python code).
Gate observed: first full run `cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test` failed clippy on 4 missing `;` in my test module; fixed, then `cargo clippy --all-targets -- -D warnings` finished clean and `cargo test --lib` -> 265 passed. In the first run the cargo test integration targets all passed (fixtures 1 passed) and `cargo test --test fixtures -- --nocapture` printed `cases passing: 27477/77293` (same as the starting 27477: the `_with` functions are not wired into the checker by this task). The complete `cargo test` was not re-run after the semicolon fix (only test-module code changed).

## 6. Deviations
- BigInt numbers (beyond i64): `FieldCal` option treated as huge, `_check_port_index` returns silently on an unparseable index, `_p5_interval_us` returns None above i128 (no diagnostic either way for the last).
- `check_cdm_address_types`: a nan/inf `CDMType` (Python crashes in `int(kind)`) takes the "string or fraction" branch.
- Scan intervals use `saturating_mul` (Python ints are unbounded) in `check_epi_scan_buffers`; thresholds are unaffected.
- `Diagnostics.error` calls use `error_basic` (default kind/phase, as Python's defaults).
