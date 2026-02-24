# Tracing hub-forking benchmark setup

## What was added

- `sentry-tracing/Cargo.toml`
  - Added dev dependency: `criterion = "0.8.2"`
  - Added bench target:
    - `[[bench]]`
    - `name = "tracing_layer_perf"`
    - `harness = false`

- `sentry-tracing/benches/tracing_layer_perf.rs`
  - Criterion benchmarks for:
    - `enter_exit_existing_span`
    - `create_enter_exit_close_span`
    - `reenter_same_span_depth2`
    - `cross_thread_shared_span`
  - Each scenario runs in:
    - `sentry_active`
    - `tracing_only_control`

- `scripts/bench/compare-tracing-perf-master-vs-head.sh`
  - Automates `origin/master` vs current branch comparison
  - Creates/reuses a master worktree at `target/bench-worktrees/tracing-perf-master`
  - Reuses current checkout as head candidate
  - Runs identical `cargo bench` commands on both
  - Saves artifacts to `target/bench-compare/<timestamp>/`
  - Produces:
    - raw logs
    - commands used
    - summary markdown table with deltas

## How to run

Full profile (default):

```bash
scripts/bench/compare-tracing-perf-master-vs-head.sh
```

Reduced profile:

```bash
scripts/bench/compare-tracing-perf-master-vs-head.sh --reduced
```

## Output

Artifacts are written to:

- `target/bench-compare/<timestamp>/summary.md`
- `target/bench-compare/<timestamp>/commands.txt`
- `target/bench-compare/<timestamp>/raw/`
