{
  self,
  nixpkgs,
  microvm,
}:
let
  inherit (nixpkgs) lib;

  nixos =
    system: modules:
    lib.nixosSystem {
      modules = [
        microvm.nixosModules.microvm
        ./mcp.nix
        ./modules
        ./surfaces.nix
        ./base.nix
        { nixpkgs.hostPlatform = system; }
      ]
      ++ modules;
    };
in
{
  mkRunner =
    {
      system,
      uid,
      kind,
      runtime,
      module,
      extraModules ? [ ],
    }:
    let
      pkgs = nixpkgs.legacyPackages.${system};
      guestAgent = self.packages.${system}.tsoup-guest;

      kinds = {
        headless = import ./headless.nix { inherit guestAgent; };
        graphical = import ./graphical.nix { inherit guestAgent; };
      };

      host = [
        { tsoup = { inherit uid runtime; }; }
        kinds.${kind}
      ]
      ++ extraModules;

      trusted = nixos system host;
      untrusted = nixos system (host ++ [ module ]);

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
    };

  describe =
    {
      system,
      extraModules ? [ ],
    }:
    (nixos system extraModules).config.tsoup.description;
}
