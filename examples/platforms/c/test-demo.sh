#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/../../.." && pwd)"
demo_dir="$repo_root/examples/demo"
build_dir="$script_dir/build"

(
    cd "$demo_dir"
    cargo run --quiet --manifest-path "$repo_root/Cargo.toml" -p boltffi_cli -- \
        --cargo-arg=--features --cargo-arg=c-demo pack c --experimental
)

cmake -S "$script_dir" -B "$build_dir" -DCMAKE_BUILD_TYPE=Debug
cmake --build "$build_dir" --config Debug
ctest --test-dir "$build_dir" --build-config Debug --output-on-failure
