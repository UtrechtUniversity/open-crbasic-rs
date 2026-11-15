# Port progress

Spec: open-crbasic `c547b85` (1.2.1), sibling checkout `../open-crbasic`. Scope: reference verdict
only (stdout, message file, exit status; all four loggers). Plan and rules: `AGENTS.md`
"Porting rules". Status: `todo | running | merged | failed`. H = Haiku, S = Sonnet.

Next wave: continue P4. The SPI open-order round is implemented.

The first live verdict replay matched Python on 25,964 of 77,293 cases and all cases in 20 of
613 files. CLI probes matched Python's stdout, message-file bytes and exit status in 14 of
15 cases, including a minimal program for each logger. An empty CR6 file still misses
`No BeginProg in Program.`, and unfinished checker functions emit panic reports on stderr
when compiling existing files.

P4 table-name round: ported name extraction, diagnostic classification and CR300/CR350
finalization in `instr_data/part3.rs`. Verdict parity rose by 134 cases to 26,098 of 77,293,
with all 20 baseline files still passing. The CR6/CR1000X caller is
`check_table_and_menu_placement`. The separate `check_table_structure` skeleton needs the
Python stop-location return type.

P4 Units round: ported target validation and duplicate assignment diagnostics in
`instr_data/part2.rs`. Verdict parity rose to 26,118 of 77,293 cases.
`r16_units_extra_words.json` became the 21st baseline file. Twenty CLI probes across all four
loggers matched the pinned Python verdict for unknown, extra indexed, duplicate, indexed
override and alias targets.

P4 argument-text round: ported `_pick_tokens` and `_arg_text`. Direct tests cover raw
`MenuPick` commas, trailing slots, comments, Latin-1 character offsets and `Select Case`
spacing. Verdict parity reached 26,122 of 77,293 and all 21 baseline files passed. The Boolean
picklist stubs need their `Env` parameters, `_pick_value_bad` needs the environment, program,
profile and variable names, and `_check_pick_consts` needs its full parameter list.

P4 SampleFieldCal round: ported the FieldCal/FieldCalStrain prerequisite check. The two
SampleFieldCal fixture files matched all 24 cases, and `samplefieldcal_a.json` became the 22nd
baseline file. Full verdict parity reached 26,130 of 77,293 cases.

P4 SPI round: ported terminal and numeric SPI port matching and the requirement that
`SPIOpen` precede `SPIRead` and `SPIWrite`. Connected SPI and I2C terminal checks to the
implemented Long conversion and constant-name collection. All 10 SPI open-order cases
matched Python, including cases that report both an invalid port and an open-order error.
Verdict parity rose by 278 cases to 26,408 of 77,293, and `spi_open_order.json` became the
23rd baseline file.

Operating notes (learned in wave 1):
- A wave of 4 agents (3 Sonnet, 1 Haiku) used about 40% of a 5-hour window. Launch a wave of 3
  only below 50% used, a wave of 2 below 65%, a single task below 75%; otherwise hand off.
- Do not use the Agent tool's `isolation: "worktree"`: it cuts from `main`, deletes the worktree
  when unchanged and blocks the sibling checkout. Create worktrees by hand from `port`
  (`git worktree add -b <task> .claude/worktrees/<task> port`) and give the agent that path.
- In a worktree `../open-crbasic` does not resolve: agents set
  `OPENCRBASIC_PY=<absolute path of the Python checkout>`. Never commit that absolute path.
- Goldens live in each checkout's own `target/golden` (about 50 s to regenerate with
  `tools/pydump.py`).
- `CHECKER_REDESIGN.md` is no longer in the Python checkout; the module docstring of
  `checker/__init__.py` is the description of the phase model.
- Facts for later tasks: profiles read only their own data directory (no fallback);
  `data/cr300/terminals.json` holds `-Infinity` (handled in `profiles.rs`); `status_table.json`
  is for the tables port to embed; the AST JSON renames `ConstDecl.type` to `type_` (the Python
  AST dump must do the same); `Diagnostics.items` is `Vec<Rc<Diagnostic>>`; nodes mutated by the
  parser after sharing are `Cell`/`RefCell` in `StmtBase`, everything else must be final before
  the node is wrapped in `Rc`.

| Id | Python source (lines) | Rust files | Model | Status | Notes |
|---|---|---|---|---|---|
| P0-a | (helpers; Python `re`/`str`/number semantics) | `src/pyre.rs`, `src/pystr.rs` | S | merged | regexes are `pyre::Regex` values in `LazyLock` statics, offsets in chars; ints are `i64` only |
| P0-b | `syntax.py` (342), `diagnostics.py` (278), `provenance.py` (17) | `src/syntax.rs`, `src/diagnostics.rs` | S | merged | `diagnostics.rs` regex helpers carry `TODO(P1): switch to pyre` |
| P0-c | `profiles.py` (266), `data/**` | `src/profiles.rs`, `data/**` | S | merged | |
| P0-d | (harness) | `tools/pydump.py`, `tests/fixtures.rs`, `tests/fixture_baseline.txt`, `.github/workflows/ci.yml` | H | merged | 77,293 cases in 613 files; Python equals the reference on 71,524; lines/ast dumps come with P1 |
| P1-a | `lexer.py` (183) | `src/lexer.rs` | H | merged | `sources.py` moved to P1-b; 6 unit tests |
| P1-b | `sources.py` (125), `source.py` (625) | `src/sources.rs`, `src/source.rs` | S | merged | `29e207e`; macros excluded; `golden_lines` 77,903/77,903 |
| P1-c | `preprocess.py` (269) | `src/preprocess.rs` | S | merged | `2bb6952`; 3 unit tests |
| P1-d | `consteval.py` (327) | `src/consteval.rs` | S | merged | `0b6c140`; 7 unit tests |
| P1-e1..4 | `parser.py` (3336), 4 ranges | `src/parser/*.rs` | S | merged | `a2acf82`; `golden_ast` 77,903/77,903; 7 unit tests |
| P2-a | all `checker/*`, `tables.py`, `mode.py`, `compiler.py` signatures | skeleton, `todo!()` bodies, `RULES` table | S | merged | `5af36e1`; 54 files |
| P2-b1..2 | `checker/context.py` (771) | `src/checker/context.rs` | S | merged | `d27829a`; 4 unit tests |
| P2-c1..2 | `checker/__init__.py` (817) | `src/checker/mod.rs` | S | merged | `06b9037`; 3 unit tests |
| P3-h | `ess` 81, `longconv` 133, `symbols` 157, `structs` 236, `terminals` 427, `storage` 456, `mode.py` 143, `tdf.py` structs | one file each | H | merged | P3-h1..4 (`1266faa`, `43f16a0`, `c5cd931`, `d504604`), `tdf.rs` (`126277f`) merged; 15 unit tests |
| P3-s | `var_types` 553, `intervals` 679, `scan_time` 580, `warnings` 666, `compiler.py` 324, `cli.py` verdict path | one file each | S | merged | P3-s1..5 (`4511f1f`, `7c61220`, `aac8de2`, `01bd124`, `126277f`) merged; 27 unit tests. P3-s6 adds `crbc`, connects the existing mode implementation and runs actual compiler verdicts in the fixture harness. |
| P4 | `structure` 6309 (9), `expressions` 3758 (5), `instr_rules` 3737 (5), `cr3xx` 2364 (4), `instr_data` 1963 (3), `instr_measure` 1803 (3), `tables.py` 2177 (3) | module directories | S | running | Ported `_unparsed_opener` and `_leftover_first_name_known` in structure part 4. `check_end_blocks` needs the `_block_word` argument slice and the `calltable_targets` implementation. Merged `tables/part1.rs` (`50306d4`), `part2.rs` (`7018ecd`), `part3.rs` (`43616c1`) — `tables.py` 1-2177 ported; needs cross-part wiring + `TdfField`/`TdfTable` attrs. Merged `checker/expressions/part1.rs` (`4390d10`), `part2.rs` (`98327e1`), `part3.rs` (`b721666`), `part4.rs` (`f4e1e1f`), `part5.rs` (`f1034a7`) — `expressions.py` 1-3758 ported; needs cross-part wiring (`KnownNames`, `part1..5` `*_with`, `pending_wiring` in `part3`, `cr3xx::part2`, `structure::part2`, `tables::build_tables`, `instr_data::TABLE_ONLY_INSTRUCTIONS`, `instr_rules`). |
| P5 | convergence: bucket (H), fix per bucket (S), station + fuzz + CLI bytes (S) | | H/S | todo | |
| P6 | README, AGENTS status, CI | | H | todo | |
