#!/usr/bin/env bash
set -euo pipefail

if [ ! -f "rust/Cargo.toml" ] && [ ! -d ".git" ]; then
  echo "ERROR: no typical file detected（rust/Cargo.toml or .git）。"
  echo "Make sure you ar running this script under the root path of this project DataSpringFlow ！"
  exit 1
fi

if [[ "${0}" != "./scripts/release.sh" && "${0}" != "scripts/release.sh" ]]; then
  echo "ERROR: don't enter dir \`scripts\` to run this script！"
  echo "Run this script under the root path of this project: ./scripts/release.sh"
  exit 1
fi

DIST_DIR="dist_payload"
rm -rf "$DIST_DIR" payload.tar.gz ./release/dsf_installer.sh
mkdir -p "$DIST_DIR"

echo " [Step 1] Starting manylinux container and compile..."

mkdir -p .cargo_cache/rustup .cargo_cache/cargo rust/target_container

podman run --rm \
  -v "$(pwd)":/io:z \
  -v "$(pwd)/.cargo_cache/rustup:/root/.rustup:z" \
  -v "$(pwd)/.cargo_cache/cargo:/root/.cargo:z" \
  quay.io/pypa/manylinux_2_28_x86_64 bash -c "
    set -euo pipefail
    cd /io
   
    export CARGO_TARGET_DIR=\"/io/rust/target_container_release\"

    if ! command -v cargo &>/dev/null; then
        echo '--> Installing Rust toolchain (First time only)...'
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
        source /root/.cargo/env
    fi
    
    echo '--> Compling dsf binary...'
    cd rust
    cargo build --release --bin dsf
    cd ..

    echo '--> Compling manylinux Wheel...'
    /opt/python/cp311-cp311/bin/pip install --quiet maturin
    
    /opt/python/cp311-cp311/bin/maturin build \
        --interpreter /opt/python/cp311-cp311/bin/python \
        --release \
        --strip \
        --manylinux manylinux_2_28
"

echo "[Step 2] Collecting artifacts..."
# 顺便改一下这里的复制路径，直接去我们专属的 target_container 目录拿
echo "[Step 2] Collecting artifacts..."
cp "rust/target_container_release/release/dsf" "$DIST_DIR/"
cp rust/target_container_release/wheels/*.whl "$DIST_DIR/"
cp README.md LICENSE "$DIST_DIR/" || true

echo "[Step 3] Packing installer script..."
tar -czf payload.tar.gz -C "$DIST_DIR" .
mkdir -p release
cat scripts/install_payload.sh payload.tar.gz >./release/dsf_installer.sh
chmod +x ./release/dsf_installer.sh
rm -rf "$DIST_DIR" payload.tar.gz
echo "Finished building installer: ./release/dsf_installer.sh"
