{
  lib,
  rustPlatform,
}:
rustPlatform.buildRustPackage {
  pname = "tsoup-node";
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
    "tsoup-node"
  ];
  cargoTestFlags = [
    "-p"
    "tsoup-node"
  ];

  meta.mainProgram = "tsoup-node";
}
