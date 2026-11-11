#!/bin/sh
# Runtime gate: serve the Dioxus SSR app from the real crate against the dev
# database, then smoke-test shell navigation with Playwright (chromium and
# firefox) plus a few curl spot checks. A second server (8131) serves the
# `dx build` client bundle, and `tests/dioxus_client.spec.ts` checks
# hydration/router takeover against it. Uses ports 8127/8131 to keep clear
# of other local dev services.
#
# Run from anywhere:  sh end2end/run-dioxus-gate.sh
set -u

# Refuse to start on occupied ports: a leaked server on 8127/8131 would
# otherwise answer the pins from its own (stale) code and only fail as a
# confusing timing test.
for port in 8127 8131; do
    if ss -ltn 2>/dev/null | grep -q ":$port "; then
        echo "port $port is already in use — kill the stale server first"
        exit 1
    fi
done

ROOT=/home/anthony/Projects/Rust/white-label
OUT=/tmp/dioxus-shell-gate
rm -rf "$OUT"
mkdir -p "$OUT"
cd "$ROOT" || exit 1

# A missing page marker must fail the gate. `grep -c` prints a count and
# exits 0 whatever it counts, so pins never reached the exit status on their
# own (an images pin once sat at 0 for a whole development cycle without
# anyone noticing).
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

# The web client bundle: index.html + /wasm glue, with the
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
# need the server feature). The desktop cfg branches compile differently from
# the server build, so they are pinned separately.
echo "== desktop clippy =="
cargo clippy --offline --no-default-features --features desktop -- -D warnings >"$OUT/desktop-clippy.log" 2>&1 || {
    echo "DESKTOP CLIPPY FAILED"; tail -n 30 "$OUT/desktop-clippy.log"; exit 1
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
    # A public artist whose only job is to carry streaming links, so its artist
    # page always renders the "Listen to ..." logo tiles that the backing pin in
    # `tests/dioxus_shell.spec.ts` measures. It is separate from `e2e-tabs` on
    # purpose: the links save/clear tests delete `e2e-tabs`' rows mid-run, and
    # the specs run in parallel. `music_services` has no unique index over
    # (artist_id, platform), so the links guard with NOT EXISTS, not ON CONFLICT.
    psql "$DATABASE_URL" -c "INSERT INTO artists (name, slug, description, website, label_id, published_at)
        SELECT 'E2E Logos Artist', 'e2e-logos', 'Gate fixture artist for the streaming-logo pins',
               'https://e2e-logos.example.com', id, now()
        FROM labels ORDER BY id LIMIT 1
        ON CONFLICT (name) DO UPDATE SET
            slug = EXCLUDED.slug,
            label_id = EXCLUDED.label_id,
            published_at = COALESCE(artists.published_at, EXCLUDED.published_at),
            deleted_at = NULL;
        INSERT INTO music_services (artist_id, platform, url)
        SELECT a.id, link.platform::platform, link.url FROM artists a
        CROSS JOIN (VALUES ('Spotify', 'https://open.spotify.com/artist/e2e-logos'),
                           ('Bandcamp', 'https://e2e-logos.bandcamp.com'))
            AS link(platform, url)
        WHERE a.slug = 'e2e-logos'
          AND NOT EXISTS (SELECT 1 FROM music_services ms
              WHERE ms.artist_id = a.id AND ms.platform = link.platform::platform);" \
        || echo "WARNING: could not seed the e2e-logos streaming artist (is psql installed?)"
    # The release editor client spec creates a row per browser run (a unique
    # name and catalogue number, left soft-deleted on purpose). Pruning first
    # keeps the seeded artist's release list short and every run equivalent to
    # the last, whatever earlier runs left behind.
    psql "$DATABASE_URL" -c "DELETE FROM release_artists WHERE release_id IN
            (SELECT id FROM releases WHERE slug LIKE 'e2e-reactive-%');
        DELETE FROM releases WHERE slug LIKE 'e2e-reactive-%';" \
        || echo "WARNING: could not prune the e2e-reactive releases (is psql installed?)"
    # An audio fixture: a published release and track under `e2e-tabs`, which
    # the audio upload specs post to (the track editor's audio uploader is
    # addressed by artist, release and track slugs). Kept undeleted on purpose:
    # every run uploads another file to it. The track conflicts on its ISRC
    # rather than its name (`tracks` has no unique name index, and a UI rename
    # keeps the ISRC while re-slugifying), so a rerun repairs the slug back.
    # The audio column is repaired to a fixed name every run, and that file is
    # written in the uploads seed below: the public release page's player pins
    # run while the upload specs are replacing this row's audio, so they need
    # the row to name *some* file at all times (the exact name changes
    # mid-run, so the pins assert the `/uploads/tracks/` shape, not a name).
    psql "$DATABASE_URL" -c "INSERT INTO releases (name, slug, description, label_id, catalogue_number, primary_artist_id, published_at)
        SELECT 'E2E Audio Release', 'e2e-audio-release', 'Gate fixture release for the audio uploader',
               a.label_id, 'E2E-AUDIO-1', a.id, now()
        FROM artists a WHERE a.slug = 'e2e-tabs'
        ON CONFLICT (name) DO UPDATE SET
            slug = EXCLUDED.slug,
            label_id = EXCLUDED.label_id,
            catalogue_number = EXCLUDED.catalogue_number,
            primary_artist_id = EXCLUDED.primary_artist_id,
            published_at = COALESCE(releases.published_at, EXCLUDED.published_at),
            deleted_at = NULL;
        INSERT INTO tracks (name, slug, description, lyrics, isrc_code, bpm, primary_artist_id, release_id, track_number, published_at, audio)
        SELECT 'E2E Audio Track', 'e2e-audio-track', 'Gate fixture track for the audio uploader',
               '', 'E2EAUD12345', 120, r.primary_artist_id, r.id, 1, now(),
               '999999999-e2e-audio-track.wav'
        FROM releases r WHERE r.slug = 'e2e-audio-release'
        ON CONFLICT (isrc_code) DO UPDATE SET
            name = EXCLUDED.name,
            slug = EXCLUDED.slug,
            release_id = EXCLUDED.release_id,
            primary_artist_id = EXCLUDED.primary_artist_id,
            published_at = COALESCE(tracks.published_at, EXCLUDED.published_at),
            deleted_at = NULL,
            audio = EXCLUDED.audio;
        INSERT INTO release_artists (artist_id, release_id)
        SELECT a.id, r.id FROM artists a, releases r
            WHERE a.slug = 'e2e-tabs' AND r.slug = 'e2e-audio-release'
        ON CONFLICT (artist_id, release_id) DO NOTHING;" \
        || echo "WARNING: could not seed the audio fixture (is psql installed?)"
    # The auto-append fixtures: a published artist (`e2e-radio`) with two
    # published releases — album A carries one audio track, album B two — so
    # the player's continuation fetch always has the same three-track
    # catalogue to offer. Separate from `e2e-tabs` on purpose: the pins that
    # watch the *no*-continuation case run in parallel with the specs that
    # mutate that fixture, and the fetch keys on the playing track's primary
    # artist. Tracks conflict on fresh ISRCs (`tracks` has no unique name
    # index); the audio columns are hard-repaired to fixed names every run
    # and their bytes written in the uploads seed below (the pins never
    # decode sound).
    psql "$DATABASE_URL" -c "INSERT INTO artists (name, slug, description, website, label_id, published_at)
        SELECT 'E2E Radio Artist', 'e2e-radio', 'Gate fixture artist for the auto-append pins',
               'https://e2e-radio.example.com', id, now()
        FROM labels ORDER BY id LIMIT 1
        ON CONFLICT (name) DO UPDATE SET
            slug = EXCLUDED.slug,
            label_id = EXCLUDED.label_id,
            published_at = COALESCE(artists.published_at, EXCLUDED.published_at),
            deleted_at = NULL;
        INSERT INTO releases (name, slug, description, label_id, catalogue_number, primary_artist_id, published_at, primary_image)
        SELECT 'E2E Radio Album A', 'e2e-radio-album-a', 'Auto-append fixture: the played album',
               a.label_id, 'E2E-RADIO-A', a.id, now(), '999999993-e2e-radio-album-a.jpg'
        FROM artists a WHERE a.slug = 'e2e-radio'
        ON CONFLICT (name) DO UPDATE SET
            slug = EXCLUDED.slug,
            label_id = EXCLUDED.label_id,
            catalogue_number = EXCLUDED.catalogue_number,
            primary_artist_id = EXCLUDED.primary_artist_id,
            published_at = COALESCE(releases.published_at, EXCLUDED.published_at),
            deleted_at = NULL,
            primary_image = EXCLUDED.primary_image;
        INSERT INTO releases (name, slug, description, label_id, catalogue_number, primary_artist_id, published_at, primary_image)
        SELECT 'E2E Radio Album B', 'e2e-radio-album-b', 'Auto-append fixture: the continuation album',
               a.label_id, 'E2E-RADIO-B', a.id, now(), '999999992-e2e-radio-album-b.jpg'
        FROM artists a WHERE a.slug = 'e2e-radio'
        ON CONFLICT (name) DO UPDATE SET
            slug = EXCLUDED.slug,
            label_id = EXCLUDED.label_id,
            catalogue_number = EXCLUDED.catalogue_number,
            primary_artist_id = EXCLUDED.primary_artist_id,
            published_at = COALESCE(releases.published_at, EXCLUDED.published_at),
            deleted_at = NULL,
            primary_image = EXCLUDED.primary_image;
        INSERT INTO tracks (name, slug, description, lyrics, isrc_code, bpm, primary_artist_id, release_id, track_number, published_at, audio)
        SELECT 'Radio Track A1', 'e2e-radio-track-a1', 'Auto-append fixture track',
               '', 'E2ERADIOA1', 120, r.primary_artist_id, r.id, 1, now(),
               '999999996-e2e-radio-a1.wav'
        FROM releases r WHERE r.slug = 'e2e-radio-album-a'
        ON CONFLICT (isrc_code) DO UPDATE SET
            name = EXCLUDED.name,
            slug = EXCLUDED.slug,
            release_id = EXCLUDED.release_id,
            primary_artist_id = EXCLUDED.primary_artist_id,
            published_at = COALESCE(tracks.published_at, EXCLUDED.published_at),
            deleted_at = NULL,
            audio = EXCLUDED.audio;
        INSERT INTO tracks (name, slug, description, lyrics, isrc_code, bpm, primary_artist_id, release_id, track_number, published_at, audio)
        SELECT 'Radio Track B1', 'e2e-radio-track-b1', 'Auto-append fixture track',
               '', 'E2ERADIOB1', 120, r.primary_artist_id, r.id, 1, now(),
               '999999995-e2e-radio-b1.wav'
        FROM releases r WHERE r.slug = 'e2e-radio-album-b'
        ON CONFLICT (isrc_code) DO UPDATE SET
            name = EXCLUDED.name,
            slug = EXCLUDED.slug,
            release_id = EXCLUDED.release_id,
            primary_artist_id = EXCLUDED.primary_artist_id,
            published_at = COALESCE(tracks.published_at, EXCLUDED.published_at),
            deleted_at = NULL,
            audio = EXCLUDED.audio;
        INSERT INTO tracks (name, slug, description, lyrics, isrc_code, bpm, primary_artist_id, release_id, track_number, published_at, audio)
        SELECT 'Radio Track B2', 'e2e-radio-track-b2', 'Auto-append fixture track',
               '', 'E2ERADIOB2', 120, r.primary_artist_id, r.id, 2, now(),
               '999999994-e2e-radio-b2.wav'
        FROM releases r WHERE r.slug = 'e2e-radio-album-b'
        ON CONFLICT (isrc_code) DO UPDATE SET
            name = EXCLUDED.name,
            slug = EXCLUDED.slug,
            release_id = EXCLUDED.release_id,
            primary_artist_id = EXCLUDED.primary_artist_id,
            published_at = COALESCE(tracks.published_at, EXCLUDED.published_at),
            deleted_at = NULL,
            audio = EXCLUDED.audio;
        INSERT INTO release_artists (artist_id, release_id)
        SELECT a.id, r.id FROM artists a, releases r
            WHERE a.slug = 'e2e-radio'
              AND r.slug IN ('e2e-radio-album-a', 'e2e-radio-album-b')
        ON CONFLICT (artist_id, release_id) DO NOTHING;" \
        || echo "WARNING: could not seed the e2e-radio fixtures (is psql installed?)"
    # A housekeeping fixture: one page soft-deleted *past* the 90 day purge
    # window, kept aged on every run. The Housekeeping preview tests expect
    # to see `E2E Purge Page` in the listing; no test clicks the confirm
    # button, so the page survives run after run.
    psql "$DATABASE_URL" -c "INSERT INTO pages (name, slug, description, body, label_id, published_at, deleted_at)
        SELECT 'E2E Purge Page', 'e2e-purge-page', 'Housekeeping preview fixture',
               'Left soft-deleted past the purge window on purpose.',
               id, now(), now() - interval '91 days'
        FROM labels
        WHERE NOT EXISTS (SELECT 1 FROM pages WHERE slug = 'e2e-purge-page')
        ORDER BY id LIMIT 1;
        UPDATE pages SET deleted_at = now() - interval '91 days'
            WHERE slug = 'e2e-purge-page';" \
        || echo "WARNING: could not seed the housekeeping fixture page (is psql installed?)"
    # A purge fixture that *names upload files*: a release past the window
    # with a primary image, and a track under it with an audio file (doomed
    # through its release). The preview lists those names in its
    # "Upload files" column (the files themselves are written in the uploads
    # seed below). Aged on every run; no test clicks the confirm button, so
    # the rows survive run after run.
    psql "$DATABASE_URL" -c "INSERT INTO releases (name, slug, description, label_id, catalogue_number, primary_artist_id, published_at, deleted_at, primary_image)
        SELECT 'E2E Purge Release', 'e2e-purge-release', 'Housekeeping purge fixture with an upload',
               a.label_id, 'E2E-PURGE-1', a.id, now(), now() - interval '91 days',
               '999999999-e2e-purge-release.jpg'
        FROM artists a WHERE a.slug = 'e2e-tabs'
        ON CONFLICT (name) DO UPDATE SET
            slug = EXCLUDED.slug,
            label_id = EXCLUDED.label_id,
            catalogue_number = EXCLUDED.catalogue_number,
            primary_artist_id = EXCLUDED.primary_artist_id,
            published_at = COALESCE(releases.published_at, EXCLUDED.published_at),
            deleted_at = EXCLUDED.deleted_at,
            primary_image = EXCLUDED.primary_image;
        INSERT INTO tracks (name, slug, description, lyrics, isrc_code, bpm, primary_artist_id, release_id, track_number, published_at, deleted_at, audio)
        SELECT 'E2E Purge Track', 'e2e-purge-track', 'Housekeeping purge fixture with an audio upload',
               '', 'E2EPURGE123', 100, r.primary_artist_id, r.id, 1,
               now(), now() - interval '91 days', '999999999-e2e-purge-track.wav'
        FROM releases r WHERE r.slug = 'e2e-purge-release'
        ON CONFLICT (isrc_code) DO UPDATE SET
            name = EXCLUDED.name,
            slug = EXCLUDED.slug,
            release_id = EXCLUDED.release_id,
            primary_artist_id = EXCLUDED.primary_artist_id,
            published_at = COALESCE(tracks.published_at, EXCLUDED.published_at),
            deleted_at = EXCLUDED.deleted_at,
            audio = EXCLUDED.audio;" \
        || echo "WARNING: could not seed the purge upload fixtures (is psql installed?)"
else
    echo "WARNING: DATABASE_URL not set; skipping the e2e-tabs seed"
fi

echo "== seed the uploads directories (idempotent) =="
# The dioxus server only reads UPLOAD_PATH and never creates these directories,
# so the gate makes sure the avatar upload in `dioxus_client.spec.ts` has
# somewhere to write.
if [ -n "${UPLOAD_PATH:-}" ]; then
    mkdir -p "$UPLOAD_PATH/tmp" "$UPLOAD_PATH/avatars" "$UPLOAD_PATH/artists" "$UPLOAD_PATH/releases" "$UPLOAD_PATH/tracks"
    # The bytes the purge upload fixtures name. Contents irrelevant: the
    # preview lists names straight from the database, and only a confirm
    # click — which no test performs — would ever remove them.
    printf 'E2E purge preview fixture, not a real image' > "$UPLOAD_PATH/releases/999999999-e2e-purge-release.jpg"
    printf 'E2E purge preview fixture, not a real wave' > "$UPLOAD_PATH/tracks/999999999-e2e-purge-track.wav"
    # The bytes the audio fixture row names. Contents irrelevant: the public
    # player pins never decode sound, they only check that a src appears and
    # survives navigation. Rewritten every run, and any file the upload specs
    # add later just replaces the row's name (this one becomes an orphan,
    # which the sweep fixtures already keep the purge preview honest about).
    printf 'E2E player pin fixture, not a real wave' > "$UPLOAD_PATH/tracks/999999999-e2e-audio-track.wav"
    # The bytes the e2e-radio fixture rows name: the same zero-sample trick —
    # the auto-append pins watch the bar's queue state, never the sound.
    printf 'E2E auto-append pin fixture, not a real wave' > "$UPLOAD_PATH/tracks/999999996-e2e-radio-a1.wav"
    printf 'E2E auto-append pin fixture, not a real wave' > "$UPLOAD_PATH/tracks/999999995-e2e-radio-b1.wav"
    printf 'E2E auto-append pin fixture, not a real wave' > "$UPLOAD_PATH/tracks/999999994-e2e-radio-b2.wav"
    # The album art the radio fixture releases name: the auto-append pins
    # watch the bar fall back to it (the radio tracks never get an image of
    # their own), so the bytes only need to exist for the `<img>` to load.
    printf 'E2E album-art fallback pin fixture, not a real jpeg' > "$UPLOAD_PATH/releases/999999993-e2e-radio-album-a.jpg"
    printf 'E2E album-art fallback pin fixture, not a real jpeg' > "$UPLOAD_PATH/releases/999999992-e2e-radio-album-b.jpg"
    # The unreferenced-file sweep fixture: a file no row (or seed) ever
    # names, which the preview must always list and a confirmed purge
    # would remove. Recreated every run, so an accidental manual purge
    # costs nothing.
    printf 'E2E replacement orphan, named by no row' > "$UPLOAD_PATH/artists/999999998-e2e-orphan.png"
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
    # releases tab is a real form, so it needs a live artist row
    # (the seed above); a missing slug would render NotFound instead.
    curl -sS -b "$OUT/admin-cookies.txt" -o "$OUT/nested.html" -w "nested status=%{http_code}\n" "$base/admin/artist/e2e-tabs/releases"
    pin "$OUT/nested.html" 'tabs tabs-border'
    pin "$OUT/nested.html" 'E2E Tabs Artist Releases'
    # The artist create page serves the real form handlers.
    curl -sS -b "$OUT/admin-cookies.txt" -o "$OUT/artist_create.html" "$base/admin/artist"
    pin "$OUT/artist_create.html" 'action="/admin/forms/artist"'
    # The release create page serves its real form handlers too.
    curl -sS -b "$OUT/admin-cookies.txt" -o "$OUT/release_create.html" "$base/admin/artist/e2e-tabs/releases/new"
    pin "$OUT/release_create.html" 'action="/admin/forms/release"'
    # The artist links editor serves its real form handler.
    curl -sS -b "$OUT/admin-cookies.txt" -o "$OUT/artist_links.html" "$base/admin/artist/e2e-tabs/links"
    pin "$OUT/artist_links.html" 'action="/admin/forms/links"'
    # The artist Images tab serves the upload picker; the upload
    # itself is the wasm client's job (checked in dioxus_client.spec.ts).
    curl -sS -b "$OUT/admin-cookies.txt" -o "$OUT/artist_images.html" \
      -w "artist images status=%{http_code}\n" "$base/admin/artist/e2e-tabs/images"
    pin "$OUT/artist_images.html" 'E2E Tabs Artist Images'
    pin "$OUT/artist_images.html" 'name="file_to_upload"'
    pin "$OUT/artist_images.html" 'accept=".jpg,.JPG,.jpeg,.JPEG,.png,.PNG,.gif,.GIF,.webp,.WEBP"'
    # The track editor carries both uploaders: the audio slot (its own accept
    # list, and the release slug the javascript-off form needs to come back to
    # this page) and the image slot, each naming its configuration so a
    # javascript-off outcome redirects to the widget that produced it.
    curl -sS -b "$OUT/admin-cookies.txt" -o "$OUT/track_editor.html" -w "track editor status=%{http_code}\n" \
      "$base/admin/artist/e2e-tabs/release/e2e-audio-release/track/e2e-audio-track"
    pin "$OUT/track_editor.html" 'E2E Audio Track - E2E Audio Release - E2E Tabs Artist'
    pin "$OUT/track_editor.html" 'name="file_type" value="TrackAudio"'
    pin "$OUT/track_editor.html" 'name="file_type" value="TrackImage"'
    pin "$OUT/track_editor.html" 'accept=".mp3,.MP3,.wav,.WAV,.flac,.FLAC,.ogg,.OGG"'
    pin "$OUT/track_editor.html" 'accept=".jpg,.JPG,.jpeg,.JPEG,.png,.PNG,.gif,.GIF,.webp,.WEBP"'
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
# The continuation endpoint: at this point the only playable `e2e-tabs` track
# is `e2e-audio-track` (the purge fixture is soft-deleted), so excluding it
# leaves nothing — the empty-list shape is pinned here, while the client spec
# pins the appending itself against the `e2e-radio` fixtures.
curl -sS -o "$OUT/continuation.json" -w "continuation status=%{http_code}\n" \
  "$base/api/get_continuation_tracks?artist_slug=e2e-tabs&exclude_slugs=e2e-audio-track"
pin "$OUT/continuation.json" '"tracks":\[\]'
# The public album B page: its rows show the track artwork chain — the radio
# tracks have none of their own, and the release's seeded album art (bytes
# written in the uploads seed above) is the next link, rendered server-side.
curl -sS -o "$OUT/radio_release.html" -w "radio release status=%{http_code}\n" \
  "$base/artists/e2e-radio/e2e-radio-album-b"
pin "$OUT/radio_release.html" 'src="/uploads/releases/999999992-e2e-radio-album-b.jpg" alt="Radio Track B1"'
# The play counts live in the browser, so the server always ships the plain
# Play button: a server-rendered row never shows `No plays left` and never
# counts down.
no_pin "$OUT/radio_release.html" 'No plays left'
no_pin "$OUT/radio_release.html" 'plays left'
# The upload widget needs a session, so a guest never sees its markup.
no_pin "$OUT/guest_profile.html" 'file_to_upload'
curl -sS -o /dev/null -w "bad login status=%{http_code}\n" \
  --data-urlencode "username=e2e-dioxus" \
  --data-urlencode "password=definitely-wrong" \
  "$base/auth/login"

echo "== profile upload widget (SSR markup) =="
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
