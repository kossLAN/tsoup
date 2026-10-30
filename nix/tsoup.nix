{
  lib,
  stdenv,
  rustPlatform,
  fetchNpmDeps,
  npmHooks,
  nodejs_24,
  flake,
  extraModules ? [ ],
}:
let
  web = lib.fileset.difference ../web (
    lib.fileset.unions [
      (lib.fileset.maybeMissing ../web/node_modules)
      (lib.fileset.maybeMissing ../web/.svelte-kit)
      (lib.fileset.maybeMissing ../web/build)
    ]
  );
in
rustPlatform.buildRustPackage {
  pname = "tsoup";
  version = (lib.importTOML ../Cargo.toml).workspace.package.version;

  src = lib.fileset.toSource {
    root = ../.;
    fileset = lib.fileset.unions [
      ../Cargo.toml
      ../Cargo.lock
      ../crates
      web
    ];
  };

  cargoLock.lockFile = ../Cargo.lock;
  cargoBuildFlags = [
    "-p"
    "tsoup"
  ];
  cargoTestFlags = [
    "-p"
    "tsoup"
  ];

  npmRoot = "web";
  npmDeps = fetchNpmDeps {
    name = "tsoup-web-npm-deps";
    src = lib.fileset.toSource {
      root = ../web;
      fileset = ../web/package-lock.json;
    };
    hash = "sha256-RITEmnTTVCxpI3T8QPJxSqnEAZ8GnGROWzzS98pqNjE=";
  };

  nativeBuildInputs = [
    nodejs_24
    npmHooks.npmConfigHook
  ];

  env = {
    TSOUP_FLAKE = "path:${flake}";
    TSOUP_EXTRA_MODULES = builtins.toJSON extraModules;
    TSOUP_DESCRIPTION = flake.lib.describe {
      inherit (stdenv.hostPlatform) system;
      inherit extraModules;
    };
  };

  meta.mainProgram = "tsoup";
}
