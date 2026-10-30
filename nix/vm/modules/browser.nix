{
  config,
  lib,
  pkgs,
  ...
}:
{
  options.tsoup.browser.enable = lib.mkEnableOption "a headless browser the agent drives over MCP";

  config.tsoup = {
    description = ''
      Set tsoup.browser.enable = true; in the create_vm module to get a headless browser,
      open a server's localhost URL in it to check a website yourself.
    '';

    mcp = lib.mkIf config.tsoup.browser.enable {
      browser.command = [
        (lib.getExe pkgs.playwright-mcp)
        "--headless"
        "--output-dir"
        "/tmp/playwright-mcp"
      ];
    };
  };
}
