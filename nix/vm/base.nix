{
  uid,
  guestAgent,
}:
{
  config,
  lib,
  pkgs,
  ...
}:
let
  session = {
    user = "agent";
    command = lib.getExe config.programs.sway.package;
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

  programs.sway = {
    enable = lib.mkForce true;
    extraPackages = lib.mkForce [ ];
  };

  services.greetd = {
    enable = lib.mkForce true;
    settings = {
      initial_session = lib.mkForce session;
      default_session = lib.mkForce session;
    };
  };

  environment.sessionVariables = {
    WLR_RENDERER = lib.mkForce "pixman";
    WLR_NO_HARDWARE_CURSORS = lib.mkForce "1";
  };

  environment.etc."sway/config".text = lib.mkForce ''
    set $mod Mod1

    output * bg #161616 solid_color
    default_border pixel 1
    default_floating_border pixel 1

    client.focused #78a9ff #78a9ff #161616 #78a9ff #78a9ff
    client.focused_inactive #444444 #1c1c1c #96989b #444444 #444444
    client.unfocused #444444 #161616 #96989b #444444 #444444

    bindsym $mod+Return exec ${lib.getExe pkgs.foot}
    bindsym $mod+q kill
    bindsym $mod+f fullscreen toggle

    exec ${lib.getExe guestAgent}
  '';

  environment.systemPackages = [ pkgs.foot ];
}
