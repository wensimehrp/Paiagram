default:
    just --list

# Download fonts
get-fonts:
    wget https://github.com/be5invis/Sarasa-Gothic/releases/download/v1.0.33/SarasaUiSC-TTF-1.0.33.7z
    7z x SarasaUiSC-TTF-1.0.33.7z -oassets/fonts -y
    wget https://github.com/be5invis/Sarasa-Gothic/releases/download/v1.0.33/SarasaTermSC-TTF-1.0.33.7z
    7z x SarasaTermSC-TTF-1.0.33.7z -oassets/fonts -y

# Check documentation build
doc-check:
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

# Build HTML and PDF documentation
build-docs:
    typst c docs/index.typ --features html -f html --root .
    typst c docs/index.typ --features html -f pdf --root .

# Prepare Pages artifact
prepare-pages:
    rm -rf web-out
    mkdir -p web-out
    cp -r web/* web-out/
    mkdir -p web-out/nightly/api-docs web-out/nightly/docs
    cp -r target/doc/* web-out/nightly/api-docs/
    cp -r wasm-out/* web-out/nightly/
    cp docs/index.* web-out/nightly/docs/
    cp docs/style.css web-out/nightly/docs/

# Full build for nightly. Make sure to run `get-fonts` before running this target
web-nightly: doc-check build-wasm build-docs prepare-pages
