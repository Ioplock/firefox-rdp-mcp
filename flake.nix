{
  description = "MCP server for an existing shared Firefox via DevTools RDP";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    let
      overlay = final: _prev: {
        ff-rdp = final.callPackage ./nix/ff-rdp.nix { };
        firefox-rdp-mcp = final.callPackage ./nix/package.nix { inherit (final) ff-rdp; };
      };
    in
    flake-utils.lib.eachSystem [ "x86_64-linux" "aarch64-linux" ] (system:
      let
        pkgs = import nixpkgs { inherit system; overlays = [ overlay ]; };
      in {
        packages = {
          inherit (pkgs) firefox-rdp-mcp ff-rdp;
          default = pkgs.firefox-rdp-mcp;
        };
        apps.default = {
          type = "app";
          program = pkgs.lib.getExe pkgs.firefox-rdp-mcp;
          meta.description = "Connect an MCP client to an existing Firefox RDP session";
        };
        devShells.default = pkgs.mkShell {
          packages = with pkgs; [ cargo rustc rustfmt clippy python3 ff-rdp ];
          FF_RDP_BIN = "${pkgs.ff-rdp}/bin/ff-rdp";
        };
        checks = {
          package = pkgs.firefox-rdp-mcp;
          formatting = pkgs.runCommand "firefox-rdp-mcp-format" { nativeBuildInputs = [ pkgs.rustfmt ]; } ''
            rustfmt --check --edition 2021 ${./src}/*.rs
            touch $out
          '';
          protocol = pkgs.runCommand "firefox-rdp-mcp-protocol" { nativeBuildInputs = [ pkgs.python3 ]; } ''
            export PYTHONDONTWRITEBYTECODE=1
            python ${./tests}/protocol_test.py ${pkgs.lib.getExe pkgs.firefox-rdp-mcp}
            touch $out
          '';
        };
      }) // { overlays.default = overlay; };
}
