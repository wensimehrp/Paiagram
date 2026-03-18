default:
    just --list

# Download fonts
get-fonts:
    mkdir -p crates/paiagram-ui/assets/fonts
    wget https://github.com/be5invis/Sarasa-Gothic/releases/download/v1.0.33/SarasaUiSC-TTF-1.0.33.7z
    7z x SarasaUiSC-TTF-1.0.33.7z -ocrates/paiagram-ui/assets/fonts -y
    wget https://github.com/be5invis/Sarasa-Gothic/releases/download/v1.0.33/SarasaTermSC-TTF-1.0.33.7z
    7z x SarasaTermSC-TTF-1.0.33.7z -ocrates/paiagram-ui/assets/fonts -y
    wget https://github.com/ButTaiwan/diapro/releases/download/v1.200/DiaProV1200.zip
    7z x DiaProV1200.zip -ocrates/paiagram-ui/assets/fonts -y

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
    @du -sh target/wasm32-unknown-unknown/release/paiagram.wasm
    @du -s target/wasm32-unknown-unknown/release/paiagram.wasm
    wasm-opt -O4 --all-features --fast-math -o wasm-out/paiagram_bg.wasm wasm-out/paiagram_bg.wasm
    @du -sh wasm-out/paiagram_bg.wasm
    @du -s wasm-out/paiagram_bg.wasm
    split -b 24M -d "wasm-out/paiagram_bg.wasm" "wasm-out/paiagram_bg.wasm."
    rm -f wasm-out/paiagram_bg.wasm

prep-wasm: rust-docs build-wasm
    rm -rf dist/nightly
    mkdir -p dist/nightly
    mkdir -p dist/nightly/api-docs
    cp -r web/nightly/* dist/nightly
    cp -r target/doc/* dist/nightly/api-docs/
    cp -r wasm-out/* dist/nightly/
    cp crates/paiagram-ui/assets/fonts/SarasaUiSC-Regular.ttf dist/nightly/
    git rev-parse HEAD > dist/nightly/git-revision.txt
    cargo about generate about.hbs > dist/nightly/license.html
    # fix for input tools. See https://github.com/wasm-bindgen/wasm-bindgen/pull/5034 for details
    sed -i '/function passStringToWasm0/a\
        arg = arg ?? "";' dist/nightly/paiagram.js

prep-docs:
    rm -rf dist/docs
    mkdir -p dist/docs
    shiroa build docs --mode static-html --path-to-root /docs
    cp -r docs/dist/. dist/docs

prep-main:
    rm -rf dist/main
    mkdir -p dist/main
    cp -r web/main/. dist/main
