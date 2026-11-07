#!/bin/sh
# Web dev launcher (MIGRATION.md Phase 6).
#
# Builds the wasm client and serves the hydrated site — the shape the deployed
# site has — in one command. The bare SSR server (`DIOXUS_PUBLIC_PATH=public`)
# serves no wasm bundle, so uploads and reactive forms do nothing there; use
# this when clicking through the app matters.
#
# `IP`, `PORT`, `DATABASE_URL` and `UPLOAD_PATH` are honoured by the server.
# Run from anywhere:  sh end2end/run-dioxus-web.sh
set -u
ROOT=/home/anthony/Projects/Rust/white-label
cd "$ROOT" || exit 1

dx build --platform web --bin dioxus-web --features web --no-default-features || exit 1

# The dx output is self-contained: every `public/` static plus the wasm bundle,
# and its `index.html` is what makes the SSR renderer inject the bootstrap.
DIOXUS_PUBLIC_PATH=target/dx/dioxus-web/debug/web/public \
    exec cargo run --no-default-features --features server --bin dioxus-server
