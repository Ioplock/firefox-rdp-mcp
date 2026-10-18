# firefox-rdp-mcp

[Русская версия](README.ru.md)

A Rust **stdio MCP server** for a Firefox window shared by a human and an AI
agent. It connects to an existing Firefox DevTools Remote Debugging Protocol
(RDP) server. It does not launch Firefox, install extensions or use WebDriver.

The Nix package includes a pinned [ff-rdp](https://github.com/ractive/ff-rdp)
backend with a `bc:ID` tab-selection patch. Browser-level RDP operations create
and navigate tabs; page inspection and interaction use ff-rdp.

## Build and run

Requires Nix with `nix-command` and `flakes` enabled. Outputs are provided for
`x86_64-linux` and `aarch64-linux`; verification was performed on x86_64 NixOS.
The development shell contains build/test tools and the backend, **no Firefox or
AI client**. First builds may download dependencies.

```sh
git clone git@github.com:Ioplock/firefox-rdp-mcp.git
cd firefox-rdp-mcp
nix build --cores 2 --max-jobs 1
# For an MCP client, not an interactive CLI:
./result/bin/firefox-rdp-mcp
```

`nix run .` starts the same stdio server. stdout is reserved for JSON-RPC;
diagnostics go to stderr. It waits for the MCP client's initialize request.

## Add to another project's flake

```nix
{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    firefox-rdp-mcp.url = "github:Ioplock/firefox-rdp-mcp";
  };
  outputs = { nixpkgs, firefox-rdp-mcp, ... }:
    let
      system = "x86_64-linux";
      pkgs = import nixpkgs { inherit system; };
    in {
      devShells.${system}.default = pkgs.mkShell {
        packages = [ firefox-rdp-mcp.packages.${system}.default ];
      };
    };
}
```

Run the MCP client from that project's `nix develop` environment. Commit its
`flake.lock` to pin the MCP revision. To reuse the consumer's compatible nixpkgs,
optionally set `inputs.firefox-rdp-mcp.inputs.nixpkgs.follows = "nixpkgs"`.
For a private repository use `git+ssh://git@github.com/Ioplock/firefox-rdp-mcp.git`
instead of the GitHub shorthand.

An overlay is also available:

```nix
pkgs = import nixpkgs {
  inherit system;
  overlays = [ firefox-rdp-mcp.overlays.default ];
};
# pkgs.firefox-rdp-mcp and pkgs.ff-rdp
```

## MCP client configuration

For clients using `mcpServers`, start the client inside the consumer's dev shell:

```json
{
  "mcpServers": {
    "firefox": {
      "command": "firefox-rdp-mcp",
      "env": { "FF_RDP_HOST": "127.0.0.1", "FF_RDP_PORT": "6000" }
    }
  }
}
```

OpenCode uses a different local configuration format:

```json
{
  "$schema": "https://opencode.ai/config.json",
  "mcp": {
    "firefox": {
      "type": "local",
      "command": ["firefox-rdp-mcp"],
      "enabled": true,
      "timeout": 130000,
      "environment": { "FF_RDP_HOST": "127.0.0.1", "FF_RDP_PORT": "6000" }
    }
  }
}
```

A GUI client started outside the shell needs an absolute installed executable
path, or a command array such as
`["nix", "run", "--cores", "2", "--max-jobs", "1", "github:Ioplock/firefox-rdp-mcp", "--"]`.
Prebuild before connecting to avoid client startup timeouts. Reconnect MCP after
updating the package; an already running server keeps its previous version.

## Firefox requirements and setup

Use **Firefox Desktop with DevTools RDP and browser-chrome debugging enabled**.
The current implementation was live-tested with Firefox **157.0 on Linux**;
other versions and Firefox forks have not been validated. Firefox is deliberately
not a dependency of the MCP build. Reuse an existing correctly configured window;
do not start another instance if the shared browser is already running.

For a **new dedicated profile**, create its directory outside source trees:

```sh
FIREFOX_PROFILE="${XDG_STATE_HOME:-$HOME/.local/state}/firefox-rdp-mcp/profile"
mkdir -p "$FIREFOX_PROFILE"
```

Create `user.js` in that profile with the following settings. When reusing a
profile, merge them with its existing preferences instead of overwriting the file.

```js
user_pref("devtools.debugger.remote-enabled", true);
user_pref("devtools.chrome.enabled", true);
user_pref("devtools.debugger.force-local", true);
user_pref("devtools.debugger.prompt-connection", false);
```

Start the dedicated browser manually:

```sh
firefox --no-remote --profile "$FIREFOX_PROFILE" --start-debugger-server 6000 about:blank
```

`devtools.chrome.enabled` is needed for native tab creation/navigation. Keep
`force-local` enabled: RDP exposes browser control without authentication. On Linux,
check `ss -ltn 'sport = :6000'`; listeners must be loopback only. Use
`--start-debugger-server`, not `--remote-debugging-port` (a different protocol).
Preference changes require restarting that Firefox instance.

Keep profiles, cookies and session data out of Git **and Nix source inputs**.
A non-Git local path flake can copy ignored runtime files into the Nix store;
placing the profile outside project source trees avoids that problem.

Upstream documentation: [Firefox command-line options](https://firefox-source-docs.mozilla.org/browser/CommandLineParameters.html),
[Browser Toolbox settings](https://firefox-source-docs.mozilla.org/devtools-user/browser_toolbox/index.html),
[RDP protocol](https://firefox-source-docs.mozilla.org/devtools/backend/protocol.html).

## Tools and behavior

26 tools cover status/tabs, snapshots, text, navigation, click/type/scroll/history,
evaluation, console/network, screenshots, storage/cookies, styles/sources,
performance, readiness waits and list discovery/extraction.

- Call `firefox_status` first. Use stable `bc:ID` handles from status/list results.
  URL/title selectors must be unambiguous. Selecting a page sets the MCP's default
  target; it does not focus the GUI tab. Closed targets require reselection.
- Prefer snapshot refs for interactions. The human can change a page at any time;
  refresh the snapshot after their changes. Handle CAPTCHA/MFA manually.
- `firefox_navigate` supports `new_tab`, `wait` (`interactive` by default,
  `complete` or `loading`) and `wait_selector`. A 10-second readiness stage may
  return `partial`; use `firefox_wait` or inspect, rather than repeating the action.
- Tools are serialized. `timeout_ms` is a total deadline including queue time:
  default 25000, clamped to 100–120000 ms. Cancellation/timeouts stop the backend
  process. An action may already have happened; never blindly retry it.
- Structured JSON responses are capped at 16000 bytes with an omission marker;
  image responses are separate. Retained URLs and refs remain complete. Use
  queries, `firefox_discover_lists`, and `firefox_extract` pagination (`offset`,
  `next_offset`, `has_more`) for large pages. A field's `max` limits text/attributes,
  while hrefs retain their full navigable URL.

### Agent-friendly interactions

- `firefox_close_page` requires an explicit `{"tab":"bc:58"}`. It closes only that
  tab, reports `closed`, remaining `tabs`, and the MCP default `selected` target.
  `closed:false` means closure was not observed (for example a page may request
  confirmation); inspect before retrying. Closing the default target selects a
  surviving target. Only close tabs the user authorized closing.
- `firefox_find_elements` accepts `role`, required `name`, and `exact` (default
  true; false enables case-insensitive substring matching). It returns all
  matches with refs, `total`, `offset`, `next_offset`, `has_more`, and `omitted`.
  Use a returned ref with click/type; never assume the first of multiple matches
  is correct. Labels and names are derived from the DOM, not a complete platform
  accessibility implementation. Hidden/inert elements are excluded; disabled
  controls are reported. This operates in the main document; frame/shadow content
  may require the backend tree or other inspection.
- `firefox_snapshot` now defaults to `format:"compact"`: a flat list of
  interactive elements, with dialogs and main content before header/footer links.
  It supports `query`, `offset`, `limit`, and `max_chars`. `format:"tree"` retains
  the previous backend tree and its `depth` option. Names longer than 500 characters
  are marked `name_truncated`; matching uses the full name. URLs/refs stay complete.
  Navigation and click/type page views also use this compact format. Consumers
  reading the previous nested page tree should request `format:"tree"` separately.
- Navigation readiness returns `status:ready|pending|target_missing|read_failed`.
  A lost target includes current tabs when available; select a current `bc:ID`.
  `pending` calls for `firefox_wait`, not reload/repeated navigation. A missing
  target before an action is an invalid-parameters error with recovery data.
  Targets are never silently replaced by another tab with a similar URL.
- Standard CSS is validated before click/type, scroll, extraction and waits.
  Invalid selectors such as `button:contains(...)` return an argument error,
  rather than a misleading element timeout. Use role/name lookup instead.
- `firefox_discover_lists` includes an `extract_example` with field selectors
  ready to pass to `firefox_extract`. `price_candidate` is only a heuristic:
  verify units, discounts and availability from the returned text.
- Click/type accept an optional `expect` condition. Conditions use `selector`
  (body by default), `state` (`appeared`, `disappeared`, `text_contains`, or
  `text_changed`), and `text` for `text_contains`. Appearance means DOM presence.
  Text observations use the control value for inputs and visible text otherwise.
  `text_changed` compares against the value observed before the action.
  `postcondition.confirmed` reports the evidence independently of action dispatch.
  An unmet condition returns `pending`; a read failure or lost target is explicit.
  An action without a condition does not confirm the application's outcome.
- `firefox_wait` accepts the same `condition`; `text_changed` additionally requires
  `previous_text`. Condition polling lasts at most five seconds, bounded by the
  overall tool deadline. Inspect or wait again if unconfirmed; do not repeat a
  mutation blindly. Verify the actual cart/form/application state before claiming
  completion. A text match alone is not proof that a purchase or submission succeeded.

```json
{"role":"button","name":"В корзину","exact":true}
```

Pass that to `firefox_find_elements`, then use a unique returned ref:

```json
{"ref":"e583016","expect":{"selector":"#cart-count","state":"text_changed"}}
```

| Environment variable | Default | Purpose |
|---|---|---|
| `FF_RDP_HOST` | `127.0.0.1` | Existing Firefox RDP host |
| `FF_RDP_PORT` | `6000` | RDP TCP port |
| `FF_RDP_TIMEOUT_MS` | `8000` | Maximum time per backend subprocess |
| `FF_RDP_BIN` | `ff-rdp` | Backend for unwrapped/cargo builds; Nix packages force the bundled patched backend |
| `RUST_LOG` | info logging | stderr tracing filter |

If Firefox is unavailable, tools report an error; the server does not launch it.
A status showing version `0.2.0` confirms this MCP version is connected.

## Development and verification

```sh
nix develop
cargo fmt --check
cargo test --locked -j 2
nix flake check --cores 2 --max-jobs 1
```

`flake check` runs Rust tests (including process termination), formatting, fixed
page-script unit fixtures in Node.js, and an offline MCP protocol/schema test.
No browser is required. `tests/mcp_client.py`
uses only the Python standard library.

With the existing local shared Firefox running, explicitly opt into live tests:

```sh
nix build --cores 2 --max-jobs 1
nix develop --command python3 tests/browser_smoke.py ./result/bin/firefox-rdp-mcp
```

The test creates local HTTP fixture tabs, verifies their navigation/readiness,
extraction, duplicate URL/title handling, actions, deadlines and cancellation,
and closes only its own tabs. It checks that original tab identities/URLs remain
unchanged. `FF_RDP_HOST` must be loopback; `FF_RDP_PORT` can select another local
instance. Avoid manually changing tabs during this test. No browser is launched.

License: [MIT](LICENSE). Backend: [ractive/ff-rdp](https://github.com/ractive/ff-rdp).
