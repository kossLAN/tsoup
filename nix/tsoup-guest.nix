{
  lib,
  rustPlatform,
}:
rustPlatform.buildRustPackage {
  pname = "tsoup-guest";
  version = (lib.importTOML ../Cargo.toml).workspace.package.version;

  src = lib.fileset.toSource {
    root = ../.;
    fileset = lib.fileset.unions [
      ../Cargo.toml
      ../Cargo.lock
      ../crates
    ];
  };

  cargoLock.lockFile = ../Cargo.lock;
  cargoBuildFlags = [
    "-p"
    "tsoup-guest"
  ];
  cargoTestFlags = [
    "-p"
    "tsoup-guest"
  ];

  meta.mainProgram = "tsoup-guest";
}
