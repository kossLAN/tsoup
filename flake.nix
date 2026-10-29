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

      eachSystem = f: lib.genAttrs systems (system: f nixpkgs.legacyPackages.${system});
    in
    {
      packages = eachSystem (pkgs: rec {
        default = tsoup;
        tsoup = pkgs.callPackage ./nix/tsoup.nix { flake = self; };
        tsoup-guest = pkgs.callPackage ./nix/tsoup-guest.nix { };
      });

      lib.mkRunner = import ./nix/vm { inherit self nixpkgs microvm; };

      devShells = eachSystem (pkgs: {
        default = import ./shell.nix { inherit pkgs; };
      });

      formatter = eachSystem (pkgs: pkgs.nixfmt-tree);
    };
}
