let
  # Nixpkgs revision including a missing `rust-darwin-setup.bash` script in the
  # rustup derivation:
  pinnedNixpkgsSrc = builtins.fetchTarball {
    url = "https://github.com/lschuermann/nixpkgs/archive/95a65bf0a34045b57787676c0c10765604a7ccd7.tar.gz";
    sha256 = "sha256:0j3pyxx6ib4vnpj875j5a5wij2vr9ks8rnywhvhd7m14y9qmpz0f";
  };

in
{ pkgs ? import pinnedNixpkgsSrc {} }:

  pkgs.llvmPackages.stdenv.mkDerivation {
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
    ];

    shellHook = ''
      # Required for rust-bindgen:
      export LIBCLANG_PATH="${pkgs.libclang.lib}/lib"

      # Required for dlopen:
      export LD_LIBRARY_PATH="${pkgs.lib.makeLibraryPath (with pkgs; [
        libsodium csfml freeglut libGL glew libGLU brotli openblas stdenv.cc.cc.lib
      ])}"

      # Required for building Tock boards:
      export OBJDUMP="${pkgs.llvm}/bin/llvm-objdump"
    '';
  }
