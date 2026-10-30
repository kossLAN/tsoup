{
  self,
  nixpkgs,
  microvm,
}:
{
  system,
  uid,
  kind,
  runtime,
  module,
}:
let
  inherit (nixpkgs) lib;

  pkgs = nixpkgs.legacyPackages.${system};
  guestAgent = self.packages.${system}.tsoup-guest;

  kinds = {
    headless = { };
    graphical = import ./graphical.nix { inherit guestAgent; };
  };

  nixos =
    extraModules:
    lib.nixosSystem {
      modules = [
        microvm.nixosModules.microvm
        (import ./surfaces.nix { inherit runtime; })
        (import ./base.nix { inherit uid guestAgent; })
        kinds.${kind}
        { nixpkgs.hostPlatform = system; }
      ]
      ++ extraModules;
    };

  trusted = nixos [ ];
  untrusted = nixos [ module ];

  storeDisk =
    let
      definitions = untrusted.options.microvm.storeDisk.definitionsWithLocations;
      definedByMicrovm = lib.all (
        { file, ... }: lib.hasPrefix "${microvm}/nixos-modules/microvm/" file
      ) definitions;
    in
    assert lib.assertMsg definedByMicrovm "environment modules must not set microvm.storeDisk";
    untrusted.config.microvm.storeDisk;

  runner = microvm.lib.buildRunner {
    inherit pkgs;
    toplevel = untrusted.config.system.build.toplevel;

    microvmConfig = trusted.config.microvm // {
      inherit (trusted.config.networking) hostName;
      inherit (untrusted.config.microvm) kernelParams;
      inherit storeDisk;
    };
  };

  surfaces = pkgs.writeTextDir "share/tsoup/surfaces.json" (
    builtins.toJSON (lib.attrValues trusted.config.tsoup.surfaces)
  );
in
pkgs.symlinkJoin {
  name = "tsoup-runner";
  paths = [
    runner
    surfaces
  ];
}
