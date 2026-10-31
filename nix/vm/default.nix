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
        ./previews.nix
        ./base.nix
        { nixpkgs.hostPlatform = system; }
      ]
      ++ modules;
    };

  catalog = system: (nixos system [ ]).config.tsoup.modules;

  enabled =
    configuration:
    lib.attrNames (lib.filterAttrs (_: module: module.enable) configuration.config.tsoup.modules);
in
{
  mkRunner =
    {
      system,
      uid,
      modules,
      runtime,
      module,
    }:
    let
      pkgs = nixpkgs.legacyPackages.${system};
      available = lib.attrNames (catalog system);
      unknown = lib.subtractLists available modules;

      host.tsoup = {
        inherit uid runtime;
        guestAgent = self.packages.${system}.tsoup-guest;
        modules = lib.genAttrs modules (_: {
          enable = true;
        });
      };

      trusted = nixos system [ host ];
      untrusted = nixos system [
        host
        module
      ];

      checks = [
        {
          assertion = unknown == [ ];
          message = "there are no modules named ${lib.concatStringsSep ", " unknown}, the available modules are ${lib.concatStringsSep ", " available}";
        }
        {
          assertion = lib.all (
            { file, ... }: lib.hasPrefix "${microvm}/nixos-modules/microvm/" file
          ) untrusted.options.microvm.storeDisk.definitionsWithLocations;
          message = "environment modules must not set microvm.storeDisk";
        }
        {
          assertion = enabled untrusted == enabled trusted;
          message = "enable modules with create_vm's modules argument, not in the module";
        }
      ];

      runner = microvm.lib.buildRunner {
        inherit pkgs;
        toplevel = untrusted.config.system.build.toplevel;

        microvmConfig = trusted.config.microvm // {
          inherit (trusted.config.networking) hostName;
          inherit (untrusted.config.microvm) kernelParams storeDisk;
        };
      };

      previews = pkgs.writeTextDir "share/tsoup/previews.json" (
        builtins.toJSON (
          lib.sort (left: right: left.priority > right.priority) (
            lib.attrValues trusted.config.tsoup.previews
          )
        )
      );
    in
    lib.asserts.checkAssertWarn checks [ ] (
      pkgs.symlinkJoin {
        name = "tsoup-runner";
        paths = [
          runner
          previews
        ];
      }
    );

  describe =
    system:
    lib.concatMapAttrsStringSep "\n" (name: module: "- ${name}: ${lib.trim module.description}") (
      catalog system
    );
}
