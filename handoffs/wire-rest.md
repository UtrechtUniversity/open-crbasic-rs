# HANDOFF wire-rest

## Done
1. src/tables/ (part1-3, mod.rs): all stub/`_with`/`_impl` pairs merged to Python names. Duplicate types
   (VarInfoData, OutFieldData, Part3VarInfoData, DataTableDeclData, ConstTableDeclData, StructTypeInfo) and
   p1_*/_local helper copies deleted; part2/part3 use part1's VarInfo/DataTableDecl/ConstTableDecl/StructTypeDef and
   part2::OutField. `build_tables(program, profile, consts) -> (tables, text, TableExtras)` now does what Python does
   (no hooks). `TableExtras` (dup_fields / interval_display / storage per table index) replaces the ad-hoc attributes
   Python hangs on TdfTable (TdfTable is in tdf.rs, not editable). `tables::attach` = tables_hook.py::attach;
   `CompileResult.table_extras` added (compiler.rs).
   checker/mod.rs now calls attach, check_duplicate_fields (with dup entries) and storage::check (with _storage metas).
   expressions::_data_table_fields and instr_data::_getrecord_short now call build_tables (catch_unwind = Python except).
2. checker/storage.rs pairs merged; local tables helpers replaced by tables:: functions.
   intervals::check / var_types::check stubs removed (caller in checker/mod.rs passes profile/env).

## Observed gate (after step 1): fixtures 76901/77293 cases, 590/613 files fully passing; golden_ast, golden_lines ok.

## Remaining: see end of file (updated as work proceeds)

## Final (all three files groups done)
- Merged pairs/renamed in intervals, scan_time, warnings, var_types, symbols, structs, longconv, ess, mode, compiler (+ storage, tables).
  Callers in checker/mod.rs, instr_measure/part3.rs, structure/part8-9.rs, main.rs, tests/fixtures.rs updated to real args.
- Remaining `todo!(`: 0. Remaining `_with` fn: cr3xx/part2.rs::_leading_unknown_words_with (Python optional args callables/state).
- Resolved items: intervals::_timer_context known/out_of_scope check; intervals local TABLE_ONLY copy -> instr_data;
  warnings::check_background_calibration busy/timed from scan_time::budgets; var_types::check_pointer_deref uses ctx.known_names;
  result.pass1_errors set + pass1_items passed to fold_timeout_expressions; compiler pull_log/pull_site retry loop wired.
- Deviations: mode.rs takes `&Env` instead of Python's is_constant/value_of callables; warnings::check and add_truncate_warnings
  omit Python's unused `result`/`profile` params; render_table_list takes interval displays (TdfTable has no `_interval_display`).
- Caller TODO(wire): compiler.rs `_finish`: checker/unused.py (`-u`) and signature.program_signature not ported (no module).
- Observed gate: fmt, clippy -D warnings, lib tests 281 pass; fixtures cases passing 77021/77293, 598/613 files; golden_ast/golden_lines ok.
