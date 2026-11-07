#!/bin/sh
# Desktop dev launcher.
#
# Builds the desktop client with `dx`, puts the app's static files where the
# native asset resolver looks for them, then opens the window against the API.
#
# `dx` only bundles files registered with `asset!()`, and this app keeps its
# statics in `public/` (shared with the web and SSR targets), so they are
# copied into the bundle here. `dx bundle` will take them from
# `[bundle] resources` instead.
#
# Start the API first, if it is not already running:
#   IP=127.0.0.1 PORT=8127 DIOXUS_PUBLIC_PATH=public ./target/debug/dioxus-server
#
# Run from anywhere:  sh end2end/run-dioxus-desktop.sh
set -u
ROOT=/home/anthony/Projects/Rust/white-label
BUNDLE="$ROOT/target/dx/dioxus-desktop/debug/linux/app"
cd "$ROOT" || exit 1

dx build --platform desktop --bin dioxus-desktop --features desktop --no-default-features || exit 1

# `dioxus-asset-resolver::native` serves `/assets/<path>` from the executable's
# own directory (see `get_asset_root`), which is why CSS and the service icons
# have to be here for the window to render styled.
mkdir -p "$BUNDLE/assets"
cp -R "$ROOT/public/." "$BUNDLE/assets/"

# The API origin for server fns and uploaded files (`WHITE_LABEL_API_URL`,
# default matches the gate's SSR-only server on 8127).
API_ORIGIN=${WHITE_LABEL_API_URL:-http://127.0.0.1:8127}
echo "opening the desktop window against $API_ORIGIN"
WHITE_LABEL_API_URL="$API_ORIGIN" exec "$BUNDLE/dioxus-desktop"
