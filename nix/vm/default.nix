{
  self,
  nixpkgs,
  microvm,
}:
{
  system,
  uid,
  module,
}:
let
  inherit (nixpkgs) lib;

  base = import ./base.nix {
    inherit uid;
    guestAgent = self.packages.${system}.tsoup-guest;
  };

  nixos =
    extraModules:
    lib.nixosSystem {
      modules = [
        microvm.nixosModules.microvm
        base
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
in
microvm.lib.buildRunner {
  pkgs = nixpkgs.legacyPackages.${system};
  toplevel = untrusted.config.system.build.toplevel;

  microvmConfig = trusted.config.microvm // {
    inherit (trusted.config.networking) hostName;
    inherit (untrusted.config.microvm) kernelParams;
    inherit storeDisk;
  };
}
