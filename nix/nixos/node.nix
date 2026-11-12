{
  config,
  lib,
  pkgs,
  ...
}:
let
  cfg = config.services.tsoup-node;

  mcpConfig = pkgs.writeText "tsoup-node-mcp.json" (builtins.toJSON cfg.mcpServers);
in
{
  options.services.tsoup-node = {
    enable = lib.mkEnableOption ''
      a tsoup node, which lets a tsoup server run sessions on this machine.
      It runs as a user service of whoever logs into a graphical session, so sessions
      can do anything that user can and see the session's environment
    '';

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

    dataDir = lib.mkOption {
      type = lib.types.nullOr lib.types.str;
      default = null;
      example = "%h/tsoup-node";
      description = ''
        Where sessions are stored, defaults to `$XDG_DATA_HOME/tsoup-node`.
        `%h` stands for the user's home directory.
      '';
    };

    directory = lib.mkOption {
      type = lib.types.str;
      default = "%h";
      example = "%h/projects";
      description = ''
        The directory sessions work in, each of its folders is a place a session can start.
        It must already exist, `%h` stands for the user's home directory.
      '';
    };

    mount = {
      enable = lib.mkEnableOption ''
        mounting the workspaces of the server's sandbox sessions on this machine,
        one folder for each session
      '';

      directory = lib.mkOption {
        type = lib.types.str;
        default = "%h/tsoup";
        example = "%h/sessions";
        description = ''
          Where the workspaces are mounted, it is created when missing.
          `%h` stands for the user's home directory.
        '';
      };
    };

    sandbox.enable = lib.mkEnableOption ''
      confining sessions with bubblewrap, so that their commands and file tools only reach
      the folder of the session, the system's programs and a home directory and `/tmp` kept
      for that folder under the data directory. The MCP servers stay outside the sandbox
    '';

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
    systemd.user.services.tsoup-node = {
      description = "tsoup node";
      wantedBy = [ "graphical-session.target" ];
      partOf = [ "graphical-session.target" ];
      after = [ "graphical-session.target" ];

      unitConfig.ConditionUser = "!@system";

      path =
        cfg.extraPackages
        ++ lib.optional cfg.sandbox.enable pkgs.bubblewrap
        ++ [
          "/run/wrappers"
          "/etc/profiles/per-user/%u"
          "/run/current-system/sw"
        ];

      environment = {
        TSOUP_SERVER = cfg.server;
        TSOUP_NODE_NAME = cfg.name;
        TSOUP_NODE_DIRECTORY = cfg.directory;
        TSOUP_NODE_DATA_DIR = cfg.dataDir;
        TSOUP_NODE_MOUNT = lib.mkIf cfg.mount.enable cfg.mount.directory;
        TSOUP_NODE_SANDBOX = lib.mkIf cfg.sandbox.enable "true";
        TSOUP_NODE_MCP_CONFIG = lib.mkIf (cfg.mcpServers != { }) mcpConfig;
      };

      serviceConfig = {
        ExecStart = lib.getExe cfg.package;
        Restart = "on-failure";

        WorkingDirectory = cfg.directory;
      };
    };
  };
}
