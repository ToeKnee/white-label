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

echo "== seed the e2e login user (idempotent) =="
# Creates `e2e-dioxus` via the app's own register handler the first time;
# later runs answer 303 -> /login?bad=taken, which is just as fine.
curl -sS -o /dev/null -w "e2e seed status=%{http_code}\n" \
  --data-urlencode "username=e2e-dioxus" \
  --data-urlencode "email=e2e-dioxus@example.com" \
  --data-urlencode "password=password" \
  --data-urlencode "password_confirmation=password" \
  "$base/auth/register"

echo "== seed the e2e-tabs artist fixture (idempotent) =="
# The nested-admin-tabs Playwright test edits this artist through the real
# profile editor, so the row must exist (`name` and `slug` are both UNIQUE,
# so re-running is a no-op). The row hangs off the first label; the profile
# editor loads artists by slug, not by label.
if [ -f "$ROOT/.env" ]; then
    . "$ROOT/.env"
fi
if [ -n "${DATABASE_URL:-}" ]; then
    psql "$DATABASE_URL" -c "INSERT INTO artists (name, slug, description, website, label_id, published_at)
        SELECT 'E2E Tabs Artist', 'e2e-tabs', 'Gate fixture artist', 'https://e2e-tabs.example.com',
               id, now()
        FROM labels ORDER BY id LIMIT 1
        ON CONFLICT (slug) DO NOTHING;" >/dev/null \
        || echo "WARNING: could not seed the e2e-tabs artist (is psql installed?)"
else
    echo "WARNING: DATABASE_URL not set; skipping the e2e-tabs seed"
fi

echo "== admin session checks =="
# The admin pages need a user with the `admin` permission, which the
# register handler can't grant. Seed `e2e-admin` once (register it here,
# then add the permissions in psql):
#   psql "$DATABASE_URL" -c "INSERT INTO user_permissions (user_id, token)
#     SELECT id, 'admin' FROM users WHERE username='e2e-admin'
#       AND NOT EXISTS (SELECT 1 FROM user_permissions p2 WHERE p2.user_id = users.id AND p2.token='admin');"
#   ... and the same with 'label_owner' for the label/page editors.
# Until then the admin browser checks are skipped (the Rust SSR tests in
# crate::server cover the same flows in-process).
admin_ok=0
curl -sS -o /dev/null -w "e2e-admin seed status=%{http_code}\n" \
  --data-urlencode "username=e2e-admin" \
  --data-urlencode "email=e2e-admin@example.com" \
  --data-urlencode "password=password" \
  --data-urlencode "password_confirmation=password" \
  "$base/auth/register"
if curl -sS -c "$OUT/admin-cookies.txt" -o /dev/null \
     --data-urlencode "username=e2e-admin" \
     --data-urlencode "password=password" \
     "$base/auth/login" && \
   curl -sS -b "$OUT/admin-cookies.txt" -o "$OUT/admin_dashboard.html" "$base/admin" && \
   grep -q 'card-title\">Artists' "$OUT/admin_dashboard.html"; then
    admin_ok=1
fi
if [ "$admin_ok" -eq 1 ]; then
    echo "admin session active (e2e-admin)"
    grep -c 'Admin Dashboard' "$OUT/admin_dashboard.html"
    grep -c 'w-56 menu bg-base-200 rounded-box' "$OUT/admin_dashboard.html"
    # The nested tab layout only renders behind the admin gate. The
    # releases tab is real since slice 5b, so it needs a live artist row
    # (the seed above); a missing slug would render NotFound instead.
    curl -sS -b "$OUT/admin-cookies.txt" -o "$OUT/nested.html" -w "nested status=%{http_code}\n" "$base/admin/artist/e2e-tabs/releases"
    grep -c 'tabs tabs-border' "$OUT/nested.html"
    # (slice 5a) The artist create page serves the real form handlers.
    curl -sS -b "$OUT/admin-cookies.txt" -o "$OUT/artist_create.html" "$base/admin/artist"
    grep -c 'action="/admin/forms/artist"' "$OUT/artist_create.html"
    # (slice 5b) The release create page serves its real form handlers too.
    curl -sS -b "$OUT/admin-cookies.txt" -o "$OUT/release_create.html" "$base/admin/artist/e2e-tabs/releases/new"
    grep -c 'action="/admin/forms/release"' "$OUT/release_create.html"
    # (slice 5d) The artist links editor serves its real form handler.
    curl -sS -b "$OUT/admin-cookies.txt" -o "$OUT/artist_links.html" "$base/admin/artist/e2e-tabs/links"
    grep -c 'action="/admin/forms/links"' "$OUT/artist_links.html"
else
    echo "ADMIN CHECKS SKIPPED: e2e-admin lacks the admin permission (see instructions above)"
    # Guest view of a guarded admin route: the login page replaces it.
    curl -sS -o "$OUT/nested.html" -w "nested status=%{http_code}\n" "$base/admin/artist/jimi/releases"
    grep -c 'action="/auth/login"' "$OUT/nested.html"
fi

echo "== curl spot checks =="
curl -sS -o "$OUT/home.html" -w "home status=%{http_code}\n" "$base/"
grep -c 'navbar bg-primary text-primary-content' "$OUT/home.html"
grep -c 'footer bg-neutral text-neutral-content' "$OUT/home.html"
grep -c 'href="/main.css"' "$OUT/home.html"
curl -sS -o /dev/null -w "main.css status=%{http_code}\n" "$base/main.css"
curl -sS -o "$OUT/guest_profile.html" -w "guest profile status=%{http_code}\n" "$base/profile"
grep -c 'action="/auth/login"' "$OUT/guest_profile.html"
curl -sS -o /dev/null -w "bad login status=%{http_code}\n" \
  --data-urlencode "username=e2e-dioxus" \
  --data-urlencode "password=definitely-wrong" \
  "$base/auth/login"

echo "== playwright navigation smoke (chromium) =="
cd "$ROOT/end2end" || exit 1
if [ "$admin_ok" -eq 1 ]; then
    E2E_ADMIN_OK=1 DIOXUS_URL="$base" npx playwright test tests/dioxus_shell.spec.ts --project=chromium --reporter=list
else
    DIOXUS_URL="$base" npx playwright test tests/dioxus_shell.spec.ts --project=chromium --reporter=list
fi
status=$?

if [ "$status" -ne 0 ]; then
    echo "== server log tail =="
    tail -n 20 "$OUT/server.log"
fi
exit $status
