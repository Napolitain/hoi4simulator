# hoi4simulator

Rust foundations for a country-extensible Hearts of Iron IV simulator and solver.

## Current scope

- Separate repository from `twoptimizer`
- Exact daily simulation semantics
- Fully single-threaded execution for now
- France 1936 as the first reference scenario
- No combat and no active trade in V1
- Explicit dated actions for construction, production, focus, laws, research, and advisors
- Pure simulator-search planning with a rolling beam-search shape

## Current crate layout

- `domain/`: dates, laws, planning milestones, strategic goal weights, and division-template demand and fitness
- `scenario/`: zero-cost country-scenario interface plus France 1936 defaults and readiness targets
- `data/`: Clausewitz parsing, exact-data mirroring, structured dataset generation, and exact France 1936 loading
- `sim/`: dense-ID action types, contiguous runtime state, daily simulation engine, and France-specific heuristic rule validation
- `solver/`: rolling beam-search configuration plus a France planner that evaluates strategy templates, pivot dates, and strategic-goal weights

## Performance direction

- dense integer IDs instead of string lookups in the hot path
- contiguous `Box<[T]>` storage for state records and production lines
- fixed-point integer economics and production math instead of floats
- single-threaded planning and simulation, with preallocated `Vec` buffers in the hot path
- release profile tuned for lower overhead (`thin` LTO, single codegen unit, `panic = "abort"`)

## Testing style

The project is being shaped around a Tiger-style verification approach:

- assertion-heavy core logic
- simple explicit control flow
- deterministic simulation behavior
- positive and negative space tests
- scenario and invariant coverage before broad feature count

## Current status

This is the first implementation slice, not the full simulator. The repository currently provides:

- validated domain types for dates, laws, and milestones
- France 1936 scenario defaults, pivot window, curated state set, fort targets, and readiness targets
- country-scenario trait seams so future countries can reuse the same simulator and solver boundaries
- canonical France division-template demand modeling plus template fitness and constraint checks for future template search
- exact-data pipeline support for local-only `data/raw/<profile>/` mirrors and normalized Apache Fory datasets under `data/structured/<profile>/`
- daily construction, production, focus, research, and political-power simulation
- heuristic rule validation for pre-pivot and post-pivot planning behavior
- rolling-horizon beam search that scores France strategy templates, pivot dates, and broader strategic goals

## Trying the simulator

Run the curated reference planner end-to-end:

```bash
cargo run --example france_1936_plan
```

That example drives the solver on top of the daily simulator and prints the chosen strategy template, pivot date, first dated actions, and final factory/readiness summary.

To use exact local HOI4 data instead of the curated bootstrap:

```bash
cargo run --bin ingest_data -- --game-dir "/path/to/Hearts of Iron IV" --profile vanilla
cargo run --bin france_1936 -- --profile vanilla
```

The ingest step mirrors selected exact game files into `data/raw/<profile>/` and writes a normalized Apache Fory dataset to `data/structured/<profile>/`. The France scenario runner then loads that binary structured dataset and fails loudly if the required exact data is missing.

Because this repository is public, those `data/raw/` and `data/structured/` trees are intentionally gitignored.

## Linting, tests, and coverage

Use the same workflow locally that CI enforces:

```bash
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
cargo install cargo-llvm-cov --locked
cargo llvm-cov --workspace --all-features --all-targets --summary-only
```

Property tests run through `cargo test`. Fuzzing expectations and assertion guidance are documented in `AGENTS.md`.

## Targeted mutation testing

Mutation testing is configured through `.cargo/mutants.toml` for the invariant-heavy modules that already have strong `proptest` coverage. This is intentionally narrower than the full crate so solver and exact-scenario runs do not become an always-on bottleneck.

Run it locally with:

```bash
cargo install cargo-mutants --locked
cargo mutants --list-files
cargo mutants
```

The repository also includes a manual GitHub Actions workflow, `mutation`, that runs the same targeted configuration and uploads the `mutants.out/` results as an artifact.
