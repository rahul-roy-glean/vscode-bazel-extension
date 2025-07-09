# VSCode Bazel Extension - Quick Reference

## Summary

A comprehensive VSCode extension that provides unified build, test, debug, and code navigation support for all languages (Go, TypeScript, Python, Java) in the Scio/Glean Bazel monorepo. This eliminates the need to switch between multiple IDEs.

## Key Features

### 🔨 Build & Test
- Execute Bazel build/test commands directly from VSCode
- Visual test explorer with inline results
- Support for custom Scio rules (`scio_java_test`, etc.)
- Streaming build output with error navigation

### 🐛 Debugging
- **Go**: Delve integration with Bazel-built binaries
- **Python**: Local & remote debugging (DevDock port 4329)
- **Java**: JVM debug on ports 5005-5010 (per service)
- **TypeScript**: Chrome DevTools integration

### 🔍 Code Navigation
- Accurate go-to-definition across Bazel targets
- Cross-language navigation (e.g., proto files)
- Symbol search that understands build dependencies
- File-to-target mapping

### 📁 Workspace Features
- Bazel target tree view
- BUILD file syntax highlighting
- CodeLens for build/test/debug actions
- Dependency visualization

## Quick Start

### Installation
```bash
# Clone the extension repo (separate from Scio)
git clone https://github.com/askscio/vscode-bazel-extension
cd vscode-bazel-extension
npm install
npm run compile

# Install in VSCode
code --install-extension ./vscode-bazel-extension-0.1.0.vsix
```

### Basic Configuration
```json
// .vscode/settings.json
{
  "bazel.executable": "bazel",
  "bazel.buildFlags": ["--config=dev"],
  "bazel.languages.go.enabled": true,
  "bazel.languages.typescript.enabled": true,
  "bazel.languages.python.enabled": true,
  "bazel.languages.java.enabled": true
}
```

## Common Commands

### Command Palette
- `Bazel: Build Target` - Build current file's target
- `Bazel: Test Target` - Run tests for current file
- `Bazel: Debug Test` - Debug current test
- `Bazel: Show Dependencies` - View target dependencies
- `Bazel: Clean` - Clean build outputs

### Keyboard Shortcuts
- `Cmd+Shift+B` - Build current target
- `Cmd+Shift+T` - Test current target
- `F5` - Debug current target

## DevDock Integration

### Python Debugging with DevDock
```json
// .vscode/launch.json
{
  "name": "Python: Attach to DevDock QP",
  "type": "python",
  "request": "attach",
  "connect": {
    "host": "localhost",
    "port": 4329  // QP service
  },
  "pathMappings": [{
    "localRoot": "${workspaceFolder}/python_scio",
    "remoteRoot": "/app/qp/query_parser.runfiles/com_github_askscio_scio/python_scio"
  }]
}
```

### Service Debug Ports
| Service | Language | Port | Profile Flag |
|---------|----------|------|--------------|
| QE | Go | 2345 | `DEBUG_MODE: 'True'` |
| QP | Python | 4329 | `DEBUG_MODE: 'True'` |
| Admin | Java | 5005 | Default enabled |
| DocBuilder | Java | 5006 | Default enabled |

## Language-Specific Tips

### Go
- Exclude `bazel-*` directories in settings
- gopls configured automatically for Bazel
- Custom linters run on save

### TypeScript
- Path mappings resolved from tsconfig.json
- Webpack dev server integration
- Support for both web and extension code

### Python
- Uses Bazel's hermetic Python (3.10)
- Virtual env: `bazel-bin/python_scio/scio_env/`
- Requirements managed by Bazel

### Java
- Classpath built from Bazel query
- Maven dependencies resolved
- Support for generated sources

## Troubleshooting

### Code Navigation Not Working
1. Check if BUILD files are parsed: View > Output > Bazel
2. Verify language servers are running: View > Output > [Language]
3. Clear cache: Command Palette > "Bazel: Clear Cache"

### Build Errors
1. Ensure Bazel is in PATH: `which bazel`
2. Check workspace root is correct
3. Verify you're on a compatible Bazel version (6.0+)

### Debug Issues
1. Build target with debug symbols first
2. Check debug ports aren't already in use
3. For DevDock: ensure service has `DEBUG_MODE: 'True'`

## Performance Tips

1. **Limit Scope**: Use folder-specific workspaces for large areas
2. **Disable Unused Languages**: Turn off providers you don't need
3. **Cache Settings**: Increase TTL for stable codebases
4. **Background Indexing**: Let initial indexing complete

## Extension Development

### Running Locally
```bash
# Watch mode for development
npm run watch

# Run extension in new VSCode window
F5 (in VSCode)

# Run tests
npm test
```

### Key Files
- `src/extension.ts` - Entry point
- `src/bazel/client.ts` - Bazel CLI wrapper
- `src/languages/*/provider.ts` - Language providers
- `package.json` - Extension manifest

## Related Resources

- [Bazel Build](https://bazel.build/)
- [DevDock README](devdock/README.md)
- [Scio Development Guide](.codeagent/development_guide.md)
- Extension Repository: `https://github.com/askscio/vscode-bazel-extension` (to be created)

## Future Enhancements

- Remote development support
- AI-powered code suggestions
- Bazel query builder UI
- Performance profiling integration
- Custom rule generators

## Feedback & Support

- File issues in the extension repository
- Slack: #vscode-bazel-extension (to be created)
- Documentation: Internal wiki page (to be created)