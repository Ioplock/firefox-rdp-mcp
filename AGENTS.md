# Working on firefox-rdp-mcp

This repository contains the Rust stdio MCP server, a pinned patched ff-rdp
backend, Nix packaging and tests. Firefox is an external, already running browser;
the build/dev shell must not launch or depend on a GUI browser or an AI client.
README.md is the English default; keep README.ru.md equivalent when behavior,
installation, configuration or verification changes.

## Build and verify

- Build sequentially: `nix build --cores 2 --max-jobs 1`.
- Run `nix flake check --cores 2 --max-jobs 1`: Rust tests, formatting, fixed
  page-script tests and an offline stdio protocol check. These must not require a browser or network.
- `nix develop` supplies cargo/rustc/rustfmt/clippy, Python, Node.js and the pinned
  patched backend. Run `cargo fmt --check` and `cargo test --locked -j 2` as needed.
- Live tests are opt-in: `python3 tests/browser_smoke.py ./result/bin/firefox-rdp-mcp`.
  They use the existing local Firefox at FF_RDP_HOST/FF_RDP_PORT, default
  127.0.0.1:6000, and create/close only their own fixture tabs.
- Never start another browser, Selenium, geckodriver, Marionette, WebDriver,
  Playwright or Puppeteer unless the user explicitly asks.
- If verification/CAPTCHA/MFA appears, stop interacting with that page and ask
  the user to complete it; wait for confirmation and take a fresh snapshot.
- Never close or navigate user-created tabs during tests. Inspect existing tabs
  first, and verify their identities and URLs are preserved after the test.

## Implementation constraints

- stdout is exclusively MCP JSON-RPC. Log diagnostics to stderr; never log
  form contents, cookies or JavaScript expressions.
- Keep a single total deadline per tool, including queue time. Cancellation
  must stop the outstanding backend process; do not automatically retry actions.
- Tab handles use stable browsingContextID (`bc:ID`). RDP actor IDs belong to a
  connection and cannot identify tabs across separate ff-rdp invocations.
- Browser-chrome evaluation is private and fixed-purpose. Never expose it as a
  tool accepting arbitrary caller JavaScript. Page evaluation stays page-scoped.
- Preserve complete URLs and refs when compacting; mark omitted results.
- Keep Firefox profile/session data outside tracked sources and Nix inputs.
  Stage explicit source paths; never add runtime profiles, logs or session exports.
- Changes to backend CLI contracts must update its pinned patch/tests/docs.

## Suggested consumer-agent instructions

Call firefox_status first; prefer bc:ID handles and snapshot refs. Use
firefox_find_elements for role/name lookup; inspect all matches before choosing
a ref. Compact snapshots prioritize dialogs/content; format=tree retains the
backend tree. Use firefox_discover_lists extraction examples / firefox_extract
for repeated cards, queries for text, and pagination after truncation. Close
authorized tabs with firefox_close_page and an explicit bc:ID. When an action
returns partial or times out, inspect/wait before deciding whether another
action is needed; target_missing requires a current tab selection, not reload.
Use expect/condition to observe changes, and verify application state before
claiming success: a click alone is not proof. Leave requested result tabs open.
The user may change Firefox at any time: refresh the snapshot before interacting
with a page they have changed.
