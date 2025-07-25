#!/bin/bash
set -e

echo "Packaging VSCode Bazel Extension..."

# Get the script directory
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
PROJECT_ROOT="$( cd "$SCRIPT_DIR/.." && pwd )"

cd "$PROJECT_ROOT"

# Build everything first
echo "Building extension..."
bash scripts/build.sh

# Build for multiple platforms
echo "Building Rust LSP server for multiple platforms..."
cd lsp

# Build for current platform
cargo build --release

# Create platform-specific directories
mkdir -p ../extension/server/darwin-arm64
cp target/release/bazel-lsp ../extension/server/darwin-arm64/

# Package the extension
echo "Creating VSIX package..."
cd ../extension

# Install vsce if not already installed
if ! command -v vsce &> /dev/null; then
    echo "Installing vsce..."
    npm install -g @vscode/vsce
fi

# Package the extension
vsce package

echo "Package complete!"
echo "The VSIX file is located in the extension directory" 