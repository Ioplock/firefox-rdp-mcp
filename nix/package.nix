{ lib, rustPlatform, makeWrapper, ff-rdp }:
let
  manifest = builtins.fromTOML (builtins.readFile ../Cargo.toml);
in
rustPlatform.buildRustPackage {
  pname = "firefox-rdp-mcp";
  version = manifest.package.version;
  src = lib.fileset.toSource {
    root = ../.;
    fileset = lib.fileset.unions [ ../Cargo.toml ../Cargo.lock ../src ../README.md ../LICENSE ];
  };
  cargoLock.lockFile = ../Cargo.lock;
  doCheck = true;
  nativeBuildInputs = [ makeWrapper ];
  postInstall = ''
    wrapProgram $out/bin/firefox-rdp-mcp --set FF_RDP_BIN ${ff-rdp}/bin/ff-rdp
  '';
  meta = {
    description = manifest.package.description;
    homepage = "https://github.com/Ioplock/firefox-rdp-mcp";
    license = lib.licenses.mit;
    platforms = lib.platforms.linux;
    mainProgram = "firefox-rdp-mcp";
  };
}
