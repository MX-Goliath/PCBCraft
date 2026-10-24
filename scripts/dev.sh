#!/usr/bin/env bash
set -euo pipefail
project_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
if [[ -x "$project_dir/.tools/cargo/bin/rustup" ]]; then
    export RUSTUP_HOME="$project_dir/.tools/rustup"
    export PATH="$project_dir/.tools/cargo/bin:$PATH"
fi
cd "$project_dir"
exec cargo "$@"
