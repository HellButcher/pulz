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
