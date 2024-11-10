let
  pkgs = import <nixpkgs> {};

in
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
