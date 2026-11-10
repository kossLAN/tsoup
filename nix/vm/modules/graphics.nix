{
  config,
  lib,
  pkgs,
  ...
}:
let
  vncPort = 5900;
in
{
  config = lib.mkMerge [
    {
      tsoup.modules.graphics.description = ''
        a labwc Wayland desktop the user watches live in a Display preview,
        graphical programs you start with shell in the background appear on their screen.
      '';
    }

    (lib.mkIf config.tsoup.modules.graphics.enable {
      tsoup = {
        session = [
          (lib.getExe' pkgs.dbus "dbus-run-session")
          (lib.getExe pkgs.labwc)
          "--startup"
          "${lib.getExe pkgs.wayvnc} 127.0.0.1 ${toString vncPort}"
          "--session"
          (lib.getExe config.tsoup.guestAgent)
        ];

        previews.display = {
          label = "Display";
          port = 6080;
          path = "/vnc.html?autoconnect=true&reconnect=true&resize=remote";
          priority = 10;
        };
      };

      systemd.services.tsoup-display = {
        wantedBy = [ "multi-user.target" ];

        serviceConfig = {
          User = "agent";
          Restart = "always";
          ExecStart = lib.escapeShellArgs [
            (lib.getExe pkgs.python3Packages.websockify)
            "--web"
            "${pkgs.novnc}/share/webapps/novnc"
            (toString config.tsoup.previews.display.port)
            "localhost:${toString vncPort}"
          ];
        };
      };

      environment.sessionVariables = {
        WLR_BACKENDS = "headless";
        WLR_RENDERER = if config.tsoup.gpu then "gles2" else "pixman";
      };

      fonts.enableDefaultPackages = lib.mkForce true;
      hardware.graphics.enable = lib.mkForce true;
      programs.xwayland.enable = lib.mkForce true;

      environment.systemPackages = [ pkgs.foot ];
    })
  ];
}
