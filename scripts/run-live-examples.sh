#!/bin/bash
set -euo pipefail

provider="${1:-all}"

exec cargo run -p arky --example 09_live_matrix -- "${provider}"
