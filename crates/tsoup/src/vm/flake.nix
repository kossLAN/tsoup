{
  inputs.tsoup.url = "tsoup";

  outputs =
    { tsoup, ... }:
    let
      host = builtins.fromJSON (builtins.readFile ./host.json);
    in
    {
      packages = builtins.mapAttrs (system: _: {
        default = tsoup.lib.mkRunner {
          inherit system;
          inherit (host) uid kind runtime;
          module = ./module.nix;
          extraModules = map (
            module: (builtins.fetchTree ({ type = "path"; } // module)).outPath
          ) host.extraModules;
        };
      }) tsoup.packages;
    };
}
