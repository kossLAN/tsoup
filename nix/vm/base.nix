{
  config,
  lib,
  pkgs,
  ...
}:
{
  options.tsoup = {
    uid = lib.mkOption {
      type = lib.types.int;
      internal = true;
    };

    memory = lib.mkOption {
      type = lib.types.ints.positive;
      internal = true;
    };

    cpus = lib.mkOption {
      type = lib.types.ints.positive;
      internal = true;
    };

    gpu = lib.mkOption {
      type = lib.types.bool;
      internal = true;
    };

    guestAgent = lib.mkOption {
      type = lib.types.package;
      internal = true;
    };

    session = lib.mkOption {
      type = lib.types.listOf lib.types.str;
      default = [
        (lib.getExe' pkgs.dbus "dbus-run-session")
        (lib.getExe config.tsoup.guestAgent)
      ];
      internal = true;
    };
  };

  config = {
    tsoup.previews.terminal = {
      label = "Terminal";
      port = 7681;
    };

    system.stateVersion = lib.mkForce lib.trivial.release;
    networking.hostName = lib.mkForce "tsoup";
    documentation.enable = lib.mkForce false;

    microvm = {
      hypervisor = lib.mkForce "qemu";
      mem = lib.mkForce config.tsoup.memory;
      vcpu = lib.mkForce config.tsoup.cpus;
      socket = lib.mkForce null;

      graphics = {
        enable = lib.mkForce config.tsoup.gpu;
        backend = lib.mkForce "headless";
      };

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
      isNormalUser = lib.mkForce (config.tsoup.uid >= 1000);
      isSystemUser = lib.mkForce (config.tsoup.uid < 1000);
      uid = lib.mkForce config.tsoup.uid;
      group = lib.mkDefault "users";
      createHome = lib.mkDefault true;
      home = lib.mkDefault "/home/agent";
      homeMode = lib.mkDefault "700";
      useDefaultShell = lib.mkDefault true;
      autoSubUidGidRange = lib.mkDefault true;
      extraGroups = [ "wheel" ];
    };

    security.sudo.wheelNeedsPassword = lib.mkForce false;

    services.udev.extraRules = ''
      SUBSYSTEM=="virtio-ports", ATTR{name}=="tsoup.agent", OWNER="agent"
    '';

    services.greetd = {
      enable = lib.mkForce true;

      settings =
        let
          session = {
            user = "agent";
            command = lib.escapeShellArgs config.tsoup.session;
          };
        in
        {
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
          (toString config.tsoup.previews.terminal.port)
          "--client-option"
          "disableLeaveAlert=true"
          (lib.getExe pkgs.bashInteractive)
          "--login"
        ];
      };
    };
  };
}
