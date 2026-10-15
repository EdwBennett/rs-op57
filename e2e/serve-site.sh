#!/usr/bin/env bash
# Builds hello-core to WASM, fetches Verovio, and starts a throwaway
# `zola serve` for the Playwright test suite (see playwright.config.js's
# `webServer`).
#
# Not meant to be run by hand — `npx playwright test` (or
# ../tools/run-e2e-tests) starts and stops this automatically. Deliberately
# independent of ../tools/rebuild-my-zola-site: no systemd service, no
# permanent output directory, just a real build served on a dedicated
# test-only port until Playwright is done with it.
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.." # repo root

# rustup/cargo install wasm-pack into ~/.cargo/bin, which isn't always on
# PATH in non-interactive shells (e.g. when Playwright launches this script).
export PATH="${CARGO_HOME:-$HOME/.cargo}/bin:$PATH"

echo "==> Building hello-core for wasm32-unknown-unknown"
(cd crates/hello-core && wasm-pack build --target web --out-dir ../../site/static/wasm)

tools/fetch-verovio

echo "==> Serving the Zola site for e2e tests"
# --no-port-append matters: --base-url already names the port, and zola
# serve's default behaviour of also appending the port produces a broken
# double-port URL (http://127.0.0.1:1191:1191) that the page's WASM import
# then 404s on.
exec zola --root site serve \
  --port 1191 \
  --base-url http://127.0.0.1:1191 \
  --no-port-append
