{
  config,
  lib,
  ...
}:
let
  cfg = config.services.tsoup;
in
{
  options.services.tsoup = {
    enable = lib.mkEnableOption "the tsoup server";

    package = lib.mkOption {
      type = lib.types.package;
      description = "The tsoup package to run.";
    };

    hostname = lib.mkOption {
      type = lib.types.str;
      default = "127.0.0.1";
      example = "0.0.0.0";
      description = ''
        The hostname or address to serve the web interface and API on.
        Neither is authenticated, so only expose them to networks you trust.
      '';
    };

    port = lib.mkOption {
      type = lib.types.port;
      default = 7878;
      description = "The port to serve the web interface and API on.";
    };

    openFirewall = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = "Whether to open the port in the firewall.";
    };

    dataDir = lib.mkOption {
      type = lib.types.path;
      default = "/var/lib/tsoup";
      description = "Where settings, sessions and their workspaces are stored.";
    };

    vmModules = lib.mkOption {
      type = lib.types.listOf lib.types.path;
      default = [ ];
      example = lib.literalExpression "[ ./vm/rust.nix ]";
      description = ''
        NixOS modules to import into every session's VM.
        To offer one for sessions to opt into instead, have it set
        `tsoup.modules.<name>.description` and apply the rest of its configuration
        only when `config.tsoup.modules.<name>.enable` is set.
      '';
    };
  };

  config = lib.mkIf cfg.enable {
    users.groups.tsoup = { };

    users.users.tsoup = {
      isSystemUser = true;
      group = "tsoup";
      home = cfg.dataDir;
      extraGroups = [ "kvm" ];
    };

    networking.firewall.allowedTCPPorts = lib.mkIf cfg.openFirewall [ cfg.port ];

    systemd.tmpfiles.settings.tsoup.${cfg.dataDir}.d = {
      user = "tsoup";
      group = "tsoup";
      mode = "0700";
    };

    systemd.services.tsoup = {
      description = "tsoup server";
      wantedBy = [ "multi-user.target" ];
      wants = [ "network-online.target" ];
      after = [ "network-online.target" ];

      path = [ config.nix.package ];

      environment = {
        TSOUP_HOSTNAME = cfg.hostname;
        TSOUP_PORT = toString cfg.port;
        TSOUP_DATA_DIR = cfg.dataDir;
        TSOUP_RUNTIME_DIR = "/run/tsoup";
      };

      serviceConfig = {
        ExecStart = lib.getExe (cfg.package.override { extraModules = cfg.vmModules; });
        User = "tsoup";
        Group = "tsoup";
        Restart = "on-failure";

        RuntimeDirectory = "tsoup";
        RuntimeDirectoryMode = "0700";
        WorkingDirectory = cfg.dataDir;

        NoNewPrivileges = true;
        PrivateTmp = true;
        ProtectHome = true;
        ProtectSystem = "strict";
        ReadWritePaths = [ cfg.dataDir ];
      };
    };
  };
}
