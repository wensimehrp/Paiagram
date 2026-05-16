{
  description = "Paiagram development flake";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs =
    {
      nixpkgs,
      rust-overlay,
      flake-utils,
      ...
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        overlays = [ (import rust-overlay) ];

        pkgs = import nixpkgs {
          inherit system overlays;
        };

        rustToolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;

        # Define the runtime dependencies needed by egui
        runtimeLibs = with pkgs; [
          vulkan-loader
          libX11
          libXcursor
          libXi
          libXrandr
          libxkbcommon
          wayland
          libGL
          libudev-zero
          alsa-lib
          dbus
        ];

      in
      {
        devShells.default =
          with pkgs;
          mkShell {
            buildInputs = [
              rustToolchain
              pkg-config
              openssl # TODO: remove this
              wasm-bindgen-cli_0_2_114
              just
              wget
              p7zip
              binaryen
              cargo-about
              cargo-shear
              gitui
            ]
            ++ runtimeLibs
            ++ [
              mold
              clang
              stdenv.cc.cc
            ];

            RUST_SRC_PATH = "${rustToolchain}/lib/rustlib/src/rust/library";
            LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath (runtimeLibs ++ [ stdenv.cc.cc ]);
          };
      }
    );
}
