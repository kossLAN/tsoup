{ lib, ... }:
{
  imports = [ ./browser.nix ];

  options.tsoup.description = lib.mkOption {
    type = lib.types.lines;
    default = "";
  };
}
