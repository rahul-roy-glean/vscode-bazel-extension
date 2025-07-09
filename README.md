# VSCode Bazel Extension

A high-performance Visual Studio Code extension that provides comprehensive support for Bazel-based development across multiple languages (Go, TypeScript, Python, Java) in monorepos.

## Features

### 🚀 Core Features
- **Multi-Language Support**: Unified development experience for Go, TypeScript, Python, and Java
- **Intelligent Code Navigation**: Bazel-aware go-to-definition, find references, and symbol search
- **Build & Test Integration**: Execute Bazel commands directly from VSCode with visual feedback
- **Debugging Support**: Language-specific debugging with proper source mapping
- **Fast BUILD File Analysis**: Rust-powered parsing for instant feedback

### 🔨 Build System Integration
- Execute `bazel build`, `bazel test`, and `bazel run` commands
- Visual test explorer with inline results
- Support for custom Bazel rules (e.g., `scio_java_test`)
- Streaming build output with error navigation
- Build Event Protocol integration for detailed results

### 🐛 Debugging
- **Go**: Delve integration with Bazel-built binaries
- **Python**: Local & remote debugging support
- **Java**: JVM debugging with automatic port configuration
- **TypeScript**: Chrome DevTools integration

### 📁 Workspace Features
- Bazel target tree view
- BUILD file syntax highlighting and validation
- CodeLens for build/test/debug actions
- Dependency visualization
- File-to-target mapping

## Installation

### From VSCode Marketplace (Coming Soon)
```
ext install askscio.bazel-extension
```

### From Source
```bash
git clone https://github.com/rahul-roy-glean/vscode-bazel-extension
cd vscode-bazel-extension
npm install
npm run compile
npm run package
code --install-extension bazel-extension-0.1.0.vsix
```

## Quick Start

1. Open a Bazel workspace in VSCode
2. The extension will automatically activate when it detects BUILD files
3. Configure your language preferences in settings:

```json
{
  "bazel.executable": "bazel",
  "bazel.buildFlags": ["--config=dev"],
  "bazel.languages.go.enabled": true,
  "bazel.languages.typescript.enabled": true,
  "bazel.languages.python.enabled": true,
  "bazel.languages.java.enabled": true
}
```

## Usage

### Command Palette Commands
- `Bazel: Build Target` - Build the current file's target
- `Bazel: Test Target` - Run tests for the current file
- `Bazel: Debug Test` - Debug the current test
- `Bazel: Show Dependencies` - Visualize target dependencies
- `Bazel: Clean` - Clean build outputs

### Keyboard Shortcuts
- `Cmd+Shift+B` (Mac) / `Ctrl+Shift+B` (Win/Linux) - Build current target
- `Cmd+Shift+T` / `Ctrl+Shift+T` - Test current target
- `F5` - Debug current target

### CodeLens Actions
Click the inline actions above BUILD targets and test files:
- ▶️ Run Test
- 🐛 Debug Test
- 🔨 Build Target

## Configuration

### Basic Settings
```json
{
  "bazel.executable": "/usr/local/bin/bazel",
  "bazel.workspaceRoot": "${workspaceFolder}",
  "bazel.buildFlags": ["--config=dev"],
  "bazel.testFlags": ["--test_output=errors"],
  "bazel.enableCodeLens": true
}
```

### Language-Specific Settings
```json
{
  "bazel.languages.go": {
    "enabled": true,
    "goplsPath": "gopls"
  },
  "bazel.languages.typescript": {
    "enabled": true,
    "tsserverPath": "auto"
  },
  "bazel.languages.python": {
    "enabled": true,
    "interpreter": "auto"
  },
  "bazel.languages.java": {
    "enabled": true,
    "jdtlsPath": "auto"
  }
}
```

### Performance Settings
```json
{
  "bazel.cache.queryResults": true,
  "bazel.cache.ttl": 300,
  "bazel.parallelism": "auto"
}
```

## Architecture

The extension uses a hybrid architecture:
- **TypeScript Extension**: VSCode API integration and UI
- **Rust Language Server**: High-performance BUILD file parsing and Bazel operations
- **Language Server Proxies**: Integration with existing language servers (gopls, tsserver, etc.)

## Requirements

- VSCode 1.80.0 or later
- Bazel 6.0 or later
- Language-specific requirements:
  - Go: gopls installed
  - TypeScript: Node.js 16+
  - Python: Python 3.8+
  - Java: JDK 11+

## Troubleshooting

### Extension Not Activating
- Ensure you have BUILD or BUILD.bazel files in your workspace
- Check the Output panel (View > Output > Bazel) for errors

### Code Navigation Not Working
- Wait for initial workspace indexing to complete
- Check if language servers are running: View > Output > [Language]
- Try clearing the cache: Command Palette > "Bazel: Clear Cache"

### Build Errors
- Verify Bazel is in your PATH: `which bazel`
- Check workspace root is correctly detected
- Review build flags in settings

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for development setup and contribution guidelines.

## License

This project is licensed under the Apache License 2.0 - see the [LICENSE](LICENSE) file for details.

## Acknowledgments

- Built with [tower-lsp](https://github.com/ebkalderon/tower-lsp) for LSP implementation
- Inspired by [rust-analyzer](https://github.com/rust-analyzer/rust-analyzer) architecture
- Uses language servers: [gopls](https://github.com/golang/tools/tree/master/gopls), [typescript-language-server](https://github.com/typescript-language-server/typescript-language-server), [pylsp](https://github.com/python-lsp/python-lsp-server), [eclipse.jdt.ls](https://github.com/eclipse/eclipse.jdt.ls)

## Support

- File issues: [GitHub Issues](https://github.com/rahul-roy-glean/vscode-bazel-extension/issues)
- Documentation: [Wiki](https://github.com/rahul-roy-glean/vscode-bazel-extension/wiki)
- Slack: #vscode-bazel-extension (internal)

