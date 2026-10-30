{ guestAgent }:
{
  lib,
  pkgs,
  ...
}:
let
  session = {
    user = "agent";
    command = lib.escapeShellArgs [
      (lib.getExe' pkgs.dbus "dbus-run-session")
      (lib.getExe guestAgent)
    ];
  };
in
{
  services.greetd.settings = {
    initial_session = lib.mkForce session;
    default_session = lib.mkForce session;
  };
}
