{ runtime }:
{
  config,
  lib,
  ...
}:
let
  surfaces = lib.attrValues config.tsoup.surfaces;
  http = lib.filter (surface: surface.protocol == "http") surfaces;
  vnc = lib.filter (surface: surface.protocol == "vnc") surfaces;

  surface =
    { name, ... }:
    {
      options = {
        name = lib.mkOption {
          type = lib.types.str;
          default = name;
          readOnly = true;
        };

        socket = lib.mkOption {
          type = lib.types.str;
          default = "${runtime}/${name}.sock";
          readOnly = true;
        };

        label = lib.mkOption {
          type = lib.types.str;
        };

        protocol = lib.mkOption {
          type = lib.types.enum [
            "http"
            "vnc"
          ];
        };

        port = lib.mkOption {
          type = lib.types.nullOr lib.types.port;
          default = null;
        };

        forwarded = lib.mkOption {
          type = lib.types.bool;
          default = false;
        };
      };
    };
in
{
  options.tsoup.surfaces = lib.mkOption {
    type = lib.types.attrsOf (lib.types.submodule surface);
    default = { };
    internal = true;
  };

  config = {
    assertions = map (surface: {
      assertion = surface.protocol != "http" || surface.port != null;
      message = "the http surface ${surface.name} needs a port";
    }) surfaces;

    networking.firewall.allowedTCPPorts = map (surface: surface.port) http;

    microvm = {
      interfaces = lib.mkForce [ ];

      qemu.extraArgs = lib.mkForce (
        [
          "-netdev"
          (lib.concatStringsSep "," (
            [
              "user"
              "id=net0"
            ]
            ++ map (surface: "hostfwd=unix:${surface.socket}-:${toString surface.port}") http
          ))
          "-device"
          "virtio-net-pci,netdev=net0,mac=02:00:00:00:00:01,romfile="
        ]
        ++ lib.concatMap (surface: [
          "-vnc"
          "unix:${surface.socket}"
        ]) vnc
      );
    };
  };
}
