#!/usr/bin/env bash
set -eo pipefail

SCRIPT_DIR=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )

cargo run -r --manifest-path "$SCRIPT_DIR/Cargo.toml" -- "$@"
