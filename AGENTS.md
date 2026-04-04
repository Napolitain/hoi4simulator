# AGENTS.md

This repository is a Rust simulator/planner crate with a performance-sensitive core. Treat verification as mandatory work, not cleanup.

## Required local verification before handoff

Run these commands for every non-trivial change:

```bash
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
cargo llvm-cov --workspace --all-features --all-targets --summary-only
```

## Required testing workflow

- Keep the simulator and planner assertion-heavy. Encode invariants around dates, resource conservation, building slots, production efficiency, manpower, political power, and state transitions directly in code.
- Add or expand property tests with `proptest` when touching date math, action ordering, state transitions, production accounting, or planner invariants.
- Use targeted fuzzing for parser boundaries, action decoding, and simulator state-transition surfaces. Run `cargo fuzz` locally for risky changes even if CI does not run open-ended fuzz jobs on every push.
- Prefer deterministic, side-effect-free APIs so they remain easy to cover with unit tests, property tests, and fuzz harnesses.

## CI expectations

The GitHub Actions workflow is expected to enforce formatting, clippy, tests, and coverage. Fuzzing remains a required engineering practice for risky surfaces even when it is run manually or on a dedicated cadence instead of every PR.
