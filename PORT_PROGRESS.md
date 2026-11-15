# Port progress

Spec: open-crbasic `c547b85` (1.2.1), sibling checkout `../open-crbasic`. Scope: reference verdict
only (stdout, message file, exit status; all four loggers). Plan and rules: `AGENTS.md`
"Porting rules". Status: `todo | running | merged | failed`. H = Haiku, S = Sonnet.

Next wave: P0-a, P0-b, P0-c, P0-d (parallel, disjoint files).

| Id | Python source (lines) | Rust files | Model | Status | Notes |
|---|---|---|---|---|---|
| P0-a | (helpers; Python `re`/`str`/number semantics) | `src/pyre.rs`, `src/pystr.rs` | S | todo | |
| P0-b | `syntax.py` (342), `diagnostics.py` (278), `provenance.py` (17) | `src/syntax.rs`, `src/diagnostics.rs` | S | todo | attribute/`id()` inventory in the report |
| P0-c | `profiles.py` (266), `data/**` | `src/profiles.rs`, `data/**` | S | todo | |
| P0-d | (harness) | `tools/pydump.py`, `tests/fixtures.rs`, `tests/fixture_baseline.txt`, `.github/workflows/ci.yml` | H | todo | verdict goldens only; lines/ast dumps come with P1 |
| P1-a | `lexer.py` (183), `sources.py` (125) | `src/lexer.rs`, `src/sources.rs` | H | todo | |
| P1-b | `source.py` (625) | `src/source.rs` | S | todo | macros excluded |
| P1-c | `preprocess.py` (269) | `src/preprocess.rs` | S | todo | after P1-e |
| P1-d | `consteval.py` (327) | `src/consteval.rs` | S | todo | |
| P1-e1..4 | `parser.py` (3336), 4 ranges | `src/parser/*.rs` | S | todo | serial; ranges fixed when P1-b merges |
| P2-a | all `checker/*`, `tables.py`, `mode.py`, `compiler.py` signatures | skeleton, `todo!()` bodies, `RULES` table | S | todo | serial, one task per module group |
| P2-b1..2 | `checker/context.py` (771) | `src/checker/context.rs` | S | todo | read `../open-crbasic/CHECKER_REDESIGN.md` |
| P2-c1..2 | `checker/__init__.py` (817) | `src/checker/mod.rs` | S | todo | |
| P3-h | `ess` 81, `longconv` 133, `symbols` 157, `structs` 236, `terminals` 427, `storage` 456, `mode.py` 143, `tdf.py` structs | one file each | H | todo | one task per module |
| P3-s | `var_types` 553, `intervals` 679, `scan_time` 580, `warnings` 666, `compiler.py` 324, `cli.py` verdict path | one file each | S | todo | one task per module |
| P4 | `structure` 6309 (8), `expressions` 3758 (5), `instr_rules` 3737 (5), `cr3xx` 2364 (3), `instr_data` 1963 (3), `instr_measure` 1803 (3), `tables.py` 2177 (3) | module directories | S | todo | ranges fixed by P2-a's file split |
| P5 | convergence: bucket (H), fix per bucket (S), station + fuzz + CLI bytes (S) | | H/S | todo | |
| P6 | README, AGENTS status, CI | | H | todo | |
