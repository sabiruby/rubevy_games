#!/usr/bin/env bash
# Serves web/dist/ at http://localhost:8080/ (a module script and fetch() need http, not file://).
# That address is the entry page; the games are at /sabibots/ and /garden/, which is also where
# they are on the published site. `web/build.sh <one game>` writes only that game's directory and
# leaves the entry page as it was, so open the game's own URL after one of those.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/dist"
exec python3 -m http.server "${PORT:-8080}"
