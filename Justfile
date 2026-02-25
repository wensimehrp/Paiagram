default:
    just --list

# Download fonts
get-fonts:
    wget https://github.com/be5invis/Sarasa-Gothic/releases/download/v1.0.33/SarasaUiSC-TTF-1.0.33.7z
    7z x SarasaUiSC-TTF-1.0.33.7z -oassets/fonts -y
    wget https://github.com/be5invis/Sarasa-Gothic/releases/download/v1.0.33/SarasaTermSC-TTF-1.0.33.7z
    7z x SarasaTermSC-TTF-1.0.33.7z -oassets/fonts -y
    wget https://github.com/ButTaiwan/diapro/releases/download/v1.200/DiaProV1200.zip
    7z x DiaProV1200.zip -oassets/fonts -y

# Build rust docs
rust-docs:
    cargo doc --workspace --no-deps --release

# Build WASM binary
build-wasm:
    cargo build --release --target wasm32-unknown-unknown
    wasm-bindgen \
        --out-dir wasm-out \
        --out-name paiagram \
        --target web \
        --no-typescript \
        target/wasm32-unknown-unknown/release/paiagram.wasm
    wasm-opt -O4 --all-features --fast-math -o wasm-out/paiagram_bg.wasm wasm-out/paiagram_bg.wasm

prep-docs:
    shiroa build docs --mode static-html
    rm -rf dist/nightly/docs
    mkdir -p dist/nightly/docs
    cp -r docs/dist/. dist/nightly/docs

prep-wasm: rust-docs build-wasm
    mkdir -p dist
    rm -rf dist/nightly
    cp -r web/* dist
    mkdir -p dist/nightly/api-docs
    cp -r web/nightly/* dist/nightly
    cp -r target/doc/* dist/nightly/api-docs/
    cp -r wasm-out/* dist/nightly/

nightly-build: prep-wasm prep-docs
