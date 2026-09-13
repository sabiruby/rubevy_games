#!/usr/bin/env bash
# Serves web/dist/ at http://localhost:8080/ (a module script and fetch() need http, not file://).
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/dist"
exec python3 -m http.server "${PORT:-8080}"
