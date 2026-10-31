{ lib, ... }:
{
  imports = map (name: ./${name}) (
    lib.attrNames (removeAttrs (builtins.readDir ./.) [ "default.nix" ])
  );

  options.tsoup.modules = lib.mkOption {
    type = lib.types.attrsOf (
      lib.types.submodule {
        options = {
          enable = lib.mkEnableOption "the module";

          description = lib.mkOption {
            type = lib.types.lines;
          };
        };
      }
    );
    default = { };
    internal = true;
  };
}
