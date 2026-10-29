{
  pkgs ? import <nixpkgs> { },
}:
pkgs.mkShell {
  nativeBuildInputs = with pkgs; [
    rustc
    cargo
    clippy
    rustfmt
    pkg-config
    nodejs_24
  ];
}
