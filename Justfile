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
    rm -rf dist
    mkdir -p dist
    cp -r docs/dist dist

prep-wasm: rust-docs build-wasm
    rm -rf dist
    mkdir -p dist
    mkdir -p dist/api-docs
    cp -r web/nightly/* dist
    cp -r target/doc/* dist/api-docs/
    cp -r wasm-out/* dist/
