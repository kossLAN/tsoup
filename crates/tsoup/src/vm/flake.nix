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
          inherit (host) uid modules runtime;
          module = ./module.nix;
        };
      }) tsoup.packages;
    };
}
