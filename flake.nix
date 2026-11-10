{
  description = "Paiagram development flake";

  inputs = {
    nixpkgs.url = "https://channels.nixos.org/nixpkgs-unstable/nixexprs.tar.zst";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = inputs: {
    devShells = builtins.mapAttrs (system: pkgs: {
      default =
        let
          overlays = [ (import inputs.rust-overlay) ];

          pkgs = import inputs.nixpkgs {
            inherit system overlays;
          };

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
        in
        pkgs.mkShell {
          buildInputs =
            with pkgs;
            [
              rustToolchain
              pkg-config
              just
              wget
              p7zip
              binaryen
              cargo-about
              cargo-shear
              cargo-expand
              gitui
              typst
              trunk
              imagemagick
            ]
            ++ runtimeLibs
            ++ [
              mold
              clang
              stdenv.cc.cc
            ];

          RUST_SRC_PATH = "${rustToolchain}/lib/rustlib/src/rust/library";
          LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath (runtimeLibs ++ [ pkgs.stdenv.cc.cc ]);

          # QuickJS (compiled by rquickjs-sys) must be built with a wasm-capable clang.
          CC_wasm32_unknown_unknown = "${pkgs.llvmPackages.clang-unwrapped}/bin/clang";
        };
    }) inputs.nixpkgs.legacyPackages;
  };
}
