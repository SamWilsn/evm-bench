#!/usr/bin/env bash
set -eo pipefail

SCRIPT_DIR=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )

cd "$SCRIPT_DIR"
cmake -S . -B build -DCMAKE_BUILD_TYPE=Release >&2
cmake --build build --parallel >&2
build/runner "$@"
