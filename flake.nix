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

        # We pull in the nightly rustfmt specifically to get access to unstable formatting features
        # like `imports_granularity` while keeping the rest of our toolchain completely stable.
        nightlyRustfmt = pkgs.rust-bin.nightly.latest.rustfmt;

        wasm-bindgen-cli-custom = pkgs.rustPlatform.buildRustPackage rec {
          pname = "wasm-bindgen-cli";
          version = "0.2.122";
          src = pkgs.fetchCrate {
            inherit pname version;
            hash = "sha256-vO4RSxi/sMWxmsEs3GuljdMfIRSu75A+Q+c5wgYToRU=";
          };
          cargoHash = "sha256-Inup6vvJSG5ghNyeDPyZbfZo4d0LsMG2OJfStoaeDBs=";
          doCheck = false;
        };

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
              wasm-bindgen-cli-custom
              just
              wget
              p7zip
              binaryen
              cargo-about
              cargo-shear
              cargo-expand
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

            # Prepend nightly rustfmt to the PATH so that `cargo fmt` and `rust-analyzer`
            # pick it up instead of the stable one.
            shellHook = ''
              export PATH="${nightlyRustfmt}/bin:$PATH"
            '';
          };
      }
    );
}
