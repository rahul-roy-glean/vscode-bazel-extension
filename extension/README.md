# Bazel Extension for Visual Studio Code

High-performance Bazel support for VSCode with multi-language debugging.

## Features

- **Syntax Highlighting**: Full syntax highlighting for BUILD and .bazel files
- **IntelliSense**: Code completion for Bazel rules and targets
- **Go to Definition**: Navigate to target definitions and imported files
- **Code Lens**: Build, test, and run targets directly from the editor
- **Tree View**: Browse all Bazel targets in your workspace
- **Multi-Language Support**: Integrated support for Go, TypeScript, Python, and Java
- **High Performance**: Rust-based language server for fast parsing and analysis

## Requirements

- Bazel installed and available in PATH
- VSCode 1.80.0 or higher

## Extension Settings

This extension contributes the following settings:

* `bazel.executable`: Path to the Bazel executable (default: "bazel")
* `bazel.buildFlags`: Additional flags to pass to bazel build
* `bazel.testFlags`: Additional flags to pass to bazel test
* `bazel.enableCodeLens`: Enable/disable CodeLens features

## Commands

- `Bazel: Build Target` - Build the current target
- `Bazel: Test Target` - Run tests for the current target
- `Bazel: Run Target` - Run the current target
- `Bazel: Clean` - Clean Bazel build outputs
- `Bazel: Show Dependencies` - Show target dependencies
- `Bazel: Refresh Workspace` - Refresh the workspace

## Known Issues

- Debugging is currently only supported for Go and Python targets

## Release Notes

### 0.1.0

Initial release with core Bazel support features. 