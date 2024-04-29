#!/usr/bin/env bash
set -eo pipefail

SCRIPT_DIR=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )

cd "$SCRIPT_DIR"
npm install >&2
npm run --silent nodejs-runner -- "$@"
