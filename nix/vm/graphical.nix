{ guestAgent }:
{
  lib,
  pkgs,
  ...
}:
{
  tsoup.session = lib.mkForce [
    (lib.getExe' pkgs.dbus "dbus-run-session")
    (lib.getExe' pkgs.kdePackages.kwin "kwin_wayland")
    "--drm"
    "--no-lockscreen"
    "--xwayland"
    "--exit-with-session=${lib.getExe guestAgent}"
  ];

  tsoup.surfaces.display = {
    label = "Display";
    protocol = "vnc";
  };

  fonts.enableDefaultPackages = lib.mkForce true;

  microvm.qemu.extraArgs = lib.mkForce [
    "-device"
    "virtio-gpu-pci"
    "-device"
    "virtio-keyboard-pci"
    "-device"
    "virtio-tablet-pci"
  ];

  boot.kernelModules = [ "virtio_gpu" ];

  users.users.agent.extraGroups = [
    "video"
    "input"
  ];

  hardware.graphics.enable = lib.mkForce true;
  programs.xwayland.enable = lib.mkForce true;

  environment.systemPackages = [
    pkgs.foot
    pkgs.kdePackages.breeze
  ];
}
