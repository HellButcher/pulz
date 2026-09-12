alias b := build
alias t := test
alias c := check

default:
  @just --list

build:
    cargo build

clippy:
    cargo clippy --workspace --all-targets --all-features

clippy-fix:
    cargo clippy --fix --workspace --all-targets --all-features

test *testname:
    cargo test --workspace --all-targets --all-features {{testname}}

run:
    cargo run

fmt:
    cargo +nightly fmt --all

check-fmt:
    cargo +nightly fmt --all --check

check: check-fmt clippy
fix: clippy-fix fmt


# --- benchmarking ---

bench *ARGS:
    cargo bench -p benches --bench ecs_world -- --noplot {{ARGS}}

# --- Performance profiling ---

# Build & profile the bench binary with perf (uses .cargo/config.toml [profile.perf])
# Usage: just perf                          # default: spawn_and_alter1/pulz/5000
#        just perf spawn_and_alter2/bevy     # custom benchmark pattern
#        just perf pulz/10000               # custom entity count

perf benchmark_pattern="spawn_and_alter1/pulz/5000":
    #!/usr/bin/bash
    BINARY="$(cargo test --profile perf -p benches --bench ecs_world --quiet --config 'target."cfg(unix)".runner="echo"')"
    echo "Profiling $BINARY"
    perf record -g -F 997 --call-graph=dwarf,4096 -- "$BINARY" --bench --noplot --exact {{benchmark_pattern}}
    perf report -n --sort symbol 2>&1 | head -60

# Quick perf stat (no flamegraph, just event counts)
perf-stat benchmark_pattern="spawn_and_alter1/pulz/5000":
    #!/usr/bin/bash
    BINARY=$(cargo test --profile perf -p benches --bench ecs_world --quiet --config 'target."cfg(unix)".runner="echo"')
    echo "Profiling $BINARY"
    perf stat -e cycles,instructions,cache-misses,cache-references,LLC-load-misses,LLC-store-misses,branches,branch-misses -- "$BINARY" --bench --noplot --exact {{benchmark_pattern}} 2>&1

samply benchmark_pattern="spawn_and_alter1/pulz/5000":
    #!/usr/bin/bash
    BINARY="$(cargo test --profile perf -p benches --bench ecs_world --quiet --config 'target."cfg(unix)".runner="echo"')"
    echo "Profiling $BINARY"
    samply record "$BINARY" --bench --noplot --exact {{benchmark_pattern}}

