self:
{ lib, pkgs, ... }:
let
  packages = self.packages.${pkgs.stdenv.hostPlatform.system};
in
{
  imports = [
    ./server.nix
    ./node.nix
  ];

  services.tsoup.package = lib.mkDefault packages.tsoup;
  services.tsoup-node.package = lib.mkDefault packages.tsoup-node;
}
