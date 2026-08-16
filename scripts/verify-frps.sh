#!/usr/bin/env bash

set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
FRP_COMPONENT=frps exec bash "${script_dir}/verify-frpc.sh" "$@"
