#!/bin/sh
# Phase 3 runtime gate (MIGRATION.md): serve the Dioxus SSR app from the real
# crate against the dev database, then smoke-test shell navigation with
# Playwright (chromium) plus a few curl spot checks. Uses port 8127 so it
# never collides with the Leptos app (3000) or other local services (8080).
#
# Run from anywhere:  sh end2end/run-dioxus-gate.sh
set -u
ROOT=/home/anthony/Projects/Rust/white-label
OUT=/tmp/dioxus-shell-gate
rm -rf "$OUT"
mkdir -p "$OUT"
cd "$ROOT" || exit 1

echo "== build =="
cargo build --offline --no-default-features --features server --bin dioxus-server >"$OUT/build.log" 2>&1 || {
    echo "BUILD FAILED"; tail -n 30 "$OUT/build.log"; exit 1
}

IP=127.0.0.1 PORT=8127 DIOXUS_PUBLIC_PATH="$ROOT/public" ./target/debug/dioxus-server >"$OUT/server.log" 2>&1 &
SERVER=$!
trap 'kill "$SERVER" 2>/dev/null' EXIT
base=http://127.0.0.1:8127

echo "== waiting for server =="
ready=0
for _ in 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20 21 22 23 24 25 26 27 28 29 30; do
    if curl -s -o /dev/null "$base/" 2>/dev/null; then ready=1; break; fi
    sleep 2
done
if [ "$ready" -eq 0 ]; then
    echo "SERVER NEVER STARTED; log follows"
    tail -n 30 "$OUT/server.log"
    exit 1
fi

echo "== curl spot checks =="
curl -sS -o "$OUT/home.html" -w "home status=%{http_code}\n" "$base/"
grep -c 'navbar bg-primary text-primary-content' "$OUT/home.html"
grep -c 'footer bg-neutral text-neutral-content' "$OUT/home.html"
grep -c 'href="/main.css"' "$OUT/home.html"
curl -sS -o /dev/null -w "main.css status=%{http_code}\n" "$base/main.css"
curl -sS -o "$OUT/nested.html" -w "nested status=%{http_code}\n" "$base/admin/artist/jimi/releases"
grep -c 'tabs tabs-border' "$OUT/nested.html"

echo "== playwright navigation smoke (chromium) =="
cd "$ROOT/end2end" || exit 1
DIOXUS_URL="$base" npx playwright test tests/dioxus_shell.spec.ts --project=chromium --reporter=list
status=$?

if [ "$status" -ne 0 ]; then
    echo "== server log tail =="
    tail -n 20 "$OUT/server.log"
fi
exit $status
