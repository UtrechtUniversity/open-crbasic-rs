# Port progress

Spec: open-crbasic `c547b85` (1.2.1), sibling checkout `../open-crbasic`. Scope: reference verdict
only (stdout, message file, exit status; all four loggers). Plan and rules: `AGENTS.md`
"Porting rules". Status: `todo | running | merged | failed`. H = Haiku, S = Sonnet.

Next wave (after the usage reset of 2026-11-15 13:50 UTC): first finish P0-a (branch
`p0a-pyre-pystr`, worktree `.claude/worktrees/p0a`: if it has a commit, rebase onto `port`, run
the check set, fast-forward `port`; if not, re-run the task). Then P1-a, P1-b, P1-d in parallel.

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
| P0-a | (helpers; Python `re`/`str`/number semantics) | `src/pyre.rs`, `src/pystr.rs` | S | running | branch `p0a-pyre-pystr` |
| P0-b | `syntax.py` (342), `diagnostics.py` (278), `provenance.py` (17) | `src/syntax.rs`, `src/diagnostics.rs` | S | merged | `diagnostics.rs` regex helpers carry `TODO(P1): switch to pyre` |
| P0-c | `profiles.py` (266), `data/**` | `src/profiles.rs`, `data/**` | S | merged | |
| P0-d | (harness) | `tools/pydump.py`, `tests/fixtures.rs`, `tests/fixture_baseline.txt`, `.github/workflows/ci.yml` | H | merged | 77,293 cases in 613 files; Python equals the reference on 71,524; lines/ast dumps come with P1 |
| P1-a | `lexer.py` (183), `sources.py` (125) | `src/lexer.rs`, `src/sources.rs` | H | todo | |
| P1-b | `source.py` (625) | `src/source.rs` | S | todo | macros excluded |
| P1-c | `preprocess.py` (269) | `src/preprocess.rs` | S | todo | after P1-e |
| P1-d | `consteval.py` (327) | `src/consteval.rs` | S | todo | |
| P1-e1..4 | `parser.py` (3336), 4 ranges | `src/parser/*.rs` | S | todo | serial; ranges fixed when P1-b merges |
| P2-a | all `checker/*`, `tables.py`, `mode.py`, `compiler.py` signatures | skeleton, `todo!()` bodies, `RULES` table | S | todo | serial, one task per module group |
| P2-b1..2 | `checker/context.py` (771) | `src/checker/context.rs` | S | todo | |
| P2-c1..2 | `checker/__init__.py` (817) | `src/checker/mod.rs` | S | todo | |
| P3-h | `ess` 81, `longconv` 133, `symbols` 157, `structs` 236, `terminals` 427, `storage` 456, `mode.py` 143, `tdf.py` structs | one file each | H | todo | one task per module |
| P3-s | `var_types` 553, `intervals` 679, `scan_time` 580, `warnings` 666, `compiler.py` 324, `cli.py` verdict path | one file each | S | todo | one task per module |
| P4 | `structure` 6309 (8), `expressions` 3758 (5), `instr_rules` 3737 (5), `cr3xx` 2364 (3), `instr_data` 1963 (3), `instr_measure` 1803 (3), `tables.py` 2177 (3) | module directories | S | todo | ranges fixed by P2-a's file split |
| P5 | convergence: bucket (H), fix per bucket (S), station + fuzz + CLI bytes (S) | | H/S | todo | |
| P6 | README, AGENTS status, CI | | H | todo | |
