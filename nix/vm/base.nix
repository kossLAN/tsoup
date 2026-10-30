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
    command = lib.escapeShellArgs config.tsoup.session;
  };
in
{
  options.tsoup.session = lib.mkOption {
    type = lib.types.listOf lib.types.str;
    internal = true;
  };

  config = {
    tsoup.session = lib.mkDefault [
      (lib.getExe' pkgs.dbus "dbus-run-session")
      (lib.getExe guestAgent)
    ];

    tsoup.surfaces = {
      terminal = {
        label = "Terminal";
        protocol = "http";
        port = 7681;
      };

      website = {
        label = "Website";
        protocol = "http";
        port = 7680;
        forwarded = true;
      };
    };

    system.stateVersion = lib.mkForce lib.trivial.release;
    networking.hostName = lib.mkForce "tsoup";
    documentation.enable = lib.mkForce false;

    microvm = {
      hypervisor = lib.mkForce "qemu";
      mem = lib.mkForce 4096;
      vcpu = lib.mkForce 4;
      socket = lib.mkForce null;

      shares = lib.mkForce [
        {
          proto = "9p";
          tag = "workspace";
          source = "workspace";
          mountPoint = "/workspace";
        }
      ];

      qemu.extraArgs = lib.mkForce [
        "-chardev"
        "socket,id=agent,path=agent.sock"
        "-device"
        "virtio-serial-pci"
        "-device"
        "virtserialport,chardev=agent,name=tsoup.agent"
      ];
    };

    users.users.agent = {
      isNormalUser = lib.mkForce true;
      uid = lib.mkForce uid;
      extraGroups = [ "wheel" ];
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

    systemd.services.tsoup-terminal = {
      wantedBy = [ "multi-user.target" ];
      unitConfig.RequiresMountsFor = "/workspace";

      serviceConfig = {
        User = "agent";
        WorkingDirectory = "/workspace";
        Restart = "always";
        ExecStart = lib.escapeShellArgs [
          (lib.getExe pkgs.ttyd)
          "--writable"
          "--port"
          (toString config.tsoup.surfaces.terminal.port)
          "--client-option"
          "disableLeaveAlert=true"
          (lib.getExe pkgs.bashInteractive)
          "--login"
        ];
      };
    };
  };
}
