alias b := build
alias t := test
alias c := check
alias re := run-example

default:
  @just --list

build:
    cargo build

clippy:
    cargo clippy --workspace --all-targets --all-features

clippy-fix:
    cargo clippy --fix --workspace --all-targets --all-features --allow-dirty --allow-staged

test *testname:
    cargo test --workspace --all-targets --all-features {{testname}}

_run_defaults_sh := '
export RUST_LOG="${RUST_LOG:-debug}"
export PULZ_DUMP_SCHEDULE="dump.dot"
export RUST_BACKTRACE="${RUST_BACKTRACE:-1}"
'

_vk_layers_sh := '
export VK_INSTANCE_LAYERS=VK_LAYER_KHRONOS_validation
export VK_DEVICE_LAYERS=VK_LAYER_KHRONOS_validation
export VK_LAYER_DISABLES=
export VK_LAYER_ENABLES="VK_VALIDATION_FEATURE_ENABLE_SYNCHRONIZATION_VALIDATION_EXT:VALIDATION_CHECK_ENABLE_SYNCHRONIZATION_VALIDATION_QUEUE_SUBMIT:VK_VALIDATION_FEATURE_ENABLE_BEST_PRACTICES_EXT"
# :VALIDATION_CHECK_ENABLE_VENDOR_SPECIFIC_NVIDIA:VALIDATION_CHECK_ENABLE_VENDOR_SPECIFIC_AMD
'

_default_example := 'render-ash-demo'

list-examples:
  cargo run --example

run-example example=_default_example:
    #!/usr/bin/bash
    set -e
    {{ _run_defaults_sh }}
    set -x
    exec cargo run --example {{example}}

validate-example example=_default_example:
    #!/usr/bin/bash
    set -ex
    {{ _run_defaults_sh }}
    {{ _vk_layers_sh }}
    set -x
    exec cargo run --example {{example}}

capture-example example=_default_example:
    #!/usr/bin/bash
    set -e
    {{ _run_defaults_sh }}
    # force usage of x11
    unset WAYLAND_DISPLAY
    export CAPTURE_OPTS="\
      --capture-file renderdoc-capture.rdc
      --opt-capture-callstacks \
      --opt-hook-children \
      --opt-api-validation
      --opt-api-validation-unmute
      --wait-for-exit \
      "
    set -x
    exec renderdoccmd capture $CAPTURE_OPTS cargo run --example {{example}}

fmt:
    cargo +nightly fmt --all

check-fmt:
    cargo +nightly fmt --all --check

check: check-fmt clippy
fix: clippy-fix fmt
