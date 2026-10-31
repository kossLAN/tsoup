{
  lib,
  runCommand,
  rustPlatform,
  fetchNpmDeps,
  npmHooks,
  nodejs_24,
  flake,
  extraModules ? [ ],
}:
let
  vmFlake = runCommand "tsoup-flake" { } ''
    cp -r --no-preserve=mode ${flake} $out
    ${lib.concatImapStrings (index: module: ''
      cp -r ${module} $out/nix/vm/modules/extra-${toString index}
    '') extraModules}
  '';

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

  env.TSOUP_FLAKE = "path:${vmFlake}";

  meta.mainProgram = "tsoup";
}
