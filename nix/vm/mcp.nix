{
  config,
  lib,
  ...
}:
{
  options.tsoup.mcp = lib.mkOption {
    type = lib.types.attrsOf (
      lib.types.submodule {
        options.command = lib.mkOption {
          type = lib.types.listOf lib.types.str;
        };
      }
    );
    default = { };
    internal = true;
  };

  config.environment.etc."tsoup/mcp.json".text = builtins.toJSON config.tsoup.mcp;
}
