{
  config,
  lib,
  ...
}:
let
  previews = lib.attrValues config.tsoup.previews;

  preview =
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
          default = "${config.tsoup.runtime}/${name}.sock";
          readOnly = true;
        };

        label = lib.mkOption {
          type = lib.types.str;
        };

        port = lib.mkOption {
          type = lib.types.port;
        };

        path = lib.mkOption {
          type = lib.types.strMatching "/.*";
          default = "/";
        };

        priority = lib.mkOption {
          type = lib.types.int;
          default = 0;
        };

        forwarded = lib.mkOption {
          type = lib.types.bool;
          default = false;
        };
      };
    };
in
{
  options.tsoup = {
    runtime = lib.mkOption {
      type = lib.types.str;
      internal = true;
    };

    previews = lib.mkOption {
      type = lib.types.attrsOf (lib.types.submodule preview);
      default = { };
      internal = true;
    };
  };

  config = {
    networking.firewall.allowedTCPPorts = map (preview: preview.port) previews;

    microvm = {
      interfaces = lib.mkForce [ ];

      qemu.extraArgs = lib.mkForce [
        "-netdev"
        (lib.concatStringsSep "," (
          [
            "user"
            "id=net0"
          ]
          ++ map (preview: "hostfwd=unix:${preview.socket}-:${toString preview.port}") previews
        ))
        "-device"
        "virtio-net-pci,netdev=net0,mac=02:00:00:00:00:01,romfile="
      ];
    };
  };
}
