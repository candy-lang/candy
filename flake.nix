{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs?ref=nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs =
    {
      self,
      nixpkgs,
      flake-utils,
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs {
          inherit system;
          config = { };
        };
      in
      {
        devShell =
          with pkgs;
          mkShell {
            buildInputs = [
              cairo
              dart
              gcc13
              hyperfine
              nodejs_22
              pkg-config
              sdl3
              vsce
            ];
          };
      }
    );
}
