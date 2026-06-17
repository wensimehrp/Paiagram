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

        sarasaUiSrc = pkgs.fetchurl {
          url = "https://github.com/be5invis/Sarasa-Gothic/releases/download/v1.0.33/SarasaUiSC-TTF-1.0.33.7z";
          hash = "sha256-2OT2xqTY4Xm2BYTsQihUYt2fxW4LtZD9GHjrTGNM8oE=";
        };

        diaProSrc = pkgs.fetchurl {
          url = "https://github.com/ButTaiwan/diapro/releases/download/v1.200/DiaProV1200.zip";
          hash = "sha256-VSr0PU0szuP2mOVmx+0Dz7vKV/wShrysE21lVKbyGu0=";
        };

        # for building the application
        # cargoToml = fromTOML (builtins.readFile ./Cargo.toml);
      in
      {
        packages.default = pkgs.rustPlatform.buildRustPackage {
          pname = "paiagram";
          version = "0.1.3"; # keep it hardcoded until nix supports toml v1.1
          src = pkgs.lib.cleanSource ./.;
          cargoLock.lockFile = ./Cargo.lock;
          cargoLock.outputHashes = {
            "paiagram-oudia-0.1.2" = "sha256-njaXWjL5xqbZ2fyfDVWZ+egU4ljYawuvXxR93whnV3E=";
          };
          nativeBuildInputs = with pkgs; [
            mold
            pkg-config
            makeWrapper
          ];
          buildInputs = runtimeLibs ++ [ pkgs.openssl ];

          postPatch = ''
            mkdir -p crates/paiagram-ui/assets/fonts
            ${pkgs.p7zip}/bin/7z x ${sarasaUiSrc} -ocrates/paiagram-ui/assets/fonts -y
            ${pkgs.p7zip}/bin/7z x ${diaProSrc} -ocrates/paiagram-ui/assets/fonts -y
          '';

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
