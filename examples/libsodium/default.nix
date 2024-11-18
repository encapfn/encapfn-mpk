{ pkgs ? import <nixpkgs> {} }:

let
  fenix = import (builtins.fetchTarball {
    url = "https://github.com/nix-community/fenix/archive/e10ba121773f754a30d31b6163919a3e404a434f.tar.gz";
    sha256 = "sha256:042avds56swf77zkd9d7nggr2x3a94jxndgzs8vvvmapv5ry31dl";
  }) {};

  rustToolchain = fenix.fromToolchainFile {
    file = ../../rust-toolchain.toml;
  };

  rustPlatform = pkgs.makeRustPlatform {
    cargo = rustToolchain;
    rustc = rustToolchain;
  };

in

rustPlatform.buildRustPackage {
  pname = "encapfn-mpk-example-libsodium";
  version = "0.1.0";

  src = pkgs.nix-gitignore.gitignoreSource [] ../..;
  buildAndTestSubdir = "examples/libsodium";

  cargoLock = {
    lockFile = ../../Cargo.lock;
    outputHashes = {
      "encapfn-0.1.0" = "sha256-iWgjL4gYXMfUHTIJC0w6aP/GGul+pFK4xrejc+2g0YQ=";
      "bindgen-0.69.4" = "sha256-Cs2wPEH97BATmrh6oNcPUUBRGuQeX8dih9fDAxAeHEQ=";
    };
  };

  nativeBuildInputs = [ pkgs.clang pkgs.autoPatchelfHook ];
  buildInputs = [ pkgs.libsodium pkgs.stdenv.cc.cc.lib ];

  # Required for rust-bindgen:
  LIBCLANG_PATH="${pkgs.libclang.lib}/lib";
}
