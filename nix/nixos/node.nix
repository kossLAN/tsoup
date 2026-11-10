{
  config,
  lib,
  pkgs,
  ...
}:
let
  cfg = config.services.tsoup-node;

  defaultDirectory = "${cfg.dataDir}/workspace";
  ownsAccount = cfg.user == "tsoup-node";

  mcpConfig = pkgs.writeText "tsoup-node-mcp.json" (builtins.toJSON cfg.mcpServers);
in
{
  options.services.tsoup-node = {
    enable = lib.mkEnableOption "a tsoup node, which lets a tsoup server run sessions on this machine";

    package = lib.mkOption {
      type = lib.types.package;
      description = "The tsoup-node package to run.";
    };

    server = lib.mkOption {
      type = lib.types.str;
      example = "http://soup:7878";
      description = "The URL of the tsoup server to connect to.";
    };

    name = lib.mkOption {
      type = lib.types.nullOr lib.types.str;
      default = null;
      description = "The name sessions know this machine by, defaults to its hostname.";
    };

    user = lib.mkOption {
      type = lib.types.str;
      default = "tsoup-node";
      example = "alice";
      description = ''
        The user sessions run as, they can do anything this user can.
        The default user is created for you, any other must already exist.
      '';
    };

    group = lib.mkOption {
      type = lib.types.str;
      default = if ownsAccount then "tsoup-node" else "users";
      description = "The group sessions run as.";
    };

    dataDir = lib.mkOption {
      type = lib.types.path;
      default = "/var/lib/tsoup-node";
      description = "Where sessions are stored.";
    };

    directory = lib.mkOption {
      type = lib.types.path;
      default = defaultDirectory;
      example = "/home/alice/projects";
      description = ''
        The directory sessions work in, each of its folders is a place a session can start.
        Any directory but the default must already exist.
      '';
    };

    mcpServers = lib.mkOption {
      type = lib.types.attrsOf (
        lib.types.submodule {
          options.command = lib.mkOption {
            type = lib.types.nonEmptyListOf lib.types.str;
            description = "The program that serves MCP over stdio, followed by its arguments.";
          };
        }
      );
      default = { };
      example = lib.literalExpression ''
        {
          github.command = [
            (lib.getExe pkgs.github-mcp-server)
            "stdio"
          ];
        }
      '';
      description = "The MCP servers to offer sessions.";
    };

    extraPackages = lib.mkOption {
      type = lib.types.listOf lib.types.package;
      default = [ ];
      example = lib.literalExpression "[ pkgs.ripgrep ]";
      description = "Packages to put on the PATH of sessions, ahead of the user's and the system's.";
    };
  };

  config = lib.mkIf cfg.enable {
    users.groups = lib.mkIf ownsAccount { tsoup-node = { }; };

    users.users = lib.mkIf ownsAccount {
      tsoup-node = {
        isSystemUser = true;
        group = cfg.group;
        home = cfg.dataDir;
      };
    };

    systemd.tmpfiles.settings.tsoup-node =
      lib.genAttrs ([ cfg.dataDir ] ++ lib.optional (cfg.directory == defaultDirectory) cfg.directory)
        (_: {
          d = {
            inherit (cfg) user group;
            mode = "0700";
          };
        });

    systemd.services.tsoup-node = {
      description = "tsoup node";
      wantedBy = [ "multi-user.target" ];
      wants = [ "network-online.target" ];
      after = [ "network-online.target" ];

      path = cfg.extraPackages ++ [
        "/run/wrappers"
        "/etc/profiles/per-user/${cfg.user}"
        "/run/current-system/sw"
      ];

      environment = {
        TSOUP_SERVER = cfg.server;
        TSOUP_NODE_NAME = cfg.name;
        TSOUP_NODE_DIRECTORY = cfg.directory;
        TSOUP_NODE_DATA_DIR = cfg.dataDir;
        TSOUP_NODE_MCP_CONFIG = lib.mkIf (cfg.mcpServers != { }) mcpConfig;
      };

      serviceConfig = {
        ExecStart = lib.getExe cfg.package;
        User = cfg.user;
        Group = cfg.group;
        Restart = "on-failure";

        WorkingDirectory = cfg.directory;
      };
    };
  };
}
