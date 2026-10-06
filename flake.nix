{
  description = "Paiagram development flake";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    # Pinned to the last nixpkgs whose `trunk` builds
    nixpkgs-trunk.url = "github:NixOS/nixpkgs/56c02bc00adcf003215cc4bd996d6efaf4cff188";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs =
    {
      nixpkgs,
      nixpkgs-trunk,
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

        trunkPkgs = import nixpkgs-trunk { inherit system; };

        rustToolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;

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

        # for building the application
        # cargoToml = fromTOML (builtins.readFile ./Cargo.toml);
      in
      {
        packages.default = pkgs.rustPlatform.buildRustPackage {
          pname = "paiagram";
          version = "0.1.3"; # keep it hardcoded until nix supports toml v1.1
          src = pkgs.lib.cleanSource ./.;
          cargoLock.lockFile = ./Cargo.lock;
          nativeBuildInputs = with pkgs; [
            mold
            pkg-config
            makeWrapper
          ];
          buildInputs = runtimeLibs ++ [ pkgs.openssl ];

          postInstall = ''
            wrapProgram $out/bin/paiagram \
              --prefix LD_LIBRARY_PATH : "${pkgs.lib.makeLibraryPath (runtimeLibs ++ [ pkgs.stdenv.cc.cc ])}"
          '';
        };
        devShells.default =
          with pkgs;
          mkShell {
            buildInputs = [
              rustToolchain
              pkg-config
              openssl # TODO: remove this
              just
              wget
              p7zip
              binaryen
              cargo-about
              cargo-shear
              cargo-expand
              gitui
              typst
              # pinned via the `nixpkgs-trunk` input, see the top of this file.
              trunkPkgs.trunk
              imagemagick
            ]
            ++ runtimeLibs
            ++ [
              mold
              clang
              stdenv.cc.cc
            ];

            RUST_SRC_PATH = "${rustToolchain}/lib/rustlib/src/rust/library";
            LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath (runtimeLibs ++ [ stdenv.cc.cc ]);

            # QuickJS (compiled by rquickjs-sys) must be built with a wasm-capable clang.
            CC_wasm32_unknown_unknown = "${llvmPackages.clang-unwrapped}/bin/clang";
          };
      }
    );
}
