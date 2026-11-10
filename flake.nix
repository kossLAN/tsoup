{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

    microvm = {
      url = "github:microvm-nix/microvm.nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      microvm,
    }:
    let
      inherit (nixpkgs) lib;

      systems = [
        "x86_64-linux"
        "aarch64-linux"
      ];

      eachSystem =
        f:
        lib.genAttrs systems (
          system:
          f (
            import nixpkgs {
              inherit system;
              config.allowUnfreePredicate = package: lib.getName package == "claude-code";
            }
          )
        );
    in
    {
      packages = eachSystem (pkgs: rec {
        default = tsoup;
        tsoup = pkgs.callPackage ./nix/tsoup.nix { flake = self; };
        tsoup-guest = pkgs.callPackage ./nix/tsoup-guest.nix { };
        tsoup-node = pkgs.callPackage ./nix/tsoup-node.nix { };
      });

      lib = import ./nix/vm { inherit self nixpkgs microvm; };

      nixosModules.default = import ./nix/nixos self;

      devShells = eachSystem (pkgs: {
        default = import ./shell.nix { inherit pkgs; };
      });

      formatter = eachSystem (pkgs: pkgs.nixfmt-tree);
    };
}
