{
  config,
  lib,
  pkgs,
  ...
}:
{
  config = lib.mkMerge [
    {
      tsoup.modules.browser.description = ''
        a Website preview and a headless browser you drive over MCP.
        Start a web server in the background with shell, call show_preview with preview website
        and the server's port to show it to the user, and open its localhost URL in the browser
        to check it yourself.
      '';
    }

    (lib.mkIf config.tsoup.modules.browser.enable {
      tsoup = {
        previews.website = {
          label = "Website";
          port = 7680;
          priority = 20;
          forwarded = true;
        };

        mcp.browser.command = [
          (lib.getExe pkgs.playwright-mcp)
          "--headless"
          "--output-dir"
          "/tmp/playwright-mcp"
        ];
      };
    })
  ];
}
