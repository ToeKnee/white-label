#!/bin/sh
# Phase 0 runtime gate: serve the spike against the real backend and capture
# SSR/auth/router/server-fn behaviour with curl. Uses port 8123 so it never
# collides with the Leptos app on 8080.
set -u
OUT=/tmp/spike-run
rm -rf "$OUT"
mkdir -p "$OUT"
cd /home/anthony/Projects/Rust/white-label/spike || exit 1

cargo build >"$OUT/build.log" 2>&1 || { echo "BUILD FAILED"; tail -n 30 "$OUT/build.log"; exit 1; }
mkdir -p "$OUT/public"
IP=127.0.0.1 PORT=8123 DIOXUS_PUBLIC_PATH="$OUT/public" cargo run >"$OUT/server.log" 2>&1 &
SERVER=$!
base=http://127.0.0.1:8123

echo "== waiting for server =="
ready=0
for _ in 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20 21 22 23 24 25 26 27 28 29 30 31 32 33 34 35 36 37 38 39 40 41 42 43 44 45 46 47 48 49 50 51 52 53 54 55 56 57 58 59 60; do
    if curl -s -o /dev/null "$base/" 2>/dev/null; then ready=1; break; fi
    sleep 2
done
if [ "$ready" -eq 0 ]; then
    echo "SERVER NEVER STARTED; log follows"
    tail -n 30 "$OUT/server.log"
    kill "$SERVER" 2>/dev/null
    exit 1
fi

echo "== GET / (not logged in) =="
curl -sS -o "$OUT/home.html" -w "status=%{http_code}\n" "$base/"
grep -o 'Dioxus spike' "$OUT/home.html" | head -1
grep -o 'Not logged in' "$OUT/home.html" | head -1
grep -o 'navbar bg-primary text-primary-content' "$OUT/home.html" | head -1
grep -o 'href="/login"' "$OUT/home.html" | head -1

echo "== GET /login (daisy markup parity) =="
curl -sS -o "$OUT/login.html" -w "status=%{http_code}\n" "$base/login"
grep -oE 'class="(fieldset|input|checkbox|btn|label)[^"]*"' "$OUT/login.html" | sort -u

echo "== GET /artists (use_resource SSR against real DB) =="
curl -sS -o "$OUT/artists.html" -w "status=%{http_code}\n" "$base/artists"
grep -oE 'href="/admin/artist/[^"]+"' "$OUT/artists.html" | head -5

SLUG=$(grep -oE 'href="/admin/artist/[^"]+"' "$OUT/artists.html" | head -1 | sed -E 's|href="/admin/artist/||; s|"$||; s|/$||')
echo "discovered slug: $SLUG"

if [ -n "$SLUG" ]; then
    echo "== nested /admin/artist/SLUG (nest layout + overview SSR) =="
    curl -sS -o "$OUT/nested.html" -w "status=%{http_code}\n" "$base/admin/artist/$SLUG"
    grep -o 'tabs tabs-boxed' "$OUT/nested.html" | head -1
    grep -o 'Artist overview' "$OUT/nested.html" | head -1
    grep -o "slug from nest: [^<]*" "$OUT/nested.html" | head -1
    grep -o 'Open a placeholder release edit page' "$OUT/nested.html" | head -1

    echo "== nested with trailing slash =="
    curl -sS -o "$OUT/nested2.html" -w "status=%{http_code}\n" "$base/admin/artist/$SLUG/"
    grep -o 'Artist overview' "$OUT/nested2.html" | head -1

    echo "== leaf /releases/x/ with trailing slash =="
    curl -sS -o "$OUT/release.html" -w "status=%{http_code}\n" "$base/admin/artist/$SLUG/releases/tom-test/"
    grep -o 'Release edit' "$OUT/release.html" | head -1
    grep -o 'release slug from leaf: [^<]*' "$OUT/release.html" | head -1

    echo "== GET /api/artist real slug (server-fn JSON wire format) =="
    curl -sS -w "\nstatus=%{http_code}\n" "$base/api/artist?artist_slug=$SLUG"
fi

echo "== GET /api/artist bogus slug (error wire format) =="
curl -sS -w "\nstatus=%{http_code}\n" "$base/api/artist?artist_slug=no-such-slug-xyz"

echo "== POST /api/user/login bogus creds (omit remember) =="
curl -sS -w "\nstatus=%{http_code}\n" -X POST -H 'content-type: application/json' \
    --data '{"username":"nope","password":"nope"}' "$base/api/user/login"

echo "== POST /api/user/login bogus creds (remember:null) =="
curl -sS -w "\nstatus=%{http_code}\n" -X POST -H 'content-type: application/json' \
    --data '{"username":"nope","password":"nope","remember":null}' "$base/api/user/login"

echo "== GET /api/user unauthenticated =="
curl -sS -w "\nstatus=%{http_code}\n" "$base/api/user"

echo "== GET /api/file_progress unauthenticated (permission gate) =="
curl -sS -w "\nstatus=%{http_code}\n" "$base/api/file_progress?filename=whatever"

echo "== POST /api/user/logout (session reset) =="
curl -sS -o "$OUT/logout.json" -D "$OUT/logout.headers" -w "status=%{http_code}\n" \
    -X POST -H 'content-type: application/json' --data '{}' "$base/api/user/logout"
cat "$OUT/logout.json"
echo
grep -i '^set-cookie' "$OUT/logout.headers" || echo "(no set-cookie header on logout)"

kill "$SERVER" 2>/dev/null
sleep 1
kill -9 "$SERVER" 2>/dev/null
pkill -f 'target/debug/spike' 2>/dev/null
echo "== server log tail =="
tail -n 20 "$OUT/server.log"
exit 0
