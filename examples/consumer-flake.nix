# Copy this file to your project's flake.nix and run nix develop.
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
