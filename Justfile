default:
    just --list

download-font:
    wget https://github.com/be5invis/Sarasa-Gothic/releases/download/v1.0.41/SarasaUiCL-TTF-1.0.41.7z -O fonts.7z
    rm -rf fonts
    7z x fonts.7z -ofonts
    rm fonts.7z
    mv fonts/SarasaUiCL-Regular.ttf crates/paiagram/
    rm -r fonts

# Build rust docs
rust-docs:
    rm -rf dist/api-docs
    mkdir -p dist/api-docs
    cargo doc --workspace --no-deps --release --document-private-items
    mv target/doc/* dist/api-docs

watch-wasm-app:
    trunk --config ./crates/paiagram serve --public-url .

# Build WASM binary
build-wasm-app:
    rm -rf dist/app
    trunk --config ./crates/paiagram build --cargo-profile wasm-release -M -d $PWD/dist/app --public-url .

release-wasm-app: build-wasm-app rust-docs
    du -sh target/wasm32-unknown-unknown/wasm-release/paiagram.wasm | sort -hr
    du -sh dist/app/* | sort -hr
