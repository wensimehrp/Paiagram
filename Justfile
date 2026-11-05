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
build-rust-docs:
    rm -rf dist/api-docs
    mkdir -p dist/api-docs
    cargo doc --workspace --no-deps --release --document-private-items
    mv target/doc/* dist/api-docs

# Build user docs
build-user-docs:
    rm -rf dist/docs
    mkdir -p dist/docs
    ./docs.typ compile

watch-wasm-app:
    trunk --config ./crates/paiagram serve --public-url .

watch-release-wasm-app:
    trunk --config ./crates/paiagram serve --release --public-url .

# Build wasm binary
build-wasm-app:
    rm -rf dist
    trunk \
        --config ./crates/paiagram \
        build --release \
        -d $PWD/dist --public-url .

# Build the wasm release version
build-release-wasm-app: build-wasm-app build-rust-docs build-user-docs
    du -sh target/wasm32-unknown-unknown/release/paiagram.wasm | sort -hr
    du -sh dist/* | sort -hr
