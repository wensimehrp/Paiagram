{
  description = "bevy flake";

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

        # Define the runtime dependencies needed by Bevy
        runtimeLibs = with pkgs; [
          vulkan-loader
          xorg.libX11
          xorg.libXcursor
          xorg.libXi
          xorg.libXrandr
          libxkbcommon
          wayland
          libGL
          libudev-zero
          alsa-lib
        ];

        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = [ "rust-src" ];
          targets = [ "x86_64-unknown-linux-gnu" "wasm32-unknown-unknown" ];
        };

      in
      {
        packages.default = pkgs.rustPlatform.buildRustPackage {
          pname = "paiagram";
          version = "0.1.0";
          src = ./.;

          cargoLock.lockFile = ./Cargo.lock;
          nativeBuildInputs = [ pkgs.pkg-config pkgs.makeWrapper ];
          buildInputs = runtimeLibs;
          postInstall = ''
            wrapProgram $out/bin/paiagram \
              --prefix LD_LIBRARY_PATH : "${pkgs.lib.makeLibraryPath runtimeLibs}"
          '';
        };

        devShells.default =
          with pkgs;
          mkShell {
            buildInputs = [
              rustToolchain
              pkg-config
              wasm-bindgen-cli
              typst
              just
              wget
              p7zip
              binaryen
              trunk
              cargo-about
              gitui
            ] ++ runtimeLibs ++ [ mold clang stdenv.cc.cc ];

            RUST_SRC_PATH = "${pkgs.rust.packages.stable.rustPlatform.rustLibSrc}";
            LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath (runtimeLibs ++ [ stdenv.cc.cc ]);
          };
      }
    );
}
