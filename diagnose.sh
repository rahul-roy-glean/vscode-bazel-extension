#!/bin/bash

echo "=== VSCode Bazel Extension Diagnostic ==="
echo

# Check VSCode version
echo "1. VSCode Version:"
code --version 2>/dev/null || echo "VSCode CLI not found"
echo

# Check if extension is installed
echo "2. Checking if Bazel extension is installed:"
code --list-extensions 2>/dev/null | grep -i bazel || echo "No Bazel extension found"
echo

# Check server binary
echo "3. Checking LSP server binary:"
if [ -f "extension/server/bazel-lsp" ]; then
    echo "✓ Binary exists"
    echo "  Size: $(ls -lh extension/server/bazel-lsp | awk '{print $5}')"
    echo "  Permissions: $(ls -l extension/server/bazel-lsp | awk '{print $1}')"
    
    # Test if binary can run
    echo "  Testing binary..."
    timeout 1 ./extension/server/bazel-lsp 2>&1 | head -5 || echo "  Binary test completed"
else
    echo "✗ Binary not found at extension/server/bazel-lsp"
fi
echo

# Check for BUILD/WORKSPACE files
echo "4. Checking for Bazel files in workspace:"
find . -maxdepth 3 \( -name "BUILD" -o -name "BUILD.bazel" -o -name "WORKSPACE" -o -name "WORKSPACE.bazel" \) 2>/dev/null | head -10
echo

# Check package.json
echo "5. Extension configuration:"
if [ -f "extension/package.json" ]; then
    echo "  Publisher: $(grep -o '"publisher": "[^"]*"' extension/package.json | cut -d'"' -f4)"
    echo "  Version: $(grep -o '"version": "[^"]*"' extension/package.json | cut -d'"' -f4)"
    echo "  Main: $(grep -o '"main": "[^"]*"' extension/package.json | cut -d'"' -f4)"
fi
echo

echo "6. Quick fixes to try:"
echo "  a) Reload VSCode: Cmd+Shift+P → 'Developer: Reload Window'"
echo "  b) Check Developer Console: Cmd+Shift+P → 'Developer: Toggle Developer Tools'"
echo "  c) Force activation: Open test/BUILD or run Cmd+Shift+P → 'Bazel: Build Target'"
echo
echo "See DEBUGGING.md for detailed troubleshooting steps." 