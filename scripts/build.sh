#!/bin/bash
set -e

echo "Building VSCode Bazel Extension..."

# Get the script directory
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
PROJECT_ROOT="$( cd "$SCRIPT_DIR/.." && pwd )"

cd "$PROJECT_ROOT"

# Build Rust LSP server
echo "Building Rust LSP server..."
cd bazel-lsp
cargo build --release

# Create server directory in extension
mkdir -p ../extension/server

# Copy the binary
if [[ "$OSTYPE" == "msys" || "$OSTYPE" == "win32" ]]; then
    cp target/release/bazel-lsp.exe ../extension/server/
else
    cp target/release/bazel-lsp ../extension/server/
fi

# Build TypeScript extension
echo "Building TypeScript extension..."
cd ../extension
npm install
npm run compile

echo "Build complete!"
echo "To package the extension, run: npm run package"