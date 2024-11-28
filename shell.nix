let
  # Nixpkgs revision including a missing `rust-darwin-setup.bash` script in the
  # rustup derivation:
  pinnedNixpkgsSrc = builtins.fetchTarball {
    url = "https://github.com/NixOS/nixpkgs/archive/3380c823c9963e39f7d90582e90ec6fb1c67b296.tar.gz";
    sha256 = "sha256:1b4sdanbm7qa5wxsc6dwysnrsw4x4p5ygc1rxnqfx0pasjvgnafp";
  };

in
{ pkgs ? import pinnedNixpkgsSrc {} }:

  pkgs.llvmPackages.stdenv.mkDerivation rec {
    name = "encapfn-mpk-devshell";

    buildInputs = with pkgs; [
      # Base dependencies
      rustup clang pkg-config

      # Dependencies of the libsodium tests:
      libsodium

      # Dependencies of the sfml tests:
      csfml freeglut libGL.dev glew

      # Dependencies of the tinyfiledialog tests (other alternatives can work as well):
      kdialog

      # Dependencies of the brotli test:
      brotli

      # Dependencies of the OpenBLAS test:
      openblas


      # Dependencies for building Tock and the EF bindings / libraries in there:
      clang llvm qemu

      # Development tools:
      gdb
    ];

    shellHook = ''
      # Required for rust-bindgen:
      export LIBCLANG_PATH="${pkgs.libclang.lib}/lib"

      # Required for dlopen:
      export LD_LIBRARY_PATH="${pkgs.lib.makeLibraryPath buildInputs}"

      # Required for building Tock boards:
      export OBJDUMP="${pkgs.llvm}/bin/llvm-objdump"
    '';
  }
