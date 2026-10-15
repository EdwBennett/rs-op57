# rs_hello

A template for a Rust workspace with a shared library, a CLI, and a
[Zola](https://www.getzola.org/) static site deployed to GitHub Pages.

> **If you cloned this repo:** It carries
> machine-specific automation: `tools/rebuild-my-zola-site` (see
> `docs/Run local-only Zola html_css_javascript_wasm webpage.md`) assumes a systemd user
> service serving the built site at `http://127.0.0.1:8080` and local tooling paths,
> `.claude/settings.json`
> runs that script automatically (via Claude Code hooks) whenever `site/` or
> `crates/hello-core/` changes.

## Layout

| Path | What it is |
| --- | --- |
| `crates/hello-core` | Shared logic, compiled both natively (`rlib`, for `hello-cli`) and to WebAssembly (`cdylib`, for the Zola site): `random_consecutive_primes` (bare numbers only — the sentence around them is the consumer's), and `NoteIndex`, which reads the MEI Verovio makes from a score and describes a note in words for the piano page (that text *is* the feature, so it lives here, tested by `cargo test`). DOM updates live in each consumer, not here. |
| `crates/hello-cli` | Command-line consumer of `hello-core`. |
| `site/` | Zola static site — not a Rust crate, no `Cargo.toml` — deployed to GitHub Pages. Loads `hello-core` as WASM and builds the displayed sentence in its own inline JS. The `/piano/` page (`site/content/piano/`, a page bundle with its MusicXML score) engraves the score in the browser with a self-hosted, pinned [Verovio](https://www.verovio.org) (LGPL-3.0-or-later) — see `tools/fetch-verovio`. Clicking a note highlights it and shows `hello-core`'s description of it below the score. |
| `.github/workflows` | `ci.yml` (fmt, clippy, tests, WASM + site build check) and `pages.yml` (builds the WASM bundle, then deploys the site). |
| `tools/` | Personal local-dev scripts, not part of the published site or CI — see `docs/Run local-only Zola html_css_javascript_wasm webpage.md`. `rebuild-my-zola-site` is also run automatically by a Claude Code hook (`.claude/settings.json` / `.claude/hooks/`) on `site/` or `hello-core` changes. `run-e2e-tests` and `fetch-verovio` are the exceptions — they're portable (`fetch-verovio` also runs in both workflows). |
| `e2e/` | Playwright end-to-end test(s) for the site — see "Testing the site end-to-end" below. |

## Run

```sh
cargo run -p hello-cli
cargo test --workspace
```

Site (needs [Zola](https://www.getzola.org/documentation/getting-started/installation/) and
[`wasm-pack`](https://rustwasm.github.io/wasm-pack/installer/) installed locally, plus the
`wasm32-unknown-unknown` target: `rustup target add wasm32-unknown-unknown`):

```sh
cd crates/hello-core
wasm-pack build --target web --out-dir ../../site/static/wasm
cd ../..
tools/fetch-verovio
cd site
zola serve
```

`site/static/wasm/` is generated (gitignored) — rebuild it with the `wasm-pack` command above
whenever `hello-core` changes. `site/static/verovio/` is generated (gitignored) too, by
`tools/fetch-verovio` (a no-op once the pinned version is in place; edit the script to upgrade).

## Testing the site end-to-end

[Playwright](https://playwright.dev/) drives a real browser against a throwaway local build of
the site (own `zola serve`, own port — no personal setup needed, and nothing is left running
afterwards). First time on a machine (needs [Node.js](https://nodejs.org/) + npm):

```sh
cd e2e
npm install
npx playwright install --with-deps
```

After that:

```sh
tools/run-e2e-tests
```

which is just `cd e2e && npx playwright test` — see that script for options (e.g. `--headed` to
watch it run in a real browser window).

To run everything — `cargo test --workspace`, then the e2e tests (only if the Rust tests pass):

```sh
tools/run-all-tests
```

## Deploying to GitHub Pages

1. In the repo settings, go to **Pages** and set **Source** to **GitHub Actions**.
2. Push to `main`/`master`. `pages.yml` builds `site/` with Zola and deploys it.

## Using this template

1. Click **Use this template** on GitHub (enable *Template repository* in the repo settings first).
2. Rename: replace `hello` in crate names and directories, and update `repository` in the root
   `Cargo.toml`.
3. Update `site/config.toml`: `base_url` (must match `https://<user-or-org>.github.io/<repo>`),
   `title`, `description`.

## Versions

The Zola version used in CI and in the Pages deploy is pinned in both `.github/workflows/ci.yml`
and `.github/workflows/pages.yml` (`ZOLA_VERSION`) — keep the two in sync when bumping it.
