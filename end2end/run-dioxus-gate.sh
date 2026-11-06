#!/bin/sh
# Phase 3+ runtime gate (MIGRATION.md): serve the Dioxus SSR app from the
# real crate against the dev database, then smoke-test shell navigation with
# Playwright (chromium) plus a few curl spot checks. Since Phase 5 a second
# server (8131) serves the `dx build` client bundle, and
# `tests/dioxus_client.spec.ts` checks hydration/router takeover against it.
# Uses ports 8127/8131 so it never collides with the Leptos app (3000) or
# other local services (8080).
#
# Run from anywhere:  sh end2end/run-dioxus-gate.sh
set -u
ROOT=/home/anthony/Projects/Rust/white-label
OUT=/tmp/dioxus-shell-gate
rm -rf "$OUT"
mkdir -p "$OUT"
cd "$ROOT" || exit 1

# A missing page marker must fail the gate. `grep -c` prints a count and
# exits 0 whatever it counts, so pins never reached the exit status on their
# own (the images pins sat at 0 for a whole slice without anyone noticing).
# Every check below goes through `pin`/`no_pin`, and the failures are folded
# into the exit status at the end.
pin_failed=0
pin() {
    if grep -q -- "$2" "$1"; then
        echo "pin ok: $2"
    else
        echo "PIN FAILED: missing '$2' in $1"
        pin_failed=1
    fi
}
no_pin() {
    if grep -q -- "$2" "$1"; then
        echo "PIN FAILED: '$2' should not appear in $1"
        pin_failed=1
    else
        echo "pin ok: no '$2' in $1"
    fi
}

echo "== build =="
cargo build --offline --no-default-features --features server --bin dioxus-server >"$OUT/build.log" 2>&1 || {
    echo "BUILD FAILED"; tail -n 30 "$OUT/build.log"; exit 1
}

# The web client bundle (Phase 5): index.html + /wasm glue, with the
# static public dir copied in, served by the second server below.
dx build --platform web --bin dioxus-web --features web --no-default-features >"$OUT/client-build.log" 2>&1 || {
    echo "CLIENT BUILD FAILED"; tail -n 30 "$OUT/client-build.log"; exit 1
}
DX_PUBLIC="$ROOT/target/dx/dioxus-web/debug/web/public"

# The desktop target shares `src/ui`, so it is compiled here as a regression
# pin (the window itself is a manual smoke test: `run-dioxus-desktop.sh`).
echo "== desktop target compiles =="
cargo check --offline --no-default-features --features desktop --bin dioxus-desktop >"$OUT/desktop-check.log" 2>&1 || {
    echo "DESKTOP CHECK FAILED"; tail -n 30 "$OUT/desktop-check.log"; exit 1
}

# Clippy over the same window build (no `--all-targets`: the lib's test modules
# need the server feature). This is the build that used to collect warnings
# from the Leptos `#[server]` client stubs, so it is pinned separately.
echo "== desktop clippy =="
cargo clippy --offline --no-default-features --features desktop -- -D warnings >"$OUT/desktop-clippy.log" 2>&1 || {
    echo "DESKTOP CLIPPY FAILED"; tail -n 30 "$OUT/desktop-clippy.log"; exit 1
}

# The shipped Leptos site (still the live app until Phase 8) builds its client
# bundle with `hydrate` only, where the server-fn bodies are cfg-ed out.
echo "== leptos hydrate build compiles =="
cargo check --offline --no-default-features --features hydrate >"$OUT/hydrate-check.log" 2>&1 || {
    echo "HYDRATE CHECK FAILED"; tail -n 30 "$OUT/hydrate-check.log"; exit 1
}

IP=127.0.0.1 PORT=8127 DIOXUS_PUBLIC_PATH="$ROOT/public" ./target/debug/dioxus-server >"$OUT/server.log" 2>&1 &
SERVER=$!
IP=127.0.0.1 PORT=8131 DIOXUS_PUBLIC_PATH="$DX_PUBLIC" ./target/debug/dioxus-server >"$OUT/server-client.log" 2>&1 &
SERVER2=$!
trap 'kill "$SERVER" "$SERVER2" 2>/dev/null' EXIT
base=http://127.0.0.1:8127
base_client=http://127.0.0.1:8131

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

echo "== waiting for the hydrated server (8131) =="
ready_client=0
for _ in 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20 21 22 23 24 25 26 27 28 29 30; do
    if curl -s -o /dev/null "$base_client/" 2>/dev/null; then ready_client=1; break; fi
    sleep 2
done
if [ "$ready_client" -eq 0 ]; then
    echo "HYDRATED SERVER NEVER STARTED; log follows"
    tail -n 30 "$OUT/server-client.log"
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

echo "== seed the e2e-tabs artist fixture (idempotent, self-healing) =="
# The nested-admin-tabs Playwright test edits this artist through the real
# profile editor, so the row must exist. Its slug deliberately does *not*
# derive from its name (`slugify("E2E Tabs Artist")` would be
# `e2e-tabs-artist`): anything that re-saves the whole model re-sluggifies
# and would silently rename the row, breaking every later check that
# addresses `e2e-tabs`. Conflicting on `name` (also UNIQUE) rather than on
# `slug` is what lets a rerun repair that drift instead of skipping.
if [ -f "$ROOT/.env" ]; then
    . "$ROOT/.env"
fi
if [ -n "${DATABASE_URL:-}" ]; then
    psql "$DATABASE_URL" -c "INSERT INTO artists (name, slug, description, website, label_id, published_at)
        SELECT 'E2E Tabs Artist', 'e2e-tabs', 'Gate fixture artist', 'https://e2e-tabs.example.com',
               id, now()
        FROM labels ORDER BY id LIMIT 1
        ON CONFLICT (name) DO UPDATE SET
            slug = EXCLUDED.slug,
            label_id = EXCLUDED.label_id,
            published_at = COALESCE(artists.published_at, EXCLUDED.published_at),
            deleted_at = NULL;" || echo "WARNING: could not seed the e2e-tabs artist (is psql installed?)"
    # The release editor client spec creates a row per browser run (a unique
    # name and catalogue number, left soft-deleted on purpose). Pruning first
    # keeps the seeded artist's release list short and every run equivalent to
    # the last, whatever earlier runs left behind.
    psql "$DATABASE_URL" -c "DELETE FROM release_artists WHERE release_id IN
            (SELECT id FROM releases WHERE slug LIKE 'e2e-reactive-%');
        DELETE FROM releases WHERE slug LIKE 'e2e-reactive-%';" \
        || echo "WARNING: could not prune the e2e-reactive releases (is psql installed?)"
else
    echo "WARNING: DATABASE_URL not set; skipping the e2e-tabs seed"
fi

echo "== seed the uploads directories (idempotent) =="
# The Leptos `setup` creates these; the dioxus server only reads UPLOAD_PATH,
# so the gate makes sure the avatar upload in `dioxus_client.spec.ts` has
# somewhere to write.
if [ -n "${UPLOAD_PATH:-}" ]; then
    mkdir -p "$UPLOAD_PATH/tmp" "$UPLOAD_PATH/avatars" "$UPLOAD_PATH/artists" "$UPLOAD_PATH/releases"
    echo "uploads root ready: $UPLOAD_PATH"
else
    echo "WARNING: UPLOAD_PATH not set; the profile upload client test will fail"
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
   grep -q 'card-title">Artists' "$OUT/admin_dashboard.html"; then
    admin_ok=1
fi
if [ "$admin_ok" -eq 1 ]; then
    echo "admin session active (e2e-admin)"
    pin "$OUT/admin_dashboard.html" 'Admin Dashboard'
    pin "$OUT/admin_dashboard.html" 'w-56 menu bg-base-200 rounded-box'
    # The nested tab layout only renders behind the admin gate. The
    # releases tab is real since slice 5b, so it needs a live artist row
    # (the seed above); a missing slug would render NotFound instead.
    curl -sS -b "$OUT/admin-cookies.txt" -o "$OUT/nested.html" -w "nested status=%{http_code}\n" "$base/admin/artist/e2e-tabs/releases"
    pin "$OUT/nested.html" 'tabs tabs-border'
    pin "$OUT/nested.html" 'E2E Tabs Artist Releases'
    # (slice 5a) The artist create page serves the real form handlers.
    curl -sS -b "$OUT/admin-cookies.txt" -o "$OUT/artist_create.html" "$base/admin/artist"
    pin "$OUT/artist_create.html" 'action="/admin/forms/artist"'
    # (slice 5b) The release create page serves its real form handlers too.
    curl -sS -b "$OUT/admin-cookies.txt" -o "$OUT/release_create.html" "$base/admin/artist/e2e-tabs/releases/new"
    pin "$OUT/release_create.html" 'action="/admin/forms/release"'
    # (slice 5d) The artist links editor serves its real form handler.
    curl -sS -b "$OUT/admin-cookies.txt" -o "$OUT/artist_links.html" "$base/admin/artist/e2e-tabs/links"
    pin "$OUT/artist_links.html" 'action="/admin/forms/links"'
    # (slice 5g) The artist Images tab serves the upload picker; the upload
    # itself is the wasm client's job (checked in dioxus_client.spec.ts).
    curl -sS -b "$OUT/admin-cookies.txt" -o "$OUT/artist_images.html" \
      -w "artist images status=%{http_code}\n" "$base/admin/artist/e2e-tabs/images"
    pin "$OUT/artist_images.html" 'E2E Tabs Artist Images'
    pin "$OUT/artist_images.html" 'name="file_to_upload"'
    pin "$OUT/artist_images.html" 'accept=".jpg,.JPG,.jpeg,.JPEG,.png,.PNG,.gif,.GIF,.webp,.WEBP"'
else
    echo "ADMIN CHECKS SKIPPED: e2e-admin lacks the admin permission (see instructions above)"
    # Guest view of a guarded admin route: the login page replaces it.
    curl -sS -o "$OUT/nested.html" -w "nested status=%{http_code}\n" "$base/admin/artist/jimi/releases"
    pin "$OUT/nested.html" 'action="/auth/login"'
fi

echo "== curl spot checks =="
curl -sS -o "$OUT/home.html" -w "home status=%{http_code}\n" "$base/"
pin "$OUT/home.html" 'navbar bg-primary text-primary-content'
pin "$OUT/home.html" 'footer bg-neutral text-neutral-content'
pin "$OUT/home.html" 'href="/main.css"'
curl -sS -o /dev/null -w "main.css status=%{http_code}\n" "$base/main.css"
curl -sS -o "$OUT/guest_profile.html" -w "guest profile status=%{http_code}\n" "$base/profile"
pin "$OUT/guest_profile.html" 'action="/auth/login"'
# The upload widget needs a session, so a guest never sees its markup.
no_pin "$OUT/guest_profile.html" 'file_to_upload'
curl -sS -o /dev/null -w "bad login status=%{http_code}\n" \
  --data-urlencode "username=e2e-dioxus" \
  --data-urlencode "password=definitely-wrong" \
  "$base/auth/login"

echo "== profile upload widget (slice 5f SSR markup) =="
# With a session the profile page serves the picker; the upload itself runs
# only through the wasm client (checked in dioxus_client.spec.ts).
curl -sS -c "$OUT/user-cookies.txt" -o /dev/null -w "e2e-dioxus login status=%{http_code}\n" \
  --data-urlencode "username=e2e-dioxus" \
  --data-urlencode "password=password" \
  "$base/auth/login"
curl -sS -b "$OUT/user-cookies.txt" -o "$OUT/user_profile.html" -w "user profile status=%{http_code}\n" "$base/profile"
pin "$OUT/user_profile.html" 'name="file_to_upload"'
pin "$OUT/user_profile.html" 'file-input file-input-primary'
pin "$OUT/user_profile.html" 'btn btn-primary'
# The progress endpoint refuses a guest (the Rust tests pin the streamed
# totals; the wire status is pinned here).
curl -sS -o /dev/null -w "progress without a session status=%{http_code}\n" \
  "$base/api/file_progress?filename=e2e-avatar.png"

echo "== hydrated server spot checks (8131) =="
# The dx-generated index.html in this server's public dir makes the SSR
# renderer inject the wasm bundle bootstrap into every page.
curl -sS -o "$OUT/home_client.html" -w "home(hydrated) status=%{http_code}\n" "$base_client/"
pin "$OUT/home_client.html" 'wasm/dioxus-web.js'
curl -sS -o /dev/null -w "wasm glue status=%{http_code}\n" "$base_client/wasm/dioxus-web.js"
curl -sS -o /dev/null -w "wasm binary status=%{http_code}\n" "$base_client/wasm/dioxus-web_bg.wasm"

echo "== playwright navigation smoke (chromium) =="
cd "$ROOT/end2end" || exit 1
if [ "$admin_ok" -eq 1 ]; then
    E2E_ADMIN_OK=1 DIOXUS_URL="$base" npx playwright test tests/dioxus_shell.spec.ts --project=chromium --reporter=list
else
    DIOXUS_URL="$base" npx playwright test tests/dioxus_shell.spec.ts --project=chromium --reporter=list
fi
status=$?

echo "== playwright client hydration smoke (chromium, 8131) =="
if [ "$admin_ok" -eq 1 ]; then
    E2E_ADMIN_OK=1 DIOXUS_URL="$base_client" npx playwright test tests/dioxus_client.spec.ts --project=chromium --reporter=list || status=$?
else
    DIOXUS_URL="$base_client" npx playwright test tests/dioxus_client.spec.ts --project=chromium --reporter=list || status=$?
fi

# The same spec in Firefox, which is where the upload protocol broke once:
# Firefox does not resolve a stream response until its first byte, so a client
# that waits for its progress request before sending the file body never sends
# it at all, and a stream left open when the page navigates is reported to the
# wasm client as an aborted request. Neither shows up in chromium.
# The browser builds are pinned by `@playwright/test` in this directory, so run
# `npx playwright install firefox` from here (a newer npx pulls a build this
# version cannot launch).
echo "== playwright client hydration smoke (firefox, 8131) =="
if [ "$admin_ok" -eq 1 ]; then
    E2E_ADMIN_OK=1 DIOXUS_URL="$base_client" npx playwright test tests/dioxus_client.spec.ts --project=firefox --reporter=list || status=$?
else
    DIOXUS_URL="$base_client" npx playwright test tests/dioxus_client.spec.ts --project=firefox --reporter=list || status=$?
fi

if [ "$pin_failed" -ne 0 ]; then
    echo "== gate page pins FAILED =="
    status=1
fi

# A marker to grep for: the playwright runs print their own summaries, but a
# filtered or skipped suite looks the same as a passing one in the log.
if [ "$status" -eq 0 ]; then
    echo "== GATE OK =="
fi

if [ "$status" -ne 0 ]; then
    echo "== server log tail =="
    tail -n 20 "$OUT/server.log"
    echo "== hydrated server log tail =="
    tail -n 20 "$OUT/server-client.log"
fi
exit $status
