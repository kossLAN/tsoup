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
          memory = host.memory or 4096;
          cpus = host.cpus or 4;
          gpu = host.gpu or false;
          module = ./module.nix;
        };
      }) tsoup.packages;
    };
}
