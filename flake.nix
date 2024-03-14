{
  description = "Development environment for working on the Candy compiler";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs?ref=nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay }:
    let
      overlays = [
        rust-overlay.overlays.default
        (final: prev: {
          rustToolchain =
            prev.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
        })
      ];
    in flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit overlays system;
          config = { };
        };
      in {
        devShell = with pkgs;
          mkShell {
            LLVM_SYS_150_PREFIX = "${pkgs.llvmPackages_15.llvm.dev}";
            RUSTC_WRAPPER = "sccache";
            buildInputs = [
              libffi
              llvmPackages_15.bintools
              llvmPackages_15.clangUseLLVM
              llvmPackages_15.llvm
              rustToolchain
              rust-analyzer
              sccache
            ];
          };
      });
}