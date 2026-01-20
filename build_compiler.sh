#!/usr/bin/env bash
set -euo pipefail

ROOT="/home/chris/dev/axis-lang-public"
cd "$ROOT"

echo "=== Axis: Build Complete Toolchain ==="
echo

# Step 1: Build all Rust components
echo "--- Building Rust components ---"
echo "Building axis-core-compiler..."
cd "$ROOT/core-compiler"
cargo build --release --quiet

echo "Building axis-rust-bridge..."
cd "$ROOT/rust-bridge"
cargo build --release --quiet

cd "$ROOT"
echo "✓ Rust components built"
echo
