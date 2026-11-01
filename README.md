# open-crbasic-rs

A Rust implementation of [opencrbasic](https://github.com/UtrechtUniversity/open-crbasic), the open
re-implementation of Campbell Scientific's CRBasic compilers. Work has not started; the crate is an
empty library with its checks in place. MIT licensed; not affiliated with Campbell Scientific.

## Development

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
uvx pre-commit install   # the same checks before each commit
```

CI (`.github/workflows/ci.yml`) runs them on every push and pull request. Agent and contributor
conventions are in [AGENTS.md](AGENTS.md).
