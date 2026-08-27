default:
    just --list

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
