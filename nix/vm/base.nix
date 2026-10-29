{
  uid,
  guestAgent,
}:
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
      (lib.getExe' pkgs.kdePackages.kwin "kwin_wayland")
      "--drm"
      "--no-lockscreen"
      "--xwayland"
      "--exit-with-session=${lib.getExe guestAgent}"
    ];
  };
in
{
  system.stateVersion = lib.mkForce lib.trivial.release;
  networking.hostName = lib.mkForce "tsoup";
  documentation.enable = lib.mkForce false;
  fonts.enableDefaultPackages = lib.mkForce true;

  microvm = {
    hypervisor = lib.mkForce "qemu";
    mem = lib.mkForce 4096;
    vcpu = lib.mkForce 4;
    socket = lib.mkForce null;

    interfaces = lib.mkForce [
      {
        type = "user";
        id = "net0";
        mac = "02:00:00:00:00:01";
      }
    ];

    shares = lib.mkForce [
      {
        proto = "9p";
        tag = "workspace";
        source = "workspace";
        mountPoint = "/workspace";
      }
    ];

    qemu.extraArgs = lib.mkForce [
      "-vnc"
      "unix:vnc.sock"
      "-device"
      "virtio-gpu-pci"
      "-device"
      "virtio-keyboard-pci"
      "-device"
      "virtio-tablet-pci"
      "-chardev"
      "socket,id=agent,path=agent.sock"
      "-device"
      "virtio-serial-pci"
      "-device"
      "virtserialport,chardev=agent,name=tsoup.agent"
    ];
  };

  boot.kernelModules = [ "virtio_gpu" ];

  users.users.agent = {
    isNormalUser = lib.mkForce true;
    uid = lib.mkForce uid;
    extraGroups = [
      "wheel"
      "video"
      "input"
    ];
  };

  security.sudo.wheelNeedsPassword = lib.mkForce false;

  services.udev.extraRules = ''
    SUBSYSTEM=="virtio-ports", ATTR{name}=="tsoup.agent", OWNER="agent"
  '';

  services.greetd = {
    enable = lib.mkForce true;
    settings = {
      initial_session = lib.mkForce session;
      default_session = lib.mkForce session;
    };
  };

  hardware.graphics.enable = lib.mkForce true;
  programs.xwayland.enable = lib.mkForce true;

  environment.systemPackages = [
    pkgs.foot
    pkgs.kdePackages.breeze
  ];
}
