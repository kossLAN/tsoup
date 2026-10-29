{
  inputs.tsoup.url = "tsoup";

  outputs =
    { tsoup, ... }:
    {
      packages = builtins.mapAttrs (system: _: {
        default = tsoup.lib.mkRunner {
          inherit system;
          inherit (builtins.fromJSON (builtins.readFile ./host.json)) uid;
          module = ./module.nix;
        };
      }) tsoup.packages;
    };
}
