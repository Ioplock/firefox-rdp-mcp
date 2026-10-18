# Pinned ff-rdp backend (Firefox DevTools RDP CLI).
#
# Upstream: https://github.com/ractive/ff-rdp
# Crate: ff-rdp-cli 0.4.1 (crates.io, published 2026-10-05).
# The crate depends on ff-rdp-core from crates.io as well,
# so fetchCrate + buildRustPackage is self-contained and reproducible.
#
# CLI surface used by firefox-rdp-mcp (verified from upstream README):
#   global flags: --host, --port, --tab, --jq, --timeout, --format json
#   tabs | navigate <url> | back | forward | reload [--hard]
#   snapshot [--query] | a11y summary [--query] | page-text [--full --query]
#   dom <selector> [--text --attrs] | click (--ref eN | <selector>)
#   type (--ref eN | <selector>) <text> [--clear --submit]
#   wait | eval <js> | console [--level --pattern --limit]
#   network [--filter --method --source] | cookies [--name]
#   storage <local|session> [--key] | screenshot [--output --full-page]
#   computed <selector> [--prop --all] | perf [--type] | vitals
#   sources [--filter --pattern] | inspect <expr> | doctor
#
# Note: ff-rdp is a STATELESS CLI. Every command opens its own RDP
# connection, does its work and disconnects. There is no background
# daemon or state file, so no daemon management is needed.

{
  lib,
  rustPlatform,
  fetchCrate,
}:

rustPlatform.buildRustPackage rec {
  pname = "ff-rdp-cli";
  version = "0.4.1";

  src = fetchCrate {
    inherit pname version;
    hash = "sha256-KdV8YkdissL4xj04oFJKw9f5GSNRPokg9ZR97uVYPGQ=";
  };

  patches = [ ./patches/ff-rdp-context.patch ];

  cargoHash = "sha256-RZyn4SeLCFq/zJWtiw1vJYT3QXVtm4WHm9Oe+YmPo8c=";

  # Upstream tests need a live Firefox / network; skip in sandbox.
  doCheck = false;

  meta = with lib; {
    description = "CLI for the Firefox Remote Debugging Protocol";
    homepage = "https://github.com/ractive/ff-rdp";
    license = licenses.mit;
    mainProgram = "ff-rdp";
  };
}
