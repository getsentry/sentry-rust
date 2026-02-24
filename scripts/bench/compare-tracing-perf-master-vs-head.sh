#!/usr/bin/env bash
set -euo pipefail

profile="full"
if [[ "${1:-}" == "--reduced" ]]; then
  profile="reduced"
fi

if [[ "$profile" == "full" ]]; then
  sample_size=100
  measurement_time=10
  warm_up_time=3
else
  sample_size=20
  measurement_time=2
  warm_up_time=1
fi

repo_root="$(git rev-parse --show-toplevel)"
current_branch="$(git rev-parse --abbrev-ref HEAD)"
head_sha="$(git rev-parse HEAD)"
timestamp="$(date -u +%Y%m%dT%H%M%SZ)"

worktree_root="$repo_root/target/bench-worktrees"
master_worktree="$worktree_root/tracing-perf-master"
head_worktree="$repo_root"

artifact_dir="$repo_root/target/bench-compare/$timestamp"
raw_dir="$artifact_dir/raw"
mkdir -p "$raw_dir/master" "$raw_dir/head"

mkdir -p "$worktree_root"

git config --global --add safe.directory "$repo_root" >/dev/null 2>&1 || true
git config --global --add safe.directory "$master_worktree" >/dev/null 2>&1 || true

git fetch origin master >/dev/null

if [[ ! -e "$master_worktree/.git" ]]; then
  git worktree add --detach "$master_worktree" origin/master >/dev/null
else
  git -C "$master_worktree" fetch origin master >/dev/null
  git -C "$master_worktree" checkout --detach -f origin/master >/dev/null
fi

# Keep benchmark harness identical between baseline and candidate.
mkdir -p "$master_worktree/sentry-tracing/benches"
cp "$repo_root/sentry-tracing/Cargo.toml" "$master_worktree/sentry-tracing/Cargo.toml"
cp "$repo_root/sentry-tracing/benches/tracing_layer_perf.rs" "$master_worktree/sentry-tracing/benches/tracing_layer_perf.rs"
cp "$repo_root/Cargo.lock" "$master_worktree/Cargo.lock"

scenarios=(
  enter_exit_existing_span
  create_enter_exit_close_span
  reenter_same_span_depth2
  cross_thread_shared_span
)

modes=(
  sentry_active
  tracing_only_control
)

commands_file="$artifact_dir/commands.txt"
{
  echo "# profile=$profile"
  echo "# branch=$current_branch"
  echo "# head_sha=$head_sha"
  echo "cargo bench -p sentry-tracing --bench tracing_layer_perf --no-run"
  for scenario in "${scenarios[@]}"; do
    echo "cargo bench -p sentry-tracing --bench tracing_layer_perf $scenario -- --sample-size $sample_size --measurement-time $measurement_time --warm-up-time $warm_up_time --noplot"
  done
} >"$commands_file"

run_suite() {
  local label="$1"
  local worktree="$2"

  (
    cd "$worktree"
    cargo bench -p sentry-tracing --bench tracing_layer_perf --no-run
  ) >"$raw_dir/$label/build.log" 2>&1

  for scenario in "${scenarios[@]}"; do
    local log_file="$raw_dir/$label/$scenario.log"
    (
      cd "$worktree"
      cargo bench -p sentry-tracing --bench tracing_layer_perf "$scenario" -- \
        --sample-size "$sample_size" \
        --measurement-time "$measurement_time" \
        --warm-up-time "$warm_up_time" \
        --noplot
    ) 2>&1 | tee "$log_file"
  done
}

parse_mean_ns() {
  local log_file="$1"
  local bench_name="$2"

  awk -v bench="$bench_name" '
    function to_ns(v, u) {
      if (u == "ns") return v
      if (u == "us" || u == "µs" || u == "μs") return v * 1000
      if (u == "ms") return v * 1000000
      if (u == "s") return v * 1000000000
      return -1
    }
    /^Benchmarking / {
      current = $0
      sub(/^Benchmarking /, "", current)
      sub(/:.*/, "", current)
    }
    $1 == "time:" && current == bench {
      gsub("\\[", "", $2)
      middle_value = $4
      middle_unit = $5
      print to_ns(middle_value + 0, middle_unit)
      exit
    }
  ' "$log_file"
}

run_suite master "$master_worktree"
run_suite head "$head_worktree"

summary_file="$artifact_dir/summary.md"
{
  echo "# tracing_layer_perf: origin/master vs $current_branch"
  echo
  printf -- "- profile: \`%s\`\n" "$profile"
  printf -- "- baseline: \`%s\`\n" "origin/master"
  printf -- "- candidate: \`%s (%s)\`\n" "$current_branch" "$head_sha"
  echo
  echo "| scenario | mode | master mean (ns) | head mean (ns) | delta |"
  echo "|---|---|---:|---:|---:|"

  for scenario in "${scenarios[@]}"; do
    for mode in "${modes[@]}"; do
      bench_name="$scenario/$mode"
      master_ns="$(parse_mean_ns "$raw_dir/master/$scenario.log" "$bench_name")"
      head_ns="$(parse_mean_ns "$raw_dir/head/$scenario.log" "$bench_name")"

      if [[ -z "$master_ns" || -z "$head_ns" ]]; then
        delta="n/a"
      else
        delta="$(awk -v a="$master_ns" -v b="$head_ns" 'BEGIN { printf "%.2f%%", ((b - a) / a) * 100 }')"
      fi

      echo "| $scenario | $mode | $master_ns | $head_ns | $delta |"
    done
  done
  echo
  printf -- "Commands: \`%s\`\n" "$commands_file"
  printf -- "Raw logs: \`%s\`\n" "$raw_dir"
} >"$summary_file"

echo "Benchmark comparison complete."
echo "Artifacts: $artifact_dir"
echo "Summary: $summary_file"
